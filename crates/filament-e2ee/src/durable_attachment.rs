//! Durable native storage and acknowledgment for authenticated attachments.
//!
//! Attachment descriptors are discovered only in MLS-authenticated local
//! history. Opaque server blobs are authenticated and decrypted before their
//! plaintext is chunked into the encrypted local store. The content and a
//! per-group acknowledgment outbox are committed atomically, so a lost server
//! response cannot discard the only durable copy.

use std::collections::HashSet;

use filament_core::{DeviceId, GroupId};
use filament_protocol::{
    AckE2eeAttachmentsRequest, PutE2eeAttachmentResponse, E2EE_ATTACHMENT_CIPHERTEXT_BUCKETS,
    MAX_E2EE_ATTACHMENT_ACK_BATCH_SIZE,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use thiserror::Error;
use ulid::Ulid;
use zeroize::{Zeroize as _, Zeroizing};

use crate::{
    decrypt_attachment, durable_mailbox::parse_history_key, AttachmentContent,
    AttachmentDescriptor, AttachmentError, AttachmentId, EncryptedAttachment, EncryptedChatEvent,
    KeyStoreError, LocalKeyStore, StoreKey, VersionedApplicationEvent, MAX_ATTACHMENT_BYTES,
    MAX_ATTACHMENT_FILENAME_BYTES, MAX_ATTACHMENT_MIME_BYTES, MAX_ENCRYPTED_ATTACHMENT_BYTES,
    MAX_STORE_BATCH_ENTRIES, MAX_STORE_VALUE_BYTES,
};

const ATTACHMENT_MANIFEST_VERSION: u16 = 1;
const ATTACHMENT_ACK_VERSION: u16 = 1;
const ATTACHMENT_UPLOAD_VERSION: u16 = 1;
const MAX_UNIX_TIMESTAMP: i64 = 253_402_300_799;
/// Maximum descriptors returned in one bounded native download scan.
pub const MAX_PENDING_ATTACHMENT_DOWNLOADS: usize = 32;
const ATTACHMENT_CHUNK_BYTES: usize = 1024 * 1024;
const MAX_ATTACHMENT_CHUNKS: usize = MAX_ATTACHMENT_BYTES.div_ceil(ATTACHMENT_CHUNK_BYTES);
const MAX_ATTACHMENT_UPLOAD_CHUNKS: usize =
    MAX_ENCRYPTED_ATTACHMENT_BYTES.div_ceil(ATTACHMENT_CHUNK_BYTES);

/// Fail-closed errors at the authenticated attachment persistence boundary.
#[derive(Debug, Error)]
pub enum DurableAttachmentError {
    /// A prior verified attachment acknowledgment must be resolved first.
    #[error("an attachment acknowledgment is already pending")]
    PendingAcknowledgment,
    /// One exact encrypted upload must be confirmed before preparing another.
    #[error("an attachment upload is already pending")]
    PendingUpload,
    /// Authenticated history or stored attachment metadata was inconsistent.
    #[error("authenticated attachment metadata is invalid")]
    InvalidMetadata,
    /// Attachment authentication, padding, hash, or MIME verification failed.
    #[error(transparent)]
    Attachment(#[from] AttachmentError),
    /// Encrypted local persistence failed.
    #[error(transparent)]
    KeyStore(#[from] KeyStoreError),
}

/// One private descriptor discovered in authenticated local history.
#[derive(Clone, PartialEq, Eq)]
pub struct PendingAttachmentDownload {
    /// MLS group whose active-device delivery owns the opaque blob.
    pub group_id: GroupId,
    /// Authenticated transport message that carried the descriptor.
    pub message_id: String,
    /// MLS-authenticated sending device. This lets a host avoid re-downloading
    /// content authored and already retained by the current device.
    pub sender_device_id: DeviceId,
    /// Authenticated local deletion deadline inherited from the message.
    pub expires_at_unix: Option<i64>,
    /// Private descriptor containing the independent attachment content key.
    pub descriptor: AttachmentDescriptor,
}

/// One server-confirmed upload whose private descriptor remains native-only.
///
/// The descriptor must be authenticated inside an MLS application event before
/// the upload record is removed. Retaining it here closes the crash window
/// between opaque upload acceptance and durable message preparation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfirmedAttachmentUpload {
    /// MLS group that owns the opaque upload.
    pub group_id: GroupId,
    /// Active local device that created the upload.
    pub device_id: DeviceId,
    /// Private descriptor to carry only inside MLS ciphertext.
    pub descriptor: AttachmentDescriptor,
    /// Exact Delivery Service acceptance response.
    pub response: PutE2eeAttachmentResponse,
}

impl core::fmt::Debug for PendingAttachmentDownload {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("PendingAttachmentDownload")
            .field("group_id", &self.group_id)
            .field("message_id", &self.message_id)
            .field("sender_device_id", &self.sender_device_id)
            .field("expires_at_unix", &self.expires_at_unix)
            .field("descriptor", &self.descriptor)
            .finish()
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredAttachmentManifest {
    version: u16,
    group_id: String,
    message_id: String,
    attachment_id: String,
    filename: String,
    mime_type: String,
    plaintext_size: u64,
    content_hash: [u8; 32],
    descriptor_sha256: [u8; 32],
    chunk_count: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    expires_at_unix: Option<i64>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AttachmentAckRecord {
    version: u16,
    request: AckE2eeAttachmentsRequest,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AttachmentUploadRecord {
    version: u16,
    group_id: String,
    device_id: String,
    descriptor: AttachmentDescriptor,
    ciphertext_bytes: u64,
    ciphertext_sha256: [u8; 32],
    chunk_count: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    response: Option<PutE2eeAttachmentResponse>,
}

/// Encrypt and atomically persist one exact opaque attachment upload.
///
/// Only one upload may be staged per group. The private descriptor and every
/// exact-bucket ciphertext chunk become durable in the same encrypted-store
/// transaction before a network request can be made.
///
/// # Errors
/// Rejects invalid attachment input, an existing group upload, impossible
/// transport sizes, or encrypted-store failures.
pub fn prepare_attachment_upload(
    store: &dyn LocalKeyStore,
    group_id: GroupId,
    device_id: DeviceId,
    filename: impl Into<String>,
    plaintext: &[u8],
) -> Result<AttachmentDescriptor, DurableAttachmentError> {
    let manifest_key = attachment_upload_manifest_key(group_id)?;
    if store.exists(&manifest_key)? {
        return Err(DurableAttachmentError::PendingUpload);
    }
    let (descriptor, encrypted) = crate::encrypt_attachment(filename, plaintext)?;
    let chunk_count = encrypted.ciphertext.len().div_ceil(ATTACHMENT_CHUNK_BYTES);
    if !E2EE_ATTACHMENT_CIPHERTEXT_BUCKETS.contains(&encrypted.ciphertext.len())
        || chunk_count == 0
        || chunk_count > MAX_ATTACHMENT_UPLOAD_CHUNKS
        || chunk_count
            .checked_add(1)
            .is_none_or(|entries| entries > MAX_STORE_BATCH_ENTRIES)
    {
        return Err(DurableAttachmentError::InvalidMetadata);
    }
    let record = AttachmentUploadRecord {
        version: ATTACHMENT_UPLOAD_VERSION,
        group_id: group_id.to_string(),
        device_id: device_id.to_string(),
        descriptor: descriptor.clone(),
        ciphertext_bytes: u64::try_from(encrypted.ciphertext.len())
            .map_err(|_| DurableAttachmentError::InvalidMetadata)?,
        ciphertext_sha256: Sha256::digest(&encrypted.ciphertext).into(),
        chunk_count: u16::try_from(chunk_count)
            .map_err(|_| DurableAttachmentError::InvalidMetadata)?,
        response: None,
    };
    let mut entries = Vec::with_capacity(chunk_count + 1);
    entries.push((
        manifest_key,
        encode_json(&record).map_err(DurableAttachmentError::KeyStore)?,
    ));
    for (index, chunk) in encrypted
        .ciphertext
        .chunks(ATTACHMENT_CHUNK_BYTES)
        .enumerate()
    {
        entries.push((
            attachment_upload_chunk_key(group_id, index)?,
            chunk.to_vec(),
        ));
    }
    store.store_batch(entries)?;
    Ok(descriptor)
}

/// Reconstruct the exact durable ciphertext that must be uploaded or retried.
///
/// A confirmed upload returns `None`; its descriptor remains available through
/// [`confirmed_attachment_upload`] until the authenticated message is durable.
///
/// # Errors
/// Rejects corrupt, torn, substituted, or cross-device upload records.
pub fn pending_attachment_upload(
    store: &dyn LocalKeyStore,
    group_id: GroupId,
    device_id: DeviceId,
) -> Result<Option<EncryptedAttachment>, DurableAttachmentError> {
    let Some(record) = load_attachment_upload_record(store, group_id)? else {
        return Ok(None);
    };
    validate_attachment_upload_record(&record, group_id, device_id)?;
    if record.response.is_some() {
        return Ok(None);
    }
    let ciphertext = load_attachment_upload_ciphertext(store, group_id, &record)?;
    Ok(Some(EncryptedAttachment {
        attachment_id: record.descriptor.attachment_id,
        ciphertext,
    }))
}

/// Mark an exact durable upload as accepted while retaining its descriptor.
///
/// The ciphertext chunks intentionally remain until the descriptor has been
/// committed inside authenticated local message history. This permits strict
/// reconciliation and avoids a secret-loss window after server acceptance.
///
/// # Errors
/// Rejects substituted ciphertext/responses, expired acceptance, corrupt local
/// records, duplicate confirmation, or encrypted-store failures.
pub fn confirm_attachment_upload(
    store: &dyn LocalKeyStore,
    group_id: GroupId,
    device_id: DeviceId,
    submitted: &EncryptedAttachment,
    response: &PutE2eeAttachmentResponse,
    now_unix: i64,
) -> Result<(), DurableAttachmentError> {
    if !(0..=MAX_UNIX_TIMESTAMP).contains(&now_unix) {
        return Err(DurableAttachmentError::InvalidMetadata);
    }
    let mut record = load_attachment_upload_record(store, group_id)?
        .ok_or(DurableAttachmentError::InvalidMetadata)?;
    validate_attachment_upload_record(&record, group_id, device_id)?;
    if record.response.is_some()
        || submitted.attachment_id != record.descriptor.attachment_id
        || submitted.ciphertext.len()
            != usize::try_from(record.ciphertext_bytes)
                .map_err(|_| DurableAttachmentError::InvalidMetadata)?
        || Sha256::digest(&submitted.ciphertext).as_slice() != record.ciphertext_sha256
        || response.attachment_id != record.descriptor.attachment_id.to_string()
        || response.ciphertext_bytes != record.ciphertext_bytes
        || response.expires_at_unix <= now_unix
        || response.expires_at_unix > MAX_UNIX_TIMESTAMP
    {
        return Err(DurableAttachmentError::InvalidMetadata);
    }
    let stored_ciphertext = load_attachment_upload_ciphertext(store, group_id, &record)?;
    if stored_ciphertext != submitted.ciphertext {
        return Err(DurableAttachmentError::InvalidMetadata);
    }
    record.response = Some(response.clone());
    store.store(
        attachment_upload_manifest_key(group_id)?,
        encode_json(&record).map_err(DurableAttachmentError::KeyStore)?,
    )?;
    Ok(())
}

/// Load a server-confirmed private descriptor for native MLS composition.
///
/// # Errors
/// Rejects corrupt, cross-device, expired, or unconfirmed records.
pub fn confirmed_attachment_upload(
    store: &dyn LocalKeyStore,
    group_id: GroupId,
    device_id: DeviceId,
    now_unix: i64,
) -> Result<Option<ConfirmedAttachmentUpload>, DurableAttachmentError> {
    if !(0..=MAX_UNIX_TIMESTAMP).contains(&now_unix) {
        return Err(DurableAttachmentError::InvalidMetadata);
    }
    let Some(record) = load_attachment_upload_record(store, group_id)? else {
        return Ok(None);
    };
    validate_attachment_upload_record(&record, group_id, device_id)?;
    let Some(response) = record.response else {
        return Ok(None);
    };
    if response.expires_at_unix <= now_unix {
        return Err(DurableAttachmentError::InvalidMetadata);
    }
    Ok(Some(ConfirmedAttachmentUpload {
        group_id,
        device_id,
        descriptor: record.descriptor,
        response,
    }))
}

/// Remove an accepted upload only after its descriptor is durable in an
/// authenticated outbound message.
///
/// # Errors
/// Rejects an unconfirmed/substituted upload, torn chunk state, or storage
/// failures.
pub fn remove_confirmed_attachment_upload(
    store: &dyn LocalKeyStore,
    confirmed: &ConfirmedAttachmentUpload,
) -> Result<(), DurableAttachmentError> {
    let record = load_attachment_upload_record(store, confirmed.group_id)?
        .ok_or(DurableAttachmentError::InvalidMetadata)?;
    validate_attachment_upload_record(&record, confirmed.group_id, confirmed.device_id)?;
    if record.response.as_ref() != Some(&confirmed.response)
        || record.descriptor != confirmed.descriptor
    {
        return Err(DurableAttachmentError::InvalidMetadata);
    }
    let _ = load_attachment_upload_ciphertext(store, confirmed.group_id, &record)?;
    let mut keys = Vec::with_capacity(usize::from(record.chunk_count) + 1);
    keys.push(attachment_upload_manifest_key(confirmed.group_id)?);
    for index in 0..usize::from(record.chunk_count) {
        keys.push(attachment_upload_chunk_key(confirmed.group_id, index)?);
    }
    if store.remove_batch(&keys)? != keys.len() {
        return Err(DurableAttachmentError::InvalidMetadata);
    }
    Ok(())
}

/// Hard-delete a confirmed upload after its Delivery Service lifetime ends.
///
/// Unconfirmed uploads are retained for exact retry. A confirmed upload is
/// removed only when its authenticated server expiry is reached.
///
/// # Errors
/// Rejects invalid clocks, corrupt/torn records, or storage failures.
pub fn purge_expired_attachment_upload(
    store: &dyn LocalKeyStore,
    group_id: GroupId,
    device_id: DeviceId,
    now_unix: i64,
) -> Result<bool, DurableAttachmentError> {
    if !(0..=MAX_UNIX_TIMESTAMP).contains(&now_unix) {
        return Err(DurableAttachmentError::InvalidMetadata);
    }
    let Some(record) = load_attachment_upload_record(store, group_id)? else {
        return Ok(false);
    };
    validate_attachment_upload_record(&record, group_id, device_id)?;
    let Some(response) = record.response.clone() else {
        return Ok(false);
    };
    if response.expires_at_unix > now_unix {
        return Ok(false);
    }
    remove_confirmed_attachment_upload(
        store,
        &ConfirmedAttachmentUpload {
            group_id,
            device_id,
            descriptor: record.descriptor,
            response,
        },
    )?;
    Ok(true)
}

/// Discover missing attachment blobs referenced by MLS-authenticated history.
///
/// The encrypted store is the source of truth: no server routing field can
/// introduce a descriptor. Expired messages and already verified content are
/// omitted. The scan and result count are bounded by the store and `limit`.
///
/// # Errors
/// Rejects zero/oversized limits, corrupt history, descriptor reuse, torn local
/// attachment state, or invalid stored metadata.
pub fn pending_attachment_downloads(
    store: &dyn LocalKeyStore,
    group_id: GroupId,
    now_unix: i64,
    limit: usize,
) -> Result<Vec<PendingAttachmentDownload>, DurableAttachmentError> {
    if !(0..=MAX_UNIX_TIMESTAMP).contains(&now_unix)
        || !(1..=MAX_PENDING_ATTACHMENT_DOWNLOADS).contains(&limit)
    {
        return Err(DurableAttachmentError::InvalidMetadata);
    }
    let mut pending = Vec::new();
    let mut seen = HashSet::new();
    for key in store.list_keys()? {
        let Some((history_group_id, message_id)) = parse_history_key(&key)? else {
            continue;
        };
        if history_group_id != group_id {
            continue;
        }
        let stored = match crate::load_stored_message_at(store, group_id, &message_id, now_unix) {
            Ok(stored) => stored,
            Err(KeyStoreError::NotFound) => continue,
            Err(error) => return Err(error.into()),
        };
        let Ok(event) = VersionedApplicationEvent::decode(&stored.message.plaintext) else {
            // Legacy authenticated application payloads have no attachment
            // descriptor surface and remain presentation-only.
            continue;
        };
        let EncryptedChatEvent::Attachments { attachments, .. } = event.event else {
            continue;
        };
        for descriptor in attachments.as_slice().iter().flat_map(|reference| {
            core::iter::once(&reference.file).chain(reference.thumbnail.as_ref())
        }) {
            descriptor
                .validate()
                .map_err(|_| DurableAttachmentError::InvalidMetadata)?;
            if !seen.insert(descriptor.attachment_id) {
                return Err(DurableAttachmentError::InvalidMetadata);
            }
            let manifest_key = attachment_manifest_key(group_id, descriptor.attachment_id)?;
            if store.exists(&manifest_key)? {
                let manifest = load_manifest(store, &manifest_key)?;
                validate_manifest(&manifest, group_id, &message_id, descriptor)?;
                continue;
            }
            pending.push(PendingAttachmentDownload {
                group_id,
                message_id: message_id.clone(),
                sender_device_id: stored.message.sender_device_id,
                expires_at_unix: stored.expires_at_unix,
                descriptor: descriptor.clone(),
            });
            if pending.len() == limit {
                return Ok(pending);
            }
        }
    }
    Ok(pending)
}

/// Verify and atomically persist one downloaded attachment plus its ack outbox.
///
/// Plaintext is split into bounded records so every protocol attachment bucket
/// remains compatible with the encrypted local store's per-value cap. The
/// acknowledgment becomes sendable only in the same transaction that makes all
/// verified content durable.
///
/// # Errors
/// Rejects substituted descriptors/blobs, existing or torn content, a pending
/// acknowledgment, or any encrypted-store failure.
pub fn persist_downloaded_attachment(
    store: &dyn LocalKeyStore,
    device_id: DeviceId,
    pending: &PendingAttachmentDownload,
    encrypted: &EncryptedAttachment,
) -> Result<AckE2eeAttachmentsRequest, DurableAttachmentError> {
    validate_pending(pending)?;
    let ack_key = attachment_ack_key(pending.group_id)?;
    if store.exists(&ack_key)? {
        return Err(DurableAttachmentError::PendingAcknowledgment);
    }
    let manifest_key = attachment_manifest_key(pending.group_id, pending.descriptor.attachment_id)?;
    if store.exists(&manifest_key)? {
        return Err(DurableAttachmentError::InvalidMetadata);
    }
    if encrypted.attachment_id != pending.descriptor.attachment_id
        || !E2EE_ATTACHMENT_CIPHERTEXT_BUCKETS.contains(&encrypted.ciphertext.len())
    {
        return Err(DurableAttachmentError::InvalidMetadata);
    }

    let content = decrypt_attachment(&pending.descriptor, encrypted)?;
    let chunk_count = content.bytes.len().div_ceil(ATTACHMENT_CHUNK_BYTES);
    if chunk_count == 0
        || chunk_count > MAX_ATTACHMENT_CHUNKS
        || chunk_count
            .checked_add(2)
            .is_none_or(|entries| entries > MAX_STORE_BATCH_ENTRIES)
    {
        return Err(DurableAttachmentError::InvalidMetadata);
    }
    let descriptor_sha256 = descriptor_digest(&pending.descriptor)?;
    let manifest = StoredAttachmentManifest {
        version: ATTACHMENT_MANIFEST_VERSION,
        group_id: pending.group_id.to_string(),
        message_id: pending.message_id.clone(),
        attachment_id: pending.descriptor.attachment_id.to_string(),
        filename: content.filename.clone(),
        mime_type: content.mime_type.clone(),
        plaintext_size: u64::try_from(content.bytes.len())
            .map_err(|_| DurableAttachmentError::InvalidMetadata)?,
        content_hash: pending.descriptor.content_hash,
        descriptor_sha256,
        chunk_count: u16::try_from(chunk_count)
            .map_err(|_| DurableAttachmentError::InvalidMetadata)?,
        expires_at_unix: pending.expires_at_unix,
    };
    let request = AckE2eeAttachmentsRequest {
        device_id: device_id.to_string(),
        attachment_ids: vec![pending.descriptor.attachment_id.to_string()],
    };
    let mut entries = Vec::with_capacity(chunk_count + 2);
    entries.push((
        manifest_key,
        encode_json(&manifest).map_err(DurableAttachmentError::KeyStore)?,
    ));
    for (index, chunk) in content.bytes.chunks(ATTACHMENT_CHUNK_BYTES).enumerate() {
        entries.push((
            attachment_chunk_key(pending.group_id, pending.descriptor.attachment_id, index)?,
            chunk.to_vec(),
        ));
    }
    entries.push((
        ack_key,
        encode_json(&AttachmentAckRecord {
            version: ATTACHMENT_ACK_VERSION,
            request: request.clone(),
        })
        .map_err(DurableAttachmentError::KeyStore)?,
    ));
    store.store_batch(entries)?;
    Ok(request)
}

/// Return a durable verified-decryption acknowledgment for retry.
///
/// # Errors
/// Rejects a corrupt record, wrong device/group binding, or oversized batch.
pub fn pending_attachment_acknowledgment(
    store: &dyn LocalKeyStore,
    group_id: GroupId,
    device_id: DeviceId,
) -> Result<Option<AckE2eeAttachmentsRequest>, DurableAttachmentError> {
    let key = attachment_ack_key(group_id)?;
    if !store.exists(&key)? {
        return Ok(None);
    }
    let encoded = store.load(&key)?;
    let record: AttachmentAckRecord =
        serde_json::from_slice(&encoded).map_err(|_| DurableAttachmentError::InvalidMetadata)?;
    if record.version != ATTACHMENT_ACK_VERSION
        || record.request.device_id != device_id.to_string()
        || record.request.attachment_ids.is_empty()
        || record.request.attachment_ids.len() > MAX_E2EE_ATTACHMENT_ACK_BATCH_SIZE
        || record
            .request
            .attachment_ids
            .iter()
            .any(|value| AttachmentId::try_from(value.clone()).is_err())
    {
        return Err(DurableAttachmentError::InvalidMetadata);
    }
    Ok(Some(record.request))
}

/// Remove an exact attachment acknowledgment after server success.
///
/// # Errors
/// Rejects substituted requests and storage failures.
pub fn confirm_attachment_acknowledgment(
    store: &dyn LocalKeyStore,
    group_id: GroupId,
    submitted: &AckE2eeAttachmentsRequest,
) -> Result<(), DurableAttachmentError> {
    let device_id = DeviceId::try_from(submitted.device_id.clone())
        .map_err(|_| DurableAttachmentError::InvalidMetadata)?;
    let pending = pending_attachment_acknowledgment(store, group_id, device_id)?
        .ok_or(DurableAttachmentError::InvalidMetadata)?;
    if pending != *submitted {
        return Err(DurableAttachmentError::InvalidMetadata);
    }
    store.remove(&attachment_ack_key(group_id)?)?;
    Ok(())
}

/// Load locally retained attachment plaintext after revalidating every chunk.
///
/// # Errors
/// Rejects a descriptor mismatch, missing/torn chunks, altered plaintext,
/// expired content, or invalid MIME/hash metadata.
pub fn load_downloaded_attachment(
    store: &dyn LocalKeyStore,
    group_id: GroupId,
    message_id: &str,
    descriptor: &AttachmentDescriptor,
    now_unix: i64,
) -> Result<Option<AttachmentContent>, DurableAttachmentError> {
    if !(0..=MAX_UNIX_TIMESTAMP).contains(&now_unix) {
        return Err(DurableAttachmentError::InvalidMetadata);
    }
    let key = attachment_manifest_key(group_id, descriptor.attachment_id)?;
    if !store.exists(&key)? {
        return Ok(None);
    }
    let manifest = load_manifest(store, &key)?;
    validate_manifest(&manifest, group_id, message_id, descriptor)?;
    if manifest
        .expires_at_unix
        .is_some_and(|expires_at| expires_at <= now_unix)
    {
        return Ok(None);
    }
    let chunk_count = usize::from(manifest.chunk_count);
    let expected_size = usize::try_from(manifest.plaintext_size)
        .map_err(|_| DurableAttachmentError::InvalidMetadata)?;
    let mut bytes = Zeroizing::new(Vec::with_capacity(expected_size));
    for index in 0..chunk_count {
        let chunk = store.load(&attachment_chunk_key(
            group_id,
            descriptor.attachment_id,
            index,
        )?)?;
        if chunk.is_empty() || chunk.len() > ATTACHMENT_CHUNK_BYTES {
            return Err(DurableAttachmentError::InvalidMetadata);
        }
        bytes.extend_from_slice(&chunk);
    }
    if chunk_count < MAX_ATTACHMENT_CHUNKS
        && store.exists(&attachment_chunk_key(
            group_id,
            descriptor.attachment_id,
            chunk_count,
        )?)?
    {
        return Err(DurableAttachmentError::InvalidMetadata);
    }
    if bytes.len() != expected_size
        || Sha256::digest(bytes.as_slice()).as_slice() != manifest.content_hash
        || sniff_mime(&bytes) != manifest.mime_type
    {
        return Err(DurableAttachmentError::InvalidMetadata);
    }
    Ok(Some(AttachmentContent {
        filename: manifest.filename,
        mime_type: manifest.mime_type,
        bytes,
    }))
}

/// Atomically hard-delete expired verified attachment content.
///
/// Each attachment is removed in its own bounded transaction, so a backend
/// failure can leave only whole attachments for a later retry.
///
/// # Errors
/// Rejects invalid clocks, corrupt manifests/keys, torn chunks, or storage
/// failures.
pub fn purge_expired_attachments(
    store: &dyn LocalKeyStore,
    now_unix: i64,
) -> Result<usize, DurableAttachmentError> {
    if !(0..=MAX_UNIX_TIMESTAMP).contains(&now_unix) {
        return Err(DurableAttachmentError::InvalidMetadata);
    }
    let mut removed_attachments = 0_usize;
    for key in store.list_keys()? {
        let Some((group_id, attachment_id)) = parse_manifest_key(&key)? else {
            continue;
        };
        let manifest = load_manifest(store, &key)?;
        validate_manifest_key_binding(&manifest, group_id, attachment_id)?;
        if manifest
            .expires_at_unix
            .is_none_or(|expires_at| expires_at > now_unix)
        {
            continue;
        }
        let chunk_count = usize::from(manifest.chunk_count);
        let mut keys = Vec::with_capacity(chunk_count + 1);
        keys.push(key);
        for index in 0..chunk_count {
            let chunk_key = attachment_chunk_key(group_id, attachment_id, index)?;
            if !store.exists(&chunk_key)? {
                return Err(DurableAttachmentError::InvalidMetadata);
            }
            keys.push(chunk_key);
        }
        if store.remove_batch(&keys)? != keys.len() {
            return Err(DurableAttachmentError::InvalidMetadata);
        }
        removed_attachments = removed_attachments
            .checked_add(1)
            .ok_or(DurableAttachmentError::InvalidMetadata)?;
    }
    Ok(removed_attachments)
}

fn validate_pending(pending: &PendingAttachmentDownload) -> Result<(), DurableAttachmentError> {
    pending
        .descriptor
        .validate()
        .map_err(|_| DurableAttachmentError::InvalidMetadata)?;
    validate_ulid(&pending.message_id)?;
    if pending
        .expires_at_unix
        .is_some_and(|value| !(1..=MAX_UNIX_TIMESTAMP).contains(&value))
    {
        return Err(DurableAttachmentError::InvalidMetadata);
    }
    Ok(())
}

fn load_manifest(
    store: &dyn LocalKeyStore,
    key: &StoreKey,
) -> Result<StoredAttachmentManifest, DurableAttachmentError> {
    let encoded = store.load(key)?;
    serde_json::from_slice(&encoded).map_err(|_| DurableAttachmentError::InvalidMetadata)
}

fn validate_manifest(
    manifest: &StoredAttachmentManifest,
    group_id: GroupId,
    message_id: &str,
    descriptor: &AttachmentDescriptor,
) -> Result<(), DurableAttachmentError> {
    validate_manifest_key_binding(manifest, group_id, descriptor.attachment_id)?;
    validate_ulid(message_id)?;
    descriptor
        .validate()
        .map_err(|_| DurableAttachmentError::InvalidMetadata)?;
    let plaintext_size = usize::try_from(manifest.plaintext_size)
        .map_err(|_| DurableAttachmentError::InvalidMetadata)?;
    let expected_chunks = plaintext_size.div_ceil(ATTACHMENT_CHUNK_BYTES);
    if manifest.message_id != message_id
        || manifest.filename != descriptor.filename
        || manifest.mime_type != descriptor.mime_type
        || manifest.plaintext_size != descriptor.plaintext_size
        || manifest.content_hash != descriptor.content_hash
        || manifest.descriptor_sha256 != descriptor_digest(descriptor)?
        || expected_chunks == 0
        || expected_chunks > MAX_ATTACHMENT_CHUNKS
        || usize::from(manifest.chunk_count) != expected_chunks
    {
        return Err(DurableAttachmentError::InvalidMetadata);
    }
    Ok(())
}

fn validate_manifest_key_binding(
    manifest: &StoredAttachmentManifest,
    group_id: GroupId,
    attachment_id: AttachmentId,
) -> Result<(), DurableAttachmentError> {
    if manifest.version != ATTACHMENT_MANIFEST_VERSION
        || manifest.group_id != group_id.to_string()
        || manifest.attachment_id != attachment_id.to_string()
        || GroupId::try_from(manifest.group_id.clone()).is_err()
        || AttachmentId::try_from(manifest.attachment_id.clone()).is_err()
        || validate_ulid(&manifest.message_id).is_err()
        || manifest.filename.is_empty()
        || manifest.filename.len() > MAX_ATTACHMENT_FILENAME_BYTES
        || manifest
            .filename
            .chars()
            .any(|character| character.is_control() || matches!(character, '/' | '\\' | ':' | '\0'))
        || manifest.mime_type.is_empty()
        || manifest.mime_type.len() > MAX_ATTACHMENT_MIME_BYTES
        || !manifest
            .mime_type
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'-' | b'+' | b'.'))
        || !manifest
            .mime_type
            .split_once('/')
            .is_some_and(|(top, sub)| !top.is_empty() && !sub.is_empty())
        || manifest.plaintext_size == 0
        || manifest.plaintext_size > MAX_ATTACHMENT_BYTES as u64
        || usize::from(manifest.chunk_count) == 0
        || usize::from(manifest.chunk_count) > MAX_ATTACHMENT_CHUNKS
        || usize::try_from(manifest.plaintext_size)
            .ok()
            .map(|size| size.div_ceil(ATTACHMENT_CHUNK_BYTES))
            != Some(usize::from(manifest.chunk_count))
        || manifest
            .expires_at_unix
            .is_some_and(|value| !(1..=MAX_UNIX_TIMESTAMP).contains(&value))
    {
        return Err(DurableAttachmentError::InvalidMetadata);
    }
    Ok(())
}

fn descriptor_digest(
    descriptor: &AttachmentDescriptor,
) -> Result<[u8; 32], DurableAttachmentError> {
    let mut encoded = Zeroizing::new(
        serde_json::to_vec(descriptor).map_err(|_| DurableAttachmentError::InvalidMetadata)?,
    );
    if encoded.is_empty() || encoded.len() > MAX_STORE_VALUE_BYTES {
        encoded.zeroize();
        return Err(DurableAttachmentError::InvalidMetadata);
    }
    Ok(Sha256::digest(encoded.as_slice()).into())
}

fn encode_json<T: Serialize>(value: &T) -> Result<Vec<u8>, KeyStoreError> {
    let encoded = serde_json::to_vec(value).map_err(|_| KeyStoreError::InvalidValue)?;
    if encoded.is_empty() || encoded.len() > MAX_STORE_VALUE_BYTES {
        return Err(KeyStoreError::LimitExceeded);
    }
    Ok(encoded)
}

fn load_attachment_upload_record(
    store: &dyn LocalKeyStore,
    group_id: GroupId,
) -> Result<Option<AttachmentUploadRecord>, DurableAttachmentError> {
    let key = attachment_upload_manifest_key(group_id)?;
    if !store.exists(&key)? {
        return Ok(None);
    }
    let encoded = store.load(&key)?;
    serde_json::from_slice(&encoded)
        .map(Some)
        .map_err(|_| DurableAttachmentError::InvalidMetadata)
}

fn validate_attachment_upload_record(
    record: &AttachmentUploadRecord,
    group_id: GroupId,
    device_id: DeviceId,
) -> Result<(), DurableAttachmentError> {
    record
        .descriptor
        .validate()
        .map_err(|_| DurableAttachmentError::InvalidMetadata)?;
    let ciphertext_bytes = usize::try_from(record.ciphertext_bytes)
        .map_err(|_| DurableAttachmentError::InvalidMetadata)?;
    let expected_chunks = ciphertext_bytes.div_ceil(ATTACHMENT_CHUNK_BYTES);
    if record.version != ATTACHMENT_UPLOAD_VERSION
        || record.group_id != group_id.to_string()
        || record.device_id != device_id.to_string()
        || GroupId::try_from(record.group_id.clone()).is_err()
        || DeviceId::try_from(record.device_id.clone()).is_err()
        || !E2EE_ATTACHMENT_CIPHERTEXT_BUCKETS.contains(&ciphertext_bytes)
        || expected_chunks == 0
        || expected_chunks > MAX_ATTACHMENT_UPLOAD_CHUNKS
        || usize::from(record.chunk_count) != expected_chunks
        || record.response.as_ref().is_some_and(|response| {
            response.attachment_id != record.descriptor.attachment_id.to_string()
                || response.ciphertext_bytes != record.ciphertext_bytes
                || !(1..=MAX_UNIX_TIMESTAMP).contains(&response.expires_at_unix)
        })
    {
        return Err(DurableAttachmentError::InvalidMetadata);
    }
    Ok(())
}

fn load_attachment_upload_ciphertext(
    store: &dyn LocalKeyStore,
    group_id: GroupId,
    record: &AttachmentUploadRecord,
) -> Result<Vec<u8>, DurableAttachmentError> {
    let expected_size = usize::try_from(record.ciphertext_bytes)
        .map_err(|_| DurableAttachmentError::InvalidMetadata)?;
    let chunk_count = usize::from(record.chunk_count);
    let mut ciphertext = Vec::with_capacity(expected_size);
    for index in 0..chunk_count {
        let chunk = store.load(&attachment_upload_chunk_key(group_id, index)?)?;
        if chunk.is_empty()
            || chunk.len() > ATTACHMENT_CHUNK_BYTES
            || (index + 1 < chunk_count && chunk.len() != ATTACHMENT_CHUNK_BYTES)
        {
            return Err(DurableAttachmentError::InvalidMetadata);
        }
        ciphertext.extend_from_slice(&chunk);
    }
    if chunk_count < MAX_ATTACHMENT_UPLOAD_CHUNKS
        && store.exists(&attachment_upload_chunk_key(group_id, chunk_count)?)?
    {
        return Err(DurableAttachmentError::InvalidMetadata);
    }
    if ciphertext.len() != expected_size
        || Sha256::digest(&ciphertext).as_slice() != record.ciphertext_sha256
    {
        return Err(DurableAttachmentError::InvalidMetadata);
    }
    Ok(ciphertext)
}

fn attachment_upload_manifest_key(group_id: GroupId) -> Result<StoreKey, KeyStoreError> {
    StoreKey::new(format!("attachment-upload:{group_id}:manifest:v1"))
}

fn attachment_upload_chunk_key(group_id: GroupId, index: usize) -> Result<StoreKey, KeyStoreError> {
    if index >= MAX_ATTACHMENT_UPLOAD_CHUNKS {
        return Err(KeyStoreError::InvalidIdentifier);
    }
    StoreKey::new(format!("attachment-upload:{group_id}:chunk:{index}"))
}

fn attachment_manifest_key(
    group_id: GroupId,
    attachment_id: AttachmentId,
) -> Result<StoreKey, KeyStoreError> {
    StoreKey::new(format!("attachment:{group_id}:{attachment_id}:manifest"))
}

fn attachment_chunk_key(
    group_id: GroupId,
    attachment_id: AttachmentId,
    index: usize,
) -> Result<StoreKey, KeyStoreError> {
    if index >= MAX_ATTACHMENT_CHUNKS {
        return Err(KeyStoreError::InvalidIdentifier);
    }
    StoreKey::new(format!(
        "attachment:{group_id}:{attachment_id}:chunk:{index}"
    ))
}

fn attachment_ack_key(group_id: GroupId) -> Result<StoreKey, KeyStoreError> {
    StoreKey::new(format!("attachment:{group_id}:ack:v1"))
}

fn parse_manifest_key(
    key: &StoreKey,
) -> Result<Option<(GroupId, AttachmentId)>, DurableAttachmentError> {
    let Some(suffix) = key.as_str().strip_prefix("attachment:") else {
        return Ok(None);
    };
    let Some(identity) = suffix.strip_suffix(":manifest") else {
        return Ok(None);
    };
    let (group_id, attachment_id) = identity
        .split_once(':')
        .ok_or(DurableAttachmentError::InvalidMetadata)?;
    Ok(Some((
        GroupId::try_from(group_id.to_owned())
            .map_err(|_| DurableAttachmentError::InvalidMetadata)?,
        AttachmentId::try_from(attachment_id.to_owned())
            .map_err(|_| DurableAttachmentError::InvalidMetadata)?,
    )))
}

fn validate_ulid(value: &str) -> Result<(), DurableAttachmentError> {
    if Ulid::from_string(value).is_ok_and(|parsed| parsed.to_string() == value) {
        Ok(())
    } else {
        Err(DurableAttachmentError::InvalidMetadata)
    }
}

fn sniff_mime(plaintext: &[u8]) -> String {
    infer::get(plaintext).map_or_else(
        || String::from("application/octet-stream"),
        |kind| kind.mime_type().to_owned(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        encrypt_attachment, encrypt_thumbnail, AttachmentSet, ChatMessageBody,
        DecryptedApplicationMessage, EncryptedAttachmentReference, EncryptedMessageId,
        InMemoryKeyStore, StoredMailboxMessage,
    };
    use filament_core::UserId;

    const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDRverified attachment";

    fn stored_attachment_message(
        store: &InMemoryKeyStore,
        group_id: GroupId,
        sender_device_id: DeviceId,
        file: AttachmentDescriptor,
        thumbnail: Option<AttachmentDescriptor>,
        expires_at_unix: Option<i64>,
    ) -> (String, AttachmentDescriptor) {
        let message_id = Ulid::new().to_string();
        let event = VersionedApplicationEvent {
            event_id: crate::ApplicationEventId::new(),
            retention_secs: None,
            event: EncryptedChatEvent::Attachments {
                message_id: EncryptedMessageId::new(),
                body: Some(ChatMessageBody::try_from("private file".to_owned()).unwrap()),
                attachments: AttachmentSet::try_from(vec![EncryptedAttachmentReference {
                    file: file.clone(),
                    thumbnail,
                }])
                .unwrap(),
            },
        };
        let stored = StoredMailboxMessage {
            message_id: message_id.clone(),
            group_id,
            created_at_unix: 100,
            expires_at_unix,
            message: DecryptedApplicationMessage {
                sender_user_id: UserId::new(),
                sender_device_id,
                generation: 0,
                plaintext: event.encode().unwrap(),
            },
        };
        let (key, encoded) = crate::durable_mailbox::history_storage_entry(&stored).unwrap();
        store.store(key, encoded).unwrap();
        (message_id, file)
    }

    #[test]
    fn outbound_upload_is_exact_retryable_and_retains_descriptor_until_message_durable() {
        let store = InMemoryKeyStore::new();
        let group_id = GroupId::new();
        let device_id = DeviceId::new();
        let descriptor =
            prepare_attachment_upload(&store, group_id, device_id, "proof.png", PNG).unwrap();
        let pending = pending_attachment_upload(&store, group_id, device_id)
            .unwrap()
            .unwrap();
        assert_eq!(pending.attachment_id, descriptor.attachment_id);
        assert!(E2EE_ATTACHMENT_CIPHERTEXT_BUCKETS.contains(&pending.ciphertext.len()));

        let restarted = pending_attachment_upload(&store, group_id, device_id)
            .unwrap()
            .unwrap();
        assert_eq!(restarted, pending);
        let response = PutE2eeAttachmentResponse {
            attachment_id: descriptor.attachment_id.to_string(),
            ciphertext_bytes: u64::try_from(pending.ciphertext.len()).unwrap(),
            expires_at_unix: 500,
        };
        confirm_attachment_upload(&store, group_id, device_id, &pending, &response, 100).unwrap();
        assert!(pending_attachment_upload(&store, group_id, device_id)
            .unwrap()
            .is_none());

        let confirmed = confirmed_attachment_upload(&store, group_id, device_id, 200)
            .unwrap()
            .unwrap();
        assert_eq!(confirmed.descriptor, descriptor);
        assert_eq!(confirmed.response, response);
        assert!(store
            .list_keys()
            .unwrap()
            .iter()
            .any(|key| key.as_str().starts_with("attachment-upload:")));

        remove_confirmed_attachment_upload(&store, &confirmed).unwrap();
        assert!(
            confirmed_attachment_upload(&store, group_id, device_id, 200)
                .unwrap()
                .is_none()
        );
        assert!(!store
            .list_keys()
            .unwrap()
            .iter()
            .any(|key| key.as_str().starts_with("attachment-upload:")));
    }

    #[test]
    fn outbound_upload_rejects_substitution_and_torn_ciphertext() {
        let store = InMemoryKeyStore::new();
        let group_id = GroupId::new();
        let device_id = DeviceId::new();
        let descriptor =
            prepare_attachment_upload(&store, group_id, device_id, "proof.png", PNG).unwrap();
        assert!(matches!(
            prepare_attachment_upload(&store, group_id, device_id, "other.png", PNG),
            Err(DurableAttachmentError::PendingUpload)
        ));
        let mut pending = pending_attachment_upload(&store, group_id, device_id)
            .unwrap()
            .unwrap();
        let response = PutE2eeAttachmentResponse {
            attachment_id: descriptor.attachment_id.to_string(),
            ciphertext_bytes: u64::try_from(pending.ciphertext.len()).unwrap(),
            expires_at_unix: 500,
        };
        pending.ciphertext[0] ^= 1;
        assert!(matches!(
            confirm_attachment_upload(&store, group_id, device_id, &pending, &response, 100),
            Err(DurableAttachmentError::InvalidMetadata)
        ));
        assert!(pending_attachment_upload(&store, group_id, device_id)
            .unwrap()
            .is_some());

        store
            .remove(&attachment_upload_chunk_key(group_id, 0).unwrap())
            .unwrap();
        assert!(matches!(
            pending_attachment_upload(&store, group_id, device_id),
            Err(DurableAttachmentError::KeyStore(KeyStoreError::NotFound))
        ));
    }

    #[test]
    fn confirmed_outbound_upload_is_hard_deleted_at_server_expiry() {
        let store = InMemoryKeyStore::new();
        let group_id = GroupId::new();
        let device_id = DeviceId::new();
        let descriptor =
            prepare_attachment_upload(&store, group_id, device_id, "proof.png", PNG).unwrap();
        let pending = pending_attachment_upload(&store, group_id, device_id)
            .unwrap()
            .unwrap();
        let response = PutE2eeAttachmentResponse {
            attachment_id: descriptor.attachment_id.to_string(),
            ciphertext_bytes: u64::try_from(pending.ciphertext.len()).unwrap(),
            expires_at_unix: 150,
        };
        confirm_attachment_upload(&store, group_id, device_id, &pending, &response, 100).unwrap();

        assert!(!purge_expired_attachment_upload(&store, group_id, device_id, 149).unwrap());
        assert!(purge_expired_attachment_upload(&store, group_id, device_id, 150).unwrap());
        assert!(store
            .list_keys()
            .unwrap()
            .iter()
            .all(|key| !key.as_str().starts_with("attachment-upload:")));
    }

    #[test]
    fn authenticated_history_drives_verified_durable_download_and_ack_retry() {
        let store = InMemoryKeyStore::new();
        let group_id = GroupId::new();
        let device_id = DeviceId::new();
        let sender_device_id = DeviceId::new();
        let (descriptor, encrypted) = encrypt_attachment("photo.png", PNG).unwrap();
        let (message_id, descriptor) = stored_attachment_message(
            &store,
            group_id,
            sender_device_id,
            descriptor,
            None,
            Some(500),
        );

        let pending = pending_attachment_downloads(&store, group_id, 200, 4).unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].sender_device_id, sender_device_id);
        let acknowledgment =
            persist_downloaded_attachment(&store, device_id, &pending[0], &encrypted).unwrap();
        assert_eq!(
            pending_attachment_acknowledgment(&store, group_id, device_id).unwrap(),
            Some(acknowledgment.clone())
        );
        let loaded = load_downloaded_attachment(&store, group_id, &message_id, &descriptor, 200)
            .unwrap()
            .unwrap();
        assert_eq!(loaded.filename, "photo.png");
        assert_eq!(loaded.mime_type, "image/png");
        assert_eq!(loaded.bytes.as_slice(), PNG);
        assert!(pending_attachment_downloads(&store, group_id, 200, 4)
            .unwrap()
            .is_empty());

        confirm_attachment_acknowledgment(&store, group_id, &acknowledgment).unwrap();
        assert!(
            pending_attachment_acknowledgment(&store, group_id, device_id)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn full_size_range_is_chunked_below_local_value_and_batch_caps() {
        let store = InMemoryKeyStore::new();
        let group_id = GroupId::new();
        let device_id = DeviceId::new();
        let plaintext = vec![0xA5; 4 * 1024 * 1024 + 17];
        let (descriptor, encrypted) = encrypt_attachment("large.bin", &plaintext).unwrap();
        let (message_id, descriptor) =
            stored_attachment_message(&store, group_id, DeviceId::new(), descriptor, None, None);
        let pending = pending_attachment_downloads(&store, group_id, 100, 1).unwrap();
        persist_downloaded_attachment(&store, device_id, &pending[0], &encrypted).unwrap();
        let loaded = load_downloaded_attachment(&store, group_id, &message_id, &descriptor, 100)
            .unwrap()
            .unwrap();
        assert_eq!(loaded.bytes.as_slice(), plaintext);
        let chunk_keys = store
            .list_keys()
            .unwrap()
            .into_iter()
            .filter(|key| key.as_str().contains(":chunk:"))
            .collect::<Vec<_>>();
        assert_eq!(chunk_keys.len(), 5);
        assert!(chunk_keys
            .iter()
            .all(|key| store.load(key).unwrap().len() <= ATTACHMENT_CHUNK_BYTES));
    }

    #[test]
    fn tampering_never_creates_content_or_acknowledgment() {
        let store = InMemoryKeyStore::new();
        let group_id = GroupId::new();
        let device_id = DeviceId::new();
        let (descriptor, mut encrypted) =
            encrypt_attachment("secret.bin", b"authenticated secret").unwrap();
        stored_attachment_message(&store, group_id, DeviceId::new(), descriptor, None, None);
        let pending = pending_attachment_downloads(&store, group_id, 100, 1).unwrap();
        encrypted.ciphertext[0] ^= 1;
        assert!(matches!(
            persist_downloaded_attachment(&store, device_id, &pending[0], &encrypted),
            Err(DurableAttachmentError::Attachment(
                AttachmentError::VerificationFailed
            ))
        ));
        assert!(
            pending_attachment_acknowledgment(&store, group_id, device_id)
                .unwrap()
                .is_none()
        );
        assert!(!store
            .list_keys()
            .unwrap()
            .iter()
            .any(|key| key.as_str().contains(":manifest")));
    }

    #[test]
    fn expiry_cleanup_removes_complete_content_but_not_retained_content() {
        let store = InMemoryKeyStore::new();
        let group_id = GroupId::new();
        let device_id = DeviceId::new();
        let (expired_descriptor, expired_encrypted) =
            encrypt_attachment("expired.bin", b"expires").unwrap();
        let (expired_message_id, expired_descriptor) = stored_attachment_message(
            &store,
            group_id,
            DeviceId::new(),
            expired_descriptor,
            None,
            Some(200),
        );
        let expired = pending_attachment_downloads(&store, group_id, 100, 1)
            .unwrap()
            .remove(0);
        let ack =
            persist_downloaded_attachment(&store, device_id, &expired, &expired_encrypted).unwrap();
        confirm_attachment_acknowledgment(&store, group_id, &ack).unwrap();

        let (retained_descriptor, retained_encrypted) =
            encrypt_thumbnail("retained.png", PNG).unwrap();
        let (retained_message_id, _retained_file_descriptor) = stored_attachment_message(
            &store,
            group_id,
            DeviceId::new(),
            encrypt_attachment("retained.bin", b"retained").unwrap().0,
            Some(retained_descriptor.clone()),
            None,
        );
        let retained = pending_attachment_downloads(&store, group_id, 100, 2)
            .unwrap()
            .into_iter()
            .find(|pending| pending.descriptor.attachment_id == retained_descriptor.attachment_id)
            .unwrap();
        let ack = persist_downloaded_attachment(&store, device_id, &retained, &retained_encrypted)
            .unwrap();
        confirm_attachment_acknowledgment(&store, group_id, &ack).unwrap();

        assert_eq!(purge_expired_attachments(&store, 200).unwrap(), 1);
        assert!(load_downloaded_attachment(
            &store,
            group_id,
            &expired_message_id,
            &expired_descriptor,
            200
        )
        .unwrap()
        .is_none());
        assert!(load_downloaded_attachment(
            &store,
            group_id,
            &retained_message_id,
            &retained_descriptor,
            200
        )
        .unwrap()
        .is_some());
    }
}
