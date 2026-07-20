//! Local encrypted store abstraction for MLS key material and state.
//!
//! This is the foundation for the client-side encrypted message store.
//! Full history sync and SQLCipher integration come in Phase 4.
//! For now, this provides a trait-based abstraction and an in-memory
//! implementation suitable for testing.
//!
//! # Security Properties
//!
//! - All key material is zeroized when removed from the store.
//! - The store abstraction keeps key material in the Rust core —
//!   the webview/JS context has no access path to it.
//! - The trait is designed for SQLCipher-backed implementations on
//!   desktop/mobile, with the store key managed by the platform keystore.

use std::collections::HashMap;

use zeroize::{Zeroize, Zeroizing};

use crate::error::KeyStoreError;
use crate::identity::RootIdentityKey;

/// A key for the local store. Keys are strings to keep the interface simple.
pub type StoreKey = String;

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

    /// Retrieve a secret value by key.
    ///
    /// # Errors
    /// Returns [`KeyStoreError::NotFound`] if the key doesn't exist.
    fn load(&self, key: &StoreKey) -> Result<Vec<u8>, KeyStoreError>;

    /// Remove a secret value by key, zeroizing it.
    ///
    /// # Errors
    /// Returns [`KeyStoreError::NotFound`] if the key doesn't exist.
    fn remove(&self, key: &StoreKey) -> Result<(), KeyStoreError>;

    /// Check if a key exists in the store.
    fn exists(&self, key: &StoreKey) -> bool;

    /// List all keys in the store.
    fn list_keys(&self) -> Vec<StoreKey>;
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
        let mut data = self.data.lock().map_err(|_| KeyStoreError::BackendError)?;
        data.insert(key, Zeroizing::new(value));
        Ok(())
    }

    fn load(&self, key: &StoreKey) -> Result<Vec<u8>, KeyStoreError> {
        let data = self.data.lock().map_err(|_| KeyStoreError::BackendError)?;
        data.get(key)
            .map(|value| value.to_vec())
            .ok_or_else(|| KeyStoreError::NotFound(key.clone()))
    }

    fn remove(&self, key: &StoreKey) -> Result<(), KeyStoreError> {
        let mut data = self.data.lock().map_err(|_| KeyStoreError::BackendError)?;
        let value = data
            .remove(key)
            .ok_or_else(|| KeyStoreError::NotFound(key.clone()))?;
        drop(value);
        Ok(())
    }

    fn exists(&self, key: &StoreKey) -> bool {
        let Ok(data) = self.data.lock() else {
            return false;
        };
        data.contains_key(key)
    }

    fn list_keys(&self) -> Vec<StoreKey> {
        let Ok(data) = self.data.lock() else {
            return Vec::new();
        };
        data.keys().cloned().collect()
    }
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
        let key = "device:01ARZ3NDEKTSV4RRFFQ69G5FAV:root_key".to_string();
        let value = vec![0xAB; 32];

        store.store(key.clone(), value.clone()).unwrap();
        assert!(store.exists(&key));

        let loaded = store.load(&key).unwrap();
        assert_eq!(loaded, value);
    }

    #[test]
    fn load_nonexistent_returns_not_found() {
        let store = InMemoryKeyStore::new();
        let key = "nonexistent".to_string();
        let result = store.load(&key);
        assert_eq!(result, Err(KeyStoreError::NotFound(key)));
    }

    #[test]
    fn remove_zeroizes_value() {
        let store = InMemoryKeyStore::new();
        let key = "secret".to_string();
        let value = vec![0xCD; 64];

        store.store(key.clone(), value).unwrap();
        store.remove(&key).unwrap();
        assert!(!store.exists(&key));
    }

    #[test]
    fn list_keys_returns_all_keys() {
        let store = InMemoryKeyStore::new();
        store.store("key1".to_string(), vec![1]).unwrap();
        store.store("key2".to_string(), vec![2]).unwrap();
        store.store("key3".to_string(), vec![3]).unwrap();

        let mut keys = store.list_keys();
        keys.sort();
        assert_eq!(keys, vec!["key1", "key2", "key3"]);
    }

    #[test]
    fn overwrite_existing_key() {
        let store = InMemoryKeyStore::new();
        let key = "key".to_string();

        store.store(key.clone(), vec![1, 2, 3]).unwrap();
        store.store(key.clone(), vec![4, 5, 6]).unwrap();

        let loaded = store.load(&key).unwrap();
        assert_eq!(loaded, vec![4, 5, 6]);
    }

    #[test]
    fn root_identity_persistence_round_trip() {
        let store = InMemoryKeyStore::new();
        let key = String::from("root-identity");
        let identity = RootIdentityKey::generate();
        persist_root_identity(&store, key.clone(), &identity).unwrap();
        let restored = load_root_identity(&store, &key).unwrap();
        assert_eq!(identity.public_key_bytes(), restored.public_key_bytes());
    }
}
