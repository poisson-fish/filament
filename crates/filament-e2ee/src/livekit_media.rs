//! Opaque bridge from authenticated MLS epochs to LiveKit's native frame cryptor.
//!
//! Frame protection remains inside LiveKit's upstream libwebrtc integration.
//! This module owns the native key provider and deliberately exposes no method
//! that returns exporter or frame-encryption key bytes.

use std::{collections::HashMap, fmt};

use filament_core::{GroupId, LiveKitIdentity};
use libwebrtc::{
    native::frame_cryptor::{
        EncryptionAlgorithm, FrameCryptor, KeyDerivationAlgorithm, KeyProvider, KeyProviderOptions,
    },
    peer_connection_factory::PeerConnectionFactory,
    rtp_receiver::RtpReceiver,
    rtp_sender::RtpSender,
};

use crate::{error::MediaError, media::MediaEpochSecret};

const LIVEKIT_MEDIA_KEY_RING_SIZE: i32 = 256;
const LIVEKIT_MEDIA_KEY_INDEX_MODULUS: u64 = 256;
const LIVEKIT_RATCHET_WINDOW_SIZE: i32 = 16;
const LIVEKIT_RATCHET_SALT: &[u8] = b"FilamentLiveKitMediaV1";
/// Maximum simultaneously attached encrypted RTP senders and receivers.
pub const MAX_LIVEKIT_MEDIA_TRACKS: usize = 256;
/// Maximum encoded bytes in a LiveKit track SID.
pub const MAX_LIVEKIT_MEDIA_TRACK_ID_BYTES: usize = 128;

/// Validated LiveKit track SID used to bind a native frame cryptor.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LiveKitMediaTrackId(String);

impl LiveKitMediaTrackId {
    /// Borrow the validated identifier.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for LiveKitMediaTrackId {
    type Error = MediaError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        if value.is_empty()
            || value.len() > MAX_LIVEKIT_MEDIA_TRACK_ID_BYTES
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
        {
            return Err(MediaError::InvalidTrackId);
        }
        Ok(Self(value))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum LiveKitMediaTrackDirection {
    Sender,
    Receiver,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct LiveKitMediaBinding {
    track_id: LiveKitMediaTrackId,
    direction: LiveKitMediaTrackDirection,
}

/// MLS-backed keyring for LiveKit's native AES-GCM frame cryptor.
///
/// Callers can inspect only the authenticated group and MLS epoch. The native
/// provider is retained privately so a later in-crate media-room adapter can
/// attach it directly to RTP senders and receivers without crossing IPC.
pub struct LiveKitMediaKeyring {
    group_id: GroupId,
    epoch: u64,
    provider: KeyProvider,
    cryptors: HashMap<LiveKitMediaBinding, FrameCryptor>,
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
            cryptors: HashMap::new(),
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

    /// Number of RTP senders and receivers protected by this keyring.
    #[must_use]
    pub fn attached_track_count(&self) -> usize {
        self.cryptors.len()
    }

    /// Attach encryption to a locally published RTP sender.
    ///
    /// The frame cryptor is always enabled before this method returns and is
    /// retained privately for the life of the binding. The sender must come
    /// from LiveKit's local-track-published callback, where it is guaranteed to
    /// have a live media track; callers must not publish until this succeeds.
    /// There is intentionally no API to disable encryption or retrieve the
    /// cryptor/provider.
    ///
    /// # Errors
    /// Returns [`MediaError::DuplicateTrack`] for a repeated sender binding or
    /// [`MediaError::TrackLimitExceeded`] after the hard per-call cap.
    pub fn attach_sender(
        &mut self,
        factory: &PeerConnectionFactory,
        participant: &LiveKitIdentity,
        track_id: LiveKitMediaTrackId,
        sender: RtpSender,
    ) -> Result<(), MediaError> {
        let binding = LiveKitMediaBinding {
            track_id,
            direction: LiveKitMediaTrackDirection::Sender,
        };
        self.ensure_attachable(&binding)?;
        let cryptor = FrameCryptor::new_for_rtp_sender(
            factory,
            participant.as_str().to_owned(),
            EncryptionAlgorithm::AesGcm,
            self.provider.clone(),
            sender,
        );
        configure_cryptor(&cryptor, self.epoch);
        self.cryptors.insert(binding, cryptor);
        Ok(())
    }

    /// Attach decryption to a subscribed remote RTP receiver.
    ///
    /// The frame cryptor is always enabled before this method returns and is
    /// retained privately for the life of the binding. The receiver must come
    /// from LiveKit's track-subscribed callback, where it is guaranteed to have
    /// a live media track; callers must not render until this succeeds. There
    /// is intentionally no plaintext fallback when attachment cannot be
    /// established.
    ///
    /// # Errors
    /// Returns [`MediaError::DuplicateTrack`] for a repeated receiver binding
    /// or [`MediaError::TrackLimitExceeded`] after the hard per-call cap.
    pub fn attach_receiver(
        &mut self,
        factory: &PeerConnectionFactory,
        participant: &LiveKitIdentity,
        track_id: LiveKitMediaTrackId,
        receiver: RtpReceiver,
    ) -> Result<(), MediaError> {
        let binding = LiveKitMediaBinding {
            track_id,
            direction: LiveKitMediaTrackDirection::Receiver,
        };
        self.ensure_attachable(&binding)?;
        let cryptor = FrameCryptor::new_for_rtp_receiver(
            factory,
            participant.as_str().to_owned(),
            EncryptionAlgorithm::AesGcm,
            self.provider.clone(),
            receiver,
        );
        configure_cryptor(&cryptor, self.epoch);
        self.cryptors.insert(binding, cryptor);
        Ok(())
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
        for cryptor in self.cryptors.values() {
            cryptor.set_key_index(key_index(epoch));
        }
        self.epoch = epoch;
        Ok(())
    }

    fn ensure_attachable(&self, binding: &LiveKitMediaBinding) -> Result<(), MediaError> {
        validate_attachment(self.cryptors.len(), self.cryptors.contains_key(binding))
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

fn validate_attachment(attached_tracks: usize, duplicate: bool) -> Result<(), MediaError> {
    if duplicate {
        return Err(MediaError::DuplicateTrack);
    }
    if attached_tracks >= MAX_LIVEKIT_MEDIA_TRACKS {
        return Err(MediaError::TrackLimitExceeded);
    }
    Ok(())
}

impl fmt::Debug for LiveKitMediaKeyring {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LiveKitMediaKeyring")
            .field("group_id", &self.group_id)
            .field("epoch", &self.epoch)
            .field("attached_tracks", &self.cryptors.len())
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

fn configure_cryptor(cryptor: &FrameCryptor, epoch: u64) {
    cryptor.set_key_index(key_index(epoch));
    cryptor.set_enabled(true);
}

#[cfg(test)]
mod tests {
    use filament_core::LiveKitIdentity;
    use libwebrtc::native::frame_cryptor::{
        DataPacketCryptor, EncryptedPacket, EncryptionAlgorithm,
    };
    use libwebrtc::{
        audio_source::{native::NativeAudioSource, AudioSourceOptions},
        peer_connection::{AnswerOptions, OfferOptions},
        peer_connection_factory::{
            native::PeerConnectionFactoryExt, PeerConnectionFactory, RtcConfiguration,
        },
    };

    use super::*;

    fn secret(group_id: GroupId, epoch: u64, byte: u8) -> MediaEpochSecret {
        MediaEpochSecret::new(group_id, epoch, [byte; 32])
    }

    fn track_id(value: &str) -> LiveKitMediaTrackId {
        LiveKitMediaTrackId::try_from(value.to_owned()).unwrap()
    }

    fn participant(value: &str) -> LiveKitIdentity {
        LiveKitIdentity::try_from(value.to_owned()).unwrap()
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
    fn track_identifiers_are_strictly_bounded() {
        assert_eq!(
            LiveKitMediaTrackId::try_from(String::new()).unwrap_err(),
            MediaError::InvalidTrackId
        );
        assert_eq!(
            LiveKitMediaTrackId::try_from("remote/track".to_owned()).unwrap_err(),
            MediaError::InvalidTrackId
        );
        assert_eq!(
            LiveKitMediaTrackId::try_from("x".repeat(MAX_LIVEKIT_MEDIA_TRACK_ID_BYTES + 1))
                .unwrap_err(),
            MediaError::InvalidTrackId
        );
        assert_eq!(track_id("TR_audio-01").as_str(), "TR_audio-01");
    }

    #[test]
    fn native_attachment_count_is_bounded_and_duplicates_fail_first() {
        assert_eq!(validate_attachment(0, false), Ok(()));
        assert_eq!(
            validate_attachment(MAX_LIVEKIT_MEDIA_TRACKS, false),
            Err(MediaError::TrackLimitExceeded)
        );
        assert_eq!(
            validate_attachment(MAX_LIVEKIT_MEDIA_TRACKS, true),
            Err(MediaError::DuplicateTrack)
        );
    }

    #[tokio::test]
    async fn native_rtp_bindings_are_enabled_and_follow_authenticated_epoch_rotation() {
        let group_id = GroupId::new();
        let mut keyring = LiveKitMediaKeyring::new(secret(group_id, 17, 0x71)).unwrap();
        let factory = PeerConnectionFactory::default();
        let publisher = factory
            .create_peer_connection(RtcConfiguration::default())
            .unwrap();
        let subscriber = factory
            .create_peer_connection(RtcConfiguration::default())
            .unwrap();
        let source = NativeAudioSource::new(AudioSourceOptions::default(), 48_000, 1, 100);
        let track = factory.create_audio_track("filament-test", source);
        let sender = publisher
            .add_track(track.into(), &["filament-test"])
            .unwrap();

        let offer = publisher
            .create_offer(OfferOptions {
                offer_to_receive_audio: true,
                ..OfferOptions::default()
            })
            .await
            .unwrap();
        publisher
            .set_local_description(offer.clone())
            .await
            .unwrap();
        subscriber.set_remote_description(offer).await.unwrap();
        let answer = subscriber
            .create_answer(AnswerOptions::default())
            .await
            .unwrap();
        subscriber
            .set_local_description(answer.clone())
            .await
            .unwrap();
        publisher.set_remote_description(answer).await.unwrap();
        let receiver = subscriber.receivers().into_iter().next().unwrap();

        keyring
            .attach_sender(
                &factory,
                &participant("user_device_local"),
                track_id("TR_sender"),
                sender,
            )
            .unwrap();
        keyring
            .attach_receiver(
                &factory,
                &participant("user_device_remote"),
                track_id("TR_receiver"),
                receiver,
            )
            .unwrap();

        assert_eq!(keyring.attached_track_count(), 2);
        for cryptor in keyring.cryptors.values() {
            assert!(cryptor.enabled());
            assert_eq!(cryptor.key_index(), 17);
        }

        keyring.rotate(secret(group_id, 18, 0x72)).unwrap();
        for cryptor in keyring.cryptors.values() {
            assert!(cryptor.enabled());
            assert_eq!(cryptor.key_index(), 18);
        }
        drop(keyring);
        subscriber.close();
        publisher.close();
    }

    #[tokio::test]
    async fn duplicate_native_binding_fails_closed() {
        let group_id = GroupId::new();
        let mut keyring = LiveKitMediaKeyring::new(secret(group_id, 1, 0x31)).unwrap();
        let factory = PeerConnectionFactory::default();
        let publisher = factory
            .create_peer_connection(RtcConfiguration::default())
            .unwrap();
        let subscriber = factory
            .create_peer_connection(RtcConfiguration::default())
            .unwrap();
        let source = NativeAudioSource::new(AudioSourceOptions::default(), 48_000, 1, 100);
        let track = factory.create_audio_track("filament-test", source);
        let sender = publisher
            .add_track(track.into(), &["filament-test"])
            .unwrap();
        let offer = publisher
            .create_offer(OfferOptions::default())
            .await
            .unwrap();
        publisher
            .set_local_description(offer.clone())
            .await
            .unwrap();
        subscriber.set_remote_description(offer).await.unwrap();
        let answer = subscriber
            .create_answer(AnswerOptions::default())
            .await
            .unwrap();
        subscriber
            .set_local_description(answer.clone())
            .await
            .unwrap();
        publisher.set_remote_description(answer).await.unwrap();
        let identity = participant("user_device_local");

        keyring
            .attach_sender(
                &factory,
                &identity,
                track_id("TR_duplicate"),
                sender.clone(),
            )
            .unwrap();
        assert_eq!(
            keyring
                .attach_sender(&factory, &identity, track_id("TR_duplicate"), sender,)
                .unwrap_err(),
            MediaError::DuplicateTrack
        );
        assert_eq!(keyring.attached_track_count(), 1);
        drop(keyring);
        subscriber.close();
        publisher.close();
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
