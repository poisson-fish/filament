//! Authenticated device-to-device transfer of encrypted local message history.
//!
//! History synchronization is deliberately separate from QR root-key pairing.
//! A newly certified device creates a fresh, short-lived HPKE receiver offer
//! and signs it with its root-certified device key. One existing device takes a
//! bounded snapshot of its already-authenticated local history, encrypts it in
//! ordered pages, and signs every page. The transport only sees ciphertext.

use std::collections::HashSet;

use base64::{engine::general_purpose::STANDARD, Engine as _};
use filament_core::{DeviceCertificate, DeviceId, GroupId, UserId};
use openmls::prelude::{
    HpkeAeadType, HpkeCiphertext, HpkeConfig, HpkeKdfType, HpkeKemType, OpenMlsCrypto as _,
    OpenMlsProvider as _, OpenMlsRand as _, SignatureScheme,
};
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

use crate::{
    durable_mailbox::{history_key, history_storage_entry},
    load_stored_message, verify_device_certificate, DecryptedApplicationMessage, HistorySyncError,
    LocalKeyStore, MlsDevice, StoredMailboxMessage, MAX_APPLICATION_PLAINTEXT_BYTES,
    MAX_STORE_BATCH_ENTRIES,
};

/// Default lifetime of a receiving history-sync offer.
pub const DEFAULT_HISTORY_SYNC_TTL_SECS: i64 = 5 * 60;
/// Hard maximum lifetime of a receiving history-sync offer.
pub const MAX_HISTORY_SYNC_TTL_SECS: i64 = 5 * 60;
/// Maximum encoded offer size.
pub const MAX_HISTORY_SYNC_OFFER_BYTES: usize = 4 * 1_024;
/// Maximum encoded encrypted page size.
pub const MAX_HISTORY_SYNC_PAGE_BYTES: usize = 1_024 * 1_024;

const HISTORY_SYNC_VERSION: u16 = 1;
const SYNC_ID_BYTES: usize = 32;
const HPKE_PUBLIC_KEY_BYTES: usize = 32;
const HPKE_PRIVATE_KEY_BYTES: usize = 32;
const HPKE_KEM_OUTPUT_BYTES: usize = 32;
const ED25519_PUBLIC_KEY_BYTES: usize = 32;
const ED25519_SIGNATURE_BYTES: usize = 64;
const MAX_HISTORY_SYNC_RECORDS_PER_PAGE: usize = 64;
const MAX_HISTORY_SYNC_RECORDS: usize = 4_096;
const MAX_HISTORY_SYNC_PAGES: u16 = 4_096;
const MAX_HISTORY_SYNC_PLAINTEXT_BYTES_PER_PAGE: usize = 512 * 1_024;
const MAX_HISTORY_SYNC_INNER_BYTES: usize = 768 * 1_024;
const MAX_HISTORY_SYNC_CIPHERTEXT_BYTES: usize = MAX_HISTORY_SYNC_INNER_BYTES + 16;
const MAX_UNIX_TIMESTAMP: i64 = 253_402_300_799;
const MAX_CLOCK_SKEW_SECS: i64 = 30;
const OFFER_SIGNATURE_DOMAIN: &[u8] = b"filament:e2ee:history_sync_offer:v1";
const PAGE_SIGNATURE_DOMAIN: &[u8] = b"filament:e2ee:history_sync_page:v1";
const HPKE_INFO: &[u8] = b"filament:e2ee:history_sync_hpke:v1";

fn hpke_config() -> HpkeConfig {
    HpkeConfig(
        HpkeKemType::DhKem25519,
        HpkeKdfType::HkdfSha256,
        HpkeAeadType::ChaCha20Poly1305,
    )
}

#[derive(Clone)]
struct HistorySyncContext {
    user_id: UserId,
    receiver_device_id: DeviceId,
    receiver_certificate: DeviceCertificate,
    root_key_pub: [u8; 32],
    sync_id: [u8; SYNC_ID_BYTES],
    receiver_public_key: [u8; HPKE_PUBLIC_KEY_BYTES],
    created_at_unix: i64,
    expires_at_unix: i64,
    receiver_signature: [u8; ED25519_SIGNATURE_BYTES],
}

/// Single-use native receiver state for an ordered history snapshot.
pub struct HistorySyncReceiver {
    context: HistorySyncContext,
    receiver_private_key: Zeroizing<Vec<u8>>,
    expected_sequence: u16,
    sender_device_id: Option<DeviceId>,
    imported_keys: HashSet<String>,
    complete: bool,
}

impl core::fmt::Debug for HistorySyncReceiver {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("HistorySyncReceiver")
            .field("receiver_device_id", &self.context.receiver_device_id)
            .field("expected_sequence", &self.expected_sequence)
            .field("complete", &self.complete)
            .field("key_material", &"<redacted>")
            .finish_non_exhaustive()
    }
}

impl HistorySyncReceiver {
    /// Start a fresh short-lived receiving session on a certified new device.
    ///
    /// # Errors
    /// Rejects invalid lifetimes and provider failures without returning
    /// partial receiver key material.
    pub fn begin(
        receiver: &MlsDevice,
        now_unix: i64,
        ttl_secs: i64,
    ) -> Result<Self, HistorySyncError> {
        if now_unix < 1 || !(1..=MAX_HISTORY_SYNC_TTL_SECS).contains(&ttl_secs) {
            return Err(HistorySyncError::InvalidPayload);
        }
        let expires_at_unix = now_unix
            .checked_add(ttl_secs)
            .ok_or(HistorySyncError::InvalidPayload)?;
        let sync_id = receiver
            .provider()
            .rand()
            .random_array::<SYNC_ID_BYTES>()
            .map_err(|_| HistorySyncError::CryptoError)?;
        let key_ikm = Zeroizing::new(
            receiver
                .provider()
                .rand()
                .random_vec(HPKE_PRIVATE_KEY_BYTES)
                .map_err(|_| HistorySyncError::CryptoError)?,
        );
        let key_pair = receiver
            .provider()
            .crypto()
            .derive_hpke_keypair(hpke_config(), &key_ikm)
            .map_err(|_| HistorySyncError::CryptoError)?;
        let receiver_public_key = key_pair
            .public
            .as_slice()
            .try_into()
            .map_err(|_| HistorySyncError::CryptoError)?;
        let mut context = HistorySyncContext {
            user_id: receiver.user_id(),
            receiver_device_id: receiver.device_id(),
            receiver_certificate: receiver.certificate().clone(),
            root_key_pub: *receiver.root_key_public(),
            sync_id,
            receiver_public_key,
            created_at_unix: now_unix,
            expires_at_unix,
            receiver_signature: [0; ED25519_SIGNATURE_BYTES],
        };
        let signature = receiver
            .sign_history_sync(&offer_signature_payload(&context))
            .map_err(|_| HistorySyncError::CryptoError)?;
        context.receiver_signature = signature
            .try_into()
            .map_err(|_| HistorySyncError::CryptoError)?;
        Ok(Self {
            context,
            receiver_private_key: Zeroizing::new(key_pair.private.to_vec()),
            expected_sequence: 0,
            sender_device_id: None,
            imported_keys: HashSet::new(),
            complete: false,
        })
    }

    /// Encode the public, signed receiving offer for a device-to-device transport.
    ///
    /// # Errors
    /// Returns a serialization error if the strict offer exceeds its hard cap.
    pub fn offer_payload(&self) -> Result<Vec<u8>, HistorySyncError> {
        let wire = HistorySyncOfferWireRef {
            v: HISTORY_SYNC_VERSION,
            user_id: self.context.user_id.to_string(),
            receiver_device_id: self.context.receiver_device_id.to_string(),
            receiver_certificate: &self.context.receiver_certificate,
            root_key_pub: &self.context.root_key_pub,
            sync_id: &self.context.sync_id,
            receiver_public_key: &self.context.receiver_public_key,
            created_at_unix: self.context.created_at_unix,
            expires_at_unix: self.context.expires_at_unix,
            receiver_signature: &self.context.receiver_signature,
        };
        let encoded =
            serde_json::to_vec(&wire).map_err(|_| HistorySyncError::SerializationFailed)?;
        if encoded.is_empty() || encoded.len() > MAX_HISTORY_SYNC_OFFER_BYTES {
            return Err(HistorySyncError::SerializationFailed);
        }
        Ok(encoded)
    }

    /// Authenticate, decrypt, validate, and atomically import the next page.
    ///
    /// Sequence state advances only after durable storage succeeds. Exact local
    /// duplicates are idempotent; conflicting records fail closed.
    ///
    /// # Errors
    /// Rejects expiry, replay/reordering, wrong devices, forged pages, corrupt
    /// records, local conflicts, and persistence failures.
    pub fn import_page(
        &mut self,
        receiver: &MlsDevice,
        store: &dyn LocalKeyStore,
        page: &EncryptedHistorySyncPage,
        now_unix: i64,
    ) -> Result<HistorySyncImport, HistorySyncError> {
        validate_window(&self.context, now_unix)?;
        if self.complete
            || page.sequence != self.expected_sequence
            || page.sequence >= MAX_HISTORY_SYNC_PAGES
        {
            return Err(HistorySyncError::OutOfOrder);
        }
        if receiver.user_id() != self.context.user_id
            || receiver.root_key_public() != &self.context.root_key_pub
            || receiver.device_id() != self.context.receiver_device_id
            || receiver.certificate() != &self.context.receiver_certificate
        {
            return Err(HistorySyncError::DeviceMismatch);
        }
        if page.sync_id != self.context.sync_id
            || page.receiver_device_id != self.context.receiver_device_id
        {
            return Err(HistorySyncError::AuthenticationFailed);
        }
        let (sender_user_id, sender_device_id, sender_signature_key) =
            certificate_fields(&page.sender_certificate)?;
        if sender_user_id != self.context.user_id {
            return Err(HistorySyncError::UserMismatch);
        }
        if sender_device_id == self.context.receiver_device_id {
            return Err(HistorySyncError::DeviceMismatch);
        }
        if self
            .sender_device_id
            .is_some_and(|expected| expected != sender_device_id)
        {
            return Err(HistorySyncError::AuthenticationFailed);
        }
        verify_device_certificate(
            &self.context.root_key_pub,
            sender_user_id,
            sender_device_id,
            &sender_signature_key,
            page.sender_certificate
                .root_key_signature
                .as_slice()
                .try_into()
                .map_err(|_| HistorySyncError::InvalidPayload)?,
        )
        .map_err(|_| HistorySyncError::AuthenticationFailed)?;

        let header = page_header_payload(&self.context, page);
        let mut signed =
            Vec::with_capacity(header.len() + page.kem_output.len() + page.ciphertext.len());
        signed.extend_from_slice(&header);
        signed.extend_from_slice(&page.kem_output);
        signed.extend_from_slice(&page.ciphertext);
        receiver
            .provider()
            .crypto()
            .verify_signature(
                SignatureScheme::ED25519,
                &signed,
                &sender_signature_key,
                &page.sender_signature,
            )
            .map_err(|_| HistorySyncError::AuthenticationFailed)?;

        let hpke_ciphertext = HpkeCiphertext {
            kem_output: page.kem_output.to_vec().into(),
            ciphertext: page.ciphertext.clone().into(),
        };
        let cleartext = Zeroizing::new(
            receiver
                .provider()
                .crypto()
                .hpke_open(
                    hpke_config(),
                    &hpke_ciphertext,
                    self.receiver_private_key.as_slice(),
                    HPKE_INFO,
                    &header,
                )
                .map_err(|_| HistorySyncError::AuthenticationFailed)?,
        );
        if cleartext.is_empty() || cleartext.len() > MAX_HISTORY_SYNC_INNER_BYTES {
            return Err(HistorySyncError::InvalidPayload);
        }
        let mut plaintext: HistorySyncPagePlaintext =
            serde_json::from_slice(&cleartext).map_err(|_| HistorySyncError::InvalidPayload)?;
        let imported = self.import_plaintext(store, &plaintext, page.final_page, sender_device_id);
        plaintext.zeroize();
        imported
    }

    fn import_plaintext(
        &mut self,
        store: &dyn LocalKeyStore,
        plaintext: &HistorySyncPagePlaintext,
        final_page: bool,
        sender_device_id: DeviceId,
    ) -> Result<HistorySyncImport, HistorySyncError> {
        if plaintext.v != HISTORY_SYNC_VERSION
            || plaintext.records.len() > MAX_HISTORY_SYNC_RECORDS_PER_PAGE
            || (plaintext.records.is_empty() && !final_page)
        {
            return Err(HistorySyncError::InvalidPayload);
        }
        let mut page_keys = HashSet::with_capacity(plaintext.records.len());
        let mut entries = SensitiveStoreEntries::with_capacity(plaintext.records.len());
        let mut plaintext_bytes = 0_usize;
        for record in &plaintext.records {
            let mut message = record.to_stored_message()?;
            plaintext_bytes = plaintext_bytes
                .checked_add(message.message.plaintext.len())
                .ok_or(HistorySyncError::LimitExceeded)?;
            if plaintext_bytes > MAX_HISTORY_SYNC_PLAINTEXT_BYTES_PER_PAGE {
                return Err(HistorySyncError::LimitExceeded);
            }
            let key = history_key(message.group_id, &message.message_id)?;
            let key_text = key.as_str().to_owned();
            if !page_keys.insert(key_text.clone()) || self.imported_keys.contains(&key_text) {
                return Err(HistorySyncError::OutOfOrder);
            }
            let entry = history_storage_entry(&message);
            message.message.plaintext.zeroize();
            entries.push(entry?);
        }
        if entries.len() > MAX_STORE_BATCH_ENTRIES {
            return Err(HistorySyncError::LimitExceeded);
        }
        let imported_records = if entries.is_empty() {
            0
        } else {
            store
                .store_batch_if_absent_or_equal(entries.take())
                .map_err(|error| {
                    if error == crate::KeyStoreError::InvalidValue {
                        HistorySyncError::Conflict
                    } else {
                        error.into()
                    }
                })?
        };
        self.imported_keys.extend(page_keys);
        self.sender_device_id = Some(sender_device_id);
        self.expected_sequence = self
            .expected_sequence
            .checked_add(1)
            .ok_or(HistorySyncError::LimitExceeded)?;
        self.complete = final_page;
        Ok(HistorySyncImport {
            imported_records,
            existing_records: plaintext.records.len() - imported_records,
            final_page,
            sender_device_id,
        })
    }
}

/// Verified receiving offer held by the existing source device.
pub struct ScannedHistorySyncOffer {
    context: HistorySyncContext,
}

impl core::fmt::Debug for ScannedHistorySyncOffer {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ScannedHistorySyncOffer")
            .field("receiver_device_id", &self.context.receiver_device_id)
            .field("expires_at_unix", &self.context.expires_at_unix)
            .field("receiver_key", &"<redacted>")
            .finish()
    }
}

impl ScannedHistorySyncOffer {
    /// Strictly parse and authenticate a receiving offer against the source
    /// device's pinned account root.
    ///
    /// # Errors
    /// Rejects malformed, expired, cross-user, self-sync, uncertified, or
    /// incorrectly signed offers.
    pub fn from_payload(
        payload: &[u8],
        source: &MlsDevice,
        now_unix: i64,
    ) -> Result<Self, HistorySyncError> {
        if payload.is_empty() || payload.len() > MAX_HISTORY_SYNC_OFFER_BYTES {
            return Err(HistorySyncError::InvalidPayload);
        }
        let wire: HistorySyncOfferWire =
            serde_json::from_slice(payload).map_err(|_| HistorySyncError::SerializationFailed)?;
        if wire.v != HISTORY_SYNC_VERSION {
            return Err(HistorySyncError::InvalidPayload);
        }
        let context = HistorySyncContext {
            user_id: UserId::try_from(wire.user_id)
                .map_err(|_| HistorySyncError::InvalidPayload)?,
            receiver_device_id: DeviceId::try_from(wire.receiver_device_id)
                .map_err(|_| HistorySyncError::InvalidPayload)?,
            receiver_certificate: wire.receiver_certificate,
            root_key_pub: wire
                .root_key_pub
                .try_into()
                .map_err(|_| HistorySyncError::InvalidPayload)?,
            sync_id: wire
                .sync_id
                .try_into()
                .map_err(|_| HistorySyncError::InvalidPayload)?,
            receiver_public_key: wire
                .receiver_public_key
                .try_into()
                .map_err(|_| HistorySyncError::InvalidPayload)?,
            created_at_unix: wire.created_at_unix,
            expires_at_unix: wire.expires_at_unix,
            receiver_signature: wire
                .receiver_signature
                .try_into()
                .map_err(|_| HistorySyncError::InvalidPayload)?,
        };
        validate_window(&context, now_unix)?;
        if context.user_id != source.user_id() || context.root_key_pub != *source.root_key_public()
        {
            return Err(HistorySyncError::UserMismatch);
        }
        if context.receiver_device_id == source.device_id() {
            return Err(HistorySyncError::DeviceMismatch);
        }
        let (receiver_user_id, receiver_device_id, receiver_signature_key) =
            certificate_fields(&context.receiver_certificate)?;
        if receiver_user_id != context.user_id || receiver_device_id != context.receiver_device_id {
            return Err(HistorySyncError::InvalidPayload);
        }
        let root_signature: [u8; ED25519_SIGNATURE_BYTES] = context
            .receiver_certificate
            .root_key_signature
            .as_slice()
            .try_into()
            .map_err(|_| HistorySyncError::InvalidPayload)?;
        verify_device_certificate(
            &context.root_key_pub,
            receiver_user_id,
            receiver_device_id,
            &receiver_signature_key,
            &root_signature,
        )
        .map_err(|_| HistorySyncError::AuthenticationFailed)?;
        source
            .provider()
            .crypto()
            .verify_signature(
                SignatureScheme::ED25519,
                &offer_signature_payload(&context),
                &receiver_signature_key,
                &context.receiver_signature,
            )
            .map_err(|_| HistorySyncError::AuthenticationFailed)?;
        Ok(Self { context })
    }
}

/// Bounded source-side snapshot and ordered page generator.
pub struct HistorySyncSender {
    context: HistorySyncContext,
    source_device_id: DeviceId,
    source_root_key_pub: [u8; 32],
    snapshot: Vec<(GroupId, String)>,
    offset: usize,
    next_sequence: u16,
    terminal_sent: bool,
}

impl core::fmt::Debug for HistorySyncSender {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("HistorySyncSender")
            .field("source_device_id", &self.source_device_id)
            .field("snapshot_records", &self.snapshot.len())
            .field("next_sequence", &self.next_sequence)
            .field("terminal_sent", &self.terminal_sent)
            .finish_non_exhaustive()
    }
}

impl HistorySyncSender {
    /// Freeze the current canonical local-history key set for one receiving offer.
    ///
    /// # Errors
    /// Rejects corrupt/non-canonical history keys and snapshots beyond the
    /// device-local hard record limit.
    pub fn begin(
        source: &MlsDevice,
        store: &dyn LocalKeyStore,
        offer: ScannedHistorySyncOffer,
    ) -> Result<Self, HistorySyncError> {
        if source.user_id() != offer.context.user_id
            || source.root_key_public() != &offer.context.root_key_pub
            || source.device_id() == offer.context.receiver_device_id
        {
            return Err(HistorySyncError::DeviceMismatch);
        }
        let mut keys = store.list_keys()?;
        keys.sort();
        let mut snapshot = Vec::new();
        for key in keys {
            let Some(remainder) = key.as_str().strip_prefix("history:") else {
                continue;
            };
            let (group_id, message_id) = remainder
                .split_once(':')
                .ok_or(crate::KeyStoreError::InvalidValue)?;
            if message_id.contains(':') {
                return Err(crate::KeyStoreError::InvalidValue.into());
            }
            let group_id = GroupId::try_from(group_id.to_owned())
                .map_err(|_| crate::KeyStoreError::InvalidValue)?;
            let message = load_stored_message(store, group_id, message_id)?;
            snapshot.push((message.group_id, message.message_id));
            if snapshot.len() > MAX_HISTORY_SYNC_RECORDS {
                return Err(HistorySyncError::LimitExceeded);
            }
        }
        Ok(Self {
            context: offer.context,
            source_device_id: source.device_id(),
            source_root_key_pub: *source.root_key_public(),
            snapshot,
            offset: 0,
            next_sequence: 0,
            terminal_sent: false,
        })
    }

    /// Encrypt and sign the next immutable snapshot page.
    ///
    /// # Errors
    /// Rejects expired sessions, source substitution, changed/deleted snapshot
    /// records, hard limits, and provider failures.
    pub fn next_page(
        &mut self,
        source: &MlsDevice,
        store: &dyn LocalKeyStore,
        now_unix: i64,
    ) -> Result<EncryptedHistorySyncPage, HistorySyncError> {
        validate_window(&self.context, now_unix)?;
        if self.terminal_sent || self.next_sequence >= MAX_HISTORY_SYNC_PAGES {
            return Err(HistorySyncError::OutOfOrder);
        }
        if source.device_id() != self.source_device_id
            || source.user_id() != self.context.user_id
            || source.root_key_public() != &self.source_root_key_pub
        {
            return Err(HistorySyncError::DeviceMismatch);
        }
        let mut records = Vec::new();
        let mut plaintext_bytes = 0_usize;
        while self.offset + records.len() < self.snapshot.len()
            && records.len() < MAX_HISTORY_SYNC_RECORDS_PER_PAGE
        {
            let (group_id, message_id) = &self.snapshot[self.offset + records.len()];
            let message = load_stored_message(store, *group_id, message_id)?;
            let next_bytes = plaintext_bytes
                .checked_add(message.message.plaintext.len())
                .ok_or(HistorySyncError::LimitExceeded)?;
            if !records.is_empty() && next_bytes > MAX_HISTORY_SYNC_PLAINTEXT_BYTES_PER_PAGE {
                break;
            }
            if next_bytes > MAX_HISTORY_SYNC_PLAINTEXT_BYTES_PER_PAGE {
                return Err(HistorySyncError::LimitExceeded);
            }
            plaintext_bytes = next_bytes;
            records.push(HistorySyncRecord::from_stored_message(message));
        }
        let new_offset = self.offset + records.len();
        let final_page = new_offset == self.snapshot.len();
        let mut inner = HistorySyncPagePlaintext {
            v: HISTORY_SYNC_VERSION,
            records,
        };
        let cleartext = Zeroizing::new(
            serde_json::to_vec(&inner).map_err(|_| HistorySyncError::SerializationFailed)?,
        );
        if cleartext.is_empty() || cleartext.len() > MAX_HISTORY_SYNC_INNER_BYTES {
            inner.zeroize();
            return Err(HistorySyncError::LimitExceeded);
        }
        inner.zeroize();
        let mut page = EncryptedHistorySyncPage {
            sync_id: self.context.sync_id,
            receiver_device_id: self.context.receiver_device_id,
            sender_certificate: source.certificate().clone(),
            sequence: self.next_sequence,
            final_page,
            kem_output: [0; HPKE_KEM_OUTPUT_BYTES],
            ciphertext: Vec::new(),
            sender_signature: [0; ED25519_SIGNATURE_BYTES],
        };
        let header = page_header_payload(&self.context, &page);
        let encrypted = source
            .provider()
            .crypto()
            .hpke_seal(
                hpke_config(),
                &self.context.receiver_public_key,
                HPKE_INFO,
                &header,
                &cleartext,
            )
            .map_err(|_| HistorySyncError::CryptoError)?;
        page.kem_output = encrypted
            .kem_output
            .as_slice()
            .try_into()
            .map_err(|_| HistorySyncError::CryptoError)?;
        page.ciphertext = encrypted.ciphertext.as_slice().to_vec();
        if page.ciphertext.is_empty() || page.ciphertext.len() > MAX_HISTORY_SYNC_CIPHERTEXT_BYTES {
            return Err(HistorySyncError::LimitExceeded);
        }
        let mut signed =
            Vec::with_capacity(header.len() + page.kem_output.len() + page.ciphertext.len());
        signed.extend_from_slice(&header);
        signed.extend_from_slice(&page.kem_output);
        signed.extend_from_slice(&page.ciphertext);
        page.sender_signature = source
            .sign_history_sync(&signed)
            .map_err(|_| HistorySyncError::CryptoError)?
            .try_into()
            .map_err(|_| HistorySyncError::CryptoError)?;
        if page.to_payload()?.len() > MAX_HISTORY_SYNC_PAGE_BYTES {
            return Err(HistorySyncError::LimitExceeded);
        }
        self.offset = new_offset;
        self.next_sequence = self
            .next_sequence
            .checked_add(1)
            .ok_or(HistorySyncError::LimitExceeded)?;
        self.terminal_sent = final_page;
        Ok(page)
    }
}

/// Opaque encrypted page safe for an untrusted device-to-device transport.
#[derive(Clone, PartialEq, Eq)]
pub struct EncryptedHistorySyncPage {
    sync_id: [u8; SYNC_ID_BYTES],
    receiver_device_id: DeviceId,
    sender_certificate: DeviceCertificate,
    sequence: u16,
    final_page: bool,
    kem_output: [u8; HPKE_KEM_OUTPUT_BYTES],
    ciphertext: Vec<u8>,
    sender_signature: [u8; ED25519_SIGNATURE_BYTES],
}

impl core::fmt::Debug for EncryptedHistorySyncPage {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("EncryptedHistorySyncPage")
            .field("receiver_device_id", &self.receiver_device_id)
            .field("sequence", &self.sequence)
            .field("final_page", &self.final_page)
            .field("ciphertext_bytes", &self.ciphertext.len())
            .finish_non_exhaustive()
    }
}

impl EncryptedHistorySyncPage {
    /// Encode the strict encrypted page wire payload.
    ///
    /// # Errors
    /// Rejects any payload beyond the hard transport cap.
    pub fn to_payload(&self) -> Result<Vec<u8>, HistorySyncError> {
        let wire = HistorySyncPageWireRef {
            v: HISTORY_SYNC_VERSION,
            sync_id: &self.sync_id,
            receiver_device_id: self.receiver_device_id.to_string(),
            sender_certificate: &self.sender_certificate,
            sequence: self.sequence,
            final_page: self.final_page,
            kem_output: &self.kem_output,
            ciphertext: &self.ciphertext,
            sender_signature: &self.sender_signature,
        };
        let encoded =
            serde_json::to_vec(&wire).map_err(|_| HistorySyncError::SerializationFailed)?;
        if encoded.is_empty() || encoded.len() > MAX_HISTORY_SYNC_PAGE_BYTES {
            return Err(HistorySyncError::LimitExceeded);
        }
        Ok(encoded)
    }

    /// Strictly decode an opaque page before authentication and import.
    ///
    /// # Errors
    /// Rejects unknown fields, wrong versions, malformed lengths, and
    /// oversized ciphertext before cryptographic processing.
    pub fn from_payload(payload: &[u8]) -> Result<Self, HistorySyncError> {
        if payload.is_empty() || payload.len() > MAX_HISTORY_SYNC_PAGE_BYTES {
            return Err(HistorySyncError::InvalidPayload);
        }
        let wire: HistorySyncPageWire =
            serde_json::from_slice(payload).map_err(|_| HistorySyncError::SerializationFailed)?;
        if wire.v != HISTORY_SYNC_VERSION
            || wire.ciphertext.is_empty()
            || wire.ciphertext.len() > MAX_HISTORY_SYNC_CIPHERTEXT_BYTES
        {
            return Err(HistorySyncError::InvalidPayload);
        }
        Ok(Self {
            sync_id: wire
                .sync_id
                .try_into()
                .map_err(|_| HistorySyncError::InvalidPayload)?,
            receiver_device_id: DeviceId::try_from(wire.receiver_device_id)
                .map_err(|_| HistorySyncError::InvalidPayload)?,
            sender_certificate: wire.sender_certificate,
            sequence: wire.sequence,
            final_page: wire.final_page,
            kem_output: wire
                .kem_output
                .try_into()
                .map_err(|_| HistorySyncError::InvalidPayload)?,
            ciphertext: wire.ciphertext,
            sender_signature: wire
                .sender_signature
                .try_into()
                .map_err(|_| HistorySyncError::InvalidPayload)?,
        })
    }
}

/// Result of one authenticated and durable page import.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HistorySyncImport {
    /// Records newly inserted into encrypted local history.
    pub imported_records: usize,
    /// Exact pre-existing records accepted idempotently.
    pub existing_records: usize,
    /// Whether this was the terminal snapshot page.
    pub final_page: bool,
    /// Existing certified device that authored the snapshot.
    pub sender_device_id: DeviceId,
}

#[derive(Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
#[serde(deny_unknown_fields)]
struct HistorySyncPagePlaintext {
    v: u16,
    records: Vec<HistorySyncRecord>,
}

#[derive(Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
#[serde(deny_unknown_fields)]
struct HistorySyncRecord {
    message_id: String,
    group_id: String,
    created_at_unix: i64,
    #[serde(default)]
    expires_at_unix: Option<i64>,
    sender_user_id: String,
    sender_device_id: String,
    generation: u64,
    #[serde(with = "base64_bytes")]
    plaintext: Vec<u8>,
}

struct SensitiveStoreEntries {
    entries: Vec<(crate::StoreKey, Vec<u8>)>,
}

impl SensitiveStoreEntries {
    fn with_capacity(capacity: usize) -> Self {
        Self {
            entries: Vec::with_capacity(capacity),
        }
    }

    fn push(&mut self, entry: (crate::StoreKey, Vec<u8>)) {
        self.entries.push(entry);
    }

    fn len(&self) -> usize {
        self.entries.len()
    }

    fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn take(&mut self) -> Vec<(crate::StoreKey, Vec<u8>)> {
        core::mem::take(&mut self.entries)
    }
}

impl Drop for SensitiveStoreEntries {
    fn drop(&mut self) {
        for (_, value) in &mut self.entries {
            value.zeroize();
        }
    }
}

impl HistorySyncRecord {
    fn from_stored_message(message: StoredMailboxMessage) -> Self {
        Self {
            message_id: message.message_id,
            group_id: message.group_id.to_string(),
            created_at_unix: message.created_at_unix,
            expires_at_unix: message.expires_at_unix,
            sender_user_id: message.message.sender_user_id.to_string(),
            sender_device_id: message.message.sender_device_id.to_string(),
            generation: message.message.generation,
            plaintext: message.message.plaintext,
        }
    }

    fn to_stored_message(&self) -> Result<StoredMailboxMessage, HistorySyncError> {
        if self.plaintext.is_empty()
            || self.plaintext.len() > MAX_APPLICATION_PLAINTEXT_BYTES
            || !(0..=MAX_UNIX_TIMESTAMP).contains(&self.created_at_unix)
            || self.expires_at_unix.is_some_and(|expires_at| {
                expires_at <= self.created_at_unix || expires_at > MAX_UNIX_TIMESTAMP
            })
        {
            return Err(HistorySyncError::InvalidPayload);
        }
        Ok(StoredMailboxMessage {
            message_id: canonical_ulid(&self.message_id)?,
            group_id: GroupId::try_from(self.group_id.clone())
                .map_err(|_| HistorySyncError::InvalidPayload)?,
            created_at_unix: self.created_at_unix,
            expires_at_unix: self.expires_at_unix,
            message: DecryptedApplicationMessage {
                sender_user_id: UserId::try_from(self.sender_user_id.clone())
                    .map_err(|_| HistorySyncError::InvalidPayload)?,
                sender_device_id: DeviceId::try_from(self.sender_device_id.clone())
                    .map_err(|_| HistorySyncError::InvalidPayload)?,
                generation: self.generation,
                plaintext: self.plaintext.clone(),
            },
        })
    }
}

fn canonical_ulid(value: &str) -> Result<String, HistorySyncError> {
    let parsed = ulid::Ulid::from_string(value).map_err(|_| HistorySyncError::InvalidPayload)?;
    if parsed.to_string() != value {
        return Err(HistorySyncError::InvalidPayload);
    }
    Ok(value.to_owned())
}

fn validate_window(context: &HistorySyncContext, now_unix: i64) -> Result<(), HistorySyncError> {
    let lifetime = context
        .expires_at_unix
        .checked_sub(context.created_at_unix)
        .ok_or(HistorySyncError::InvalidPayload)?;
    if context.created_at_unix < 1
        || !(1..=MAX_HISTORY_SYNC_TTL_SECS).contains(&lifetime)
        || context.created_at_unix > now_unix.saturating_add(MAX_CLOCK_SKEW_SECS)
        || context.expires_at_unix <= now_unix
    {
        return Err(HistorySyncError::Expired);
    }
    Ok(())
}

fn certificate_fields(
    certificate: &DeviceCertificate,
) -> Result<(UserId, DeviceId, [u8; ED25519_PUBLIC_KEY_BYTES]), HistorySyncError> {
    Ok((
        UserId::try_from(certificate.user_id.clone())
            .map_err(|_| HistorySyncError::InvalidPayload)?,
        DeviceId::try_from(certificate.device_id.clone())
            .map_err(|_| HistorySyncError::InvalidPayload)?,
        certificate
            .device_signature_pubkey
            .as_slice()
            .try_into()
            .map_err(|_| HistorySyncError::InvalidPayload)?,
    ))
}

fn offer_signature_payload(context: &HistorySyncContext) -> Vec<u8> {
    let mut payload = Vec::with_capacity(512);
    payload.extend_from_slice(OFFER_SIGNATURE_DOMAIN);
    payload.extend_from_slice(&HISTORY_SYNC_VERSION.to_be_bytes());
    append_bytes(&mut payload, context.user_id.to_string().as_bytes());
    append_bytes(
        &mut payload,
        context.receiver_device_id.to_string().as_bytes(),
    );
    append_certificate(&mut payload, &context.receiver_certificate);
    payload.extend_from_slice(&context.root_key_pub);
    payload.extend_from_slice(&context.sync_id);
    payload.extend_from_slice(&context.receiver_public_key);
    payload.extend_from_slice(&context.created_at_unix.to_be_bytes());
    payload.extend_from_slice(&context.expires_at_unix.to_be_bytes());
    payload
}

fn page_header_payload(context: &HistorySyncContext, page: &EncryptedHistorySyncPage) -> Vec<u8> {
    let mut payload = Vec::with_capacity(768);
    payload.extend_from_slice(PAGE_SIGNATURE_DOMAIN);
    payload.extend_from_slice(&offer_signature_payload(context));
    payload.extend_from_slice(&context.receiver_signature);
    append_certificate(&mut payload, &page.sender_certificate);
    payload.extend_from_slice(&page.sequence.to_be_bytes());
    payload.push(u8::from(page.final_page));
    payload
}

fn append_certificate(payload: &mut Vec<u8>, certificate: &DeviceCertificate) {
    append_bytes(payload, certificate.user_id.as_bytes());
    append_bytes(payload, certificate.device_id.as_bytes());
    append_bytes(payload, &certificate.device_signature_pubkey);
    append_bytes(payload, &certificate.root_key_signature);
}

fn append_bytes(payload: &mut Vec<u8>, value: &[u8]) {
    let length = u32::try_from(value.len()).unwrap_or(u32::MAX);
    payload.extend_from_slice(&length.to_be_bytes());
    payload.extend_from_slice(value);
}

fn deserialize_exact_base64<'de, D, const N: usize>(deserializer: D) -> Result<Vec<u8>, D::Error>
where
    D: Deserializer<'de>,
{
    let encoded = String::deserialize(deserializer)?;
    let decoded = STANDARD.decode(encoded).map_err(de::Error::custom)?;
    if decoded.len() != N {
        return Err(de::Error::invalid_length(
            decoded.len(),
            &"exact-length bytes",
        ));
    }
    Ok(decoded)
}

fn deserialize_ciphertext<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
where
    D: Deserializer<'de>,
{
    let encoded = String::deserialize(deserializer)?;
    if encoded.len() > MAX_HISTORY_SYNC_PAGE_BYTES {
        return Err(de::Error::custom("history sync ciphertext exceeds limit"));
    }
    let decoded = STANDARD.decode(encoded).map_err(de::Error::custom)?;
    if decoded.is_empty() || decoded.len() > MAX_HISTORY_SYNC_CIPHERTEXT_BYTES {
        return Err(de::Error::custom("invalid history sync ciphertext length"));
    }
    Ok(decoded)
}

fn serialize_base64<S>(value: &[u8], serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(&STANDARD.encode(value))
}

macro_rules! exact_base64_deserializer {
    ($name:ident, $size:expr) => {
        fn $name<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
        where
            D: Deserializer<'de>,
        {
            deserialize_exact_base64::<D, $size>(deserializer)
        }
    };
}

exact_base64_deserializer!(deserialize_root_key, 32);
exact_base64_deserializer!(deserialize_sync_id, SYNC_ID_BYTES);
exact_base64_deserializer!(deserialize_hpke_public_key, HPKE_PUBLIC_KEY_BYTES);
exact_base64_deserializer!(deserialize_kem_output, HPKE_KEM_OUTPUT_BYTES);
exact_base64_deserializer!(deserialize_signature, ED25519_SIGNATURE_BYTES);

#[derive(Serialize)]
struct HistorySyncOfferWireRef<'a> {
    v: u16,
    user_id: String,
    receiver_device_id: String,
    receiver_certificate: &'a DeviceCertificate,
    #[serde(serialize_with = "serialize_base64")]
    root_key_pub: &'a [u8],
    #[serde(serialize_with = "serialize_base64")]
    sync_id: &'a [u8],
    #[serde(serialize_with = "serialize_base64")]
    receiver_public_key: &'a [u8],
    created_at_unix: i64,
    expires_at_unix: i64,
    #[serde(serialize_with = "serialize_base64")]
    receiver_signature: &'a [u8],
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HistorySyncOfferWire {
    v: u16,
    user_id: String,
    receiver_device_id: String,
    receiver_certificate: DeviceCertificate,
    #[serde(deserialize_with = "deserialize_root_key")]
    root_key_pub: Vec<u8>,
    #[serde(deserialize_with = "deserialize_sync_id")]
    sync_id: Vec<u8>,
    #[serde(deserialize_with = "deserialize_hpke_public_key")]
    receiver_public_key: Vec<u8>,
    created_at_unix: i64,
    expires_at_unix: i64,
    #[serde(deserialize_with = "deserialize_signature")]
    receiver_signature: Vec<u8>,
}

#[derive(Serialize)]
struct HistorySyncPageWireRef<'a> {
    v: u16,
    #[serde(serialize_with = "serialize_base64")]
    sync_id: &'a [u8],
    receiver_device_id: String,
    sender_certificate: &'a DeviceCertificate,
    sequence: u16,
    final_page: bool,
    #[serde(serialize_with = "serialize_base64")]
    kem_output: &'a [u8],
    #[serde(serialize_with = "serialize_base64")]
    ciphertext: &'a [u8],
    #[serde(serialize_with = "serialize_base64")]
    sender_signature: &'a [u8],
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HistorySyncPageWire {
    v: u16,
    #[serde(deserialize_with = "deserialize_sync_id")]
    sync_id: Vec<u8>,
    receiver_device_id: String,
    sender_certificate: DeviceCertificate,
    sequence: u16,
    final_page: bool,
    #[serde(deserialize_with = "deserialize_kem_output")]
    kem_output: Vec<u8>,
    #[serde(deserialize_with = "deserialize_ciphertext")]
    ciphertext: Vec<u8>,
    #[serde(deserialize_with = "deserialize_signature")]
    sender_signature: Vec<u8>,
}

mod base64_bytes {
    use super::*;

    pub fn serialize<S>(value: &[u8], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serialize_base64(value, serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let encoded = String::deserialize(deserializer)?;
        if encoded.len() > (MAX_APPLICATION_PLAINTEXT_BYTES * 2) {
            return Err(de::Error::custom("history plaintext exceeds limit"));
        }
        let decoded = STANDARD.decode(encoded).map_err(de::Error::custom)?;
        if decoded.is_empty() || decoded.len() > MAX_APPLICATION_PLAINTEXT_BYTES {
            return Err(de::Error::custom("invalid history plaintext length"));
        }
        Ok(decoded)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        create_pairing_transfer, InMemoryKeyStore, PairingReceiver, RootIdentityKey,
        ScannedPairingOffer, StoreKey,
    };
    use ulid::Ulid;

    const NOW: i64 = 1_750_000_000;

    fn paired_devices() -> (MlsDevice, MlsDevice) {
        let user_id = UserId::new();
        let root = RootIdentityKey::generate();
        let source = MlsDevice::generate(user_id, DeviceId::new(), &root).unwrap();
        let receiver = PairingReceiver::begin(
            user_id,
            DeviceId::new(),
            NOW,
            crate::DEFAULT_PAIRING_TTL_SECS,
        )
        .unwrap();
        let new_device_id = receiver.new_device_id();
        let offer =
            ScannedPairingOffer::from_qr_payload(&receiver.qr_payload().unwrap(), NOW).unwrap();
        let transfer = create_pairing_transfer(&source, &root, &offer, NOW).unwrap();
        let paired = receiver.complete(&transfer, NOW).unwrap();
        let restored_root = paired.into_root_identity();
        let destination = MlsDevice::generate(user_id, new_device_id, &restored_root).unwrap();
        (source, destination)
    }

    fn stored_message(
        group_id: GroupId,
        message_id: String,
        sender_user_id: UserId,
        sender_device_id: DeviceId,
        generation: u64,
        plaintext: &[u8],
    ) -> StoredMailboxMessage {
        StoredMailboxMessage {
            message_id,
            group_id,
            created_at_unix: NOW - 60,
            expires_at_unix: None,
            message: DecryptedApplicationMessage {
                sender_user_id,
                sender_device_id,
                generation,
                plaintext: plaintext.to_vec(),
            },
        }
    }

    fn seed_history(
        store: &dyn LocalKeyStore,
        count: usize,
    ) -> (GroupId, UserId, DeviceId, Vec<(String, Vec<u8>)>) {
        let group_id = GroupId::new();
        let sender_user_id = UserId::new();
        let sender_device_id = DeviceId::new();
        let mut expected = Vec::with_capacity(count);
        for generation in 0..count {
            let message_id = Ulid::new().to_string();
            let plaintext = format!("private history record {generation}").into_bytes();
            let message = stored_message(
                group_id,
                message_id.clone(),
                sender_user_id,
                sender_device_id,
                u64::try_from(generation).unwrap(),
                &plaintext,
            );
            let (key, encoded) = history_storage_entry(&message).unwrap();
            store.store(key, encoded).unwrap();
            expected.push((message_id, plaintext));
        }
        (group_id, sender_user_id, sender_device_id, expected)
    }

    #[test]
    fn paired_device_imports_bounded_encrypted_history_pages() {
        let (source, destination) = paired_devices();
        let source_store = InMemoryKeyStore::new();
        let destination_store = InMemoryKeyStore::new();
        assert!(destination_store.list_keys().unwrap().is_empty());
        let (group_id, _, _, expected) =
            seed_history(&source_store, MAX_HISTORY_SYNC_RECORDS_PER_PAGE + 1);

        let mut receiver =
            HistorySyncReceiver::begin(&destination, NOW, DEFAULT_HISTORY_SYNC_TTL_SECS).unwrap();
        let offer =
            ScannedHistorySyncOffer::from_payload(&receiver.offer_payload().unwrap(), &source, NOW)
                .unwrap();
        let mut sender = HistorySyncSender::begin(&source, &source_store, offer).unwrap();
        let mut page_count = 0;
        let mut imported = 0;
        loop {
            let page = sender.next_page(&source, &source_store, NOW).unwrap();
            let payload = page.to_payload().unwrap();
            assert!(payload.len() <= MAX_HISTORY_SYNC_PAGE_BYTES);
            assert!(!payload
                .windows(b"private history record".len())
                .any(|window| window == b"private history record"));
            let decoded = EncryptedHistorySyncPage::from_payload(&payload).unwrap();
            let result = receiver
                .import_page(&destination, &destination_store, &decoded, NOW)
                .unwrap();
            page_count += 1;
            imported += result.imported_records;
            assert_eq!(result.sender_device_id, source.device_id());
            if result.final_page {
                break;
            }
        }
        assert_eq!(page_count, 2);
        assert_eq!(imported, expected.len());
        assert_eq!(
            sender.next_page(&source, &source_store, NOW),
            Err(HistorySyncError::OutOfOrder)
        );
        for (message_id, plaintext) in expected {
            assert_eq!(
                load_stored_message(&destination_store, group_id, &message_id)
                    .unwrap()
                    .message
                    .plaintext,
                plaintext
            );
        }
    }

    #[test]
    fn forged_replayed_and_reordered_pages_do_not_advance_receiver() {
        let (source, destination) = paired_devices();
        let source_store = InMemoryKeyStore::new();
        let destination_store = InMemoryKeyStore::new();
        seed_history(&source_store, MAX_HISTORY_SYNC_RECORDS_PER_PAGE + 1);
        let mut receiver = HistorySyncReceiver::begin(&destination, NOW, 60).unwrap();
        let offer =
            ScannedHistorySyncOffer::from_payload(&receiver.offer_payload().unwrap(), &source, NOW)
                .unwrap();
        let mut sender = HistorySyncSender::begin(&source, &source_store, offer).unwrap();
        let first = sender.next_page(&source, &source_store, NOW).unwrap();
        let second = sender.next_page(&source, &source_store, NOW).unwrap();

        assert_eq!(
            receiver.import_page(&destination, &destination_store, &second, NOW),
            Err(HistorySyncError::OutOfOrder)
        );
        let mut forged = first.clone();
        forged.ciphertext[0] ^= 1;
        assert_eq!(
            receiver.import_page(&destination, &destination_store, &forged, NOW),
            Err(HistorySyncError::AuthenticationFailed)
        );
        assert!(
            !receiver
                .import_page(&destination, &destination_store, &first, NOW)
                .unwrap()
                .final_page
        );
        assert_eq!(
            receiver.import_page(&destination, &destination_store, &first, NOW),
            Err(HistorySyncError::OutOfOrder)
        );
        assert!(
            receiver
                .import_page(&destination, &destination_store, &second, NOW)
                .unwrap()
                .final_page
        );
    }

    struct RejectWrites<'a> {
        inner: &'a InMemoryKeyStore,
    }

    impl LocalKeyStore for RejectWrites<'_> {
        fn store(&self, _key: StoreKey, _value: Vec<u8>) -> Result<(), crate::KeyStoreError> {
            Err(crate::KeyStoreError::BackendError)
        }

        fn store_batch(
            &self,
            _entries: Vec<(StoreKey, Vec<u8>)>,
        ) -> Result<(), crate::KeyStoreError> {
            Err(crate::KeyStoreError::BackendError)
        }

        fn load(&self, key: &StoreKey) -> Result<Zeroizing<Vec<u8>>, crate::KeyStoreError> {
            self.inner.load(key)
        }

        fn remove(&self, key: &StoreKey) -> Result<(), crate::KeyStoreError> {
            self.inner.remove(key)
        }

        fn exists(&self, key: &StoreKey) -> Result<bool, crate::KeyStoreError> {
            self.inner.exists(key)
        }

        fn list_keys(&self) -> Result<Vec<StoreKey>, crate::KeyStoreError> {
            self.inner.list_keys()
        }
    }

    #[test]
    fn storage_failure_and_conflict_leave_page_retryable() {
        let (source, destination) = paired_devices();
        let source_store = InMemoryKeyStore::new();
        let destination_store = InMemoryKeyStore::new();
        let (group_id, sender_user_id, sender_device_id, expected) = seed_history(&source_store, 1);
        let mut receiver = HistorySyncReceiver::begin(&destination, NOW, 60).unwrap();
        let offer =
            ScannedHistorySyncOffer::from_payload(&receiver.offer_payload().unwrap(), &source, NOW)
                .unwrap();
        let mut sender = HistorySyncSender::begin(&source, &source_store, offer).unwrap();
        let page = sender.next_page(&source, &source_store, NOW).unwrap();

        assert_eq!(
            receiver.import_page(
                &destination,
                &RejectWrites {
                    inner: &destination_store,
                },
                &page,
                NOW,
            ),
            Err(HistorySyncError::KeyStore(
                crate::KeyStoreError::BackendError
            ))
        );

        let (message_id, _) = &expected[0];
        let conflict = stored_message(
            group_id,
            message_id.clone(),
            sender_user_id,
            sender_device_id,
            0,
            b"conflicting local plaintext",
        );
        let (key, encoded) = history_storage_entry(&conflict).unwrap();
        destination_store.store(key.clone(), encoded).unwrap();
        assert_eq!(
            receiver.import_page(&destination, &destination_store, &page, NOW),
            Err(HistorySyncError::Conflict)
        );
        destination_store.remove(&key).unwrap();
        assert!(
            receiver
                .import_page(&destination, &destination_store, &page, NOW)
                .unwrap()
                .final_page
        );
    }

    #[test]
    fn offers_are_root_bound_expiring_and_strict() {
        let (source, destination) = paired_devices();
        let receiver = HistorySyncReceiver::begin(&destination, NOW, 60).unwrap();
        let payload = receiver.offer_payload().unwrap();
        assert_eq!(
            ScannedHistorySyncOffer::from_payload(&payload, &source, NOW + 60).unwrap_err(),
            HistorySyncError::Expired
        );

        let other_root = RootIdentityKey::generate();
        let other_source =
            MlsDevice::generate(UserId::new(), DeviceId::new(), &other_root).unwrap();
        assert_eq!(
            ScannedHistorySyncOffer::from_payload(&payload, &other_source, NOW).unwrap_err(),
            HistorySyncError::UserMismatch
        );

        let mut value: serde_json::Value = serde_json::from_slice(&payload).unwrap();
        value["unexpected"] = serde_json::Value::Bool(true);
        assert_eq!(
            ScannedHistorySyncOffer::from_payload(
                &serde_json::to_vec(&value).unwrap(),
                &source,
                NOW,
            )
            .unwrap_err(),
            HistorySyncError::SerializationFailed
        );
    }

    #[test]
    fn debug_output_redacts_sync_secrets_and_plaintext() {
        let (source, destination) = paired_devices();
        let source_store = InMemoryKeyStore::new();
        seed_history(&source_store, 1);
        let receiver = HistorySyncReceiver::begin(&destination, NOW, 60).unwrap();
        let offer =
            ScannedHistorySyncOffer::from_payload(&receiver.offer_payload().unwrap(), &source, NOW)
                .unwrap();
        let mut sender = HistorySyncSender::begin(&source, &source_store, offer).unwrap();
        let page = sender.next_page(&source, &source_store, NOW).unwrap();
        for debug in [
            format!("{receiver:?}"),
            format!("{sender:?}"),
            format!("{page:?}"),
        ] {
            assert!(!debug.contains("private history"));
            assert!(!debug.contains(&STANDARD.encode(receiver.receiver_private_key.as_slice())));
        }
    }
}
