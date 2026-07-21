//! E2EE wire-contract DTOs for MLS-based end-to-end encryption.
//!
//! These types define the wire format for all E2EE REST endpoints and
//! gateway events. They use `#[serde(deny_unknown_fields)]` for strict
//! boundary parsing and enforce size bounds at parse time.
//!
//! The server never parses MLS interiors — all MLS payloads are opaque
//! `Vec<u8>` blobs. Server-side validation is shape-only: size bounds,
//! field presence, and epoch monotonicity.
//!
//! See [`docs/adr/0001-e2ee-mls-openmls.md`] and [`plans/PLAN_E2EE.md`] for
//! the protocol design.
//!
// MLS protocol terms (KeyPackage, Welcome, GroupInfo, PrivateMessage, etc.)
// are RFC 9420 proper nouns, not variable names. Suppressing the doc-markdown
// lint avoids noise without hiding real issues.
#![allow(clippy::doc_markdown)]

use serde::{de, Deserialize, Deserializer, Serialize};

// ---------------------------------------------------------------------------
// Size bounds
// ---------------------------------------------------------------------------

/// Maximum size for a serialized KeyPackage blob (4 KiB).
pub const MAX_KEYPACKAGE_BYTES: usize = 4_096;

/// Maximum size for a serialized MLS message blob (64 KiB).
pub const MAX_MLS_MESSAGE_BYTES: usize = 65_536;

/// Maximum size for a serialized commit blob (64 KiB).
pub const MAX_COMMIT_BYTES: usize = 65_536;

/// Maximum size for a serialized Welcome blob (64 KiB).
pub const MAX_WELCOME_BYTES: usize = 65_536;

/// Maximum size for a serialized proposal blob (64 KiB).
pub const MAX_PROPOSAL_BYTES: usize = 65_536;

/// Maximum size for a serialized GroupInfo blob (64 KiB).
pub const MAX_GROUP_INFO_BYTES: usize = 65_536;

/// Maximum KeyPackage pool size per upload request.
pub const MAX_KEYPACKAGE_POOL_SIZE: usize = 100;

/// Maximum number of devices returned in a device list response.
pub const MAX_DEVICE_LIST_SIZE: usize = 100;

/// Maximum number of opaque messages returned by one mailbox page.
pub const MAX_E2EE_MAILBOX_PAGE_SIZE: usize = 50;

/// Maximum number of opaque commits returned by one mailbox page.
pub const MAX_E2EE_COMMIT_MAILBOX_PAGE_SIZE: usize = 50;

/// Maximum aggregate ciphertext bytes returned by one mailbox page.
///
/// JSON represents byte vectors as integer arrays, so this keeps the encoded
/// response bounded near the server's default 1 MiB HTTP body limit.
pub const MAX_E2EE_MAILBOX_PAGE_BLOB_BYTES: usize = 256 * 1_024;

/// Maximum aggregate commit and Welcome bytes returned by one mailbox page.
pub const MAX_E2EE_COMMIT_MAILBOX_PAGE_BLOB_BYTES: usize = 256 * 1_024;

/// Maximum number of message acknowledgments accepted in one request.
pub const MAX_E2EE_MESSAGE_ACK_BATCH_SIZE: usize = 100;

/// Maximum number of commit epochs acknowledged in one request.
pub const MAX_E2EE_COMMIT_ACK_BATCH_SIZE: usize = 100;

/// Root-identity rotation wire protocol version.
pub const ROOT_IDENTITY_ROTATION_PROTOCOL_VERSION: u16 = 1;

/// Maximum destructive root rotations retained for one identity.
pub const MAX_ROOT_IDENTITY_ROTATIONS: usize = 100;

/// Ed25519 public-key size.
pub const ED25519_PUBLIC_KEY_BYTES: usize = 32;

/// Ed25519 signature size.
pub const ED25519_SIGNATURE_BYTES: usize = 64;

fn deserialize_exact_bytes<'de, D, const N: usize>(deserializer: D) -> Result<Vec<u8>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Vec::<u8>::deserialize(deserializer)?;
    if value.len() != N {
        return Err(de::Error::invalid_length(
            value.len(),
            &"an exact-length byte array",
        ));
    }
    Ok(value)
}

fn deserialize_ed25519_public_key<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_exact_bytes::<D, ED25519_PUBLIC_KEY_BYTES>(deserializer)
}

fn deserialize_ed25519_signature<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_exact_bytes::<D, ED25519_SIGNATURE_BYTES>(deserializer)
}

fn deserialize_key_package_blob<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_bounded_blob::<D, MAX_KEYPACKAGE_BYTES>(deserializer)
}

fn deserialize_bounded_blob<'de, D, const N: usize>(deserializer: D) -> Result<Vec<u8>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Vec::<u8>::deserialize(deserializer)?;
    if value.is_empty() || value.len() > N {
        return Err(de::Error::invalid_length(
            value.len(),
            &"a non-empty blob within the protocol size limit",
        ));
    }
    Ok(value)
}

fn deserialize_optional_bounded_blob<'de, D, const N: usize>(
    deserializer: D,
) -> Result<Option<Vec<u8>>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<Vec<u8>>::deserialize(deserializer)?;
    value
        .map(|blob| {
            if blob.is_empty() || blob.len() > N {
                Err(de::Error::invalid_length(
                    blob.len(),
                    &"a non-empty blob within the protocol size limit",
                ))
            } else {
                Ok(blob)
            }
        })
        .transpose()
}

fn deserialize_device_list<'de, D>(deserializer: D) -> Result<Vec<DeviceInfo>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Vec::<DeviceInfo>::deserialize(deserializer)?;
    if value.len() > MAX_DEVICE_LIST_SIZE {
        return Err(de::Error::invalid_length(
            value.len(),
            &"no more than 100 devices",
        ));
    }
    Ok(value)
}

fn deserialize_root_rotation_list<'de, D>(
    deserializer: D,
) -> Result<Vec<RootIdentityRotationEntry>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Vec::<RootIdentityRotationEntry>::deserialize(deserializer)?;
    if value.len() > MAX_ROOT_IDENTITY_ROTATIONS {
        return Err(de::Error::invalid_length(
            value.len(),
            &"no more than 100 root identity rotations",
        ));
    }
    Ok(value)
}

fn deserialize_group_info_blob<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_bounded_blob::<D, MAX_GROUP_INFO_BYTES>(deserializer)
}

fn deserialize_commit_blob<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_bounded_blob::<D, MAX_COMMIT_BYTES>(deserializer)
}

fn deserialize_optional_welcome_blob<'de, D>(deserializer: D) -> Result<Option<Vec<u8>>, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_optional_bounded_blob::<D, MAX_WELCOME_BYTES>(deserializer)
}

fn deserialize_bounded_welcome_blob<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_bounded_blob::<D, MAX_WELCOME_BYTES>(deserializer)
}

fn deserialize_optional_group_info_blob<'de, D>(
    deserializer: D,
) -> Result<Option<Vec<u8>>, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_optional_bounded_blob::<D, MAX_GROUP_INFO_BYTES>(deserializer)
}

fn deserialize_mls_message_blob<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_bounded_blob::<D, MAX_MLS_MESSAGE_BYTES>(deserializer)
}

fn deserialize_padded_mls_message_blob<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = deserialize_mls_message_blob(deserializer)?;
    if !matches!(value.len(), 512 | 1_024 | 4_096 | 16_384) {
        return Err(de::Error::invalid_length(
            value.len(),
            &"an exact MLS ciphertext padding bucket",
        ));
    }
    Ok(value)
}

fn deserialize_mls_v1_mode<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let value = String::deserialize(deserializer)?;
    if value != "mls_v1" {
        return Err(de::Error::custom("mailbox crypto mode must be mls_v1"));
    }
    Ok(value)
}

fn deserialize_mailbox_messages<'de, D>(
    deserializer: D,
) -> Result<Vec<E2eeMailboxMessage>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Vec::<E2eeMailboxMessage>::deserialize(deserializer)?;
    if value.len() > MAX_E2EE_MAILBOX_PAGE_SIZE {
        return Err(de::Error::invalid_length(
            value.len(),
            &"no more than 50 mailbox messages",
        ));
    }
    let aggregate_bytes = value.iter().try_fold(0_usize, |total, message| {
        total.checked_add(message.message_blob.len())
    });
    if aggregate_bytes.is_none_or(|total| total > MAX_E2EE_MAILBOX_PAGE_BLOB_BYTES) {
        return Err(de::Error::custom(
            "mailbox ciphertext aggregate exceeds protocol limit",
        ));
    }
    Ok(value)
}

fn deserialize_message_ack_ids<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Vec::<String>::deserialize(deserializer)?;
    if value.is_empty() || value.len() > MAX_E2EE_MESSAGE_ACK_BATCH_SIZE {
        return Err(de::Error::invalid_length(
            value.len(),
            &"between 1 and 100 message IDs",
        ));
    }
    Ok(value)
}

fn deserialize_commit_mailbox_entries<'de, D>(
    deserializer: D,
) -> Result<Vec<E2eeCommitMailboxEntry>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Vec::<E2eeCommitMailboxEntry>::deserialize(deserializer)?;
    if value.len() > MAX_E2EE_COMMIT_MAILBOX_PAGE_SIZE {
        return Err(de::Error::invalid_length(
            value.len(),
            &"no more than 50 commit mailbox entries",
        ));
    }
    let aggregate_bytes = value.iter().try_fold(0_usize, |total, entry| {
        total
            .checked_add(entry.commit_blob.len())?
            .checked_add(entry.welcome_blob.as_ref().map_or(0, Vec::len))
    });
    if aggregate_bytes.is_none_or(|total| total > MAX_E2EE_COMMIT_MAILBOX_PAGE_BLOB_BYTES) {
        return Err(de::Error::custom(
            "commit mailbox aggregate exceeds protocol limit",
        ));
    }
    Ok(value)
}

fn deserialize_commit_ack_epochs<'de, D>(deserializer: D) -> Result<Vec<u64>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Vec::<u64>::deserialize(deserializer)?;
    if value.is_empty() || value.len() > MAX_E2EE_COMMIT_ACK_BATCH_SIZE || value.contains(&0) {
        return Err(de::Error::invalid_length(
            value.len(),
            &"between 1 and 100 positive commit epochs",
        ));
    }
    Ok(value)
}

fn deserialize_key_package_entries<'de, D>(
    deserializer: D,
) -> Result<Vec<KeyPackageEntry>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Vec::<KeyPackageEntry>::deserialize(deserializer)?;
    if value.is_empty() || value.len() > MAX_KEYPACKAGE_POOL_SIZE {
        return Err(de::Error::invalid_length(
            value.len(),
            &"between 1 and 100 KeyPackages",
        ));
    }
    Ok(value)
}

// ---------------------------------------------------------------------------
// Device Certificate endpoints
// ---------------------------------------------------------------------------

/// Request body for `PUT /e2ee/devices/{device_id}` — publish device certificate.
///
/// The certificate is signed by the user's root identity key. The server
/// validates the signature against the published root key and stores the
/// certificate as public material. It never holds the root key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublishDeviceCertificateRequest {
    /// The device's MLS signature public key (raw bytes).
    #[serde(deserialize_with = "deserialize_ed25519_public_key")]
    pub device_signature_pubkey: Vec<u8>,
    /// The root identity key's signature over the certificate.
    #[serde(deserialize_with = "deserialize_ed25519_signature")]
    pub root_key_signature: Vec<u8>,
    /// The user's Ed25519 root identity public key.
    #[serde(deserialize_with = "deserialize_ed25519_public_key")]
    pub root_key_pub: Vec<u8>,
}

/// Response body for `PUT /e2ee/devices/{device_id}` — device publish result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublishDeviceCertificateResponse {
    /// The device ID that was published.
    pub device_id: String,
    /// Whether the certificate was accepted.
    pub published: bool,
}

/// Response body for `DELETE /e2ee/devices/{device_id}`.
///
/// Removal is irreversible for a device ID. Any unclaimed KeyPackages for the
/// device are destroyed in the same transaction as the certificate tombstone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RemoveDeviceResponse {
    /// The tombstoned device ID.
    pub device_id: String,
    /// Unix timestamp (seconds) when the device was tombstoned.
    pub tombstoned_at_unix: i64,
    /// Number of unclaimed KeyPackages destroyed during removal.
    pub deleted_keypackage_count: u32,
}

/// Response body for `GET /e2ee/users/{user_id}/devices` — certified device list.
///
/// The list is a hint; clients verify certificate signatures against pinned
/// root keys. The server cannot mint devices — it never holds root keys.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeviceListResponse {
    /// The user whose devices are listed.
    pub user_id: String,
    /// The certified device list (public material only).
    #[serde(deserialize_with = "deserialize_device_list")]
    pub devices: Vec<DeviceInfo>,
}

/// A single device entry in a `DeviceListResponse`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeviceInfo {
    /// The device ID (ULID string).
    pub device_id: String,
    /// The device's MLS signature public key (raw bytes).
    #[serde(deserialize_with = "deserialize_ed25519_public_key")]
    pub device_signature_pubkey: Vec<u8>,
    /// The root identity key's signature over the certificate.
    #[serde(deserialize_with = "deserialize_ed25519_signature")]
    pub root_key_signature: Vec<u8>,
    /// The user's Ed25519 root identity public key.
    #[serde(deserialize_with = "deserialize_ed25519_public_key")]
    pub root_key_pub: Vec<u8>,
    /// Unix timestamp (seconds) when the device was registered.
    pub created_at_unix: i64,
    /// Whether the device has been tombstoned (removed).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tombstoned_at_unix: Option<i64>,
}

/// Request body for `POST /e2ee/identity/rotate`.
///
/// Both roots sign the same continuity transition. The replacement root also
/// certifies fresh signing material for the sole device retained by the
/// destructive rotation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RotateRootIdentityRequest {
    /// Exact protocol version; currently [`ROOT_IDENTITY_ROTATION_PROTOCOL_VERSION`].
    pub protocol_version: u16,
    /// Current sequence observed by the client. The signed transition uses
    /// `expected_rotation_sequence + 1`.
    pub expected_rotation_sequence: u64,
    /// Existing active device retained and re-certified by the rotation.
    pub device_id: String,
    /// Replacement root public key.
    #[serde(deserialize_with = "deserialize_ed25519_public_key")]
    pub new_root_key_pub: Vec<u8>,
    /// Previous root's authorization signature over the transition.
    #[serde(deserialize_with = "deserialize_ed25519_signature")]
    pub previous_root_signature: Vec<u8>,
    /// Replacement root's proof-of-possession signature over the transition.
    #[serde(deserialize_with = "deserialize_ed25519_signature")]
    pub new_root_signature: Vec<u8>,
    /// Fresh device MLS signature public key.
    #[serde(deserialize_with = "deserialize_ed25519_public_key")]
    pub new_device_signature_pubkey: Vec<u8>,
    /// Replacement root's certificate signature for the retained device.
    #[serde(deserialize_with = "deserialize_ed25519_signature")]
    pub new_device_root_signature: Vec<u8>,
}

/// Response body for `POST /e2ee/identity/rotate`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RotateRootIdentityResponse {
    pub protocol_version: u16,
    pub user_id: String,
    pub device_id: String,
    pub rotation_sequence: u64,
    #[serde(deserialize_with = "deserialize_ed25519_public_key")]
    pub previous_root_key_pub: Vec<u8>,
    #[serde(deserialize_with = "deserialize_ed25519_public_key")]
    pub new_root_key_pub: Vec<u8>,
    pub revoked_device_count: u32,
    pub deleted_keypackage_count: u32,
    pub rotated_at_unix: i64,
}

/// One public, dual-signed root continuity transition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RootIdentityRotationEntry {
    pub sequence: u64,
    #[serde(deserialize_with = "deserialize_ed25519_public_key")]
    pub previous_root_key_pub: Vec<u8>,
    #[serde(deserialize_with = "deserialize_ed25519_public_key")]
    pub new_root_key_pub: Vec<u8>,
    #[serde(deserialize_with = "deserialize_ed25519_signature")]
    pub previous_root_signature: Vec<u8>,
    #[serde(deserialize_with = "deserialize_ed25519_signature")]
    pub new_root_signature: Vec<u8>,
    pub rotating_device_id: String,
    pub rotated_at_unix: i64,
}

/// Response body for `GET /e2ee/users/{user_id}/identity`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RootIdentityDirectoryResponse {
    pub protocol_version: u16,
    pub user_id: String,
    #[serde(deserialize_with = "deserialize_ed25519_public_key")]
    pub current_root_key_pub: Vec<u8>,
    pub rotation_sequence: u64,
    #[serde(deserialize_with = "deserialize_root_rotation_list")]
    pub rotations: Vec<RootIdentityRotationEntry>,
}

// ---------------------------------------------------------------------------
// KeyPackage endpoints
// ---------------------------------------------------------------------------

/// Request body for `POST /e2ee/keypackages` — upload KeyPackage pool.
///
/// KeyPackages are MLS's prekey analog. Each device uploads a pool of
/// single-use KeyPackages plus one one-time last-resort fallback. The server stores
/// them as opaque blobs and never parses interiors.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UploadKeyPackagesRequest {
    /// The device uploading the packages (ULID string).
    pub device_id: String,
    /// The KeyPackage blobs (opaque, serialized MLS KeyPackages).
    #[serde(deserialize_with = "deserialize_key_package_entries")]
    pub key_packages: Vec<KeyPackageEntry>,
}

/// A single KeyPackage entry in an upload request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KeyPackageEntry {
    /// The serialized KeyPackage blob (opaque to the server).
    #[serde(deserialize_with = "deserialize_key_package_blob")]
    pub key_package_blob: Vec<u8>,
    /// Whether this is the one-time fallback reserved until ordinary packages
    /// are exhausted.
    pub is_last_resort: bool,
}

/// Response body for `POST /e2ee/keypackages` — upload result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UploadKeyPackagesResponse {
    /// The number of KeyPackages stored.
    pub stored_count: u32,
}

/// Request body for `POST /e2ee/keypackages/claim` — claim a KeyPackage.
///
/// Claims are rate-limited and audit-logged. The server atomically
/// decrements the pool and returns one KeyPackage for the target user/device.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClaimKeyPackageRequest {
    /// The target user whose KeyPackage is being claimed (ULID string).
    pub target_user_id: String,
    /// Optional: target a specific device (ULID string). If `None`, any
    /// device's KeyPackage may be claimed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_device_id: Option<String>,
}

/// Response body for `POST /e2ee/keypackages/claim` — claimed KeyPackage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClaimKeyPackageResponse {
    /// The device ID whose KeyPackage was claimed (ULID string).
    pub device_id: String,
    /// The claimed KeyPackage blob (opaque to the server).
    #[serde(deserialize_with = "deserialize_key_package_blob")]
    pub key_package_blob: Vec<u8>,
    /// Whether this was a last-resort KeyPackage.
    pub is_last_resort: bool,
}

// ---------------------------------------------------------------------------
// Group and message transport endpoints
// ---------------------------------------------------------------------------

/// Atomic bootstrap for a new two-user MLS v1 conversation.
///
/// The initial Add commit, Welcome, and GroupInfo are required so the server
/// never exposes a half-provisioned group that cannot be joined or recovered.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateMlsConversationRequest {
    /// Client-generated canonical conversation ULID used for idempotency.
    pub conversation_id: String,
    /// The other user in this 1:1 conversation.
    pub peer_user_id: String,
    /// Client-generated canonical MLS group ULID.
    pub group_id: String,
    /// Ciphersuite identifier carried for suite agility.
    pub suite_id: u16,
    /// Active device that authored the initial Add commit.
    pub committer_device_id: String,
    /// Active peer device whose claimed KeyPackage encrypts the Welcome.
    pub welcome_device_id: String,
    /// Initial MLS commit advancing epoch 0 to epoch 1.
    #[serde(deserialize_with = "deserialize_commit_blob")]
    pub commit_blob: Vec<u8>,
    /// Initial Welcome for the invited participant.
    #[serde(deserialize_with = "deserialize_bounded_welcome_blob")]
    pub welcome_blob: Vec<u8>,
    /// Initial GroupInfo used for join and recovery.
    #[serde(deserialize_with = "deserialize_group_info_blob")]
    pub group_info_blob: Vec<u8>,
}

/// Atomic bootstrap fields for explicitly upgrading an existing plaintext
/// two-user conversation to MLS v1.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpgradeMlsConversationRequest {
    /// Client-generated canonical MLS group ULID.
    pub group_id: String,
    /// Ciphersuite identifier carried for suite agility.
    pub suite_id: u16,
    /// Active device that authored the initial Add commit.
    pub committer_device_id: String,
    /// Active peer device whose claimed KeyPackage encrypts the Welcome.
    pub welcome_device_id: String,
    /// Initial MLS commit advancing epoch 0 to epoch 1.
    #[serde(deserialize_with = "deserialize_commit_blob")]
    pub commit_blob: Vec<u8>,
    /// Initial Welcome for the invited participant.
    #[serde(deserialize_with = "deserialize_bounded_welcome_blob")]
    pub welcome_blob: Vec<u8>,
    /// Initial GroupInfo used for join and recovery.
    #[serde(deserialize_with = "deserialize_group_info_blob")]
    pub group_info_blob: Vec<u8>,
}

/// Result of atomically creating or upgrading a two-user MLS conversation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MlsConversationProvisionResponse {
    /// Canonical conversation ULID.
    pub conversation_id: String,
    /// Canonical MLS group ULID.
    pub group_id: String,
    /// Conversation-level mode. Always `mls_v1`.
    #[serde(deserialize_with = "deserialize_mls_v1_mode")]
    pub crypto: String,
    /// Accepted initial epoch. Always 1 for this protocol version.
    pub epoch: u64,
    /// Accepted ciphersuite identifier.
    pub suite_id: u16,
    /// Unix timestamp when provisioning committed.
    pub provisioned_at_unix: i64,
}

/// Response body for `GET /e2ee/groups/{group_id}/info` — encrypted GroupInfo.
///
/// The GroupInfo blob is published by group members to support joins and
/// external-commit recovery. It encodes membership structure (which the
/// server already knows from routing) and is treated as sensitive-not-secret.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GroupInfoResponse {
    /// The group ID (ULID string).
    pub group_id: String,
    /// The current epoch of the group.
    pub epoch: u64,
    /// The ciphersuite ID (e.g. 0x0003).
    pub suite_id: u16,
    /// The serialized GroupInfo blob (opaque to the server).
    #[serde(deserialize_with = "deserialize_group_info_blob")]
    pub group_info_blob: Vec<u8>,
}

/// Request body for `POST /e2ee/groups/{group_id}/commits` — commit ingestion.
///
/// The Delivery Service enforces single-writer-per-epoch: the first
/// order-valid commit for epoch N is accepted; competing commits receive
/// `409 epoch_conflict`. Server validation is shape-only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PostCommitRequest {
    /// The epoch this commit advances to.
    pub epoch: u64,
    /// The previous epoch (for monotonicity validation).
    pub prior_epoch: u64,
    /// The committing device ID (ULID string).
    pub committer_device_id: String,
    /// The serialized commit blob (opaque MLS Commit).
    #[serde(deserialize_with = "deserialize_commit_blob")]
    pub commit_blob: Vec<u8>,
    /// Optional Welcome message for new members (opaque blob).
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_optional_welcome_blob"
    )]
    pub welcome_blob: Option<Vec<u8>>,
    /// Exact active device whose KeyPackage the optional Welcome targets.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub welcome_device_id: Option<String>,
    /// Optional updated GroupInfo blob for joins/recovery.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_optional_group_info_blob"
    )]
    pub group_info_blob: Option<Vec<u8>>,
}

/// Response body for `POST /e2ee/groups/{group_id}/commits` — commit result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PostCommitResponse {
    /// Whether the commit was accepted.
    pub accepted: bool,
    /// The new epoch after the commit.
    pub epoch: u64,
}

/// Request body for `POST /e2ee/groups/{group_id}/messages` — PrivateMessage transport.
///
/// The server stores the opaque, bucket-padded serialized `PrivateMessage`
/// frame plus a minimal routing envelope. It never parses MLS interiors.
/// Size-bucket padding is verified.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PostMessageRequest {
    /// The epoch in which the message was sent.
    pub epoch: u64,
    /// The ciphersuite ID (routing hint only).
    pub suite_id: u16,
    /// The sending device ID (ULID string, routing hint only).
    pub sender_device_id: String,
    /// The serialized MLS PrivateMessage blob (opaque to the server).
    #[serde(deserialize_with = "deserialize_mls_message_blob")]
    pub message_blob: Vec<u8>,
}

/// Response body for `POST /e2ee/groups/{group_id}/messages` — message accepted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PostMessageResponse {
    /// The assigned message ID (ULID string).
    pub message_id: String,
    /// Unix timestamp (seconds) when the message was received.
    pub created_at_unix: i64,
}

/// Query parameters for a device's pending message mailbox.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct E2eeMailboxQuery {
    /// Active device whose snapshotted deliveries should be returned.
    pub device_id: String,
    /// Exclusive message-ID cursor from the preceding page.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after_message_id: Option<String>,
    /// Requested record count. The server caps this at 50.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u16>,
}

/// One opaque MLS `PrivateMessage` pending delivery to a device.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct E2eeMailboxMessage {
    /// Server-assigned message ID (ULID string).
    pub message_id: String,
    /// Conversation-level crypto mode. Always `mls_v1` for this endpoint.
    #[serde(deserialize_with = "deserialize_mls_v1_mode")]
    pub crypto: String,
    /// Epoch routing hint; clients verify it against authenticated MLS data.
    pub epoch: u64,
    /// Ciphersuite routing hint; clients verify it against local group state.
    pub suite_id: u16,
    /// Sender device routing hint; clients verify it after MLS authentication.
    pub sender_device_id: String,
    /// Bucket-padded serialized MLS `PrivateMessage`, opaque to the server.
    #[serde(deserialize_with = "deserialize_padded_mls_message_blob")]
    pub message_blob: Vec<u8>,
    /// Unix timestamp (seconds) when the server accepted the message.
    pub created_at_unix: i64,
    /// Unix timestamp (seconds) after which the server hard-deletes it.
    pub expires_at_unix: i64,
}

/// Response for `GET /e2ee/groups/{group_id}/mailbox`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct E2eeMailboxResponse {
    /// Pending device deliveries in ascending message-ID order.
    #[serde(deserialize_with = "deserialize_mailbox_messages")]
    pub messages: Vec<E2eeMailboxMessage>,
    /// Cursor for the next page, or `None` when this page is empty.
    pub next_after_message_id: Option<String>,
}

/// Successful-decryption acknowledgments for one active device.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AckE2eeMessagesRequest {
    /// Active device that locally authenticated and decrypted the messages.
    pub device_id: String,
    /// Message IDs to acknowledge. Duplicates are rejected by the server.
    #[serde(deserialize_with = "deserialize_message_ack_ids")]
    pub message_ids: Vec<String>,
}

/// Result of a batched per-device mailbox acknowledgment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AckE2eeMessagesResponse {
    /// Delivery rows newly or previously acknowledged by this device.
    pub acknowledged_count: u32,
    /// Messages hard-deleted because every snapshotted device acknowledged.
    pub deleted_count: u32,
}

/// Query parameters for a device's pending commit mailbox.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct E2eeCommitMailboxQuery {
    /// Active device whose snapshotted commit deliveries should be returned.
    pub device_id: String,
    /// Exclusive epoch cursor from the preceding page.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after_epoch: Option<u64>,
    /// Requested record count. The server caps this at 50.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u16>,
}

/// One opaque MLS commit pending delivery to a device.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct E2eeCommitMailboxEntry {
    /// Epoch reached by this commit.
    pub epoch: u64,
    /// Epoch from which this commit advances.
    pub prior_epoch: u64,
    /// Device that authored the commit.
    pub committer_device_id: String,
    /// Serialized MLS commit, opaque to the server.
    #[serde(deserialize_with = "deserialize_commit_blob")]
    pub commit_blob: Vec<u8>,
    /// Serialized Welcome, returned only to its exact target device.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_optional_welcome_blob"
    )]
    pub welcome_blob: Option<Vec<u8>>,
    /// Unix timestamp (seconds) when the server accepted the commit.
    pub created_at_unix: i64,
    /// Unix timestamp (seconds) after which the server hard-deletes it.
    pub expires_at_unix: i64,
}

/// Response for `GET /e2ee/groups/{group_id}/commits`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct E2eeCommitMailboxResponse {
    /// Pending device deliveries in ascending epoch order.
    #[serde(deserialize_with = "deserialize_commit_mailbox_entries")]
    pub commits: Vec<E2eeCommitMailboxEntry>,
    /// Cursor for the next page, or `None` when this page is empty.
    pub next_after_epoch: Option<u64>,
}

/// Successful-processing acknowledgments for commit epochs on one device.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AckE2eeCommitsRequest {
    /// Active device that authenticated and processed the commits.
    pub device_id: String,
    /// Positive commit epochs to acknowledge.
    #[serde(deserialize_with = "deserialize_commit_ack_epochs")]
    pub epochs: Vec<u64>,
}

/// Result of a batched per-device commit acknowledgment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AckE2eeCommitsResponse {
    /// Delivery rows newly or previously acknowledged by this device.
    pub acknowledged_count: u32,
    /// Commits hard-deleted because every snapshotted device acknowledged.
    pub deleted_count: u32,
}

// ---------------------------------------------------------------------------
// Gateway event data types
// ---------------------------------------------------------------------------

/// Event data for `mls_message` gateway event (scope: channel, schema_version: 1).
///
/// Notifies clients of a new MLS application message in a group.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MlsMessageEvent {
    /// The group ID (ULID string).
    pub group_id: String,
    /// The conversation ID (ULID string).
    pub conversation_id: String,
    /// The message ID (ULID string).
    pub message_id: String,
    /// The epoch in which the message was sent.
    pub epoch: u64,
    /// The ciphersuite ID (routing hint only).
    pub suite_id: u16,
    /// The sending device ID (ULID string, routing hint only).
    pub sender_device_id: String,
    /// Unix timestamp (seconds) when the message was received.
    pub created_at_unix: i64,
}

/// Event data for `mls_commit` gateway event (scope: channel, schema_version: 1).
///
/// Notifies clients of a new MLS commit (membership or state change).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MlsCommitEvent {
    /// The group ID (ULID string).
    pub group_id: String,
    /// The conversation ID (ULID string).
    pub conversation_id: String,
    /// The new epoch after the commit.
    pub epoch: u64,
    /// The prior epoch.
    pub prior_epoch: u64,
    /// The committing device ID (ULID string).
    pub committer_device_id: String,
    /// Unix timestamp (seconds) when the commit was processed.
    pub created_at_unix: i64,
}

/// Event data for `mls_welcome` gateway event (scope: channel, schema_version: 1).
///
/// Notifies a client that they have been added to an MLS group and have
/// a Welcome message to process.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MlsWelcomeEvent {
    /// The group ID (ULID string).
    pub group_id: String,
    /// The conversation ID (ULID string).
    pub conversation_id: String,
    /// The epoch of the group when the Welcome was issued.
    pub epoch: u64,
    /// The ciphersuite ID.
    pub suite_id: u16,
    /// Unix timestamp (seconds) when the Welcome was received.
    pub created_at_unix: i64,
}

/// Event data for `mls_proposal` gateway event (scope: channel, schema_version: 1).
///
/// Notifies clients of a pending MLS proposal in a group.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MlsProposalEvent {
    /// The group ID (ULID string).
    pub group_id: String,
    /// The conversation ID (ULID string).
    pub conversation_id: String,
    /// The epoch in which the proposal was made.
    pub epoch: u64,
    /// The proposing device ID (ULID string).
    pub proposer_device_id: String,
    /// Unix timestamp (seconds) when the proposal was received.
    pub created_at_unix: i64,
}

/// Event data for `device_list_update` gateway event (scope: user, schema_version: 1).
///
/// Notifies a user's connected devices that a peer's device list has changed
/// (device added or removed). Clients should re-verify device certificates.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeviceListUpdateEvent {
    /// The user whose device list changed (ULID string).
    pub user_id: String,
    /// The device count after the change.
    pub device_count: u32,
    /// Unix timestamp (seconds) when the change was recorded.
    pub created_at_unix: i64,
}

/// Event data for `keypackage_low` gateway event (scope: user, schema_version: 1).
///
/// Notifies a client that its KeyPackage pool has dropped below the
/// replenishment water mark. The client should upload new KeyPackages.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KeyPackageLowEvent {
    /// The device whose pool is low (ULID string).
    pub device_id: String,
    /// The current pool size.
    pub remaining_count: u32,
    /// The water mark threshold.
    pub water_mark: u32,
    /// Unix timestamp (seconds) when the alert was generated.
    pub created_at_unix: i64,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conversation_provisioning_contract_is_strict_and_bounded() {
        let request = CreateMlsConversationRequest {
            conversation_id: String::from("01ARZ3NDEKTSV4RRFFQ69G5FAV"),
            peer_user_id: String::from("01ARZ3NDEKTSV4RRFFQ69G5FAW"),
            group_id: String::from("01ARZ3NDEKTSV4RRFFQ69G5FAX"),
            suite_id: 3,
            committer_device_id: String::from("01ARZ3NDEKTSV4RRFFQ69G5FAY"),
            welcome_device_id: String::from("01ARZ3NDEKTSV4RRFFQ69G5FAZ"),
            commit_blob: vec![1; 64],
            welcome_blob: vec![2; 64],
            group_info_blob: vec![3; 64],
        };
        let mut value = serde_json::to_value(&request).unwrap();
        value["extra"] = serde_json::json!(true);
        assert!(serde_json::from_value::<CreateMlsConversationRequest>(value).is_err());

        let mut oversized = request;
        oversized.welcome_blob = vec![0; MAX_WELCOME_BYTES + 1];
        let encoded = serde_json::to_vec(&oversized).unwrap();
        assert!(serde_json::from_slice::<CreateMlsConversationRequest>(&encoded).is_err());
    }

    #[test]
    fn provision_response_rejects_plaintext_mode() {
        let json = r#"{"conversation_id":"c","group_id":"g","crypto":"plaintext","epoch":1,"suite_id":3,"provisioned_at_unix":1}"#;
        assert!(serde_json::from_str::<MlsConversationProvisionResponse>(json).is_err());
    }

    // -- Device certificate DTOs --

    #[test]
    fn publish_device_certificate_request_deny_unknown_fields() {
        let json = format!(
            r#"{{"device_signature_pubkey":{},"root_key_signature":{},"root_key_pub":{},"extra":1}}"#,
            serde_json::to_string(&vec![0xAB; ED25519_PUBLIC_KEY_BYTES]).unwrap(),
            serde_json::to_string(&vec![0xCD; ED25519_SIGNATURE_BYTES]).unwrap(),
            serde_json::to_string(&vec![0xEF; ED25519_PUBLIC_KEY_BYTES]).unwrap(),
        );
        let result: Result<PublishDeviceCertificateRequest, _> = serde_json::from_str(&json);
        assert!(result.is_err());
    }

    #[test]
    fn publish_device_certificate_request_round_trip() {
        let req = PublishDeviceCertificateRequest {
            device_signature_pubkey: vec![0xAB; 32],
            root_key_signature: vec![0xCD; 64],
            root_key_pub: vec![0xEF; 32],
        };
        let json = serde_json::to_string(&req).unwrap();
        let parsed: PublishDeviceCertificateRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, req);
    }

    #[test]
    fn remove_device_response_round_trip_and_deny_unknown_fields() {
        let response = RemoveDeviceResponse {
            device_id: String::from("01ARZ3NDEKTSV4RRFFQ69G5FAV"),
            tombstoned_at_unix: 1_700_000_000,
            deleted_keypackage_count: 3,
        };
        let json = serde_json::to_string(&response).unwrap();
        let parsed: RemoveDeviceResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, response);

        let invalid = json.trim_end_matches('}').to_string() + ",\"extra\":true}";
        assert!(serde_json::from_str::<RemoveDeviceResponse>(&invalid).is_err());
    }

    #[test]
    fn device_list_response_deny_unknown_fields() {
        let json = r#"{"user_id":"01ARZ3NDEKTSV4RRFFQ69G5FAV","devices":[],"extra":1}"#;
        let result: Result<DeviceListResponse, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn device_info_with_optional_tombstone_round_trip() {
        let info = DeviceInfo {
            device_id: String::from("01ARZ3NDEKTSV4RRFFQ69G5FAV"),
            device_signature_pubkey: vec![0xAB; 32],
            root_key_signature: vec![0xCD; 64],
            root_key_pub: vec![0xEF; 32],
            created_at_unix: 1_700_000_000,
            tombstoned_at_unix: None,
        };
        let json = serde_json::to_string(&info).unwrap();
        let parsed: DeviceInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, info);
        assert!(parsed.tombstoned_at_unix.is_none());
    }

    #[test]
    fn root_rotation_contract_round_trips_and_rejects_unknown_fields() {
        let request = RotateRootIdentityRequest {
            protocol_version: ROOT_IDENTITY_ROTATION_PROTOCOL_VERSION,
            expected_rotation_sequence: 0,
            device_id: String::from("01ARZ3NDEKTSV4RRFFQ69G5FAV"),
            new_root_key_pub: vec![0x11; ED25519_PUBLIC_KEY_BYTES],
            previous_root_signature: vec![0x22; ED25519_SIGNATURE_BYTES],
            new_root_signature: vec![0x33; ED25519_SIGNATURE_BYTES],
            new_device_signature_pubkey: vec![0x44; ED25519_PUBLIC_KEY_BYTES],
            new_device_root_signature: vec![0x55; ED25519_SIGNATURE_BYTES],
        };
        let json = serde_json::to_string(&request).unwrap();
        assert_eq!(
            serde_json::from_str::<RotateRootIdentityRequest>(&json).unwrap(),
            request
        );
        let invalid = json.trim_end_matches('}').to_string() + ",\"extra\":true}";
        assert!(serde_json::from_str::<RotateRootIdentityRequest>(&invalid).is_err());
    }

    #[test]
    fn root_identity_directory_caps_rotation_history() {
        let entry = RootIdentityRotationEntry {
            sequence: 1,
            previous_root_key_pub: vec![0x11; ED25519_PUBLIC_KEY_BYTES],
            new_root_key_pub: vec![0x22; ED25519_PUBLIC_KEY_BYTES],
            previous_root_signature: vec![0x33; ED25519_SIGNATURE_BYTES],
            new_root_signature: vec![0x44; ED25519_SIGNATURE_BYTES],
            rotating_device_id: String::from("01ARZ3NDEKTSV4RRFFQ69G5FAV"),
            rotated_at_unix: 1_700_000_000,
        };
        let oversized = RootIdentityDirectoryResponse {
            protocol_version: ROOT_IDENTITY_ROTATION_PROTOCOL_VERSION,
            user_id: String::from("01ARZ3NDEKTSV4RRFFQ69G5FAW"),
            current_root_key_pub: vec![0x22; ED25519_PUBLIC_KEY_BYTES],
            rotation_sequence: u64::try_from(MAX_ROOT_IDENTITY_ROTATIONS + 1).unwrap(),
            rotations: vec![entry; MAX_ROOT_IDENTITY_ROTATIONS + 1],
        };
        let json = serde_json::to_string(&oversized).unwrap();
        assert!(serde_json::from_str::<RootIdentityDirectoryResponse>(&json).is_err());
    }

    // -- KeyPackage DTOs --

    #[test]
    fn upload_key_packages_request_round_trip() {
        let req = UploadKeyPackagesRequest {
            device_id: String::from("01ARZ3NDEKTSV4RRFFQ69G5FAV"),
            key_packages: vec![
                KeyPackageEntry {
                    key_package_blob: vec![0x01; 128],
                    is_last_resort: false,
                },
                KeyPackageEntry {
                    key_package_blob: vec![0x02; 128],
                    is_last_resort: true,
                },
            ],
        };
        let json = serde_json::to_string(&req).unwrap();
        let parsed: UploadKeyPackagesRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, req);
    }

    #[test]
    fn upload_key_packages_request_deny_unknown_fields() {
        let json = r#"{"device_id":"01ARZ3NDEKTSV4RRFFQ69G5FAV","key_packages":[],"extra":1}"#;
        let result: Result<UploadKeyPackagesRequest, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn device_certificate_request_rejects_wrong_crypto_lengths_during_parse() {
        let json = format!(
            r#"{{"device_signature_pubkey":[1],"root_key_signature":{},"root_key_pub":{}}}"#,
            serde_json::to_string(&vec![0xCD; ED25519_SIGNATURE_BYTES]).unwrap(),
            serde_json::to_string(&vec![0xEF; ED25519_PUBLIC_KEY_BYTES]).unwrap(),
        );
        let result: Result<PublishDeviceCertificateRequest, _> = serde_json::from_str(&json);
        assert!(result.is_err());
    }

    #[test]
    fn key_package_upload_rejects_empty_and_oversized_values_during_parse() {
        let empty = r#"{"device_id":"01ARZ3NDEKTSV4RRFFQ69G5FAV","key_packages":[]}"#;
        assert!(serde_json::from_str::<UploadKeyPackagesRequest>(empty).is_err());

        let oversized = UploadKeyPackagesRequest {
            device_id: String::from("01ARZ3NDEKTSV4RRFFQ69G5FAV"),
            key_packages: vec![KeyPackageEntry {
                key_package_blob: vec![0xAA; MAX_KEYPACKAGE_BYTES + 1],
                is_last_resort: false,
            }],
        };
        let json = serde_json::to_string(&oversized).unwrap();
        assert!(serde_json::from_str::<UploadKeyPackagesRequest>(&json).is_err());
    }

    #[test]
    fn claim_key_package_request_with_optional_device() {
        let req = ClaimKeyPackageRequest {
            target_user_id: String::from("01ARZ3NDEKTSV4RRFFQ69G5FAV"),
            target_device_id: None,
        };
        let json = serde_json::to_string(&req).unwrap();
        let parsed: ClaimKeyPackageRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, req);
        assert!(parsed.target_device_id.is_none());
    }

    #[test]
    fn claim_key_package_request_deny_unknown_fields() {
        let json =
            r#"{"target_user_id":"01ARZ3NDEKTSV4RRFFQ69G5FAV","target_device_id":null,"extra":1}"#;
        let result: Result<ClaimKeyPackageRequest, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    // -- Group and message transport DTOs --

    #[test]
    fn group_info_response_round_trip() {
        let resp = GroupInfoResponse {
            group_id: String::from("01ARZ3NDEKTSV4RRFFQ69G5FAV"),
            epoch: 5,
            suite_id: 0x0003,
            group_info_blob: vec![0xFF; 256],
        };
        let json = serde_json::to_string(&resp).unwrap();
        let parsed: GroupInfoResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, resp);
    }

    #[test]
    fn post_commit_request_with_optional_welcome_round_trip() {
        let req = PostCommitRequest {
            epoch: 3,
            prior_epoch: 2,
            committer_device_id: String::from("01ARZ3NDEKTSV4RRFFQ69G5FAV"),
            commit_blob: vec![0x01; 512],
            welcome_blob: Some(vec![0x02; 256]),
            welcome_device_id: Some(String::from("01ARZ3NDEKTSV4RRFFQ69G5FAW")),
            group_info_blob: None,
        };
        let json = serde_json::to_string(&req).unwrap();
        let parsed: PostCommitRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, req);
    }

    #[test]
    fn post_commit_request_deny_unknown_fields() {
        let json = r#"{"epoch":3,"prior_epoch":2,"committer_device_id":"01ARZ3NDEKTSV4RRFFQ69G5FAV","commit_blob":[1],"extra":1}"#;
        let result: Result<PostCommitRequest, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn post_message_request_round_trip() {
        let req = PostMessageRequest {
            epoch: 1,
            suite_id: 0x0003,
            sender_device_id: String::from("01ARZ3NDEKTSV4RRFFQ69G5FAV"),
            message_blob: vec![0xAA; 1024],
        };
        let json = serde_json::to_string(&req).unwrap();
        let parsed: PostMessageRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, req);
    }

    #[test]
    fn post_message_request_deny_unknown_fields() {
        let json = r#"{"epoch":1,"suite_id":3,"sender_device_id":"01ARZ3NDEKTSV4RRFFQ69G5FAV","message_blob":[170],"extra":1}"#;
        let result: Result<PostMessageRequest, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn mailbox_and_ack_contracts_round_trip_with_strict_fields() {
        let message = E2eeMailboxMessage {
            message_id: String::from("01ARZ3NDEKTSV4RRFFQ69G5FAV"),
            crypto: String::from("mls_v1"),
            epoch: 4,
            suite_id: 3,
            sender_device_id: String::from("01ARZ3NDEKTSV4RRFFQ69G5FAVD"),
            message_blob: vec![0xA5; 512],
            created_at_unix: 1_700_000_000,
            expires_at_unix: 1_700_003_600,
        };
        let response = E2eeMailboxResponse {
            messages: vec![message.clone()],
            next_after_message_id: Some(message.message_id.clone()),
        };
        let json = serde_json::to_string(&response).unwrap();
        let parsed: E2eeMailboxResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, response);

        let ack = AckE2eeMessagesRequest {
            device_id: message.sender_device_id,
            message_ids: vec![message.message_id],
        };
        let json = serde_json::to_string(&ack).unwrap();
        let parsed: AckE2eeMessagesRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, ack);

        let unknown = r#"{"device_id":"d","message_ids":["m"],"extra":1}"#;
        assert!(serde_json::from_str::<AckE2eeMessagesRequest>(unknown).is_err());
    }

    #[test]
    fn mailbox_and_ack_contracts_reject_unbounded_collections() {
        let oversized_ack = AckE2eeMessagesRequest {
            device_id: String::from("device"),
            message_ids: vec![String::from("message"); MAX_E2EE_MESSAGE_ACK_BATCH_SIZE + 1],
        };
        let json = serde_json::to_string(&oversized_ack).unwrap();
        assert!(serde_json::from_str::<AckE2eeMessagesRequest>(&json).is_err());

        let empty_ack = AckE2eeMessagesRequest {
            device_id: String::from("device"),
            message_ids: Vec::new(),
        };
        let json = serde_json::to_string(&empty_ack).unwrap();
        assert!(serde_json::from_str::<AckE2eeMessagesRequest>(&json).is_err());

        let large_message = E2eeMailboxMessage {
            message_id: String::from("message"),
            crypto: String::from("mls_v1"),
            epoch: 1,
            suite_id: 3,
            sender_device_id: String::from("device"),
            message_blob: vec![0xA5; 16_384],
            created_at_unix: 1,
            expires_at_unix: 2,
        };
        let oversized_mailbox = E2eeMailboxResponse {
            messages: vec![large_message; 17],
            next_after_message_id: None,
        };
        let json = serde_json::to_string(&oversized_mailbox).unwrap();
        assert!(serde_json::from_str::<E2eeMailboxResponse>(&json).is_err());

        let invalid_routing_mode = E2eeMailboxResponse {
            messages: vec![E2eeMailboxMessage {
                message_id: String::from("message"),
                crypto: String::from("plaintext"),
                epoch: 1,
                suite_id: 3,
                sender_device_id: String::from("device"),
                message_blob: vec![0xA5; 512],
                created_at_unix: 1,
                expires_at_unix: 2,
            }],
            next_after_message_id: None,
        };
        let json = serde_json::to_string(&invalid_routing_mode).unwrap();
        assert!(serde_json::from_str::<E2eeMailboxResponse>(&json).is_err());
    }

    #[test]
    fn commit_mailbox_and_ack_contracts_are_strict_and_bounded() {
        let entry = E2eeCommitMailboxEntry {
            epoch: 2,
            prior_epoch: 1,
            committer_device_id: String::from("01ARZ3NDEKTSV4RRFFQ69G5FAV"),
            commit_blob: vec![0xA1; 512],
            welcome_blob: Some(vec![0xA2; 256]),
            created_at_unix: 1,
            expires_at_unix: 2,
        };
        let response = E2eeCommitMailboxResponse {
            commits: vec![entry],
            next_after_epoch: Some(2),
        };
        let json = serde_json::to_string(&response).unwrap();
        assert_eq!(
            serde_json::from_str::<E2eeCommitMailboxResponse>(&json).unwrap(),
            response
        );

        let ack = AckE2eeCommitsRequest {
            device_id: String::from("01ARZ3NDEKTSV4RRFFQ69G5FAW"),
            epochs: vec![1, 2],
        };
        let json = serde_json::to_string(&ack).unwrap();
        assert_eq!(
            serde_json::from_str::<AckE2eeCommitsRequest>(&json).unwrap(),
            ack
        );

        let invalid_ack = AckE2eeCommitsRequest {
            device_id: String::from("device"),
            epochs: vec![0],
        };
        let json = serde_json::to_string(&invalid_ack).unwrap();
        assert!(serde_json::from_str::<AckE2eeCommitsRequest>(&json).is_err());

        let oversized_entry = E2eeCommitMailboxEntry {
            epoch: 1,
            prior_epoch: 0,
            committer_device_id: String::from("device"),
            commit_blob: vec![0xA1; MAX_COMMIT_BYTES],
            welcome_blob: None,
            created_at_unix: 1,
            expires_at_unix: 2,
        };
        let oversized = E2eeCommitMailboxResponse {
            commits: vec![oversized_entry; 5],
            next_after_epoch: None,
        };
        let json = serde_json::to_string(&oversized).unwrap();
        assert!(serde_json::from_str::<E2eeCommitMailboxResponse>(&json).is_err());

        let unknown = r#"{"device_id":"d","epochs":[1],"extra":true}"#;
        assert!(serde_json::from_str::<AckE2eeCommitsRequest>(unknown).is_err());
    }

    #[test]
    fn device_list_and_all_mls_blobs_enforce_parse_limits() {
        let device = DeviceInfo {
            device_id: String::from("01ARZ3NDEKTSV4RRFFQ69G5FAV"),
            device_signature_pubkey: vec![0xAB; ED25519_PUBLIC_KEY_BYTES],
            root_key_signature: vec![0xCD; ED25519_SIGNATURE_BYTES],
            root_key_pub: vec![0xEF; ED25519_PUBLIC_KEY_BYTES],
            created_at_unix: 1_700_000_000,
            tombstoned_at_unix: None,
        };
        let oversized_list = DeviceListResponse {
            user_id: String::from("01ARZ3NDEKTSV4RRFFQ69G5FAV"),
            devices: vec![device; MAX_DEVICE_LIST_SIZE + 1],
        };
        let json = serde_json::to_string(&oversized_list).unwrap();
        assert!(serde_json::from_str::<DeviceListResponse>(&json).is_err());

        let oversized_group_info = GroupInfoResponse {
            group_id: String::from("01ARZ3NDEKTSV4RRFFQ69G5FAV"),
            epoch: 1,
            suite_id: 3,
            group_info_blob: vec![0x01; MAX_GROUP_INFO_BYTES + 1],
        };
        let json = serde_json::to_string(&oversized_group_info).unwrap();
        assert!(serde_json::from_str::<GroupInfoResponse>(&json).is_err());

        let oversized_commit = PostCommitRequest {
            epoch: 2,
            prior_epoch: 1,
            committer_device_id: String::from("01ARZ3NDEKTSV4RRFFQ69G5FAV"),
            commit_blob: vec![0x01; MAX_COMMIT_BYTES + 1],
            welcome_blob: None,
            welcome_device_id: None,
            group_info_blob: None,
        };
        let json = serde_json::to_string(&oversized_commit).unwrap();
        assert!(serde_json::from_str::<PostCommitRequest>(&json).is_err());

        let oversized_welcome = PostCommitRequest {
            epoch: 2,
            prior_epoch: 1,
            committer_device_id: String::from("01ARZ3NDEKTSV4RRFFQ69G5FAV"),
            commit_blob: vec![0x01],
            welcome_blob: Some(vec![0x02; MAX_WELCOME_BYTES + 1]),
            welcome_device_id: Some(String::from("01ARZ3NDEKTSV4RRFFQ69G5FAW")),
            group_info_blob: None,
        };
        let json = serde_json::to_string(&oversized_welcome).unwrap();
        assert!(serde_json::from_str::<PostCommitRequest>(&json).is_err());

        let oversized_message = PostMessageRequest {
            epoch: 1,
            suite_id: 3,
            sender_device_id: String::from("01ARZ3NDEKTSV4RRFFQ69G5FAV"),
            message_blob: vec![0x03; MAX_MLS_MESSAGE_BYTES + 1],
        };
        let json = serde_json::to_string(&oversized_message).unwrap();
        assert!(serde_json::from_str::<PostMessageRequest>(&json).is_err());
    }

    // -- Gateway event data types --

    #[test]
    fn mls_message_event_round_trip() {
        let event = MlsMessageEvent {
            group_id: String::from("01ARZ3NDEKTSV4RRFFQ69G5FAV"),
            conversation_id: String::from("01ARZ3NDEKTSV4RRFFQ69G5FAVD"),
            message_id: String::from("01ARZ3NDEKTSV4RRFFQ69G5FAVE"),
            epoch: 1,
            suite_id: 0x0003,
            sender_device_id: String::from("01ARZ3NDEKTSV4RRFFQ69G5FAVF"),
            created_at_unix: 1_700_000_000,
        };
        let json = serde_json::to_string(&event).unwrap();
        let parsed: MlsMessageEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, event);
    }

    #[test]
    fn mls_message_event_deny_unknown_fields() {
        let json = r#"{"group_id":"g","conversation_id":"c","message_id":"m","epoch":1,"suite_id":3,"sender_device_id":"d","created_at_unix":1700000000,"extra":1}"#;
        let result: Result<MlsMessageEvent, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn mls_commit_event_round_trip() {
        let event = MlsCommitEvent {
            group_id: String::from("01ARZ3NDEKTSV4RRFFQ69G5FAV"),
            conversation_id: String::from("01ARZ3NDEKTSV4RRFFQ69G5FAVD"),
            epoch: 2,
            prior_epoch: 1,
            committer_device_id: String::from("01ARZ3NDEKTSV4RRFFQ69G5FAVF"),
            created_at_unix: 1_700_000_000,
        };
        let json = serde_json::to_string(&event).unwrap();
        let parsed: MlsCommitEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, event);
    }

    #[test]
    fn mls_welcome_event_round_trip() {
        let event = MlsWelcomeEvent {
            group_id: String::from("01ARZ3NDEKTSV4RRFFQ69G5FAV"),
            conversation_id: String::from("01ARZ3NDEKTSV4RRFFQ69G5FAVD"),
            epoch: 1,
            suite_id: 0x0003,
            created_at_unix: 1_700_000_000,
        };
        let json = serde_json::to_string(&event).unwrap();
        let parsed: MlsWelcomeEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, event);
    }

    #[test]
    fn mls_proposal_event_round_trip() {
        let event = MlsProposalEvent {
            group_id: String::from("01ARZ3NDEKTSV4RRFFQ69G5FAV"),
            conversation_id: String::from("01ARZ3NDEKTSV4RRFFQ69G5FAVD"),
            epoch: 1,
            proposer_device_id: String::from("01ARZ3NDEKTSV4RRFFQ69G5FAVF"),
            created_at_unix: 1_700_000_000,
        };
        let json = serde_json::to_string(&event).unwrap();
        let parsed: MlsProposalEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, event);
    }

    #[test]
    fn device_list_update_event_round_trip() {
        let event = DeviceListUpdateEvent {
            user_id: String::from("01ARZ3NDEKTSV4RRFFQ69G5FAV"),
            device_count: 3,
            created_at_unix: 1_700_000_000,
        };
        let json = serde_json::to_string(&event).unwrap();
        let parsed: DeviceListUpdateEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, event);
    }

    #[test]
    fn device_list_update_event_deny_unknown_fields() {
        let json = r#"{"user_id":"u","device_count":3,"created_at_unix":1700000000,"extra":1}"#;
        let result: Result<DeviceListUpdateEvent, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn keypackage_low_event_round_trip() {
        let event = KeyPackageLowEvent {
            device_id: String::from("01ARZ3NDEKTSV4RRFFQ69G5FAV"),
            remaining_count: 2,
            water_mark: 5,
            created_at_unix: 1_700_000_000,
        };
        let json = serde_json::to_string(&event).unwrap();
        let parsed: KeyPackageLowEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, event);
    }

    #[test]
    fn keypackage_low_event_deny_unknown_fields() {
        let json = r#"{"device_id":"d","remaining_count":2,"water_mark":5,"created_at_unix":1700000000,"extra":1}"#;
        let result: Result<KeyPackageLowEvent, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    // -- Size bound constants --

    #[test]
    fn size_bounds_are_sensible() {
        // These are compile-time constants; just reference them to ensure
        // they don't get optimized away and are sensible values.
        const _: usize = MAX_KEYPACKAGE_BYTES;
        const _: usize = MAX_MLS_MESSAGE_BYTES;
        const _: usize = MAX_COMMIT_BYTES;
        const _: usize = MAX_WELCOME_BYTES;
        const _: usize = MAX_PROPOSAL_BYTES;
        const _: usize = MAX_GROUP_INFO_BYTES;
        const _: usize = MAX_KEYPACKAGE_POOL_SIZE;
        const _: usize = MAX_DEVICE_LIST_SIZE;
        const _: usize = MAX_ROOT_IDENTITY_ROTATIONS;
        const _: usize = MAX_E2EE_MAILBOX_PAGE_SIZE;
        const _: usize = MAX_E2EE_MAILBOX_PAGE_BLOB_BYTES;
        const _: usize = MAX_E2EE_MESSAGE_ACK_BATCH_SIZE;
        // Compile-time invariant: pool count must be smaller than per-blob cap.
        const _: () = const { assert!(MAX_KEYPACKAGE_POOL_SIZE < MAX_KEYPACKAGE_BYTES) };
    }
}
