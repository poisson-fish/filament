//! Client-side encryption for attachments in MLS conversations.
//!
//! The server receives only an exact-bucket ciphertext. The random content
//! key, nonce, filename, MIME type, plaintext size, and content hash are kept
//! in [`AttachmentDescriptor`] and must travel inside an authenticated MLS
//! application event. Decryption verifies every descriptor field before any
//! content is exposed to the native UI.

use std::collections::HashSet;

use openmls::prelude::{AeadType, OpenMlsCrypto as _, OpenMlsProvider as _, OpenMlsRand as _};
use openmls_rust_crypto::OpenMlsRustCrypto;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use ulid::Ulid;
use zeroize::{Zeroize, Zeroizing};

use crate::AttachmentError;
pub use filament_protocol::{
    E2EE_ATTACHMENT_CIPHERTEXT_BUCKETS as ATTACHMENT_CIPHERTEXT_BUCKETS,
    MAX_E2EE_ATTACHMENT_BYTES as MAX_ENCRYPTED_ATTACHMENT_BYTES,
};

const ATTACHMENT_PROTOCOL_VERSION: u16 = 1;
const CONTENT_KEY_BYTES: usize = 32;
const NONCE_BYTES: usize = 12;
const AEAD_TAG_BYTES: usize = 16;
const CONTENT_HASH_BYTES: usize = 32;
const ATTACHMENT_AAD_DOMAIN: &[u8] = b"filament:e2ee:attachment:v1";

/// Maximum original attachment size accepted by the native client.
pub const MAX_ATTACHMENT_BYTES: usize = 24 * 1_024 * 1_024;
/// Maximum original thumbnail size accepted by the native client.
pub const MAX_THUMBNAIL_BYTES: usize = 1_024 * 1_024;
/// Maximum UTF-8 bytes in a private attachment filename.
pub const MAX_ATTACHMENT_FILENAME_BYTES: usize = 128;
/// Maximum UTF-8 bytes in a private MIME type.
pub const MAX_ATTACHMENT_MIME_BYTES: usize = 64;

/// Canonical client-generated attachment identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct AttachmentId(Ulid);

impl AttachmentId {
    /// Generate an unpredictable attachment identifier.
    #[must_use]
    pub fn new() -> Self {
        Self(Ulid::new())
    }
}

impl Default for AttachmentId {
    fn default() -> Self {
        Self::new()
    }
}

impl TryFrom<String> for AttachmentId {
    type Error = AttachmentError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        let parsed = Ulid::from_string(&value).map_err(|_| AttachmentError::InvalidAttachment)?;
        if parsed.to_string() != value {
            return Err(AttachmentError::InvalidAttachment);
        }
        Ok(Self(parsed))
    }
}

impl From<AttachmentId> for String {
    fn from(value: AttachmentId) -> Self {
        value.0.to_string()
    }
}

impl core::fmt::Display for AttachmentId {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

/// Whether an encrypted object is the user-selected file or its local thumbnail.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttachmentKind {
    /// Original user-selected content.
    File,
    /// Client-generated preview content.
    Thumbnail,
}

impl AttachmentKind {
    const fn marker(self) -> u8 {
        match self {
            Self::File => 1,
            Self::Thumbnail => 2,
        }
    }

    const fn max_plaintext_bytes(self) -> usize {
        match self {
            Self::File => MAX_ATTACHMENT_BYTES,
            Self::Thumbnail => MAX_THUMBNAIL_BYTES,
        }
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
struct AttachmentContentKey([u8; CONTENT_KEY_BYTES]);

impl core::fmt::Debug for AttachmentContentKey {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("<redacted attachment key>")
    }
}

impl Drop for AttachmentContentKey {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

/// Private metadata authenticated by the attachment AEAD and carried inside MLS.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AttachmentDescriptor {
    version: u16,
    /// Identifier used to bind the opaque upload to the MLS event.
    pub attachment_id: AttachmentId,
    /// Separates original files from client-generated thumbnails.
    pub kind: AttachmentKind,
    /// Private display filename, never sent as upload metadata.
    pub filename: String,
    /// Client-sniffed MIME type, verified again after decryption.
    pub mime_type: String,
    /// Unpadded plaintext byte length.
    pub plaintext_size: u64,
    /// SHA-256 of the unpadded plaintext, verified after AEAD authentication.
    pub content_hash: [u8; CONTENT_HASH_BYTES],
    content_key: AttachmentContentKey,
    nonce: [u8; NONCE_BYTES],
}

impl AttachmentDescriptor {
    /// Validate a descriptor decoded from an authenticated MLS event.
    ///
    /// # Errors
    /// Rejects unsupported versions, malformed metadata, or impossible sizes.
    pub fn validate(&self) -> Result<(), AttachmentError> {
        if self.version != ATTACHMENT_PROTOCOL_VERSION
            || !valid_filename(&self.filename)
            || !valid_mime(&self.mime_type)
        {
            return Err(AttachmentError::InvalidAttachment);
        }
        let plaintext_size =
            usize::try_from(self.plaintext_size).map_err(|_| AttachmentError::LimitExceeded)?;
        if plaintext_size == 0 || plaintext_size > self.kind.max_plaintext_bytes() {
            return Err(AttachmentError::LimitExceeded);
        }
        Ok(())
    }
}

/// Opaque exact-bucket object safe to upload to the Delivery Service.
#[derive(Clone, PartialEq, Eq)]
pub struct EncryptedAttachment {
    /// Identifier matching the private MLS descriptor.
    pub attachment_id: AttachmentId,
    /// AEAD ciphertext. Its length is always one approved transport bucket.
    pub ciphertext: Vec<u8>,
}

impl core::fmt::Debug for EncryptedAttachment {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("EncryptedAttachment")
            .field("attachment_id", &self.attachment_id)
            .field("ciphertext_bytes", &self.ciphertext.len())
            .finish()
    }
}

/// Decrypted attachment content returned only after all verification succeeds.
pub struct AttachmentContent {
    /// Private display filename.
    pub filename: String,
    /// Verified client-sniffed MIME type.
    pub mime_type: String,
    /// Sensitive plaintext, zeroized when dropped.
    pub bytes: Zeroizing<Vec<u8>>,
}

impl core::fmt::Debug for AttachmentContent {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("AttachmentContent")
            .field("filename", &self.filename)
            .field("mime_type", &self.mime_type)
            .field("plaintext_bytes", &self.bytes.len())
            .finish()
    }
}

/// Encrypt one attachment with a fresh random key and exact size padding.
///
/// # Errors
/// Rejects empty/oversized content and unsafe filenames. Provider failures do
/// not return partial ciphertext or key material.
pub fn encrypt_attachment(
    filename: impl Into<String>,
    plaintext: &[u8],
) -> Result<(AttachmentDescriptor, EncryptedAttachment), AttachmentError> {
    encrypt_object(AttachmentKind::File, filename.into(), plaintext)
}

/// Encrypt a locally generated thumbnail with its own independent key.
///
/// # Errors
/// Applies the thumbnail-specific size cap and all attachment invariants.
pub fn encrypt_thumbnail(
    filename: impl Into<String>,
    plaintext: &[u8],
) -> Result<(AttachmentDescriptor, EncryptedAttachment), AttachmentError> {
    encrypt_object(AttachmentKind::Thumbnail, filename.into(), plaintext)
}

fn encrypt_object(
    kind: AttachmentKind,
    filename: String,
    plaintext: &[u8],
) -> Result<(AttachmentDescriptor, EncryptedAttachment), AttachmentError> {
    if !valid_filename(&filename) || plaintext.is_empty() {
        return Err(AttachmentError::InvalidAttachment);
    }
    if plaintext.len() > kind.max_plaintext_bytes() {
        return Err(AttachmentError::LimitExceeded);
    }
    let ciphertext_bucket = ciphertext_bucket(plaintext.len())?;
    let provider = OpenMlsRustCrypto::default();
    let content_key = provider
        .rand()
        .random_array::<CONTENT_KEY_BYTES>()
        .map_err(|_| AttachmentError::CryptoError)?;
    let nonce = provider
        .rand()
        .random_array::<NONCE_BYTES>()
        .map_err(|_| AttachmentError::CryptoError)?;
    let attachment_id = AttachmentId::new();
    let mime_type = sniff_mime(plaintext);
    let content_hash: [u8; CONTENT_HASH_BYTES] = Sha256::digest(plaintext).into();
    let descriptor = AttachmentDescriptor {
        version: ATTACHMENT_PROTOCOL_VERSION,
        attachment_id,
        kind,
        filename,
        mime_type,
        plaintext_size: u64::try_from(plaintext.len())
            .map_err(|_| AttachmentError::LimitExceeded)?,
        content_hash,
        content_key: AttachmentContentKey(content_key),
        nonce,
    };
    let mut padded = Zeroizing::new(vec![0_u8; ciphertext_bucket - AEAD_TAG_BYTES]);
    padded[..plaintext.len()].copy_from_slice(plaintext);
    let aad = descriptor_aad(&descriptor)?;
    let ciphertext = provider
        .crypto()
        .aead_encrypt(
            AeadType::ChaCha20Poly1305,
            &descriptor.content_key.0,
            &padded,
            &descriptor.nonce,
            &aad,
        )
        .map_err(|_| AttachmentError::CryptoError)?;
    if ciphertext.len() != ciphertext_bucket {
        return Err(AttachmentError::CryptoError);
    }
    Ok((
        descriptor,
        EncryptedAttachment {
            attachment_id,
            ciphertext,
        },
    ))
}

/// Authenticate, decrypt, unpad, and verify one downloaded attachment.
///
/// # Errors
/// Fails closed on identifier mismatch, non-bucket sizes, AEAD failure,
/// non-zero padding, hash mismatch, or MIME mismatch.
pub fn decrypt_attachment(
    descriptor: &AttachmentDescriptor,
    encrypted: &EncryptedAttachment,
) -> Result<AttachmentContent, AttachmentError> {
    descriptor.validate()?;
    if encrypted.attachment_id != descriptor.attachment_id
        || !ATTACHMENT_CIPHERTEXT_BUCKETS.contains(&encrypted.ciphertext.len())
    {
        return Err(AttachmentError::InvalidAttachment);
    }
    let plaintext_size =
        usize::try_from(descriptor.plaintext_size).map_err(|_| AttachmentError::LimitExceeded)?;
    let padded_size = encrypted
        .ciphertext
        .len()
        .checked_sub(AEAD_TAG_BYTES)
        .ok_or(AttachmentError::InvalidAttachment)?;
    if plaintext_size > padded_size
        || ciphertext_bucket(plaintext_size)? != encrypted.ciphertext.len()
    {
        return Err(AttachmentError::InvalidAttachment);
    }
    let provider = OpenMlsRustCrypto::default();
    let aad = descriptor_aad(descriptor)?;
    let mut padded = Zeroizing::new(
        provider
            .crypto()
            .aead_decrypt(
                AeadType::ChaCha20Poly1305,
                &descriptor.content_key.0,
                &encrypted.ciphertext,
                &descriptor.nonce,
                &aad,
            )
            .map_err(|_| AttachmentError::VerificationFailed)?,
    );
    if padded.len() != padded_size || padded[plaintext_size..].iter().any(|byte| *byte != 0) {
        return Err(AttachmentError::VerificationFailed);
    }
    padded.truncate(plaintext_size);
    if Sha256::digest(padded.as_slice()).as_slice() != descriptor.content_hash
        || sniff_mime(&padded) != descriptor.mime_type
    {
        return Err(AttachmentError::VerificationFailed);
    }
    Ok(AttachmentContent {
        filename: descriptor.filename.clone(),
        mime_type: descriptor.mime_type.clone(),
        bytes: padded,
    })
}

fn ciphertext_bucket(plaintext_size: usize) -> Result<usize, AttachmentError> {
    let required = plaintext_size
        .checked_add(AEAD_TAG_BYTES)
        .ok_or(AttachmentError::LimitExceeded)?;
    ATTACHMENT_CIPHERTEXT_BUCKETS
        .iter()
        .copied()
        .find(|bucket| required <= *bucket)
        .ok_or(AttachmentError::LimitExceeded)
}

fn valid_filename(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_ATTACHMENT_FILENAME_BYTES
        && value != "."
        && value != ".."
        && !value
            .chars()
            .any(|character| character.is_control() || matches!(character, '/' | '\\' | ':' | '\0'))
}

fn valid_mime(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_ATTACHMENT_MIME_BYTES
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'-' | b'+' | b'.'))
        && value
            .split_once('/')
            .is_some_and(|(top, sub)| !top.is_empty() && !sub.is_empty())
}

fn sniff_mime(plaintext: &[u8]) -> String {
    infer::get(plaintext).map_or_else(
        || String::from("application/octet-stream"),
        |kind| kind.mime_type().to_owned(),
    )
}

fn descriptor_aad(
    descriptor: &AttachmentDescriptor,
) -> Result<Zeroizing<Vec<u8>>, AttachmentError> {
    descriptor.validate()?;
    let filename_len =
        u16::try_from(descriptor.filename.len()).map_err(|_| AttachmentError::InvalidAttachment)?;
    let mime_len =
        u8::try_from(descriptor.mime_type.len()).map_err(|_| AttachmentError::InvalidAttachment)?;
    let mut aad = Zeroizing::new(Vec::with_capacity(
        ATTACHMENT_AAD_DOMAIN.len()
            + 2
            + 26
            + 1
            + 2
            + descriptor.filename.len()
            + 1
            + descriptor.mime_type.len()
            + 8
            + CONTENT_HASH_BYTES,
    ));
    aad.extend_from_slice(ATTACHMENT_AAD_DOMAIN);
    aad.extend_from_slice(&descriptor.version.to_be_bytes());
    aad.extend_from_slice(descriptor.attachment_id.to_string().as_bytes());
    aad.push(descriptor.kind.marker());
    aad.extend_from_slice(&filename_len.to_be_bytes());
    aad.extend_from_slice(descriptor.filename.as_bytes());
    aad.push(mime_len);
    aad.extend_from_slice(descriptor.mime_type.as_bytes());
    aad.extend_from_slice(&descriptor.plaintext_size.to_be_bytes());
    aad.extend_from_slice(&descriptor.content_hash);
    Ok(aad)
}

pub(crate) fn validate_attachment_ids<'a>(
    descriptors: impl IntoIterator<Item = &'a AttachmentDescriptor>,
) -> Result<(), AttachmentError> {
    let mut seen = HashSet::new();
    for descriptor in descriptors {
        descriptor.validate()?;
        if !seen.insert(descriptor.attachment_id) {
            return Err(AttachmentError::InvalidAttachment);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDRfixture";

    #[test]
    fn round_trip_verifies_content_metadata_and_padding_bucket() {
        let (descriptor, encrypted) = encrypt_attachment("photo.png", PNG).unwrap();
        assert_eq!(descriptor.mime_type, "image/png");
        assert_eq!(encrypted.ciphertext.len(), ATTACHMENT_CIPHERTEXT_BUCKETS[0]);
        assert!(!encrypted
            .ciphertext
            .windows(PNG.len())
            .any(|window| window == PNG));

        let decrypted = decrypt_attachment(&descriptor, &encrypted).unwrap();
        assert_eq!(decrypted.filename, "photo.png");
        assert_eq!(decrypted.mime_type, "image/png");
        assert_eq!(decrypted.bytes.as_slice(), PNG);
    }

    #[test]
    fn encryption_is_random_and_never_convergent() {
        let (first_descriptor, first) = encrypt_attachment("same.bin", b"same bytes").unwrap();
        let (second_descriptor, second) = encrypt_attachment("same.bin", b"same bytes").unwrap();
        assert_ne!(first.attachment_id, second.attachment_id);
        assert_ne!(first.ciphertext, second.ciphertext);
        assert_ne!(first_descriptor.content_key, second_descriptor.content_key);
        assert_ne!(first_descriptor.nonce, second_descriptor.nonce);
    }

    #[test]
    fn debug_output_redacts_content_keys_and_plaintext() {
        let (descriptor, encrypted) = encrypt_attachment("secret.bin", b"secret bytes").unwrap();
        let descriptor_debug = format!("{descriptor:?}");
        assert!(descriptor_debug.contains("<redacted attachment key>"));
        assert!(!descriptor_debug.contains("AttachmentContentKey(["));
        assert!(!format!("{encrypted:?}").contains("secret bytes"));
    }

    #[test]
    fn tampering_and_descriptor_substitution_fail_closed() {
        let (descriptor, mut encrypted) = encrypt_attachment("document.bin", b"private").unwrap();
        encrypted.ciphertext[0] ^= 1;
        assert_eq!(
            decrypt_attachment(&descriptor, &encrypted).unwrap_err(),
            AttachmentError::VerificationFailed
        );

        let (mut descriptor, encrypted) = encrypt_attachment("document.bin", b"private").unwrap();
        descriptor.filename = String::from("renamed.bin");
        assert_eq!(
            decrypt_attachment(&descriptor, &encrypted).unwrap_err(),
            AttachmentError::VerificationFailed
        );
    }

    #[test]
    fn unsafe_metadata_and_hard_limits_are_rejected() {
        for filename in ["", "../secret", "folder/file", "C:secret", "nul\0file"] {
            assert_eq!(
                encrypt_attachment(filename, b"content").unwrap_err(),
                AttachmentError::InvalidAttachment
            );
        }
        assert_eq!(
            encrypt_attachment("empty.bin", &[]).unwrap_err(),
            AttachmentError::InvalidAttachment
        );
        assert_eq!(
            encrypt_thumbnail("preview.bin", &vec![0; MAX_THUMBNAIL_BYTES + 1]).unwrap_err(),
            AttachmentError::LimitExceeded
        );
    }

    #[test]
    fn thumbnail_uses_independent_key_and_kind() {
        let (file_descriptor, _) = encrypt_attachment("photo.png", PNG).unwrap();
        let (thumbnail_descriptor, thumbnail) = encrypt_thumbnail("preview.png", PNG).unwrap();
        assert_eq!(thumbnail_descriptor.kind, AttachmentKind::Thumbnail);
        assert_ne!(
            file_descriptor.content_key,
            thumbnail_descriptor.content_key
        );
        assert_eq!(
            decrypt_attachment(&thumbnail_descriptor, &thumbnail)
                .unwrap()
                .bytes
                .as_slice(),
            PNG
        );
    }

    #[test]
    fn descriptor_parser_rejects_unknown_fields_and_versions() {
        let (descriptor, _) = encrypt_attachment("file.bin", b"private").unwrap();
        let mut value = serde_json::to_value(&descriptor).unwrap();
        value["server_visible"] = serde_json::Value::Bool(true);
        assert!(serde_json::from_value::<AttachmentDescriptor>(value).is_err());

        let mut value = serde_json::to_value(&descriptor).unwrap();
        value["version"] = serde_json::Value::from(2);
        let decoded: AttachmentDescriptor = serde_json::from_value(value).unwrap();
        assert_eq!(
            decoded.validate().unwrap_err(),
            AttachmentError::InvalidAttachment
        );
    }
}
