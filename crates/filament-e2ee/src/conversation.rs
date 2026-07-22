//! Bounded MLS conversation lifecycle and fail-closed message processing.
//!
//! The server-provided group, epoch, suite, and sender fields are routing hints
//! only. This module checks every hint against the locally pinned conversation
//! and MLS-authenticated state before releasing plaintext.

use std::collections::{BTreeMap, HashMap};

use filament_core::{
    CiphersuiteId, ConversationCrypto, DeviceCertificate, DeviceId, GroupId as FilamentGroupId,
    UserId,
};
use filament_protocol::GroupInfoResponse;
use openmls::prelude::group_info::GroupInfo;
use openmls::prelude::*;
use tls_codec::{Deserialize as TlsDeserialize, Serialize as TlsSerialize};

use crate::{
    error::ConversationError,
    identity::verify_device_certificate,
    keypackage::{MlsDevice, DEVICE_CREDENTIAL_DOMAIN},
};

const CIPHERSUITE: Ciphersuite = Ciphersuite::MLS_128_DHKEMX25519_CHACHA20POLY1305_SHA256_Ed25519;
const APPLICATION_ENVELOPE_VERSION: u16 = 1;
const APPLICATION_HEADER_BYTES: usize = 2 + 8 + 4;
const APPLICATION_PADDING_BUCKETS: [usize; 4] = [256, 768, 3_584, 15_360];
const MAX_KEYPACKAGE_BYTES: usize = 4_096;
const MAX_MLS_MESSAGE_BYTES: usize = 65_536;
const MESSAGE_TRANSPORT_PADDING_BUCKETS: [usize; 4] = [512, 1_024, 4_096, 16_384];
const MAX_WELCOME_BYTES: usize = 65_536;
const MAX_COMMIT_BYTES: usize = 65_536;
const MAX_GROUP_INFO_BYTES: usize = 65_536;
/// Maximum MLS leaves in one conversation.
pub const MAX_MLS_GROUP_LEAVES: usize = 200;
/// Maximum root-identity users in one group DM.
pub const MAX_MLS_GROUP_USERS: usize = 100;
/// Maximum certified device leaves belonging to one user.
pub const MAX_MLS_DEVICES_PER_USER: usize = 100;
const OUT_OF_ORDER_TOLERANCE: u32 = 64;
const MAXIMUM_FORWARD_DISTANCE: u32 = 256;

/// Maximum plaintext bytes accepted by the MLS application layer.
///
/// This leaves bounded room for the application envelope and MLS framing in
/// the largest 16 KiB transport bucket.
pub const MAX_APPLICATION_PLAINTEXT_BYTES: usize = 12 * 1_024;

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

/// Locally enforced audience policy for an MLS conversation.
///
/// This is domain state, not a server routing hint. Direct messages retain
/// their exact two-user invariant while group DMs admit a bounded set of
/// root-pinned participants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConversationAudience {
    /// Exactly two root identities must remain represented.
    DirectMessage,
    /// Between two and [`MAX_MLS_GROUP_USERS`] root identities may be represented.
    GroupDm,
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
    /// TLS-serialized Welcome when this commit adds a device.
    pub welcome_blob: Option<Vec<u8>>,
    /// Optional TLS-serialized GroupInfo for recovery.
    pub group_info_blob: Option<Vec<u8>>,
}

/// Strictly validated Delivery Service input for external-commit recovery.
///
/// Every field remains an untrusted routing hint until it is matched against
/// the signed MLS `GroupInfo` while building the recovery commit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalCommitRecoveryInfo {
    /// Locally routed MLS group.
    pub group_id: FilamentGroupId,
    /// Delivery Service's claimed current epoch.
    pub epoch: u64,
    /// Delivery Service's claimed ciphersuite.
    pub suite: CiphersuiteId,
    /// TLS-serialized signed MLS `GroupInfo`.
    pub group_info_blob: Vec<u8>,
}

impl TryFrom<GroupInfoResponse> for ExternalCommitRecoveryInfo {
    type Error = ConversationError;

    fn try_from(response: GroupInfoResponse) -> Result<Self, Self::Error> {
        let group_id = FilamentGroupId::try_from(response.group_id)
            .map_err(|_| ConversationError::MetadataMismatch)?;
        let suite = CiphersuiteId::try_from(response.suite_id)
            .map_err(|_| ConversationError::MetadataMismatch)?;
        if response.epoch == 0
            || response.group_info_blob.is_empty()
            || response.group_info_blob.len() > MAX_GROUP_INFO_BYTES
        {
            return Err(ConversationError::LimitExceeded);
        }
        Ok(Self {
            group_id,
            epoch: response.epoch,
            suite,
            group_info_blob: response.group_info_blob,
        })
    }
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
            .field("welcome_bytes", &self.welcome_blob.as_ref().map(Vec::len))
            .field(
                "group_info_bytes",
                &self.group_info_blob.as_ref().map(Vec::len),
            )
            .finish()
    }
}

/// Result of rebasing one locally rejected commit intent.
#[derive(Debug)]
pub enum PendingCommitRebase {
    /// The intent remains valid and was restaged at the new epoch.
    Rebased(PendingGroupCommit),
    /// The accepted competing commit already performed the intended Add or Remove.
    AlreadySatisfied,
    /// The winning membership change made the intent unsafe or removed this device.
    Invalidated,
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

/// Opaque MLS commit plus untrusted Delivery Service routing hints.
#[derive(Clone, PartialEq, Eq)]
pub struct EncryptedGroupCommit {
    /// Locally pinned group identifier.
    pub group_id: FilamentGroupId,
    /// Epoch from which the commit advances.
    pub prior_epoch: u64,
    /// Epoch reached by the commit.
    pub epoch: u64,
    /// Device that the Delivery Service claims authored the commit.
    pub committer_device_id: DeviceId,
    /// TLS-serialized MLS commit.
    pub commit_blob: Vec<u8>,
}

impl core::fmt::Debug for EncryptedGroupCommit {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("EncryptedGroupCommit")
            .field("group_id", &self.group_id)
            .field("prior_epoch", &self.prior_epoch)
            .field("epoch", &self.epoch)
            .field("committer_device_id", &self.committer_device_id)
            .field("commit_bytes", &self.commit_blob.len())
            .finish()
    }
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
    /// The newly MLS-authenticated plaintext, including an out-of-order
    /// message that is not yet safe to display. Persist this before acking its
    /// transport record.
    pub authenticated_message: DecryptedApplicationMessage,
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

enum PendingCommitIntent {
    SelfUpdate,
    AddDevice {
        target: PinnedUserIdentity,
        target_device_id: DeviceId,
        keypackage_blob: Vec<u8>,
    },
    RemoveDevice {
        target_device_id: DeviceId,
    },
    RemoveParticipant {
        target_user_id: UserId,
    },
}

/// Client-side state for one bounded MLS v1 conversation.
pub struct MlsConversation {
    group_id: FilamentGroupId,
    group: MlsGroup,
    own_device_id: DeviceId,
    audience: ConversationAudience,
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
            .field("audience", &self.audience)
            .field("member_count", &self.group.members().count())
            .field("active", &self.active)
            .field("state", &"<MLS key material omitted>")
            .finish_non_exhaustive()
    }
}

impl MlsConversation {
    pub(crate) fn recover_by_external_commit(
        self,
        recovery: &ExternalCommitRecoveryInfo,
        device: &MlsDevice,
    ) -> Result<(Self, PendingGroupCommit), ConversationError> {
        self.ensure_device(device)?;
        if self.group_id != recovery.group_id
            || self.pinned_roots.get(&device.user_id()) != Some(device.root_key_public())
            || recovery.suite != CiphersuiteId::baseline()
            || recovery.group_info_blob.is_empty()
            || recovery.group_info_blob.len() > MAX_GROUP_INFO_BYTES
        {
            return Err(ConversationError::MetadataMismatch);
        }
        let message = MlsMessageIn::tls_deserialize_exact(&recovery.group_info_blob)
            .map_err(|_| ConversationError::SerializationFailed)?;
        let MlsMessageBodyIn::GroupInfo(group_info) = message.extract() else {
            return Err(ConversationError::InvalidCommit);
        };
        if group_info.group_id().as_slice() != recovery.group_id.to_string().as_bytes()
            || group_info.epoch().as_u64() != recovery.epoch
            || group_info.ciphersuite() != CIPHERSUITE
        {
            return Err(ConversationError::MetadataMismatch);
        }

        let join_config = MlsGroupJoinConfig::builder()
            .wire_format_policy(PURE_PLAINTEXT_WIRE_FORMAT_POLICY)
            .use_ratchet_tree_extension(true)
            .sender_ratchet_configuration(sender_ratchet_configuration())
            .build();
        let (group, bundle) = MlsGroup::external_commit_builder()
            .with_config(join_config)
            .build_group(device.provider(), group_info, device.credential_with_key())
            .map_err(|_| ConversationError::CryptoError)?
            .load_psks(device.provider().storage())
            .map_err(|_| ConversationError::CryptoError)?
            .use_ratchet_tree_extension(true)
            .build(
                device.provider().rand(),
                device.provider().crypto(),
                device.signer(),
                |_| true,
            )
            .map_err(|_| ConversationError::CryptoError)?
            .finalize(device.provider())
            .map_err(|_| ConversationError::CryptoError)?;

        let expected_epoch = recovery
            .epoch
            .checked_add(1)
            .ok_or(ConversationError::LimitExceeded)?;
        let audience = self.audience;
        let pinned_roots = self.pinned_roots;
        if group.group_id().as_slice() != recovery.group_id.to_string().as_bytes()
            || group.epoch().as_u64() != expected_epoch
            || group.ciphersuite() != CIPHERSUITE
        {
            return Err(ConversationError::MetadataMismatch);
        }
        validate_group(&group, &pinned_roots, audience, Some(device.device_id()))?;

        let (commit, welcome, group_info) = bundle.into_contents();
        if welcome.is_some() {
            return Err(ConversationError::UnexpectedMembership);
        }
        let commit_blob = commit
            .to_bytes()
            .map_err(|_| ConversationError::SerializationFailed)?;
        let group_info_blob = group_info
            .map(|info| {
                MlsMessageOut::from(info)
                    .to_bytes()
                    .map_err(|_| ConversationError::SerializationFailed)
            })
            .transpose()?;
        enforce_serialized_limits(&commit_blob, None, group_info_blob.as_deref())?;
        let pending = PendingGroupCommit {
            group_id: recovery.group_id,
            prior_epoch: recovery.epoch,
            epoch: expected_epoch,
            suite: CiphersuiteId::baseline(),
            committer_device_id: device.device_id(),
            commit_blob,
            welcome_blob: None,
            group_info_blob,
        };
        let mut recovered = Self {
            group_id: recovery.group_id,
            group,
            own_device_id: device.device_id(),
            audience,
            pinned_roots,
            outbound_generation: self.outbound_generation,
            inbound: self.inbound,
            active: true,
        };
        recovered.prune_inbound_for_current_members()?;
        Ok((recovered, pending))
    }

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
        Self::create_with_members(
            group_id,
            device,
            ConversationAudience::DirectMessage,
            &[(peer, peer_keypackage_blob)],
        )
    }

    /// Create a group DM with one certified device for every initial invitee.
    ///
    /// All invitees are added by the same initial commit. The returned opaque
    /// Welcome contains an encrypted group secret for each supplied
    /// KeyPackage and must only be relayed to those exact devices.
    ///
    /// # Errors
    /// Rejects fewer than two invitees, duplicate/root-conflicting users or
    /// devices, oversized groups, and invalid certified KeyPackages.
    pub fn create_group(
        group_id: FilamentGroupId,
        device: &MlsDevice,
        invitees: &[(PinnedUserIdentity, Vec<u8>)],
    ) -> Result<(Self, PendingGroupCommit), ConversationError> {
        let borrowed = invitees
            .iter()
            .map(|(identity, blob)| (*identity, blob.as_slice()))
            .collect::<Vec<_>>();
        Self::create_with_members(group_id, device, ConversationAudience::GroupDm, &borrowed)
    }

    fn create_with_members(
        group_id: FilamentGroupId,
        device: &MlsDevice,
        audience: ConversationAudience,
        invitees: &[(PinnedUserIdentity, &[u8])],
    ) -> Result<(Self, PendingGroupCommit), ConversationError> {
        let expected_invitees = match audience {
            ConversationAudience::DirectMessage => 1..=1,
            ConversationAudience::GroupDm => 2..=MAX_MLS_GROUP_USERS.saturating_sub(1),
        };
        if !expected_invitees.contains(&invitees.len()) {
            return Err(ConversationError::LimitExceeded);
        }
        let mut pinned_roots = HashMap::with_capacity(invitees.len() + 1);
        pinned_roots.insert(device.user_id(), *device.root_key_public());
        let mut key_packages = Vec::with_capacity(invitees.len());
        let mut device_ids = std::collections::HashSet::with_capacity(invitees.len() + 1);
        device_ids.insert(device.device_id());
        for (identity, blob) in invitees {
            if identity.user_id == device.user_id()
                || pinned_roots
                    .insert(identity.user_id, identity.root_key_pub)
                    .is_some()
            {
                return Err(ConversationError::UnexpectedMembership);
            }
            let key_package = parse_and_verify_keypackage(device, blob, *identity)?;
            let verified = verify_member_credential(
                key_package.leaf_node().credential(),
                key_package.leaf_node().signature_key().as_slice(),
                &pinned_roots,
            )?;
            if !device_ids.insert(verified.device_id) {
                return Err(ConversationError::UnexpectedMembership);
            }
            key_packages.push(key_package);
        }
        let join_config = sender_ratchet_configuration();
        let openmls_group_id =
            openmls::prelude::GroupId::from_slice(group_id.to_string().as_bytes());
        let mut group = MlsGroup::builder()
            .with_group_id(openmls_group_id)
            .ciphersuite(CIPHERSUITE)
            .with_wire_format_policy(PURE_PLAINTEXT_WIRE_FORMAT_POLICY)
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
            .add_members(device.provider(), device.signer(), &key_packages)
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
        enforce_serialized_limits(
            &commit_blob,
            Some(&welcome_blob),
            group_info_blob.as_deref(),
        )?;

        let conversation = Self {
            group_id,
            group,
            own_device_id: device.device_id(),
            audience,
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
            welcome_blob: Some(welcome_blob),
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
        Self::join_with_members(
            group_id,
            device,
            ConversationAudience::DirectMessage,
            &[peer],
            welcome_blob,
        )
    }

    /// Join a group DM from an opaque Welcome and an independently verified
    /// root pin for every other participant.
    ///
    /// # Errors
    /// Rejects incomplete, duplicate, or oversized audiences and any Welcome
    /// whose authenticated MLS membership differs from the pinned audience.
    pub fn join_group_from_welcome(
        group_id: FilamentGroupId,
        device: &MlsDevice,
        participants: &[PinnedUserIdentity],
        welcome_blob: &[u8],
    ) -> Result<Self, ConversationError> {
        Self::join_with_members(
            group_id,
            device,
            ConversationAudience::GroupDm,
            participants,
            welcome_blob,
        )
    }

    fn join_with_members(
        group_id: FilamentGroupId,
        device: &MlsDevice,
        audience: ConversationAudience,
        participants: &[PinnedUserIdentity],
        welcome_blob: &[u8],
    ) -> Result<Self, ConversationError> {
        let expected_participants = match audience {
            ConversationAudience::DirectMessage => 1..=1,
            ConversationAudience::GroupDm => 2..=MAX_MLS_GROUP_USERS.saturating_sub(1),
        };
        if !expected_participants.contains(&participants.len())
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
            .wire_format_policy(PURE_PLAINTEXT_WIRE_FORMAT_POLICY)
            .use_ratchet_tree_extension(true)
            .sender_ratchet_configuration(sender_ratchet_configuration())
            .build();
        let staged =
            StagedWelcome::new_from_welcome(device.provider(), &join_config, welcome, None)
                .map_err(|_| ConversationError::CryptoError)?;
        let mut group = staged
            .into_group(device.provider())
            .map_err(|_| ConversationError::CryptoError)?;
        let mut pinned_roots = HashMap::with_capacity(participants.len() + 1);
        pinned_roots.insert(device.user_id(), *device.root_key_public());
        for participant in participants {
            if participant.user_id == device.user_id()
                || pinned_roots
                    .insert(participant.user_id, participant.root_key_pub)
                    .is_some()
            {
                group
                    .delete(device.provider().storage())
                    .map_err(|_| ConversationError::CryptoError)?;
                return Err(ConversationError::UnexpectedMembership);
            }
        }
        let validation = if group.group_id().as_slice() != group_id.to_string().as_bytes() {
            Err(ConversationError::GroupMismatch)
        } else if group.ciphersuite() != CIPHERSUITE {
            Err(ConversationError::MetadataMismatch)
        } else {
            validate_group(&group, &pinned_roots, audience, Some(device.device_id()))
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
            audience,
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
        let evicted_users = fully_removed_users(
            &self.group,
            self.group
                .pending_commit()
                .ok_or(ConversationError::NoPendingCommit)?,
            &self.pinned_roots,
        )?;
        self.group
            .merge_pending_commit(device.provider())
            .map_err(|_| ConversationError::CryptoError)?;
        self.remove_evicted_group_pins(&evicted_users, device.user_id());
        self.prune_unrepresented_group_pins(device.user_id())?;
        self.active = self.group.is_active();
        validate_group(
            &self.group,
            &self.pinned_roots,
            self.audience,
            self.active.then_some(self.own_device_id),
        )?;
        self.prune_inbound_for_current_members()?;
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
            .map_err(|_| ConversationError::CryptoError)?;
        if self.active {
            self.prune_unrepresented_group_pins(device.user_id())?;
        }
        Ok(())
    }

    /// Rebase a rejected local commit on the authenticated commit that won the
    /// Delivery Service's single-writer race.
    ///
    /// The winning commit must advance the same prior epoch as the pending
    /// local commit. This method validates its routing envelope before
    /// discarding rejected state, merges it through the normal credential and
    /// membership checks, and then restages the original self-update, Add, or
    /// Remove intent. [`PendingCommitRebase::AlreadySatisfied`] means the
    /// winning commit already added or removed the intended device, while
    /// [`PendingCommitRebase::Invalidated`] means the winner made the intent
    /// unsafe and no retry should be submitted.
    ///
    /// A rebased Add reuses its claimed KeyPackage only because the rejected
    /// commit and Welcome must never be delivered. The caller must submit only
    /// the newly returned commit and recipient-bound Welcome.
    ///
    /// # Errors
    /// Returns [`ConversationError`] if there is no supported pending commit,
    /// the winning commit is not the authenticated competitor for that epoch,
    /// the accepted commit fails MLS authentication, or restaging fails.
    pub fn rebase_pending_commit(
        &mut self,
        device: &MlsDevice,
        accepted_commit: &EncryptedGroupCommit,
    ) -> Result<PendingCommitRebase, ConversationError> {
        self.ensure_device(device)?;
        if !self.active {
            return Err(ConversationError::NotActive);
        }
        let pending_epoch = self
            .group
            .pending_commit()
            .ok_or(ConversationError::NoPendingCommit)?
            .epoch()
            .as_u64();
        if accepted_commit.prior_epoch != self.epoch() || accepted_commit.epoch != pending_epoch {
            return Err(ConversationError::MetadataMismatch);
        }

        let intent = self.pending_commit_intent()?;
        let protocol_message = self.validate_incoming_commit(accepted_commit)?;
        let staged_commit =
            self.stage_incoming_commit(device, accepted_commit, protocol_message)?;
        self.group
            .clear_pending_commit(device.provider().storage())
            .map_err(|_| ConversationError::CryptoError)?;
        self.merge_staged_incoming_commit(device, *staged_commit)?;
        if !self.active {
            return Ok(PendingCommitRebase::Invalidated);
        }

        match intent {
            PendingCommitIntent::SelfUpdate => self
                .create_self_update(device)
                .map(PendingCommitRebase::Rebased),
            PendingCommitIntent::AddDevice {
                target,
                target_device_id,
                keypackage_blob,
            } => {
                if self.has_verified_member_device(target_device_id)? {
                    Ok(PendingCommitRebase::AlreadySatisfied)
                } else {
                    let counts = verified_member_counts(&self.group, &self.pinned_roots)?;
                    if counts.total >= MAX_MLS_GROUP_LEAVES
                        || counts.per_user.get(&target.user_id).copied().unwrap_or(0)
                            >= MAX_MLS_DEVICES_PER_USER
                    {
                        return Ok(PendingCommitRebase::Invalidated);
                    }
                    if self.pinned_roots.contains_key(&target.user_id) {
                        self.create_add_device(device, target, &keypackage_blob)
                            .map(PendingCommitRebase::Rebased)
                    } else {
                        self.create_add_participant(device, target, &keypackage_blob)
                            .map(PendingCommitRebase::Rebased)
                    }
                }
            }
            PendingCommitIntent::RemoveDevice { target_device_id } => {
                let members = verified_members(&self.group, &self.pinned_roots)?;
                let Some((_, target)) = members
                    .iter()
                    .find(|(_, member)| member.device_id == target_device_id)
                else {
                    return Ok(PendingCommitRebase::AlreadySatisfied);
                };
                if members
                    .iter()
                    .filter(|(_, member)| member.user_id == target.user_id)
                    .count()
                    <= 1
                {
                    return Ok(PendingCommitRebase::Invalidated);
                }
                self.create_remove_device(device, target_device_id)
                    .map(PendingCommitRebase::Rebased)
            }
            PendingCommitIntent::RemoveParticipant { target_user_id } => {
                if !self.pinned_roots.contains_key(&target_user_id) {
                    Ok(PendingCommitRebase::AlreadySatisfied)
                } else if self.pinned_roots.len() <= 2 {
                    Ok(PendingCommitRebase::Invalidated)
                } else {
                    self.create_remove_participant(device, target_user_id)
                        .map(PendingCommitRebase::Rebased)
                }
            }
        }
    }

    /// Stage a post-compromise self-update for Delivery Service ordering.
    ///
    /// The returned commit remains pending and blocks sends until the caller
    /// either accepts it after a successful server response or rejects it
    /// after an epoch conflict.
    ///
    /// # Errors
    /// Returns [`ConversationError`] if this device does not own the group,
    /// the group is inactive or already has a pending commit, or OpenMLS
    /// cannot create bounded commit material.
    pub fn create_self_update(
        &mut self,
        device: &MlsDevice,
    ) -> Result<PendingGroupCommit, ConversationError> {
        self.ensure_device(device)?;
        if !self.active {
            return Err(ConversationError::NotActive);
        }
        if self.group.pending_commit().is_some() {
            return Err(ConversationError::PendingCommit);
        }
        let prior_epoch = self.group.epoch().as_u64();
        let (commit, welcome, group_info) = self
            .group
            .self_update(
                device.provider(),
                device.signer(),
                LeafNodeParameters::default(),
            )
            .map_err(|_| ConversationError::CryptoError)?
            .into_contents();
        let epoch = self
            .group
            .pending_commit()
            .ok_or(ConversationError::NoPendingCommit)?
            .epoch()
            .as_u64();
        let commit_blob = commit
            .to_bytes()
            .map_err(|_| ConversationError::SerializationFailed)?;
        let welcome_blob = welcome
            .map(|value| {
                MlsMessageOut::from_welcome(value, ProtocolVersion::Mls10)
                    .to_bytes()
                    .map_err(|_| ConversationError::SerializationFailed)
            })
            .transpose()?;
        let group_info_blob = group_info
            .map(|info| {
                MlsMessageOut::from(info)
                    .to_bytes()
                    .map_err(|_| ConversationError::SerializationFailed)
            })
            .transpose()?;
        enforce_serialized_limits(
            &commit_blob,
            welcome_blob.as_deref(),
            group_info_blob.as_deref(),
        )?;
        Ok(PendingGroupCommit {
            group_id: self.group_id,
            prior_epoch,
            epoch,
            suite: CiphersuiteId::baseline(),
            committer_device_id: self.own_device_id,
            commit_blob,
            welcome_blob,
            group_info_blob,
        })
    }

    /// Stage an Add commit for one additional certified device of either user.
    ///
    /// The caller must claim the exact target device's single-use KeyPackage
    /// and send the returned Welcome only to that device. Adds are serialized
    /// one device per epoch to match the Delivery Service's recipient binding.
    ///
    /// # Errors
    /// Rejects unpinned roots, duplicate devices, pending commits, inactive
    /// groups, device/group caps, and malformed or uncertified KeyPackages.
    pub fn create_add_device(
        &mut self,
        device: &MlsDevice,
        target: PinnedUserIdentity,
        target_keypackage_blob: &[u8],
    ) -> Result<PendingGroupCommit, ConversationError> {
        self.ensure_operational(device)?;
        if self.pinned_roots.get(&target.user_id) != Some(&target.root_key_pub) {
            return Err(ConversationError::UntrustedCredential);
        }
        let member_counts = verified_member_counts(&self.group, &self.pinned_roots)?;
        if member_counts.total >= MAX_MLS_GROUP_LEAVES
            || member_counts
                .per_user
                .get(&target.user_id)
                .copied()
                .unwrap_or(0)
                >= MAX_MLS_DEVICES_PER_USER
        {
            return Err(ConversationError::LimitExceeded);
        }
        let key_package = parse_and_verify_keypackage(device, target_keypackage_blob, target)?;
        let added = verify_member_credential(
            key_package.leaf_node().credential(),
            key_package.leaf_node().signature_key().as_slice(),
            &self.pinned_roots,
        )?;
        if self.has_verified_member_device(added.device_id)? {
            return Err(ConversationError::UnexpectedMembership);
        }
        let prior_epoch = self.epoch();
        let (commit, welcome, group_info) = self
            .group
            .add_members(device.provider(), device.signer(), &[key_package])
            .map_err(|_| ConversationError::CryptoError)?;
        pending_commit_from_messages(
            self.group_id,
            prior_epoch,
            self.own_device_id,
            &self.group,
            &commit,
            Some(welcome),
            group_info,
        )
    }

    /// Stage a member-signed Add for a new root-pinned group-DM participant.
    ///
    /// The pin must come from independently verified local trust state. It is
    /// retained only if the commit is accepted; rejection removes the
    /// provisional pin before the conversation can send again.
    ///
    /// # Errors
    /// Rejects direct messages, existing/conflicting users, audience caps,
    /// malformed KeyPackages, pending commits, and inactive groups.
    pub fn create_add_participant(
        &mut self,
        device: &MlsDevice,
        target: PinnedUserIdentity,
        target_keypackage_blob: &[u8],
    ) -> Result<PendingGroupCommit, ConversationError> {
        self.ensure_operational(device)?;
        if self.audience != ConversationAudience::GroupDm
            || self.pinned_roots.len() >= MAX_MLS_GROUP_USERS
            || self.pinned_roots.contains_key(&target.user_id)
            || target.user_id == device.user_id()
        {
            return Err(ConversationError::UnexpectedMembership);
        }
        parse_and_verify_keypackage(device, target_keypackage_blob, target)?;
        self.pinned_roots
            .insert(target.user_id, target.root_key_pub);
        match self.create_add_device(device, target, target_keypackage_blob) {
            Ok(pending) => Ok(pending),
            Err(error) => {
                self.pinned_roots.remove(&target.user_id);
                Err(error)
            }
        }
    }

    /// Stage a Remove commit for one non-local device.
    ///
    /// Removing the final device of either user is rejected because a Phase 2
    /// conversation must retain at least one leaf for each pinned participant.
    ///
    /// # Errors
    /// Rejects the local device, unknown devices, final-user-device removal,
    /// pending commits, inactive groups, or OpenMLS failures.
    pub fn create_remove_device(
        &mut self,
        device: &MlsDevice,
        target_device_id: DeviceId,
    ) -> Result<PendingGroupCommit, ConversationError> {
        self.ensure_operational(device)?;
        if target_device_id == self.own_device_id {
            return Err(ConversationError::UnexpectedMembership);
        }
        let members = verified_members(&self.group, &self.pinned_roots)?;
        let (target_index, target) = members
            .iter()
            .find(|(_, member)| member.device_id == target_device_id)
            .copied()
            .ok_or(ConversationError::UnexpectedMembership)?;
        if members
            .iter()
            .filter(|(_, member)| member.user_id == target.user_id)
            .count()
            <= 1
        {
            return Err(ConversationError::UnexpectedMembership);
        }
        let prior_epoch = self.epoch();
        let (commit, welcome, group_info) = self
            .group
            .remove_members(device.provider(), device.signer(), &[target_index])
            .map_err(|_| ConversationError::CryptoError)?;
        pending_commit_from_messages(
            self.group_id,
            prior_epoch,
            self.own_device_id,
            &self.group,
            &commit,
            welcome,
            group_info,
        )
    }

    /// Stage one member-signed group-DM eviction for every device belonging
    /// to `target_user_id`.
    ///
    /// Removing all of a participant's leaves in one epoch prevents a second
    /// device from retaining access after the UI reports that user removed.
    /// Group DMs may not be reduced below two root identities.
    ///
    /// # Errors
    /// Rejects direct-message conversations, self-removal, unknown users,
    /// groups with only two participants, pending commits, and MLS failures.
    pub fn create_remove_participant(
        &mut self,
        device: &MlsDevice,
        target_user_id: UserId,
    ) -> Result<PendingGroupCommit, ConversationError> {
        self.ensure_operational(device)?;
        if self.audience != ConversationAudience::GroupDm
            || target_user_id == device.user_id()
            || self.pinned_roots.len() <= 2
        {
            return Err(ConversationError::UnexpectedMembership);
        }
        let target_indices = verified_members(&self.group, &self.pinned_roots)?
            .into_iter()
            .filter_map(|(index, member)| (member.user_id == target_user_id).then_some(index))
            .collect::<Vec<_>>();
        if target_indices.is_empty() {
            return Err(ConversationError::UnexpectedMembership);
        }
        let prior_epoch = self.epoch();
        let (commit, welcome, group_info) = self
            .group
            .remove_members(device.provider(), device.signer(), &target_indices)
            .map_err(|_| ConversationError::CryptoError)?;
        pending_commit_from_messages(
            self.group_id,
            prior_epoch,
            self.own_device_id,
            &self.group,
            &commit,
            welcome,
            group_info,
        )
    }

    /// Authenticate, inspect, and merge one ordered peer commit.
    ///
    /// Phase 2 permits updates plus a single certified device Add or safe
    /// Remove. Commits that introduce a third user, exceed device caps, remove
    /// a user's final leaf, or combine membership changes fail closed.
    ///
    /// # Errors
    /// Returns [`ConversationError`] when routing hints disagree with the
    /// authenticated commit, the epoch is not the next local epoch, the
    /// committer is not root-certified, or membership violates Phase 2 bounds.
    pub fn process_incoming_commit(
        &mut self,
        device: &MlsDevice,
        commit: &EncryptedGroupCommit,
    ) -> Result<(), ConversationError> {
        self.ensure_device(device)?;
        if !self.active {
            return Err(ConversationError::NotActive);
        }
        if self.group.pending_commit().is_some() {
            return Err(ConversationError::PendingCommit);
        }
        let protocol_message = self.validate_incoming_commit(commit)?;
        self.merge_incoming_commit(device, commit, protocol_message)
    }

    /// Authenticate and merge an Add commit for one newly pinned group-DM
    /// participant.
    ///
    /// Supplying the expected root pin is mandatory: the ordinary commit path
    /// rejects credentials from users outside the current audience. The
    /// authenticated commit must contain exactly one Add for this identity.
    ///
    /// # Errors
    /// Fails closed without merging when the commit is not the expected Add,
    /// the pin conflicts, the group is stale, or MLS authentication fails.
    pub fn process_incoming_participant_add(
        &mut self,
        device: &MlsDevice,
        commit: &EncryptedGroupCommit,
        target: PinnedUserIdentity,
    ) -> Result<(), ConversationError> {
        self.ensure_operational(device)?;
        if self.audience != ConversationAudience::GroupDm
            || self.pinned_roots.len() >= MAX_MLS_GROUP_USERS
            || self.pinned_roots.contains_key(&target.user_id)
            || target.user_id == device.user_id()
        {
            return Err(ConversationError::UnexpectedMembership);
        }
        self.pinned_roots
            .insert(target.user_id, target.root_key_pub);
        let staged = self
            .validate_incoming_commit(commit)
            .and_then(|message| self.stage_incoming_commit(device, commit, message));
        let staged = match staged {
            Ok(staged) => staged,
            Err(error) => {
                self.pinned_roots.remove(&target.user_id);
                return Err(error);
            }
        };
        let added = staged
            .add_proposals()
            .next()
            .filter(|_| staged.add_proposals().count() == 1)
            .and_then(|proposal| {
                let leaf = proposal.add_proposal().key_package().leaf_node();
                verify_member_credential(
                    leaf.credential(),
                    leaf.signature_key().as_slice(),
                    &self.pinned_roots,
                )
                .ok()
            });
        if added.is_none_or(|member| member.user_id != target.user_id) {
            self.pinned_roots.remove(&target.user_id);
            return Err(ConversationError::UnexpectedMembership);
        }
        self.merge_staged_incoming_commit(device, *staged)
    }

    fn validate_incoming_commit(
        &self,
        commit: &EncryptedGroupCommit,
    ) -> Result<ProtocolMessage, ConversationError> {
        let expected_epoch = self
            .group
            .epoch()
            .as_u64()
            .checked_add(1)
            .ok_or(ConversationError::LimitExceeded)?;
        if commit.group_id != self.group_id
            || commit.prior_epoch != self.group.epoch().as_u64()
            || commit.epoch != expected_epoch
            || commit.commit_blob.is_empty()
            || commit.commit_blob.len() > MAX_COMMIT_BYTES
        {
            return Err(ConversationError::MetadataMismatch);
        }
        let message = MlsMessageIn::tls_deserialize_exact(&commit.commit_blob)
            .map_err(|_| ConversationError::SerializationFailed)?;
        let protocol_message = message
            .try_into_protocol_message()
            .map_err(|_| ConversationError::InvalidCommit)?;
        if protocol_message.group_id().as_slice() != self.group_id.to_string().as_bytes()
            || protocol_message.epoch().as_u64() != commit.prior_epoch
            || protocol_message.content_type() != ContentType::Commit
        {
            return Err(ConversationError::MetadataMismatch);
        }
        let ProtocolMessage::PublicMessage(public_message) = &protocol_message else {
            return Err(ConversationError::InvalidCommit);
        };
        match public_message.sender() {
            Sender::Member(sender_index) => {
                let verified = self.verify_member_at(*sender_index)?;
                if verified.device_id != commit.committer_device_id {
                    return Err(ConversationError::MetadataMismatch);
                }
            }
            Sender::NewMemberCommit => {}
            _ => return Err(ConversationError::MetadataMismatch),
        }
        Ok(protocol_message)
    }

    fn merge_incoming_commit(
        &mut self,
        device: &MlsDevice,
        commit: &EncryptedGroupCommit,
        protocol_message: ProtocolMessage,
    ) -> Result<(), ConversationError> {
        let staged_commit = self.stage_incoming_commit(device, commit, protocol_message)?;
        self.merge_staged_incoming_commit(device, *staged_commit)
    }

    fn stage_incoming_commit(
        &mut self,
        device: &MlsDevice,
        commit: &EncryptedGroupCommit,
        protocol_message: ProtocolMessage,
    ) -> Result<Box<StagedCommit>, ConversationError> {
        let processed = self
            .group
            .process_message(device.provider(), protocol_message)
            .map_err(|_| ConversationError::CryptoError)?;
        let sender = processed.sender().clone();
        let ProcessedMessageContent::StagedCommitMessage(staged_commit) = processed.into_content()
        else {
            return Err(ConversationError::InvalidCommit);
        };
        match sender {
            Sender::Member(sender_index) => {
                let verified_sender = self.verify_member_at(sender_index)?;
                if verified_sender.device_id != commit.committer_device_id {
                    return Err(ConversationError::MetadataMismatch);
                }
                self.validate_staged_commit(&staged_commit, verified_sender, commit.epoch)?;
                validate_staged_membership_change(
                    &self.group,
                    &staged_commit,
                    &self.pinned_roots,
                    self.audience,
                )?;
            }
            Sender::NewMemberCommit => validate_external_commit(
                &self.group,
                &staged_commit,
                &self.pinned_roots,
                commit.committer_device_id,
                commit.epoch,
            )?,
            _ => return Err(ConversationError::UnexpectedMembership),
        }
        Ok(staged_commit)
    }

    fn merge_staged_incoming_commit(
        &mut self,
        device: &MlsDevice,
        staged_commit: StagedCommit,
    ) -> Result<(), ConversationError> {
        let evicted_users = fully_removed_users(&self.group, &staged_commit, &self.pinned_roots)?;
        self.group
            .merge_staged_commit(device.provider(), staged_commit)
            .map_err(|_| ConversationError::CryptoError)?;
        self.remove_evicted_group_pins(&evicted_users, device.user_id());
        self.prune_unrepresented_group_pins(device.user_id())?;
        self.active = self.group.is_active();
        validate_group(
            &self.group,
            &self.pinned_roots,
            self.audience,
            self.active.then_some(self.own_device_id),
        )?;
        self.prune_inbound_for_current_members()
    }

    fn remove_evicted_group_pins(&mut self, evicted_users: &[UserId], own_user_id: UserId) {
        if self.audience == ConversationAudience::GroupDm {
            for user_id in evicted_users {
                if *user_id != own_user_id {
                    self.pinned_roots.remove(user_id);
                }
            }
        }
    }

    fn prune_unrepresented_group_pins(
        &mut self,
        own_user_id: UserId,
    ) -> Result<(), ConversationError> {
        if self.audience != ConversationAudience::GroupDm {
            return Ok(());
        }
        let represented = verified_members(&self.group, &self.pinned_roots)?
            .into_iter()
            .map(|(_, member)| member.user_id)
            .collect::<std::collections::HashSet<_>>();
        self.pinned_roots
            .retain(|user_id, _| *user_id == own_user_id || represented.contains(user_id));
        Ok(())
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
        let serialized_message = message
            .to_bytes()
            .map_err(|_| ConversationError::SerializationFailed)?;
        let message_blob = pad_transport_message(serialized_message)?;
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
        if !MESSAGE_TRANSPORT_PADDING_BUCKETS.contains(&message.message_blob.len()) {
            return Err(ConversationError::LimitExceeded);
        }
        let wire_message = deserialize_transport_message(&message.message_blob)?;
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

    /// Locally enforced conversation audience policy.
    #[must_use]
    pub const fn audience(&self) -> ConversationAudience {
        self.audience
    }

    /// Whether authenticated messages remain buffered behind a missing
    /// per-sender application generation.
    #[must_use]
    pub fn messages_may_be_missing(&self) -> bool {
        self.inbound.values().any(|queue| !queue.pending.is_empty())
    }

    pub(crate) fn has_verified_member_device(
        &self,
        device_id: DeviceId,
    ) -> Result<bool, ConversationError> {
        for member in self.group.members() {
            let verified = verify_member_credential(
                &member.credential,
                &member.signature_key,
                &self.pinned_roots,
            )?;
            if verified.device_id == device_id {
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub(crate) fn persistence_metadata(&self) -> ConversationPersistenceMetadata {
        let mut pinned_roots = self
            .pinned_roots
            .iter()
            .map(|(user_id, root_key_pub)| (*user_id, *root_key_pub))
            .collect::<Vec<_>>();
        pinned_roots.sort_by_key(|(user_id, _)| user_id.to_string());
        let mut inbound = self
            .inbound
            .iter()
            .map(|(device_id, queue)| InboundPersistenceMetadata {
                device_id: *device_id,
                next_generation: queue.next_generation,
                pending: queue.pending.values().cloned().collect(),
            })
            .collect::<Vec<_>>();
        inbound.sort_by_key(|queue| queue.device_id.to_string());
        ConversationPersistenceMetadata {
            group_id: self.group_id,
            epoch: self.epoch(),
            own_device_id: self.own_device_id,
            audience: self.audience,
            pinned_roots,
            outbound_generation: self.outbound_generation,
            inbound,
            active: self.active,
        }
    }

    pub(crate) fn restore(
        device: &MlsDevice,
        metadata: ConversationPersistenceMetadata,
    ) -> Result<Self, ConversationError> {
        if metadata.own_device_id != device.device_id()
            || !valid_root_count(metadata.audience, metadata.pinned_roots.len())
        {
            return Err(ConversationError::UnexpectedMembership);
        }
        let pinned_roots = metadata.pinned_roots.into_iter().collect::<HashMap<_, _>>();
        if !valid_root_count(metadata.audience, pinned_roots.len())
            || pinned_roots.get(&device.user_id()) != Some(device.root_key_public())
        {
            return Err(ConversationError::UntrustedCredential);
        }
        let openmls_group_id =
            openmls::prelude::GroupId::from_slice(metadata.group_id.to_string().as_bytes());
        let group = MlsGroup::load(device.provider().storage(), &openmls_group_id)
            .map_err(|_| ConversationError::CryptoError)?
            .ok_or(ConversationError::CryptoError)?;
        validate_restored_group(
            &group,
            &pinned_roots,
            metadata.group_id,
            metadata.epoch,
            metadata.own_device_id,
            metadata.audience,
            metadata.active,
        )?;

        let mut inbound = HashMap::with_capacity(metadata.inbound.len());
        for queue in metadata.inbound {
            if queue.pending.len()
                > usize::try_from(MAX_BUFFERED_GENERATION_GAP).unwrap_or(usize::MAX)
            {
                return Err(ConversationError::LimitExceeded);
            }
            let verified = group
                .members()
                .find_map(|member| {
                    verify_member_credential(
                        &member.credential,
                        &member.signature_key,
                        &pinned_roots,
                    )
                    .ok()
                    .filter(|member| member.device_id == queue.device_id)
                })
                .ok_or(ConversationError::UnexpectedMembership)?;
            let mut pending = BTreeMap::new();
            for message in queue.pending {
                if message.sender_device_id != queue.device_id
                    || message.sender_user_id != verified.user_id
                    || message.generation < queue.next_generation
                    || message.generation.saturating_sub(queue.next_generation)
                        > MAX_BUFFERED_GENERATION_GAP
                    || message.plaintext.is_empty()
                    || message.plaintext.len() > MAX_APPLICATION_PLAINTEXT_BYTES
                    || pending.insert(message.generation, message).is_some()
                {
                    return Err(ConversationError::MetadataMismatch);
                }
            }
            if inbound
                .insert(
                    queue.device_id,
                    InboundGenerationQueue {
                        next_generation: queue.next_generation,
                        pending,
                    },
                )
                .is_some()
            {
                return Err(ConversationError::MetadataMismatch);
            }
        }
        Ok(Self {
            group_id: metadata.group_id,
            group,
            own_device_id: metadata.own_device_id,
            audience: metadata.audience,
            pinned_roots,
            outbound_generation: metadata.outbound_generation,
            inbound,
            active: metadata.active,
        })
    }

    fn ensure_device(&self, device: &MlsDevice) -> Result<(), ConversationError> {
        if device.device_id() != self.own_device_id {
            return Err(ConversationError::UnexpectedMembership);
        }
        Ok(())
    }

    fn ensure_operational(&self, device: &MlsDevice) -> Result<(), ConversationError> {
        self.ensure_device(device)?;
        if !self.active {
            return Err(ConversationError::NotActive);
        }
        if self.group.pending_commit().is_some() {
            return Err(ConversationError::PendingCommit);
        }
        Ok(())
    }

    fn pending_commit_intent(&self) -> Result<PendingCommitIntent, ConversationError> {
        let pending = self
            .group
            .pending_commit()
            .ok_or(ConversationError::NoPendingCommit)?;
        let queued_count = pending.queued_proposals().count();
        let add_count = pending.add_proposals().count();
        let remove_count = pending.remove_proposals().count();
        let update_count = pending.update_proposals().count();

        if queued_count == 0
            && add_count == 0
            && remove_count == 0
            && update_count == 0
            && pending.update_path_leaf_node().is_some()
        {
            return Ok(PendingCommitIntent::SelfUpdate);
        }
        if queued_count == 1 && add_count == 1 && remove_count == 0 && update_count == 0 {
            let add = pending
                .add_proposals()
                .next()
                .ok_or(ConversationError::UnexpectedMembership)?;
            let key_package = add.add_proposal().key_package();
            let added = verify_member_credential(
                key_package.leaf_node().credential(),
                key_package.leaf_node().signature_key().as_slice(),
                &self.pinned_roots,
            )?;
            let root_key_pub = *self
                .pinned_roots
                .get(&added.user_id)
                .ok_or(ConversationError::UntrustedCredential)?;
            let keypackage_blob = key_package
                .tls_serialize_detached()
                .map_err(|_| ConversationError::SerializationFailed)?;
            if keypackage_blob.is_empty() || keypackage_blob.len() > MAX_KEYPACKAGE_BYTES {
                return Err(ConversationError::LimitExceeded);
            }
            return Ok(PendingCommitIntent::AddDevice {
                target: PinnedUserIdentity::new(added.user_id, root_key_pub),
                target_device_id: added.device_id,
                keypackage_blob,
            });
        }
        if self.audience == ConversationAudience::GroupDm
            && queued_count == remove_count
            && add_count == 0
            && remove_count > 0
            && update_count == 0
        {
            let removed_users = fully_removed_users(&self.group, pending, &self.pinned_roots)?;
            if removed_users.len() == 1 {
                return Ok(PendingCommitIntent::RemoveParticipant {
                    target_user_id: removed_users[0],
                });
            }
        }
        if queued_count == 1 && add_count == 0 && remove_count == 1 && update_count == 0 {
            let removed_index = pending
                .remove_proposals()
                .next()
                .ok_or(ConversationError::UnexpectedMembership)?
                .remove_proposal()
                .removed();
            let target_device_id = self
                .verify_member_at(removed_index)
                .map(|member| member.device_id)?;
            return Ok(PendingCommitIntent::RemoveDevice { target_device_id });
        }
        Err(ConversationError::UnexpectedMembership)
    }

    fn prune_inbound_for_current_members(&mut self) -> Result<(), ConversationError> {
        let members = verified_members(&self.group, &self.pinned_roots)?;
        let device_ids = members
            .into_iter()
            .map(|(_, member)| member.device_id)
            .collect::<std::collections::HashSet<_>>();
        self.inbound
            .retain(|device_id, _| device_ids.contains(device_id));
        Ok(())
    }

    fn validate_staged_commit(
        &self,
        staged_commit: &StagedCommit,
        verified_sender: VerifiedMember,
        expected_epoch: u64,
    ) -> Result<(), ConversationError> {
        if staged_commit.epoch().as_u64() != expected_epoch {
            return Err(ConversationError::UnexpectedMembership);
        }
        if let Some(leaf) = staged_commit.update_path_leaf_node() {
            let path_member = verify_member_credential(
                leaf.credential(),
                leaf.signature_key().as_slice(),
                &self.pinned_roots,
            )?;
            if path_member != verified_sender {
                return Err(ConversationError::UnexpectedMembership);
            }
        }
        for update in staged_commit.update_proposals() {
            let leaf = update.update_proposal().leaf_node();
            let updated = verify_member_credential(
                leaf.credential(),
                leaf.signature_key().as_slice(),
                &self.pinned_roots,
            )?;
            let Sender::Member(updated_index) = update.sender() else {
                return Err(ConversationError::UnexpectedMembership);
            };
            if self.verify_member_at(*updated_index)? != updated {
                return Err(ConversationError::UnexpectedMembership);
            }
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
        let authenticated_message = DecryptedApplicationMessage {
            sender_user_id: sender.user_id,
            sender_device_id: sender.device_id,
            generation,
            plaintext,
        };
        queue
            .pending
            .insert(generation, authenticated_message.clone());
        let mut ready_messages = Vec::new();
        while let Some(ready) = queue.pending.remove(&queue.next_generation) {
            ready_messages.push(ready);
            queue.next_generation = queue
                .next_generation
                .checked_add(1)
                .ok_or(ConversationError::LimitExceeded)?;
        }
        Ok(DecryptionOutcome {
            authenticated_message,
            ready_messages,
            messages_may_be_missing: !queue.pending.is_empty(),
        })
    }
}

pub(crate) struct ConversationPersistenceMetadata {
    pub group_id: FilamentGroupId,
    pub epoch: u64,
    pub own_device_id: DeviceId,
    pub audience: ConversationAudience,
    pub pinned_roots: Vec<(UserId, [u8; 32])>,
    pub outbound_generation: u64,
    pub inbound: Vec<InboundPersistenceMetadata>,
    pub active: bool,
}

pub(crate) struct InboundPersistenceMetadata {
    pub device_id: DeviceId,
    pub next_generation: u64,
    pub pending: Vec<DecryptedApplicationMessage>,
}

fn validate_restored_group(
    group: &MlsGroup,
    pinned_roots: &HashMap<UserId, [u8; 32]>,
    group_id: FilamentGroupId,
    epoch: u64,
    own_device_id: DeviceId,
    audience: ConversationAudience,
    active: bool,
) -> Result<(), ConversationError> {
    if group.epoch().as_u64() != epoch
        || group.group_id().as_slice() != group_id.to_string().as_bytes()
        || group.ciphersuite() != CIPHERSUITE
        || (active && !group.is_active())
    {
        return Err(ConversationError::MetadataMismatch);
    }
    if active {
        return validate_group(group, pinned_roots, audience, Some(own_device_id));
    }
    if epoch == 0 {
        return validate_initial_pending_group(group, pinned_roots, audience, own_device_id);
    }
    if group.is_active() {
        return Err(ConversationError::MetadataMismatch);
    }
    validate_group(group, pinned_roots, audience, None)?;
    if verified_members(group, pinned_roots)?
        .iter()
        .any(|(_, member)| member.device_id == own_device_id)
    {
        return Err(ConversationError::MetadataMismatch);
    }
    Ok(())
}

fn pad_transport_message(mut serialized_message: Vec<u8>) -> Result<Vec<u8>, ConversationError> {
    if serialized_message.is_empty() || serialized_message.len() > MAX_MLS_MESSAGE_BYTES {
        return Err(ConversationError::LimitExceeded);
    }
    let bucket = MESSAGE_TRANSPORT_PADDING_BUCKETS
        .into_iter()
        .find(|bucket| serialized_message.len() <= *bucket)
        .ok_or(ConversationError::LimitExceeded)?;
    serialized_message.resize(bucket, 0);
    Ok(serialized_message)
}

fn deserialize_transport_message(bytes: &[u8]) -> Result<MlsMessageIn, ConversationError> {
    let mut encoded = bytes;
    let message = MlsMessageIn::tls_deserialize(&mut encoded)
        .map_err(|_| ConversationError::SerializationFailed)?;
    if encoded.iter().any(|byte| *byte != 0) {
        return Err(ConversationError::SerializationFailed);
    }
    Ok(message)
}

pub(crate) fn validate_commit_envelope(
    commit: &EncryptedGroupCommit,
) -> Result<(), ConversationError> {
    if commit.commit_blob.is_empty() || commit.commit_blob.len() > MAX_COMMIT_BYTES {
        return Err(ConversationError::LimitExceeded);
    }
    let message = MlsMessageIn::tls_deserialize_exact(&commit.commit_blob)
        .map_err(|_| ConversationError::SerializationFailed)?;
    let protocol_message = message
        .try_into_protocol_message()
        .map_err(|_| ConversationError::InvalidCommit)?;
    if protocol_message.group_id().as_slice() != commit.group_id.to_string().as_bytes()
        || protocol_message.epoch().as_u64() != commit.prior_epoch
        || protocol_message.content_type() != ContentType::Commit
    {
        return Err(ConversationError::MetadataMismatch);
    }
    Ok(())
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

struct VerifiedMemberCounts {
    total: usize,
    per_user: HashMap<UserId, usize>,
}

fn verified_members(
    group: &MlsGroup,
    pinned_roots: &HashMap<UserId, [u8; 32]>,
) -> Result<Vec<(LeafNodeIndex, VerifiedMember)>, ConversationError> {
    let mut members = Vec::new();
    let mut device_ids = std::collections::HashSet::new();
    for member in group.members() {
        let verified =
            verify_member_credential(&member.credential, &member.signature_key, pinned_roots)?;
        if !device_ids.insert(verified.device_id) {
            return Err(ConversationError::UnexpectedMembership);
        }
        members.push((member.index, verified));
    }
    Ok(members)
}

fn verified_member_counts(
    group: &MlsGroup,
    pinned_roots: &HashMap<UserId, [u8; 32]>,
) -> Result<VerifiedMemberCounts, ConversationError> {
    let members = verified_members(group, pinned_roots)?;
    let mut per_user = HashMap::with_capacity(2);
    for (_, member) in &members {
        *per_user.entry(member.user_id).or_insert(0) += 1;
    }
    Ok(VerifiedMemberCounts {
        total: members.len(),
        per_user,
    })
}

fn fully_removed_users(
    group: &MlsGroup,
    staged_commit: &StagedCommit,
    pinned_roots: &HashMap<UserId, [u8; 32]>,
) -> Result<Vec<UserId>, ConversationError> {
    let members = verified_members(group, pinned_roots)?;
    let mut current_counts = HashMap::<UserId, usize>::new();
    for (_, member) in &members {
        *current_counts.entry(member.user_id).or_insert(0) += 1;
    }
    let mut removed_counts = HashMap::<UserId, usize>::new();
    for remove in staged_commit.remove_proposals() {
        let index = remove.remove_proposal().removed();
        let removed = members
            .iter()
            .find(|(member_index, _)| *member_index == index)
            .map(|(_, member)| *member)
            .ok_or(ConversationError::UnexpectedMembership)?;
        *removed_counts.entry(removed.user_id).or_insert(0) += 1;
    }
    let replacement_user = staged_commit
        .update_path_leaf_node()
        .map(|leaf| {
            verify_member_credential(
                leaf.credential(),
                leaf.signature_key().as_slice(),
                pinned_roots,
            )
            .map(|member| member.user_id)
        })
        .transpose()?;
    Ok(removed_counts
        .into_iter()
        .filter_map(|(user_id, removed)| {
            (current_counts.get(&user_id) == Some(&removed) && replacement_user != Some(user_id))
                .then_some(user_id)
        })
        .collect())
}

fn valid_root_count(audience: ConversationAudience, count: usize) -> bool {
    match audience {
        ConversationAudience::DirectMessage => count == 2,
        ConversationAudience::GroupDm => (2..=MAX_MLS_GROUP_USERS).contains(&count),
    }
}

fn validate_group(
    group: &MlsGroup,
    pinned_roots: &HashMap<UserId, [u8; 32]>,
    audience: ConversationAudience,
    required_device_id: Option<DeviceId>,
) -> Result<(), ConversationError> {
    let members = verified_members(group, pinned_roots)?;
    let counts = verified_member_counts(group, pinned_roots)?;
    let missing_root_count = pinned_roots
        .keys()
        .filter(|user_id| !counts.per_user.contains_key(user_id))
        .count();
    let representation_is_valid = match audience {
        ConversationAudience::DirectMessage => missing_root_count == 0,
        ConversationAudience::GroupDm => {
            missing_root_count == 0 || (required_device_id.is_none() && missing_root_count == 1)
        }
    };
    if !valid_root_count(audience, pinned_roots.len())
        || counts.total < 2
        || counts.total > MAX_MLS_GROUP_LEAVES
        || required_device_id.is_some_and(|required| {
            !members
                .iter()
                .any(|(_, member)| member.device_id == required)
        })
        || counts
            .per_user
            .values()
            .any(|count| !(1..=MAX_MLS_DEVICES_PER_USER).contains(count))
        || !representation_is_valid
    {
        return Err(ConversationError::UnexpectedMembership);
    }
    Ok(())
}

fn validate_staged_membership_change(
    group: &MlsGroup,
    staged_commit: &StagedCommit,
    pinned_roots: &HashMap<UserId, [u8; 32]>,
    audience: ConversationAudience,
) -> Result<(), ConversationError> {
    let queued_count = staged_commit.queued_proposals().count();
    let update_count = staged_commit.update_proposals().count();
    let add_count = staged_commit.add_proposals().count();
    let remove_count = staged_commit.remove_proposals().count();
    let membership_count = add_count
        .checked_add(remove_count)
        .ok_or(ConversationError::LimitExceeded)?;
    if membership_count == 0 {
        if queued_count != update_count {
            return Err(ConversationError::UnexpectedMembership);
        }
        return Ok(());
    }
    if update_count != 0
        || match audience {
            ConversationAudience::DirectMessage => membership_count != 1 || queued_count != 1,
            ConversationAudience::GroupDm => {
                membership_count == 0
                    || queued_count != membership_count
                    || (add_count > 0 && remove_count > 0)
                    || add_count > 1
            }
        }
    {
        return Err(ConversationError::UnexpectedMembership);
    }

    let members = verified_members(group, pinned_roots)?;
    if add_count == 1 {
        let counts = verified_member_counts(group, pinned_roots)?;
        let add = staged_commit
            .add_proposals()
            .next()
            .ok_or(ConversationError::UnexpectedMembership)?;
        let leaf = add.add_proposal().key_package().leaf_node();
        let added = verify_member_credential(
            leaf.credential(),
            leaf.signature_key().as_slice(),
            pinned_roots,
        )?;
        if members
            .iter()
            .any(|(_, member)| member.device_id == added.device_id)
            || counts.total >= MAX_MLS_GROUP_LEAVES
            || counts.per_user.get(&added.user_id).copied().unwrap_or(0) >= MAX_MLS_DEVICES_PER_USER
        {
            return Err(ConversationError::UnexpectedMembership);
        }
        return Ok(());
    }

    let removed = staged_commit
        .remove_proposals()
        .map(|proposal| {
            let removed_index = proposal.remove_proposal().removed();
            members
                .iter()
                .find(|(index, _)| *index == removed_index)
                .map(|(_, member)| *member)
                .ok_or(ConversationError::UnexpectedMembership)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let target_user = removed
        .first()
        .map(|member| member.user_id)
        .ok_or(ConversationError::UnexpectedMembership)?;
    let current_target_count = members
        .iter()
        .filter(|(_, member)| member.user_id == target_user)
        .count();
    if removed.iter().any(|member| member.user_id != target_user)
        || match audience {
            ConversationAudience::DirectMessage => current_target_count <= removed.len(),
            ConversationAudience::GroupDm => {
                removed.len() > current_target_count
                    || (removed.len() == current_target_count && pinned_roots.len() <= 2)
            }
        }
    {
        return Err(ConversationError::UnexpectedMembership);
    }
    Ok(())
}

fn validate_external_commit(
    group: &MlsGroup,
    staged_commit: &StagedCommit,
    pinned_roots: &HashMap<UserId, [u8; 32]>,
    committer_device_id: DeviceId,
    expected_epoch: u64,
) -> Result<(), ConversationError> {
    if staged_commit.epoch().as_u64() != expected_epoch {
        return Err(ConversationError::MetadataMismatch);
    }
    let joining_leaf = staged_commit
        .update_path_leaf_node()
        .ok_or(ConversationError::UnexpectedMembership)?;
    let joining_member = verify_member_credential(
        joining_leaf.credential(),
        joining_leaf.signature_key().as_slice(),
        pinned_roots,
    )?;
    if joining_member.device_id != committer_device_id {
        return Err(ConversationError::MetadataMismatch);
    }

    let proposals = staged_commit.queued_proposals().collect::<Vec<_>>();
    let external_init_count = proposals
        .iter()
        .filter(|proposal| proposal.proposal().proposal_type() == ProposalType::ExternalInit)
        .count();
    let remove_count = staged_commit.remove_proposals().count();
    if external_init_count != 1
        || remove_count > 1
        || proposals.len() != 1 + remove_count
        || proposals
            .iter()
            .any(|proposal| proposal.sender() != &Sender::NewMemberCommit)
    {
        return Err(ConversationError::UnexpectedMembership);
    }

    let members = verified_members(group, pinned_roots)?;
    let existing = members
        .iter()
        .find(|(_, member)| member.device_id == joining_member.device_id);
    match (existing, staged_commit.remove_proposals().next()) {
        (Some((existing_index, _)), Some(remove))
            if remove.remove_proposal().removed() == *existing_index =>
        {
            Ok(())
        }
        (None, None) => {
            let counts = verified_member_counts(group, pinned_roots)?;
            if counts.total >= MAX_MLS_GROUP_LEAVES
                || counts
                    .per_user
                    .get(&joining_member.user_id)
                    .copied()
                    .unwrap_or(0)
                    >= MAX_MLS_DEVICES_PER_USER
            {
                return Err(ConversationError::UnexpectedMembership);
            }
            Ok(())
        }
        _ => Err(ConversationError::UnexpectedMembership),
    }
}

fn validate_initial_pending_group(
    group: &MlsGroup,
    pinned_roots: &HashMap<UserId, [u8; 32]>,
    audience: ConversationAudience,
    own_device_id: DeviceId,
) -> Result<(), ConversationError> {
    if group.epoch().as_u64() != 0 || !valid_root_count(audience, pinned_roots.len()) {
        return Err(ConversationError::UnexpectedMembership);
    }
    let members = group.members().collect::<Vec<_>>();
    if members.len() != 1 {
        return Err(ConversationError::UnexpectedMembership);
    }
    let own_member = verify_member_credential(
        &members[0].credential,
        &members[0].signature_key,
        pinned_roots,
    )?;
    if own_member.device_id != own_device_id {
        return Err(ConversationError::UnexpectedMembership);
    }
    let pending = group
        .pending_commit()
        .ok_or(ConversationError::NoPendingCommit)?;
    let expected_adds = pinned_roots.len().saturating_sub(1);
    if pending.epoch().as_u64() != 1
        || pending.queued_proposals().count() != expected_adds
        || pending.add_proposals().count() != expected_adds
    {
        return Err(ConversationError::UnexpectedMembership);
    }
    let mut added_users = std::collections::HashSet::with_capacity(expected_adds);
    for add in pending.add_proposals() {
        let added_leaf = add.add_proposal().key_package().leaf_node();
        let added = verify_member_credential(
            added_leaf.credential(),
            added_leaf.signature_key().as_slice(),
            pinned_roots,
        )?;
        if added.device_id == own_device_id
            || added.user_id == own_member.user_id
            || !added_users.insert(added.user_id)
        {
            return Err(ConversationError::UnexpectedMembership);
        }
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

fn pending_commit_from_messages(
    group_id: FilamentGroupId,
    prior_epoch: u64,
    committer_device_id: DeviceId,
    group: &MlsGroup,
    commit: &MlsMessageOut,
    welcome: Option<MlsMessageOut>,
    group_info: Option<GroupInfo>,
) -> Result<PendingGroupCommit, ConversationError> {
    let epoch = group
        .pending_commit()
        .ok_or(ConversationError::NoPendingCommit)?
        .epoch()
        .as_u64();
    if prior_epoch.checked_add(1) != Some(epoch) {
        return Err(ConversationError::MetadataMismatch);
    }
    let commit_blob = commit
        .to_bytes()
        .map_err(|_| ConversationError::SerializationFailed)?;
    let welcome_blob = welcome
        .map(|message| {
            message
                .to_bytes()
                .map_err(|_| ConversationError::SerializationFailed)
        })
        .transpose()?;
    let group_info_blob = group_info
        .map(|info| {
            MlsMessageOut::from(info)
                .to_bytes()
                .map_err(|_| ConversationError::SerializationFailed)
        })
        .transpose()?;
    enforce_serialized_limits(
        &commit_blob,
        welcome_blob.as_deref(),
        group_info_blob.as_deref(),
    )?;
    Ok(PendingGroupCommit {
        group_id,
        prior_epoch,
        epoch,
        suite: CiphersuiteId::baseline(),
        committer_device_id,
        commit_blob,
        welcome_blob,
        group_info_blob,
    })
}

fn enforce_serialized_limits(
    commit_blob: &[u8],
    welcome_blob: Option<&[u8]>,
    group_info_blob: Option<&[u8]>,
) -> Result<(), ConversationError> {
    if commit_blob.is_empty()
        || commit_blob.len() > MAX_COMMIT_BYTES
        || welcome_blob.is_some_and(|blob| blob.is_empty() || blob.len() > MAX_WELCOME_BYTES)
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
    let unpadded_len = APPLICATION_HEADER_BYTES
        .checked_add(plaintext.len())
        .ok_or(ConversationError::LimitExceeded)?;
    let padded_len = APPLICATION_PADDING_BUCKETS
        .into_iter()
        .find(|bucket| unpadded_len <= *bucket)
        .ok_or(ConversationError::LimitExceeded)?;
    let mut encoded = Vec::with_capacity(padded_len);
    encoded.extend_from_slice(&APPLICATION_ENVELOPE_VERSION.to_be_bytes());
    encoded.extend_from_slice(&generation.to_be_bytes());
    encoded.extend_from_slice(&content_len.to_be_bytes());
    encoded.extend_from_slice(plaintext);
    encoded.resize(padded_len, 0);
    Ok(encoded)
}

fn decode_application_payload(bytes: &[u8]) -> Result<(u64, Vec<u8>), ConversationError> {
    if !APPLICATION_PADDING_BUCKETS.contains(&bytes.len()) {
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
    let content_end = APPLICATION_HEADER_BYTES
        .checked_add(content_len)
        .ok_or(ConversationError::InvalidApplicationMessage)?;
    if content_len == 0
        || content_len > MAX_APPLICATION_PLAINTEXT_BYTES
        || content_end > bytes.len()
        || bytes[content_end..].iter().any(|byte| *byte != 0)
    {
        return Err(ConversationError::InvalidApplicationMessage);
    }
    Ok((
        generation,
        bytes[APPLICATION_HEADER_BYTES..content_end].to_vec(),
    ))
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
            pending.welcome_blob.as_deref().unwrap(),
        )
        .unwrap();
        (
            fixture.alice,
            alice_conversation,
            fixture.bob,
            bob_conversation,
        )
    }

    fn encrypted_commit(pending: &PendingGroupCommit) -> EncryptedGroupCommit {
        EncryptedGroupCommit {
            group_id: pending.group_id,
            prior_epoch: pending.prior_epoch,
            epoch: pending.epoch,
            committer_device_id: pending.committer_device_id,
            commit_blob: pending.commit_blob.clone(),
        }
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn group_participant_add_and_eviction_are_fail_closed() {
        let alice_root = RootIdentityKey::generate();
        let bob_root = RootIdentityKey::generate();
        let charlie_root = RootIdentityKey::generate();
        let dave_root = RootIdentityKey::generate();
        let alice = MlsDevice::generate(UserId::new(), DeviceId::new(), &alice_root).unwrap();
        let bob = MlsDevice::generate(UserId::new(), DeviceId::new(), &bob_root).unwrap();
        let bob_second = MlsDevice::generate(bob.user_id(), DeviceId::new(), &bob_root).unwrap();
        let charlie = MlsDevice::generate(UserId::new(), DeviceId::new(), &charlie_root).unwrap();
        let dave = MlsDevice::generate(UserId::new(), DeviceId::new(), &dave_root).unwrap();
        let alice_pin = PinnedUserIdentity::new(alice.user_id(), *alice.root_key_public());
        let bob_pin = PinnedUserIdentity::new(bob.user_id(), *bob.root_key_public());
        let charlie_pin = PinnedUserIdentity::new(charlie.user_id(), *charlie.root_key_public());
        let dave_pin = PinnedUserIdentity::new(dave.user_id(), *dave.root_key_public());
        let bob_package = generate_key_package_batch(&bob, 1).unwrap().remove(0).blob;
        let charlie_package = generate_key_package_batch(&charlie, 1)
            .unwrap()
            .remove(0)
            .blob;
        let group_id = FilamentGroupId::new();
        let (mut alice_group, initial) = MlsConversation::create_group(
            group_id,
            &alice,
            &[(bob_pin, bob_package), (charlie_pin, charlie_package)],
        )
        .unwrap();
        assert_eq!(alice_group.audience(), ConversationAudience::GroupDm);
        alice_group.accept_pending_commit(&alice).unwrap();
        let mut bob_group = MlsConversation::join_group_from_welcome(
            group_id,
            &bob,
            &[alice_pin, charlie_pin],
            initial.welcome_blob.as_deref().unwrap(),
        )
        .unwrap();
        let mut charlie_group = MlsConversation::join_group_from_welcome(
            group_id,
            &charlie,
            &[alice_pin, bob_pin],
            initial.welcome_blob.as_deref().unwrap(),
        )
        .unwrap();

        let bob_second_package = generate_key_package_batch(&bob_second, 1)
            .unwrap()
            .remove(0)
            .blob;
        let add_bob_second = alice_group
            .create_add_device(&alice, bob_pin, &bob_second_package)
            .unwrap();
        alice_group.accept_pending_commit(&alice).unwrap();
        bob_group
            .process_incoming_commit(&bob, &encrypted_commit(&add_bob_second))
            .unwrap();
        charlie_group
            .process_incoming_commit(&charlie, &encrypted_commit(&add_bob_second))
            .unwrap();
        let mut bob_second_group = MlsConversation::join_group_from_welcome(
            group_id,
            &bob_second,
            &[alice_pin, charlie_pin],
            add_bob_second.welcome_blob.as_deref().unwrap(),
        )
        .unwrap();

        let dave_package = generate_key_package_batch(&dave, 1).unwrap().remove(0).blob;
        let add_dave = alice_group
            .create_add_participant(&alice, dave_pin, &dave_package)
            .unwrap();
        alice_group.accept_pending_commit(&alice).unwrap();

        // A server-delivered Add is rejected until Dave's root has been
        // independently pinned by this client.
        assert_eq!(
            bob_group
                .process_incoming_commit(&bob, &encrypted_commit(&add_dave))
                .unwrap_err(),
            ConversationError::UntrustedCredential
        );
        bob_group
            .process_incoming_participant_add(&bob, &encrypted_commit(&add_dave), dave_pin)
            .unwrap();
        charlie_group
            .process_incoming_participant_add(&charlie, &encrypted_commit(&add_dave), dave_pin)
            .unwrap();
        bob_second_group
            .process_incoming_participant_add(&bob_second, &encrypted_commit(&add_dave), dave_pin)
            .unwrap();
        let mut dave_group = MlsConversation::join_group_from_welcome(
            group_id,
            &dave,
            &[alice_pin, bob_pin, charlie_pin],
            add_dave.welcome_blob.as_deref().unwrap(),
        )
        .unwrap();

        let before_eviction = dave_group
            .encrypt_application_message(&dave, b"four-member group")
            .unwrap();
        assert_eq!(
            bob_group
                .decrypt_application_message(&bob, &before_eviction)
                .unwrap()
                .ready_messages[0]
                .plaintext,
            b"four-member group"
        );

        let remove_bob = alice_group
            .create_remove_participant(&alice, bob.user_id())
            .unwrap();
        alice_group.accept_pending_commit(&alice).unwrap();
        charlie_group
            .process_incoming_commit(&charlie, &encrypted_commit(&remove_bob))
            .unwrap();
        dave_group
            .process_incoming_commit(&dave, &encrypted_commit(&remove_bob))
            .unwrap();
        bob_group
            .process_incoming_commit(&bob, &encrypted_commit(&remove_bob))
            .unwrap();
        bob_second_group
            .process_incoming_commit(&bob_second, &encrypted_commit(&remove_bob))
            .unwrap();
        assert_eq!(
            bob_group
                .encrypt_application_message(&bob, b"evicted")
                .unwrap_err(),
            ConversationError::NotActive
        );
        assert_eq!(
            bob_second_group
                .encrypt_application_message(&bob_second, b"second device evicted")
                .unwrap_err(),
            ConversationError::NotActive
        );
        let after_eviction = charlie_group
            .encrypt_application_message(&charlie, b"post-eviction epoch")
            .unwrap();
        assert_eq!(
            bob_group
                .decrypt_application_message(&bob, &after_eviction)
                .unwrap_err(),
            ConversationError::NotActive
        );
        assert_eq!(
            dave_group
                .decrypt_application_message(&dave, &after_eviction)
                .unwrap()
                .ready_messages[0]
                .plaintext,
            b"post-eviction epoch"
        );

        let store = crate::InMemoryKeyStore::new();
        crate::persist_mls_client_state(&store, &dave, &[&dave_group]).unwrap();
        let mut restored = crate::load_mls_client_state(&store).unwrap();
        assert_eq!(
            restored.conversations[0].audience(),
            ConversationAudience::GroupDm
        );
        let after_restart = alice_group
            .encrypt_application_message(&alice, b"group state persisted")
            .unwrap();
        assert_eq!(
            restored.conversations[0]
                .decrypt_application_message(&restored.device, &after_restart)
                .unwrap()
                .ready_messages[0]
                .plaintext,
            b"group state persisted"
        );
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn group_participant_add_rebases_on_authenticated_commit_winner() {
        let alice_root = RootIdentityKey::generate();
        let bob_root = RootIdentityKey::generate();
        let charlie_root = RootIdentityKey::generate();
        let dave_root = RootIdentityKey::generate();
        let alice = MlsDevice::generate(UserId::new(), DeviceId::new(), &alice_root).unwrap();
        let bob = MlsDevice::generate(UserId::new(), DeviceId::new(), &bob_root).unwrap();
        let charlie = MlsDevice::generate(UserId::new(), DeviceId::new(), &charlie_root).unwrap();
        let dave = MlsDevice::generate(UserId::new(), DeviceId::new(), &dave_root).unwrap();
        let alice_pin = PinnedUserIdentity::new(alice.user_id(), *alice.root_key_public());
        let bob_pin = PinnedUserIdentity::new(bob.user_id(), *bob.root_key_public());
        let charlie_pin = PinnedUserIdentity::new(charlie.user_id(), *charlie.root_key_public());
        let dave_pin = PinnedUserIdentity::new(dave.user_id(), *dave.root_key_public());
        let bob_package = generate_key_package_batch(&bob, 1).unwrap().remove(0).blob;
        let charlie_package = generate_key_package_batch(&charlie, 1)
            .unwrap()
            .remove(0)
            .blob;
        let group_id = FilamentGroupId::new();
        let (mut alice_group, initial) = MlsConversation::create_group(
            group_id,
            &alice,
            &[(bob_pin, bob_package), (charlie_pin, charlie_package)],
        )
        .unwrap();
        alice_group.accept_pending_commit(&alice).unwrap();
        let mut bob_group = MlsConversation::join_group_from_welcome(
            group_id,
            &bob,
            &[alice_pin, charlie_pin],
            initial.welcome_blob.as_deref().unwrap(),
        )
        .unwrap();
        let mut charlie_group = MlsConversation::join_group_from_welcome(
            group_id,
            &charlie,
            &[alice_pin, bob_pin],
            initial.welcome_blob.as_deref().unwrap(),
        )
        .unwrap();

        let dave_package = generate_key_package_batch(&dave, 1).unwrap().remove(0).blob;
        let rejected_add = alice_group
            .create_add_participant(&alice, dave_pin, &dave_package)
            .unwrap();
        let accepted_update = bob_group.create_self_update(&bob).unwrap();
        assert_eq!(rejected_add.epoch, accepted_update.epoch);
        bob_group.accept_pending_commit(&bob).unwrap();
        charlie_group
            .process_incoming_commit(&charlie, &encrypted_commit(&accepted_update))
            .unwrap();

        let PendingCommitRebase::Rebased(rebased_add) = alice_group
            .rebase_pending_commit(&alice, &encrypted_commit(&accepted_update))
            .unwrap()
        else {
            panic!("group participant Add must be restaged on the accepted epoch");
        };
        assert_eq!(rebased_add.prior_epoch, accepted_update.epoch);
        assert_eq!(rebased_add.epoch, accepted_update.epoch + 1);
        assert_ne!(rejected_add.commit_blob, rebased_add.commit_blob);
        assert_ne!(rejected_add.welcome_blob, rebased_add.welcome_blob);
        alice_group.accept_pending_commit(&alice).unwrap();
        bob_group
            .process_incoming_participant_add(&bob, &encrypted_commit(&rebased_add), dave_pin)
            .unwrap();
        charlie_group
            .process_incoming_participant_add(&charlie, &encrypted_commit(&rebased_add), dave_pin)
            .unwrap();
        let mut dave_group = MlsConversation::join_group_from_welcome(
            group_id,
            &dave,
            &[alice_pin, bob_pin, charlie_pin],
            rebased_add.welcome_blob.as_deref().unwrap(),
        )
        .unwrap();

        let encrypted = dave_group
            .encrypt_application_message(&dave, b"joined from rebased group Welcome")
            .unwrap();
        for (device, conversation) in [
            (&alice, &mut alice_group),
            (&bob, &mut bob_group),
            (&charlie, &mut charlie_group),
        ] {
            assert_eq!(
                conversation
                    .decrypt_application_message(device, &encrypted)
                    .unwrap()
                    .ready_messages[0]
                    .plaintext,
                b"joined from rebased group Welcome"
            );
        }
    }

    struct ThreeDeviceFixture {
        alice: MlsDevice,
        alice_group: MlsConversation,
        bob: MlsDevice,
        bob_group: MlsConversation,
        bob_second: MlsDevice,
        bob_second_group: MlsConversation,
    }

    fn three_device_fixture() -> ThreeDeviceFixture {
        let alice_root = RootIdentityKey::generate();
        let bob_root = RootIdentityKey::generate();
        let alice = MlsDevice::generate(UserId::new(), DeviceId::new(), &alice_root).unwrap();
        let bob = MlsDevice::generate(UserId::new(), DeviceId::new(), &bob_root).unwrap();
        let bob_second = MlsDevice::generate(bob.user_id(), DeviceId::new(), &bob_root).unwrap();
        let alice_pin = PinnedUserIdentity::new(alice.user_id(), *alice.root_key_public());
        let bob_pin = PinnedUserIdentity::new(bob.user_id(), *bob.root_key_public());
        let bob_package = generate_key_package_batch(&bob, 1).unwrap().remove(0).blob;
        let group_id = FilamentGroupId::new();
        let (mut alice_group, initial) =
            MlsConversation::create_two_member(group_id, &alice, bob_pin, &bob_package).unwrap();
        alice_group.accept_pending_commit(&alice).unwrap();
        let mut bob_group = MlsConversation::join_from_welcome(
            group_id,
            &bob,
            alice_pin,
            initial.welcome_blob.as_deref().unwrap(),
        )
        .unwrap();
        let second_package = generate_key_package_batch(&bob_second, 1)
            .unwrap()
            .remove(0)
            .blob;
        let add = alice_group
            .create_add_device(&alice, bob_pin, &second_package)
            .unwrap();
        alice_group.accept_pending_commit(&alice).unwrap();
        bob_group
            .process_incoming_commit(&bob, &encrypted_commit(&add))
            .unwrap();
        let bob_second_group = MlsConversation::join_from_welcome(
            group_id,
            &bob_second,
            alice_pin,
            add.welcome_blob.as_deref().unwrap(),
        )
        .unwrap();
        ThreeDeviceFixture {
            alice,
            alice_group,
            bob,
            bob_group,
            bob_second,
            bob_second_group,
        }
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
    fn concurrent_self_update_rebases_on_authenticated_winner() {
        let (alice, mut alice_group, bob, mut bob_group) = joined_conversations();
        let alice_rejected = alice_group.create_self_update(&alice).unwrap();
        let bob_winner = bob_group.create_self_update(&bob).unwrap();
        assert_eq!(alice_rejected.epoch, bob_winner.epoch);
        bob_group.accept_pending_commit(&bob).unwrap();

        let PendingCommitRebase::Rebased(alice_rebased) = alice_group
            .rebase_pending_commit(&alice, &encrypted_commit(&bob_winner))
            .unwrap()
        else {
            panic!("self-update intent must be restaged");
        };
        assert_eq!(alice_group.epoch(), 2);
        assert_eq!(alice_rebased.prior_epoch, 2);
        assert_eq!(alice_rebased.epoch, 3);
        assert_ne!(alice_rejected.commit_blob, alice_rebased.commit_blob);

        alice_group.accept_pending_commit(&alice).unwrap();
        bob_group
            .process_incoming_commit(&bob, &encrypted_commit(&alice_rebased))
            .unwrap();
        let encrypted = alice_group
            .encrypt_application_message(&alice, b"after deterministic rebase")
            .unwrap();
        assert_eq!(
            bob_group
                .decrypt_application_message(&bob, &encrypted)
                .unwrap()
                .ready_messages[0]
                .plaintext,
            b"after deterministic rebase"
        );
    }

    #[test]
    fn add_device_intent_rebases_with_a_new_recipient_bound_welcome() {
        let alice_root = RootIdentityKey::generate();
        let bob_root = RootIdentityKey::generate();
        let alice = MlsDevice::generate(UserId::new(), DeviceId::new(), &alice_root).unwrap();
        let bob = MlsDevice::generate(UserId::new(), DeviceId::new(), &bob_root).unwrap();
        let bob_second = MlsDevice::generate(bob.user_id(), DeviceId::new(), &bob_root).unwrap();
        let alice_pin = PinnedUserIdentity::new(alice.user_id(), *alice.root_key_public());
        let bob_pin = PinnedUserIdentity::new(bob.user_id(), *bob.root_key_public());
        let bob_package = generate_key_package_batch(&bob, 1).unwrap().remove(0).blob;
        let group_id = FilamentGroupId::new();
        let (mut alice_group, initial) =
            MlsConversation::create_two_member(group_id, &alice, bob_pin, &bob_package).unwrap();
        alice_group.accept_pending_commit(&alice).unwrap();
        let mut bob_group = MlsConversation::join_from_welcome(
            group_id,
            &bob,
            alice_pin,
            initial.welcome_blob.as_deref().unwrap(),
        )
        .unwrap();
        let second_package = generate_key_package_batch(&bob_second, 1)
            .unwrap()
            .remove(0)
            .blob;
        let rejected_add = alice_group
            .create_add_device(&alice, bob_pin, &second_package)
            .unwrap();
        let bob_winner = bob_group.create_self_update(&bob).unwrap();
        assert_eq!(rejected_add.epoch, bob_winner.epoch);
        bob_group.accept_pending_commit(&bob).unwrap();

        let PendingCommitRebase::Rebased(rebased_add) = alice_group
            .rebase_pending_commit(&alice, &encrypted_commit(&bob_winner))
            .unwrap()
        else {
            panic!("Add intent must be restaged");
        };
        assert_eq!(rebased_add.prior_epoch, 2);
        assert_eq!(rebased_add.epoch, 3);
        assert!(rebased_add.welcome_blob.is_some());
        assert_ne!(rejected_add.commit_blob, rebased_add.commit_blob);
        assert_ne!(rejected_add.welcome_blob, rebased_add.welcome_blob);

        alice_group.accept_pending_commit(&alice).unwrap();
        bob_group
            .process_incoming_commit(&bob, &encrypted_commit(&rebased_add))
            .unwrap();
        let mut bob_second_group = MlsConversation::join_from_welcome(
            group_id,
            &bob_second,
            alice_pin,
            rebased_add.welcome_blob.as_deref().unwrap(),
        )
        .unwrap();
        let encrypted = alice_group
            .encrypt_application_message(&alice, b"joined only from rebased Welcome")
            .unwrap();
        assert_eq!(
            bob_second_group
                .decrypt_application_message(&bob_second, &encrypted)
                .unwrap()
                .ready_messages[0]
                .plaintext,
            b"joined only from rebased Welcome"
        );
    }

    #[test]
    fn remove_device_intent_rebases_or_is_safely_satisfied() {
        let ThreeDeviceFixture {
            alice,
            mut alice_group,
            bob,
            mut bob_group,
            bob_second,
            mut bob_second_group,
        } = three_device_fixture();
        let _rejected_remove = alice_group
            .create_remove_device(&alice, bob.device_id())
            .unwrap();
        let bob_update = bob_second_group.create_self_update(&bob_second).unwrap();
        bob_second_group.accept_pending_commit(&bob_second).unwrap();

        let PendingCommitRebase::Rebased(rebased_remove) = alice_group
            .rebase_pending_commit(&alice, &encrypted_commit(&bob_update))
            .unwrap()
        else {
            panic!("safe Remove intent must be restaged");
        };
        assert_eq!(rebased_remove.prior_epoch, 3);
        assert_eq!(rebased_remove.epoch, 4);
        alice_group.accept_pending_commit(&alice).unwrap();
        bob_second_group
            .process_incoming_commit(&bob_second, &encrypted_commit(&rebased_remove))
            .unwrap();
        bob_group
            .process_incoming_commit(&bob, &encrypted_commit(&bob_update))
            .unwrap();
        bob_group
            .process_incoming_commit(&bob, &encrypted_commit(&rebased_remove))
            .unwrap();
        assert_eq!(
            bob_group
                .encrypt_application_message(&bob, b"evicted")
                .unwrap_err(),
            ConversationError::NotActive
        );

        // Two current devices racing to remove the same leaf do not create a
        // second removal after the accepted winner has satisfied the intent.
        let ThreeDeviceFixture {
            alice,
            mut alice_group,
            bob,
            bob_group: _,
            bob_second,
            mut bob_second_group,
        } = three_device_fixture();
        let _alice_rejected = alice_group
            .create_remove_device(&alice, bob.device_id())
            .unwrap();
        let bob_winner = bob_second_group
            .create_remove_device(&bob_second, bob.device_id())
            .unwrap();
        bob_second_group.accept_pending_commit(&bob_second).unwrap();
        assert!(matches!(
            alice_group
                .rebase_pending_commit(&alice, &encrypted_commit(&bob_winner))
                .unwrap(),
            PendingCommitRebase::AlreadySatisfied
        ));
        assert_eq!(alice_group.epoch(), 3);
    }

    #[test]
    fn invalid_winner_does_not_discard_pending_commit() {
        let (alice, mut alice_group, bob, mut bob_group) = joined_conversations();
        let _pending = alice_group.create_self_update(&alice).unwrap();
        let invalid = EncryptedGroupCommit {
            group_id: alice_group.group_id(),
            prior_epoch: 1,
            epoch: 2,
            committer_device_id: DeviceId::new(),
            commit_blob: vec![0xFF],
        };
        assert_eq!(
            alice_group
                .rebase_pending_commit(&alice, &invalid)
                .unwrap_err(),
            ConversationError::SerializationFailed
        );

        let winner = bob_group.create_self_update(&bob).unwrap();
        let mut tampered = encrypted_commit(&winner);
        *tampered.commit_blob.last_mut().unwrap() ^= 0x01;
        assert_eq!(
            alice_group
                .rebase_pending_commit(&alice, &tampered)
                .unwrap_err(),
            ConversationError::CryptoError
        );
        assert_eq!(
            alice_group
                .encrypt_application_message(&alice, b"still blocked")
                .unwrap_err(),
            ConversationError::PendingCommit
        );
        alice_group.reject_pending_commit(&alice).unwrap();
    }

    #[test]
    fn rebase_invalidates_remove_that_would_delete_a_users_final_device() {
        let ThreeDeviceFixture {
            alice,
            mut alice_group,
            bob,
            mut bob_group,
            bob_second,
            bob_second_group: _,
        } = three_device_fixture();
        let _alice_rejected = alice_group
            .create_remove_device(&alice, bob.device_id())
            .unwrap();
        let bob_winner = bob_group
            .create_remove_device(&bob, bob_second.device_id())
            .unwrap();
        bob_group.accept_pending_commit(&bob).unwrap();

        assert!(matches!(
            alice_group
                .rebase_pending_commit(&alice, &encrypted_commit(&bob_winner))
                .unwrap(),
            PendingCommitRebase::Invalidated
        ));
        assert_eq!(alice_group.epoch(), 3);
        let encrypted = bob_group
            .encrypt_application_message(&bob, b"winner remains usable")
            .unwrap();
        assert_eq!(
            alice_group
                .decrypt_application_message(&alice, &encrypted)
                .unwrap()
                .ready_messages[0]
                .plaintext,
            b"winner remains usable"
        );
    }

    #[test]
    fn multi_device_add_message_remove_churn_is_fail_closed() {
        // A second device must chain to the same pinned Bob root. Recreate the
        // fixture explicitly so both Bob devices share that root secret.
        let alice_root = RootIdentityKey::generate();
        let shared_bob_root = RootIdentityKey::generate();
        let alice = MlsDevice::generate(UserId::new(), DeviceId::new(), &alice_root).unwrap();
        let bob = MlsDevice::generate(UserId::new(), DeviceId::new(), &shared_bob_root).unwrap();
        let bob_second =
            MlsDevice::generate(bob.user_id(), DeviceId::new(), &shared_bob_root).unwrap();
        let bob_package = generate_key_package_batch(&bob, 1).unwrap().remove(0).blob;
        let group_id = FilamentGroupId::new();
        let bob_pin = PinnedUserIdentity::new(bob.user_id(), *bob.root_key_public());
        let alice_pin = PinnedUserIdentity::new(alice.user_id(), *alice.root_key_public());
        let (mut alice_group, initial) =
            MlsConversation::create_two_member(group_id, &alice, bob_pin, &bob_package).unwrap();
        alice_group.accept_pending_commit(&alice).unwrap();
        let mut bob_group = MlsConversation::join_from_welcome(
            group_id,
            &bob,
            alice_pin,
            initial.welcome_blob.as_deref().unwrap(),
        )
        .unwrap();

        let second_package = generate_key_package_batch(&bob_second, 1)
            .unwrap()
            .remove(0)
            .blob;
        let add = alice_group
            .create_add_device(&alice, bob_pin, &second_package)
            .unwrap();
        assert_eq!(add.epoch, 2);
        assert!(add.welcome_blob.is_some());
        alice_group.accept_pending_commit(&alice).unwrap();
        bob_group
            .process_incoming_commit(&bob, &encrypted_commit(&add))
            .unwrap();
        let mut bob_second_group = MlsConversation::join_from_welcome(
            group_id,
            &bob_second,
            alice_pin,
            add.welcome_blob.as_deref().unwrap(),
        )
        .unwrap();

        let to_all_devices = alice_group
            .encrypt_application_message(&alice, b"fanout to both Bob devices")
            .unwrap();
        assert_eq!(
            bob_group
                .decrypt_application_message(&bob, &to_all_devices)
                .unwrap()
                .ready_messages[0]
                .plaintext,
            b"fanout to both Bob devices"
        );
        assert_eq!(
            bob_second_group
                .decrypt_application_message(&bob_second, &to_all_devices)
                .unwrap()
                .ready_messages[0]
                .plaintext,
            b"fanout to both Bob devices"
        );

        let remove = alice_group
            .create_remove_device(&alice, bob.device_id())
            .unwrap();
        assert!(remove.welcome_blob.is_none());
        alice_group.accept_pending_commit(&alice).unwrap();
        bob_second_group
            .process_incoming_commit(&bob_second, &encrypted_commit(&remove))
            .unwrap();
        bob_group
            .process_incoming_commit(&bob, &encrypted_commit(&remove))
            .unwrap();
        assert_eq!(
            bob_group
                .encrypt_application_message(&bob, b"removed device must fail")
                .unwrap_err(),
            ConversationError::NotActive
        );
        let after_removal = alice_group
            .encrypt_application_message(&alice, b"only active Bob device")
            .unwrap();
        assert_eq!(
            bob_second_group
                .decrypt_application_message(&bob_second, &after_removal)
                .unwrap()
                .ready_messages[0]
                .plaintext,
            b"only active Bob device"
        );
        assert_eq!(
            bob_group
                .decrypt_application_message(&bob, &after_removal)
                .unwrap_err(),
            ConversationError::NotActive
        );
    }

    #[test]
    fn last_user_device_and_unpinned_add_are_rejected() {
        let (alice, mut alice_group, bob, _bob_group) = joined_conversations();
        assert_eq!(
            alice_group
                .create_remove_device(&alice, bob.device_id())
                .unwrap_err(),
            ConversationError::UnexpectedMembership
        );

        let duplicate_package = generate_key_package_batch(&bob, 1).unwrap().remove(0).blob;
        assert_eq!(
            alice_group
                .create_add_device(
                    &alice,
                    PinnedUserIdentity::new(bob.user_id(), *bob.root_key_public()),
                    &duplicate_package,
                )
                .unwrap_err(),
            ConversationError::UnexpectedMembership
        );

        let attacker_root = RootIdentityKey::generate();
        let attacker = MlsDevice::generate(UserId::new(), DeviceId::new(), &attacker_root).unwrap();
        let attacker_package = generate_key_package_batch(&attacker, 1)
            .unwrap()
            .remove(0)
            .blob;
        assert_eq!(
            alice_group
                .create_add_device(
                    &alice,
                    PinnedUserIdentity::new(attacker.user_id(), *attacker.root_key_public()),
                    &attacker_package,
                )
                .unwrap_err(),
            ConversationError::UntrustedCredential
        );
    }

    #[test]
    fn combined_membership_commit_is_rejected_before_merge() {
        let alice_root = RootIdentityKey::generate();
        let bob_root = RootIdentityKey::generate();
        let alice = MlsDevice::generate(UserId::new(), DeviceId::new(), &alice_root).unwrap();
        let bob = MlsDevice::generate(UserId::new(), DeviceId::new(), &bob_root).unwrap();
        let bob_second = MlsDevice::generate(bob.user_id(), DeviceId::new(), &bob_root).unwrap();
        let bob_third = MlsDevice::generate(bob.user_id(), DeviceId::new(), &bob_root).unwrap();
        let bob_package = generate_key_package_batch(&bob, 1).unwrap().remove(0).blob;
        let group_id = FilamentGroupId::new();
        let alice_pin = PinnedUserIdentity::new(alice.user_id(), *alice.root_key_public());
        let bob_pin = PinnedUserIdentity::new(bob.user_id(), *bob.root_key_public());
        let (mut alice_group, initial) =
            MlsConversation::create_two_member(group_id, &alice, bob_pin, &bob_package).unwrap();
        alice_group.accept_pending_commit(&alice).unwrap();
        let mut bob_group = MlsConversation::join_from_welcome(
            group_id,
            &bob,
            alice_pin,
            initial.welcome_blob.as_deref().unwrap(),
        )
        .unwrap();
        let second = generate_key_package_batch(&bob_second, 1)
            .unwrap()
            .remove(0);
        let third = generate_key_package_batch(&bob_third, 1).unwrap().remove(0);
        let (commit, _, _) = alice_group
            .group
            .add_members(
                alice.provider(),
                alice.signer(),
                &[second.key_package().clone(), third.key_package().clone()],
            )
            .unwrap();
        let combined = EncryptedGroupCommit {
            group_id,
            prior_epoch: 1,
            epoch: 2,
            committer_device_id: alice.device_id(),
            commit_blob: commit.to_bytes().unwrap(),
        };

        assert_eq!(
            bob_group
                .process_incoming_commit(&bob, &combined)
                .unwrap_err(),
            ConversationError::UnexpectedMembership
        );
        assert_eq!(bob_group.epoch(), 1);
    }

    #[test]
    fn application_padding_matches_delivery_service_transport_buckets() {
        let (alice, mut alice_group, _bob, _bob_group) = joined_conversations();
        let short = alice_group
            .encrypt_application_message(&alice, b"a")
            .unwrap();
        let same_bucket = alice_group
            .encrypt_application_message(&alice, &[0x42; 128])
            .unwrap();
        let maximum = alice_group
            .encrypt_application_message(&alice, &[0x43; MAX_APPLICATION_PLAINTEXT_BYTES])
            .unwrap();

        assert_eq!(short.message_blob.len(), 512);
        assert_eq!(same_bucket.message_blob.len(), 512);
        assert_eq!(maximum.message_blob.len(), 16_384);
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
