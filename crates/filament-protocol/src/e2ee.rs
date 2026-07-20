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
    let value = Vec::<u8>::deserialize(deserializer)?;
    if value.is_empty() || value.len() > MAX_KEYPACKAGE_BYTES {
        return Err(de::Error::invalid_length(
            value.len(),
            &"a non-empty KeyPackage no larger than 4096 bytes",
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

// ---------------------------------------------------------------------------
// KeyPackage endpoints
// ---------------------------------------------------------------------------

/// Request body for `POST /e2ee/keypackages` — upload KeyPackage pool.
///
/// KeyPackages are MLS's prekey analog. Each device uploads a pool of
/// single-use KeyPackages plus one last-resort package. The server stores
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
    /// Whether this is a last-resort KeyPackage (reuse semantics).
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
    pub commit_blob: Vec<u8>,
    /// Optional Welcome message for new members (opaque blob).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub welcome_blob: Option<Vec<u8>>,
    /// Optional updated GroupInfo blob for joins/recovery.
    #[serde(default, skip_serializing_if = "Option::is_none")]
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
/// The server stores the opaque `PrivateMessage` blob plus a minimal routing
/// envelope. It never parses MLS interiors. Size-bucket padding is verified.
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
        // Compile-time invariant: pool count must be smaller than per-blob cap.
        const _: () = const { assert!(MAX_KEYPACKAGE_POOL_SIZE < MAX_KEYPACKAGE_BYTES) };
    }
}
