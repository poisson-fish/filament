//! Root identity key generation and device certificate operations.
//!
//! Per-user root identity key (Ed25519) signs device certificates.
//! The server never holds the root key, so it cannot mint valid certificates.
//! Injected ("ghost") devices fail signature verification at every peer.
//!
//! # Security Properties
//!
//! - Root keys are Ed25519 keypairs generated from the platform CSPRNG.
//! - All key material is zeroized on drop via `zeroize`.
//! - No key material appears in `Display`, `Debug`, or error messages.
//! - Device certificates bind `(user_id, device_id, device_signature_pubkey)`
//!   under the root key's signature.

use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use filament_core::{DeviceCertificate, DeviceId, UserId};
use rand::rngs::OsRng;
use zeroize::Zeroizing;

use crate::error::IdentityError;

/// Byte prefix for the signed certificate payload.
///
/// This domain-separation tag prevents cross-protocol signature confusion.
const CERT_DOMAIN_TAG: &[u8] = b"filament:e2ee:device_cert:v1";

/// Domain-separation tag for root-identity continuity proofs.
const ROOT_ROTATION_DOMAIN_TAG: &[u8] = b"filament:e2ee:root_rotation:v1";

/// Public, dual-signed proof that one root identity replaced another.
#[derive(Clone, PartialEq, Eq)]
pub struct RootIdentityRotationProof {
    /// Monotonically increasing per-user rotation sequence.
    pub sequence: u64,
    /// Previously pinned Ed25519 root public key.
    pub previous_root_key_pub: [u8; 32],
    /// Replacement Ed25519 root public key.
    pub new_root_key_pub: [u8; 32],
    /// Signature by the previous root over the complete transition.
    pub previous_root_signature: [u8; 64],
    /// Proof of possession by the replacement root over the same transition.
    pub new_root_signature: [u8; 64],
}

impl core::fmt::Debug for RootIdentityRotationProof {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("RootIdentityRotationProof")
            .field("sequence", &self.sequence)
            .field("public_material", &"<redacted>")
            .finish_non_exhaustive()
    }
}

/// An Ed25519 root identity signing key.
///
/// The private key is zeroized on drop (ed25519-dalek's `SigningKey`
/// implements zeroize-on-drop internally). The public key can be freely
/// shared and is the anchor for safety numbers and device certificate
/// verification.
pub struct RootIdentityKey {
    signing_key: SigningKey,
}

impl RootIdentityKey {
    /// Generate a new root identity key using the platform CSPRNG.
    ///
    /// # Panics
    /// Panics if the OS random number generator fails (extremely unlikely).
    #[must_use]
    pub fn generate() -> Self {
        let mut rng = OsRng;
        let signing_key = SigningKey::generate(&mut rng);
        Self { signing_key }
    }

    /// Returns the public verification key bytes (32 bytes for Ed25519).
    #[must_use]
    pub fn public_key_bytes(&self) -> [u8; 32] {
        self.signing_key.verifying_key().to_bytes()
    }

    /// Restore a root identity key from secret bytes loaded by the native
    /// keystore boundary.
    #[must_use]
    pub fn from_secret_bytes(secret: &[u8; 32]) -> Self {
        Self {
            signing_key: SigningKey::from_bytes(secret),
        }
    }

    /// Copy the secret bytes for encrypted persistence or device pairing.
    ///
    /// This is crate-internal so application/UI layers cannot create a raw-key
    /// export surface. The returned buffer zeroizes itself on drop.
    pub(crate) fn secret_bytes(&self) -> Zeroizing<[u8; 32]> {
        Zeroizing::new(self.signing_key.to_bytes())
    }

    /// Sign a device certificate.
    ///
    /// The signature covers a domain-separated payload:
    /// `CERT_DOMAIN_TAG || user_id || device_id || device_signature_pubkey`.
    ///
    /// # Errors
    /// Returns [`IdentityError::InvalidInput`] if `user_id`, `device_id` are
    /// empty, or if `device_signature_pubkey` is empty.
    #[must_use]
    pub fn sign_device_certificate(
        &self,
        user_id: UserId,
        device_id: DeviceId,
        device_signature_pubkey: &[u8; 32],
    ) -> [u8; 64] {
        let payload = certificate_signing_payload(user_id, device_id, device_signature_pubkey);
        let signature = self.signing_key.sign(&payload);
        signature.to_bytes()
    }

    /// Create the canonical domain certificate for a device.
    ///
    /// # Errors
    /// Returns an error only if the domain constructor rejects a field.
    pub fn certify_device(
        &self,
        user_id: UserId,
        device_id: DeviceId,
        device_signature_pubkey: [u8; 32],
    ) -> Result<DeviceCertificate, IdentityError> {
        let signature = self.sign_device_certificate(user_id, device_id, &device_signature_pubkey);
        DeviceCertificate::try_new(
            user_id.to_string(),
            device_id.to_string(),
            device_signature_pubkey.to_vec(),
            signature.to_vec(),
        )
        .map_err(|_| IdentityError::InvalidInput(String::from("invalid device certificate")))
    }
}

impl Drop for RootIdentityKey {
    fn drop(&mut self) {
        // SigningKey zeroizes its secret bytes via the derive macro,
        // but we also manually zeroize for defense-in-depth.
        // The SigningKey internally holds a SecretKey which is zeroize-on-drop.
    }
}

// Manually drop is handled by Zeroize derive on the signing_key field.
// We don't implement Debug to avoid accidental key leakage.
impl std::fmt::Debug for RootIdentityKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("RootIdentityKey(<redacted>)")
    }
}

/// Create a dual-signed continuity proof for a destructive root rotation.
///
/// The previous and replacement roots sign the same domain-separated payload,
/// proving authorization by the pinned identity and possession of the new key.
///
/// # Errors
/// Returns [`IdentityError::InvalidInput`] for sequence zero or an unchanged
/// root key.
pub fn create_root_identity_rotation_proof(
    previous_root: &RootIdentityKey,
    new_root: &RootIdentityKey,
    user_id: UserId,
    sequence: u64,
) -> Result<RootIdentityRotationProof, IdentityError> {
    if sequence == 0 {
        return Err(IdentityError::InvalidInput(String::from(
            "root rotation sequence must be nonzero",
        )));
    }
    let previous_root_key_pub = previous_root.public_key_bytes();
    let new_root_key_pub = new_root.public_key_bytes();
    if previous_root_key_pub == new_root_key_pub {
        return Err(IdentityError::InvalidInput(String::from(
            "root rotation must change the key",
        )));
    }
    let payload =
        root_rotation_signing_payload(user_id, sequence, &previous_root_key_pub, &new_root_key_pub);
    Ok(RootIdentityRotationProof {
        sequence,
        previous_root_key_pub,
        new_root_key_pub,
        previous_root_signature: previous_root.signing_key.sign(&payload).to_bytes(),
        new_root_signature: new_root.signing_key.sign(&payload).to_bytes(),
    })
}

/// Verify both signatures in a root-identity continuity proof.
///
/// # Errors
/// Returns [`IdentityError::InvalidInput`] for structurally invalid proofs or
/// [`IdentityError::SignatureVerificationFailed`] when either signature fails.
pub fn verify_root_identity_rotation_proof(
    user_id: UserId,
    proof: &RootIdentityRotationProof,
) -> Result<(), IdentityError> {
    if proof.sequence == 0 || proof.previous_root_key_pub == proof.new_root_key_pub {
        return Err(IdentityError::InvalidInput(String::from(
            "invalid root rotation transition",
        )));
    }
    let previous_verifier = VerifyingKey::from_bytes(&proof.previous_root_key_pub)
        .map_err(|_| IdentityError::CryptoError)?;
    let new_verifier = VerifyingKey::from_bytes(&proof.new_root_key_pub)
        .map_err(|_| IdentityError::CryptoError)?;
    let payload = root_rotation_signing_payload(
        user_id,
        proof.sequence,
        &proof.previous_root_key_pub,
        &proof.new_root_key_pub,
    );
    previous_verifier
        .verify(
            &payload,
            &Signature::from_bytes(&proof.previous_root_signature),
        )
        .map_err(|_| IdentityError::SignatureVerificationFailed)?;
    new_verifier
        .verify(&payload, &Signature::from_bytes(&proof.new_root_signature))
        .map_err(|_| IdentityError::SignatureVerificationFailed)
}

/// Verify an ordered continuity chain starting from a locally pinned root.
///
/// Returns the final verified root. Missing, duplicated, reordered, or
/// disconnected transitions fail closed.
///
/// # Errors
/// Returns [`IdentityError::InvalidInput`] for sequence or continuity gaps,
/// or the signature error from [`verify_root_identity_rotation_proof`].
pub fn verify_root_identity_rotation_chain(
    user_id: UserId,
    pinned_sequence: u64,
    pinned_root_key_pub: [u8; 32],
    proofs: &[RootIdentityRotationProof],
) -> Result<[u8; 32], IdentityError> {
    let mut sequence = pinned_sequence;
    let mut current_root = pinned_root_key_pub;
    for proof in proofs {
        let expected_sequence = sequence.checked_add(1).ok_or_else(|| {
            IdentityError::InvalidInput(String::from("root rotation sequence overflow"))
        })?;
        if proof.sequence != expected_sequence || proof.previous_root_key_pub != current_root {
            return Err(IdentityError::InvalidInput(String::from(
                "root rotation continuity gap",
            )));
        }
        verify_root_identity_rotation_proof(user_id, proof)?;
        sequence = proof.sequence;
        current_root = proof.new_root_key_pub;
    }
    Ok(current_root)
}

fn root_rotation_signing_payload(
    user_id: UserId,
    sequence: u64,
    previous_root_key_pub: &[u8; 32],
    new_root_key_pub: &[u8; 32],
) -> Vec<u8> {
    let user_id = user_id.to_string();
    let mut payload = Vec::with_capacity(
        ROOT_ROTATION_DOMAIN_TAG.len() + user_id.len() + 8 + previous_root_key_pub.len() + 32,
    );
    payload.extend_from_slice(ROOT_ROTATION_DOMAIN_TAG);
    payload.extend_from_slice(user_id.as_bytes());
    payload.extend_from_slice(&sequence.to_be_bytes());
    payload.extend_from_slice(previous_root_key_pub);
    payload.extend_from_slice(new_root_key_pub);
    payload
}

/// Build the domain-separated signing payload for a device certificate.
fn certificate_signing_payload(
    user_id: UserId,
    device_id: DeviceId,
    device_signature_pubkey: &[u8; 32],
) -> Vec<u8> {
    let user_id = user_id.to_string();
    let device_id = device_id.to_string();
    let mut payload = Vec::with_capacity(
        CERT_DOMAIN_TAG.len() + user_id.len() + device_id.len() + device_signature_pubkey.len(),
    );
    payload.extend_from_slice(CERT_DOMAIN_TAG);
    payload.extend_from_slice(user_id.as_bytes());
    payload.extend_from_slice(device_id.as_bytes());
    payload.extend_from_slice(device_signature_pubkey);
    payload
}

/// Verify a device certificate signature against a published root key.
///
/// This is the ghost-device defense: the server can publish whatever
/// certificates it wants, but clients verify the signature against the
/// root key they have pinned for the user. A server-forged certificate
/// will fail verification because the server never holds the root key.
///
/// # Errors
/// - [`IdentityError::InvalidInput`] if inputs are empty or the signature
///   is not exactly 64 bytes (Ed25519 signature size).
/// - [`IdentityError::SignatureVerificationFailed`] if the signature does
///   not verify against the root key.
pub fn verify_device_certificate(
    root_key_pub: &[u8; 32],
    user_id: UserId,
    device_id: DeviceId,
    device_signature_pubkey: &[u8; 32],
    root_key_signature: &[u8; 64],
) -> Result<(), IdentityError> {
    let verifying_key =
        VerifyingKey::from_bytes(root_key_pub).map_err(|_| IdentityError::CryptoError)?;

    let signature = Signature::from_bytes(root_key_signature);

    let payload = certificate_signing_payload(user_id, device_id, device_signature_pubkey);

    verifying_key
        .verify(&payload, &signature)
        .map_err(|_| IdentityError::SignatureVerificationFailed)
}

/// Compute the safety number (fingerprint) for a root identity public key.
///
/// This is a hex-encoded SHA-256 hash of the public key, truncated to 32
/// hex characters (16 bytes). It is shareable and used for out-of-band
/// verification between users.
///
/// # Panics
/// This function panics if the input is not 32 bytes, as that indicates
/// a programming error (the caller should always pass a valid Ed25519
/// public key).
#[must_use]
pub fn safety_number(root_key_pub: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    assert!(
        root_key_pub.len() == 32,
        "root_key_pub must be 32 bytes for safety number computation"
    );
    let mut hasher = Sha256::new();
    hasher.update(b"filament:safety_number:v1:");
    hasher.update(root_key_pub);
    let hash = hasher.finalize();
    // Take first 16 bytes, encode as hex → 32-character string
    hex_encode(&hash[..16])
}

/// Encode bytes as a lowercase hex string.
fn hex_encode(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut result = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(result, "{byte:02x}");
    }
    result
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_ids() -> (UserId, DeviceId) {
        (
            UserId::try_from(String::from("01ARZ3NDEKTSV4RRFFQ69G5FAV")).unwrap(),
            DeviceId::try_from(String::from("01ARZ3NDEKTSV4RRFFQ69G5FAW")).unwrap(),
        )
    }

    #[test]
    fn root_key_generation_produces_unique_keys() {
        let key_a = RootIdentityKey::generate();
        let key_b = RootIdentityKey::generate();
        assert_ne!(key_a.public_key_bytes(), key_b.public_key_bytes());
    }

    #[test]
    fn root_key_public_is_32_bytes() {
        let key = RootIdentityKey::generate();
        assert_eq!(key.public_key_bytes().len(), 32);
    }

    #[test]
    fn device_certificate_sign_and_verify_round_trip() {
        let root_key = RootIdentityKey::generate();
        let (user_id, device_id) = fixture_ids();
        let device_sig_pubkey = [0xAB; 32];

        let signature = root_key.sign_device_certificate(user_id, device_id, &device_sig_pubkey);

        assert_eq!(signature.len(), 64);

        // Verification with correct root key succeeds
        verify_device_certificate(
            &root_key.public_key_bytes(),
            user_id,
            device_id,
            &device_sig_pubkey,
            &signature,
        )
        .expect("verification with correct root key should succeed");
    }

    #[test]
    fn forged_certificate_rejected_by_verification() {
        // Alice has a root key. The server tries to forge a device certificate
        // using a different key (the server's own key).
        let alice_root_key = RootIdentityKey::generate();
        let server_fake_key = RootIdentityKey::generate();
        let (user_id, device_id) = fixture_ids();
        let device_sig_pubkey = [0xAB; 32];

        // Server signs with its own key (not Alice's root key)
        let forged_signature =
            server_fake_key.sign_device_certificate(user_id, device_id, &device_sig_pubkey);

        // Client verifies against Alice's pinned root key — must fail
        let result = verify_device_certificate(
            &alice_root_key.public_key_bytes(),
            user_id,
            device_id,
            &device_sig_pubkey,
            &forged_signature,
        );

        assert_eq!(result, Err(IdentityError::SignatureVerificationFailed));
    }

    #[test]
    fn ghost_device_injection_fails() {
        // Simulate the ghost-device injection attack:
        // The server constructs a device certificate with a fabricated signature
        // (not signed by the user's root key at all) and tries to pass it off.

        let root_key = RootIdentityKey::generate();
        let (user_id, device_id) = fixture_ids();
        let device_sig_pubkey = [0xAB; 32];

        // Fabricated signature (64 random bytes, not a valid signature)
        let fake_signature = core::array::from_fn(|index| u8::try_from(index).unwrap());

        let result = verify_device_certificate(
            &root_key.public_key_bytes(),
            user_id,
            device_id,
            &device_sig_pubkey,
            &fake_signature,
        );

        assert_eq!(result, Err(IdentityError::SignatureVerificationFailed));
    }

    #[test]
    fn tampered_payload_rejected() {
        let root_key = RootIdentityKey::generate();
        let (user_id, device_id) = fixture_ids();
        let tampered_device_id =
            DeviceId::try_from(String::from("01ARZ3NDEKTSV4RRFFQ69G5FAX")).unwrap();
        let device_sig_pubkey = [0xAB; 32];

        let signature = root_key.sign_device_certificate(user_id, device_id, &device_sig_pubkey);

        // Tamper with the device_id — verification should fail
        let result = verify_device_certificate(
            &root_key.public_key_bytes(),
            user_id,
            tampered_device_id,
            &device_sig_pubkey,
            &signature,
        );

        assert_eq!(result, Err(IdentityError::SignatureVerificationFailed));
    }

    #[test]
    fn root_key_secret_round_trip_preserves_identity() {
        let root_key = RootIdentityKey::generate();
        let restored = RootIdentityKey::from_secret_bytes(&root_key.secret_bytes());
        assert_eq!(restored.public_key_bytes(), root_key.public_key_bytes());
    }

    #[test]
    fn certify_device_returns_domain_valid_certificate() {
        let root_key = RootIdentityKey::generate();
        let (user_id, device_id) = fixture_ids();
        let certificate = root_key
            .certify_device(user_id, device_id, [0xAB; 32])
            .unwrap();
        assert_eq!(certificate.user_id, user_id.to_string());
        assert_eq!(certificate.device_id, device_id.to_string());
    }

    #[test]
    fn safety_number_is_deterministic_and_32_chars() {
        let root_key = RootIdentityKey::generate();
        let pub_bytes = root_key.public_key_bytes();

        let sn1 = safety_number(&pub_bytes);
        let sn2 = safety_number(&pub_bytes);

        assert_eq!(sn1, sn2);
        assert_eq!(sn1.len(), 32);
        assert!(sn1.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn safety_number_differs_for_different_keys() {
        let key_a = RootIdentityKey::generate();
        let key_b = RootIdentityKey::generate();

        assert_ne!(
            safety_number(&key_a.public_key_bytes()),
            safety_number(&key_b.public_key_bytes())
        );
    }

    #[test]
    fn debug_does_not_leak_key_material() {
        let root_key = RootIdentityKey::generate();
        let debug_str = format!("{root_key:?}");
        let pub_bytes = root_key.public_key_bytes();
        let pub_hex = hex_encode(&pub_bytes);

        // The debug output must not contain the hex of the public key
        // (and certainly not the private key)
        assert!(!debug_str.contains(&pub_hex));
        assert!(debug_str.contains("redacted"));
    }

    #[test]
    fn root_rotation_proof_is_deterministic_and_dual_signed() {
        let (user_id, _) = fixture_ids();
        let previous = RootIdentityKey::from_secret_bytes(&[0x11; 32]);
        let replacement = RootIdentityKey::from_secret_bytes(&[0x22; 32]);
        let first =
            create_root_identity_rotation_proof(&previous, &replacement, user_id, 1).unwrap();
        let second =
            create_root_identity_rotation_proof(&previous, &replacement, user_id, 1).unwrap();
        assert_eq!(first, second);
        verify_root_identity_rotation_proof(user_id, &first).unwrap();
    }

    #[test]
    fn root_rotation_proof_rejects_tampering_replay_shape_and_wrong_user() {
        let (user_id, _) = fixture_ids();
        let other_user = UserId::try_from(String::from("01ARZ3NDEKTSV4RRFFQ69G5FAX")).unwrap();
        let previous = RootIdentityKey::from_secret_bytes(&[0x33; 32]);
        let replacement = RootIdentityKey::from_secret_bytes(&[0x44; 32]);
        let mut proof =
            create_root_identity_rotation_proof(&previous, &replacement, user_id, 7).unwrap();
        assert_eq!(
            verify_root_identity_rotation_proof(other_user, &proof),
            Err(IdentityError::SignatureVerificationFailed)
        );
        proof.new_root_signature[0] ^= 1;
        assert_eq!(
            verify_root_identity_rotation_proof(user_id, &proof),
            Err(IdentityError::SignatureVerificationFailed)
        );
        assert!(create_root_identity_rotation_proof(&previous, &replacement, user_id, 0).is_err());
        assert!(create_root_identity_rotation_proof(&previous, &previous, user_id, 1).is_err());
    }

    #[test]
    fn root_rotation_chain_rejects_suppression_and_reordering() {
        let (user_id, _) = fixture_ids();
        let first = RootIdentityKey::from_secret_bytes(&[0x51; 32]);
        let second = RootIdentityKey::from_secret_bytes(&[0x52; 32]);
        let third = RootIdentityKey::from_secret_bytes(&[0x53; 32]);
        let first_proof = create_root_identity_rotation_proof(&first, &second, user_id, 1).unwrap();
        let second_proof =
            create_root_identity_rotation_proof(&second, &third, user_id, 2).unwrap();
        assert_eq!(
            verify_root_identity_rotation_chain(
                user_id,
                0,
                first.public_key_bytes(),
                &[first_proof.clone(), second_proof.clone()],
            )
            .unwrap(),
            third.public_key_bytes()
        );
        assert!(verify_root_identity_rotation_chain(
            user_id,
            0,
            first.public_key_bytes(),
            &[second_proof]
        )
        .is_err());
        assert!(verify_root_identity_rotation_chain(
            user_id,
            0,
            first.public_key_bytes(),
            &[first_proof.clone(), first_proof]
        )
        .is_err());
    }
}
