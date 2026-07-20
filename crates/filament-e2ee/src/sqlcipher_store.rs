//! SQLCipher-backed device-local secret storage.
//!
//! The database key comes from a native [`StoreKeyProvider`](crate::StoreKeyProvider)
//! and is applied before any database read. The implementation fails closed if
//! SQLCipher is absent, the key is wrong, the path is a symlink, or a hard
//! storage limit is exceeded.

use std::{
    fmt::Write as _,
    fs::{self, OpenOptions},
    path::{Path, PathBuf},
    sync::Mutex,
    time::Duration,
};

use rusqlite::{limits::Limit, params, Connection, OpenFlags, OptionalExtension as _};
use zeroize::Zeroizing;

use crate::{
    KeyStoreError, LocalKeyStore, LocalStoreId, StoreKey, StoreKeyProvider, MAX_STORE_ENTRIES,
    MAX_STORE_KEY_BYTES, MAX_STORE_VALUE_BYTES, STORE_ENCRYPTION_KEY_BYTES,
};

/// Maximum encrypted database size for the Phase 1 foundation.
pub const MAX_ENCRYPTED_STORE_BYTES: usize = 64 * 1024 * 1024;

const SQLCIPHER_PAGE_BYTES: usize = 4_096;
const MAX_PAGE_COUNT: usize = MAX_ENCRYPTED_STORE_BYTES / SQLCIPHER_PAGE_BYTES;
const BUSY_TIMEOUT: Duration = Duration::from_secs(2);

/// Production encrypted local store backed by bundled SQLCipher.
pub struct SqlCipherKeyStore {
    connection: Mutex<Connection>,
}

impl core::fmt::Debug for SqlCipherKeyStore {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("SqlCipherKeyStore(<path and key redacted>)")
    }
}

impl SqlCipherKeyStore {
    /// Open or create a device-scoped encrypted store.
    ///
    /// The path must be absolute. Its parent must already exist, and an
    /// existing target must be a regular non-symlink file. On Unix, the file
    /// is created and maintained with mode `0600`.
    ///
    /// # Errors
    /// Fails closed when the platform key is unavailable or malformed,
    /// SQLCipher is unavailable, a wrong key is supplied, path validation
    /// fails, or schema/limit configuration fails.
    pub fn open(
        path: &Path,
        store_id: &LocalStoreId,
        key_provider: &dyn StoreKeyProvider,
    ) -> Result<Self, KeyStoreError> {
        let path = prepare_database_file(path)?;
        let key = key_provider.load_or_create_key(store_id)?;
        if key.len() != STORE_ENCRYPTION_KEY_BYTES {
            return Err(KeyStoreError::InvalidValue);
        }

        let flags = OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX;
        let connection = Connection::open_with_flags(&path, flags).map_err(map_backend_error)?;
        apply_sqlcipher_key(&connection, &key)?;
        configure_connection(&connection)?;
        initialize_schema(&connection)?;
        enforce_file_permissions(&path)?;

        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    fn connection(&self) -> Result<std::sync::MutexGuard<'_, Connection>, KeyStoreError> {
        self.connection
            .lock()
            .map_err(|_| KeyStoreError::BackendError)
    }
}

impl LocalKeyStore for SqlCipherKeyStore {
    fn store(&self, key: StoreKey, value: Vec<u8>) -> Result<(), KeyStoreError> {
        let value = Zeroizing::new(value);
        if value.is_empty() || value.len() > MAX_STORE_VALUE_BYTES {
            return Err(KeyStoreError::LimitExceeded);
        }
        let connection = self.connection()?;
        connection
            .execute(
                "INSERT INTO local_secrets (store_key, secret_value)
                 VALUES (?1, ?2)
                 ON CONFLICT(store_key) DO UPDATE SET secret_value = excluded.secret_value",
                params![key.as_str(), value.as_slice()],
            )
            .map_err(|error| map_sqlite_limit_error(&error))?;
        Ok(())
    }

    fn load(&self, key: &StoreKey) -> Result<Zeroizing<Vec<u8>>, KeyStoreError> {
        let connection = self.connection()?;
        let value = connection
            .query_row(
                "SELECT secret_value FROM local_secrets WHERE store_key = ?1",
                [key.as_str()],
                |row| row.get::<_, Vec<u8>>(0),
            )
            .optional()
            .map_err(map_backend_error)?
            .ok_or(KeyStoreError::NotFound)?;
        if value.is_empty() || value.len() > MAX_STORE_VALUE_BYTES {
            return Err(KeyStoreError::InvalidValue);
        }
        Ok(Zeroizing::new(value))
    }

    fn remove(&self, key: &StoreKey) -> Result<(), KeyStoreError> {
        let connection = self.connection()?;
        let deleted = connection
            .execute(
                "DELETE FROM local_secrets WHERE store_key = ?1",
                [key.as_str()],
            )
            .map_err(map_backend_error)?;
        if deleted == 0 {
            return Err(KeyStoreError::NotFound);
        }
        Ok(())
    }

    fn exists(&self, key: &StoreKey) -> Result<bool, KeyStoreError> {
        let connection = self.connection()?;
        connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM local_secrets WHERE store_key = ?1)",
                [key.as_str()],
                |row| row.get(0),
            )
            .map_err(map_backend_error)
    }

    fn list_keys(&self) -> Result<Vec<StoreKey>, KeyStoreError> {
        let connection = self.connection()?;
        let mut statement = connection
            .prepare("SELECT store_key FROM local_secrets ORDER BY store_key LIMIT ?1")
            .map_err(map_backend_error)?;
        let limit = i64::try_from(MAX_STORE_ENTRIES).map_err(|_| KeyStoreError::BackendError)?;
        let rows = statement
            .query_map([limit], |row| row.get::<_, String>(0))
            .map_err(map_backend_error)?;
        let mut keys = Vec::new();
        for row in rows {
            let raw = row.map_err(map_backend_error)?;
            keys.push(StoreKey::new(raw).map_err(|_| KeyStoreError::InvalidValue)?);
        }
        Ok(keys)
    }
}

fn prepare_database_file(path: &Path) -> Result<PathBuf, KeyStoreError> {
    if !path.is_absolute() || path.file_name().is_none() {
        return Err(KeyStoreError::InvalidPath);
    }
    let parent = path.parent().ok_or(KeyStoreError::InvalidPath)?;
    let canonical_parent = parent
        .canonicalize()
        .map_err(|_| KeyStoreError::InvalidPath)?;
    let normalized = canonical_parent.join(path.file_name().ok_or(KeyStoreError::InvalidPath)?);

    match fs::symlink_metadata(&normalized) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                return Err(KeyStoreError::InvalidPath);
            }
            if usize::try_from(metadata.len())
                .map_or(true, |length| length > MAX_ENCRYPTED_STORE_BYTES)
            {
                return Err(KeyStoreError::LimitExceeded);
            }
            reject_hard_linked_file(&metadata)?;
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            create_private_file(&normalized)?;
        }
        Err(_) => return Err(KeyStoreError::InvalidPath),
    }
    enforce_file_permissions(&normalized)?;
    Ok(normalized)
}

#[cfg(unix)]
fn reject_hard_linked_file(metadata: &fs::Metadata) -> Result<(), KeyStoreError> {
    use std::os::unix::fs::MetadataExt as _;

    if metadata.nlink() != 1 {
        return Err(KeyStoreError::InvalidPath);
    }
    Ok(())
}

#[cfg(not(unix))]
fn reject_hard_linked_file(_metadata: &fs::Metadata) -> Result<(), KeyStoreError> {
    Ok(())
}

#[cfg(unix)]
fn create_private_file(path: &Path) -> Result<(), KeyStoreError> {
    use std::os::unix::fs::OpenOptionsExt as _;

    OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .map(|_| ())
        .map_err(|_| KeyStoreError::InvalidPath)
}

#[cfg(not(unix))]
fn create_private_file(path: &Path) -> Result<(), KeyStoreError> {
    OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(path)
        .map(|_| ())
        .map_err(|_| KeyStoreError::InvalidPath)
}

#[cfg(unix)]
fn enforce_file_permissions(path: &Path) -> Result<(), KeyStoreError> {
    use std::os::unix::fs::PermissionsExt as _;

    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
        .map_err(|_| KeyStoreError::InvalidPath)
}

#[cfg(not(unix))]
fn enforce_file_permissions(_path: &Path) -> Result<(), KeyStoreError> {
    Ok(())
}

fn apply_sqlcipher_key(
    connection: &Connection,
    key: &Zeroizing<Vec<u8>>,
) -> Result<(), KeyStoreError> {
    let mut key_hex = Zeroizing::new(String::with_capacity(STORE_ENCRYPTION_KEY_BYTES * 2));
    for byte in key.iter() {
        write!(&mut *key_hex, "{byte:02x}").map_err(|_| KeyStoreError::BackendError)?;
    }
    let pragma = Zeroizing::new(format!(
        "PRAGMA key = \"x'{}'\";
         PRAGMA cipher_compatibility = 4;
         PRAGMA cipher_page_size = {SQLCIPHER_PAGE_BYTES};",
        key_hex.as_str()
    ));
    connection
        .execute_batch(pragma.as_str())
        .map_err(map_backend_error)?;

    let cipher_version = connection
        .query_row("PRAGMA cipher_version", [], |row| row.get::<_, String>(0))
        .map_err(map_backend_error)?;
    if cipher_version.trim().is_empty() {
        return Err(KeyStoreError::BackendError);
    }
    let cipher_page_size = connection
        .query_row("PRAGMA cipher_page_size", [], |row| row.get::<_, String>(0))
        .map_err(map_backend_error)?;
    let cipher_page_size = cipher_page_size
        .parse::<usize>()
        .map_err(|_| KeyStoreError::BackendError)?;
    if cipher_page_size != SQLCIPHER_PAGE_BYTES {
        return Err(KeyStoreError::BackendError);
    }
    connection
        .query_row("SELECT count(*) FROM sqlite_schema", [], |row| {
            row.get::<_, i64>(0)
        })
        .map_err(map_backend_error)?;
    Ok(())
}

fn configure_connection(connection: &Connection) -> Result<(), KeyStoreError> {
    connection
        .busy_timeout(BUSY_TIMEOUT)
        .map_err(map_backend_error)?;
    connection
        .execute_batch(
            "PRAGMA cipher_memory_security = ON;
             PRAGMA cipher_plaintext_header_size = 0;
             PRAGMA foreign_keys = ON;
             PRAGMA trusted_schema = OFF;
             PRAGMA secure_delete = ON;
             PRAGMA temp_store = MEMORY;
             PRAGMA journal_mode = DELETE;",
        )
        .map_err(map_backend_error)?;
    let configured_page_cap = connection
        .query_row(
            &format!("PRAGMA max_page_count = {MAX_PAGE_COUNT}"),
            [],
            |row| row.get::<_, i64>(0),
        )
        .map_err(map_backend_error)?;
    let configured_page_cap =
        usize::try_from(configured_page_cap).map_err(|_| KeyStoreError::BackendError)?;
    if configured_page_cap > MAX_PAGE_COUNT {
        return Err(KeyStoreError::LimitExceeded);
    }
    let sqlite_length_limit = i32::try_from(MAX_STORE_VALUE_BYTES + MAX_STORE_KEY_BYTES + 1_024)
        .map_err(|_| KeyStoreError::BackendError)?;
    connection
        .set_limit(Limit::SQLITE_LIMIT_LENGTH, sqlite_length_limit)
        .map_err(map_backend_error)?;
    Ok(())
}

fn initialize_schema(connection: &Connection) -> Result<(), KeyStoreError> {
    connection
        .execute_batch(&format!(
            "CREATE TABLE IF NOT EXISTS local_secrets (
                store_key TEXT PRIMARY KEY NOT NULL
                    CHECK(length(store_key) BETWEEN 1 AND {MAX_STORE_KEY_BYTES}),
                secret_value BLOB NOT NULL
                    CHECK(length(secret_value) BETWEEN 1 AND {MAX_STORE_VALUE_BYTES})
            ) STRICT, WITHOUT ROWID;
            CREATE TRIGGER IF NOT EXISTS local_secrets_entry_cap
            BEFORE INSERT ON local_secrets
            WHEN (SELECT count(*) FROM local_secrets) >= {MAX_STORE_ENTRIES}
                 AND NOT EXISTS (
                    SELECT 1 FROM local_secrets WHERE store_key = NEW.store_key
                 )
            BEGIN
                SELECT RAISE(ABORT, 'local encrypted store entry limit exceeded');
            END;"
        ))
        .map_err(map_backend_error)
}

fn map_backend_error(_error: rusqlite::Error) -> KeyStoreError {
    KeyStoreError::BackendError
}

fn map_sqlite_limit_error(error: &rusqlite::Error) -> KeyStoreError {
    match error {
        rusqlite::Error::SqliteFailure(_, message)
            if message.as_deref().is_some_and(|message| {
                message.contains("limit exceeded") || message.contains("too big")
            }) =>
        {
            KeyStoreError::LimitExceeded
        }
        _ => KeyStoreError::BackendError,
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        sync::atomic::{AtomicUsize, Ordering},
    };

    use filament_core::{DeviceId, UserId};
    use tempfile::tempdir;

    use super::*;
    use crate::{load_root_identity, persist_root_identity, RootIdentityKey};

    struct FixedKeyProvider {
        key: [u8; STORE_ENCRYPTION_KEY_BYTES],
        calls: AtomicUsize,
    }

    impl FixedKeyProvider {
        fn new(byte: u8) -> Self {
            Self {
                key: [byte; STORE_ENCRYPTION_KEY_BYTES],
                calls: AtomicUsize::new(0),
            }
        }
    }

    impl StoreKeyProvider for FixedKeyProvider {
        fn load_or_create_key(
            &self,
            _store_id: &LocalStoreId,
        ) -> Result<Zeroizing<Vec<u8>>, KeyStoreError> {
            self.calls.fetch_add(1, Ordering::Relaxed);
            Ok(Zeroizing::new(self.key.to_vec()))
        }
    }

    fn store_id() -> LocalStoreId {
        LocalStoreId::new(UserId::new(), DeviceId::new())
    }

    #[test]
    fn encrypted_store_round_trips_and_reopens_without_plaintext_header() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("device.db");
        let provider = FixedKeyProvider::new(0x42);
        let key = StoreKey::root_identity();
        {
            let store = SqlCipherKeyStore::open(&path, &store_id(), &provider).unwrap();
            store.store(key.clone(), vec![0xAB; 32]).unwrap();
            assert_eq!(store.load(&key).unwrap().as_slice(), [0xAB; 32]);
            assert!(store.exists(&key).unwrap());
        }

        let header = fs::read(&path).unwrap();
        assert!(!header.starts_with(b"SQLite format 3\0"));
        assert!(!header.windows(32).any(|window| window == [0xAB; 32]));

        let reopened = SqlCipherKeyStore::open(&path, &store_id(), &provider).unwrap();
        assert_eq!(reopened.load(&key).unwrap().as_slice(), [0xAB; 32]);
        assert_eq!(provider.calls.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn root_identity_round_trips_through_sqlcipher() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("device.db");
        let provider = FixedKeyProvider::new(0x42);
        let store = SqlCipherKeyStore::open(&path, &store_id(), &provider).unwrap();
        let original = RootIdentityKey::generate();
        let key = StoreKey::root_identity();
        persist_root_identity(&store, key.clone(), &original).unwrap();
        let restored = load_root_identity(&store, &key).unwrap();
        assert_eq!(original.public_key_bytes(), restored.public_key_bytes());
    }

    #[test]
    fn wrong_key_symlink_and_relative_path_fail_closed() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("device.db");
        let store_id = store_id();
        let correct = FixedKeyProvider::new(0x42);
        drop(SqlCipherKeyStore::open(&path, &store_id, &correct).unwrap());
        let wrong = FixedKeyProvider::new(0x24);
        assert_eq!(
            SqlCipherKeyStore::open(&path, &store_id, &wrong).unwrap_err(),
            KeyStoreError::BackendError
        );
        assert_eq!(
            SqlCipherKeyStore::open(Path::new("relative.db"), &store_id, &correct).unwrap_err(),
            KeyStoreError::InvalidPath
        );

        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;

            let target = directory.path().join("target.db");
            fs::write(&target, []).unwrap();
            let link = directory.path().join("link.db");
            symlink(&target, &link).unwrap();
            assert_eq!(
                SqlCipherKeyStore::open(&link, &store_id, &correct).unwrap_err(),
                KeyStoreError::InvalidPath
            );

            let hard_link = directory.path().join("hard-link.db");
            fs::hard_link(&target, &hard_link).unwrap();
            assert_eq!(
                SqlCipherKeyStore::open(&hard_link, &store_id, &correct).unwrap_err(),
                KeyStoreError::InvalidPath
            );
        }

        let oversized = directory.path().join("oversized.db");
        fs::File::create(&oversized)
            .unwrap()
            .set_len(u64::try_from(MAX_ENCRYPTED_STORE_BYTES).unwrap() + 1)
            .unwrap();
        assert_eq!(
            SqlCipherKeyStore::open(&oversized, &store_id, &correct).unwrap_err(),
            KeyStoreError::LimitExceeded
        );
    }

    #[test]
    fn value_limits_and_debug_redaction_are_enforced() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("device.db");
        let provider = FixedKeyProvider::new(0x42);
        let store = SqlCipherKeyStore::open(&path, &store_id(), &provider).unwrap();
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
        assert_eq!(
            format!("{store:?}"),
            "SqlCipherKeyStore(<path and key redacted>)"
        );
        assert!(!format!("{store:?}").contains(path.to_string_lossy().as_ref()));
    }
}
