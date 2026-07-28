//! Concrete Tauri runtime wiring for the packaged Filament clients.
//!
//! The runtime persists the bounded session in the platform credential store,
//! discovers the authenticated user through a pinned native HTTPS origin, and
//! enrolls only a fresh account as its first certified MLS device. Existing
//! accounts without a native device binding remain pairing-gated.

use std::{
    fs,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Condvar, Mutex, TryLockError, Weak,
    },
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use filament_core::{DeviceId, UserId};
use filament_e2ee::{
    clear_pending_keypackage_upload, confirm_attachment_acknowledgment, confirm_attachment_upload,
    confirm_commit_acknowledgment, confirm_message_acknowledgment, confirm_proposal_acknowledgment,
    confirmed_attachment_upload, finalize_confirmed_attachment_upload, generate_key_package_batch,
    generate_last_resort_key_package, load_mls_client_state, load_pending_keypackage_upload,
    load_pending_root_identity_rotation, load_root_identity, load_root_identity_rotation_sequence,
    pending_attachment_acknowledgment, pending_attachment_downloads, pending_attachment_upload,
    pending_commit_acknowledgment, pending_message_acknowledgment, pending_proposal_acknowledgment,
    persist_downloaded_attachment, persist_initial_device_bootstrap,
    persist_root_identity_rotation_sequence, prepare_pending_root_identity_rotation,
    purge_expired_attachment_upload, purge_expired_attachments, purge_expired_messages,
    ConversationAudience, DurableAttachmentError, DurableMailboxError, DurableMlsClient,
    KeyStoreError, LocalKeyStore, LocalStoreId, MailboxConversationRoute, MlsDevice,
    RootIdentityKey, StoreKey, DEFAULT_BATCH_SIZE,
};
use filament_protocol::{
    RotateRootIdentityResponse, MAX_E2EE_COMMIT_MAILBOX_PAGE_SIZE, MAX_E2EE_MAILBOX_PAGE_SIZE,
    MAX_E2EE_PROPOSAL_MAILBOX_PAGE_SIZE,
};
use tauri::ipc::InvokeBody;
use tauri::Manager as _;

use crate::{
    device_registry::{DeviceRegistry, DeviceRegistryError, OsDeviceRegistry},
    native_api::{
        verify_directory_device, verify_directory_root, verify_root_identity_directory,
        NativeApiError, NativeEnrollmentApi, ReqwestNativeEnrollmentApi,
    },
    native_gateway::{
        decode_gateway_wake, GatewayWake, GatewayWakeQueue, GatewayWork, KeyPackageLowWake,
        NativeGatewayConnector, NativeGatewayFrame, TungsteniteNativeGatewayConnector,
    },
    session_store::{
        OsSessionCredentialStore, SessionCredentialError, SessionCredentialStore, StoredSession,
        StoredSessionMetadata,
    },
    validate_runtime_navigation, DesktopCommandBackend, DesktopCommandBackendError,
    DesktopCommandError, DesktopCommandHost, DesktopE2eeStore, E2eeStoreStatus,
    EncryptionDeviceVerification, EncryptionSettingsDevice, EncryptionSettingsSnapshot,
    RotateIdentityCommandRequest, SessionMetadata, StoreSessionRequest, UnixExpiry,
    ValidatedStoreSessionRequest,
};

/// Maximum serialized request body accepted at the native IPC boundary.
pub const MAX_TAURI_IPC_REQUEST_BYTES: usize = 16 * 1024;
const MAX_MAILBOX_GROUPS_PER_SYNC: usize = 8;
const MAX_MAILBOX_PAGES_PER_GROUP: usize = 4;
const MAX_OUTBOUND_COMMIT_ATTEMPTS: usize = 4;
const MAX_ATTACHMENT_DOWNLOADS_PER_GROUP: usize = 4;
const BACKGROUND_SYNC_INTERVAL: Duration = Duration::from_secs(15);
const BACKGROUND_SYNC_MAX_BACKOFF: Duration = Duration::from_secs(5 * 60);
const GATEWAY_RECONNECT_INITIAL: Duration = Duration::from_secs(1);
const GATEWAY_RECONNECT_MAX: Duration = Duration::from_secs(30);
const GATEWAY_READY_TIMEOUT: Duration = Duration::from_secs(7);
const GATEWAY_IDLE_TIMEOUT: Duration = Duration::from_secs(45);

#[derive(Clone, Copy)]
struct GatewayConnectionPolicy {
    ready_timeout: Duration,
    idle_timeout: Duration,
}

const PRODUCTION_GATEWAY_CONNECTION_POLICY: GatewayConnectionPolicy = GatewayConnectionPolicy {
    ready_timeout: GATEWAY_READY_TIMEOUT,
    idle_timeout: GATEWAY_IDLE_TIMEOUT,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BackgroundSyncOutcome {
    Idle,
    Busy,
    Synchronized,
}

struct ProductionDesktopBackend {
    session_store: Arc<dyn SessionCredentialStore>,
    device_registry: Arc<dyn DeviceRegistry>,
    api: Arc<dyn NativeEnrollmentApi>,
    store_factory: Arc<dyn NativeStoreFactory>,
    gateway: Arc<dyn NativeGatewayConnector>,
    gateway_wakes: Arc<GatewayWakeQueue>,
    gateway_lifecycle: Arc<GatewayLifecycle>,
    clock: Arc<Clock>,
    active: Mutex<Option<ActiveE2eeState>>,
}

impl core::fmt::Debug for ProductionDesktopBackend {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("ProductionDesktopBackend(<native state redacted>)")
    }
}

impl ProductionDesktopBackend {
    fn new(app_data_root: PathBuf) -> Result<Self, DesktopCommandBackendError> {
        Ok(Self {
            session_store: Arc::new(OsSessionCredentialStore),
            device_registry: Arc::new(OsDeviceRegistry),
            api: Arc::new(
                ReqwestNativeEnrollmentApi::from_build_config().map_err(map_native_api_error)?,
            ),
            store_factory: Arc::new(DesktopStoreFactory { app_data_root }),
            gateway: Arc::new(
                TungsteniteNativeGatewayConnector::from_build_config()
                    .map_err(|_| DesktopCommandBackendError::Unavailable)?,
            ),
            gateway_wakes: Arc::new(GatewayWakeQueue::new()),
            gateway_lifecycle: Arc::new(GatewayLifecycle::new()),
            clock: Arc::new(system_time_unix),
            active: Mutex::new(None),
        })
    }

    #[cfg(test)]
    fn with_dependencies(
        session_store: Arc<dyn SessionCredentialStore>,
        device_registry: Arc<dyn DeviceRegistry>,
        api: Arc<dyn NativeEnrollmentApi>,
        store_factory: Arc<dyn NativeStoreFactory>,
        gateway: Arc<dyn NativeGatewayConnector>,
        clock: impl Fn() -> Result<i64, DesktopCommandBackendError> + Send + Sync + 'static,
    ) -> Self {
        Self {
            session_store,
            device_registry,
            api,
            store_factory,
            gateway,
            gateway_wakes: Arc::new(GatewayWakeQueue::new()),
            gateway_lifecycle: Arc::new(GatewayLifecycle::new()),
            clock: Arc::new(clock),
            active: Mutex::new(None),
        }
    }

    fn load_valid_session(&self) -> Result<StoredSession, DesktopCommandBackendError> {
        let session = self
            .session_store
            .load()
            .map_err(map_session_error)?
            .ok_or(DesktopCommandBackendError::Rejected)?;
        UnixExpiry::new(session.expires_at_unix, (self.clock)()?)
            .map_err(|_| DesktopCommandBackendError::Rejected)?;
        Ok(session)
    }

    fn open_store(
        &self,
        user_id: UserId,
        device_id: DeviceId,
    ) -> Result<Arc<dyn LocalKeyStore>, DesktopCommandBackendError> {
        self.store_factory
            .open(LocalStoreId::new(user_id, device_id))
    }

    fn initialize_existing_or_pending(
        &self,
        session: &StoredSession,
        user_id: UserId,
        device_id: DeviceId,
        store: &Arc<dyn LocalKeyStore>,
    ) -> Result<(), DesktopCommandBackendError> {
        let root_exists = store
            .exists(&StoreKey::root_identity())
            .map_err(|error| map_keystore_error(&error))?;
        let state_exists = store
            .exists(&StoreKey::mls_client_state())
            .map_err(|error| map_keystore_error(&error))?;
        if root_exists != state_exists {
            return Err(DesktopCommandBackendError::Rejected);
        }
        if !root_exists {
            let directory = self
                .api
                .list_devices(&session.access_token, user_id)
                .map_err(map_native_api_error)?;
            if !directory.devices.is_empty() {
                return Err(DesktopCommandBackendError::Rejected);
            }
            return self.bootstrap_first_device(session, user_id, device_id, store);
        }

        self.reconcile_pending_rotation(session, store.as_ref())?;
        let root = load_root_identity(store.as_ref(), &StoreKey::root_identity())
            .map_err(|error| map_keystore_error(&error))?;
        let state =
            load_mls_client_state(store.as_ref()).map_err(|error| map_keystore_error(&error))?;
        if state.device.user_id() != user_id
            || state.device.device_id() != device_id
            || state.device.root_key_public() != &root.public_key_bytes()
        {
            return Err(DesktopCommandBackendError::Rejected);
        }
        let mut directory = self
            .api
            .list_devices(&session.access_token, user_id)
            .map_err(map_native_api_error)?;
        if directory.devices.is_empty() {
            self.api
                .publish_device(&session.access_token, &state.device)
                .map_err(map_native_api_error)?;
            directory = self
                .api
                .list_devices(&session.access_token, user_id)
                .map_err(map_native_api_error)?;
        }
        verify_directory_device(
            user_id,
            device_id,
            state.device.certificate(),
            state.device.root_key_public(),
            &directory,
        )
        .map_err(map_native_api_error)?;
        self.ensure_authenticated_rotation_sequence(session, user_id, &root, store.as_ref())?;
        self.flush_pending_keypackages(session, device_id, store.as_ref())
    }

    fn bootstrap_first_device(
        &self,
        session: &StoredSession,
        user_id: UserId,
        device_id: DeviceId,
        store: &Arc<dyn LocalKeyStore>,
    ) -> Result<(), DesktopCommandBackendError> {
        let root = RootIdentityKey::generate();
        let device = MlsDevice::generate(user_id, device_id, &root)
            .map_err(|_| DesktopCommandBackendError::Unavailable)?;
        let mut packages = generate_key_package_batch(&device, DEFAULT_BATCH_SIZE)
            .map_err(|_| DesktopCommandBackendError::Unavailable)?;
        packages.push(
            generate_last_resort_key_package(&device)
                .map_err(|_| DesktopCommandBackendError::Unavailable)?,
        );
        persist_initial_device_bootstrap(store.as_ref(), &root, &device, &packages)
            .map_err(|error| map_keystore_error(&error))?;
        self.api
            .publish_device(&session.access_token, &device)
            .map_err(map_native_api_error)?;
        self.flush_pending_keypackages(session, device_id, store.as_ref())?;
        let directory = self
            .api
            .list_devices(&session.access_token, user_id)
            .map_err(map_native_api_error)?;
        verify_directory_device(
            user_id,
            device_id,
            device.certificate(),
            device.root_key_public(),
            &directory,
        )
        .map_err(map_native_api_error)?;
        self.ensure_authenticated_rotation_sequence(session, user_id, &root, store.as_ref())?;
        Ok(())
    }

    fn flush_pending_keypackages(
        &self,
        session: &StoredSession,
        device_id: DeviceId,
        store: &dyn LocalKeyStore,
    ) -> Result<(), DesktopCommandBackendError> {
        let pending = match load_pending_keypackage_upload(store) {
            Ok(pending) => pending,
            Err(KeyStoreError::NotFound) => return Ok(()),
            Err(error) => return Err(map_keystore_error(&error)),
        };
        self.api
            .upload_keypackages(&session.access_token, device_id, &pending)
            .map_err(map_native_api_error)?;
        clear_pending_keypackage_upload(store).map_err(|error| map_keystore_error(&error))
    }

    fn reconcile_pending_rotation(
        &self,
        session: &StoredSession,
        store: &dyn LocalKeyStore,
    ) -> Result<Option<RotateRootIdentityResponse>, DesktopCommandBackendError> {
        let pending = match load_pending_root_identity_rotation(store) {
            Ok(pending) => pending,
            Err(KeyStoreError::NotFound) => return Ok(None),
            Err(error) => return Err(map_keystore_error(&error)),
        };
        let response = self
            .api
            .rotate_root_identity(&session.access_token, pending.wire_request())
            .map_err(map_native_api_error)?;
        pending
            .finish(&response, store)
            .map_err(|error| map_keystore_error(&error))?;
        Ok(Some(response))
    }

    fn ensure_authenticated_rotation_sequence(
        &self,
        session: &StoredSession,
        user_id: UserId,
        root: &RootIdentityKey,
        store: &dyn LocalKeyStore,
    ) -> Result<u64, DesktopCommandBackendError> {
        let stored_sequence = match load_root_identity_rotation_sequence(store) {
            Ok(sequence) => Some(sequence),
            Err(KeyStoreError::NotFound) => None,
            Err(error) => return Err(map_keystore_error(&error)),
        };
        let directory = self
            .api
            .root_identity(&session.access_token, user_id)
            .map_err(map_native_api_error)?;
        let sequence = verify_root_identity_directory(
            user_id,
            &root.public_key_bytes(),
            stored_sequence,
            &directory,
        )
        .map_err(map_native_api_error)?;
        if stored_sequence.is_none() {
            persist_root_identity_rotation_sequence(store, sequence)
                .map_err(|error| map_keystore_error(&error))?;
        }
        Ok(sequence)
    }

    fn settings_snapshot(
        &self,
        session: &StoredSession,
        active: &ActiveE2eeState,
    ) -> Result<EncryptionSettingsSnapshot, DesktopCommandBackendError> {
        let root = load_root_identity(active.store.as_ref(), &StoreKey::root_identity())
            .map_err(|error| map_keystore_error(&error))?;
        let root_public = root.public_key_bytes();
        let rotation_sequence = self.ensure_authenticated_rotation_sequence(
            session,
            active.user_id,
            &root,
            active.store.as_ref(),
        )?;
        let directory = self
            .api
            .list_devices(&session.access_token, active.user_id)
            .map_err(map_native_api_error)?;
        verify_directory_root(active.user_id, &root_public, &directory)
            .map_err(map_native_api_error)?;
        let mut devices = Vec::with_capacity(directory.devices.len());
        for entry in directory.devices {
            let device_id = DeviceId::try_from(entry.device_id)
                .map_err(|_| DesktopCommandBackendError::Rejected)?;
            let signature_key: &[u8; 32] = entry
                .device_signature_pubkey
                .as_slice()
                .try_into()
                .map_err(|_| DesktopCommandBackendError::Rejected)?;
            let signature: &[u8; 64] = entry
                .root_key_signature
                .as_slice()
                .try_into()
                .map_err(|_| DesktopCommandBackendError::Rejected)?;
            let entry_root: &[u8; 32] = entry
                .root_key_pub
                .as_slice()
                .try_into()
                .map_err(|_| DesktopCommandBackendError::Rejected)?;
            if entry_root != &root_public {
                return Err(DesktopCommandBackendError::Rejected);
            }
            filament_e2ee::verify_device_certificate(
                entry_root,
                active.user_id,
                device_id,
                signature_key,
                signature,
            )
            .map_err(|_| DesktopCommandBackendError::Rejected)?;
            devices.push(
                EncryptionSettingsDevice::new(
                    device_id,
                    entry.created_at_unix,
                    device_id == active.device_id,
                    EncryptionDeviceVerification::Verified,
                )
                .map_err(|_| DesktopCommandBackendError::Rejected)?,
            );
        }
        if !devices
            .iter()
            .any(EncryptionSettingsDevice::is_current_device)
        {
            return Err(DesktopCommandBackendError::Rejected);
        }
        EncryptionSettingsSnapshot::new(&root_public, rotation_sequence, devices, false)
            .map_err(|_| DesktopCommandBackendError::Rejected)
    }

    fn synchronize_active_once(&self) -> Result<BackgroundSyncOutcome, DesktopCommandBackendError> {
        let mut active = match self.active.try_lock() {
            Ok(active) => active,
            Err(TryLockError::WouldBlock) => return Ok(BackgroundSyncOutcome::Busy),
            Err(TryLockError::Poisoned(_)) => return Err(DesktopCommandBackendError::Unavailable),
        };
        let Some(active) = active.as_mut() else {
            return Ok(BackgroundSyncOutcome::Idle);
        };
        let session = self.load_valid_session()?;
        let user_id = self
            .api
            .current_user(&session.access_token)
            .map_err(map_native_api_error)?;
        if user_id != active.user_id {
            return Err(DesktopCommandBackendError::Rejected);
        }
        self.synchronize_mailboxes(&session, active)?;
        self.flush_pending_keypackages(&session, active.device_id, active.store.as_ref())?;
        Ok(BackgroundSyncOutcome::Synchronized)
    }

    fn synchronize_active_group_once(
        &self,
        group_id: filament_core::GroupId,
    ) -> Result<BackgroundSyncOutcome, DesktopCommandBackendError> {
        let mut active = self
            .active
            .lock()
            .map_err(|_| DesktopCommandBackendError::Unavailable)?;
        let Some(active) = active.as_mut() else {
            return Ok(BackgroundSyncOutcome::Idle);
        };
        let session = self.load_valid_session()?;
        let user_id = self
            .api
            .current_user(&session.access_token)
            .map_err(map_native_api_error)?;
        if user_id != active.user_id {
            return Err(DesktopCommandBackendError::Rejected);
        }
        let route = active
            .mailbox
            .mailbox_routes()
            .map_err(map_durable_mailbox_error)?
            .into_iter()
            .find(|route| route.group_id == group_id);
        let Some(route) = route else {
            return Ok(BackgroundSyncOutcome::Idle);
        };
        self.synchronize_group_mailboxes(&session, active, &route)?;
        Ok(BackgroundSyncOutcome::Synchronized)
    }

    fn replenish_active_keypackages_once(
        &self,
        wake: KeyPackageLowWake,
    ) -> Result<BackgroundSyncOutcome, DesktopCommandBackendError> {
        let mut active = self
            .active
            .lock()
            .map_err(|_| DesktopCommandBackendError::Unavailable)?;
        let Some(active) = active.as_mut() else {
            return Ok(BackgroundSyncOutcome::Idle);
        };
        if wake.device_id != active.device_id {
            return Ok(BackgroundSyncOutcome::Idle);
        }
        let session = self.load_valid_session()?;
        let user_id = self
            .api
            .current_user(&session.access_token)
            .map_err(map_native_api_error)?;
        if user_id != active.user_id {
            return Err(DesktopCommandBackendError::Rejected);
        }

        match load_pending_keypackage_upload(active.store.as_ref()) {
            Ok(_) => {
                self.flush_pending_keypackages(&session, active.device_id, active.store.as_ref())?;
                return Ok(BackgroundSyncOutcome::Synchronized);
            }
            Err(KeyStoreError::NotFound) => {}
            Err(error) => return Err(map_keystore_error(&error)),
        }

        let missing = wake
            .water_mark
            .checked_sub(wake.remaining_count)
            .and_then(|count| usize::try_from(count).ok())
            .map(|count| count.min(DEFAULT_BATCH_SIZE))
            .filter(|count| *count > 0)
            .ok_or(DesktopCommandBackendError::Rejected)?;
        active
            .mailbox
            .prepare_keypackage_replenishment(active.store.as_ref(), missing)
            .map_err(map_durable_mailbox_error)?;
        self.flush_pending_keypackages(&session, active.device_id, active.store.as_ref())?;
        Ok(BackgroundSyncOutcome::Synchronized)
    }

    fn gateway_session_snapshot(
        &self,
    ) -> Result<Option<(StoredSession, UserId)>, DesktopCommandBackendError> {
        let user_id = {
            let active = self
                .active
                .lock()
                .map_err(|_| DesktopCommandBackendError::Unavailable)?;
            let Some(active) = active.as_ref() else {
                return Ok(None);
            };
            active.user_id
        };
        self.load_valid_session()
            .map(|session| Some((session, user_id)))
    }

    fn synchronize_mailboxes(
        &self,
        session: &StoredSession,
        active: &mut ActiveE2eeState,
    ) -> Result<(), DesktopCommandBackendError> {
        self.flush_pending_conversation_provision(session, active)?;
        let now_unix = (self.clock)()?;
        purge_expired_messages(active.store.as_ref(), now_unix)
            .map_err(|error| map_keystore_error(&error))?;
        purge_expired_attachments(active.store.as_ref(), now_unix)
            .map_err(|error| map_durable_attachment_error(&error))?;
        let routes = active
            .mailbox
            .mailbox_routes()
            .map_err(map_durable_mailbox_error)?;
        if routes.is_empty() {
            return Ok(());
        }
        let route_count = routes.len();
        let selected_count = route_count.min(MAX_MAILBOX_GROUPS_PER_SYNC);
        let start = active.mailbox_route_offset % route_count;
        for offset in 0..selected_count {
            let route = &routes[(start + offset) % route_count];
            self.synchronize_group_mailboxes(session, active, route)?;
        }
        active.mailbox_route_offset = (start + selected_count) % route_count;
        Ok(())
    }

    fn flush_pending_conversation_provision(
        &self,
        session: &StoredSession,
        active: &mut ActiveE2eeState,
    ) -> Result<(), DesktopCommandBackendError> {
        let Some(request) = active
            .mailbox
            .pending_conversation_provision(active.store.as_ref())
            .map_err(map_durable_mailbox_error)?
        else {
            return Ok(());
        };
        let response = self
            .api
            .provision_conversation(&session.access_token, &request)
            .map_err(map_native_api_error)?;
        active
            .mailbox
            .confirm_conversation_provision(active.store.as_ref(), &request, &response)
            .map_err(map_durable_mailbox_error)
    }

    fn synchronize_group_mailboxes(
        &self,
        session: &StoredSession,
        active: &mut ActiveE2eeState,
        route: &MailboxConversationRoute,
    ) -> Result<(), DesktopCommandBackendError> {
        self.coordinate_attachment_upload(session, active, route.group_id)?;
        self.flush_outbound_message(session, active, route.group_id)?;
        self.coordinate_attachment_upload(session, active, route.group_id)?;
        self.synchronize_proposals(session, active, route.group_id)?;
        self.synchronize_attachments(session, active, route.group_id)?;

        self.flush_commit_acknowledgment(session, active, route.group_id)?;
        for _ in 0..MAX_MAILBOX_PAGES_PER_GROUP {
            let page = self
                .api
                .commit_mailbox(&session.access_token, route.group_id, active.device_id)
                .map_err(map_native_api_error)?;
            let page_len = page.commits.len();
            let batch = match route.audience {
                ConversationAudience::DirectMessage => {
                    let peer = route
                        .participants
                        .first()
                        .copied()
                        .ok_or(DesktopCommandBackendError::Rejected)?;
                    active.mailbox.process_commit_mailbox(
                        active.store.as_ref(),
                        route.group_id,
                        peer,
                        page,
                    )
                }
                ConversationAudience::GroupDm => active.mailbox.process_group_commit_mailbox(
                    active.store.as_ref(),
                    route.group_id,
                    &route.participants,
                    page,
                ),
            }
            .map_err(map_durable_mailbox_error)?;
            if let Some(acknowledgment) = batch.acknowledgment.as_ref() {
                self.api
                    .acknowledge_commits(&session.access_token, route.group_id, acknowledgment)
                    .map_err(map_native_api_error)?;
                confirm_commit_acknowledgment(
                    active.store.as_ref(),
                    route.group_id,
                    acknowledgment,
                )
                .map_err(|error| map_keystore_error(&error))?;
            }
            if batch.rejected_commit.is_some() {
                return Err(DesktopCommandBackendError::Rejected);
            }
            if page_len < MAX_E2EE_COMMIT_MAILBOX_PAGE_SIZE {
                break;
            }
        }

        self.flush_message_acknowledgment(session, active, route.group_id)?;
        for _ in 0..MAX_MAILBOX_PAGES_PER_GROUP {
            let page = self
                .api
                .message_mailbox(&session.access_token, route.group_id, active.device_id)
                .map_err(map_native_api_error)?;
            let page_len = page.messages.len();
            let batch = active
                .mailbox
                .process_message_mailbox_at(
                    active.store.as_ref(),
                    route.group_id,
                    page,
                    (self.clock)()?,
                )
                .map_err(map_durable_mailbox_error)?;
            if let Some(acknowledgment) = batch.acknowledgment.as_ref() {
                self.api
                    .acknowledge_messages(&session.access_token, route.group_id, acknowledgment)
                    .map_err(map_native_api_error)?;
                confirm_message_acknowledgment(
                    active.store.as_ref(),
                    route.group_id,
                    acknowledgment,
                )
                .map_err(|error| map_keystore_error(&error))?;
            }
            self.synchronize_attachments(session, active, route.group_id)?;
            if !batch.rejected_messages.is_empty() {
                return Err(DesktopCommandBackendError::Rejected);
            }
            if page_len < MAX_E2EE_MAILBOX_PAGE_SIZE {
                break;
            }
        }
        Ok(())
    }

    fn synchronize_attachments(
        &self,
        session: &StoredSession,
        active: &ActiveE2eeState,
        group_id: filament_core::GroupId,
    ) -> Result<(), DesktopCommandBackendError> {
        self.flush_attachment_acknowledgment(session, active, group_id)?;
        let now_unix = (self.clock)()?;
        for _ in 0..MAX_ATTACHMENT_DOWNLOADS_PER_GROUP {
            let mut pending =
                pending_attachment_downloads(active.store.as_ref(), group_id, now_unix, 1)
                    .map_err(|error| map_durable_attachment_error(&error))?;
            let Some(pending) = pending.pop() else {
                return Ok(());
            };
            let encrypted = self
                .api
                .get_attachment(
                    &session.access_token,
                    group_id,
                    pending.descriptor.attachment_id,
                    active.device_id,
                )
                .map_err(map_native_api_error)?;
            let acknowledgment = persist_downloaded_attachment(
                active.store.as_ref(),
                active.device_id,
                &pending,
                &encrypted,
            )
            .map_err(|error| map_durable_attachment_error(&error))?;
            self.api
                .acknowledge_attachments(&session.access_token, group_id, &acknowledgment)
                .map_err(map_native_api_error)?;
            confirm_attachment_acknowledgment(active.store.as_ref(), group_id, &acknowledgment)
                .map_err(|error| map_durable_attachment_error(&error))?;
        }
        Ok(())
    }

    fn flush_outbound_message(
        &self,
        session: &StoredSession,
        active: &ActiveE2eeState,
        group_id: filament_core::GroupId,
    ) -> Result<(), DesktopCommandBackendError> {
        let Some(request) = active
            .mailbox
            .pending_outbound_message(active.store.as_ref(), group_id)
            .map_err(map_durable_mailbox_error)?
        else {
            return Ok(());
        };
        let response = self
            .api
            .post_message(&session.access_token, group_id, &request)
            .map_err(map_native_api_error)?;
        active
            .mailbox
            .confirm_outbound_message(active.store.as_ref(), group_id, &request, &response)
            .map_err(map_durable_mailbox_error)
    }

    fn coordinate_attachment_upload(
        &self,
        session: &StoredSession,
        active: &mut ActiveE2eeState,
        group_id: filament_core::GroupId,
    ) -> Result<(), DesktopCommandBackendError> {
        let now_unix = (self.clock)()?;
        purge_expired_attachment_upload(
            active.store.as_ref(),
            group_id,
            active.device_id,
            now_unix,
        )
        .map_err(|error| map_durable_attachment_error(&error))?;
        if let Some(upload) =
            pending_attachment_upload(active.store.as_ref(), group_id, active.device_id)
                .map_err(|error| map_durable_attachment_error(&error))?
        {
            let response = self
                .api
                .put_attachment(&session.access_token, group_id, active.device_id, upload)
                .map_err(map_native_api_error)?;
            let durable =
                pending_attachment_upload(active.store.as_ref(), group_id, active.device_id)
                    .map_err(|error| map_durable_attachment_error(&error))?
                    .ok_or(DesktopCommandBackendError::Rejected)?;
            confirm_attachment_upload(
                active.store.as_ref(),
                group_id,
                active.device_id,
                &durable,
                &response,
                now_unix,
            )
            .map_err(|error| map_durable_attachment_error(&error))?;
        }

        let Some(confirmed) = confirmed_attachment_upload(
            active.store.as_ref(),
            group_id,
            active.device_id,
            now_unix,
        )
        .map_err(|error| map_durable_attachment_error(&error))?
        else {
            return Ok(());
        };
        if finalize_confirmed_attachment_upload(active.store.as_ref(), &confirmed)
            .map_err(|error| map_durable_attachment_error(&error))?
        {
            return Ok(());
        }
        if active
            .mailbox
            .pending_outbound_message(active.store.as_ref(), group_id)
            .map_err(map_durable_mailbox_error)?
            .is_none()
        {
            active
                .mailbox
                .prepare_confirmed_attachment_message(active.store.as_ref(), &confirmed, now_unix)
                .map_err(map_durable_mailbox_error)?;
        }
        Ok(())
    }

    fn synchronize_proposals(
        &self,
        session: &StoredSession,
        active: &mut ActiveE2eeState,
        group_id: filament_core::GroupId,
    ) -> Result<(), DesktopCommandBackendError> {
        self.flush_commit_acknowledgment(session, active, group_id)?;
        self.flush_outbound_commit(session, active, group_id)?;
        self.flush_proposal_acknowledgment(session, active, group_id)?;
        for _ in 0..MAX_MAILBOX_PAGES_PER_GROUP {
            let page = self
                .api
                .proposal_mailbox(&session.access_token, group_id, active.device_id)
                .map_err(map_native_api_error)?;
            let page_len = page.proposals.len();
            let batch = active
                .mailbox
                .process_proposal_mailbox(active.store.as_ref(), group_id, page)
                .map_err(map_durable_mailbox_error)?;
            if batch.outbound_commit.is_some() {
                self.flush_outbound_commit(session, active, group_id)?;
            }
            if let Some(acknowledgment) = batch.acknowledgment.as_ref() {
                self.api
                    .acknowledge_proposals(&session.access_token, group_id, acknowledgment)
                    .map_err(map_native_api_error)?;
                confirm_proposal_acknowledgment(active.store.as_ref(), group_id, acknowledgment)
                    .map_err(|error| map_keystore_error(&error))?;
            }
            if page_len == 0 || batch.awaiting_peer_commit {
                break;
            }
            if page_len > MAX_E2EE_PROPOSAL_MAILBOX_PAGE_SIZE {
                return Err(DesktopCommandBackendError::Rejected);
            }
        }
        Ok(())
    }

    fn flush_outbound_commit(
        &self,
        session: &StoredSession,
        active: &mut ActiveE2eeState,
        group_id: filament_core::GroupId,
    ) -> Result<(), DesktopCommandBackendError> {
        for _ in 0..MAX_OUTBOUND_COMMIT_ATTEMPTS {
            let Some(request) = active
                .mailbox
                .pending_outbound_commit(active.store.as_ref(), group_id)
                .map_err(map_durable_mailbox_error)?
            else {
                return Ok(());
            };
            match self
                .api
                .post_commit(&session.access_token, group_id, &request)
            {
                Ok(response) => {
                    active
                        .mailbox
                        .confirm_outbound_commit(
                            active.store.as_ref(),
                            group_id,
                            &request,
                            &response,
                        )
                        .map_err(map_durable_mailbox_error)?;
                    return Ok(());
                }
                Err(NativeApiError::EpochConflict) => {
                    self.rebase_outbound_commit(session, active, group_id)?;
                }
                Err(error) => return Err(map_native_api_error(error)),
            }
        }
        Err(DesktopCommandBackendError::Rejected)
    }

    fn rebase_outbound_commit(
        &self,
        session: &StoredSession,
        active: &mut ActiveE2eeState,
        group_id: filament_core::GroupId,
    ) -> Result<(), DesktopCommandBackendError> {
        let page = self
            .api
            .commit_mailbox(&session.access_token, group_id, active.device_id)
            .map_err(map_native_api_error)?;
        let rebased = active
            .mailbox
            .rebase_outbound_commit(active.store.as_ref(), group_id, page)
            .map_err(map_durable_mailbox_error)?;
        self.api
            .acknowledge_commits(&session.access_token, group_id, &rebased.acknowledgment)
            .map_err(map_native_api_error)?;
        confirm_commit_acknowledgment(active.store.as_ref(), group_id, &rebased.acknowledgment)
            .map_err(|error| map_keystore_error(&error))?;
        if rebased.invalidated {
            return Err(DesktopCommandBackendError::Rejected);
        }
        Ok(())
    }

    fn flush_proposal_acknowledgment(
        &self,
        session: &StoredSession,
        active: &ActiveE2eeState,
        group_id: filament_core::GroupId,
    ) -> Result<(), DesktopCommandBackendError> {
        if let Some(acknowledgment) =
            pending_proposal_acknowledgment(active.store.as_ref(), group_id, active.device_id)
                .map_err(|error| map_keystore_error(&error))?
        {
            self.api
                .acknowledge_proposals(&session.access_token, group_id, &acknowledgment)
                .map_err(map_native_api_error)?;
            confirm_proposal_acknowledgment(active.store.as_ref(), group_id, &acknowledgment)
                .map_err(|error| map_keystore_error(&error))?;
        }
        Ok(())
    }

    fn flush_message_acknowledgment(
        &self,
        session: &StoredSession,
        active: &ActiveE2eeState,
        group_id: filament_core::GroupId,
    ) -> Result<(), DesktopCommandBackendError> {
        if let Some(acknowledgment) =
            pending_message_acknowledgment(active.store.as_ref(), group_id, active.device_id)
                .map_err(|error| map_keystore_error(&error))?
        {
            self.api
                .acknowledge_messages(&session.access_token, group_id, &acknowledgment)
                .map_err(map_native_api_error)?;
            confirm_message_acknowledgment(active.store.as_ref(), group_id, &acknowledgment)
                .map_err(|error| map_keystore_error(&error))?;
        }
        Ok(())
    }

    fn flush_commit_acknowledgment(
        &self,
        session: &StoredSession,
        active: &ActiveE2eeState,
        group_id: filament_core::GroupId,
    ) -> Result<(), DesktopCommandBackendError> {
        if let Some(acknowledgment) =
            pending_commit_acknowledgment(active.store.as_ref(), group_id, active.device_id)
                .map_err(|error| map_keystore_error(&error))?
        {
            self.api
                .acknowledge_commits(&session.access_token, group_id, &acknowledgment)
                .map_err(map_native_api_error)?;
            confirm_commit_acknowledgment(active.store.as_ref(), group_id, &acknowledgment)
                .map_err(|error| map_keystore_error(&error))?;
        }
        Ok(())
    }

    fn flush_attachment_acknowledgment(
        &self,
        session: &StoredSession,
        active: &ActiveE2eeState,
        group_id: filament_core::GroupId,
    ) -> Result<(), DesktopCommandBackendError> {
        if let Some(acknowledgment) =
            pending_attachment_acknowledgment(active.store.as_ref(), group_id, active.device_id)
                .map_err(|error| map_durable_attachment_error(&error))?
        {
            self.api
                .acknowledge_attachments(&session.access_token, group_id, &acknowledgment)
                .map_err(map_native_api_error)?;
            confirm_attachment_acknowledgment(active.store.as_ref(), group_id, &acknowledgment)
                .map_err(|error| map_durable_attachment_error(&error))?;
        }
        Ok(())
    }
}

impl DesktopCommandBackend for ProductionDesktopBackend {
    fn store_session(
        &self,
        request: ValidatedStoreSessionRequest,
    ) -> Result<SessionMetadata, DesktopCommandBackendError> {
        *self
            .active
            .lock()
            .map_err(|_| DesktopCommandBackendError::Unavailable)? = None;
        self.gateway_lifecycle.advance();
        let metadata = self
            .session_store
            .store(&request)
            .map_err(map_session_error)?;
        Ok(session_metadata(Some(metadata)))
    }

    fn clear_session(&self) -> Result<(), DesktopCommandBackendError> {
        *self
            .active
            .lock()
            .map_err(|_| DesktopCommandBackendError::Unavailable)? = None;
        self.gateway_lifecycle.advance();
        self.session_store.clear().map_err(map_session_error)
    }

    fn read_session_metadata(&self) -> Result<SessionMetadata, DesktopCommandBackendError> {
        self.session_store
            .metadata()
            .map(session_metadata)
            .map_err(map_session_error)
    }

    fn initialize_e2ee_store(&self) -> Result<E2eeStoreStatus, DesktopCommandBackendError> {
        let mut active = self
            .active
            .lock()
            .map_err(|_| DesktopCommandBackendError::Unavailable)?;
        if active.is_some() {
            return Ok(store_status());
        }
        let session = self.load_valid_session()?;
        let user_id = self
            .api
            .current_user(&session.access_token)
            .map_err(map_native_api_error)?;
        let device_id = if let Some(device_id) = self
            .device_registry
            .device_for(user_id)
            .map_err(map_device_registry_error)?
        {
            device_id
        } else {
            let directory = self
                .api
                .list_devices(&session.access_token, user_id)
                .map_err(map_native_api_error)?;
            if !directory.devices.is_empty() {
                return Err(DesktopCommandBackendError::Rejected);
            }
            let device_id = DeviceId::new();
            self.device_registry
                .bind(user_id, device_id)
                .map_err(map_device_registry_error)?;
            device_id
        };
        let store = self.open_store(user_id, device_id)?;
        self.initialize_existing_or_pending(&session, user_id, device_id, &store)?;
        let mut initialized = ActiveE2eeState {
            user_id,
            device_id,
            mailbox: DurableMlsClient::load(store.as_ref())
                .map_err(|error| map_keystore_error(&error))?,
            store,
            mailbox_route_offset: 0,
        };
        self.synchronize_mailboxes(&session, &mut initialized)?;
        *active = Some(initialized);
        self.gateway_lifecycle.advance();
        Ok(store_status())
    }

    fn read_e2ee_store_status(&self) -> Result<E2eeStoreStatus, DesktopCommandBackendError> {
        if self
            .active
            .lock()
            .map_err(|_| DesktopCommandBackendError::Unavailable)?
            .is_some()
        {
            Ok(store_status())
        } else {
            Err(DesktopCommandBackendError::Unavailable)
        }
    }

    fn read_encryption_settings(
        &self,
    ) -> Result<EncryptionSettingsSnapshot, DesktopCommandBackendError> {
        let session = self.load_valid_session()?;
        let mut active = self
            .active
            .lock()
            .map_err(|_| DesktopCommandBackendError::Unavailable)?;
        let active = active
            .as_mut()
            .ok_or(DesktopCommandBackendError::Unavailable)?;
        let user_id = self
            .api
            .current_user(&session.access_token)
            .map_err(map_native_api_error)?;
        if user_id != active.user_id {
            return Err(DesktopCommandBackendError::Rejected);
        }
        self.synchronize_mailboxes(&session, active)?;
        self.flush_pending_keypackages(&session, active.device_id, active.store.as_ref())?;
        self.settings_snapshot(&session, active)
    }

    fn rotate_root_identity(
        &self,
        request: RotateIdentityCommandRequest,
    ) -> Result<RotateRootIdentityResponse, DesktopCommandBackendError> {
        request
            .validate()
            .map_err(|_| DesktopCommandBackendError::Rejected)?;
        let session = self.load_valid_session()?;
        let mut active = self
            .active
            .lock()
            .map_err(|_| DesktopCommandBackendError::Unavailable)?;
        let active = active
            .as_mut()
            .ok_or(DesktopCommandBackendError::Unavailable)?;
        let user_id = self
            .api
            .current_user(&session.access_token)
            .map_err(map_native_api_error)?;
        if user_id != active.user_id {
            return Err(DesktopCommandBackendError::Rejected);
        }
        if let Some(response) = self.reconcile_pending_rotation(&session, active.store.as_ref())? {
            active.mailbox = DurableMlsClient::load(active.store.as_ref())
                .map_err(|error| map_keystore_error(&error))?;
            active.mailbox_route_offset = 0;
            let _ =
                self.flush_pending_keypackages(&session, active.device_id, active.store.as_ref());
            return Ok(response);
        }
        let root = load_root_identity(active.store.as_ref(), &StoreKey::root_identity())
            .map_err(|error| map_keystore_error(&error))?;
        let state = load_mls_client_state(active.store.as_ref())
            .map_err(|error| map_keystore_error(&error))?;
        if state.device.user_id() != active.user_id
            || state.device.device_id() != active.device_id
            || state.device.root_key_public() != &root.public_key_bytes()
        {
            return Err(DesktopCommandBackendError::Rejected);
        }
        let sequence = self.ensure_authenticated_rotation_sequence(
            &session,
            active.user_id,
            &root,
            active.store.as_ref(),
        )?;
        let pending = prepare_pending_root_identity_rotation(
            active.store.as_ref(),
            active.user_id,
            active.device_id,
            sequence,
            &root,
        )
        .map_err(|error| map_keystore_error(&error))?;
        let response = self
            .api
            .rotate_root_identity(&session.access_token, pending.wire_request())
            .map_err(map_native_api_error)?;
        pending
            .finish(&response, active.store.as_ref())
            .map_err(|error| map_keystore_error(&error))?;
        active.mailbox = DurableMlsClient::load(active.store.as_ref())
            .map_err(|error| map_keystore_error(&error))?;
        active.mailbox_route_offset = 0;
        let _ = self.flush_pending_keypackages(&session, active.device_id, active.store.as_ref());
        Ok(response)
    }
}

type Clock = dyn Fn() -> Result<i64, DesktopCommandBackendError> + Send + Sync;

struct ActiveE2eeState {
    user_id: UserId,
    device_id: DeviceId,
    store: Arc<dyn LocalKeyStore>,
    mailbox: DurableMlsClient,
    mailbox_route_offset: usize,
}

trait NativeStoreFactory: Send + Sync + 'static {
    fn open(
        &self,
        store_id: LocalStoreId,
    ) -> Result<Arc<dyn LocalKeyStore>, DesktopCommandBackendError>;
}

struct DesktopStoreFactory {
    app_data_root: PathBuf,
}

impl NativeStoreFactory for DesktopStoreFactory {
    fn open(
        &self,
        store_id: LocalStoreId,
    ) -> Result<Arc<dyn LocalKeyStore>, DesktopCommandBackendError> {
        DesktopE2eeStore::open(&self.app_data_root, store_id)
            .map(DesktopE2eeStore::into_shared_native_store)
            .map_err(|_| DesktopCommandBackendError::Unavailable)
    }
}

const fn store_status() -> E2eeStoreStatus {
    E2eeStoreStatus {
        ready: true,
        backend: "sqlcipher",
        key_custody: "platform_keystore",
    }
}

const fn session_metadata(metadata: Option<StoredSessionMetadata>) -> SessionMetadata {
    match metadata {
        Some(metadata) => SessionMetadata {
            stored: true,
            expires_at_unix: Some(metadata.expires_at_unix),
        },
        None => SessionMetadata {
            stored: false,
            expires_at_unix: None,
        },
    }
}

const fn map_session_error(error: SessionCredentialError) -> DesktopCommandBackendError {
    match error {
        SessionCredentialError::Unavailable => DesktopCommandBackendError::Unavailable,
        SessionCredentialError::Rejected => DesktopCommandBackendError::Rejected,
    }
}

const fn map_device_registry_error(error: DeviceRegistryError) -> DesktopCommandBackendError {
    match error {
        DeviceRegistryError::Unavailable => DesktopCommandBackendError::Unavailable,
        DeviceRegistryError::Rejected => DesktopCommandBackendError::Rejected,
    }
}

const fn map_native_api_error(error: NativeApiError) -> DesktopCommandBackendError {
    match error {
        NativeApiError::Unavailable => DesktopCommandBackendError::Unavailable,
        NativeApiError::Rejected | NativeApiError::EpochConflict => {
            DesktopCommandBackendError::Rejected
        }
    }
}

const fn map_keystore_error(error: &KeyStoreError) -> DesktopCommandBackendError {
    match error {
        KeyStoreError::BackendError | KeyStoreError::KeyUnavailable => {
            DesktopCommandBackendError::Unavailable
        }
        KeyStoreError::NotFound
        | KeyStoreError::InvalidIdentifier
        | KeyStoreError::InvalidPath
        | KeyStoreError::InvalidValue
        | KeyStoreError::LimitExceeded => DesktopCommandBackendError::Rejected,
    }
}

fn map_durable_mailbox_error(error: DurableMailboxError) -> DesktopCommandBackendError {
    match error {
        DurableMailboxError::Unavailable => DesktopCommandBackendError::Unavailable,
        DurableMailboxError::KeyStore(error) => map_keystore_error(&error),
        DurableMailboxError::PendingAcknowledgment
        | DurableMailboxError::PendingOutboundCommit
        | DurableMailboxError::PendingOutboundMessage
        | DurableMailboxError::PendingConversationProvision
        | DurableMailboxError::ConversationAlreadyExists
        | DurableMailboxError::InvalidatedOutboundCommit
        | DurableMailboxError::ConversationNotFound
        | DurableMailboxError::Conversation(_) => DesktopCommandBackendError::Rejected,
    }
}

const fn map_durable_attachment_error(
    error: &DurableAttachmentError,
) -> DesktopCommandBackendError {
    match error {
        DurableAttachmentError::KeyStore(
            KeyStoreError::BackendError | KeyStoreError::KeyUnavailable,
        ) => DesktopCommandBackendError::Unavailable,
        DurableAttachmentError::PendingAcknowledgment
        | DurableAttachmentError::PendingUpload
        | DurableAttachmentError::InvalidMetadata
        | DurableAttachmentError::Attachment(_)
        | DurableAttachmentError::KeyStore(_) => DesktopCommandBackendError::Rejected,
    }
}

fn system_time_unix() -> Result<i64, DesktopCommandBackendError> {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| DesktopCommandBackendError::Unavailable)?
        .as_secs();
    i64::try_from(seconds).map_err(|_| DesktopCommandBackendError::Unavailable)
}

fn background_sync_delay(previous: Duration, succeeded: bool) -> Duration {
    if succeeded {
        BACKGROUND_SYNC_INTERVAL
    } else {
        previous
            .checked_mul(2)
            .unwrap_or(BACKGROUND_SYNC_MAX_BACKOFF)
            .min(BACKGROUND_SYNC_MAX_BACKOFF)
    }
}

fn gateway_reconnect_schedule(previous: Duration, succeeded: bool) -> (Duration, Duration) {
    let next = if succeeded {
        GATEWAY_RECONNECT_INITIAL
    } else {
        previous
            .checked_mul(2)
            .unwrap_or(GATEWAY_RECONNECT_MAX)
            .min(GATEWAY_RECONNECT_MAX)
    };
    (previous, next)
}

struct GatewayLifecycle {
    revision: AtomicU64,
    lock: Mutex<()>,
    changed: Condvar,
}

impl GatewayLifecycle {
    const fn new() -> Self {
        Self {
            revision: AtomicU64::new(0),
            lock: Mutex::new(()),
            changed: Condvar::new(),
        }
    }

    fn current(&self) -> u64 {
        self.revision.load(Ordering::Acquire)
    }

    fn advance(&self) {
        self.revision.fetch_add(1, Ordering::AcqRel);
        self.changed.notify_all();
    }

    fn wait_while_current(&self, revision: u64, timeout: Duration) {
        let Ok(lock) = self.lock.lock() else {
            return;
        };
        let _ = self
            .changed
            .wait_timeout_while(lock, timeout, |()| self.current() == revision);
    }
}

fn spawn_native_sync(backend: &Arc<ProductionDesktopBackend>) -> Result<(), std::io::Error> {
    let backend = Arc::downgrade(backend);
    thread::Builder::new()
        .name(String::from("filament-e2ee-coordinator"))
        .spawn({
            let backend = backend.clone();
            move || native_sync_coordinator_loop(&backend)
        })?;
    thread::Builder::new()
        .name(String::from("filament-e2ee-gateway"))
        .spawn(move || native_gateway_listener_loop(&backend))?;
    Ok(())
}

fn native_sync_coordinator_loop(backend_weak: &Weak<ProductionDesktopBackend>) {
    let mut delay = BACKGROUND_SYNC_INTERVAL;
    let mut fallback_deadline = Instant::now() + delay;
    loop {
        let Some(backend) = backend_weak.upgrade() else {
            return;
        };
        let wakes = Arc::clone(&backend.gateway_wakes);
        drop(backend);
        let wait = fallback_deadline.saturating_duration_since(Instant::now());
        match wakes.take(wait) {
            Ok(Some(GatewayWork::Group(group_id))) => {
                let Some(backend) = backend_weak.upgrade() else {
                    return;
                };
                let _ = backend.synchronize_active_group_once(group_id);
            }
            Ok(Some(GatewayWork::KeyPackages(wake))) => {
                let Some(backend) = backend_weak.upgrade() else {
                    return;
                };
                let _ = backend.replenish_active_keypackages_once(wake);
            }
            Ok(None) => {
                let Some(backend) = backend_weak.upgrade() else {
                    return;
                };
                let succeeded = backend.synchronize_active_once().is_ok();
                delay = background_sync_delay(delay, succeeded);
                fallback_deadline = Instant::now() + delay;
            }
            Err(_) => return,
        }
        if Instant::now() >= fallback_deadline {
            let Some(backend) = backend_weak.upgrade() else {
                return;
            };
            let succeeded = backend.synchronize_active_once().is_ok();
            delay = background_sync_delay(delay, succeeded);
            fallback_deadline = Instant::now() + delay;
        }
    }
}

fn native_gateway_listener_loop(backend_weak: &Weak<ProductionDesktopBackend>) {
    let mut reconnect_delay = GATEWAY_RECONNECT_INITIAL;
    loop {
        let Some(backend) = backend_weak.upgrade() else {
            return;
        };
        let revision = backend.gateway_lifecycle.current();
        let lifecycle = Arc::clone(&backend.gateway_lifecycle);
        let connector = Arc::clone(&backend.gateway);
        let snapshot = backend.gateway_session_snapshot();
        drop(backend);

        let Ok(Some((session, expected_user_id))) = snapshot else {
            lifecycle.wait_while_current(revision, GATEWAY_RECONNECT_INITIAL);
            reconnect_delay = GATEWAY_RECONNECT_INITIAL;
            continue;
        };
        let expires_at_unix = session.expires_at_unix;
        let connection = connector.connect(&session.access_token);
        drop(session);
        let result = connection.and_then(|mut connection| {
            drive_gateway_connection(
                backend_weak,
                revision,
                expected_user_id,
                expires_at_unix,
                connection.as_mut(),
            )
        });
        let (wait, next_delay) = gateway_reconnect_schedule(reconnect_delay, result.is_ok());
        reconnect_delay = next_delay;
        lifecycle.wait_while_current(revision, wait);
    }
}

fn drive_gateway_connection(
    backend: &Weak<ProductionDesktopBackend>,
    revision: u64,
    expected_user_id: UserId,
    expires_at_unix: i64,
    connection: &mut dyn crate::native_gateway::NativeGatewayConnection,
) -> Result<(), crate::native_gateway::NativeGatewayError> {
    drive_gateway_connection_with_policy(
        backend,
        revision,
        expected_user_id,
        expires_at_unix,
        connection,
        PRODUCTION_GATEWAY_CONNECTION_POLICY,
    )
}

fn drive_gateway_connection_with_policy(
    backend: &Weak<ProductionDesktopBackend>,
    revision: u64,
    expected_user_id: UserId,
    expires_at_unix: i64,
    connection: &mut dyn crate::native_gateway::NativeGatewayConnection,
    policy: GatewayConnectionPolicy,
) -> Result<(), crate::native_gateway::NativeGatewayError> {
    let started = Instant::now();
    let mut last_activity = started;
    let mut ready = false;
    loop {
        let Some(current) = backend.upgrade() else {
            return Ok(());
        };
        if current.gateway_lifecycle.current() != revision {
            return Ok(());
        }
        if (current.clock)().map_err(|_| crate::native_gateway::NativeGatewayError::Unavailable)?
            >= expires_at_unix
        {
            return Ok(());
        }
        let wakes = Arc::clone(&current.gateway_wakes);
        drop(current);

        match connection.read_frame()? {
            NativeGatewayFrame::Text(payload) => {
                last_activity = Instant::now();
                match decode_gateway_wake(&payload)? {
                    GatewayWake::Ready(user_id) if !ready && user_id == expected_user_id => {
                        ready = true;
                    }
                    GatewayWake::Group(group_id) if ready => {
                        wakes.enqueue(group_id)?;
                    }
                    GatewayWake::KeyPackages(wake) if ready => {
                        wakes.enqueue_keypackages(wake)?;
                    }
                    GatewayWake::Ignore if ready => {}
                    GatewayWake::Ready(_)
                    | GatewayWake::Group(_)
                    | GatewayWake::KeyPackages(_)
                    | GatewayWake::Ignore => {
                        return Err(crate::native_gateway::NativeGatewayError::Rejected);
                    }
                }
            }
            NativeGatewayFrame::Activity => last_activity = Instant::now(),
            NativeGatewayFrame::Timeout => {}
            NativeGatewayFrame::Closed => {
                return Err(crate::native_gateway::NativeGatewayError::Unavailable);
            }
        }
        let now = Instant::now();
        if (!ready && now.duration_since(started) >= policy.ready_timeout)
            || now.duration_since(last_activity) >= policy.idle_timeout
        {
            return Err(crate::native_gateway::NativeGatewayError::Unavailable);
        }
    }
}

struct RuntimeState {
    host: Arc<DesktopCommandHost>,
}

impl core::fmt::Debug for RuntimeState {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("RuntimeState(<native state redacted>)")
    }
}

#[tauri::command]
async fn store_session(
    state: tauri::State<'_, RuntimeState>,
    request: StoreSessionRequest,
) -> Result<SessionMetadata, DesktopCommandError> {
    let host = Arc::clone(&state.host);
    run_native(move || host.store_session(request)).await
}

#[tauri::command]
async fn clear_session(state: tauri::State<'_, RuntimeState>) -> Result<(), DesktopCommandError> {
    let host = Arc::clone(&state.host);
    run_native(move || host.clear_session()).await
}

#[tauri::command]
async fn read_session_metadata(
    state: tauri::State<'_, RuntimeState>,
) -> Result<SessionMetadata, DesktopCommandError> {
    let host = Arc::clone(&state.host);
    run_native(move || host.read_session_metadata()).await
}

#[tauri::command]
async fn initialize_e2ee_store(
    state: tauri::State<'_, RuntimeState>,
) -> Result<E2eeStoreStatus, DesktopCommandError> {
    let host = Arc::clone(&state.host);
    run_native(move || host.initialize_e2ee_store()).await
}

#[tauri::command]
async fn read_e2ee_store_status(
    state: tauri::State<'_, RuntimeState>,
) -> Result<E2eeStoreStatus, DesktopCommandError> {
    let host = Arc::clone(&state.host);
    run_native(move || host.read_e2ee_store_status()).await
}

#[tauri::command]
async fn read_encryption_settings(
    state: tauri::State<'_, RuntimeState>,
) -> Result<EncryptionSettingsSnapshot, DesktopCommandError> {
    let host = Arc::clone(&state.host);
    run_native(move || host.read_encryption_settings()).await
}

#[tauri::command]
async fn rotate_root_identity(
    state: tauri::State<'_, RuntimeState>,
    request: RotateIdentityCommandRequest,
) -> Result<RotateRootIdentityResponse, DesktopCommandError> {
    let host = Arc::clone(&state.host);
    run_native(move || host.rotate_root_identity(request)).await
}

async fn run_native<T: Send + 'static>(
    operation: impl FnOnce() -> Result<T, DesktopCommandError> + Send + 'static,
) -> Result<T, DesktopCommandError> {
    tauri::async_runtime::spawn_blocking(operation)
        .await
        .map_err(|_| DesktopCommandError::Unavailable)?
}

fn invoke_body_within_limit(body: &InvokeBody) -> bool {
    match body {
        InvokeBody::Json(value) => serde_json::to_vec(value)
            .is_ok_and(|serialized| serialized.len() <= MAX_TAURI_IPC_REQUEST_BYTES),
        InvokeBody::Raw(bytes) => bytes.len() <= MAX_TAURI_IPC_REQUEST_BYTES,
    }
}

/// Start the single hardened Tauri application shared by desktop and mobile.
///
/// # Panics
/// Panics during process startup if the compile-time Tauri context is invalid
/// or the native runtime cannot be initialized. No secret state exists before
/// this initialization boundary.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let app_data_root = app
                .path()
                .app_data_dir()
                .map_err(|_| std::io::Error::other("native app-data path is unavailable"))?;
            prepare_app_data_root(&app_data_root)?;
            let backend = Arc::new(
                ProductionDesktopBackend::new(app_data_root)
                    .map_err(|_| std::io::Error::other("native backend initialization failed"))?,
            );
            spawn_native_sync(&backend)
                .map_err(|_| std::io::Error::other("native sync initialization failed"))?;
            app.manage(RuntimeState {
                host: Arc::new(DesktopCommandHost::new(backend)),
            });
            Ok(())
        })
        .plugin(
            tauri::plugin::Builder::<tauri::Wry, ()>::new("filament-navigation-policy")
                .on_navigation(|_webview, url| {
                    validate_runtime_navigation(url.as_str(), cfg!(dev)).is_ok()
                })
                .build(),
        )
        .invoke_handler(handle_invoke)
        .run(tauri::generate_context!())
        .expect("hardened Tauri runtime failed");
}

fn prepare_app_data_root(path: &Path) -> Result<(), std::io::Error> {
    fs::create_dir_all(path)?;
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() || !path.is_absolute() {
        return Err(std::io::Error::other("native app-data root is invalid"));
    }
    Ok(())
}

fn handle_invoke(invoke: tauri::ipc::Invoke<tauri::Wry>) -> bool {
    if invoke_body_within_limit(invoke.message.payload()) {
        let handler: fn(tauri::ipc::Invoke<tauri::Wry>) -> bool = tauri::generate_handler![
            store_session,
            clear_session,
            read_session_metadata,
            initialize_e2ee_store,
            read_e2ee_store_status,
            read_encryption_settings,
            rotate_root_identity,
        ];
        handler(invoke)
    } else {
        invoke.resolver.reject(DesktopCommandError::InvalidRequest);
        true
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashMap,
        sync::{
            atomic::{AtomicBool, AtomicUsize, Ordering},
            Mutex,
        },
    };

    use super::*;
    use filament_e2ee::{
        AttachmentId, DeliveryServiceSigner, EncryptedAttachment, InMemoryKeyStore,
        PendingKeyPackageUpload, DELIVERY_SERVICE_SEED_BYTES,
    };
    use filament_protocol::{
        AckE2eeAttachmentsRequest, AckE2eeCommitsRequest, AckE2eeMessagesRequest,
        AckE2eeProposalsRequest, CreateMlsConversationRequest, DeviceInfo, DeviceListResponse,
        E2eeCommitMailboxEntry, E2eeCommitMailboxResponse, E2eeMailboxResponse,
        E2eeProposalMailboxEntry, E2eeProposalMailboxResponse, Envelope, EventType,
        KeyPackageLowEvent, MlsConversationProvisionResponse, MlsMessageEvent, PostCommitRequest,
        PostCommitResponse, PostMessageRequest, PostMessageResponse, PutE2eeAttachmentResponse,
        RootIdentityDirectoryResponse, RootIdentityRotationEntry, RotateRootIdentityRequest,
        PROTOCOL_VERSION, ROOT_IDENTITY_ROTATION_PROTOCOL_VERSION,
    };

    #[derive(Default)]
    struct MemorySessionStore {
        session: Mutex<Option<(String, String, i64)>>,
    }

    impl SessionCredentialStore for MemorySessionStore {
        fn store(
            &self,
            request: &ValidatedStoreSessionRequest,
        ) -> Result<StoredSessionMetadata, SessionCredentialError> {
            let expiry = request.expires_at_unix.as_i64();
            *self.session.lock().unwrap() = Some((
                request.access_token.expose().to_owned(),
                request.refresh_token.expose().to_owned(),
                expiry,
            ));
            Ok(StoredSessionMetadata {
                expires_at_unix: expiry,
            })
        }

        fn clear(&self) -> Result<(), SessionCredentialError> {
            *self.session.lock().unwrap() = None;
            Ok(())
        }

        fn metadata(&self) -> Result<Option<StoredSessionMetadata>, SessionCredentialError> {
            Ok(self
                .session
                .lock()
                .unwrap()
                .as_ref()
                .map(|(_, _, expires_at_unix)| StoredSessionMetadata {
                    expires_at_unix: *expires_at_unix,
                }))
        }

        fn load(&self) -> Result<Option<StoredSession>, SessionCredentialError> {
            self.session
                .lock()
                .unwrap()
                .as_ref()
                .map(|(access_token, refresh_token, expires_at_unix)| {
                    Ok(StoredSession {
                        access_token: crate::SessionToken::new(access_token.clone())
                            .map_err(|_| SessionCredentialError::Rejected)?,
                        expires_at_unix: *expires_at_unix,
                    })
                    .and_then(|session| {
                        let refresh_token = crate::SessionToken::new(refresh_token.clone())
                            .map_err(|_| SessionCredentialError::Rejected)?;
                        drop(refresh_token);
                        Ok(session)
                    })
                })
                .transpose()
        }
    }

    #[derive(Default)]
    struct MemoryDeviceRegistry {
        bindings: Mutex<HashMap<UserId, DeviceId>>,
    }

    impl DeviceRegistry for MemoryDeviceRegistry {
        fn device_for(&self, user_id: UserId) -> Result<Option<DeviceId>, DeviceRegistryError> {
            Ok(self.bindings.lock().unwrap().get(&user_id).copied())
        }

        fn bind(&self, user_id: UserId, device_id: DeviceId) -> Result<(), DeviceRegistryError> {
            let mut bindings = self.bindings.lock().unwrap();
            match bindings.get(&user_id) {
                Some(existing) if *existing != device_id => Err(DeviceRegistryError::Rejected),
                Some(_) => Ok(()),
                None => {
                    bindings.insert(user_id, device_id);
                    Ok(())
                }
            }
        }
    }

    struct MemoryStoreFactory {
        store: Arc<InMemoryKeyStore>,
    }

    impl NativeStoreFactory for MemoryStoreFactory {
        fn open(
            &self,
            _store_id: LocalStoreId,
        ) -> Result<Arc<dyn LocalKeyStore>, DesktopCommandBackendError> {
            Ok(self.store.clone())
        }
    }

    struct MockEnrollmentApi {
        user_id: UserId,
        devices: Mutex<Vec<DeviceInfo>>,
        rotations: Mutex<Vec<RootIdentityRotationEntry>>,
        last_rotation: Mutex<Option<(RotateRootIdentityRequest, RotateRootIdentityResponse)>>,
        lose_rotation_response: AtomicBool,
        uploaded: AtomicUsize,
        reject_upload: AtomicBool,
        message_mailboxes: Mutex<HashMap<filament_core::GroupId, E2eeMailboxResponse>>,
        lose_message_ack_response: AtomicBool,
        message_ack_attempts: AtomicUsize,
        accepted_messages:
            Mutex<HashMap<filament_core::GroupId, (PostMessageRequest, PostMessageResponse)>>,
        lose_message_response: AtomicBool,
        message_attempts: AtomicUsize,
        attachments: Mutex<HashMap<(filament_core::GroupId, AttachmentId), EncryptedAttachment>>,
        lose_attachment_upload_response: AtomicBool,
        attachment_upload_attempts: AtomicUsize,
        lose_attachment_ack_response: AtomicBool,
        attachment_ack_attempts: AtomicUsize,
        commit_mailboxes: Mutex<HashMap<filament_core::GroupId, E2eeCommitMailboxResponse>>,
        lose_commit_ack_response: AtomicBool,
        commit_ack_attempts: AtomicUsize,
        proposal_mailboxes: Mutex<HashMap<filament_core::GroupId, E2eeProposalMailboxResponse>>,
        proposal_ack_attempts: AtomicUsize,
        accepted_commits:
            Mutex<HashMap<filament_core::GroupId, (PostCommitRequest, PostCommitResponse)>>,
        lose_commit_response: AtomicBool,
        commit_attempts: AtomicUsize,
        accepted_provisions: Mutex<
            HashMap<
                filament_core::ConversationId,
                (
                    CreateMlsConversationRequest,
                    MlsConversationProvisionResponse,
                ),
            >,
        >,
        lose_provision_response: AtomicBool,
        provision_attempts: AtomicUsize,
    }

    impl MockEnrollmentApi {
        fn fresh(user_id: UserId) -> Self {
            Self {
                user_id,
                devices: Mutex::new(Vec::new()),
                rotations: Mutex::new(Vec::new()),
                last_rotation: Mutex::new(None),
                lose_rotation_response: AtomicBool::new(false),
                uploaded: AtomicUsize::new(0),
                reject_upload: AtomicBool::new(false),
                message_mailboxes: Mutex::new(HashMap::new()),
                lose_message_ack_response: AtomicBool::new(false),
                message_ack_attempts: AtomicUsize::new(0),
                accepted_messages: Mutex::new(HashMap::new()),
                lose_message_response: AtomicBool::new(false),
                message_attempts: AtomicUsize::new(0),
                attachments: Mutex::new(HashMap::new()),
                lose_attachment_upload_response: AtomicBool::new(false),
                attachment_upload_attempts: AtomicUsize::new(0),
                lose_attachment_ack_response: AtomicBool::new(false),
                attachment_ack_attempts: AtomicUsize::new(0),
                commit_mailboxes: Mutex::new(HashMap::new()),
                lose_commit_ack_response: AtomicBool::new(false),
                commit_ack_attempts: AtomicUsize::new(0),
                proposal_mailboxes: Mutex::new(HashMap::new()),
                proposal_ack_attempts: AtomicUsize::new(0),
                accepted_commits: Mutex::new(HashMap::new()),
                lose_commit_response: AtomicBool::new(false),
                commit_attempts: AtomicUsize::new(0),
                accepted_provisions: Mutex::new(HashMap::new()),
                lose_provision_response: AtomicBool::new(false),
                provision_attempts: AtomicUsize::new(0),
            }
        }

        fn with_device(device: &MlsDevice) -> Self {
            Self {
                user_id: device.user_id(),
                devices: Mutex::new(vec![device_info(device)]),
                rotations: Mutex::new(Vec::new()),
                last_rotation: Mutex::new(None),
                lose_rotation_response: AtomicBool::new(false),
                uploaded: AtomicUsize::new(0),
                reject_upload: AtomicBool::new(false),
                message_mailboxes: Mutex::new(HashMap::new()),
                lose_message_ack_response: AtomicBool::new(false),
                message_ack_attempts: AtomicUsize::new(0),
                accepted_messages: Mutex::new(HashMap::new()),
                lose_message_response: AtomicBool::new(false),
                message_attempts: AtomicUsize::new(0),
                attachments: Mutex::new(HashMap::new()),
                lose_attachment_upload_response: AtomicBool::new(false),
                attachment_upload_attempts: AtomicUsize::new(0),
                lose_attachment_ack_response: AtomicBool::new(false),
                attachment_ack_attempts: AtomicUsize::new(0),
                commit_mailboxes: Mutex::new(HashMap::new()),
                lose_commit_ack_response: AtomicBool::new(false),
                commit_ack_attempts: AtomicUsize::new(0),
                proposal_mailboxes: Mutex::new(HashMap::new()),
                proposal_ack_attempts: AtomicUsize::new(0),
                accepted_commits: Mutex::new(HashMap::new()),
                lose_commit_response: AtomicBool::new(false),
                commit_attempts: AtomicUsize::new(0),
                accepted_provisions: Mutex::new(HashMap::new()),
                lose_provision_response: AtomicBool::new(false),
                provision_attempts: AtomicUsize::new(0),
            }
        }
    }

    impl NativeEnrollmentApi for MockEnrollmentApi {
        fn current_user(
            &self,
            _access_token: &crate::SessionToken,
        ) -> Result<UserId, NativeApiError> {
            Ok(self.user_id)
        }

        fn list_devices(
            &self,
            _access_token: &crate::SessionToken,
            user_id: UserId,
        ) -> Result<DeviceListResponse, NativeApiError> {
            if user_id != self.user_id {
                return Err(NativeApiError::Rejected);
            }
            Ok(DeviceListResponse {
                user_id: user_id.to_string(),
                devices: self.devices.lock().unwrap().clone(),
            })
        }

        fn publish_device(
            &self,
            _access_token: &crate::SessionToken,
            device: &MlsDevice,
        ) -> Result<(), NativeApiError> {
            if device.user_id() != self.user_id {
                return Err(NativeApiError::Rejected);
            }
            let entry = device_info(device);
            let mut devices = self.devices.lock().unwrap();
            match devices
                .iter()
                .find(|existing| existing.device_id == entry.device_id)
            {
                Some(existing) if existing != &entry => Err(NativeApiError::Rejected),
                Some(_) => Ok(()),
                None if devices.is_empty() => {
                    devices.push(entry);
                    Ok(())
                }
                None => Err(NativeApiError::Rejected),
            }
        }

        fn upload_keypackages(
            &self,
            _access_token: &crate::SessionToken,
            _device_id: DeviceId,
            pending: &PendingKeyPackageUpload,
        ) -> Result<(), NativeApiError> {
            if self.reject_upload.load(Ordering::SeqCst) {
                return Err(NativeApiError::Unavailable);
            }
            self.uploaded
                .store(pending.packages.len(), Ordering::SeqCst);
            Ok(())
        }

        fn root_identity(
            &self,
            _access_token: &crate::SessionToken,
            user_id: UserId,
        ) -> Result<RootIdentityDirectoryResponse, NativeApiError> {
            if user_id != self.user_id {
                return Err(NativeApiError::Rejected);
            }
            let devices = self.devices.lock().unwrap();
            let current = devices.first().ok_or(NativeApiError::Rejected)?;
            let rotations = self.rotations.lock().unwrap().clone();
            Ok(RootIdentityDirectoryResponse {
                protocol_version: ROOT_IDENTITY_ROTATION_PROTOCOL_VERSION,
                user_id: user_id.to_string(),
                current_root_key_pub: current.root_key_pub.clone(),
                rotation_sequence: u64::try_from(rotations.len())
                    .map_err(|_| NativeApiError::Unavailable)?,
                rotations,
            })
        }

        fn provision_conversation(
            &self,
            _access_token: &crate::SessionToken,
            request: &CreateMlsConversationRequest,
        ) -> Result<MlsConversationProvisionResponse, NativeApiError> {
            self.provision_attempts.fetch_add(1, Ordering::SeqCst);
            let conversation_id =
                filament_core::ConversationId::try_from(request.conversation_id.clone())
                    .map_err(|_| NativeApiError::Rejected)?;
            let mut accepted = self.accepted_provisions.lock().unwrap();
            let response =
                if let Some((previous, response)) = accepted.get(&conversation_id).cloned() {
                    if previous != *request {
                        return Err(NativeApiError::Rejected);
                    }
                    response
                } else {
                    let response = MlsConversationProvisionResponse {
                        conversation_id: request.conversation_id.clone(),
                        group_id: request.group_id.clone(),
                        crypto: String::from("mls_v1"),
                        epoch: 1,
                        suite_id: request.suite_id,
                        provisioned_at_unix: 201,
                    };
                    accepted.insert(conversation_id, (request.clone(), response.clone()));
                    response
                };
            if self.lose_provision_response.swap(false, Ordering::SeqCst) {
                return Err(NativeApiError::Unavailable);
            }
            Ok(response)
        }

        fn rotate_root_identity(
            &self,
            _access_token: &crate::SessionToken,
            request: &RotateRootIdentityRequest,
        ) -> Result<RotateRootIdentityResponse, NativeApiError> {
            if let Some((previous_request, response)) = self.last_rotation.lock().unwrap().as_ref()
            {
                if previous_request == request {
                    return Ok(response.clone());
                }
            }
            let mut devices = self.devices.lock().unwrap();
            let mut rotations = self.rotations.lock().unwrap();
            let current = devices.first().ok_or(NativeApiError::Rejected)?;
            let sequence =
                u64::try_from(rotations.len()).map_err(|_| NativeApiError::Unavailable)?;
            if request.expected_rotation_sequence != sequence
                || request.device_id != current.device_id
            {
                return Err(NativeApiError::Rejected);
            }
            let next_sequence = sequence.checked_add(1).ok_or(NativeApiError::Rejected)?;
            let previous_root_key_pub = current.root_key_pub.clone();
            let revoked_device_count = u32::try_from(devices.len().saturating_sub(1))
                .map_err(|_| NativeApiError::Unavailable)?;
            let device_id = request.device_id.clone();
            *devices = vec![DeviceInfo {
                device_id: device_id.clone(),
                device_signature_pubkey: request.new_device_signature_pubkey.clone(),
                root_key_signature: request.new_device_root_signature.clone(),
                root_key_pub: request.new_root_key_pub.clone(),
                created_at_unix: 201,
                tombstoned_at_unix: None,
            }];
            rotations.push(RootIdentityRotationEntry {
                sequence: next_sequence,
                previous_root_key_pub: previous_root_key_pub.clone(),
                new_root_key_pub: request.new_root_key_pub.clone(),
                previous_root_signature: request.previous_root_signature.clone(),
                new_root_signature: request.new_root_signature.clone(),
                rotating_device_id: device_id.clone(),
                rotated_at_unix: 201,
            });
            let response = RotateRootIdentityResponse {
                protocol_version: ROOT_IDENTITY_ROTATION_PROTOCOL_VERSION,
                user_id: self.user_id.to_string(),
                device_id,
                rotation_sequence: next_sequence,
                previous_root_key_pub,
                new_root_key_pub: request.new_root_key_pub.clone(),
                revoked_device_count,
                deleted_keypackage_count: 0,
                rotated_at_unix: 201,
            };
            *self.last_rotation.lock().unwrap() = Some((request.clone(), response.clone()));
            if self.lose_rotation_response.swap(false, Ordering::SeqCst) {
                return Err(NativeApiError::Unavailable);
            }
            Ok(response)
        }

        fn message_mailbox(
            &self,
            _access_token: &crate::SessionToken,
            group_id: filament_core::GroupId,
            _device_id: DeviceId,
        ) -> Result<E2eeMailboxResponse, NativeApiError> {
            Ok(self
                .message_mailboxes
                .lock()
                .unwrap()
                .get(&group_id)
                .cloned()
                .unwrap_or(E2eeMailboxResponse {
                    messages: Vec::new(),
                    next_after_message_id: None,
                }))
        }

        fn acknowledge_messages(
            &self,
            _access_token: &crate::SessionToken,
            group_id: filament_core::GroupId,
            request: &AckE2eeMessagesRequest,
        ) -> Result<(), NativeApiError> {
            self.message_ack_attempts.fetch_add(1, Ordering::SeqCst);
            if self.lose_message_ack_response.swap(false, Ordering::SeqCst) {
                return Err(NativeApiError::Unavailable);
            }
            let mut mailboxes = self.message_mailboxes.lock().unwrap();
            let page = mailboxes
                .get_mut(&group_id)
                .ok_or(NativeApiError::Rejected)?;
            page.messages
                .retain(|message| !request.message_ids.contains(&message.message_id));
            page.next_after_message_id = page
                .messages
                .last()
                .map(|message| message.message_id.clone());
            Ok(())
        }

        fn get_attachment(
            &self,
            _access_token: &crate::SessionToken,
            group_id: filament_core::GroupId,
            attachment_id: AttachmentId,
            _device_id: DeviceId,
        ) -> Result<EncryptedAttachment, NativeApiError> {
            self.attachments
                .lock()
                .unwrap()
                .get(&(group_id, attachment_id))
                .cloned()
                .ok_or(NativeApiError::Rejected)
        }

        fn acknowledge_attachments(
            &self,
            _access_token: &crate::SessionToken,
            _group_id: filament_core::GroupId,
            request: &AckE2eeAttachmentsRequest,
        ) -> Result<(), NativeApiError> {
            self.attachment_ack_attempts.fetch_add(1, Ordering::SeqCst);
            if request.attachment_ids.len() != 1
                || AttachmentId::try_from(request.attachment_ids[0].clone()).is_err()
            {
                return Err(NativeApiError::Rejected);
            }
            if self
                .lose_attachment_ack_response
                .swap(false, Ordering::SeqCst)
            {
                return Err(NativeApiError::Unavailable);
            }
            Ok(())
        }

        fn put_attachment(
            &self,
            _access_token: &crate::SessionToken,
            group_id: filament_core::GroupId,
            _device_id: DeviceId,
            attachment: EncryptedAttachment,
        ) -> Result<PutE2eeAttachmentResponse, NativeApiError> {
            self.attachment_upload_attempts
                .fetch_add(1, Ordering::SeqCst);
            let attachment_id = attachment.attachment_id;
            let ciphertext_bytes =
                u64::try_from(attachment.ciphertext.len()).map_err(|_| NativeApiError::Rejected)?;
            let mut attachments = self.attachments.lock().unwrap();
            match attachments.get(&(group_id, attachment_id)) {
                Some(existing) if existing != &attachment => {
                    return Err(NativeApiError::Rejected);
                }
                Some(_) => {}
                None => {
                    attachments.insert((group_id, attachment_id), attachment);
                }
            }
            drop(attachments);
            if self
                .lose_attachment_upload_response
                .swap(false, Ordering::SeqCst)
            {
                return Err(NativeApiError::Unavailable);
            }
            Ok(PutE2eeAttachmentResponse {
                attachment_id: attachment_id.to_string(),
                ciphertext_bytes,
                expires_at_unix: 500,
            })
        }

        fn commit_mailbox(
            &self,
            _access_token: &crate::SessionToken,
            group_id: filament_core::GroupId,
            _device_id: DeviceId,
        ) -> Result<E2eeCommitMailboxResponse, NativeApiError> {
            Ok(self
                .commit_mailboxes
                .lock()
                .unwrap()
                .get(&group_id)
                .cloned()
                .unwrap_or(E2eeCommitMailboxResponse {
                    commits: Vec::new(),
                    next_after_epoch: None,
                }))
        }

        fn acknowledge_commits(
            &self,
            _access_token: &crate::SessionToken,
            group_id: filament_core::GroupId,
            request: &AckE2eeCommitsRequest,
        ) -> Result<(), NativeApiError> {
            self.commit_ack_attempts.fetch_add(1, Ordering::SeqCst);
            if self.lose_commit_ack_response.swap(false, Ordering::SeqCst) {
                return Err(NativeApiError::Unavailable);
            }
            let mut mailboxes = self.commit_mailboxes.lock().unwrap();
            let page = mailboxes
                .get_mut(&group_id)
                .ok_or(NativeApiError::Rejected)?;
            page.commits
                .retain(|commit| !request.epochs.contains(&commit.epoch));
            page.next_after_epoch = page.commits.last().map(|commit| commit.epoch);
            Ok(())
        }

        fn proposal_mailbox(
            &self,
            _access_token: &crate::SessionToken,
            group_id: filament_core::GroupId,
            _device_id: DeviceId,
        ) -> Result<E2eeProposalMailboxResponse, NativeApiError> {
            Ok(self
                .proposal_mailboxes
                .lock()
                .unwrap()
                .get(&group_id)
                .cloned()
                .unwrap_or(E2eeProposalMailboxResponse {
                    proposals: Vec::new(),
                    next_after_proposal_id: None,
                }))
        }

        fn acknowledge_proposals(
            &self,
            _access_token: &crate::SessionToken,
            group_id: filament_core::GroupId,
            request: &AckE2eeProposalsRequest,
        ) -> Result<(), NativeApiError> {
            self.proposal_ack_attempts.fetch_add(1, Ordering::SeqCst);
            let mut mailboxes = self.proposal_mailboxes.lock().unwrap();
            let page = mailboxes
                .get_mut(&group_id)
                .ok_or(NativeApiError::Rejected)?;
            page.proposals
                .retain(|proposal| !request.proposal_ids.contains(&proposal.proposal_id));
            page.next_after_proposal_id = page
                .proposals
                .last()
                .map(|proposal| proposal.proposal_id.clone());
            Ok(())
        }

        fn post_commit(
            &self,
            _access_token: &crate::SessionToken,
            group_id: filament_core::GroupId,
            request: &PostCommitRequest,
        ) -> Result<PostCommitResponse, NativeApiError> {
            self.commit_attempts.fetch_add(1, Ordering::SeqCst);
            let mut accepted = self.accepted_commits.lock().unwrap();
            let response = if let Some((previous, response)) = accepted.get(&group_id).cloned() {
                if previous == *request {
                    response
                } else if request.prior_epoch < response.epoch {
                    return Err(NativeApiError::EpochConflict);
                } else if request.prior_epoch == response.epoch {
                    let response = PostCommitResponse {
                        accepted: true,
                        epoch: request.epoch,
                    };
                    accepted.insert(group_id, (request.clone(), response.clone()));
                    response
                } else {
                    return Err(NativeApiError::Rejected);
                }
            } else {
                let response = PostCommitResponse {
                    accepted: true,
                    epoch: request.epoch,
                };
                accepted.insert(group_id, (request.clone(), response.clone()));
                response
            };
            if self.lose_commit_response.swap(false, Ordering::SeqCst) {
                return Err(NativeApiError::Unavailable);
            }
            Ok(response)
        }

        fn post_message(
            &self,
            _access_token: &crate::SessionToken,
            group_id: filament_core::GroupId,
            request: &PostMessageRequest,
        ) -> Result<PostMessageResponse, NativeApiError> {
            self.message_attempts.fetch_add(1, Ordering::SeqCst);
            let mut accepted = self.accepted_messages.lock().unwrap();
            let response = if let Some((previous, response)) = accepted.get(&group_id).cloned() {
                if previous != *request {
                    return Err(NativeApiError::Rejected);
                }
                response
            } else {
                let response = PostMessageResponse {
                    message_id: filament_e2ee::EncryptedMessageId::new().to_string(),
                    created_at_unix: 201,
                };
                accepted.insert(group_id, (request.clone(), response.clone()));
                response
            };
            if self.lose_message_response.swap(false, Ordering::SeqCst) {
                return Err(NativeApiError::Unavailable);
            }
            Ok(response)
        }
    }

    struct UnavailableGatewayConnector;

    impl NativeGatewayConnector for UnavailableGatewayConnector {
        fn connect(
            &self,
            _access_token: &crate::SessionToken,
        ) -> Result<
            Box<dyn crate::native_gateway::NativeGatewayConnection>,
            crate::native_gateway::NativeGatewayError,
        > {
            Err(crate::native_gateway::NativeGatewayError::Unavailable)
        }
    }

    struct ScriptedGatewayConnector {
        frames: Mutex<Option<std::collections::VecDeque<NativeGatewayFrame>>>,
    }

    impl ScriptedGatewayConnector {
        fn new(frames: Vec<NativeGatewayFrame>) -> Self {
            Self {
                frames: Mutex::new(Some(frames.into())),
            }
        }
    }

    impl NativeGatewayConnector for ScriptedGatewayConnector {
        fn connect(
            &self,
            _access_token: &crate::SessionToken,
        ) -> Result<
            Box<dyn crate::native_gateway::NativeGatewayConnection>,
            crate::native_gateway::NativeGatewayError,
        > {
            let frames = self
                .frames
                .lock()
                .unwrap()
                .take()
                .ok_or(crate::native_gateway::NativeGatewayError::Unavailable)?;
            Ok(Box::new(ScriptedGatewayConnection { frames }))
        }
    }

    struct ScriptedGatewayConnection {
        frames: std::collections::VecDeque<NativeGatewayFrame>,
    }

    impl crate::native_gateway::NativeGatewayConnection for ScriptedGatewayConnection {
        fn read_frame(
            &mut self,
        ) -> Result<NativeGatewayFrame, crate::native_gateway::NativeGatewayError> {
            if let Some(frame) = self.frames.pop_front() {
                return Ok(frame);
            }
            thread::sleep(Duration::from_millis(10));
            Ok(NativeGatewayFrame::Timeout)
        }
    }

    struct CountingTimeoutGatewayConnection {
        reads: Arc<AtomicUsize>,
    }

    impl crate::native_gateway::NativeGatewayConnection for CountingTimeoutGatewayConnection {
        fn read_frame(
            &mut self,
        ) -> Result<NativeGatewayFrame, crate::native_gateway::NativeGatewayError> {
            self.reads.fetch_add(1, Ordering::SeqCst);
            thread::sleep(Duration::from_millis(2));
            Ok(NativeGatewayFrame::Timeout)
        }
    }

    fn device_info(device: &MlsDevice) -> DeviceInfo {
        DeviceInfo {
            device_id: device.device_id().to_string(),
            device_signature_pubkey: device.certificate().device_signature_pubkey.clone(),
            root_key_signature: device.certificate().root_key_signature.clone(),
            root_key_pub: device.root_key_public().to_vec(),
            created_at_unix: 200,
            tombstoned_at_unix: None,
        }
    }

    fn backend_fixture(
        api: Arc<MockEnrollmentApi>,
    ) -> (
        ProductionDesktopBackend,
        Arc<MemorySessionStore>,
        Arc<MemoryDeviceRegistry>,
        Arc<InMemoryKeyStore>,
    ) {
        backend_fixture_with_gateway(api, Arc::new(UnavailableGatewayConnector))
    }

    fn backend_fixture_with_gateway(
        api: Arc<MockEnrollmentApi>,
        gateway: Arc<dyn NativeGatewayConnector>,
    ) -> (
        ProductionDesktopBackend,
        Arc<MemorySessionStore>,
        Arc<MemoryDeviceRegistry>,
        Arc<InMemoryKeyStore>,
    ) {
        let session_store = Arc::new(MemorySessionStore::default());
        let registry = Arc::new(MemoryDeviceRegistry::default());
        let local_store = Arc::new(InMemoryKeyStore::new());
        let backend = ProductionDesktopBackend::with_dependencies(
            session_store.clone(),
            registry.clone(),
            api,
            Arc::new(MemoryStoreFactory {
                store: local_store.clone(),
            }),
            gateway,
            || Ok(100),
        );
        (backend, session_store, registry, local_store)
    }

    fn valid_session() -> ValidatedStoreSessionRequest {
        ValidatedStoreSessionRequest::try_from_dto(
            StoreSessionRequest {
                access_token: "A".repeat(64),
                refresh_token: "B".repeat(64),
                expires_at_unix: 500,
            },
            100,
        )
        .unwrap()
    }

    fn gateway_ready_payload(user_id: UserId) -> Vec<u8> {
        serde_json::to_vec(&Envelope {
            v: PROTOCOL_VERSION,
            t: EventType::try_from(String::from("ready")).unwrap(),
            d: serde_json::json!({ "user_id": user_id.to_string() }),
        })
        .unwrap()
    }

    fn gateway_message_payload(group_id: filament_core::GroupId) -> Vec<u8> {
        serde_json::to_vec(&Envelope {
            v: PROTOCOL_VERSION,
            t: EventType::try_from(String::from("mls_message")).unwrap(),
            d: MlsMessageEvent {
                group_id: group_id.to_string(),
                conversation_id: filament_core::ConversationId::new().to_string(),
                message_id: filament_e2ee::EncryptedMessageId::new().to_string(),
                epoch: 1,
                suite_id:
                    filament_core::CiphersuiteId::MLS_128_DHKEMX25519_CHACHA20POLY1305_SHA256_ED25519
                        .as_u16(),
                sender_device_id: DeviceId::new().to_string(),
                created_at_unix: 100,
            },
        })
        .unwrap()
    }

    fn gateway_keypackage_low_payload(device_id: DeviceId) -> Vec<u8> {
        serde_json::to_vec(&Envelope {
            v: PROTOCOL_VERSION,
            t: EventType::try_from(String::from("keypackage_low")).unwrap(),
            d: KeyPackageLowEvent {
                device_id: device_id.to_string(),
                remaining_count: 3,
                water_mark: 10,
                created_at_unix: 100,
            },
        })
        .unwrap()
    }

    #[test]
    fn ipc_request_limit_is_inclusive_and_rejects_raw_oversize() {
        assert!(invoke_body_within_limit(&InvokeBody::Raw(vec![
            0;
            MAX_TAURI_IPC_REQUEST_BYTES
        ])));
        assert!(!invoke_body_within_limit(&InvokeBody::Raw(vec![
            0;
            MAX_TAURI_IPC_REQUEST_BYTES
                + 1
        ])));
    }

    #[test]
    fn background_sync_backoff_is_bounded_and_resets_after_success() {
        assert_eq!(
            background_sync_delay(BACKGROUND_SYNC_INTERVAL, false),
            Duration::from_secs(30)
        );
        assert_eq!(
            background_sync_delay(BACKGROUND_SYNC_MAX_BACKOFF, false),
            BACKGROUND_SYNC_MAX_BACKOFF
        );
        assert_eq!(
            background_sync_delay(BACKGROUND_SYNC_MAX_BACKOFF, true),
            BACKGROUND_SYNC_INTERVAL
        );
    }

    #[test]
    fn gateway_reconnect_starts_at_one_second_and_caps_exponential_backoff() {
        assert_eq!(
            gateway_reconnect_schedule(GATEWAY_RECONNECT_INITIAL, false),
            (Duration::from_secs(1), Duration::from_secs(2))
        );
        assert_eq!(
            gateway_reconnect_schedule(GATEWAY_RECONNECT_MAX, false),
            (GATEWAY_RECONNECT_MAX, GATEWAY_RECONNECT_MAX)
        );
        assert_eq!(
            gateway_reconnect_schedule(Duration::from_secs(8), true),
            (Duration::from_secs(8), GATEWAY_RECONNECT_INITIAL)
        );
    }

    #[test]
    fn background_sync_is_idle_before_native_initialization_and_skips_busy_state() {
        let api = Arc::new(MockEnrollmentApi::fresh(UserId::new()));
        let (backend, _session_store, _registry, _local_store) = backend_fixture(api);
        assert_eq!(
            backend.synchronize_active_once(),
            Ok(BackgroundSyncOutcome::Idle)
        );

        let _active = backend.active.lock().unwrap();
        assert_eq!(
            backend.synchronize_active_once(),
            Ok(BackgroundSyncOutcome::Busy)
        );
    }

    #[test]
    fn native_gateway_rejects_wrong_ready_identity_before_any_wake() {
        let expected_user = UserId::new();
        let wrong_user = UserId::new();
        let ready = gateway_ready_payload(wrong_user);
        let api = Arc::new(MockEnrollmentApi::fresh(expected_user));
        let (backend, _session_store, _registry, _local_store) = backend_fixture(api);
        let backend = Arc::new(backend);
        let weak = Arc::downgrade(&backend);
        let mut connection = ScriptedGatewayConnection {
            frames: vec![NativeGatewayFrame::Text(ready)].into(),
        };

        assert_eq!(
            drive_gateway_connection(
                &weak,
                backend.gateway_lifecycle.current(),
                expected_user,
                500,
                &mut connection,
            ),
            Err(crate::native_gateway::NativeGatewayError::Rejected)
        );
        assert_eq!(backend.gateway_wakes.take(Duration::ZERO), Ok(None));
    }

    #[test]
    fn native_gateway_requires_ready_before_the_bounded_deadline() {
        let expected_user = UserId::new();
        let api = Arc::new(MockEnrollmentApi::fresh(expected_user));
        let (backend, _session_store, _registry, _local_store) = backend_fixture(api);
        let backend = Arc::new(backend);
        let weak = Arc::downgrade(&backend);
        let mut connection = ScriptedGatewayConnection {
            frames: std::collections::VecDeque::new(),
        };

        assert_eq!(
            drive_gateway_connection_with_policy(
                &weak,
                backend.gateway_lifecycle.current(),
                expected_user,
                500,
                &mut connection,
                GatewayConnectionPolicy {
                    ready_timeout: Duration::from_millis(1),
                    idle_timeout: Duration::from_secs(1),
                },
            ),
            Err(crate::native_gateway::NativeGatewayError::Unavailable)
        );
        assert_eq!(backend.gateway_wakes.take(Duration::ZERO), Ok(None));
    }

    #[test]
    fn native_gateway_closes_an_idle_ready_connection() {
        let expected_user = UserId::new();
        let api = Arc::new(MockEnrollmentApi::fresh(expected_user));
        let (backend, _session_store, _registry, _local_store) = backend_fixture(api);
        let backend = Arc::new(backend);
        let weak = Arc::downgrade(&backend);
        let mut connection = ScriptedGatewayConnection {
            frames: vec![NativeGatewayFrame::Text(gateway_ready_payload(
                expected_user,
            ))]
            .into(),
        };

        assert_eq!(
            drive_gateway_connection_with_policy(
                &weak,
                backend.gateway_lifecycle.current(),
                expected_user,
                500,
                &mut connection,
                GatewayConnectionPolicy {
                    ready_timeout: Duration::from_secs(1),
                    idle_timeout: Duration::from_millis(1),
                },
            ),
            Err(crate::native_gateway::NativeGatewayError::Unavailable)
        );
    }

    #[test]
    fn native_gateway_disconnects_when_distinct_wakes_overflow() {
        let expected_user = UserId::new();
        let api = Arc::new(MockEnrollmentApi::fresh(expected_user));
        let (backend, _session_store, _registry, _local_store) = backend_fixture(api);
        for _ in 0..crate::native_gateway::MAX_GATEWAY_WAKE_GROUPS {
            backend
                .gateway_wakes
                .enqueue(filament_core::GroupId::new())
                .unwrap();
        }
        let overflow_group = filament_core::GroupId::new();
        let mut connection = ScriptedGatewayConnection {
            frames: vec![
                NativeGatewayFrame::Text(gateway_ready_payload(expected_user)),
                NativeGatewayFrame::Text(gateway_message_payload(overflow_group)),
            ]
            .into(),
        };
        let backend = Arc::new(backend);

        assert_eq!(
            drive_gateway_connection(
                &Arc::downgrade(&backend),
                backend.gateway_lifecycle.current(),
                expected_user,
                500,
                &mut connection,
            ),
            Err(crate::native_gateway::NativeGatewayError::Rejected)
        );
    }

    #[test]
    fn native_gateway_queues_keypackage_work_only_after_ready() {
        let expected_user = UserId::new();
        let device_id = DeviceId::new();
        let api = Arc::new(MockEnrollmentApi::fresh(expected_user));
        let (backend, _session_store, _registry, _local_store) = backend_fixture(api);
        let mut connection = ScriptedGatewayConnection {
            frames: vec![
                NativeGatewayFrame::Text(gateway_ready_payload(expected_user)),
                NativeGatewayFrame::Text(gateway_keypackage_low_payload(device_id)),
                NativeGatewayFrame::Closed,
            ]
            .into(),
        };
        let backend = Arc::new(backend);

        assert_eq!(
            drive_gateway_connection(
                &Arc::downgrade(&backend),
                backend.gateway_lifecycle.current(),
                expected_user,
                500,
                &mut connection,
            ),
            Err(crate::native_gateway::NativeGatewayError::Unavailable)
        );
        assert_eq!(
            backend.gateway_wakes.take(Duration::ZERO),
            Ok(Some(GatewayWork::KeyPackages(KeyPackageLowWake {
                device_id,
                remaining_count: 3,
                water_mark: 10,
            })))
        );
    }

    #[test]
    fn native_gateway_stops_before_reading_after_token_expiry() {
        let expected_user = UserId::new();
        let api = Arc::new(MockEnrollmentApi::fresh(expected_user));
        let (backend, _session_store, _registry, _local_store) = backend_fixture(api);
        let backend = Arc::new(backend);
        let mut connection = ScriptedGatewayConnection {
            frames: vec![NativeGatewayFrame::Text(gateway_ready_payload(
                expected_user,
            ))]
            .into(),
        };

        assert_eq!(
            drive_gateway_connection(
                &Arc::downgrade(&backend),
                backend.gateway_lifecycle.current(),
                expected_user,
                100,
                &mut connection,
            ),
            Ok(())
        );
        assert_eq!(connection.frames.len(), 1);
    }

    #[test]
    fn native_gateway_session_replacement_interrupts_the_old_connection() {
        let expected_user = UserId::new();
        let api = Arc::new(MockEnrollmentApi::fresh(expected_user));
        let (backend, _session_store, _registry, _local_store) = backend_fixture(api);
        let backend = Arc::new(backend);
        let revision = backend.gateway_lifecycle.current();
        let weak = Arc::downgrade(&backend);
        let reads = Arc::new(AtomicUsize::new(0));
        let connection_reads = Arc::clone(&reads);
        let join = thread::spawn(move || {
            let mut connection = CountingTimeoutGatewayConnection {
                reads: connection_reads,
            };
            drive_gateway_connection(&weak, revision, expected_user, 500, &mut connection)
        });
        while reads.load(Ordering::SeqCst) == 0 {
            thread::yield_now();
        }

        backend.store_session(valid_session()).unwrap();
        assert_eq!(join.join().unwrap(), Ok(()));
    }

    #[test]
    fn native_gateway_logout_interrupts_the_old_connection() {
        let expected_user = UserId::new();
        let api = Arc::new(MockEnrollmentApi::fresh(expected_user));
        let (backend, _session_store, _registry, _local_store) = backend_fixture(api);
        let backend = Arc::new(backend);
        let revision = backend.gateway_lifecycle.current();
        let weak = Arc::downgrade(&backend);
        let reads = Arc::new(AtomicUsize::new(0));
        let connection_reads = Arc::clone(&reads);
        let join = thread::spawn(move || {
            let mut connection = CountingTimeoutGatewayConnection {
                reads: connection_reads,
            };
            drive_gateway_connection(&weak, revision, expected_user, 500, &mut connection)
        });
        while reads.load(Ordering::SeqCst) == 0 {
            thread::yield_now();
        }

        backend.clear_session().unwrap();
        assert_eq!(join.join().unwrap(), Ok(()));
    }

    #[test]
    fn production_backend_enrolls_fresh_device_and_exposes_public_settings() {
        let user_id = UserId::new();
        let api = Arc::new(MockEnrollmentApi::fresh(user_id));
        let (backend, _session_store, registry, local_store) = backend_fixture(api.clone());

        assert_eq!(
            backend.store_session(valid_session()),
            Ok(SessionMetadata {
                stored: true,
                expires_at_unix: Some(500),
            })
        );
        assert_eq!(
            backend.read_session_metadata(),
            Ok(SessionMetadata {
                stored: true,
                expires_at_unix: Some(500),
            })
        );
        assert_eq!(backend.initialize_e2ee_store(), Ok(store_status()));
        assert_eq!(backend.read_e2ee_store_status(), Ok(store_status()));
        assert_eq!(api.uploaded.load(Ordering::SeqCst), DEFAULT_BATCH_SIZE + 1);
        assert_eq!(
            local_store.exists(&StoreKey::pending_keypackage_upload()),
            Ok(false)
        );
        let device_id = registry.device_for(user_id).unwrap().unwrap();
        let restored = load_mls_client_state(local_store.as_ref()).unwrap();
        assert_eq!(restored.device.user_id(), user_id);
        assert_eq!(restored.device.device_id(), device_id);

        let settings = backend.read_encryption_settings().unwrap();
        assert!(settings.ready);
        assert_eq!(settings.devices.len(), 1);
        assert!(settings.devices[0].is_current_device());

        assert_eq!(backend.clear_session(), Ok(()));
        assert_eq!(
            backend.read_session_metadata(),
            Ok(SessionMetadata {
                stored: false,
                expires_at_unix: None,
            })
        );
        assert_eq!(
            backend.read_e2ee_store_status(),
            Err(DesktopCommandBackendError::Unavailable)
        );
        assert_eq!(
            backend.initialize_e2ee_store(),
            Err(DesktopCommandBackendError::Rejected)
        );
    }

    #[test]
    fn production_conversation_provision_retries_exact_request_after_response_loss() {
        let alice_root = RootIdentityKey::generate();
        let bob_root = RootIdentityKey::generate();
        let alice = MlsDevice::generate(UserId::new(), DeviceId::new(), &alice_root).unwrap();
        let bob = MlsDevice::generate(UserId::new(), DeviceId::new(), &bob_root).unwrap();
        let bob_pin = filament_e2ee::PinnedUserIdentity::new(bob.user_id(), *bob.root_key_public());
        let claimed = filament_protocol::ClaimKeyPackageResponse {
            device_id: bob.device_id().to_string(),
            key_package_blob: generate_key_package_batch(&bob, 1).unwrap().remove(0).blob,
            is_last_resort: false,
        };
        let api = Arc::new(MockEnrollmentApi::with_device(&alice));
        api.lose_provision_response.store(true, Ordering::SeqCst);
        let (backend, _session_store, registry, local_store) = backend_fixture(api.clone());
        backend.store_session(valid_session()).unwrap();
        registry.bind(alice.user_id(), alice.device_id()).unwrap();
        filament_e2ee::persist_root_identity(
            local_store.as_ref(),
            StoreKey::root_identity(),
            &alice_root,
        )
        .unwrap();
        filament_e2ee::persist_mls_client_state(local_store.as_ref(), &alice, &[]).unwrap();
        let conversation_id = filament_core::ConversationId::new();
        let group_id = filament_core::GroupId::new();
        let runtime = DurableMlsClient::load(local_store.as_ref()).unwrap();
        let request = runtime
            .prepare_direct_message_provision(
                local_store.as_ref(),
                conversation_id,
                group_id,
                bob_pin,
                &claimed,
            )
            .unwrap();

        assert_eq!(
            backend.initialize_e2ee_store(),
            Err(DesktopCommandBackendError::Unavailable)
        );
        assert_eq!(api.provision_attempts.load(Ordering::SeqCst), 1);
        let mut restarted = DurableMlsClient::load(local_store.as_ref()).unwrap();
        assert_eq!(
            restarted
                .pending_conversation_provision(local_store.as_ref())
                .unwrap(),
            Some(request.clone())
        );

        assert_eq!(backend.initialize_e2ee_store(), Ok(store_status()));
        assert_eq!(api.provision_attempts.load(Ordering::SeqCst), 2);
        let mut restored = DurableMlsClient::load(local_store.as_ref()).unwrap();
        assert_eq!(
            restored
                .pending_conversation_provision(local_store.as_ref())
                .unwrap(),
            None
        );
        assert_eq!(restored.mailbox_routes().unwrap().len(), 1);
        assert_eq!(
            api.accepted_provisions
                .lock()
                .unwrap()
                .get(&conversation_id)
                .unwrap()
                .0,
            request
        );
    }

    #[test]
    fn production_mailbox_acknowledgment_retries_from_durable_outbox_after_restart() {
        let alice_root = RootIdentityKey::generate();
        let bob_root = RootIdentityKey::generate();
        let alice = MlsDevice::generate(UserId::new(), DeviceId::new(), &alice_root).unwrap();
        let bob = MlsDevice::generate(UserId::new(), DeviceId::new(), &bob_root).unwrap();
        let alice_pin =
            filament_e2ee::PinnedUserIdentity::new(alice.user_id(), *alice.root_key_public());
        let bob_pin = filament_e2ee::PinnedUserIdentity::new(bob.user_id(), *bob.root_key_public());
        let key_package = generate_key_package_batch(&bob, 1).unwrap().remove(0).blob;
        let group_id = filament_core::GroupId::new();
        let (mut alice_group, pending) = filament_e2ee::MlsConversation::create_two_member(
            group_id,
            &alice,
            bob_pin,
            &key_package,
        )
        .unwrap();
        alice_group.accept_pending_commit(&alice).unwrap();
        let bob_group = filament_e2ee::MlsConversation::join_from_welcome(
            group_id,
            &bob,
            alice_pin,
            pending.welcome_blob.as_deref().unwrap(),
        )
        .unwrap();
        let encrypted = alice_group
            .encrypt_application_message(&alice, b"restart-safe mailbox")
            .unwrap();
        let message_id = filament_core::GroupId::new().to_string();
        let api = Arc::new(MockEnrollmentApi::with_device(&bob));
        api.message_mailboxes.lock().unwrap().insert(
            group_id,
            E2eeMailboxResponse {
                messages: vec![filament_protocol::E2eeMailboxMessage {
                    message_id: message_id.clone(),
                    crypto: encrypted.crypto.as_str().to_owned(),
                    epoch: encrypted.epoch,
                    suite_id: encrypted.suite.as_u16(),
                    sender_device_id: encrypted.sender_device_id.to_string(),
                    message_blob: encrypted.message_blob,
                    created_at_unix: 10,
                    expires_at_unix: 1_000,
                }],
                next_after_message_id: Some(message_id.clone()),
            },
        );
        api.lose_message_ack_response.store(true, Ordering::SeqCst);
        let (backend, _session_store, registry, local_store) = backend_fixture(api.clone());
        backend.store_session(valid_session()).unwrap();
        registry.bind(bob.user_id(), bob.device_id()).unwrap();
        filament_e2ee::persist_root_identity(
            local_store.as_ref(),
            StoreKey::root_identity(),
            &bob_root,
        )
        .unwrap();
        filament_e2ee::persist_mls_client_state(local_store.as_ref(), &bob, &[&bob_group]).unwrap();

        assert_eq!(
            backend.initialize_e2ee_store(),
            Err(DesktopCommandBackendError::Unavailable)
        );
        assert!(
            pending_message_acknowledgment(local_store.as_ref(), group_id, bob.device_id())
                .unwrap()
                .is_some()
        );
        assert_eq!(
            filament_e2ee::load_stored_message_at(
                local_store.as_ref(),
                group_id,
                &message_id,
                100,
            )
            .unwrap()
            .message
            .plaintext,
            b"restart-safe mailbox"
        );

        assert_eq!(backend.initialize_e2ee_store(), Ok(store_status()));
        assert_eq!(api.message_ack_attempts.load(Ordering::SeqCst), 2);
        assert!(
            pending_message_acknowledgment(local_store.as_ref(), group_id, bob.device_id())
                .unwrap()
                .is_none()
        );
        assert!(api
            .message_mailboxes
            .lock()
            .unwrap()
            .get(&group_id)
            .unwrap()
            .messages
            .is_empty());
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn native_gateway_message_wake_drains_the_durable_mailbox_without_poll_delay() {
        let alice_root = RootIdentityKey::generate();
        let bob_root = RootIdentityKey::generate();
        let alice = MlsDevice::generate(UserId::new(), DeviceId::new(), &alice_root).unwrap();
        let bob = MlsDevice::generate(UserId::new(), DeviceId::new(), &bob_root).unwrap();
        let alice_pin =
            filament_e2ee::PinnedUserIdentity::new(alice.user_id(), *alice.root_key_public());
        let bob_pin = filament_e2ee::PinnedUserIdentity::new(bob.user_id(), *bob.root_key_public());
        let key_package = generate_key_package_batch(&bob, 1).unwrap().remove(0).blob;
        let group_id = filament_core::GroupId::new();
        let conversation_id = filament_core::ConversationId::new();
        let (mut alice_group, pending) = filament_e2ee::MlsConversation::create_two_member(
            group_id,
            &alice,
            bob_pin,
            &key_package,
        )
        .unwrap();
        alice_group.accept_pending_commit(&alice).unwrap();
        let bob_group = filament_e2ee::MlsConversation::join_from_welcome(
            group_id,
            &bob,
            alice_pin,
            pending.welcome_blob.as_deref().unwrap(),
        )
        .unwrap();
        let encrypted = alice_group
            .encrypt_application_message(&alice, b"gateway wake")
            .unwrap();
        let message_id = filament_e2ee::EncryptedMessageId::new().to_string();
        let ready = serde_json::to_vec(&Envelope {
            v: PROTOCOL_VERSION,
            t: EventType::try_from(String::from("ready")).unwrap(),
            d: serde_json::json!({ "user_id": bob.user_id().to_string() }),
        })
        .unwrap();
        let wake = serde_json::to_vec(&Envelope {
            v: PROTOCOL_VERSION,
            t: EventType::try_from(String::from("mls_message")).unwrap(),
            d: MlsMessageEvent {
                group_id: group_id.to_string(),
                conversation_id: conversation_id.to_string(),
                message_id: message_id.clone(),
                epoch: encrypted.epoch,
                suite_id: encrypted.suite.as_u16(),
                sender_device_id: alice.device_id().to_string(),
                created_at_unix: 10,
            },
        })
        .unwrap();
        let gateway = Arc::new(ScriptedGatewayConnector::new(vec![
            NativeGatewayFrame::Text(ready),
            NativeGatewayFrame::Text(wake),
        ]));
        let api = Arc::new(MockEnrollmentApi::with_device(&bob));
        let (backend, _session_store, registry, local_store) =
            backend_fixture_with_gateway(api.clone(), gateway);
        backend.store_session(valid_session()).unwrap();
        registry.bind(bob.user_id(), bob.device_id()).unwrap();
        filament_e2ee::persist_root_identity(
            local_store.as_ref(),
            StoreKey::root_identity(),
            &bob_root,
        )
        .unwrap();
        filament_e2ee::persist_mls_client_state(local_store.as_ref(), &bob, &[&bob_group]).unwrap();
        assert_eq!(backend.initialize_e2ee_store(), Ok(store_status()));

        api.message_mailboxes.lock().unwrap().insert(
            group_id,
            E2eeMailboxResponse {
                messages: vec![filament_protocol::E2eeMailboxMessage {
                    message_id: message_id.clone(),
                    crypto: encrypted.crypto.as_str().to_owned(),
                    epoch: encrypted.epoch,
                    suite_id: encrypted.suite.as_u16(),
                    sender_device_id: encrypted.sender_device_id.to_string(),
                    message_blob: encrypted.message_blob,
                    created_at_unix: 10,
                    expires_at_unix: 1_000,
                }],
                next_after_message_id: Some(message_id.clone()),
            },
        );
        let backend = Arc::new(backend);
        spawn_native_sync(&backend).unwrap();

        let deadline = Instant::now() + Duration::from_secs(2);
        let stored = loop {
            if let Ok(stored) = filament_e2ee::load_stored_message_at(
                local_store.as_ref(),
                group_id,
                &message_id,
                100,
            ) {
                break stored;
            }
            assert!(
                Instant::now() < deadline,
                "gateway wake did not drain before the polling fallback"
            );
            thread::sleep(Duration::from_millis(10));
        };
        assert_eq!(stored.message.plaintext, b"gateway wake");
        assert_eq!(api.message_ack_attempts.load(Ordering::SeqCst), 1);
        assert!(api
            .message_mailboxes
            .lock()
            .unwrap()
            .get(&group_id)
            .unwrap()
            .messages
            .is_empty());
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn production_attachment_download_is_verified_and_acknowledged_after_durable_storage() {
        let alice_root = RootIdentityKey::generate();
        let bob_root = RootIdentityKey::generate();
        let alice = MlsDevice::generate(UserId::new(), DeviceId::new(), &alice_root).unwrap();
        let bob = MlsDevice::generate(UserId::new(), DeviceId::new(), &bob_root).unwrap();
        let alice_pin =
            filament_e2ee::PinnedUserIdentity::new(alice.user_id(), *alice.root_key_public());
        let bob_pin = filament_e2ee::PinnedUserIdentity::new(bob.user_id(), *bob.root_key_public());
        let key_package = generate_key_package_batch(&bob, 1).unwrap().remove(0).blob;
        let group_id = filament_core::GroupId::new();
        let (mut alice_group, pending_commit) = filament_e2ee::MlsConversation::create_two_member(
            group_id,
            &alice,
            bob_pin,
            &key_package,
        )
        .unwrap();
        alice_group.accept_pending_commit(&alice).unwrap();
        let bob_group = filament_e2ee::MlsConversation::join_from_welcome(
            group_id,
            &bob,
            alice_pin,
            pending_commit.welcome_blob.as_deref().unwrap(),
        )
        .unwrap();
        let plaintext = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDRnative attachment";
        let (descriptor, attachment) =
            filament_e2ee::encrypt_attachment("proof.png", plaintext).unwrap();
        let event = filament_e2ee::VersionedApplicationEvent {
            event_id: filament_e2ee::ApplicationEventId::new(),
            retention_secs: None,
            event: filament_e2ee::EncryptedChatEvent::Attachments {
                message_id: filament_e2ee::EncryptedMessageId::new(),
                body: None,
                attachments: filament_e2ee::AttachmentSet::try_from(vec![
                    filament_e2ee::EncryptedAttachmentReference {
                        file: descriptor.clone(),
                        thumbnail: None,
                    },
                ])
                .unwrap(),
            },
        };
        let encrypted_message = alice_group.encrypt_chat_event(&alice, &event).unwrap();
        let message_id = filament_e2ee::EncryptedMessageId::new().to_string();
        let api = Arc::new(MockEnrollmentApi::with_device(&bob));
        api.message_mailboxes.lock().unwrap().insert(
            group_id,
            E2eeMailboxResponse {
                messages: vec![filament_protocol::E2eeMailboxMessage {
                    message_id: message_id.clone(),
                    crypto: encrypted_message.crypto.as_str().to_owned(),
                    epoch: encrypted_message.epoch,
                    suite_id: encrypted_message.suite.as_u16(),
                    sender_device_id: encrypted_message.sender_device_id.to_string(),
                    message_blob: encrypted_message.message_blob,
                    created_at_unix: 10,
                    expires_at_unix: 1_000,
                }],
                next_after_message_id: Some(message_id.clone()),
            },
        );
        api.attachments
            .lock()
            .unwrap()
            .insert((group_id, attachment.attachment_id), attachment);
        api.lose_attachment_ack_response
            .store(true, Ordering::SeqCst);
        let (backend, _session_store, registry, local_store) = backend_fixture(api.clone());
        backend.store_session(valid_session()).unwrap();
        registry.bind(bob.user_id(), bob.device_id()).unwrap();
        filament_e2ee::persist_root_identity(
            local_store.as_ref(),
            StoreKey::root_identity(),
            &bob_root,
        )
        .unwrap();
        filament_e2ee::persist_mls_client_state(local_store.as_ref(), &bob, &[&bob_group]).unwrap();

        assert_eq!(
            backend.initialize_e2ee_store(),
            Err(DesktopCommandBackendError::Unavailable)
        );
        let acknowledgment =
            pending_attachment_acknowledgment(local_store.as_ref(), group_id, bob.device_id())
                .unwrap()
                .unwrap();
        assert_eq!(
            acknowledgment.attachment_ids,
            vec![descriptor.attachment_id.to_string()]
        );
        let loaded = filament_e2ee::load_downloaded_attachment(
            local_store.as_ref(),
            group_id,
            &message_id,
            &descriptor,
            100,
        )
        .unwrap()
        .unwrap();
        assert_eq!(loaded.bytes.as_slice(), plaintext);

        assert_eq!(backend.initialize_e2ee_store(), Ok(store_status()));
        assert_eq!(api.attachment_ack_attempts.load(Ordering::SeqCst), 2);
        assert!(
            pending_attachment_acknowledgment(local_store.as_ref(), group_id, bob.device_id())
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn production_outbound_message_retries_exact_ciphertext_after_response_loss() {
        let alice_root = RootIdentityKey::generate();
        let bob_root = RootIdentityKey::generate();
        let alice = MlsDevice::generate(UserId::new(), DeviceId::new(), &alice_root).unwrap();
        let bob = MlsDevice::generate(UserId::new(), DeviceId::new(), &bob_root).unwrap();
        let bob_pin = filament_e2ee::PinnedUserIdentity::new(bob.user_id(), *bob.root_key_public());
        let key_package = generate_key_package_batch(&bob, 1).unwrap().remove(0).blob;
        let group_id = filament_core::GroupId::new();
        let (mut alice_group, _) = filament_e2ee::MlsConversation::create_two_member(
            group_id,
            &alice,
            bob_pin,
            &key_package,
        )
        .unwrap();
        alice_group.accept_pending_commit(&alice).unwrap();

        let api = Arc::new(MockEnrollmentApi::with_device(&alice));
        let (backend, _session_store, registry, local_store) = backend_fixture(api.clone());
        backend.store_session(valid_session()).unwrap();
        registry.bind(alice.user_id(), alice.device_id()).unwrap();
        filament_e2ee::persist_root_identity(
            local_store.as_ref(),
            StoreKey::root_identity(),
            &alice_root,
        )
        .unwrap();
        filament_e2ee::persist_mls_client_state(local_store.as_ref(), &alice, &[&alice_group])
            .unwrap();
        assert_eq!(backend.initialize_e2ee_store(), Ok(store_status()));
        let event = filament_e2ee::VersionedApplicationEvent {
            event_id: filament_e2ee::ApplicationEventId::new(),
            retention_secs: None,
            event: filament_e2ee::EncryptedChatEvent::Message {
                message_id: filament_e2ee::EncryptedMessageId::new(),
                body: filament_e2ee::ChatMessageBody::try_from(String::from("native retry"))
                    .unwrap(),
                reply: None,
            },
        };
        let request = {
            let mut active = backend.active.lock().unwrap();
            active
                .as_mut()
                .unwrap()
                .mailbox
                .prepare_outbound_message(local_store.as_ref(), group_id, &event)
                .unwrap()
        };

        api.lose_message_response.store(true, Ordering::SeqCst);
        assert_eq!(
            backend.synchronize_active_once(),
            Err(DesktopCommandBackendError::Unavailable)
        );
        assert_eq!(api.message_attempts.load(Ordering::SeqCst), 1);
        assert_eq!(
            DurableMlsClient::load(local_store.as_ref())
                .unwrap()
                .pending_outbound_message(local_store.as_ref(), group_id)
                .unwrap(),
            Some(request)
        );

        assert_eq!(
            backend.synchronize_active_once(),
            Ok(BackgroundSyncOutcome::Synchronized)
        );
        assert_eq!(api.message_attempts.load(Ordering::SeqCst), 2);
        let response = api
            .accepted_messages
            .lock()
            .unwrap()
            .get(&group_id)
            .unwrap()
            .1
            .clone();
        assert_eq!(
            DurableMlsClient::load(local_store.as_ref())
                .unwrap()
                .pending_outbound_message(local_store.as_ref(), group_id)
                .unwrap(),
            None
        );
        let stored = filament_e2ee::load_stored_message_at(
            local_store.as_ref(),
            group_id,
            &response.message_id,
            response.created_at_unix,
        )
        .unwrap();
        assert_eq!(
            filament_e2ee::VersionedApplicationEvent::decode(&stored.message.plaintext).unwrap(),
            event
        );
    }

    fn assert_authenticated_attachment_message(
        store: &dyn LocalKeyStore,
        api: &MockEnrollmentApi,
        group_id: filament_core::GroupId,
        descriptor: &filament_e2ee::AttachmentDescriptor,
    ) {
        let response = api
            .accepted_messages
            .lock()
            .unwrap()
            .get(&group_id)
            .unwrap()
            .1
            .clone();
        let stored = filament_e2ee::load_stored_message_at(
            store,
            group_id,
            &response.message_id,
            response.created_at_unix,
        )
        .unwrap();
        let event =
            filament_e2ee::VersionedApplicationEvent::decode(&stored.message.plaintext).unwrap();
        let filament_e2ee::EncryptedChatEvent::Attachments { attachments, .. } = event.event else {
            panic!("confirmed upload must become an authenticated attachment event");
        };
        assert_eq!(attachments.as_slice().len(), 1);
        assert_eq!(&attachments.as_slice()[0].file, descriptor);
        assert!(store
            .list_keys()
            .unwrap()
            .iter()
            .all(|key| !key.as_str().starts_with("attachment-upload:")));
    }

    fn assert_exact_attachment_upload_pending(
        store: &dyn LocalKeyStore,
        api: &MockEnrollmentApi,
        group_id: filament_core::GroupId,
        device_id: DeviceId,
        descriptor: &filament_e2ee::AttachmentDescriptor,
        exact: &EncryptedAttachment,
    ) {
        assert_eq!(
            filament_e2ee::pending_attachment_upload(store, group_id, device_id).unwrap(),
            Some(exact.clone())
        );
        assert_eq!(
            api.attachments
                .lock()
                .unwrap()
                .get(&(group_id, descriptor.attachment_id))
                .unwrap(),
            exact
        );
    }

    #[test]
    fn production_attachment_send_retries_upload_and_message_after_response_loss() {
        let alice_root = RootIdentityKey::generate();
        let bob_root = RootIdentityKey::generate();
        let alice = MlsDevice::generate(UserId::new(), DeviceId::new(), &alice_root).unwrap();
        let bob = MlsDevice::generate(UserId::new(), DeviceId::new(), &bob_root).unwrap();
        let bob_pin = filament_e2ee::PinnedUserIdentity::new(bob.user_id(), *bob.root_key_public());
        let key_package = generate_key_package_batch(&bob, 1).unwrap().remove(0).blob;
        let group_id = filament_core::GroupId::new();
        let (mut alice_group, _) = filament_e2ee::MlsConversation::create_two_member(
            group_id,
            &alice,
            bob_pin,
            &key_package,
        )
        .unwrap();
        alice_group.accept_pending_commit(&alice).unwrap();

        let api = Arc::new(MockEnrollmentApi::with_device(&alice));
        api.lose_attachment_upload_response
            .store(true, Ordering::SeqCst);
        let (backend, _session_store, registry, local_store) = backend_fixture(api.clone());
        backend.store_session(valid_session()).unwrap();
        registry.bind(alice.user_id(), alice.device_id()).unwrap();
        filament_e2ee::persist_root_identity(
            local_store.as_ref(),
            StoreKey::root_identity(),
            &alice_root,
        )
        .unwrap();
        filament_e2ee::persist_mls_client_state(local_store.as_ref(), &alice, &[&alice_group])
            .unwrap();
        let descriptor = filament_e2ee::prepare_attachment_upload(
            local_store.as_ref(),
            group_id,
            alice.device_id(),
            "proof.png",
            b"\x89PNG\r\n\x1a\n\0\0\0\rIHDRnative outbound attachment",
        )
        .unwrap();
        let exact = filament_e2ee::pending_attachment_upload(
            local_store.as_ref(),
            group_id,
            alice.device_id(),
        )
        .unwrap()
        .unwrap();

        assert_eq!(
            backend.initialize_e2ee_store(),
            Err(DesktopCommandBackendError::Unavailable)
        );
        assert_eq!(api.attachment_upload_attempts.load(Ordering::SeqCst), 1);
        assert_exact_attachment_upload_pending(
            local_store.as_ref(),
            api.as_ref(),
            group_id,
            alice.device_id(),
            &descriptor,
            &exact,
        );

        api.lose_message_response.store(true, Ordering::SeqCst);
        assert_eq!(
            backend.initialize_e2ee_store(),
            Err(DesktopCommandBackendError::Unavailable)
        );
        assert_eq!(api.attachment_upload_attempts.load(Ordering::SeqCst), 2);
        assert!(filament_e2ee::confirmed_attachment_upload(
            local_store.as_ref(),
            group_id,
            alice.device_id(),
            100,
        )
        .unwrap()
        .is_some());
        assert_eq!(api.message_attempts.load(Ordering::SeqCst), 1);
        assert!(DurableMlsClient::load(local_store.as_ref())
            .unwrap()
            .pending_outbound_message(local_store.as_ref(), group_id)
            .unwrap()
            .is_some());

        assert_eq!(backend.initialize_e2ee_store(), Ok(store_status()));
        assert_eq!(api.attachment_upload_attempts.load(Ordering::SeqCst), 2);
        assert_eq!(api.message_attempts.load(Ordering::SeqCst), 2);
        assert!(filament_e2ee::confirmed_attachment_upload(
            local_store.as_ref(),
            group_id,
            alice.device_id(),
            100,
        )
        .unwrap()
        .is_none());
        assert_authenticated_attachment_message(
            local_store.as_ref(),
            api.as_ref(),
            group_id,
            &descriptor,
        );
    }

    #[test]
    fn production_proposal_commit_retries_before_acknowledging_after_response_loss() {
        let alice_root = RootIdentityKey::generate();
        let bob_root = RootIdentityKey::generate();
        let charlie_root = RootIdentityKey::generate();
        let alice = MlsDevice::generate(UserId::new(), DeviceId::new(), &alice_root).unwrap();
        let bob = MlsDevice::generate(UserId::new(), DeviceId::new(), &bob_root).unwrap();
        let charlie = MlsDevice::generate(UserId::new(), DeviceId::new(), &charlie_root).unwrap();
        let bob_pin = filament_e2ee::PinnedUserIdentity::new(bob.user_id(), *bob.root_key_public());
        let charlie_pin =
            filament_e2ee::PinnedUserIdentity::new(charlie.user_id(), *charlie.root_key_public());
        let bob_package = generate_key_package_batch(&bob, 1).unwrap().remove(0).blob;
        let charlie_package = generate_key_package_batch(&charlie, 1)
            .unwrap()
            .remove(0)
            .blob;
        let delivery =
            DeliveryServiceSigner::from_seed([0x51; DELIVERY_SERVICE_SEED_BYTES]).unwrap();
        let group_id = filament_core::GroupId::new();
        let (mut group, _) = filament_e2ee::MlsConversation::create_group_with_delivery_service(
            group_id,
            &alice,
            &[(bob_pin, bob_package), (charlie_pin, charlie_package)],
            delivery.identity(),
        )
        .unwrap();
        group.accept_pending_commit(&alice).unwrap();
        let proposal = delivery.sign_remove(group_id, group.epoch(), 2).unwrap();
        let proposal_id = filament_core::ProposalId::new().to_string();

        let api = Arc::new(MockEnrollmentApi::with_device(&alice));
        api.proposal_mailboxes.lock().unwrap().insert(
            group_id,
            E2eeProposalMailboxResponse {
                proposals: vec![E2eeProposalMailboxEntry {
                    proposal_id: proposal_id.clone(),
                    epoch: proposal.epoch,
                    proposer_device_id: None,
                    external_sender_index: Some(0),
                    proposal_blob: proposal.proposal_blob,
                    created_at_unix: 10,
                    expires_at_unix: 1_000,
                    reconciliation_deadline_unix: Some(500),
                }],
                next_after_proposal_id: Some(proposal_id),
            },
        );
        api.lose_commit_response.store(true, Ordering::SeqCst);
        let (backend, _session_store, registry, local_store) = backend_fixture(api.clone());
        backend.store_session(valid_session()).unwrap();
        registry.bind(alice.user_id(), alice.device_id()).unwrap();
        filament_e2ee::persist_root_identity(
            local_store.as_ref(),
            StoreKey::root_identity(),
            &alice_root,
        )
        .unwrap();
        filament_e2ee::persist_mls_client_state(local_store.as_ref(), &alice, &[&group]).unwrap();

        assert_eq!(
            backend.initialize_e2ee_store(),
            Err(DesktopCommandBackendError::Unavailable)
        );
        let mut restarted = DurableMlsClient::load(local_store.as_ref()).unwrap();
        assert!(restarted
            .pending_outbound_commit(local_store.as_ref(), group_id)
            .unwrap()
            .is_some());
        assert!(
            pending_proposal_acknowledgment(local_store.as_ref(), group_id, alice.device_id())
                .unwrap()
                .is_some()
        );
        assert_eq!(api.proposal_ack_attempts.load(Ordering::SeqCst), 0);

        assert_eq!(backend.initialize_e2ee_store(), Ok(store_status()));
        assert_eq!(api.commit_attempts.load(Ordering::SeqCst), 2);
        assert_eq!(api.proposal_ack_attempts.load(Ordering::SeqCst), 1);
        assert!(
            pending_proposal_acknowledgment(local_store.as_ref(), group_id, alice.device_id())
                .unwrap()
                .is_none()
        );
        assert!(DurableMlsClient::load(local_store.as_ref())
            .unwrap()
            .pending_outbound_commit(local_store.as_ref(), group_id)
            .unwrap()
            .is_none());
        let restored = load_mls_client_state(local_store.as_ref()).unwrap();
        assert_eq!(restored.conversations[0].epoch(), 2);
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn production_proposal_commit_authenticates_winner_and_rebases_epoch_conflict() {
        let alice_root = RootIdentityKey::generate();
        let bob_root = RootIdentityKey::generate();
        let charlie_root = RootIdentityKey::generate();
        let alice = MlsDevice::generate(UserId::new(), DeviceId::new(), &alice_root).unwrap();
        let bob = MlsDevice::generate(UserId::new(), DeviceId::new(), &bob_root).unwrap();
        let charlie = MlsDevice::generate(UserId::new(), DeviceId::new(), &charlie_root).unwrap();
        let alice_pin =
            filament_e2ee::PinnedUserIdentity::new(alice.user_id(), *alice.root_key_public());
        let bob_pin = filament_e2ee::PinnedUserIdentity::new(bob.user_id(), *bob.root_key_public());
        let charlie_pin =
            filament_e2ee::PinnedUserIdentity::new(charlie.user_id(), *charlie.root_key_public());
        let bob_package = generate_key_package_batch(&bob, 1).unwrap().remove(0).blob;
        let charlie_package = generate_key_package_batch(&charlie, 1)
            .unwrap()
            .remove(0)
            .blob;
        let delivery =
            DeliveryServiceSigner::from_seed([0x52; DELIVERY_SERVICE_SEED_BYTES]).unwrap();
        let group_id = filament_core::GroupId::new();
        let (mut group, initial) =
            filament_e2ee::MlsConversation::create_group_with_delivery_service(
                group_id,
                &alice,
                &[(bob_pin, bob_package), (charlie_pin, charlie_package)],
                delivery.identity(),
            )
            .unwrap();
        group.accept_pending_commit(&alice).unwrap();
        let mut bob_group =
            filament_e2ee::MlsConversation::join_group_from_welcome_with_delivery_service(
                group_id,
                &bob,
                &[alice_pin, charlie_pin],
                initial.welcome_blob.as_deref().unwrap(),
                delivery.identity(),
            )
            .unwrap();
        let proposal = delivery.sign_remove(group_id, group.epoch(), 2).unwrap();
        let proposal_id = filament_core::ProposalId::new().to_string();

        let winner = bob_group.create_self_update(&bob).unwrap();
        bob_group.accept_pending_commit(&bob).unwrap();
        let winner_request = PostCommitRequest {
            epoch: winner.epoch,
            prior_epoch: winner.prior_epoch,
            committer_device_id: winner.committer_device_id.to_string(),
            commit_blob: winner.commit_blob.clone(),
            welcome_blob: None,
            welcome_device_id: None,
            group_info_blob: winner.group_info_blob.clone(),
            membership_change: None,
        };

        let api = Arc::new(MockEnrollmentApi::with_device(&alice));
        api.proposal_mailboxes.lock().unwrap().insert(
            group_id,
            E2eeProposalMailboxResponse {
                proposals: vec![E2eeProposalMailboxEntry {
                    proposal_id: proposal_id.clone(),
                    epoch: proposal.epoch,
                    proposer_device_id: None,
                    external_sender_index: Some(0),
                    proposal_blob: proposal.proposal_blob,
                    created_at_unix: 10,
                    expires_at_unix: 1_000,
                    reconciliation_deadline_unix: Some(500),
                }],
                next_after_proposal_id: Some(proposal_id),
            },
        );
        api.commit_mailboxes.lock().unwrap().insert(
            group_id,
            E2eeCommitMailboxResponse {
                commits: vec![E2eeCommitMailboxEntry {
                    epoch: winner.epoch,
                    prior_epoch: winner.prior_epoch,
                    committer_device_id: winner.committer_device_id.to_string(),
                    commit_blob: winner.commit_blob,
                    welcome_blob: None,
                    membership_change: None,
                    created_at_unix: 11,
                    expires_at_unix: 1_000,
                }],
                next_after_epoch: Some(winner.epoch),
            },
        );
        api.accepted_commits.lock().unwrap().insert(
            group_id,
            (
                winner_request,
                PostCommitResponse {
                    accepted: true,
                    epoch: winner.epoch,
                },
            ),
        );
        api.lose_commit_ack_response.store(true, Ordering::SeqCst);
        let (backend, _session_store, registry, local_store) = backend_fixture(api.clone());
        backend.store_session(valid_session()).unwrap();
        registry.bind(alice.user_id(), alice.device_id()).unwrap();
        filament_e2ee::persist_root_identity(
            local_store.as_ref(),
            StoreKey::root_identity(),
            &alice_root,
        )
        .unwrap();
        filament_e2ee::persist_mls_client_state(local_store.as_ref(), &alice, &[&group]).unwrap();

        assert_eq!(
            backend.initialize_e2ee_store(),
            Err(DesktopCommandBackendError::Unavailable)
        );
        assert_eq!(api.commit_attempts.load(Ordering::SeqCst), 1);
        assert_eq!(api.commit_ack_attempts.load(Ordering::SeqCst), 1);
        assert!(
            pending_commit_acknowledgment(local_store.as_ref(), group_id, alice.device_id())
                .unwrap()
                .is_some()
        );
        assert_eq!(api.proposal_ack_attempts.load(Ordering::SeqCst), 0);

        assert_eq!(backend.initialize_e2ee_store(), Ok(store_status()));
        assert_eq!(api.commit_attempts.load(Ordering::SeqCst), 2);
        assert_eq!(api.commit_ack_attempts.load(Ordering::SeqCst), 2);
        assert_eq!(api.proposal_ack_attempts.load(Ordering::SeqCst), 1);
        let accepted = api.accepted_commits.lock().unwrap();
        let (rebased_request, response) = accepted.get(&group_id).unwrap();
        assert_eq!(rebased_request.prior_epoch, winner.epoch);
        assert_eq!(response.epoch, winner.epoch + 1);
        assert!(matches!(
            rebased_request.membership_change,
            Some(filament_protocol::MlsMembershipChange::Remove { .. })
        ));
        drop(accepted);

        let restored = load_mls_client_state(local_store.as_ref()).unwrap();
        assert_eq!(restored.conversations[0].epoch(), winner.epoch + 1);
        let routes = DurableMlsClient::from_state(restored)
            .mailbox_routes()
            .unwrap();
        assert!(!routes[0]
            .participants
            .iter()
            .any(|participant| participant.user_id == charlie.user_id()));
        assert!(
            pending_proposal_acknowledgment(local_store.as_ref(), group_id, alice.device_id())
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn production_backend_replenishes_only_its_low_keypackage_pool_durably() {
        let user_id = UserId::new();
        let api = Arc::new(MockEnrollmentApi::fresh(user_id));
        let (backend, _session_store, registry, local_store) = backend_fixture(api.clone());
        backend.store_session(valid_session()).unwrap();
        backend.initialize_e2ee_store().unwrap();
        let device_id = registry.device_for(user_id).unwrap().unwrap();

        assert_eq!(
            backend.replenish_active_keypackages_once(KeyPackageLowWake {
                device_id: DeviceId::new(),
                remaining_count: 3,
                water_mark: 10,
            }),
            Ok(BackgroundSyncOutcome::Idle)
        );
        assert_eq!(api.uploaded.load(Ordering::SeqCst), DEFAULT_BATCH_SIZE + 1);

        assert_eq!(
            backend.replenish_active_keypackages_once(KeyPackageLowWake {
                device_id,
                remaining_count: 3,
                water_mark: 10,
            }),
            Ok(BackgroundSyncOutcome::Synchronized)
        );
        assert_eq!(api.uploaded.load(Ordering::SeqCst), 7);
        assert_eq!(
            local_store.exists(&StoreKey::pending_keypackage_upload()),
            Ok(false)
        );
        assert!(load_mls_client_state(local_store.as_ref()).is_ok());

        api.reject_upload.store(true, Ordering::SeqCst);
        assert_eq!(
            backend.replenish_active_keypackages_once(KeyPackageLowWake {
                device_id,
                remaining_count: 9,
                water_mark: 10,
            }),
            Err(DesktopCommandBackendError::Unavailable)
        );
        assert_eq!(
            local_store.exists(&StoreKey::pending_keypackage_upload()),
            Ok(true)
        );

        api.reject_upload.store(false, Ordering::SeqCst);
        assert_eq!(
            backend.synchronize_active_once(),
            Ok(BackgroundSyncOutcome::Synchronized)
        );
        assert_eq!(api.uploaded.load(Ordering::SeqCst), 1);
        assert_eq!(
            local_store.exists(&StoreKey::pending_keypackage_upload()),
            Ok(false)
        );
    }

    #[test]
    fn production_backend_rotates_root_without_exposing_replacement_secrets() {
        let user_id = UserId::new();
        let api = Arc::new(MockEnrollmentApi::fresh(user_id));
        let (backend, _session_store, _registry, local_store) = backend_fixture(api.clone());
        backend.store_session(valid_session()).unwrap();
        backend.initialize_e2ee_store().unwrap();
        let previous_root =
            load_root_identity(local_store.as_ref(), &StoreKey::root_identity()).unwrap();

        let response = backend
            .rotate_root_identity(RotateIdentityCommandRequest {
                confirmation: crate::ROTATE_IDENTITY_CONFIRMATION.to_owned(),
            })
            .unwrap();
        assert_eq!(response.rotation_sequence, 1);
        assert_eq!(
            response.previous_root_key_pub,
            previous_root.public_key_bytes()
        );
        let replacement_root =
            load_root_identity(local_store.as_ref(), &StoreKey::root_identity()).unwrap();
        assert_ne!(
            replacement_root.public_key_bytes(),
            previous_root.public_key_bytes()
        );
        assert_eq!(
            replacement_root.public_key_bytes().to_vec(),
            response.new_root_key_pub
        );
        assert_eq!(
            local_store.exists(&StoreKey::pending_root_identity_rotation()),
            Ok(false)
        );
        assert_eq!(
            load_root_identity_rotation_sequence(local_store.as_ref()),
            Ok(1)
        );
        assert_eq!(api.uploaded.load(Ordering::SeqCst), DEFAULT_BATCH_SIZE + 1);
        assert_eq!(
            backend
                .read_encryption_settings()
                .unwrap()
                .rotation_sequence,
            1
        );
    }

    #[test]
    fn production_backend_reconciles_a_lost_rotation_response_from_durable_state() {
        let user_id = UserId::new();
        let api = Arc::new(MockEnrollmentApi::fresh(user_id));
        let (backend, _session_store, _registry, local_store) = backend_fixture(api.clone());
        backend.store_session(valid_session()).unwrap();
        backend.initialize_e2ee_store().unwrap();
        let previous_root =
            load_root_identity(local_store.as_ref(), &StoreKey::root_identity()).unwrap();
        api.lose_rotation_response.store(true, Ordering::SeqCst);

        let command = RotateIdentityCommandRequest {
            confirmation: crate::ROTATE_IDENTITY_CONFIRMATION.to_owned(),
        };
        assert_eq!(
            backend.rotate_root_identity(command.clone()),
            Err(DesktopCommandBackendError::Unavailable)
        );
        assert_eq!(
            load_root_identity(local_store.as_ref(), &StoreKey::root_identity())
                .unwrap()
                .public_key_bytes(),
            previous_root.public_key_bytes()
        );
        assert_eq!(
            local_store.exists(&StoreKey::pending_root_identity_rotation()),
            Ok(true)
        );

        let response = backend.rotate_root_identity(command).unwrap();
        assert_eq!(response.rotation_sequence, 1);
        assert_eq!(
            local_store.exists(&StoreKey::pending_root_identity_rotation()),
            Ok(false)
        );
        assert_eq!(
            load_root_identity_rotation_sequence(local_store.as_ref()),
            Ok(1)
        );
    }

    #[test]
    fn bootstrap_upload_outbox_survives_uncertain_network_result() {
        let user_id = UserId::new();
        let api = Arc::new(MockEnrollmentApi::fresh(user_id));
        api.reject_upload.store(true, Ordering::SeqCst);
        let (backend, _session_store, registry, local_store) = backend_fixture(api.clone());
        backend.store_session(valid_session()).unwrap();

        assert_eq!(
            backend.initialize_e2ee_store(),
            Err(DesktopCommandBackendError::Unavailable)
        );
        assert_eq!(
            local_store.exists(&StoreKey::pending_keypackage_upload()),
            Ok(true)
        );
        assert!(registry.device_for(user_id).unwrap().is_some());

        api.reject_upload.store(false, Ordering::SeqCst);
        assert_eq!(backend.initialize_e2ee_store(), Ok(store_status()));
        assert_eq!(
            local_store.exists(&StoreKey::pending_keypackage_upload()),
            Ok(false)
        );
        assert_eq!(api.uploaded.load(Ordering::SeqCst), DEFAULT_BATCH_SIZE + 1);
    }

    #[test]
    fn existing_account_without_native_binding_remains_pairing_gated() {
        let root = RootIdentityKey::generate();
        let device = MlsDevice::generate(UserId::new(), DeviceId::new(), &root).unwrap();
        let api = Arc::new(MockEnrollmentApi::with_device(&device));
        let (backend, _session_store, registry, local_store) = backend_fixture(api);
        backend.store_session(valid_session()).unwrap();

        assert_eq!(
            backend.initialize_e2ee_store(),
            Err(DesktopCommandBackendError::Rejected)
        );
        assert_eq!(registry.device_for(device.user_id()), Ok(None));
        assert!(local_store.list_keys().unwrap().is_empty());
        assert_eq!(
            format!("{backend:?}"),
            "ProductionDesktopBackend(<native state redacted>)"
        );
    }
}
