//! Short-lived QR device pairing with authenticated root-key transfer.
//!
//! The new device generates an ephemeral X25519 HPKE receiver key and a
//! high-entropy pairing secret. Both are encoded in a short-lived QR payload
//! that must be scanned locally by an already-certified device. The existing
//! device then:
//!
//! 1. authenticates the returning transfer with ChaCha20-Poly1305 under the QR
//!    secret,
//! 2. signs the pairing context with its MLS Ed25519 device key, and
//! 3. encrypts the root identity secret to the new device with X25519 HPKE.
//!
//! A network attacker who did not observe the QR payload cannot substitute a
//! different root identity, even though HPKE base mode alone does not
//! authenticate the sender. Pairing state is consumed by ownership when the
//! transfer is completed, making the receiver API single-use.

use filament_core::{DeviceCertificate, DeviceId, UserId};
use openmls::prelude::{
    AeadType, HpkeAeadType, HpkeCiphertext, HpkeConfig, HpkeKdfType, HpkeKemType,
    OpenMlsCrypto as _, OpenMlsProvider as _, OpenMlsRand as _, SignatureScheme,
};
use openmls_rust_crypto::OpenMlsRustCrypto;
use serde::{de, Deserialize, Deserializer, Serialize};
use zeroize::{Zeroize, Zeroizing};

use crate::{
    error::PairingError, identity::verify_device_certificate, keypackage::MlsDevice,
    RootIdentityKey,
};

/// Default lifetime of a QR pairing offer.
pub const DEFAULT_PAIRING_TTL_SECS: i64 = 5 * 60;
/// Hard maximum lifetime of a QR pairing offer.
pub const MAX_PAIRING_TTL_SECS: i64 = 5 * 60;
/// Maximum encoded QR offer size.
pub const MAX_PAIRING_OFFER_BYTES: usize = 2_048;
/// Maximum encoded encrypted-transfer size.
pub const MAX_PAIRING_TRANSFER_BYTES: usize = 4_096;

const PAIRING_PROTOCOL_VERSION: u16 = 1;
const PAIRING_ID_BYTES: usize = 32;
const PAIRING_SECRET_BYTES: usize = 32;
const HPKE_PUBLIC_KEY_BYTES: usize = 32;
const HPKE_PRIVATE_KEY_BYTES: usize = 32;
const HPKE_KEM_OUTPUT_BYTES: usize = 32;
const ROOT_SECRET_BYTES: usize = 32;
const ROOT_PUBLIC_KEY_BYTES: usize = 32;
const ED25519_PUBLIC_KEY_BYTES: usize = 32;
const ED25519_SIGNATURE_BYTES: usize = 64;
const AUTH_NONCE_BYTES: usize = 12;
const AUTH_TAG_BYTES: usize = 16;
const HPKE_ROOT_CIPHERTEXT_BYTES: usize = ROOT_SECRET_BYTES + 16;
const MAX_CLOCK_SKEW_SECS: i64 = 30;

const AUTHORIZATION_DOMAIN: &[u8] = b"filament:e2ee:device_pairing_authorization:v1";
const HPKE_INFO: &[u8] = b"filament:e2ee:device_pairing_root_transfer:v1";

fn hpke_config() -> HpkeConfig {
    HpkeConfig(
        HpkeKemType::DhKem25519,
        HpkeKdfType::HkdfSha256,
        HpkeAeadType::ChaCha20Poly1305,
    )
}

#[derive(Clone)]
struct PairingContext {
    user_id: UserId,
    new_device_id: DeviceId,
    pairing_id: [u8; PAIRING_ID_BYTES],
    receiver_public_key: [u8; HPKE_PUBLIC_KEY_BYTES],
    created_at_unix: i64,
    expires_at_unix: i64,
}

impl core::fmt::Debug for PairingContext {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("PairingContext")
            .field("user_id", &self.user_id)
            .field("new_device_id", &self.new_device_id)
            .field("pairing_id", &"<redacted>")
            .field("receiver_public_key", &"<redacted>")
            .field("created_at_unix", &self.created_at_unix)
            .field("expires_at_unix", &self.expires_at_unix)
            .finish()
    }
}

/// Single-use receiver state held by the new device while displaying its QR.
pub struct PairingReceiver {
    context: PairingContext,
    pairing_secret: Zeroizing<[u8; PAIRING_SECRET_BYTES]>,
    receiver_private_key: Zeroizing<Vec<u8>>,
}

impl core::fmt::Debug for PairingReceiver {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("PairingReceiver(<key material redacted>)")
    }
}

impl PairingReceiver {
    /// Begin pairing for a fresh device ID.
    ///
    /// # Errors
    /// Returns [`PairingError::InvalidPayload`] for an invalid lifetime or
    /// timestamp, and [`PairingError::CryptoError`] if the approved OpenMLS
    /// provider cannot generate receiver material.
    pub fn begin(
        user_id: UserId,
        new_device_id: DeviceId,
        now_unix: i64,
        ttl_secs: i64,
    ) -> Result<Self, PairingError> {
        if now_unix < 1 || !(1..=MAX_PAIRING_TTL_SECS).contains(&ttl_secs) {
            return Err(PairingError::InvalidPayload);
        }
        let expires_at_unix = now_unix
            .checked_add(ttl_secs)
            .ok_or(PairingError::InvalidPayload)?;
        let provider = OpenMlsRustCrypto::default();
        let pairing_id = provider
            .rand()
            .random_array::<PAIRING_ID_BYTES>()
            .map_err(|_| PairingError::CryptoError)?;
        let pairing_secret = provider
            .rand()
            .random_array::<PAIRING_SECRET_BYTES>()
            .map_err(|_| PairingError::CryptoError)?;
        let key_ikm = Zeroizing::new(
            provider
                .rand()
                .random_vec(HPKE_PRIVATE_KEY_BYTES)
                .map_err(|_| PairingError::CryptoError)?,
        );
        let key_pair = provider
            .crypto()
            .derive_hpke_keypair(hpke_config(), &key_ikm)
            .map_err(|_| PairingError::CryptoError)?;
        let receiver_public_key = key_pair
            .public
            .as_slice()
            .try_into()
            .map_err(|_| PairingError::CryptoError)?;
        let receiver_private_key = Zeroizing::new(key_pair.private.to_vec());

        Ok(Self {
            context: PairingContext {
                user_id,
                new_device_id,
                pairing_id,
                receiver_public_key,
                created_at_unix: now_unix,
                expires_at_unix,
            },
            pairing_secret: Zeroizing::new(pairing_secret),
            receiver_private_key,
        })
    }

    /// The new device ID bound into this pairing session.
    #[must_use]
    pub const fn new_device_id(&self) -> DeviceId {
        self.context.new_device_id
    }

    /// Encode the sensitive, short-lived QR payload.
    ///
    /// The returned buffer zeroizes on drop. It must never be logged or sent
    /// through the Filament server.
    ///
    /// # Errors
    /// Returns [`PairingError::SerializationFailed`] if strict JSON encoding
    /// fails or unexpectedly exceeds the hard QR payload cap.
    pub fn qr_payload(&self) -> Result<Zeroizing<Vec<u8>>, PairingError> {
        let user_id = self.context.user_id.to_string();
        let new_device_id = self.context.new_device_id.to_string();
        let wire = PairingOfferWireRef {
            v: PAIRING_PROTOCOL_VERSION,
            user_id: &user_id,
            new_device_id: &new_device_id,
            pairing_id: &self.context.pairing_id,
            receiver_public_key: &self.context.receiver_public_key,
            pairing_secret: self.pairing_secret.as_slice(),
            created_at_unix: self.context.created_at_unix,
            expires_at_unix: self.context.expires_at_unix,
        };
        let encoded = serde_json::to_vec(&wire).map_err(|_| PairingError::SerializationFailed)?;
        if encoded.len() > MAX_PAIRING_OFFER_BYTES {
            return Err(PairingError::SerializationFailed);
        }
        Ok(Zeroizing::new(encoded))
    }

    /// Consume this receiver and authenticate/decrypt the returning transfer.
    ///
    /// # Errors
    /// Fails closed on expiry, QR-secret authentication failure, invalid
    /// existing-device signatures/certificates, or HPKE decryption failure.
    pub fn complete(
        self,
        transfer: &PairingTransfer,
        now_unix: i64,
    ) -> Result<PairedRootIdentity, PairingError> {
        validate_pairing_window(&self.context, now_unix)?;
        if transfer.pairing_id != self.context.pairing_id {
            return Err(PairingError::AuthenticationFailed);
        }

        let (sender_user_id, sender_device_id, sender_signature_key, sender_root_signature) =
            certificate_fields(&transfer.sender_certificate)?;
        if sender_user_id != self.context.user_id {
            return Err(PairingError::UserMismatch);
        }
        if sender_device_id == self.context.new_device_id {
            return Err(PairingError::DeviceMismatch);
        }

        let authorization_payload = authorization_payload(
            &self.context,
            &transfer.sender_certificate,
            &transfer.root_key_pub,
        );
        let provider = OpenMlsRustCrypto::default();
        let authenticated = provider.crypto().aead_decrypt(
            AeadType::ChaCha20Poly1305,
            self.pairing_secret.as_slice(),
            &transfer.authorization_tag,
            &transfer.authorization_nonce,
            &authorization_payload,
        );
        if authenticated.as_deref() != Ok(&[]) {
            return Err(PairingError::AuthenticationFailed);
        }
        provider
            .crypto()
            .verify_signature(
                SignatureScheme::ED25519,
                &authorization_payload,
                &sender_signature_key,
                &transfer.authorization_signature,
            )
            .map_err(|_| PairingError::AuthenticationFailed)?;

        let hpke_ciphertext = HpkeCiphertext {
            kem_output: transfer.kem_output.to_vec().into(),
            ciphertext: transfer.ciphertext.to_vec().into(),
        };
        let mut root_secret = Zeroizing::new(
            provider
                .crypto()
                .hpke_open(
                    hpke_config(),
                    &hpke_ciphertext,
                    self.receiver_private_key.as_slice(),
                    HPKE_INFO,
                    &authorization_payload,
                )
                .map_err(|_| PairingError::AuthenticationFailed)?,
        );
        let root_secret_array = Zeroizing::new(
            root_secret
                .as_slice()
                .try_into()
                .map_err(|_| PairingError::AuthenticationFailed)?,
        );
        root_secret.zeroize();
        let root_identity = RootIdentityKey::from_secret_bytes(&root_secret_array);
        let derived_root_public = root_identity.public_key_bytes();
        if derived_root_public != transfer.root_key_pub {
            return Err(PairingError::AuthenticationFailed);
        }
        verify_device_certificate(
            &derived_root_public,
            sender_user_id,
            sender_device_id,
            &sender_signature_key,
            &sender_root_signature,
        )
        .map_err(|_| PairingError::AuthenticationFailed)?;

        Ok(PairedRootIdentity {
            root_identity,
            user_id: self.context.user_id,
            new_device_id: self.context.new_device_id,
            existing_device_id: sender_device_id,
        })
    }
}

/// A QR offer parsed by the already-certified device.
pub struct ScannedPairingOffer {
    context: PairingContext,
    pairing_secret: Zeroizing<[u8; PAIRING_SECRET_BYTES]>,
}

impl core::fmt::Debug for ScannedPairingOffer {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("ScannedPairingOffer(<QR secret redacted>)")
    }
}

impl ScannedPairingOffer {
    /// Strictly parse and validate a scanned QR payload.
    ///
    /// # Errors
    /// Rejects oversized, malformed, unknown-field, expired, or overlong offers.
    pub fn from_qr_payload(payload: &[u8], now_unix: i64) -> Result<Self, PairingError> {
        if payload.is_empty() || payload.len() > MAX_PAIRING_OFFER_BYTES {
            return Err(PairingError::InvalidPayload);
        }
        let wire: PairingOfferWire =
            serde_json::from_slice(payload).map_err(|_| PairingError::SerializationFailed)?;
        if wire.v != PAIRING_PROTOCOL_VERSION {
            return Err(PairingError::InvalidPayload);
        }
        let context = PairingContext {
            user_id: UserId::try_from(wire.user_id).map_err(|_| PairingError::InvalidPayload)?,
            new_device_id: DeviceId::try_from(wire.new_device_id)
                .map_err(|_| PairingError::InvalidPayload)?,
            pairing_id: wire
                .pairing_id
                .try_into()
                .map_err(|_| PairingError::InvalidPayload)?,
            receiver_public_key: wire
                .receiver_public_key
                .try_into()
                .map_err(|_| PairingError::InvalidPayload)?,
            created_at_unix: wire.created_at_unix,
            expires_at_unix: wire.expires_at_unix,
        };
        validate_pairing_window(&context, now_unix)?;
        let pairing_secret = wire
            .pairing_secret
            .try_into()
            .map_err(|_| PairingError::InvalidPayload)?;
        Ok(Self {
            context,
            pairing_secret: Zeroizing::new(pairing_secret),
        })
    }

    /// User ID claimed by the new device's authenticated account session.
    #[must_use]
    pub const fn user_id(&self) -> UserId {
        self.context.user_id
    }

    /// Fresh device ID that will receive the root identity.
    #[must_use]
    pub const fn new_device_id(&self) -> DeviceId {
        self.context.new_device_id
    }
}

/// Encrypted and authenticated response returned to the new device.
pub struct PairingTransfer {
    pairing_id: [u8; PAIRING_ID_BYTES],
    sender_certificate: DeviceCertificate,
    root_key_pub: [u8; ROOT_PUBLIC_KEY_BYTES],
    kem_output: [u8; HPKE_KEM_OUTPUT_BYTES],
    ciphertext: [u8; HPKE_ROOT_CIPHERTEXT_BYTES],
    authorization_nonce: [u8; AUTH_NONCE_BYTES],
    authorization_tag: [u8; AUTH_TAG_BYTES],
    authorization_signature: [u8; ED25519_SIGNATURE_BYTES],
}

impl core::fmt::Debug for PairingTransfer {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("PairingTransfer(<encrypted root identity>)")
    }
}

impl PairingTransfer {
    /// Encode the transfer for a bounded device-to-device transport.
    ///
    /// # Errors
    /// Returns [`PairingError::SerializationFailed`] on encoding or size-cap failure.
    pub fn to_payload(&self) -> Result<Vec<u8>, PairingError> {
        let wire = PairingTransferWireRef {
            v: PAIRING_PROTOCOL_VERSION,
            pairing_id: &self.pairing_id,
            sender_certificate: &self.sender_certificate,
            root_key_pub: &self.root_key_pub,
            kem_output: &self.kem_output,
            ciphertext: &self.ciphertext,
            authorization_nonce: &self.authorization_nonce,
            authorization_tag: &self.authorization_tag,
            authorization_signature: &self.authorization_signature,
        };
        let encoded = serde_json::to_vec(&wire).map_err(|_| PairingError::SerializationFailed)?;
        if encoded.len() > MAX_PAIRING_TRANSFER_BYTES {
            return Err(PairingError::SerializationFailed);
        }
        Ok(encoded)
    }

    /// Strictly decode an encrypted transfer.
    ///
    /// # Errors
    /// Rejects empty, oversized, malformed, unknown-field, wrong-version, or
    /// wrong-length payloads before any cryptographic processing.
    pub fn from_payload(payload: &[u8]) -> Result<Self, PairingError> {
        if payload.is_empty() || payload.len() > MAX_PAIRING_TRANSFER_BYTES {
            return Err(PairingError::InvalidPayload);
        }
        let wire: PairingTransferWire =
            serde_json::from_slice(payload).map_err(|_| PairingError::SerializationFailed)?;
        if wire.v != PAIRING_PROTOCOL_VERSION {
            return Err(PairingError::InvalidPayload);
        }
        certificate_fields(&wire.sender_certificate)?;
        Ok(Self {
            pairing_id: wire
                .pairing_id
                .try_into()
                .map_err(|_| PairingError::InvalidPayload)?,
            sender_certificate: wire.sender_certificate,
            root_key_pub: wire
                .root_key_pub
                .try_into()
                .map_err(|_| PairingError::InvalidPayload)?,
            kem_output: wire
                .kem_output
                .try_into()
                .map_err(|_| PairingError::InvalidPayload)?,
            ciphertext: wire
                .ciphertext
                .try_into()
                .map_err(|_| PairingError::InvalidPayload)?,
            authorization_nonce: wire
                .authorization_nonce
                .try_into()
                .map_err(|_| PairingError::InvalidPayload)?,
            authorization_tag: wire
                .authorization_tag
                .try_into()
                .map_err(|_| PairingError::InvalidPayload)?,
            authorization_signature: wire
                .authorization_signature
                .try_into()
                .map_err(|_| PairingError::InvalidPayload)?,
        })
    }
}

/// Result of a verified transfer on the newly paired device.
pub struct PairedRootIdentity {
    root_identity: RootIdentityKey,
    user_id: UserId,
    new_device_id: DeviceId,
    existing_device_id: DeviceId,
}

impl core::fmt::Debug for PairedRootIdentity {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("PairedRootIdentity")
            .field("root_identity", &"<redacted>")
            .field("user_id", &self.user_id)
            .field("new_device_id", &self.new_device_id)
            .field("existing_device_id", &self.existing_device_id)
            .finish()
    }
}

impl PairedRootIdentity {
    /// Restored root identity; never exposed as raw secret bytes.
    #[must_use]
    pub const fn root_identity(&self) -> &RootIdentityKey {
        &self.root_identity
    }

    /// User whose identity was restored.
    #[must_use]
    pub const fn user_id(&self) -> UserId {
        self.user_id
    }

    /// Fresh device ID authorized by the QR session.
    #[must_use]
    pub const fn new_device_id(&self) -> DeviceId {
        self.new_device_id
    }

    /// Existing certified device that authorized the transfer.
    #[must_use]
    pub const fn existing_device_id(&self) -> DeviceId {
        self.existing_device_id
    }

    /// Move the restored root identity into native keystore/device setup code.
    #[must_use]
    pub fn into_root_identity(self) -> RootIdentityKey {
        self.root_identity
    }
}

/// Encrypt the user's root identity from an existing certified device to a
/// scanned, short-lived pairing receiver.
///
/// # Errors
/// Rejects cross-user/self-pairing, expired offers, inconsistent root/device
/// state, and any provider failure without returning partial key material.
pub fn create_pairing_transfer(
    existing_device: &MlsDevice,
    root_identity: &RootIdentityKey,
    offer: &ScannedPairingOffer,
    now_unix: i64,
) -> Result<PairingTransfer, PairingError> {
    validate_pairing_window(&offer.context, now_unix)?;
    let (sender_user_id, sender_device_id, sender_signature_key, sender_root_signature) =
        certificate_fields(existing_device.certificate())?;
    if sender_user_id != offer.context.user_id {
        return Err(PairingError::UserMismatch);
    }
    if sender_device_id == offer.context.new_device_id {
        return Err(PairingError::DeviceMismatch);
    }
    let root_key_pub = root_identity.public_key_bytes();
    if existing_device.root_key_public() != &root_key_pub {
        return Err(PairingError::AuthenticationFailed);
    }
    verify_device_certificate(
        &root_key_pub,
        sender_user_id,
        sender_device_id,
        &sender_signature_key,
        &sender_root_signature,
    )
    .map_err(|_| PairingError::AuthenticationFailed)?;

    let authorization_payload =
        authorization_payload(&offer.context, existing_device.certificate(), &root_key_pub);
    let provider = existing_device.provider();
    let authorization_nonce = provider
        .rand()
        .random_array::<AUTH_NONCE_BYTES>()
        .map_err(|_| PairingError::CryptoError)?;
    let authorization_tag = provider
        .crypto()
        .aead_encrypt(
            AeadType::ChaCha20Poly1305,
            offer.pairing_secret.as_slice(),
            &[],
            &authorization_nonce,
            &authorization_payload,
        )
        .map_err(|_| PairingError::CryptoError)?
        .try_into()
        .map_err(|_| PairingError::CryptoError)?;
    let authorization_signature = existing_device
        .sign_pairing_authorization(&authorization_payload)
        .map_err(|_| PairingError::CryptoError)?
        .try_into()
        .map_err(|_| PairingError::CryptoError)?;
    let root_secret = root_identity.secret_bytes();
    let encrypted = provider
        .crypto()
        .hpke_seal(
            hpke_config(),
            &offer.context.receiver_public_key,
            HPKE_INFO,
            &authorization_payload,
            root_secret.as_slice(),
        )
        .map_err(|_| PairingError::CryptoError)?;

    Ok(PairingTransfer {
        pairing_id: offer.context.pairing_id,
        sender_certificate: existing_device.certificate().clone(),
        root_key_pub,
        kem_output: encrypted
            .kem_output
            .as_slice()
            .try_into()
            .map_err(|_| PairingError::CryptoError)?,
        ciphertext: encrypted
            .ciphertext
            .as_slice()
            .try_into()
            .map_err(|_| PairingError::CryptoError)?,
        authorization_nonce,
        authorization_tag,
        authorization_signature,
    })
}

fn validate_pairing_window(context: &PairingContext, now_unix: i64) -> Result<(), PairingError> {
    let lifetime = context
        .expires_at_unix
        .checked_sub(context.created_at_unix)
        .ok_or(PairingError::InvalidPayload)?;
    if context.created_at_unix < 1
        || !(1..=MAX_PAIRING_TTL_SECS).contains(&lifetime)
        || context.created_at_unix > now_unix.saturating_add(MAX_CLOCK_SKEW_SECS)
        || context.expires_at_unix <= now_unix
    {
        return Err(PairingError::Expired);
    }
    Ok(())
}

fn certificate_fields(
    certificate: &DeviceCertificate,
) -> Result<
    (
        UserId,
        DeviceId,
        [u8; ED25519_PUBLIC_KEY_BYTES],
        [u8; ED25519_SIGNATURE_BYTES],
    ),
    PairingError,
> {
    Ok((
        UserId::try_from(certificate.user_id.clone()).map_err(|_| PairingError::InvalidPayload)?,
        DeviceId::try_from(certificate.device_id.clone())
            .map_err(|_| PairingError::InvalidPayload)?,
        certificate
            .device_signature_pubkey
            .as_slice()
            .try_into()
            .map_err(|_| PairingError::InvalidPayload)?,
        certificate
            .root_key_signature
            .as_slice()
            .try_into()
            .map_err(|_| PairingError::InvalidPayload)?,
    ))
}

fn authorization_payload(
    context: &PairingContext,
    sender_certificate: &DeviceCertificate,
    root_key_pub: &[u8; ROOT_PUBLIC_KEY_BYTES],
) -> Vec<u8> {
    let mut payload = Vec::with_capacity(320);
    payload.extend_from_slice(AUTHORIZATION_DOMAIN);
    payload.extend_from_slice(&PAIRING_PROTOCOL_VERSION.to_be_bytes());
    payload.extend_from_slice(&context.pairing_id);
    payload.extend_from_slice(context.user_id.to_string().as_bytes());
    payload.extend_from_slice(context.new_device_id.to_string().as_bytes());
    payload.extend_from_slice(&context.receiver_public_key);
    payload.extend_from_slice(&context.created_at_unix.to_be_bytes());
    payload.extend_from_slice(&context.expires_at_unix.to_be_bytes());
    payload.extend_from_slice(sender_certificate.user_id.as_bytes());
    payload.extend_from_slice(sender_certificate.device_id.as_bytes());
    payload.extend_from_slice(&sender_certificate.device_signature_pubkey);
    payload.extend_from_slice(&sender_certificate.root_key_signature);
    payload.extend_from_slice(root_key_pub);
    payload
}

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

macro_rules! exact_deserializer {
    ($name:ident, $size:expr) => {
        fn $name<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
        where
            D: Deserializer<'de>,
        {
            deserialize_exact_bytes::<D, $size>(deserializer)
        }
    };
}

exact_deserializer!(deserialize_pairing_id, PAIRING_ID_BYTES);
exact_deserializer!(deserialize_pairing_secret, PAIRING_SECRET_BYTES);
exact_deserializer!(deserialize_hpke_public_key, HPKE_PUBLIC_KEY_BYTES);
exact_deserializer!(deserialize_root_public_key, ROOT_PUBLIC_KEY_BYTES);
exact_deserializer!(deserialize_kem_output, HPKE_KEM_OUTPUT_BYTES);
exact_deserializer!(deserialize_root_ciphertext, HPKE_ROOT_CIPHERTEXT_BYTES);
exact_deserializer!(deserialize_auth_nonce, AUTH_NONCE_BYTES);
exact_deserializer!(deserialize_auth_tag, AUTH_TAG_BYTES);
exact_deserializer!(deserialize_ed25519_signature, ED25519_SIGNATURE_BYTES);

#[derive(Serialize)]
struct PairingOfferWireRef<'a> {
    v: u16,
    user_id: &'a str,
    new_device_id: &'a str,
    pairing_id: &'a [u8],
    receiver_public_key: &'a [u8],
    pairing_secret: &'a [u8],
    created_at_unix: i64,
    expires_at_unix: i64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PairingOfferWire {
    v: u16,
    user_id: String,
    new_device_id: String,
    #[serde(deserialize_with = "deserialize_pairing_id")]
    pairing_id: Vec<u8>,
    #[serde(deserialize_with = "deserialize_hpke_public_key")]
    receiver_public_key: Vec<u8>,
    #[serde(deserialize_with = "deserialize_pairing_secret")]
    pairing_secret: Vec<u8>,
    created_at_unix: i64,
    expires_at_unix: i64,
}

#[derive(Serialize)]
struct PairingTransferWireRef<'a> {
    v: u16,
    pairing_id: &'a [u8],
    sender_certificate: &'a DeviceCertificate,
    root_key_pub: &'a [u8],
    kem_output: &'a [u8],
    ciphertext: &'a [u8],
    authorization_nonce: &'a [u8],
    authorization_tag: &'a [u8],
    authorization_signature: &'a [u8],
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PairingTransferWire {
    v: u16,
    #[serde(deserialize_with = "deserialize_pairing_id")]
    pairing_id: Vec<u8>,
    sender_certificate: DeviceCertificate,
    #[serde(deserialize_with = "deserialize_root_public_key")]
    root_key_pub: Vec<u8>,
    #[serde(deserialize_with = "deserialize_kem_output")]
    kem_output: Vec<u8>,
    #[serde(deserialize_with = "deserialize_root_ciphertext")]
    ciphertext: Vec<u8>,
    #[serde(deserialize_with = "deserialize_auth_nonce")]
    authorization_nonce: Vec<u8>,
    #[serde(deserialize_with = "deserialize_auth_tag")]
    authorization_tag: Vec<u8>,
    #[serde(deserialize_with = "deserialize_ed25519_signature")]
    authorization_signature: Vec<u8>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::verify_device_certificate;

    const NOW: i64 = 1_750_000_000;

    fn pairing_fixture() -> (
        UserId,
        RootIdentityKey,
        MlsDevice,
        PairingReceiver,
        ScannedPairingOffer,
    ) {
        let user_id = UserId::new();
        let root = RootIdentityKey::generate();
        let existing = MlsDevice::generate(user_id, DeviceId::new(), &root).unwrap();
        let receiver =
            PairingReceiver::begin(user_id, DeviceId::new(), NOW, DEFAULT_PAIRING_TTL_SECS)
                .unwrap();
        let qr_payload = receiver.qr_payload().unwrap();
        let scanned = ScannedPairingOffer::from_qr_payload(&qr_payload, NOW).unwrap();
        (user_id, root, existing, receiver, scanned)
    }

    #[test]
    fn encrypted_pairing_round_trip_restores_identity_and_certifies_new_device() {
        let (user_id, root, existing, receiver, scanned) = pairing_fixture();
        let expected_root_public = root.public_key_bytes();
        let transfer = create_pairing_transfer(&existing, &root, &scanned, NOW).unwrap();
        let encoded = transfer.to_payload().unwrap();
        let decoded = PairingTransfer::from_payload(&encoded).unwrap();
        let paired = receiver.complete(&decoded, NOW).unwrap();

        assert_eq!(
            paired.root_identity().public_key_bytes(),
            expected_root_public
        );
        assert_eq!(paired.user_id(), user_id);
        assert_eq!(
            paired.existing_device_id().to_string(),
            existing.certificate().device_id
        );

        let new_device =
            MlsDevice::generate(user_id, paired.new_device_id(), paired.root_identity()).unwrap();
        let signature_key: [u8; 32] = new_device
            .certificate()
            .device_signature_pubkey
            .as_slice()
            .try_into()
            .unwrap();
        let root_signature: [u8; 64] = new_device
            .certificate()
            .root_key_signature
            .as_slice()
            .try_into()
            .unwrap();
        verify_device_certificate(
            &expected_root_public,
            user_id,
            paired.new_device_id(),
            &signature_key,
            &root_signature,
        )
        .unwrap();
    }

    #[test]
    fn transfer_payload_never_contains_plaintext_root_secret() {
        let (_, root, existing, _, scanned) = pairing_fixture();
        let secret = root.secret_bytes();
        let transfer = create_pairing_transfer(&existing, &root, &scanned, NOW).unwrap();
        let encoded = transfer.to_payload().unwrap();
        let json_encoded_secret = serde_json::to_vec(secret.as_slice()).unwrap();
        assert!(!encoded
            .windows(secret.len())
            .any(|window| window == secret.as_slice()));
        assert!(!encoded
            .windows(json_encoded_secret.len())
            .any(|window| window == json_encoded_secret));
    }

    #[test]
    fn tampered_ciphertext_and_authorization_fail_closed() {
        let (_, root, existing, receiver, scanned) = pairing_fixture();
        let transfer = create_pairing_transfer(&existing, &root, &scanned, NOW).unwrap();
        let mut value: serde_json::Value =
            serde_json::from_slice(&transfer.to_payload().unwrap()).unwrap();
        let ciphertext = value["ciphertext"].as_array_mut().unwrap();
        let first = ciphertext[0].as_u64().unwrap();
        ciphertext[0] = serde_json::Value::from(first ^ 1);
        let tampered = PairingTransfer::from_payload(&serde_json::to_vec(&value).unwrap()).unwrap();
        assert_eq!(
            receiver.complete(&tampered, NOW).unwrap_err(),
            PairingError::AuthenticationFailed
        );

        let (_, root, existing, receiver, scanned) = pairing_fixture();
        let transfer = create_pairing_transfer(&existing, &root, &scanned, NOW).unwrap();
        let mut value: serde_json::Value =
            serde_json::from_slice(&transfer.to_payload().unwrap()).unwrap();
        let signature = value["authorization_signature"].as_array_mut().unwrap();
        let first = signature[0].as_u64().unwrap();
        signature[0] = serde_json::Value::from(first ^ 1);
        let tampered = PairingTransfer::from_payload(&serde_json::to_vec(&value).unwrap()).unwrap();
        assert_eq!(
            receiver.complete(&tampered, NOW).unwrap_err(),
            PairingError::AuthenticationFailed
        );

        let (_, root, existing, receiver, scanned) = pairing_fixture();
        let transfer = create_pairing_transfer(&existing, &root, &scanned, NOW).unwrap();
        let mut value: serde_json::Value =
            serde_json::from_slice(&transfer.to_payload().unwrap()).unwrap();
        let tag = value["authorization_tag"].as_array_mut().unwrap();
        let first = tag[0].as_u64().unwrap();
        tag[0] = serde_json::Value::from(first ^ 1);
        let tampered = PairingTransfer::from_payload(&serde_json::to_vec(&value).unwrap()).unwrap();
        assert_eq!(
            receiver.complete(&tampered, NOW).unwrap_err(),
            PairingError::AuthenticationFailed
        );
    }

    #[test]
    fn wrong_receiver_and_cross_user_sender_are_rejected() {
        let (user_id, root, existing, _receiver, scanned) = pairing_fixture();
        let transfer = create_pairing_transfer(&existing, &root, &scanned, NOW).unwrap();
        let wrong_receiver =
            PairingReceiver::begin(user_id, DeviceId::new(), NOW, DEFAULT_PAIRING_TTL_SECS)
                .unwrap();
        assert_eq!(
            wrong_receiver.complete(&transfer, NOW).unwrap_err(),
            PairingError::AuthenticationFailed
        );

        let other_root = RootIdentityKey::generate();
        let other_device =
            MlsDevice::generate(UserId::new(), DeviceId::new(), &other_root).unwrap();
        assert_eq!(
            create_pairing_transfer(&other_device, &other_root, &scanned, NOW).unwrap_err(),
            PairingError::UserMismatch
        );
    }

    #[test]
    fn expiry_unknown_fields_and_size_caps_are_enforced() {
        let (_, root, existing, receiver, scanned) = pairing_fixture();
        assert_eq!(
            create_pairing_transfer(&existing, &root, &scanned, NOW + DEFAULT_PAIRING_TTL_SECS)
                .unwrap_err(),
            PairingError::Expired
        );

        let mut value: serde_json::Value =
            serde_json::from_slice(&receiver.qr_payload().unwrap()).unwrap();
        value["extra"] = serde_json::Value::Bool(true);
        assert_eq!(
            ScannedPairingOffer::from_qr_payload(&serde_json::to_vec(&value).unwrap(), NOW)
                .unwrap_err(),
            PairingError::SerializationFailed
        );
        assert_eq!(
            ScannedPairingOffer::from_qr_payload(&vec![0; MAX_PAIRING_OFFER_BYTES + 1], NOW)
                .unwrap_err(),
            PairingError::InvalidPayload
        );
        assert_eq!(
            PairingTransfer::from_payload(&vec![0; MAX_PAIRING_TRANSFER_BYTES + 1]).unwrap_err(),
            PairingError::InvalidPayload
        );
    }

    #[test]
    fn debug_output_redacts_pairing_secrets_and_root_identity() {
        let (_, root, existing, receiver, scanned) = pairing_fixture();
        assert_eq!(
            format!("{receiver:?}"),
            "PairingReceiver(<key material redacted>)"
        );
        assert_eq!(
            format!("{scanned:?}"),
            "ScannedPairingOffer(<QR secret redacted>)"
        );
        let transfer = create_pairing_transfer(&existing, &root, &scanned, NOW).unwrap();
        assert_eq!(
            format!("{transfer:?}"),
            "PairingTransfer(<encrypted root identity>)"
        );
    }
}
