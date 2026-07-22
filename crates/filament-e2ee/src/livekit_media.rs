//! Opaque bridge from authenticated MLS epochs to LiveKit's native frame cryptor.
//!
//! Frame protection remains inside LiveKit's upstream libwebrtc integration.
//! This module owns the native key provider and deliberately exposes no method
//! that returns exporter or frame-encryption key bytes.

use std::{
    collections::HashMap,
    fmt,
    net::IpAddr,
    sync::{
        atomic::{AtomicI32, AtomicU8, Ordering},
        Arc,
    },
};

use filament_core::{GroupId, LiveKitIdentity, LiveKitRoomName};
use libwebrtc::{
    native::frame_cryptor::{
        EncryptionAlgorithm, EncryptionState, FrameCryptor, KeyDerivationAlgorithm, KeyProvider,
        KeyProviderOptions,
    },
    peer_connection_factory::PeerConnectionFactory,
    rtp_receiver::RtpReceiver,
    rtp_sender::RtpSender,
};
use livekit::{
    e2ee::{
        key_provider::{
            KeyProvider as RoomKeyProvider, KeyProviderOptions as RoomKeyProviderOptions,
        },
        EncryptionType,
    },
    options::TrackPublishOptions,
    prelude::{LocalTrack, LocalTrackPublication, Room, RoomEvent, RoomOptions},
};
use rand::{rngs::OsRng, RngCore};
use tokio::{
    sync::{mpsc::UnboundedReceiver, oneshot},
    task::JoinHandle,
};
use url::{Host, Url};
use zeroize::Zeroizing;

use crate::{error::MediaError, media::MediaEpochSecret};

const LIVEKIT_MEDIA_KEY_RING_SIZE: i32 = 256;
const LIVEKIT_MEDIA_KEY_INDEX_MODULUS: u64 = 256;
const LIVEKIT_RATCHET_WINDOW_SIZE: i32 = 16;
const LIVEKIT_RATCHET_SALT: &[u8] = b"FilamentLiveKitMediaV1";
/// Maximum simultaneously attached encrypted RTP senders and receivers.
pub const MAX_LIVEKIT_MEDIA_TRACKS: usize = 256;
/// Maximum encoded bytes in a LiveKit track SID.
pub const MAX_LIVEKIT_MEDIA_TRACK_ID_BYTES: usize = 128;
/// Maximum encoded bytes in a native LiveKit signaling URL.
pub const MAX_LIVEKIT_MEDIA_URL_BYTES: usize = 256;
/// Maximum encoded bytes in a short-lived LiveKit JWT.
pub const MAX_LIVEKIT_MEDIA_ACCESS_TOKEN_BYTES: usize = 8 * 1024;

const ROOM_HEALTHY: u8 = 0;
const ROOM_CLOSED: u8 = 1;
const ROOM_REJECTED: u8 = 2;

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

/// Validated, short-lived native connection material returned by Filament.
///
/// The access token is zeroized on drop and omitted from debug output. The
/// expected room and participant bindings are checked again after LiveKit
/// connects so a hostile signaling service cannot silently redirect a call.
pub struct LiveKitMediaConnection {
    server_url: String,
    access_token: Zeroizing<String>,
    room_name: LiveKitRoomName,
    participant: LiveKitIdentity,
}

impl LiveKitMediaConnection {
    /// Validate one native LiveKit connection response.
    ///
    /// Production endpoints must use `wss`. Plain `ws` is accepted only for a
    /// literal loopback address or `localhost`, preserving local integration
    /// tests without permitting plaintext remote signaling.
    ///
    /// # Errors
    /// Returns a typed media error for an unsafe URL or malformed JWT.
    pub fn new(
        server_url: String,
        access_token: String,
        room_name: LiveKitRoomName,
        participant: LiveKitIdentity,
    ) -> Result<Self, MediaError> {
        let server_url = validate_server_url(server_url)?;
        validate_access_token(&access_token)?;
        Ok(Self {
            server_url,
            access_token: Zeroizing::new(access_token),
            room_name,
            participant,
        })
    }
}

impl fmt::Debug for LiveKitMediaConnection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LiveKitMediaConnection")
            .field("server_url", &self.server_url)
            .field("room_name", &self.room_name)
            .field("participant", &self.participant)
            .field("access_token", &"<redacted>")
            .finish()
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
    room_provider: RoomKeyProvider,
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
        let room_options = RoomKeyProviderOptions {
            ratchet_window_size: LIVEKIT_RATCHET_WINDOW_SIZE,
            ratchet_salt: LIVEKIT_RATCHET_SALT.to_vec(),
            failure_tolerance: 0,
            key_ring_size: LIVEKIT_MEDIA_KEY_RING_SIZE,
            key_derivation_algorithm: KeyDerivationAlgorithm::HKDF,
        };
        let mut bootstrap_key = Zeroizing::new([0_u8; 32]);
        OsRng.fill_bytes(bootstrap_key.as_mut());
        let room_provider = RoomKeyProvider::with_shared_key(room_options, bootstrap_key.to_vec());
        install(&provider, &secret)?;
        install_room(&room_provider, &secret)?;
        drop(secret);
        Ok(Self {
            group_id,
            epoch,
            provider,
            room_provider,
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
        install_room(&self.room_provider, &secret)?;
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

    fn key_index(&self) -> i32 {
        key_index(self.epoch)
    }

    fn encrypted_room_options(&self) -> RoomOptions {
        let mut options = RoomOptions::default();
        options.auto_subscribe = false;
        options.encryption = Some(livekit::E2eeOptions {
            encryption_type: EncryptionType::Gcm,
            key_provider: self.room_provider.clone(),
        });
        options
    }
}

/// Fail-closed native LiveKit room bound to one authenticated MLS group.
///
/// The underlying SDK room, event receiver, E2EE manager, and key providers
/// remain private. Automatic remote subscription is disabled. A dedicated
/// native guard drains the SDK event stream and closes the room if LiveKit
/// surfaces an unencrypted publication, a disabled/missing cryptor, or a frame
/// encryption failure. This type deliberately has no encryption-disable API.
pub struct LiveKitMediaRoom {
    room: Arc<Room>,
    keyring: LiveKitMediaKeyring,
    room_name: LiveKitRoomName,
    participant: LiveKitIdentity,
    current_key_index: Arc<AtomicI32>,
    health: Arc<AtomicU8>,
    guard_shutdown: Option<oneshot::Sender<()>>,
    guard: Option<JoinHandle<()>>,
}

impl LiveKitMediaRoom {
    /// Connect to LiveKit with native frame encryption enabled before the room
    /// is created and automatic remote subscriptions disabled.
    ///
    /// # Errors
    /// Returns an opaque room failure when signaling fails, or a binding/
    /// encryption error when the connected room contradicts authenticated
    /// Filament metadata.
    pub async fn connect(
        connection: LiveKitMediaConnection,
        secret: MediaEpochSecret,
    ) -> Result<Self, MediaError> {
        let LiveKitMediaConnection {
            server_url,
            access_token,
            room_name,
            participant,
        } = connection;
        let keyring = LiveKitMediaKeyring::new(secret)?;
        let options = keyring.encrypted_room_options();
        let (room, events) = Box::pin(Room::connect(&server_url, access_token.as_str(), options))
            .await
            .map_err(|_| MediaError::RoomUnavailable)?;
        let room = Arc::new(room);

        if room.name() != room_name.as_str() {
            close_rejected_room(&room).await;
            return Err(MediaError::RoomBindingMismatch);
        }
        if room.local_participant().identity().as_str() != participant.as_str() {
            close_rejected_room(&room).await;
            return Err(MediaError::ParticipantBindingMismatch);
        }
        let manager = room.e2ee_manager();
        if !manager.enabled()
            || manager.encryption_type() != EncryptionType::Gcm
            || manager
                .key_provider()
                .is_none_or(|provider| provider.get_latest_key_index() != keyring.key_index())
        {
            close_rejected_room(&room).await;
            return Err(MediaError::EncryptionUnavailable);
        }

        let current_key_index = Arc::new(AtomicI32::new(keyring.key_index()));
        let health = Arc::new(AtomicU8::new(ROOM_HEALTHY));
        let (guard_shutdown, shutdown) = oneshot::channel();
        let guard = tokio::spawn(guard_room_events(
            room.clone(),
            events,
            current_key_index.clone(),
            health.clone(),
            shutdown,
        ));

        Ok(Self {
            room,
            keyring,
            room_name,
            participant,
            current_key_index,
            health,
            guard_shutdown: Some(guard_shutdown),
            guard: Some(guard),
        })
    }

    /// Authenticated MLS group supplying the room's media keys.
    #[must_use]
    pub const fn group_id(&self) -> GroupId {
        self.keyring.group_id()
    }

    /// Current accepted MLS media epoch.
    #[must_use]
    pub const fn epoch(&self) -> u64 {
        self.keyring.epoch()
    }

    /// Authenticated LiveKit room binding.
    #[must_use]
    pub const fn room_name(&self) -> &LiveKitRoomName {
        &self.room_name
    }

    /// Authenticated local LiveKit participant binding.
    #[must_use]
    pub const fn participant(&self) -> &LiveKitIdentity {
        &self.participant
    }

    /// Publish a native track only after its SDK-owned frame cryptor exists,
    /// is enabled, and uses the current authenticated MLS epoch.
    ///
    /// The track is disabled during publication to prevent an encoded frame
    /// from racing cryptor setup. Its prior enabled state is restored only
    /// after all checks pass. Failed setup immediately unpublishes the track.
    ///
    /// # Errors
    /// Returns a typed error if the room guard rejected the call, the track cap
    /// was reached, publication failed, or encryption was not attached.
    pub async fn publish_track(
        &mut self,
        track: LocalTrack,
        options: TrackPublishOptions,
    ) -> Result<LiveKitMediaTrackId, MediaError> {
        self.ensure_healthy()?;
        if self.room.e2ee_manager().frame_cryptors().len() >= MAX_LIVEKIT_MEDIA_TRACKS {
            return Err(MediaError::TrackLimitExceeded);
        }

        let was_enabled = track.is_enabled();
        track.disable();
        let participant = self.room.local_participant();
        let Ok(publication) = participant.publish_track(track.clone(), options).await else {
            if was_enabled {
                track.enable();
            }
            return Err(MediaError::RoomUnavailable);
        };

        let result = configure_published_track(
            &self.room,
            &publication,
            participant.identity().as_str(),
            self.keyring.key_index(),
        );
        let track_id = match result {
            Ok(track_id) => track_id,
            Err(error) => {
                let _ = participant.unpublish_track(&publication.sid()).await;
                return Err(error);
            }
        };
        if was_enabled {
            track.enable();
        }
        Ok(track_id)
    }

    /// Rotate all native cryptors after the exact next MLS epoch is accepted.
    ///
    /// # Errors
    /// Returns a typed media error for an unhealthy room, invalid epoch
    /// transition, or rejected native key installation.
    pub fn rotate(&mut self, secret: MediaEpochSecret) -> Result<(), MediaError> {
        self.ensure_healthy()?;
        self.keyring.rotate(secret)?;
        let key_index = self.keyring.key_index();
        self.current_key_index.store(key_index, Ordering::Release);
        for cryptor in self.room.e2ee_manager().frame_cryptors().values() {
            if !cryptor.enabled() {
                self.health.store(ROOM_REJECTED, Ordering::Release);
                return Err(MediaError::EncryptionUnavailable);
            }
            cryptor.set_key_index(key_index);
        }
        Ok(())
    }

    /// Close the native room and wait for its event guard to stop.
    ///
    /// # Errors
    /// Returns an opaque failure if LiveKit cannot complete shutdown.
    pub async fn close(mut self) -> Result<(), MediaError> {
        if let Some(shutdown) = self.guard_shutdown.take() {
            let _ = shutdown.send(());
        }
        if let Some(guard) = self.guard.take() {
            guard.await.map_err(|_| MediaError::RoomUnavailable)?;
        }
        self.health.store(ROOM_CLOSED, Ordering::Release);
        Ok(())
    }

    fn ensure_healthy(&self) -> Result<(), MediaError> {
        match self.health.load(Ordering::Acquire) {
            ROOM_HEALTHY => Ok(()),
            ROOM_CLOSED => Err(MediaError::RoomUnavailable),
            _ => Err(MediaError::UnsafeTrack),
        }
    }
}

impl fmt::Debug for LiveKitMediaRoom {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LiveKitMediaRoom")
            .field("group_id", &self.group_id())
            .field("epoch", &self.epoch())
            .field("room_name", &self.room_name)
            .field("participant", &self.participant)
            .field("native_room", &"<opaque encrypted media state>")
            .finish_non_exhaustive()
    }
}

impl Drop for LiveKitMediaRoom {
    fn drop(&mut self) {
        if let Some(shutdown) = self.guard_shutdown.take() {
            let _ = shutdown.send(());
        }
    }
}

async fn guard_room_events(
    room: Arc<Room>,
    mut events: UnboundedReceiver<RoomEvent>,
    current_key_index: Arc<AtomicI32>,
    health: Arc<AtomicU8>,
    mut shutdown: oneshot::Receiver<()>,
) {
    loop {
        tokio::select! {
            _ = &mut shutdown => {
                health.store(ROOM_CLOSED, Ordering::Release);
                let _ = room.close().await;
                return;
            }
            event = events.recv() => {
                let Some(event) = event else {
                    health.store(ROOM_CLOSED, Ordering::Release);
                    return;
                };
                let key_index = current_key_index.load(Ordering::Acquire);
                if !validate_room_event(&room, &event, key_index) {
                    health.store(ROOM_REJECTED, Ordering::Release);
                    let _ = room.close().await;
                    return;
                }
                if matches!(event, RoomEvent::Disconnected { .. }) {
                    health.store(ROOM_CLOSED, Ordering::Release);
                    return;
                }
            }
        }
    }
}

fn validate_room_event(room: &Room, event: &RoomEvent, key_index: i32) -> bool {
    if !room.e2ee_manager().enabled()
        || room.e2ee_manager().encryption_type() != EncryptionType::Gcm
        || room.e2ee_manager().frame_cryptors().len() > MAX_LIVEKIT_MEDIA_TRACKS
    {
        return false;
    }

    match event {
        RoomEvent::Connected {
            participants_with_tracks,
        } => participants_with_tracks.iter().all(|(_, publications)| {
            publications.len() <= MAX_LIVEKIT_MEDIA_TRACKS
                && publications
                    .iter()
                    .all(|publication| publication.encryption_type() == EncryptionType::Gcm)
        }),
        RoomEvent::TrackPublished { publication, .. } => {
            publication.encryption_type() == EncryptionType::Gcm
        }
        RoomEvent::LocalTrackPublished {
            publication,
            participant,
            ..
        }
        | RoomEvent::LocalTrackRepublished {
            publication,
            participant,
            ..
        } => configure_room_cryptor(
            room,
            participant.identity().as_str(),
            publication.sid().as_str(),
            publication.encryption_type(),
            key_index,
        ),
        RoomEvent::TrackSubscribed {
            publication,
            participant,
            ..
        } => configure_room_cryptor(
            room,
            participant.identity().as_str(),
            publication.sid().as_str(),
            publication.encryption_type(),
            key_index,
        ),
        RoomEvent::E2eeStateChanged { state, .. } => matches!(
            state,
            EncryptionState::New | EncryptionState::Ok | EncryptionState::KeyRatcheted
        ),
        _ => true,
    }
}

fn configure_published_track(
    room: &Room,
    publication: &LocalTrackPublication,
    participant: &str,
    key_index: i32,
) -> Result<LiveKitMediaTrackId, MediaError> {
    let track_id = LiveKitMediaTrackId::try_from(publication.sid().as_str().to_owned())?;
    if !configure_room_cryptor(
        room,
        participant,
        track_id.as_str(),
        publication.encryption_type(),
        key_index,
    ) {
        return Err(MediaError::EncryptionUnavailable);
    }
    Ok(track_id)
}

fn configure_room_cryptor(
    room: &Room,
    participant: &str,
    track_id: &str,
    encryption_type: EncryptionType,
    key_index: i32,
) -> bool {
    if encryption_type != EncryptionType::Gcm {
        return false;
    }
    let cryptors = room.e2ee_manager().frame_cryptors();
    let Some(cryptor) = cryptors.iter().find_map(|((identity, sid), cryptor)| {
        (identity.as_str() == participant && sid.as_str() == track_id).then_some(cryptor)
    }) else {
        return false;
    };
    if !cryptor.enabled() {
        return false;
    }
    cryptor.set_key_index(key_index);
    cryptor.enabled() && cryptor.key_index() == key_index
}

async fn close_rejected_room(room: &Room) {
    let _ = room.close().await;
}

fn validate_server_url(value: String) -> Result<String, MediaError> {
    if value.is_empty() || value.len() > MAX_LIVEKIT_MEDIA_URL_BYTES || value.trim() != value {
        return Err(MediaError::InvalidServerUrl);
    }
    let parsed = Url::parse(&value).map_err(|_| MediaError::InvalidServerUrl)?;
    if parsed.cannot_be_a_base()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
    {
        return Err(MediaError::InvalidServerUrl);
    }
    let secure = parsed.scheme() == "wss";
    let loopback = parsed.scheme() == "ws"
        && parsed.host().is_some_and(|host| match host {
            Host::Domain(domain) => domain.eq_ignore_ascii_case("localhost"),
            Host::Ipv4(address) => IpAddr::V4(address).is_loopback(),
            Host::Ipv6(address) => IpAddr::V6(address).is_loopback(),
        });
    if (!secure && !loopback) || parsed.host().is_none() {
        return Err(MediaError::InvalidServerUrl);
    }
    Ok(value)
}

fn validate_access_token(value: &str) -> Result<(), MediaError> {
    if value.is_empty() || value.len() > MAX_LIVEKIT_MEDIA_ACCESS_TOKEN_BYTES {
        return Err(MediaError::InvalidAccessToken);
    }
    let segments = value.split('.').collect::<Vec<_>>();
    if segments.len() != 3
        || segments.iter().any(|segment| {
            segment.is_empty()
                || !segment
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        })
    {
        return Err(MediaError::InvalidAccessToken);
    }
    Ok(())
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
            .finish_non_exhaustive()
    }
}

fn install(provider: &KeyProvider, secret: &MediaEpochSecret) -> Result<(), MediaError> {
    let installed = provider.set_shared_key(key_index(secret.epoch()), secret.key_bytes().to_vec());
    if !installed {
        return Err(MediaError::KeyInstallationFailed);
    }
    Ok(())
}

fn install_room(provider: &RoomKeyProvider, secret: &MediaEpochSecret) -> Result<(), MediaError> {
    provider.set_shared_key(secret.key_bytes().to_vec(), key_index(secret.epoch()));
    let installed = Zeroizing::new(
        provider
            .get_shared_key(key_index(secret.epoch()))
            .ok_or(MediaError::KeyInstallationFailed)?,
    );
    if installed.as_slice() != secret.key_bytes() {
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
    use filament_core::{LiveKitIdentity, LiveKitRoomName};
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

    fn room_name(value: &str) -> LiveKitRoomName {
        LiveKitRoomName::try_from(value.to_owned()).unwrap()
    }

    fn access_token() -> String {
        String::from("eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJ1c2VyIn0.signature")
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
    fn native_connection_is_bounded_and_redacts_access_token() {
        let token = access_token();
        let connection = LiveKitMediaConnection::new(
            String::from("wss://livekit.example.test"),
            token.clone(),
            room_name("filament.voice.guild.channel"),
            participant("user_device_local"),
        )
        .unwrap();

        let debug = format!("{connection:?}");
        assert!(debug.contains("<redacted>"));
        assert!(!debug.contains(&token));
        assert_eq!(connection.server_url, "wss://livekit.example.test");
    }

    #[test]
    fn signaling_transport_rejects_remote_plaintext_and_url_smuggling() {
        for allowed in [
            "wss://livekit.example.test",
            "ws://localhost:7880",
            "ws://127.0.0.1:7880",
            "ws://[::1]:7880",
        ] {
            assert_eq!(validate_server_url(allowed.to_owned()).unwrap(), allowed);
        }
        for rejected in [
            "ws://livekit.example.test",
            "https://livekit.example.test",
            "wss://user:password@livekit.example.test",
            "wss://livekit.example.test?token=secret",
            "wss://livekit.example.test/#fragment",
            " wss://livekit.example.test",
        ] {
            assert_eq!(
                validate_server_url(rejected.to_owned()).unwrap_err(),
                MediaError::InvalidServerUrl
            );
        }
        assert_eq!(
            validate_server_url("wss://".to_owned() + &"x".repeat(MAX_LIVEKIT_MEDIA_URL_BYTES))
                .unwrap_err(),
            MediaError::InvalidServerUrl
        );
    }

    #[test]
    fn livekit_jwt_shape_is_strict_and_capped() {
        assert_eq!(validate_access_token(&access_token()), Ok(()));
        for rejected in [
            "",
            "one.two",
            "one.two.three.four",
            "one.two.bad=",
            "one. two.three",
        ] {
            assert_eq!(
                validate_access_token(rejected).unwrap_err(),
                MediaError::InvalidAccessToken
            );
        }
        assert_eq!(
            validate_access_token(&"a".repeat(MAX_LIVEKIT_MEDIA_ACCESS_TOKEN_BYTES + 1))
                .unwrap_err(),
            MediaError::InvalidAccessToken
        );
    }

    #[test]
    fn room_options_require_gcm_and_disable_automatic_subscription() {
        let group_id = GroupId::new();
        let keyring = LiveKitMediaKeyring::new(secret(group_id, 37, 0x53)).unwrap();
        let options = keyring.encrypted_room_options();

        assert!(!options.auto_subscribe);
        let encryption = options
            .encryption
            .expect("room encryption must be mandatory");
        assert_eq!(encryption.encryption_type, EncryptionType::Gcm);
        assert_eq!(encryption.key_provider.get_latest_key_index(), 37);
        let installed = Zeroizing::new(
            encryption
                .key_provider
                .get_shared_key(37)
                .expect("MLS exporter must be installed natively"),
        );
        assert_eq!(installed.as_slice(), &[0x53; 32]);
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
