//! Two-party MLS conversation lifecycle and fail-closed message processing.
//!
//! The server-provided group, epoch, suite, and sender fields are routing hints
//! only. This module checks every hint against the locally pinned conversation
//! and MLS-authenticated state before releasing plaintext.

use std::collections::{BTreeMap, HashMap};

use filament_core::{
    CiphersuiteId, ConversationCrypto, DeviceCertificate, DeviceId, GroupId as FilamentGroupId,
    UserId,
};
use openmls::prelude::*;
use tls_codec::Deserialize as TlsDeserialize;

use crate::{
    error::ConversationError,
    identity::verify_device_certificate,
    keypackage::{MlsDevice, DEVICE_CREDENTIAL_DOMAIN},
};

const CIPHERSUITE: Ciphersuite = Ciphersuite::MLS_128_DHKEMX25519_CHACHA20POLY1305_SHA256_Ed25519;
const APPLICATION_ENVELOPE_VERSION: u16 = 1;
const APPLICATION_HEADER_BYTES: usize = 2 + 8 + 4;
const MAX_KEYPACKAGE_BYTES: usize = 4_096;
const MAX_MLS_MESSAGE_BYTES: usize = 65_536;
const MAX_WELCOME_BYTES: usize = 65_536;
const MAX_COMMIT_BYTES: usize = 65_536;
const MAX_GROUP_INFO_BYTES: usize = 65_536;
const OUT_OF_ORDER_TOLERANCE: u32 = 64;
const MAXIMUM_FORWARD_DISTANCE: u32 = 256;

/// Maximum plaintext bytes accepted by the MLS application layer.
pub const MAX_APPLICATION_PLAINTEXT_BYTES: usize = 32_768;

/// Maximum number of missing application generations buffered per sender.
pub const MAX_BUFFERED_GENERATION_GAP: u64 = 64;

/// A root identity pinned independently of the untrusted directory response.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct PinnedUserIdentity {
    /// User whose root key is pinned.
    pub user_id: UserId,
    /// Ed25519 root identity public key.
    pub root_key_pub: [u8; 32],
}

impl PinnedUserIdentity {
    /// Construct a pinned identity from already verified local trust state.
    #[must_use]
    pub const fn new(user_id: UserId, root_key_pub: [u8; 32]) -> Self {
        Self {
            user_id,
            root_key_pub,
        }
    }
}

impl core::fmt::Debug for PinnedUserIdentity {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("PinnedUserIdentity")
            .field("user_id", &self.user_id)
            .field("root_key_pub", &"<public key omitted>")
            .finish()
    }
}

/// Initial Add commit and join material awaiting Delivery Service acceptance.
pub struct PendingGroupCommit {
    /// Locally pinned group identifier.
    pub group_id: FilamentGroupId,
    /// Epoch from which this commit was generated.
    pub prior_epoch: u64,
    /// Epoch reached if the commit is accepted.
    pub epoch: u64,
    /// MLS ciphersuite identifier.
    pub suite: CiphersuiteId,
    /// Device that authored the commit.
    pub committer_device_id: DeviceId,
    /// TLS-serialized MLS commit, opaque to the server.
    pub commit_blob: Vec<u8>,
    /// TLS-serialized Welcome for the invited device.
    pub welcome_blob: Vec<u8>,
    /// Optional TLS-serialized GroupInfo for recovery.
    pub group_info_blob: Option<Vec<u8>>,
}

impl core::fmt::Debug for PendingGroupCommit {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("PendingGroupCommit")
            .field("group_id", &self.group_id)
            .field("prior_epoch", &self.prior_epoch)
            .field("epoch", &self.epoch)
            .field("suite", &self.suite)
            .field("committer_device_id", &self.committer_device_id)
            .field("commit_bytes", &self.commit_blob.len())
            .field("welcome_bytes", &self.welcome_blob.len())
            .field(
                "group_info_bytes",
                &self.group_info_blob.as_ref().map(Vec::len),
            )
            .finish()
    }
}

/// Opaque MLS application record plus untrusted server routing hints.
#[derive(Clone, PartialEq, Eq)]
pub struct EncryptedApplicationMessage {
    /// Conversation-level mode; always `mls_v1` for this type.
    pub crypto: ConversationCrypto,
    /// Locally pinned group identifier.
    pub group_id: FilamentGroupId,
    /// Epoch in which OpenMLS encrypted the message.
    pub epoch: u64,
    /// MLS ciphersuite routing hint.
    pub suite: CiphersuiteId,
    /// Sender device routing hint.
    pub sender_device_id: DeviceId,
    /// TLS-serialized MLS PrivateMessage.
    pub message_blob: Vec<u8>,
}

impl core::fmt::Debug for EncryptedApplicationMessage {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("EncryptedApplicationMessage")
            .field("crypto", &self.crypto)
            .field("group_id", &self.group_id)
            .field("epoch", &self.epoch)
            .field("suite", &self.suite)
            .field("sender_device_id", &self.sender_device_id)
            .field("message_bytes", &self.message_blob.len())
            .finish()
    }
}

/// One authenticated and generation-ordered plaintext application message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecryptedApplicationMessage {
    /// Root-identity user authenticated by the MLS device credential.
    pub sender_user_id: UserId,
    /// Root-certified device authenticated by the MLS signature.
    pub sender_device_id: DeviceId,
    /// Monotonic application generation for this sender device.
    pub generation: u64,
    /// Decrypted application bytes.
    pub plaintext: Vec<u8>,
}

/// Result of processing one encrypted application message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecryptionOutcome {
    /// Newly contiguous messages, always ordered by application generation.
    pub ready_messages: Vec<DecryptedApplicationMessage>,
    /// Whether later messages remain buffered behind a missing generation.
    pub messages_may_be_missing: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct VerifiedMember {
    user_id: UserId,
    device_id: DeviceId,
}

#[derive(Default)]
struct InboundGenerationQueue {
    next_generation: u64,
    pending: BTreeMap<u64, DecryptedApplicationMessage>,
}

/// Client-side state for one two-device, two-user MLS v1 conversation.
pub struct MlsConversation {
    group_id: FilamentGroupId,
    group: MlsGroup,
    own_device_id: DeviceId,
    pinned_roots: HashMap<UserId, [u8; 32]>,
    outbound_generation: u64,
    inbound: HashMap<DeviceId, InboundGenerationQueue>,
    active: bool,
}

impl core::fmt::Debug for MlsConversation {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("MlsConversation")
            .field("group_id", &self.group_id)
            .field("epoch", &self.group.epoch().as_u64())
            .field("own_device_id", &self.own_device_id)
            .field("member_count", &self.group.members().count())
            .field("active", &self.active)
            .field("state", &"<MLS key material omitted>")
            .finish_non_exhaustive()
    }
}

impl MlsConversation {
    /// Create a two-user group and stage an Add commit from a claimed peer
    /// `KeyPackage`. The caller must submit the returned commit to the Delivery
    /// Service, then call [`Self::accept_pending_commit`] only after acceptance.
    ///
    /// # Errors
    /// Returns [`ConversationError`] if the package is invalid, is not bound to
    /// the pinned peer identity, or any MLS operation fails.
    pub fn create_two_member(
        group_id: FilamentGroupId,
        device: &MlsDevice,
        peer: PinnedUserIdentity,
        peer_keypackage_blob: &[u8],
    ) -> Result<(Self, PendingGroupCommit), ConversationError> {
        if peer.user_id == device.user_id() {
            return Err(ConversationError::UnexpectedMembership);
        }
        let key_package = parse_and_verify_keypackage(device, peer_keypackage_blob, peer)?;
        let join_config = sender_ratchet_configuration();
        let openmls_group_id =
            openmls::prelude::GroupId::from_slice(group_id.to_string().as_bytes());
        let mut group = MlsGroup::builder()
            .with_group_id(openmls_group_id)
            .ciphersuite(CIPHERSUITE)
            .use_ratchet_tree_extension(true)
            .sender_ratchet_configuration(join_config)
            .build(
                device.provider(),
                device.signer(),
                device.credential_with_key(),
            )
            .map_err(|_| ConversationError::CryptoError)?;
        let prior_epoch = group.epoch().as_u64();
        let (commit, welcome, group_info) = group
            .add_members(device.provider(), device.signer(), &[key_package])
            .map_err(|_| ConversationError::CryptoError)?;
        let epoch = group
            .pending_commit()
            .ok_or(ConversationError::NoPendingCommit)?
            .epoch()
            .as_u64();
        let commit_blob = commit
            .to_bytes()
            .map_err(|_| ConversationError::SerializationFailed)?;
        let welcome_blob = welcome
            .to_bytes()
            .map_err(|_| ConversationError::SerializationFailed)?;
        let group_info_blob = group_info
            .map(|info| {
                MlsMessageOut::from(info)
                    .to_bytes()
                    .map_err(|_| ConversationError::SerializationFailed)
            })
            .transpose()?;
        enforce_serialized_limits(&commit_blob, &welcome_blob, group_info_blob.as_deref())?;

        let mut pinned_roots = HashMap::with_capacity(2);
        pinned_roots.insert(device.user_id(), *device.root_key_public());
        pinned_roots.insert(peer.user_id, peer.root_key_pub);
        let conversation = Self {
            group_id,
            group,
            own_device_id: device.device_id(),
            pinned_roots,
            outbound_generation: 0,
            inbound: HashMap::new(),
            active: false,
        };
        let pending = PendingGroupCommit {
            group_id,
            prior_epoch,
            epoch,
            suite: CiphersuiteId::baseline(),
            committer_device_id: device.device_id(),
            commit_blob,
            welcome_blob,
            group_info_blob,
        };
        Ok((conversation, pending))
    }

    /// Join an accepted two-user group from a Welcome and validate every group
    /// member against the two locally pinned root identities.
    ///
    /// # Errors
    /// Returns [`ConversationError`] for malformed Welcome data, a mismatched
    /// group/suite, or any untrusted/extra member.
    pub fn join_from_welcome(
        group_id: FilamentGroupId,
        device: &MlsDevice,
        peer: PinnedUserIdentity,
        welcome_blob: &[u8],
    ) -> Result<Self, ConversationError> {
        if peer.user_id == device.user_id()
            || welcome_blob.is_empty()
            || welcome_blob.len() > MAX_WELCOME_BYTES
        {
            return Err(ConversationError::UnexpectedMembership);
        }
        let message = MlsMessageIn::tls_deserialize_exact(welcome_blob)
            .map_err(|_| ConversationError::SerializationFailed)?;
        let MlsMessageBodyIn::Welcome(welcome) = message.extract() else {
            return Err(ConversationError::InvalidApplicationMessage);
        };
        if welcome.ciphersuite() != CIPHERSUITE {
            return Err(ConversationError::MetadataMismatch);
        }
        let join_config = MlsGroupJoinConfig::builder()
            .use_ratchet_tree_extension(true)
            .sender_ratchet_configuration(sender_ratchet_configuration())
            .build();
        let staged =
            StagedWelcome::new_from_welcome(device.provider(), &join_config, welcome, None)
                .map_err(|_| ConversationError::CryptoError)?;
        let mut group = staged
            .into_group(device.provider())
            .map_err(|_| ConversationError::CryptoError)?;
        let mut pinned_roots = HashMap::with_capacity(2);
        pinned_roots.insert(device.user_id(), *device.root_key_public());
        pinned_roots.insert(peer.user_id, peer.root_key_pub);
        let validation = if group.group_id().as_slice() != group_id.to_string().as_bytes() {
            Err(ConversationError::GroupMismatch)
        } else if group.ciphersuite() != CIPHERSUITE {
            Err(ConversationError::MetadataMismatch)
        } else {
            validate_two_member_group(&group, &pinned_roots, device.device_id())
        };
        if let Err(error) = validation {
            group
                .delete(device.provider().storage())
                .map_err(|_| ConversationError::CryptoError)?;
            return Err(error);
        }

        Ok(Self {
            group_id,
            group,
            own_device_id: device.device_id(),
            pinned_roots,
            outbound_generation: 0,
            inbound: HashMap::new(),
            active: true,
        })
    }

    /// Merge a locally-authored commit after the Delivery Service accepts it.
    ///
    /// # Errors
    /// Returns [`ConversationError::NoPendingCommit`] if there is no staged
    /// commit or if merging/credential validation fails.
    pub fn accept_pending_commit(&mut self, device: &MlsDevice) -> Result<(), ConversationError> {
        self.ensure_device(device)?;
        if self.group.pending_commit().is_none() {
            return Err(ConversationError::NoPendingCommit);
        }
        self.group
            .merge_pending_commit(device.provider())
            .map_err(|_| ConversationError::CryptoError)?;
        validate_two_member_group(&self.group, &self.pinned_roots, self.own_device_id)?;
        self.active = true;
        Ok(())
    }

    /// Discard a locally-authored commit after a deterministic Delivery Service
    /// epoch conflict. This never silently merges rejected state.
    ///
    /// # Errors
    /// Returns [`ConversationError`] if no commit is pending or storage fails.
    pub fn reject_pending_commit(&mut self, device: &MlsDevice) -> Result<(), ConversationError> {
        self.ensure_device(device)?;
        if self.group.pending_commit().is_none() {
            return Err(ConversationError::NoPendingCommit);
        }
        self.group
            .clear_pending_commit(device.provider().storage())
            .map_err(|_| ConversationError::CryptoError)
    }

    /// Encrypt one bounded application payload as an MLS PrivateMessage.
    ///
    /// # Errors
    /// Fails while a commit is pending, for empty/oversized content, or if MLS
    /// encryption/serialization fails.
    pub fn encrypt_application_message(
        &mut self,
        device: &MlsDevice,
        plaintext: &[u8],
    ) -> Result<EncryptedApplicationMessage, ConversationError> {
        self.ensure_device(device)?;
        if self.group.pending_commit().is_some() {
            return Err(ConversationError::PendingCommit);
        }
        if !self.active {
            return Err(ConversationError::NotActive);
        }
        if plaintext.is_empty() || plaintext.len() > MAX_APPLICATION_PLAINTEXT_BYTES {
            return Err(ConversationError::LimitExceeded);
        }
        let generation = self.outbound_generation;
        let application_payload = encode_application_payload(generation, plaintext)?;
        let message = self
            .group
            .create_message(device.provider(), device.signer(), &application_payload)
            .map_err(|_| ConversationError::CryptoError)?;
        let message_blob = message
            .to_bytes()
            .map_err(|_| ConversationError::SerializationFailed)?;
        if message_blob.is_empty() || message_blob.len() > MAX_MLS_MESSAGE_BYTES {
            return Err(ConversationError::LimitExceeded);
        }
        self.outbound_generation = generation
            .checked_add(1)
            .ok_or(ConversationError::LimitExceeded)?;
        Ok(EncryptedApplicationMessage {
            crypto: ConversationCrypto::MlsV1,
            group_id: self.group_id,
            epoch: self.group.epoch().as_u64(),
            suite: CiphersuiteId::baseline(),
            sender_device_id: self.own_device_id,
            message_blob,
        })
    }

    /// Authenticate, decrypt, and generation-order one MLS PrivateMessage.
    ///
    /// Routing metadata is checked before MLS state is consumed wherever the
    /// wire format exposes the corresponding authenticated field. Sender
    /// identity is released only after MLS verification and root-certificate
    /// validation.
    ///
    /// # Errors
    /// Fails closed on mode/suite/group/epoch/sender mismatches, malformed
    /// payloads, duplicate generations, or bounded-buffer exhaustion.
    pub fn decrypt_application_message(
        &mut self,
        device: &MlsDevice,
        message: &EncryptedApplicationMessage,
    ) -> Result<DecryptionOutcome, ConversationError> {
        self.ensure_device(device)?;
        if !self.active {
            return Err(ConversationError::NotActive);
        }
        if message.crypto != ConversationCrypto::MlsV1 {
            return Err(ConversationError::CryptoModeMismatch);
        }
        if message.group_id != self.group_id
            || message.suite != CiphersuiteId::baseline()
            || self.group.ciphersuite() != CIPHERSUITE
        {
            return Err(ConversationError::MetadataMismatch);
        }
        if message.message_blob.is_empty() || message.message_blob.len() > MAX_MLS_MESSAGE_BYTES {
            return Err(ConversationError::LimitExceeded);
        }
        let wire_message = MlsMessageIn::tls_deserialize_exact(&message.message_blob)
            .map_err(|_| ConversationError::SerializationFailed)?;
        let protocol_message = wire_message
            .try_into_protocol_message()
            .map_err(|_| ConversationError::InvalidApplicationMessage)?;
        if protocol_message.wire_format() != WireFormat::PrivateMessage
            || protocol_message.group_id().as_slice() != self.group_id.to_string().as_bytes()
            || protocol_message.epoch().as_u64() != message.epoch
        {
            return Err(ConversationError::MetadataMismatch);
        }
        let processed = self
            .group
            .process_message(device.provider(), protocol_message)
            .map_err(|_| ConversationError::CryptoError)?;
        let Sender::Member(sender_index) = processed.sender() else {
            return Err(ConversationError::UnexpectedMembership);
        };
        let sender_index = *sender_index;
        let verified_sender = self.verify_member_at(sender_index)?;
        if verified_sender.device_id != message.sender_device_id {
            return Err(ConversationError::MetadataMismatch);
        }
        let ProcessedMessageContent::ApplicationMessage(application) = processed.into_content()
        else {
            return Err(ConversationError::InvalidApplicationMessage);
        };
        let (generation, plaintext) = decode_application_payload(&application.into_bytes())?;
        self.queue_generation(verified_sender, generation, plaintext)
    }

    /// Current locally verified MLS epoch.
    #[must_use]
    pub fn epoch(&self) -> u64 {
        self.group.epoch().as_u64()
    }

    /// Locally pinned group identifier.
    #[must_use]
    pub const fn group_id(&self) -> FilamentGroupId {
        self.group_id
    }

    fn ensure_device(&self, device: &MlsDevice) -> Result<(), ConversationError> {
        if device.device_id() != self.own_device_id {
            return Err(ConversationError::UnexpectedMembership);
        }
        Ok(())
    }

    fn verify_member_at(
        &self,
        sender_index: LeafNodeIndex,
    ) -> Result<VerifiedMember, ConversationError> {
        let member = self
            .group
            .members()
            .find(|member| member.index == sender_index)
            .ok_or(ConversationError::UnexpectedMembership)?;
        verify_member_credential(
            &member.credential,
            &member.signature_key,
            &self.pinned_roots,
        )
    }

    fn queue_generation(
        &mut self,
        sender: VerifiedMember,
        generation: u64,
        plaintext: Vec<u8>,
    ) -> Result<DecryptionOutcome, ConversationError> {
        let queue = self.inbound.entry(sender.device_id).or_default();
        if generation < queue.next_generation || queue.pending.contains_key(&generation) {
            return Err(ConversationError::DuplicateGeneration);
        }
        let distance = generation
            .checked_sub(queue.next_generation)
            .ok_or(ConversationError::DuplicateGeneration)?;
        if distance > MAX_BUFFERED_GENERATION_GAP
            || queue.pending.len()
                >= usize::try_from(MAX_BUFFERED_GENERATION_GAP).unwrap_or(usize::MAX)
        {
            return Err(ConversationError::GenerationGapExceeded);
        }
        queue.pending.insert(
            generation,
            DecryptedApplicationMessage {
                sender_user_id: sender.user_id,
                sender_device_id: sender.device_id,
                generation,
                plaintext,
            },
        );
        let mut ready_messages = Vec::new();
        while let Some(ready) = queue.pending.remove(&queue.next_generation) {
            ready_messages.push(ready);
            queue.next_generation = queue
                .next_generation
                .checked_add(1)
                .ok_or(ConversationError::LimitExceeded)?;
        }
        Ok(DecryptionOutcome {
            ready_messages,
            messages_may_be_missing: !queue.pending.is_empty(),
        })
    }
}

fn sender_ratchet_configuration() -> SenderRatchetConfiguration {
    SenderRatchetConfiguration::new(OUT_OF_ORDER_TOLERANCE, MAXIMUM_FORWARD_DISTANCE)
}

fn parse_and_verify_keypackage(
    device: &MlsDevice,
    blob: &[u8],
    peer: PinnedUserIdentity,
) -> Result<KeyPackage, ConversationError> {
    if blob.is_empty() || blob.len() > MAX_KEYPACKAGE_BYTES {
        return Err(ConversationError::LimitExceeded);
    }
    let incoming = KeyPackageIn::tls_deserialize_exact(blob)
        .map_err(|_| ConversationError::InvalidKeyPackage)?;
    let key_package = incoming
        .validate(device.provider().crypto(), ProtocolVersion::Mls10)
        .map_err(|_| ConversationError::InvalidKeyPackage)?;
    if key_package.ciphersuite() != CIPHERSUITE {
        return Err(ConversationError::InvalidKeyPackage);
    }
    let mut pins = HashMap::with_capacity(1);
    pins.insert(peer.user_id, peer.root_key_pub);
    let verified = verify_member_credential(
        key_package.leaf_node().credential(),
        key_package.leaf_node().signature_key().as_slice(),
        &pins,
    )?;
    if verified.user_id != peer.user_id {
        return Err(ConversationError::UntrustedCredential);
    }
    Ok(key_package)
}

fn validate_two_member_group(
    group: &MlsGroup,
    pinned_roots: &HashMap<UserId, [u8; 32]>,
    own_device_id: DeviceId,
) -> Result<(), ConversationError> {
    let mut members = HashMap::with_capacity(2);
    for member in group.members() {
        let verified =
            verify_member_credential(&member.credential, &member.signature_key, pinned_roots)?;
        if members
            .insert(verified.device_id, verified.user_id)
            .is_some()
        {
            return Err(ConversationError::UnexpectedMembership);
        }
    }
    if members.len() != 2
        || pinned_roots.len() != 2
        || !members.contains_key(&own_device_id)
        || !pinned_roots
            .keys()
            .all(|user_id| members.values().any(|member_user| member_user == user_id))
    {
        return Err(ConversationError::UnexpectedMembership);
    }
    Ok(())
}

fn verify_member_credential(
    credential: &Credential,
    signature_key: &[u8],
    pinned_roots: &HashMap<UserId, [u8; 32]>,
) -> Result<VerifiedMember, ConversationError> {
    let bytes = credential.serialized_content();
    let expected_len = DEVICE_CREDENTIAL_DOMAIN.len() + 26 + 26 + 32 + 64 + 32;
    if bytes.len() != expected_len || !bytes.starts_with(DEVICE_CREDENTIAL_DOMAIN) {
        return Err(ConversationError::UntrustedCredential);
    }
    let mut offset = DEVICE_CREDENTIAL_DOMAIN.len();
    let user_end = offset + 26;
    let user_id = core::str::from_utf8(&bytes[offset..user_end])
        .ok()
        .map(str::to_owned)
        .and_then(|value| UserId::try_from(value).ok())
        .ok_or(ConversationError::UntrustedCredential)?;
    offset = user_end;
    let device_end = offset + 26;
    let device_id = core::str::from_utf8(&bytes[offset..device_end])
        .ok()
        .map(str::to_owned)
        .and_then(|value| DeviceId::try_from(value).ok())
        .ok_or(ConversationError::UntrustedCredential)?;
    offset = device_end;
    let signature_key_end = offset + 32;
    let certified_signature_key: [u8; 32] = bytes[offset..signature_key_end]
        .try_into()
        .map_err(|_| ConversationError::UntrustedCredential)?;
    if signature_key != certified_signature_key {
        return Err(ConversationError::UntrustedCredential);
    }
    offset = signature_key_end;
    let root_signature_end = offset + 64;
    let root_signature: [u8; 64] = bytes[offset..root_signature_end]
        .try_into()
        .map_err(|_| ConversationError::UntrustedCredential)?;
    offset = root_signature_end;
    let embedded_root: [u8; 32] = bytes[offset..]
        .try_into()
        .map_err(|_| ConversationError::UntrustedCredential)?;
    let pinned_root = pinned_roots
        .get(&user_id)
        .ok_or(ConversationError::UntrustedCredential)?;
    if pinned_root != &embedded_root {
        return Err(ConversationError::UntrustedCredential);
    }
    let certificate = DeviceCertificate::try_new(
        user_id.to_string(),
        device_id.to_string(),
        certified_signature_key.to_vec(),
        root_signature.to_vec(),
    )
    .map_err(|_| ConversationError::UntrustedCredential)?;
    let certificate_key: [u8; 32] = certificate
        .device_signature_pubkey
        .as_slice()
        .try_into()
        .map_err(|_| ConversationError::UntrustedCredential)?;
    let certificate_signature: [u8; 64] = certificate
        .root_key_signature
        .as_slice()
        .try_into()
        .map_err(|_| ConversationError::UntrustedCredential)?;
    verify_device_certificate(
        pinned_root,
        user_id,
        device_id,
        &certificate_key,
        &certificate_signature,
    )
    .map_err(|_| ConversationError::UntrustedCredential)?;
    Ok(VerifiedMember { user_id, device_id })
}

fn enforce_serialized_limits(
    commit_blob: &[u8],
    welcome_blob: &[u8],
    group_info_blob: Option<&[u8]>,
) -> Result<(), ConversationError> {
    if commit_blob.is_empty()
        || commit_blob.len() > MAX_COMMIT_BYTES
        || welcome_blob.is_empty()
        || welcome_blob.len() > MAX_WELCOME_BYTES
        || group_info_blob.is_some_and(|blob| blob.is_empty() || blob.len() > MAX_GROUP_INFO_BYTES)
    {
        return Err(ConversationError::LimitExceeded);
    }
    Ok(())
}

fn encode_application_payload(
    generation: u64,
    plaintext: &[u8],
) -> Result<Vec<u8>, ConversationError> {
    let content_len =
        u32::try_from(plaintext.len()).map_err(|_| ConversationError::LimitExceeded)?;
    let mut encoded = Vec::with_capacity(APPLICATION_HEADER_BYTES + plaintext.len());
    encoded.extend_from_slice(&APPLICATION_ENVELOPE_VERSION.to_be_bytes());
    encoded.extend_from_slice(&generation.to_be_bytes());
    encoded.extend_from_slice(&content_len.to_be_bytes());
    encoded.extend_from_slice(plaintext);
    Ok(encoded)
}

fn decode_application_payload(bytes: &[u8]) -> Result<(u64, Vec<u8>), ConversationError> {
    if bytes.len() < APPLICATION_HEADER_BYTES {
        return Err(ConversationError::InvalidApplicationMessage);
    }
    let version = u16::from_be_bytes(
        bytes[0..2]
            .try_into()
            .map_err(|_| ConversationError::InvalidApplicationMessage)?,
    );
    if version != APPLICATION_ENVELOPE_VERSION {
        return Err(ConversationError::InvalidApplicationMessage);
    }
    let generation = u64::from_be_bytes(
        bytes[2..10]
            .try_into()
            .map_err(|_| ConversationError::InvalidApplicationMessage)?,
    );
    let content_len = usize::try_from(u32::from_be_bytes(
        bytes[10..14]
            .try_into()
            .map_err(|_| ConversationError::InvalidApplicationMessage)?,
    ))
    .map_err(|_| ConversationError::InvalidApplicationMessage)?;
    if content_len == 0
        || content_len > MAX_APPLICATION_PLAINTEXT_BYTES
        || bytes.len() != APPLICATION_HEADER_BYTES + content_len
    {
        return Err(ConversationError::InvalidApplicationMessage);
    }
    Ok((generation, bytes[APPLICATION_HEADER_BYTES..].to_vec()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{generate_key_package_batch, RootIdentityKey};

    struct Fixture {
        alice: MlsDevice,
        bob: MlsDevice,
        bob_pin: PinnedUserIdentity,
        group_id: FilamentGroupId,
        bob_keypackage: Vec<u8>,
    }

    fn fixture() -> Fixture {
        let alice_root = RootIdentityKey::generate();
        let bob_root = RootIdentityKey::generate();
        let alice = MlsDevice::generate(UserId::new(), DeviceId::new(), &alice_root).unwrap();
        let bob = MlsDevice::generate(UserId::new(), DeviceId::new(), &bob_root).unwrap();
        let bob_keypackage = generate_key_package_batch(&bob, 1).unwrap().remove(0).blob;
        let bob_pin = PinnedUserIdentity::new(bob.user_id(), *bob.root_key_public());
        Fixture {
            alice,
            bob,
            bob_pin,
            group_id: FilamentGroupId::new(),
            bob_keypackage,
        }
    }

    fn joined_conversations() -> (MlsDevice, MlsConversation, MlsDevice, MlsConversation) {
        let fixture = fixture();
        let alice_pin =
            PinnedUserIdentity::new(fixture.alice.user_id(), *fixture.alice.root_key_public());
        let (mut alice_conversation, pending) = MlsConversation::create_two_member(
            fixture.group_id,
            &fixture.alice,
            fixture.bob_pin,
            &fixture.bob_keypackage,
        )
        .unwrap();
        assert_eq!(pending.prior_epoch, 0);
        assert_eq!(pending.epoch, 1);
        if let Some(group_info_blob) = &pending.group_info_blob {
            assert!(matches!(
                MlsMessageIn::tls_deserialize_exact(group_info_blob)
                    .unwrap()
                    .extract(),
                MlsMessageBodyIn::GroupInfo(_)
            ));
        }
        alice_conversation
            .accept_pending_commit(&fixture.alice)
            .unwrap();
        let bob_conversation = MlsConversation::join_from_welcome(
            fixture.group_id,
            &fixture.bob,
            alice_pin,
            &pending.welcome_blob,
        )
        .unwrap();
        (
            fixture.alice,
            alice_conversation,
            fixture.bob,
            bob_conversation,
        )
    }

    #[test]
    fn two_member_create_join_and_private_message_round_trip() {
        let (alice, mut alice_group, bob, mut bob_group) = joined_conversations();
        assert_eq!(alice_group.epoch(), 1);
        assert_eq!(bob_group.epoch(), 1);
        let encrypted = alice_group
            .encrypt_application_message(&alice, b"hello bob")
            .unwrap();
        assert!(!encrypted
            .message_blob
            .windows(9)
            .any(|value| value == b"hello bob"));
        let outcome = bob_group
            .decrypt_application_message(&bob, &encrypted)
            .unwrap();
        assert!(!outcome.messages_may_be_missing);
        assert_eq!(outcome.ready_messages.len(), 1);
        assert_eq!(outcome.ready_messages[0].plaintext, b"hello bob");
        assert_eq!(outcome.ready_messages[0].sender_user_id, alice.user_id());
        assert_eq!(
            outcome.ready_messages[0].sender_device_id,
            alice.device_id()
        );
    }

    #[test]
    fn out_of_order_messages_are_buffered_and_released_in_generation_order() {
        let (alice, mut alice_group, bob, mut bob_group) = joined_conversations();
        let first = alice_group
            .encrypt_application_message(&alice, b"generation zero")
            .unwrap();
        let second = alice_group
            .encrypt_application_message(&alice, b"generation one")
            .unwrap();

        let gap = bob_group
            .decrypt_application_message(&bob, &second)
            .unwrap();
        assert!(gap.ready_messages.is_empty());
        assert!(gap.messages_may_be_missing);

        let filled = bob_group.decrypt_application_message(&bob, &first).unwrap();
        assert!(!filled.messages_may_be_missing);
        assert_eq!(filled.ready_messages.len(), 2);
        assert_eq!(filled.ready_messages[0].generation, 0);
        assert_eq!(filled.ready_messages[0].plaintext, b"generation zero");
        assert_eq!(filled.ready_messages[1].generation, 1);
        assert_eq!(filled.ready_messages[1].plaintext, b"generation one");
    }

    #[test]
    fn downgrade_hint_fails_closed_without_consuming_ciphertext() {
        let (alice, mut alice_group, bob, mut bob_group) = joined_conversations();
        let mut encrypted = alice_group
            .encrypt_application_message(&alice, b"still encrypted")
            .unwrap();
        encrypted.crypto = ConversationCrypto::Plaintext;
        assert_eq!(
            bob_group
                .decrypt_application_message(&bob, &encrypted)
                .unwrap_err(),
            ConversationError::CryptoModeMismatch
        );

        encrypted.crypto = ConversationCrypto::MlsV1;
        let decrypted = bob_group
            .decrypt_application_message(&bob, &encrypted)
            .unwrap();
        assert_eq!(decrypted.ready_messages[0].plaintext, b"still encrypted");
    }

    #[test]
    fn keypackage_from_unpinned_root_is_rejected() {
        let fixture = fixture();
        let attacker_root = RootIdentityKey::generate();
        let forged_pin =
            PinnedUserIdentity::new(fixture.bob.user_id(), attacker_root.public_key_bytes());
        assert_eq!(
            MlsConversation::create_two_member(
                fixture.group_id,
                &fixture.alice,
                forged_pin,
                &fixture.bob_keypackage,
            )
            .unwrap_err(),
            ConversationError::UntrustedCredential
        );
    }

    #[test]
    fn pending_commit_blocks_sends_and_can_be_rejected() {
        let fixture = fixture();
        let (mut conversation, _) = MlsConversation::create_two_member(
            fixture.group_id,
            &fixture.alice,
            fixture.bob_pin,
            &fixture.bob_keypackage,
        )
        .unwrap();
        assert_eq!(
            conversation
                .encrypt_application_message(&fixture.alice, b"not accepted")
                .unwrap_err(),
            ConversationError::PendingCommit
        );
        conversation.reject_pending_commit(&fixture.alice).unwrap();
        assert_eq!(
            conversation
                .encrypt_application_message(&fixture.alice, b"rejected group")
                .unwrap_err(),
            ConversationError::NotActive
        );
        assert_eq!(
            conversation
                .reject_pending_commit(&fixture.alice)
                .unwrap_err(),
            ConversationError::NoPendingCommit
        );
    }

    #[test]
    fn application_payload_parser_is_strict_and_bounded() {
        let encoded = encode_application_payload(7, b"payload").unwrap();
        assert_eq!(
            decode_application_payload(&encoded).unwrap(),
            (7, b"payload".to_vec())
        );

        let mut trailing = encoded.clone();
        trailing.push(0);
        assert_eq!(
            decode_application_payload(&trailing).unwrap_err(),
            ConversationError::InvalidApplicationMessage
        );
        assert_eq!(
            encode_application_payload(0, &vec![0; MAX_APPLICATION_PLAINTEXT_BYTES + 1])
                .and_then(|value| decode_application_payload(&value))
                .unwrap_err(),
            ConversationError::InvalidApplicationMessage
        );
    }
}
