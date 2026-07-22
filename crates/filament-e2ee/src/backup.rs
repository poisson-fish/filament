//! Portable opt-in recovery backups for native E2EE clients.
//!
//! A backup contains the account root identity and a bounded snapshot of
//! authenticated local history. It deliberately excludes device signing keys,
//! OpenMLS provider state, ratchets, mailbox outboxes, and SQLCipher keys: a
//! restored client must enroll as a fresh device before receiving new epochs.
//! The caller is responsible for moving the opaque blob to user-chosen storage.

use std::collections::HashSet;

use argon2::{Algorithm, Argon2, Params, Version};
use filament_core::{GroupId, UserId};
use openmls::prelude::{AeadType, OpenMlsCrypto as _, OpenMlsProvider as _, OpenMlsRand as _};
use openmls_rust_crypto::OpenMlsRustCrypto;
use zeroize::{Zeroize, Zeroizing};

use crate::{
    durable_mailbox::{decode_stored_message, history_storage_entry},
    BackupError, KeyStoreError, LocalKeyStore, RootIdentityKey, StoreKey, MAX_STORE_ENTRIES,
    MAX_STORE_KEY_BYTES, MAX_STORE_VALUE_BYTES,
};

/// Argon2id memory cost for a portable backup: 64 MiB.
pub const ARGON2_BACKUP_MEMORY_KIB: u32 = 64 * 1_024;
/// Argon2id iteration cost for a portable backup.
pub const ARGON2_BACKUP_ITERATIONS: u32 = 3;
/// Minimum UTF-8 byte length accepted for a backup passphrase.
pub const MIN_BACKUP_PASSPHRASE_BYTES: usize = 12;
/// Maximum UTF-8 byte length accepted for a backup passphrase.
pub const MAX_BACKUP_PASSPHRASE_BYTES: usize = 1_024;
/// Maximum complete encrypted backup size.
pub const MAX_BACKUP_BLOB_BYTES: usize = 64 * 1_024 * 1_024;

const BACKUP_VERSION: u16 = 1;
const PAYLOAD_VERSION: u16 = 1;
const ARGON2_BACKUP_LANES: u32 = 1;
const BACKUP_MAGIC: &[u8; 8] = b"FLMBKP01";
const SALT_BYTES: usize = 16;
const NONCE_BYTES: usize = 12;
const CONTENT_KEY_BYTES: usize = 32;
const AEAD_TAG_BYTES: usize = 16;
const ROOT_SECRET_BYTES: usize = 32;
const ROOT_PUBLIC_BYTES: usize = 32;
const HEADER_BYTES: usize = 58;
const MAX_BACKUP_HISTORY_RECORDS: usize = MAX_STORE_ENTRIES - 1;

/// An opaque passphrase-encrypted recovery blob.
#[derive(Clone, PartialEq, Eq)]
pub struct EncryptedBackup(Vec<u8>);

impl EncryptedBackup {
    /// Borrow the opaque bytes for a native file/export boundary.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    /// Transfer ownership of the opaque bytes.
    #[must_use]
    pub fn into_bytes(self) -> Vec<u8> {
        self.0
    }
}

impl core::fmt::Debug for EncryptedBackup {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("EncryptedBackup")
            .field("bytes", &self.0.len())
            .finish_non_exhaustive()
    }
}

/// Public-only summary of a completed atomic restore.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BackupRestore {
    /// Recovered account identity fingerprint source.
    pub root_public_key: [u8; ROOT_PUBLIC_BYTES],
    /// Number of authenticated local-history records in the backup.
    pub history_records: usize,
}

/// Create a fresh non-deterministic passphrase-encrypted backup.
///
/// Only the canonical root identity and authenticated `history:*` records are
/// included. Corrupt history causes the entire snapshot to fail closed.
///
/// # Errors
/// Rejects weak/oversized passphrases, missing or corrupt identity/history,
/// snapshots beyond hard caps, and KDF/provider failures.
pub fn create_passphrase_backup(
    store: &dyn LocalKeyStore,
    user_id: UserId,
    passphrase: &str,
) -> Result<EncryptedBackup, BackupError> {
    validate_passphrase(passphrase)?;
    let root_key = StoreKey::root_identity();
    let root_secret = store.load(&root_key)?;
    let root_secret_array = Zeroizing::new(
        root_secret
            .as_slice()
            .try_into()
            .map_err(|_| BackupError::InvalidBackup)?,
    );
    let identity = RootIdentityKey::from_secret_bytes(&root_secret_array);
    let root_public_key = identity.public_key_bytes();

    let mut history_keys = store
        .list_keys()?
        .into_iter()
        .filter(|key| key.as_str().starts_with("history:"))
        .collect::<Vec<_>>();
    history_keys.sort();
    if history_keys.len() > MAX_BACKUP_HISTORY_RECORDS {
        return Err(BackupError::LimitExceeded);
    }

    let mut payload = Zeroizing::new(Vec::new());
    push_u16(&mut payload, PAYLOAD_VERSION);
    push_sized_u16(&mut payload, user_id.to_string().as_bytes())?;
    payload.extend_from_slice(root_secret_array.as_ref());
    payload.extend_from_slice(&root_public_key);
    push_u32(
        &mut payload,
        u32::try_from(history_keys.len()).map_err(|_| BackupError::LimitExceeded)?,
    );

    for key in history_keys {
        let (group_id, message_id) = parse_history_key(&key)?;
        let value = store.load(&key)?;
        validate_canonical_history(group_id, message_id, key.as_str(), &value)?;
        push_sized_u16(&mut payload, key.as_str().as_bytes())?;
        push_sized_u32(&mut payload, &value)?;
        if payload
            .len()
            .checked_add(HEADER_BYTES + AEAD_TAG_BYTES)
            .is_none_or(|length| length > MAX_BACKUP_BLOB_BYTES)
        {
            return Err(BackupError::LimitExceeded);
        }
    }

    let provider = OpenMlsRustCrypto::default();
    let salt = provider
        .rand()
        .random_array::<SALT_BYTES>()
        .map_err(|_| BackupError::CryptoError)?;
    let nonce = provider
        .rand()
        .random_array::<NONCE_BYTES>()
        .map_err(|_| BackupError::CryptoError)?;
    let content_key = derive_content_key(passphrase, &salt)?;
    let ciphertext_len = payload
        .len()
        .checked_add(AEAD_TAG_BYTES)
        .ok_or(BackupError::LimitExceeded)?;
    let header = encode_header(&salt, &nonce, ciphertext_len)?;
    let ciphertext = provider
        .crypto()
        .aead_encrypt(
            AeadType::ChaCha20Poly1305,
            &content_key,
            &payload,
            &nonce,
            &header,
        )
        .map_err(|_| BackupError::CryptoError)?;
    if ciphertext.len() != ciphertext_len {
        return Err(BackupError::CryptoError);
    }
    let mut blob = Vec::with_capacity(header.len() + ciphertext.len());
    blob.extend_from_slice(&header);
    blob.extend_from_slice(&ciphertext);
    if blob.len() > MAX_BACKUP_BLOB_BYTES {
        blob.zeroize();
        return Err(BackupError::LimitExceeded);
    }
    Ok(EncryptedBackup(blob))
}

/// Authenticate, validate, and atomically restore a portable backup.
///
/// Exact local records are idempotent. A different root identity or any
/// conflicting history record rejects the whole transaction without partial
/// writes. The expected account ID must come from the authenticated native
/// session, not from an untrusted UI field.
///
/// # Errors
/// Rejects wrong passphrases, tampering, cross-account restores, malformed or
/// non-canonical history, local conflicts, limits, and persistence failures.
pub fn restore_passphrase_backup(
    store: &dyn LocalKeyStore,
    expected_user_id: UserId,
    passphrase: &str,
    blob: &[u8],
) -> Result<BackupRestore, BackupError> {
    validate_passphrase(passphrase)?;
    let header = parse_header(blob)?;
    let content_key = derive_content_key(passphrase, &header.salt)?;
    let provider = OpenMlsRustCrypto::default();
    let plaintext = provider
        .crypto()
        .aead_decrypt(
            AeadType::ChaCha20Poly1305,
            &content_key,
            header.ciphertext,
            &header.nonce,
            header.encoded,
        )
        .map_err(|_| BackupError::AuthenticationFailed)?;
    let mut plaintext = Zeroizing::new(plaintext);
    let decoded = decode_payload(&mut plaintext, expected_user_id)?;
    let summary = BackupRestore {
        root_public_key: decoded.root_public_key,
        history_records: decoded.entries.len() - 1,
    };
    match store.restore_backup_batch(decoded.entries.into_inner()) {
        Ok(_) => Ok(summary),
        Err(KeyStoreError::InvalidValue) => Err(BackupError::Conflict),
        Err(KeyStoreError::LimitExceeded) => Err(BackupError::LimitExceeded),
        Err(error) => Err(BackupError::KeyStore(error)),
    }
}

struct ParsedHeader<'a> {
    encoded: &'a [u8],
    salt: [u8; SALT_BYTES],
    nonce: [u8; NONCE_BYTES],
    ciphertext: &'a [u8],
}

struct DecodedPayload {
    root_public_key: [u8; ROOT_PUBLIC_BYTES],
    entries: SensitiveEntries,
}

struct SensitiveEntries(Vec<(StoreKey, Vec<u8>)>);

impl SensitiveEntries {
    fn with_capacity(capacity: usize) -> Self {
        Self(Vec::with_capacity(capacity))
    }

    fn push(&mut self, entry: (StoreKey, Vec<u8>)) {
        self.0.push(entry);
    }

    fn len(&self) -> usize {
        self.0.len()
    }

    fn into_inner(mut self) -> Vec<(StoreKey, Vec<u8>)> {
        core::mem::take(&mut self.0)
    }
}

impl Drop for SensitiveEntries {
    fn drop(&mut self) {
        for (_, value) in &mut self.0 {
            value.zeroize();
        }
    }
}

fn validate_passphrase(passphrase: &str) -> Result<(), BackupError> {
    if (MIN_BACKUP_PASSPHRASE_BYTES..=MAX_BACKUP_PASSPHRASE_BYTES).contains(&passphrase.len()) {
        Ok(())
    } else {
        Err(BackupError::InvalidPassphrase)
    }
}

fn derive_content_key(
    passphrase: &str,
    salt: &[u8; SALT_BYTES],
) -> Result<Zeroizing<Vec<u8>>, BackupError> {
    let params = Params::new(
        ARGON2_BACKUP_MEMORY_KIB,
        ARGON2_BACKUP_ITERATIONS,
        ARGON2_BACKUP_LANES,
        Some(CONTENT_KEY_BYTES),
    )
    .map_err(|_| BackupError::CryptoError)?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut key = Zeroizing::new(vec![0_u8; CONTENT_KEY_BYTES]);
    argon2
        .hash_password_into(passphrase.as_bytes(), salt, &mut key)
        .map_err(|_| BackupError::CryptoError)?;
    Ok(key)
}

fn encode_header(
    salt: &[u8; SALT_BYTES],
    nonce: &[u8; NONCE_BYTES],
    ciphertext_len: usize,
) -> Result<Vec<u8>, BackupError> {
    if ciphertext_len < AEAD_TAG_BYTES
        || ciphertext_len
            .checked_add(HEADER_BYTES)
            .is_none_or(|length| length > MAX_BACKUP_BLOB_BYTES)
    {
        return Err(BackupError::LimitExceeded);
    }
    let mut header = Vec::with_capacity(HEADER_BYTES);
    header.extend_from_slice(BACKUP_MAGIC);
    push_u16(&mut header, BACKUP_VERSION);
    push_u32(&mut header, ARGON2_BACKUP_MEMORY_KIB);
    push_u32(&mut header, ARGON2_BACKUP_ITERATIONS);
    push_u32(&mut header, ARGON2_BACKUP_LANES);
    header.extend_from_slice(salt);
    header.extend_from_slice(nonce);
    header.extend_from_slice(
        &u64::try_from(ciphertext_len)
            .map_err(|_| BackupError::LimitExceeded)?
            .to_le_bytes(),
    );
    if header.len() != HEADER_BYTES {
        return Err(BackupError::InvalidBackup);
    }
    Ok(header)
}

fn parse_header(blob: &[u8]) -> Result<ParsedHeader<'_>, BackupError> {
    if blob.len() < HEADER_BYTES + AEAD_TAG_BYTES || blob.len() > MAX_BACKUP_BLOB_BYTES {
        return Err(BackupError::InvalidBackup);
    }
    if &blob[..8] != BACKUP_MAGIC
        || read_u16(&blob[8..10])? != BACKUP_VERSION
        || read_u32(&blob[10..14])? != ARGON2_BACKUP_MEMORY_KIB
        || read_u32(&blob[14..18])? != ARGON2_BACKUP_ITERATIONS
        || read_u32(&blob[18..22])? != ARGON2_BACKUP_LANES
    {
        return Err(BackupError::InvalidBackup);
    }
    let salt = blob[22..38]
        .try_into()
        .map_err(|_| BackupError::InvalidBackup)?;
    let nonce = blob[38..50]
        .try_into()
        .map_err(|_| BackupError::InvalidBackup)?;
    let ciphertext_len =
        usize::try_from(read_u64(&blob[50..58])?).map_err(|_| BackupError::InvalidBackup)?;
    if ciphertext_len < AEAD_TAG_BYTES || ciphertext_len != blob.len() - HEADER_BYTES {
        return Err(BackupError::InvalidBackup);
    }
    Ok(ParsedHeader {
        encoded: &blob[..HEADER_BYTES],
        salt,
        nonce,
        ciphertext: &blob[HEADER_BYTES..],
    })
}

fn decode_payload(
    plaintext: &mut Zeroizing<Vec<u8>>,
    expected_user_id: UserId,
) -> Result<DecodedPayload, BackupError> {
    let mut cursor = Cursor::new(plaintext);
    if cursor.read_u16()? != PAYLOAD_VERSION {
        return Err(BackupError::InvalidBackup);
    }
    let encoded_user_id = cursor.read_sized_u16(MAX_STORE_KEY_BYTES)?;
    let encoded_user_id =
        core::str::from_utf8(encoded_user_id).map_err(|_| BackupError::InvalidBackup)?;
    let user_id =
        UserId::try_from(encoded_user_id.to_owned()).map_err(|_| BackupError::InvalidBackup)?;
    if user_id != expected_user_id || user_id.to_string() != encoded_user_id {
        return Err(BackupError::UserMismatch);
    }
    let root_secret = Zeroizing::new(
        cursor
            .read_exact(ROOT_SECRET_BYTES)?
            .try_into()
            .map_err(|_| BackupError::InvalidBackup)?,
    );
    let encoded_root_public: [u8; ROOT_PUBLIC_BYTES] = cursor
        .read_exact(ROOT_PUBLIC_BYTES)?
        .try_into()
        .map_err(|_| BackupError::InvalidBackup)?;
    let identity = RootIdentityKey::from_secret_bytes(&root_secret);
    let root_public_key = identity.public_key_bytes();
    if root_public_key != encoded_root_public {
        return Err(BackupError::InvalidBackup);
    }
    let record_count =
        usize::try_from(cursor.read_u32()?).map_err(|_| BackupError::LimitExceeded)?;
    if record_count > MAX_BACKUP_HISTORY_RECORDS {
        return Err(BackupError::LimitExceeded);
    }
    let mut entries = SensitiveEntries::with_capacity(record_count + 1);
    entries.push((StoreKey::root_identity(), root_secret.to_vec()));
    let mut seen = HashSet::with_capacity(record_count);
    let mut previous_key: Option<String> = None;
    for _ in 0..record_count {
        let key_bytes = cursor.read_sized_u16(MAX_STORE_KEY_BYTES)?;
        let key_string = core::str::from_utf8(key_bytes)
            .map_err(|_| BackupError::InvalidBackup)?
            .to_owned();
        if previous_key
            .as_ref()
            .is_some_and(|previous| previous >= &key_string)
            || !seen.insert(key_string.clone())
        {
            return Err(BackupError::InvalidBackup);
        }
        let key = StoreKey::new(key_string.clone()).map_err(|_| BackupError::InvalidBackup)?;
        let (group_id, message_id) = parse_history_key(&key)?;
        let value = cursor.read_sized_u32(MAX_STORE_VALUE_BYTES)?.to_vec();
        validate_canonical_history(group_id, message_id, &key_string, &value)?;
        entries.push((key, value));
        previous_key = Some(key_string);
    }
    if !cursor.is_finished() {
        return Err(BackupError::InvalidBackup);
    }
    Ok(DecodedPayload {
        root_public_key,
        entries,
    })
}

fn parse_history_key(key: &StoreKey) -> Result<(GroupId, &str), BackupError> {
    let remainder = key
        .as_str()
        .strip_prefix("history:")
        .ok_or(BackupError::InvalidBackup)?;
    let (group_id, message_id) = remainder
        .split_once(':')
        .ok_or(BackupError::InvalidBackup)?;
    if message_id.contains(':') {
        return Err(BackupError::InvalidBackup);
    }
    let group_id =
        GroupId::try_from(group_id.to_owned()).map_err(|_| BackupError::InvalidBackup)?;
    Ok((group_id, message_id))
}

fn validate_canonical_history(
    group_id: GroupId,
    message_id: &str,
    expected_key: &str,
    encoded: &[u8],
) -> Result<(), BackupError> {
    let mut message = decode_stored_message(group_id, message_id, encoded)
        .map_err(|_| BackupError::InvalidBackup)?;
    let (canonical_key, mut canonical_value) =
        history_storage_entry(&message).map_err(|_| BackupError::InvalidBackup)?;
    let valid = canonical_key.as_str() == expected_key && canonical_value.as_slice() == encoded;
    message.message.plaintext.zeroize();
    canonical_value.zeroize();
    if valid {
        Ok(())
    } else {
        Err(BackupError::InvalidBackup)
    }
}

fn push_sized_u16(target: &mut Vec<u8>, value: &[u8]) -> Result<(), BackupError> {
    if value.is_empty() || value.len() > usize::from(u16::MAX) {
        return Err(BackupError::LimitExceeded);
    }
    push_u16(
        target,
        u16::try_from(value.len()).map_err(|_| BackupError::LimitExceeded)?,
    );
    target.extend_from_slice(value);
    Ok(())
}

fn push_sized_u32(target: &mut Vec<u8>, value: &[u8]) -> Result<(), BackupError> {
    if value.is_empty() || value.len() > MAX_STORE_VALUE_BYTES {
        return Err(BackupError::LimitExceeded);
    }
    push_u32(
        target,
        u32::try_from(value.len()).map_err(|_| BackupError::LimitExceeded)?,
    );
    target.extend_from_slice(value);
    Ok(())
}

fn push_u16(target: &mut Vec<u8>, value: u16) {
    target.extend_from_slice(&value.to_le_bytes());
}

fn push_u32(target: &mut Vec<u8>, value: u32) {
    target.extend_from_slice(&value.to_le_bytes());
}

fn read_u16(bytes: &[u8]) -> Result<u16, BackupError> {
    Ok(u16::from_le_bytes(
        bytes.try_into().map_err(|_| BackupError::InvalidBackup)?,
    ))
}

fn read_u32(bytes: &[u8]) -> Result<u32, BackupError> {
    Ok(u32::from_le_bytes(
        bytes.try_into().map_err(|_| BackupError::InvalidBackup)?,
    ))
}

fn read_u64(bytes: &[u8]) -> Result<u64, BackupError> {
    Ok(u64::from_le_bytes(
        bytes.try_into().map_err(|_| BackupError::InvalidBackup)?,
    ))
}

struct Cursor<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> Cursor<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, position: 0 }
    }

    fn read_exact(&mut self, length: usize) -> Result<&'a [u8], BackupError> {
        let end = self
            .position
            .checked_add(length)
            .ok_or(BackupError::InvalidBackup)?;
        let value = self
            .bytes
            .get(self.position..end)
            .ok_or(BackupError::InvalidBackup)?;
        self.position = end;
        Ok(value)
    }

    fn read_u16(&mut self) -> Result<u16, BackupError> {
        read_u16(self.read_exact(2)?)
    }

    fn read_u32(&mut self) -> Result<u32, BackupError> {
        read_u32(self.read_exact(4)?)
    }

    fn read_sized_u16(&mut self, maximum: usize) -> Result<&'a [u8], BackupError> {
        let length = usize::from(self.read_u16()?);
        if length == 0 || length > maximum {
            return Err(BackupError::LimitExceeded);
        }
        self.read_exact(length)
    }

    fn read_sized_u32(&mut self, maximum: usize) -> Result<&'a [u8], BackupError> {
        let length = usize::try_from(self.read_u32()?).map_err(|_| BackupError::LimitExceeded)?;
        if length == 0 || length > maximum {
            return Err(BackupError::LimitExceeded);
        }
        self.read_exact(length)
    }

    const fn is_finished(&self) -> bool {
        self.position == self.bytes.len()
    }
}

#[cfg(test)]
mod tests {
    use filament_core::{DeviceId, GroupId};
    use ulid::Ulid;

    use super::*;
    use crate::{
        durable_mailbox::history_storage_entry, persist_root_identity, DecryptedApplicationMessage,
        InMemoryKeyStore, StoredMailboxMessage,
    };

    const PASSPHRASE: &str = "correct horse battery staple";

    struct BackupFixture {
        store: InMemoryKeyStore,
        user_id: UserId,
        root_public: [u8; ROOT_PUBLIC_BYTES],
        history: Vec<(StoreKey, Vec<u8>)>,
    }

    fn history_entry(
        group_id: GroupId,
        sender_user_id: UserId,
        plaintext: &[u8],
    ) -> (StoreKey, Vec<u8>) {
        history_storage_entry(&StoredMailboxMessage {
            message_id: Ulid::new().to_string(),
            group_id,
            created_at_unix: 1_700_000_000,
            message: DecryptedApplicationMessage {
                sender_user_id,
                sender_device_id: DeviceId::new(),
                generation: 0,
                plaintext: plaintext.to_vec(),
            },
        })
        .unwrap()
    }

    fn source_fixture() -> BackupFixture {
        let store = InMemoryKeyStore::new();
        let user_id = UserId::new();
        let root = RootIdentityKey::generate();
        let root_public = root.public_key_bytes();
        persist_root_identity(&store, StoreKey::root_identity(), &root).unwrap();
        let group_id = GroupId::new();
        let mut history = vec![
            history_entry(group_id, user_id, b"first recovered message"),
            history_entry(group_id, user_id, b"second recovered message"),
        ];
        history.sort_by(|left, right| left.0.cmp(&right.0));
        store.store_batch(history.clone()).unwrap();
        store
            .store(StoreKey::mls_client_state(), vec![0xA5; 32])
            .unwrap();
        store
            .store(StoreKey::new("mailbox:message_ack:test").unwrap(), vec![1])
            .unwrap();
        BackupFixture {
            store,
            user_id,
            root_public,
            history,
        }
    }

    #[test]
    fn backup_restores_identity_and_history_but_not_device_state() {
        let fixture = source_fixture();
        let backup = create_passphrase_backup(&fixture.store, fixture.user_id, PASSPHRASE).unwrap();
        let independently_salted =
            create_passphrase_backup(&fixture.store, fixture.user_id, PASSPHRASE).unwrap();
        assert_ne!(backup, independently_salted);
        assert!(backup.as_bytes().len() < MAX_BACKUP_BLOB_BYTES);
        assert!(!backup
            .as_bytes()
            .windows(b"first recovered message".len())
            .any(|window| window == b"first recovered message"));
        assert!(!format!("{backup:?}").contains(PASSPHRASE));

        let destination = InMemoryKeyStore::new();
        let restored =
            restore_passphrase_backup(&destination, fixture.user_id, PASSPHRASE, backup.as_bytes())
                .unwrap();
        assert_eq!(restored.root_public_key, fixture.root_public);
        assert_eq!(restored.history_records, fixture.history.len());
        assert_eq!(
            restore_passphrase_backup(
                &destination,
                fixture.user_id,
                PASSPHRASE,
                backup.as_bytes(),
            )
            .unwrap(),
            restored
        );
        assert_eq!(
            crate::load_root_identity(&destination, &StoreKey::root_identity())
                .unwrap()
                .public_key_bytes(),
            fixture.root_public
        );
        for (key, value) in fixture.history {
            assert_eq!(destination.load(&key).unwrap().as_slice(), value);
        }
        assert!(!destination.exists(&StoreKey::mls_client_state()).unwrap());
        assert!(!destination
            .exists(&StoreKey::new("mailbox:message_ack:test").unwrap())
            .unwrap());

        let second = InMemoryKeyStore::new();
        assert_eq!(
            restore_passphrase_backup(
                &second,
                fixture.user_id,
                "wrong passphrase value",
                backup.as_bytes()
            ),
            Err(BackupError::AuthenticationFailed)
        );
        let mut tampered = backup.as_bytes().to_vec();
        *tampered.last_mut().unwrap() ^= 1;
        assert_eq!(
            restore_passphrase_backup(&second, fixture.user_id, PASSPHRASE, &tampered),
            Err(BackupError::AuthenticationFailed)
        );
        assert_eq!(
            restore_passphrase_backup(&second, UserId::new(), PASSPHRASE, backup.as_bytes()),
            Err(BackupError::UserMismatch)
        );
        let mut downgraded = backup.as_bytes().to_vec();
        downgraded[10..14].copy_from_slice(&1_u32.to_le_bytes());
        assert_eq!(
            restore_passphrase_backup(&second, fixture.user_id, PASSPHRASE, &downgraded),
            Err(BackupError::InvalidBackup)
        );
    }

    #[test]
    fn conflicting_history_rolls_back_the_complete_restore() {
        let fixture = source_fixture();
        let backup = create_passphrase_backup(&fixture.store, fixture.user_id, PASSPHRASE).unwrap();
        let destination = InMemoryKeyStore::new();
        let root = fixture
            .store
            .load(&StoreKey::root_identity())
            .unwrap()
            .to_vec();
        destination.store(StoreKey::root_identity(), root).unwrap();
        destination
            .store(fixture.history[1].0.clone(), vec![0xFF])
            .unwrap();

        assert_eq!(
            restore_passphrase_backup(&destination, fixture.user_id, PASSPHRASE, backup.as_bytes(),),
            Err(BackupError::Conflict)
        );
        assert!(!destination.exists(&fixture.history[0].0).unwrap());
        assert_eq!(
            destination.load(&fixture.history[1].0).unwrap().as_slice(),
            [0xFF]
        );
    }

    #[test]
    fn passphrase_and_blob_limits_fail_before_crypto() {
        let fixture = source_fixture();
        assert_eq!(
            create_passphrase_backup(&fixture.store, fixture.user_id, "too short"),
            Err(BackupError::InvalidPassphrase)
        );
        assert_eq!(
            restore_passphrase_backup(&fixture.store, fixture.user_id, PASSPHRASE, &[]),
            Err(BackupError::InvalidBackup)
        );
        assert_eq!(
            restore_passphrase_backup(
                &fixture.store,
                fixture.user_id,
                PASSPHRASE,
                &vec![0_u8; MAX_BACKUP_BLOB_BYTES + 1],
            ),
            Err(BackupError::InvalidBackup)
        );
    }
}
