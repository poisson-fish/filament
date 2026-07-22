//! Local encrypted store abstraction for MLS key material and state.
//!
//! This is the foundation for the client-side encrypted message store.
//! The `sqlcipher-store` feature provides the production encrypted backend;
//! full message-history schemas and synchronization remain Phase 4 work.
//!
//! # Security Properties
//!
//! - All key material is zeroized when removed from the store.
//! - The store abstraction keeps key material in the Rust core —
//!   the webview/JS context has no access path to it.
//! - The trait is designed for SQLCipher-backed implementations on
//!   desktop/mobile, with the store key managed by the platform keystore.

use std::collections::{HashMap, HashSet};

use filament_core::{DeviceId, UserId};
use zeroize::{Zeroize, Zeroizing};

use crate::error::KeyStoreError;
use crate::identity::RootIdentityKey;

/// Maximum UTF-8 byte length of a local store key.
pub const MAX_STORE_KEY_BYTES: usize = 160;
/// Maximum bytes stored in one encrypted value.
pub const MAX_STORE_VALUE_BYTES: usize = 4 * 1024 * 1024;
/// Maximum records in one device-local store.
pub const MAX_STORE_ENTRIES: usize = 4_096;
/// Maximum records written in one atomic local-store transaction.
pub const MAX_STORE_BATCH_ENTRIES: usize = 128;
/// SQLCipher database key length.
pub const STORE_ENCRYPTION_KEY_BYTES: usize = 32;

/// Device-scoped identifier for an encrypted local store.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LocalStoreId {
    user_id: UserId,
    device_id: DeviceId,
}

impl LocalStoreId {
    /// Construct a store identifier from validated domain IDs.
    #[must_use]
    pub const fn new(user_id: UserId, device_id: DeviceId) -> Self {
        Self { user_id, device_id }
    }

    /// Owning user.
    #[must_use]
    pub const fn user_id(&self) -> UserId {
        self.user_id
    }

    /// Device whose local state is stored.
    #[must_use]
    pub const fn device_id(&self) -> DeviceId {
        self.device_id
    }

    /// Fixed-format account name for native credential stores.
    #[must_use]
    pub fn credential_account(&self) -> String {
        format!("filament-e2ee-{}-{}", self.user_id, self.device_id)
    }
}

/// Validated key for a record inside the encrypted local store.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct StoreKey(String);

impl StoreKey {
    /// Validate a bounded, path-independent store key.
    ///
    /// # Errors
    /// Rejects empty, oversized, or non-ASCII identifiers and characters
    /// outside `[A-Za-z0-9:_.-]`.
    pub fn new(value: impl Into<String>) -> Result<Self, KeyStoreError> {
        let value = value.into();
        if value.is_empty()
            || value.len() > MAX_STORE_KEY_BYTES
            || !value.bytes().all(|byte| {
                byte.is_ascii_alphanumeric() || matches!(byte, b':' | b'_' | b'.' | b'-')
            })
        {
            return Err(KeyStoreError::InvalidIdentifier);
        }
        Ok(Self(value))
    }

    /// Canonical record key for the device's root identity.
    #[must_use]
    pub fn root_identity() -> Self {
        Self(String::from("identity:root"))
    }

    /// Canonical record key for the device's complete MLS client checkpoint.
    #[must_use]
    pub fn mls_client_state() -> Self {
        Self(String::from("mls:client_state:v1"))
    }

    /// Borrow the validated database key.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Native provider for the device-local SQLCipher key.
///
/// Implementations must use a platform credential store and return exactly
/// [`STORE_ENCRYPTION_KEY_BYTES`] bytes. This trait is native-only and must not
/// be implemented through webview IPC.
pub trait StoreKeyProvider: Send + Sync {
    /// Load the existing key or provision one for a new store.
    ///
    /// # Errors
    /// Returns [`KeyStoreError::KeyUnavailable`] when secure storage cannot be
    /// accessed and [`KeyStoreError::InvalidValue`] for a wrong-length key.
    fn load_or_create_key(
        &self,
        store_id: &LocalStoreId,
    ) -> Result<Zeroizing<Vec<u8>>, KeyStoreError>;
}

/// Persist a root identity secret through the native keystore abstraction.
///
/// No UI-facing raw-key accessor is exposed; the temporary copy is zeroized
/// immediately after the backend copies it.
///
/// # Errors
/// Returns the backend's storage error.
pub fn persist_root_identity(
    store: &dyn LocalKeyStore,
    key: StoreKey,
    identity: &RootIdentityKey,
) -> Result<(), KeyStoreError> {
    let secret = identity.secret_bytes();
    store.store(key, secret.to_vec())
}

/// Load a root identity from the native keystore abstraction.
///
/// # Errors
/// Returns [`KeyStoreError::InvalidValue`] when the stored value is not an
/// Ed25519 secret key.
pub fn load_root_identity(
    store: &dyn LocalKeyStore,
    key: &StoreKey,
) -> Result<RootIdentityKey, KeyStoreError> {
    let mut secret = store.load(key)?;
    let secret_array = Zeroizing::new(
        secret
            .as_slice()
            .try_into()
            .map_err(|_| KeyStoreError::InvalidValue)?,
    );
    secret.zeroize();
    Ok(RootIdentityKey::from_secret_bytes(&secret_array))
}

/// Trait for local encrypted key stores.
///
/// Implementations should:
/// - Encrypt all data at rest (SQLCipher, or equivalent)
/// - Store the encryption key in the platform keystore
/// - Zeroize key material on removal
pub trait LocalKeyStore: Send + Sync {
    /// Store a secret value under the given key.
    ///
    /// # Errors
    /// Returns [`KeyStoreError::BackendError`] if the store backend fails.
    fn store(&self, key: StoreKey, value: Vec<u8>) -> Result<(), KeyStoreError>;

    /// Atomically store a bounded set of distinct records.
    ///
    /// Either every record becomes durable or none does. This is the mailbox
    /// acknowledgment boundary: MLS state, decrypted history, and the pending
    /// acknowledgment outbox must never be torn across separate writes.
    ///
    /// # Errors
    /// Returns a limit or backend error without applying a partial batch.
    fn store_batch(&self, entries: Vec<(StoreKey, Vec<u8>)>) -> Result<(), KeyStoreError>;

    /// Atomically insert records without changing any existing value.
    ///
    /// Exact existing records are accepted idempotently. If any key exists
    /// with different bytes, the whole operation returns
    /// [`KeyStoreError::InvalidValue`] without applying partial writes. The
    /// success value is the number of newly inserted records.
    ///
    /// Backends that cannot provide this compare-and-insert transaction fail
    /// closed. Production encrypted stores must override this method.
    ///
    /// # Errors
    /// Returns a limit, conflict, or backend error without partial mutation.
    fn store_batch_if_absent_or_equal(
        &self,
        mut entries: Vec<(StoreKey, Vec<u8>)>,
    ) -> Result<usize, KeyStoreError> {
        for (_, value) in &mut entries {
            value.zeroize();
        }
        Err(KeyStoreError::BackendError)
    }

    /// Atomically restore a complete portable backup without overwriting any
    /// different local value.
    ///
    /// Backup restoration may contain the root identity plus the full bounded
    /// history snapshot, so it has a larger batch ceiling than ordinary
    /// mailbox transactions. Exact existing values are idempotent. Production
    /// encrypted stores must override this method with one transaction.
    ///
    /// # Errors
    /// Returns a limit, conflict, or backend error without partial mutation.
    fn restore_backup_batch(
        &self,
        mut entries: Vec<(StoreKey, Vec<u8>)>,
    ) -> Result<usize, KeyStoreError> {
        for (_, value) in &mut entries {
            value.zeroize();
        }
        Err(KeyStoreError::BackendError)
    }

    /// Retrieve a secret value by key.
    ///
    /// # Errors
    /// Returns [`KeyStoreError::NotFound`] if the key doesn't exist.
    fn load(&self, key: &StoreKey) -> Result<Zeroizing<Vec<u8>>, KeyStoreError>;

    /// Remove a secret value by key, zeroizing it.
    ///
    /// # Errors
    /// Returns [`KeyStoreError::NotFound`] if the key doesn't exist.
    fn remove(&self, key: &StoreKey) -> Result<(), KeyStoreError>;

    /// Atomically remove a bounded set of distinct records.
    ///
    /// Production stores override this so disappearing-message sweeps cannot
    /// leave a partially deleted batch after a backend failure.
    ///
    /// # Errors
    /// Returns a limit or backend error without applying partial deletion.
    fn remove_batch(&self, keys: &[StoreKey]) -> Result<usize, KeyStoreError> {
        let _ = keys;
        Err(KeyStoreError::BackendError)
    }

    /// Check if a key exists in the store.
    ///
    /// # Errors
    /// Returns a backend error rather than treating storage failure as absence.
    fn exists(&self, key: &StoreKey) -> Result<bool, KeyStoreError>;

    /// List all keys in the store.
    ///
    /// # Errors
    /// Returns a backend error if the store cannot be read.
    fn list_keys(&self) -> Result<Vec<StoreKey>, KeyStoreError>;
}

/// In-memory key store for testing and development.
///
/// **NOT for production use** — data is stored in plaintext in memory.
/// Production implementations use SQLCipher or equivalent.
///
/// All values are zeroized when removed or when the store is dropped.
pub struct InMemoryKeyStore {
    data: std::sync::Mutex<HashMap<StoreKey, Zeroizing<Vec<u8>>>>,
}

impl InMemoryKeyStore {
    /// Create a new empty in-memory key store.
    #[must_use]
    pub fn new() -> Self {
        Self {
            data: std::sync::Mutex::new(HashMap::new()),
        }
    }
}

impl Default for InMemoryKeyStore {
    fn default() -> Self {
        Self::new()
    }
}

impl LocalKeyStore for InMemoryKeyStore {
    fn store(&self, key: StoreKey, value: Vec<u8>) -> Result<(), KeyStoreError> {
        self.store_batch(vec![(key, value)])
    }

    fn store_batch(&self, entries: Vec<(StoreKey, Vec<u8>)>) -> Result<(), KeyStoreError> {
        validate_store_batch(&entries)?;
        let entries = entries
            .into_iter()
            .map(|(key, value)| (key, Zeroizing::new(value)))
            .collect::<Vec<_>>();
        let mut data = self.data.lock().map_err(|_| KeyStoreError::BackendError)?;
        let added = entries
            .iter()
            .filter(|(key, _)| !data.contains_key(key))
            .count();
        if data
            .len()
            .checked_add(added)
            .is_none_or(|count| count > MAX_STORE_ENTRIES)
        {
            return Err(KeyStoreError::LimitExceeded);
        }
        for (key, value) in entries {
            data.insert(key, value);
        }
        Ok(())
    }

    fn store_batch_if_absent_or_equal(
        &self,
        entries: Vec<(StoreKey, Vec<u8>)>,
    ) -> Result<usize, KeyStoreError> {
        validate_store_batch(&entries)?;
        let entries = entries
            .into_iter()
            .map(|(key, value)| (key, Zeroizing::new(value)))
            .collect::<Vec<_>>();
        let mut data = self.data.lock().map_err(|_| KeyStoreError::BackendError)?;
        if entries.iter().any(|(key, value)| {
            data.get(key)
                .is_some_and(|existing| existing.as_slice() != value.as_slice())
        }) {
            return Err(KeyStoreError::InvalidValue);
        }
        let inserted = entries
            .iter()
            .filter(|(key, _)| !data.contains_key(key))
            .count();
        if data
            .len()
            .checked_add(inserted)
            .is_none_or(|count| count > MAX_STORE_ENTRIES)
        {
            return Err(KeyStoreError::LimitExceeded);
        }
        for (key, value) in entries {
            data.entry(key).or_insert(value);
        }
        Ok(inserted)
    }

    fn restore_backup_batch(
        &self,
        entries: Vec<(StoreKey, Vec<u8>)>,
    ) -> Result<usize, KeyStoreError> {
        validate_backup_restore_batch(&entries)?;
        let entries = entries
            .into_iter()
            .map(|(key, value)| (key, Zeroizing::new(value)))
            .collect::<Vec<_>>();
        let mut data = self.data.lock().map_err(|_| KeyStoreError::BackendError)?;
        if entries.iter().any(|(key, value)| {
            data.get(key)
                .is_some_and(|existing| existing.as_slice() != value.as_slice())
        }) {
            return Err(KeyStoreError::InvalidValue);
        }
        let inserted = entries
            .iter()
            .filter(|(key, _)| !data.contains_key(key))
            .count();
        if data
            .len()
            .checked_add(inserted)
            .is_none_or(|count| count > MAX_STORE_ENTRIES)
        {
            return Err(KeyStoreError::LimitExceeded);
        }
        for (key, value) in entries {
            data.entry(key).or_insert(value);
        }
        Ok(inserted)
    }

    fn load(&self, key: &StoreKey) -> Result<Zeroizing<Vec<u8>>, KeyStoreError> {
        let data = self.data.lock().map_err(|_| KeyStoreError::BackendError)?;
        data.get(key)
            .map(|value| Zeroizing::new(value.to_vec()))
            .ok_or(KeyStoreError::NotFound)
    }

    fn remove(&self, key: &StoreKey) -> Result<(), KeyStoreError> {
        let mut data = self.data.lock().map_err(|_| KeyStoreError::BackendError)?;
        let value = data.remove(key).ok_or(KeyStoreError::NotFound)?;
        drop(value);
        Ok(())
    }

    fn remove_batch(&self, keys: &[StoreKey]) -> Result<usize, KeyStoreError> {
        validate_remove_batch(keys)?;
        let mut data = self.data.lock().map_err(|_| KeyStoreError::BackendError)?;
        let mut removed = 0;
        for key in keys {
            if data.remove(key).is_some() {
                removed += 1;
            }
        }
        Ok(removed)
    }

    fn exists(&self, key: &StoreKey) -> Result<bool, KeyStoreError> {
        let data = self.data.lock().map_err(|_| KeyStoreError::BackendError)?;
        Ok(data.contains_key(key))
    }

    fn list_keys(&self) -> Result<Vec<StoreKey>, KeyStoreError> {
        let data = self.data.lock().map_err(|_| KeyStoreError::BackendError)?;
        let mut keys: Vec<_> = data.keys().cloned().collect();
        keys.sort();
        Ok(keys)
    }
}

pub(crate) fn validate_remove_batch(keys: &[StoreKey]) -> Result<(), KeyStoreError> {
    if keys.is_empty() || keys.len() > MAX_STORE_ENTRIES {
        return Err(KeyStoreError::LimitExceeded);
    }
    let mut unique = HashSet::with_capacity(keys.len());
    if keys.iter().all(|key| unique.insert(key.as_str())) {
        Ok(())
    } else {
        Err(KeyStoreError::LimitExceeded)
    }
}

pub(crate) fn validate_store_batch(entries: &[(StoreKey, Vec<u8>)]) -> Result<(), KeyStoreError> {
    if entries.is_empty() || entries.len() > MAX_STORE_BATCH_ENTRIES {
        return Err(KeyStoreError::LimitExceeded);
    }
    let mut keys = HashSet::with_capacity(entries.len());
    for (key, value) in entries {
        if value.is_empty() || value.len() > MAX_STORE_VALUE_BYTES || !keys.insert(key.as_str()) {
            return Err(KeyStoreError::LimitExceeded);
        }
    }
    Ok(())
}

pub(crate) fn validate_backup_restore_batch(
    entries: &[(StoreKey, Vec<u8>)],
) -> Result<(), KeyStoreError> {
    if entries.is_empty() || entries.len() > MAX_STORE_ENTRIES {
        return Err(KeyStoreError::LimitExceeded);
    }
    let mut keys = HashSet::with_capacity(entries.len());
    for (key, value) in entries {
        if value.is_empty() || value.len() > MAX_STORE_VALUE_BYTES || !keys.insert(key.as_str()) {
            return Err(KeyStoreError::LimitExceeded);
        }
    }
    Ok(())
}

impl Drop for InMemoryKeyStore {
    fn drop(&mut self) {
        if let Ok(mut data) = self.data.lock() {
            for (_, value) in data.iter_mut() {
                value.zeroize();
            }
            data.clear();
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn store_and_load_round_trip() {
        let store = InMemoryKeyStore::new();
        let key = StoreKey::new("device:01ARZ3NDEKTSV4RRFFQ69G5FAV:root_key").unwrap();
        let value = vec![0xAB; 32];

        store.store(key.clone(), value.clone()).unwrap();
        assert!(store.exists(&key).unwrap());

        let loaded = store.load(&key).unwrap();
        assert_eq!(loaded.as_slice(), value);
    }

    #[test]
    fn load_nonexistent_returns_not_found() {
        let store = InMemoryKeyStore::new();
        let key = StoreKey::new("nonexistent").unwrap();
        let result = store.load(&key);
        assert_eq!(result, Err(KeyStoreError::NotFound));
    }

    #[test]
    fn remove_zeroizes_value() {
        let store = InMemoryKeyStore::new();
        let key = StoreKey::new("secret").unwrap();
        let value = vec![0xCD; 64];

        store.store(key.clone(), value).unwrap();
        store.remove(&key).unwrap();
        assert!(!store.exists(&key).unwrap());
    }

    #[test]
    fn list_keys_returns_all_keys() {
        let store = InMemoryKeyStore::new();
        store
            .store(StoreKey::new("key1").unwrap(), vec![1])
            .unwrap();
        store
            .store(StoreKey::new("key2").unwrap(), vec![2])
            .unwrap();
        store
            .store(StoreKey::new("key3").unwrap(), vec![3])
            .unwrap();

        let keys: Vec<_> = store
            .list_keys()
            .unwrap()
            .into_iter()
            .map(|key| key.as_str().to_owned())
            .collect();
        assert_eq!(keys, vec!["key1", "key2", "key3"]);
    }

    #[test]
    fn overwrite_existing_key() {
        let store = InMemoryKeyStore::new();
        let key = StoreKey::new("key").unwrap();

        store.store(key.clone(), vec![1, 2, 3]).unwrap();
        store.store(key.clone(), vec![4, 5, 6]).unwrap();

        let loaded = store.load(&key).unwrap();
        assert_eq!(loaded.as_slice(), [4, 5, 6]);
    }

    #[test]
    fn compare_and_insert_is_idempotent_and_atomic_on_conflict() {
        let store = InMemoryKeyStore::new();
        let first = StoreKey::new("history:first").unwrap();
        let second = StoreKey::new("history:second").unwrap();
        store.store(first.clone(), vec![1]).unwrap();
        assert_eq!(
            store
                .store_batch_if_absent_or_equal(vec![
                    (first.clone(), vec![1]),
                    (second.clone(), vec![2]),
                ])
                .unwrap(),
            1
        );
        assert_eq!(
            store.store_batch_if_absent_or_equal(vec![
                (first.clone(), vec![9]),
                (StoreKey::new("history:third").unwrap(), vec![3]),
            ]),
            Err(KeyStoreError::InvalidValue)
        );
        assert_eq!(store.load(&first).unwrap().as_slice(), [1]);
        assert!(!store
            .exists(&StoreKey::new("history:third").unwrap())
            .unwrap());
    }

    #[test]
    fn invalid_batch_is_rejected_without_partial_overwrite() {
        let store = InMemoryKeyStore::new();
        let existing = StoreKey::new("existing").unwrap();
        store.store(existing.clone(), vec![1]).unwrap();

        assert_eq!(
            store.store_batch(vec![
                (existing.clone(), vec![2]),
                (StoreKey::new("empty").unwrap(), Vec::new()),
            ]),
            Err(KeyStoreError::LimitExceeded)
        );
        assert_eq!(store.load(&existing).unwrap().as_slice(), [1]);
    }

    #[test]
    fn root_identity_persistence_round_trip() {
        let store = InMemoryKeyStore::new();
        let key = StoreKey::root_identity();
        let identity = RootIdentityKey::generate();
        persist_root_identity(&store, key.clone(), &identity).unwrap();
        let restored = load_root_identity(&store, &key).unwrap();
        assert_eq!(identity.public_key_bytes(), restored.public_key_bytes());
    }

    #[test]
    fn store_key_and_value_limits_fail_closed() {
        assert_eq!(
            StoreKey::new("bad/key"),
            Err(KeyStoreError::InvalidIdentifier)
        );
        assert_eq!(
            StoreKey::new("x".repeat(MAX_STORE_KEY_BYTES + 1)),
            Err(KeyStoreError::InvalidIdentifier)
        );

        let store = InMemoryKeyStore::new();
        assert_eq!(
            store.store(StoreKey::new("empty").unwrap(), Vec::new()),
            Err(KeyStoreError::LimitExceeded)
        );
        assert_eq!(
            store.store(
                StoreKey::new("oversized").unwrap(),
                vec![0; MAX_STORE_VALUE_BYTES + 1]
            ),
            Err(KeyStoreError::LimitExceeded)
        );
    }
}
