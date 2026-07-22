//! Opaque bridge from authenticated MLS epochs to LiveKit's native frame cryptor.
//!
//! Frame protection remains inside LiveKit's upstream libwebrtc integration.
//! This module owns the native key provider and deliberately exposes no method
//! that returns exporter or frame-encryption key bytes.

use core::fmt;

use filament_core::GroupId;
use libwebrtc::native::frame_cryptor::{KeyDerivationAlgorithm, KeyProvider, KeyProviderOptions};

use crate::{error::MediaError, media::MediaEpochSecret};

const LIVEKIT_MEDIA_KEY_RING_SIZE: i32 = 256;
const LIVEKIT_MEDIA_KEY_INDEX_MODULUS: u64 = 256;
const LIVEKIT_RATCHET_WINDOW_SIZE: i32 = 16;
const LIVEKIT_RATCHET_SALT: &[u8] = b"FilamentLiveKitMediaV1";

/// MLS-backed keyring for LiveKit's native AES-GCM frame cryptor.
///
/// Callers can inspect only the authenticated group and MLS epoch. The native
/// provider is retained privately so a later in-crate media-room adapter can
/// attach it directly to RTP senders and receivers without crossing IPC.
pub struct LiveKitMediaKeyring {
    group_id: GroupId,
    epoch: u64,
    provider: KeyProvider,
}

impl LiveKitMediaKeyring {
    /// Create a native keyring from the current authenticated MLS media epoch.
    ///
    /// # Errors
    /// Returns [`MediaError::KeyInstallationFailed`] if the native provider
    /// refuses the key.
    pub fn new(secret: MediaEpochSecret) -> Result<Self, MediaError> {
        let group_id = secret.group_id();
        let epoch = secret.epoch();
        let provider = KeyProvider::new(KeyProviderOptions {
            shared_key: true,
            ratchet_window_size: LIVEKIT_RATCHET_WINDOW_SIZE,
            ratchet_salt: LIVEKIT_RATCHET_SALT.to_vec(),
            failure_tolerance: 0,
            key_ring_size: LIVEKIT_MEDIA_KEY_RING_SIZE,
            key_derivation_algorithm: KeyDerivationAlgorithm::HKDF,
        });
        install(&provider, &secret)?;
        drop(secret);
        Ok(Self {
            group_id,
            epoch,
            provider,
        })
    }

    /// Authenticated MLS group supplying this keyring.
    #[must_use]
    pub const fn group_id(&self) -> GroupId {
        self.group_id
    }

    /// Current authenticated MLS media epoch.
    #[must_use]
    pub const fn epoch(&self) -> u64 {
        self.epoch
    }

    /// Install the next accepted MLS epoch into the native keyring.
    ///
    /// Rotation is strictly sequential and group-bound. Staged, stale,
    /// skipped, or cross-group secrets fail closed without changing the active
    /// epoch.
    ///
    /// # Errors
    /// Returns [`MediaError::InvalidEpochTransition`] unless `secret` is for
    /// exactly the next epoch of this group, or
    /// [`MediaError::KeyInstallationFailed`] if native installation fails.
    pub fn rotate(&mut self, secret: MediaEpochSecret) -> Result<(), MediaError> {
        let expected_epoch = self
            .epoch
            .checked_add(1)
            .ok_or(MediaError::InvalidEpochTransition)?;
        if secret.group_id() != self.group_id || secret.epoch() != expected_epoch {
            return Err(MediaError::InvalidEpochTransition);
        }
        let epoch = secret.epoch();
        install(&self.provider, &secret)?;
        drop(secret);
        self.epoch = epoch;
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn provider(&self) -> KeyProvider {
        self.provider.clone()
    }

    #[cfg(test)]
    pub(crate) fn key_index(&self) -> i32 {
        key_index(self.epoch)
    }
}

impl fmt::Debug for LiveKitMediaKeyring {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LiveKitMediaKeyring")
            .field("group_id", &self.group_id)
            .field("epoch", &self.epoch)
            .field("provider", &"<native media keys omitted>")
            .finish()
    }
}

fn install(provider: &KeyProvider, secret: &MediaEpochSecret) -> Result<(), MediaError> {
    let installed = provider.set_shared_key(key_index(secret.epoch()), secret.key_bytes().to_vec());
    if !installed {
        return Err(MediaError::KeyInstallationFailed);
    }
    Ok(())
}

fn key_index(epoch: u64) -> i32 {
    i32::try_from(epoch % LIVEKIT_MEDIA_KEY_INDEX_MODULUS)
        .expect("media key index modulus always fits i32")
}

#[cfg(test)]
mod tests {
    use libwebrtc::native::frame_cryptor::{
        DataPacketCryptor, EncryptedPacket, EncryptionAlgorithm,
    };

    use super::*;

    fn secret(group_id: GroupId, epoch: u64, byte: u8) -> MediaEpochSecret {
        MediaEpochSecret::new(group_id, epoch, [byte; 32])
    }

    #[test]
    fn keyring_metadata_and_debug_are_secret_free() {
        let group_id = GroupId::new();
        let keyring = LiveKitMediaKeyring::new(secret(group_id, 7, 0x41)).unwrap();

        assert_eq!(keyring.group_id(), group_id);
        assert_eq!(keyring.epoch(), 7);
        assert_eq!(keyring.key_index(), 7);
        let debug = format!("{keyring:?}");
        assert!(debug.contains("native media keys omitted"));
        assert!(!debug.contains("65, 65"));
    }

    #[test]
    fn rotation_requires_exact_group_bound_next_epoch() {
        let group_id = GroupId::new();
        let mut keyring = LiveKitMediaKeyring::new(secret(group_id, 3, 0x11)).unwrap();

        assert_eq!(
            keyring.rotate(secret(GroupId::new(), 4, 0x22)).unwrap_err(),
            MediaError::InvalidEpochTransition
        );
        assert_eq!(
            keyring.rotate(secret(group_id, 5, 0x22)).unwrap_err(),
            MediaError::InvalidEpochTransition
        );
        assert_eq!(keyring.epoch(), 3);

        keyring.rotate(secret(group_id, 4, 0x22)).unwrap();
        assert_eq!(keyring.epoch(), 4);
    }

    #[test]
    fn endpoints_decrypt_while_an_opaque_relay_and_wrong_key_cannot() {
        let group_id = GroupId::new();
        let sender = LiveKitMediaKeyring::new(secret(group_id, 1, 0x31)).unwrap();
        let receiver = LiveKitMediaKeyring::new(secret(group_id, 1, 0x31)).unwrap();
        let outsider = LiveKitMediaKeyring::new(secret(group_id, 1, 0x99)).unwrap();

        let sender_cryptor = cryptor(&sender);
        let receiver_cryptor = cryptor(&receiver);
        let outsider_cryptor = cryptor(&outsider);
        let plaintext = b"encoded audio frame";
        let relayed = sender_cryptor
            .encrypt(
                "sender-device",
                sender.key_index().try_into().unwrap(),
                plaintext,
            )
            .unwrap();

        assert_ne!(relayed.data, plaintext);
        assert_eq!(
            receiver_cryptor.decrypt("sender-device", &relayed).unwrap(),
            plaintext
        );
        assert!(outsider_cryptor
            .decrypt("sender-device", &clone_packet(&relayed))
            .is_err());
    }

    #[test]
    fn accepted_epoch_rotation_excludes_removed_endpoint_from_new_ciphertext() {
        let group_id = GroupId::new();
        let mut sender = LiveKitMediaKeyring::new(secret(group_id, 9, 0x51)).unwrap();
        let mut receiver = LiveKitMediaKeyring::new(secret(group_id, 9, 0x51)).unwrap();
        let removed = LiveKitMediaKeyring::new(secret(group_id, 9, 0x51)).unwrap();
        let old_cryptor = cryptor(&sender);
        let old_frame = old_cryptor
            .encrypt(
                "sender-device",
                sender.key_index().try_into().unwrap(),
                b"old",
            )
            .unwrap();

        sender.rotate(secret(group_id, 10, 0x61)).unwrap();
        receiver.rotate(secret(group_id, 10, 0x61)).unwrap();
        let new_cryptor = cryptor(&sender);
        let new_frame = new_cryptor
            .encrypt(
                "sender-device",
                sender.key_index().try_into().unwrap(),
                b"new",
            )
            .unwrap();

        assert_eq!(
            cryptor(&receiver)
                .decrypt("sender-device", &clone_packet(&new_frame))
                .unwrap(),
            b"new"
        );
        assert!(cryptor(&removed)
            .decrypt("sender-device", &clone_packet(&new_frame))
            .is_err());
        assert_eq!(
            cryptor(&receiver)
                .decrypt("sender-device", &old_frame)
                .unwrap(),
            b"old"
        );
    }

    fn cryptor(keyring: &LiveKitMediaKeyring) -> DataPacketCryptor {
        DataPacketCryptor::new(EncryptionAlgorithm::AesGcm, keyring.provider())
    }

    fn clone_packet(packet: &EncryptedPacket) -> EncryptedPacket {
        EncryptedPacket {
            data: packet.data.clone(),
            iv: packet.iv.clone(),
            key_index: packet.key_index,
        }
    }
}
