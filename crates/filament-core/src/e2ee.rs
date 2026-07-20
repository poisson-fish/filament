#![forbid(unsafe_code)]

//! E2EE domain types for MLS-based end-to-end encryption.
//!
//! These types are the domain-level abstractions used across the protocol,
//! server, and client crates. They enforce invariants at construction time
//! and are the canonical types for all E2EE operations.
//!
//! See [`docs/adr/0001-e2ee-mls-openmls.md`] for the protocol-stack decision
//! and [`plans/PLAN_E2EE.md`] for the full design specification.

use serde::{Deserialize, Serialize};
use ulid::Ulid;

use crate::DomainError;

// ---------------------------------------------------------------------------
// DeviceId
// ---------------------------------------------------------------------------

/// Unique identifier for a device within a user's device set.
///
/// Like [`crate::UserId`], this is a ULID newtype. Device IDs are generated
/// on-device and embedded in device certificates signed by the user's root
/// identity key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DeviceId(Ulid);

impl DeviceId {
    #[must_use]
    pub fn new() -> Self {
        Self(Ulid::new())
    }
}

impl Default for DeviceId {
    fn default() -> Self {
        Self::new()
    }
}

impl TryFrom<String> for DeviceId {
    type Error = DomainError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        let parsed = Ulid::from_string(&value).map_err(|_| DomainError::InvalidDeviceId)?;
        Ok(Self(parsed))
    }
}

impl core::fmt::Display for DeviceId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Serialize for DeviceId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.0.to_string())
    }
}

impl<'de> Deserialize<'de> for DeviceId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ulid::from_string(&s)
            .map(Self)
            .map_err(serde::de::Error::custom)
    }
}

// ---------------------------------------------------------------------------
// GroupId
// ---------------------------------------------------------------------------

/// MLS group identifier.
///
/// Identifies an MLS group across the protocol. The server uses this for
/// routing and Delivery Service ordering; clients use it to look up local
/// group state. The internal representation is a ULID for collision resistance
/// and sortability, but the wire format is a 26-character ULID string.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GroupId(Ulid);

impl GroupId {
    #[must_use]
    pub fn new() -> Self {
        Self(Ulid::new())
    }
}

impl Default for GroupId {
    fn default() -> Self {
        Self::new()
    }
}

impl TryFrom<String> for GroupId {
    type Error = DomainError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        let parsed = Ulid::from_string(&value).map_err(|_| DomainError::InvalidGroupId)?;
        Ok(Self(parsed))
    }
}

impl core::fmt::Display for GroupId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Serialize for GroupId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.0.to_string())
    }
}

impl<'de> Deserialize<'de> for GroupId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ulid::from_string(&s)
            .map(Self)
            .map_err(serde::de::Error::custom)
    }
}

// ---------------------------------------------------------------------------
// CiphersuiteId
// ---------------------------------------------------------------------------

/// MLS ciphersuite identifier (RFC 9420 §17.1).
///
/// The baseline ciphersuite is 0x0003
/// (`MLS_128_DHKEMX25519_CHACHA20POLY1305_SHA256_Ed25519`).
/// Ciphersuite agility is mandatory in all wire formats and stored state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CiphersuiteId(u16);

impl CiphersuiteId {
    /// `MLS_128_DHKEMX25519_CHACHA20POLY1305_SHA256_Ed25519` (baseline).
    pub const MLS_128_DHKEMX25519_CHACHA20POLY1305_SHA256_ED25519: Self = Self(0x0003);

    /// `MLS_128_DHKEMP256_AES128GCM_SHA256_P256`.
    pub const MLS_128_DHKEMP256_AES128GCM_SHA256_P256: Self = Self(0x0001);

    /// `MLS_128_DHKEMX25519_AES128GCM_SHA256_Ed25519`.
    pub const MLS_128_DHKEMX25519_AES128GCM_SHA256_ED25519: Self = Self(0x0002);

    /// `MLS_256_DHKEMP384_AES256GCM_SHA384_P384`.
    pub const MLS_256_DHKEMP384_AES256GCM_SHA384_P384: Self = Self(0x0004);

    /// `MLS_256_DHKEMX448_CHACHA20POLY1305_SHA512_Ed448`.
    pub const MLS_256_DHKEMX448_CHACHA20POLY1305_SHA512_ED448: Self = Self(0x0005);

    /// `MLS_256_DHKEMP521_AES256GCM_SHA512_P521`.
    pub const MLS_256_DHKEMP521_AES256GCM_SHA512_P521: Self = Self(0x0006);

    /// `MLS_256_DHKEMX448_AES256GCM_SHA512_Ed448`.
    pub const MLS_256_DHKEMX448_AES256GCM_SHA512_ED448: Self = Self(0x0007);

    /// Returns the raw u16 value.
    #[must_use]
    pub const fn as_u16(self) -> u16 {
        self.0
    }

    /// Returns the baseline ciphersuite (0x0003).
    #[must_use]
    pub const fn baseline() -> Self {
        Self::MLS_128_DHKEMX25519_CHACHA20POLY1305_SHA256_ED25519
    }

    /// Returns `true` if the ciphersuite ID is a known RFC 9420 suite.
    #[must_use]
    pub const fn is_known(self) -> bool {
        matches!(
            self,
            Self::MLS_128_DHKEMP256_AES128GCM_SHA256_P256
                | Self::MLS_128_DHKEMX25519_AES128GCM_SHA256_ED25519
                | Self::MLS_128_DHKEMX25519_CHACHA20POLY1305_SHA256_ED25519
                | Self::MLS_256_DHKEMP384_AES256GCM_SHA384_P384
                | Self::MLS_256_DHKEMX448_CHACHA20POLY1305_SHA512_ED448
                | Self::MLS_256_DHKEMP521_AES256GCM_SHA512_P521
                | Self::MLS_256_DHKEMX448_AES256GCM_SHA512_ED448
        )
    }
}

impl TryFrom<u16> for CiphersuiteId {
    type Error = DomainError;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        let suite = Self(value);
        if suite.is_known() {
            Ok(suite)
        } else {
            Err(DomainError::InvalidCiphersuiteId)
        }
    }
}

impl core::fmt::Display for CiphersuiteId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "0x{:04X}", self.0)
    }
}

// ---------------------------------------------------------------------------
// EpochTag
// ---------------------------------------------------------------------------

/// MLS epoch number.
///
/// Epochs advance with every committed state change (add, remove, update).
/// The server uses epoch tags for Delivery Service ordering (monotonicity
/// enforcement); clients use them for state synchronization and desync
/// detection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub struct EpochTag(u64);

impl EpochTag {
    /// Creates a new `EpochTag` from a raw u64 value.
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Returns the raw u64 epoch value.
    #[must_use]
    pub const fn as_u64(self) -> u64 {
        self.0
    }

    /// Returns the initial epoch (0).
    #[must_use]
    pub const fn initial() -> Self {
        Self(0)
    }

    /// Returns `true` if this epoch is strictly after `other`.
    #[must_use]
    pub const fn is_after(self, other: Self) -> bool {
        self.0 > other.0
    }
}

impl core::fmt::Display for EpochTag {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.0)
    }
}

// ---------------------------------------------------------------------------
// ConversationCrypto
// ---------------------------------------------------------------------------

/// Conversation-level crypto mode.
///
/// This is a property of the conversation/channel, immutable except via
/// explicit upgrade. No per-message crypto toggles, no mixed channels.
///
/// - `Plaintext`: server-readable, full moderation/search availability.
/// - `MlsV1`: end-to-end encrypted via MLS (RFC 9420), server stores opaque
///   blobs only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConversationCrypto {
    /// Server-readable conversation (no E2EE).
    Plaintext,
    /// End-to-end encrypted via MLS (RFC 9420) using the `OpenMLS` stack.
    MlsV1,
}

impl ConversationCrypto {
    /// Returns the wire string for this crypto mode.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Plaintext => "plaintext",
            Self::MlsV1 => "mls_v1",
        }
    }

    /// Returns `true` if this conversation is end-to-end encrypted.
    #[must_use]
    pub const fn is_encrypted(self) -> bool {
        matches!(self, Self::MlsV1)
    }
}

impl TryFrom<String> for ConversationCrypto {
    type Error = DomainError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.as_str() {
            "plaintext" => Ok(Self::Plaintext),
            "mls_v1" => Ok(Self::MlsV1),
            _ => Err(DomainError::InvalidConversationCrypto),
        }
    }
}

// ---------------------------------------------------------------------------
// DeviceCertificate
// ---------------------------------------------------------------------------

/// Maximum size in bytes for a serialized device signature public key.
pub const MAX_DEVICE_SIGNATURE_PUBKEY_BYTES: usize = 32;

/// Maximum size in bytes for a root-key signature over the certificate.
pub const MAX_ROOT_KEY_SIGNATURE_BYTES: usize = 64;

/// A device certificate, signed by the user's root identity key.
///
/// This binds a device to a user identity. The server stores and relays
/// certificates but never holds the root key, so it cannot mint valid
/// certificates — injected devices fail verification at every peer.
///
/// # Invariants
/// - `device_signature_pubkey` is exactly 32 bytes (Ed25519).
/// - `root_key_signature` is exactly 64 bytes (Ed25519).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeviceCertificate {
    /// The user this device belongs to (ULID string).
    pub user_id: String,
    /// The device being certified (ULID string).
    pub device_id: String,
    /// The device's MLS signature public key (raw bytes, base64-encoded on wire).
    pub device_signature_pubkey: Vec<u8>,
    /// The root identity key's signature over `(user_id, device_id, device_signature_pubkey)`.
    pub root_key_signature: Vec<u8>,
}

impl DeviceCertificate {
    /// Attempts to create a `DeviceCertificate` from raw fields, enforcing invariants.
    ///
    /// # Errors
    /// Returns [`DomainError`] if any field fails validation.
    pub fn try_new(
        user_id: String,
        device_id: String,
        device_signature_pubkey: Vec<u8>,
        root_key_signature: Vec<u8>,
    ) -> Result<Self, DomainError> {
        // Validate user_id and device_id are valid ULIDs.
        Ulid::from_string(&user_id).map_err(|_| DomainError::InvalidUserId)?;
        Ulid::from_string(&device_id).map_err(|_| DomainError::InvalidDeviceId)?;

        // Validate pubkey is non-empty and within bounds.
        if device_signature_pubkey.len() != MAX_DEVICE_SIGNATURE_PUBKEY_BYTES {
            return Err(DomainError::InvalidDeviceCertificate);
        }

        // Validate signature is non-empty and within bounds.
        if root_key_signature.len() != MAX_ROOT_KEY_SIGNATURE_BYTES {
            return Err(DomainError::InvalidDeviceCertificate);
        }

        Ok(Self {
            user_id,
            device_id,
            device_signature_pubkey,
            root_key_signature,
        })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::UserId;

    #[test]
    fn device_id_round_trip_and_parse_validation() {
        let id = DeviceId::new();
        let parsed = DeviceId::try_from(id.to_string()).unwrap();
        assert_eq!(id, parsed);

        let error = DeviceId::try_from(String::from("not-a-ulid")).unwrap_err();
        assert_eq!(error, DomainError::InvalidDeviceId);
    }

    #[test]
    fn device_id_default_generates_unique() {
        let a = DeviceId::default();
        let b = DeviceId::default();
        assert_ne!(a, b);
    }

    #[test]
    fn group_id_round_trip_and_parse_validation() {
        let id = GroupId::new();
        let parsed = GroupId::try_from(id.to_string()).unwrap();
        assert_eq!(id, parsed);

        let error = GroupId::try_from(String::from("not-a-ulid")).unwrap_err();
        assert_eq!(error, DomainError::InvalidGroupId);
    }

    #[test]
    fn group_id_default_generates_unique() {
        let a = GroupId::default();
        let b = GroupId::default();
        assert_ne!(a, b);
    }

    #[test]
    fn ciphersuite_id_baseline_is_0x0003() {
        let baseline = CiphersuiteId::baseline();
        assert_eq!(baseline.as_u16(), 0x0003);
        assert_eq!(
            baseline,
            CiphersuiteId::MLS_128_DHKEMX25519_CHACHA20POLY1305_SHA256_ED25519
        );
    }

    #[test]
    fn ciphersuite_id_known_suites_are_recognized() {
        for suite in [0x0001u16, 0x0002, 0x0003, 0x0004, 0x0005, 0x0006, 0x0007] {
            let id = CiphersuiteId::try_from(suite).unwrap();
            assert!(id.is_known());
        }
    }

    #[test]
    fn ciphersuite_id_rejects_unknown_values() {
        for bad in [0x0000u16, 0x0008, 0xFFFF] {
            assert_eq!(
                CiphersuiteId::try_from(bad).unwrap_err(),
                DomainError::InvalidCiphersuiteId
            );
        }
    }

    #[test]
    fn ciphersuite_id_display_format() {
        let baseline = CiphersuiteId::baseline();
        assert_eq!(format!("{baseline}"), "0x0003");
    }

    #[test]
    fn epoch_tag_initial_and_advance() {
        let e0 = EpochTag::initial();
        assert_eq!(e0.as_u64(), 0);

        let e1 = EpochTag::new(1);
        assert!(e1.is_after(e0));
        assert!(!e0.is_after(e1));
    }

    #[test]
    fn epoch_tag_default_is_initial() {
        assert_eq!(EpochTag::default(), EpochTag::initial());
    }

    #[test]
    fn epoch_tag_display() {
        assert_eq!(format!("{}", EpochTag::new(42)), "42");
    }

    #[test]
    fn conversation_crypto_serializes_snake_case() {
        let plaintext_json = serde_json::to_string(&ConversationCrypto::Plaintext).unwrap();
        assert_eq!(plaintext_json, "\"plaintext\"");

        let mls_json = serde_json::to_string(&ConversationCrypto::MlsV1).unwrap();
        assert_eq!(mls_json, "\"mls_v1\"");
    }

    #[test]
    fn conversation_crypto_deserializes_snake_case() {
        let plaintext: ConversationCrypto = serde_json::from_str("\"plaintext\"").unwrap();
        assert_eq!(plaintext, ConversationCrypto::Plaintext);

        let mls: ConversationCrypto = serde_json::from_str("\"mls_v1\"").unwrap();
        assert_eq!(mls, ConversationCrypto::MlsV1);
    }

    #[test]
    fn conversation_crypto_try_from_string() {
        assert_eq!(
            ConversationCrypto::try_from(String::from("plaintext")).unwrap(),
            ConversationCrypto::Plaintext
        );
        assert_eq!(
            ConversationCrypto::try_from(String::from("mls_v1")).unwrap(),
            ConversationCrypto::MlsV1
        );
        assert!(ConversationCrypto::try_from(String::from("unknown")).is_err());
    }

    #[test]
    fn conversation_crypto_is_encrypted() {
        assert!(!ConversationCrypto::Plaintext.is_encrypted());
        assert!(ConversationCrypto::MlsV1.is_encrypted());
    }

    #[test]
    fn conversation_crypto_as_str() {
        assert_eq!(ConversationCrypto::Plaintext.as_str(), "plaintext");
        assert_eq!(ConversationCrypto::MlsV1.as_str(), "mls_v1");
    }

    #[test]
    fn device_certificate_accepts_valid_fields() {
        let user_id = UserId::new().to_string();
        let device_id = DeviceId::new().to_string();
        let cert =
            DeviceCertificate::try_new(user_id, device_id, vec![0xAB; 32], vec![0xCD; 64]).unwrap();

        assert_eq!(cert.device_signature_pubkey, vec![0xAB; 32]);
        assert_eq!(cert.root_key_signature, vec![0xCD; 64]);
    }

    #[test]
    fn device_certificate_rejects_invalid_user_id() {
        let error = DeviceCertificate::try_new(
            String::from("not-a-ulid"),
            DeviceId::new().to_string(),
            vec![0xAB; 32],
            vec![0xCD; 64],
        )
        .unwrap_err();
        assert_eq!(error, DomainError::InvalidUserId);
    }

    #[test]
    fn device_certificate_rejects_invalid_device_id() {
        let error = DeviceCertificate::try_new(
            UserId::new().to_string(),
            String::from("not-a-ulid"),
            vec![0xAB; 32],
            vec![0xCD; 64],
        )
        .unwrap_err();
        assert_eq!(error, DomainError::InvalidDeviceId);
    }

    #[test]
    fn device_certificate_rejects_empty_pubkey() {
        let error = DeviceCertificate::try_new(
            UserId::new().to_string(),
            DeviceId::new().to_string(),
            vec![],
            vec![0xCD; 64],
        )
        .unwrap_err();
        assert_eq!(error, DomainError::InvalidDeviceCertificate);
    }

    #[test]
    fn device_certificate_rejects_oversized_pubkey() {
        let error = DeviceCertificate::try_new(
            UserId::new().to_string(),
            DeviceId::new().to_string(),
            vec![0xAB; MAX_DEVICE_SIGNATURE_PUBKEY_BYTES + 1],
            vec![0xCD; 64],
        )
        .unwrap_err();
        assert_eq!(error, DomainError::InvalidDeviceCertificate);
    }

    #[test]
    fn device_certificate_rejects_empty_signature() {
        let error = DeviceCertificate::try_new(
            UserId::new().to_string(),
            DeviceId::new().to_string(),
            vec![0xAB; 32],
            vec![],
        )
        .unwrap_err();
        assert_eq!(error, DomainError::InvalidDeviceCertificate);
    }

    #[test]
    fn device_certificate_rejects_oversized_signature() {
        let error = DeviceCertificate::try_new(
            UserId::new().to_string(),
            DeviceId::new().to_string(),
            vec![0xAB; 32],
            vec![0xCD; MAX_ROOT_KEY_SIGNATURE_BYTES + 1],
        )
        .unwrap_err();
        assert_eq!(error, DomainError::InvalidDeviceCertificate);
    }

    #[test]
    fn device_certificate_deny_unknown_fields() {
        let user_id = UserId::new().to_string();
        let device_id = DeviceId::new().to_string();
        let json = format!(
            r#"{{"user_id":"{user_id}","device_id":"{device_id}","device_signature_pubkey":[172],"root_key_signature":[205],"extra":1}}"#
        );
        let error: Result<DeviceCertificate, _> = serde_json::from_str(&json);
        assert!(error.is_err());
    }
}
