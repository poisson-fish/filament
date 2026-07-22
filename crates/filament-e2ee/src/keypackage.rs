//! KeyPackage pool management for MLS prekey analog.
//!
//! Each device maintains a pool of single-use KeyPackages plus one
//! last-resort fallback. The server stores
//! KeyPackages as opaque blobs and never parses interiors.
//!
//! # Pool Semantics
//!
//! - Single-use KeyPackages: claimed once, then removed from the pool.
//! - Last-resort KeyPackage: reserved until ordinary packages are exhausted,
//!   then claimed exactly once.
//! - Pool size is capped at `MAX_POOL_SIZE` per device.
//! - The server atomically decrements the pool on claim.

use openmls::prelude::*;
use openmls_basic_credential::SignatureKeyPair;
use openmls_rust_crypto::OpenMlsRustCrypto;
use openmls_traits::signatures::Signer;
use tls_codec::Serialize;
use zeroize::Zeroize;

use crate::{error::KeyPackageError, identity::RootIdentityKey, KeyStoreError};
use filament_core::{DeviceCertificate, DeviceId, UserId};

/// Default pool size cap per device.
pub const DEFAULT_MAX_POOL_SIZE: usize = 100;

/// Default number of single-use KeyPackages to generate in a batch.
pub const DEFAULT_BATCH_SIZE: usize = 10;

/// Default ciphersuite for KeyPackage generation.
const CIPHERSUITE: Ciphersuite = Ciphersuite::MLS_128_DHKEMX25519_CHACHA20POLY1305_SHA256_Ed25519;

pub(crate) const DEVICE_CREDENTIAL_DOMAIN: &[u8] = b"filament:mls:device_credential:v1";
pub(crate) type ProviderRecord = (Vec<u8>, Vec<u8>);

/// Long-lived MLS state for one certified Filament device.
///
/// The provider owns HPKE private keys and future group state. It must live
/// for at least as long as any generated `KeyPackage`; dropping it makes the
/// corresponding public packages unusable. A production client persists the
/// provider through the native encrypted-store boundary.
pub struct MlsDevice {
    provider: OpenMlsRustCrypto,
    signer: SignatureKeyPair,
    credential_with_key: CredentialWithKey,
    certificate: DeviceCertificate,
    root_key_pub: [u8; 32],
    user_id: UserId,
    device_id: DeviceId,
}

impl MlsDevice {
    /// Generate MLS signature material and certify it under the user's root
    /// identity key.
    ///
    /// # Errors
    /// Returns [`KeyPackageError::CreationFailed`] if key generation/storage
    /// or certificate construction fails.
    pub fn generate(
        user_id: UserId,
        device_id: DeviceId,
        root_identity: &RootIdentityKey,
    ) -> Result<Self, KeyPackageError> {
        let provider = OpenMlsRustCrypto::default();
        let signer = SignatureKeyPair::new(CIPHERSUITE.signature_algorithm())
            .map_err(|_| KeyPackageError::CreationFailed)?;
        signer
            .store(provider.storage())
            .map_err(|_| KeyPackageError::CreationFailed)?;
        let signature_key: [u8; 32] = signer
            .to_public_vec()
            .try_into()
            .map_err(|_| KeyPackageError::CreationFailed)?;
        let certificate = root_identity
            .certify_device(user_id, device_id, signature_key)
            .map_err(|_| KeyPackageError::CreationFailed)?;
        let root_key_pub = root_identity.public_key_bytes();
        let credential = BasicCredential::new(device_credential_bytes(&certificate, &root_key_pub));
        let credential_with_key = CredentialWithKey {
            credential: credential.into(),
            signature_key: signer.to_public_vec().into(),
        };
        Ok(Self {
            provider,
            signer,
            credential_with_key,
            certificate,
            root_key_pub,
            user_id,
            device_id,
        })
    }

    /// Public device certificate bound into this device's MLS credential.
    #[must_use]
    pub fn certificate(&self) -> &DeviceCertificate {
        &self.certificate
    }

    /// Root identity public key required to verify [`Self::certificate`].
    #[must_use]
    pub const fn root_key_public(&self) -> &[u8; 32] {
        &self.root_key_pub
    }

    /// Account identity certified into this device's MLS credential.
    #[must_use]
    pub const fn user_id(&self) -> UserId {
        self.user_id
    }

    /// Stable device identity certified into this device's MLS credential.
    #[must_use]
    pub const fn device_id(&self) -> DeviceId {
        self.device_id
    }

    pub(crate) const fn provider(&self) -> &OpenMlsRustCrypto {
        &self.provider
    }

    pub(crate) const fn signer(&self) -> &SignatureKeyPair {
        &self.signer
    }

    pub(crate) fn credential_with_key(&self) -> CredentialWithKey {
        self.credential_with_key.clone()
    }

    pub(crate) fn sign_pairing_authorization(
        &self,
        payload: &[u8],
    ) -> Result<Vec<u8>, KeyPackageError> {
        self.signer
            .sign(payload)
            .map_err(|_| KeyPackageError::CreationFailed)
    }

    pub(crate) fn sign_history_sync(&self, payload: &[u8]) -> Result<Vec<u8>, KeyPackageError> {
        self.signer
            .sign(payload)
            .map_err(|_| KeyPackageError::CreationFailed)
    }

    pub(crate) fn provider_records(&self) -> Result<Vec<ProviderRecord>, KeyStoreError> {
        let values = self
            .provider
            .storage()
            .values
            .read()
            .map_err(|_| KeyStoreError::BackendError)?;
        let mut records = values
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect::<Vec<_>>();
        records.sort_by(|left, right| left.0.cmp(&right.0));
        Ok(records)
    }

    pub(crate) fn restore(
        certificate: DeviceCertificate,
        root_key_pub: [u8; 32],
        records: &[ProviderRecord],
    ) -> Result<Self, KeyStoreError> {
        let user_id = UserId::try_from(certificate.user_id.clone())
            .map_err(|_| KeyStoreError::InvalidValue)?;
        let device_id = DeviceId::try_from(certificate.device_id.clone())
            .map_err(|_| KeyStoreError::InvalidValue)?;
        let signature_key: &[u8; 32] = certificate
            .device_signature_pubkey
            .as_slice()
            .try_into()
            .map_err(|_| KeyStoreError::InvalidValue)?;
        let root_signature: &[u8; 64] = certificate
            .root_key_signature
            .as_slice()
            .try_into()
            .map_err(|_| KeyStoreError::InvalidValue)?;
        crate::verify_device_certificate(
            &root_key_pub,
            user_id,
            device_id,
            signature_key,
            root_signature,
        )
        .map_err(|_| KeyStoreError::InvalidValue)?;

        let provider = OpenMlsRustCrypto::default();
        {
            let mut values = provider
                .storage()
                .values
                .write()
                .map_err(|_| KeyStoreError::BackendError)?;
            for (key, value) in records {
                if values.insert(key.clone(), value.clone()).is_some() {
                    return Err(KeyStoreError::InvalidValue);
                }
            }
        }
        let signer = SignatureKeyPair::read(
            provider.storage(),
            &certificate.device_signature_pubkey,
            CIPHERSUITE.signature_algorithm(),
        )
        .ok_or(KeyStoreError::InvalidValue)?;
        if signer.to_public_vec() != certificate.device_signature_pubkey {
            return Err(KeyStoreError::InvalidValue);
        }
        let credential = BasicCredential::new(device_credential_bytes(&certificate, &root_key_pub));
        let credential_with_key = CredentialWithKey {
            credential: credential.into(),
            signature_key: signer.to_public_vec().into(),
        };
        Ok(Self {
            provider,
            signer,
            credential_with_key,
            certificate,
            root_key_pub,
            user_id,
            device_id,
        })
    }
}

impl core::fmt::Debug for MlsDevice {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("MlsDevice(<key material redacted>)")
    }
}

fn device_credential_bytes(certificate: &DeviceCertificate, root_key_pub: &[u8; 32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(DEVICE_CREDENTIAL_DOMAIN.len() + 180);
    bytes.extend_from_slice(DEVICE_CREDENTIAL_DOMAIN);
    bytes.extend_from_slice(certificate.user_id.as_bytes());
    bytes.extend_from_slice(certificate.device_id.as_bytes());
    bytes.extend_from_slice(&certificate.device_signature_pubkey);
    bytes.extend_from_slice(&certificate.root_key_signature);
    bytes.extend_from_slice(root_key_pub);
    bytes
}

/// A generated KeyPackage bundle with its serialized blob and metadata.
#[derive(Debug, Clone)]
pub struct GeneratedKeyPackage {
    /// The serialized KeyPackage blob (TLS-encoded, opaque to the server).
    pub blob: Vec<u8>,
    /// Whether this is the one-time last-resort fallback.
    pub is_last_resort: bool,
    key_package: KeyPackage,
}

impl GeneratedKeyPackage {
    /// Parsed public `KeyPackage` for local MLS group operations.
    #[must_use]
    pub const fn key_package(&self) -> &KeyPackage {
        &self.key_package
    }
}

/// KeyPackage pool entry as stored by the server.
///
/// This is the in-memory representation. The server stores the blob
/// as opaque BYTEA in the database.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyPackagePoolEntry {
    /// The device that owns this KeyPackage.
    pub device_id: String,
    /// A hash of the KeyPackage blob for deduplication and lookup.
    pub key_package_hash: String,
    /// The serialized KeyPackage blob (opaque to the server).
    pub key_package_blob: Vec<u8>,
    /// Whether this is a last-resort KeyPackage.
    pub is_last_resort: bool,
    /// Unix timestamp when this KeyPackage was claimed (None = unclaimed).
    pub claimed_at_unix: Option<i64>,
    /// Unix timestamp when this KeyPackage was created.
    pub created_at_unix: i64,
}

/// Generate a batch of single-use KeyPackages for a device.
///
/// Each KeyPackage is created with a fresh HPKE init key and serialized
/// via TLS encoding. The private key material is stored in the OpenMLS
/// provider's storage and must be zeroized when no longer needed.
///
/// # Errors
/// Returns [`KeyPackageError::CreationFailed`] if OpenMLS fails to generate
/// a KeyPackage, or [`KeyPackageError::SerializationFailed`] if TLS
/// serialization fails.
pub fn generate_key_package_batch(
    device: &MlsDevice,
    count: usize,
) -> Result<Vec<GeneratedKeyPackage>, KeyPackageError> {
    if count == 0 {
        return Ok(Vec::new());
    }

    if count > DEFAULT_MAX_POOL_SIZE {
        return Err(KeyPackageError::PoolLimitExceeded {
            max: DEFAULT_MAX_POOL_SIZE,
            requested: count,
        });
    }

    let mut packages = Vec::with_capacity(count);
    for _ in 0..count {
        let bundle = KeyPackage::builder()
            .build(
                CIPHERSUITE,
                device.provider(),
                device.signer(),
                device.credential_with_key(),
            )
            .map_err(|_| KeyPackageError::CreationFailed)?;

        let blob = bundle
            .key_package()
            .tls_serialize_detached()
            .map_err(|_| KeyPackageError::SerializationFailed)?;

        packages.push(GeneratedKeyPackage {
            blob,
            is_last_resort: false,
            key_package: bundle.key_package().clone(),
        });
    }

    Ok(packages)
}

/// Generate a single last-resort KeyPackage for a device.
///
/// The last-resort marker controls server ordering only. The generated
/// package remains single-use because it does not carry the MLS last-resort
/// extension; reusing its init key would violate the expected prekey model.
///
/// # Errors
/// Returns [`KeyPackageError::CreationFailed`] or [`KeyPackageError::SerializationFailed`].
pub fn generate_last_resort_key_package(
    device: &MlsDevice,
) -> Result<GeneratedKeyPackage, KeyPackageError> {
    let bundle = KeyPackage::builder()
        .build(
            CIPHERSUITE,
            device.provider(),
            device.signer(),
            device.credential_with_key(),
        )
        .map_err(|_| KeyPackageError::CreationFailed)?;

    let blob = bundle
        .key_package()
        .tls_serialize_detached()
        .map_err(|_| KeyPackageError::SerializationFailed)?;

    Ok(GeneratedKeyPackage {
        blob,
        is_last_resort: true,
        key_package: bundle.key_package().clone(),
    })
}

/// Compute a hash of a KeyPackage blob for deduplication and lookup.
///
/// Uses SHA-256 and returns a hex-encoded string.
///
/// # Panics
/// This should never panic as SHA-256 is infallible for byte slices.
#[must_use]
pub fn key_package_hash(blob: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(blob);
    let hash = hasher.finalize();
    let mut hex = String::with_capacity(hash.len() * 2);
    for byte in &hash {
        use std::fmt::Write;
        let _ = write!(hex, "{byte:02x}");
    }
    hex
}

/// In-memory KeyPackage pool for a single device.
///
/// This is used for testing and as a reference implementation. The server
/// uses the database-backed pool instead.
#[derive(Debug)]
pub struct KeyPackagePool {
    device_id: String,
    entries: Vec<KeyPackagePoolEntry>,
    max_size: usize,
}

impl KeyPackagePool {
    /// Create a new pool for a device with the given max size.
    #[must_use]
    pub fn new(device_id: String, max_size: usize) -> Self {
        Self {
            device_id,
            entries: Vec::new(),
            max_size,
        }
    }

    /// Add KeyPackages to the pool.
    ///
    /// # Errors
    /// Returns [`KeyPackageError::PoolLimitExceeded`] if adding the packages
    /// would exceed the pool's max size.
    pub fn add(
        &mut self,
        packages: Vec<GeneratedKeyPackage>,
        created_at_unix: i64,
    ) -> Result<usize, KeyPackageError> {
        let new_count = self.entries.len() + packages.len();
        if new_count > self.max_size {
            return Err(KeyPackageError::PoolLimitExceeded {
                max: self.max_size,
                requested: new_count,
            });
        }

        for package in packages {
            let hash = key_package_hash(&package.blob);
            self.entries.push(KeyPackagePoolEntry {
                device_id: self.device_id.clone(),
                key_package_hash: hash,
                key_package_blob: package.blob,
                is_last_resort: package.is_last_resort,
                claimed_at_unix: None,
                created_at_unix,
            });
        }

        Ok(self.entries.len())
    }

    /// Claim a KeyPackage from the pool.
    ///
    /// Ordinary packages are preferred, then the last-resort fallback is
    /// claimed once. Claimed entries are removed from this in-memory pool.
    ///
    /// # Errors
    /// Returns [`KeyPackageError::PoolExhausted`] if no unclaimed packages remain.
    pub fn claim(&mut self, claimed_at_unix: i64) -> Result<KeyPackagePoolEntry, KeyPackageError> {
        // Try to claim a single-use (non-last-resort) package first.
        if let Some(idx) = self
            .entries
            .iter()
            .position(|e| !e.is_last_resort && e.claimed_at_unix.is_none())
        {
            let mut entry = self.entries.remove(idx);
            entry.claimed_at_unix = Some(claimed_at_unix);
            return Ok(entry);
        }

        // Fall back to the one-time last-resort package.
        if let Some(idx) = self
            .entries
            .iter()
            .position(|entry| entry.is_last_resort && entry.claimed_at_unix.is_none())
        {
            let mut entry = self.entries.remove(idx);
            entry.claimed_at_unix = Some(claimed_at_unix);
            return Ok(entry);
        }

        Err(KeyPackageError::PoolExhausted)
    }
}

/// Returns the number of unclaimed (available) KeyPackages in the pool.
#[must_use]
pub fn available_count(pool: &KeyPackagePool) -> usize {
    pool.entries
        .iter()
        .filter(|e| e.claimed_at_unix.is_none())
        .count()
}

/// Returns the total number of entries in the pool (claimed + unclaimed).
#[must_use]
pub fn total_count(pool: &KeyPackagePool) -> usize {
    pool.entries.len()
}

/// Zeroize a KeyPackage blob in place.
///
/// Call this when a KeyPackage blob is no longer needed.
pub fn zeroize_blob(blob: &mut Vec<u8>) {
    blob.zeroize();
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tls_codec::{Deserialize, Serialize};

    fn test_device() -> MlsDevice {
        MlsDevice::generate(UserId::new(), DeviceId::new(), &RootIdentityKey::generate()).unwrap()
    }

    #[test]
    fn generate_single_use_key_package_batch() {
        let packages = generate_key_package_batch(&test_device(), 5).unwrap();
        assert_eq!(packages.len(), 5);
        for package in &packages {
            assert!(!package.is_last_resort);
            assert!(!package.blob.is_empty());
        }
    }

    #[test]
    fn generate_last_resort_key_package_works() {
        let package = generate_last_resort_key_package(&test_device()).unwrap();
        assert!(package.is_last_resort);
        assert!(!package.blob.is_empty());
    }

    #[test]
    fn key_package_serialization_round_trip() {
        let packages = generate_key_package_batch(&test_device(), 1).unwrap();
        let original_blob = &packages[0].blob;

        // Deserialize the KeyPackage from the blob via KeyPackageIn
        let decoded = KeyPackageIn::tls_deserialize(&mut original_blob.as_slice()).unwrap();
        let re_serialized = decoded.tls_serialize_detached().unwrap();

        // The re-serialized blob should match the original
        assert_eq!(original_blob.as_slice(), re_serialized.as_slice());
    }

    #[test]
    fn retained_provider_can_consume_welcome_for_generated_key_package() {
        let alice = test_device();
        let bob = test_device();
        let mut alice_group = MlsGroup::builder()
            .ciphersuite(CIPHERSUITE)
            .use_ratchet_tree_extension(true)
            .build(
                alice.provider(),
                alice.signer(),
                alice.credential_with_key(),
            )
            .unwrap();
        let bob_package = generate_key_package_batch(&bob, 1).unwrap().remove(0);
        let (_, welcome, _) = alice_group
            .add_members(
                alice.provider(),
                alice.signer(),
                &[bob_package.key_package().clone()],
            )
            .unwrap();
        alice_group.merge_pending_commit(alice.provider()).unwrap();

        let bytes = welcome.to_bytes().unwrap();
        let MlsMessageBodyIn::Welcome(welcome) = MlsMessageIn::tls_deserialize_exact(&bytes)
            .unwrap()
            .extract()
        else {
            panic!("expected Welcome");
        };
        let join_config = MlsGroupJoinConfig::builder()
            .use_ratchet_tree_extension(true)
            .build();
        let bob_group =
            StagedWelcome::new_from_welcome(bob.provider(), &join_config, welcome, None)
                .unwrap()
                .into_group(bob.provider())
                .unwrap();
        assert_eq!(bob_group.members().count(), 2);
    }

    #[test]
    fn key_package_hash_is_deterministic() {
        let packages = generate_key_package_batch(&test_device(), 1).unwrap();
        let blob = &packages[0].blob;

        let hash1 = key_package_hash(blob);
        let hash2 = key_package_hash(blob);

        assert_eq!(hash1, hash2);
        assert_eq!(hash1.len(), 64); // SHA-256 hex = 64 chars
    }

    #[test]
    fn key_package_hash_differs_for_different_blobs() {
        let packages = generate_key_package_batch(&test_device(), 2).unwrap();

        let hash1 = key_package_hash(&packages[0].blob);
        let hash2 = key_package_hash(&packages[1].blob);

        assert_ne!(hash1, hash2);
    }

    #[test]
    fn pool_add_and_claim_single_use() {
        let mut pool = KeyPackagePool::new("device-1".to_string(), 100);
        let packages = generate_key_package_batch(&test_device(), 5).unwrap();
        let now = 1_700_000_000;

        let stored = pool.add(packages, now).unwrap();
        assert_eq!(stored, 5);
        assert_eq!(available_count(&pool), 5);

        // Claim one
        let claimed = pool.claim(now).unwrap();
        assert!(!claimed.is_last_resort);
        assert_eq!(available_count(&pool), 4);

        // Claim all remaining
        for _ in 0..4 {
            pool.claim(now).unwrap();
        }
        assert_eq!(available_count(&pool), 0);

        // Pool exhausted
        assert_eq!(pool.claim(now).unwrap_err(), KeyPackageError::PoolExhausted);
    }

    #[test]
    fn pool_exhaustion_falls_back_to_last_resort() {
        let mut pool = KeyPackagePool::new("device-1".to_string(), 100);

        // Add one single-use and one last-resort
        let device = test_device();
        let mut packages = generate_key_package_batch(&device, 1).unwrap();
        let last_resort = generate_last_resort_key_package(&device).unwrap();
        packages.push(last_resort);
        pool.add(packages, 1_700_000_000).unwrap();

        // Claim the single-use one
        let claimed1 = pool.claim(1_700_000_001).unwrap();
        assert!(!claimed1.is_last_resort);

        // Pool exhausted for single-use, falls back to last-resort
        let claimed2 = pool.claim(1_700_000_002).unwrap();
        assert!(claimed2.is_last_resort);

        // The fallback is still single-use until an MLS last-resort extension
        // is implemented and reviewed.
        assert_eq!(
            pool.claim(1_700_000_003).unwrap_err(),
            KeyPackageError::PoolExhausted
        );
    }

    #[test]
    fn pool_limit_exceeded() {
        let mut pool = KeyPackagePool::new("device-1".to_string(), 3);
        let packages = generate_key_package_batch(&test_device(), 4).unwrap();

        let result = pool.add(packages, 1_700_000_000);
        assert_eq!(
            result.unwrap_err(),
            KeyPackageError::PoolLimitExceeded {
                max: 3,
                requested: 4
            }
        );
    }

    #[test]
    fn zeroize_blob_clears_data() {
        let mut blob = vec![0xAB; 128];
        zeroize_blob(&mut blob);
        assert!(blob.iter().all(|&b| b == 0));
    }
}
