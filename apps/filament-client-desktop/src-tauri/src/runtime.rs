//! Concrete Tauri runtime wiring for the packaged Filament clients.
//!
//! The runtime persists the bounded session in the platform credential store,
//! discovers the authenticated user through a pinned native HTTPS origin, and
//! enrolls only a fresh account as its first certified MLS device. Existing
//! accounts without a native device binding remain pairing-gated.

use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

use filament_core::{DeviceId, UserId};
use filament_e2ee::{
    clear_pending_keypackage_upload, generate_key_package_batch, generate_last_resort_key_package,
    load_mls_client_state, load_pending_keypackage_upload, load_pending_root_identity_rotation,
    load_root_identity, load_root_identity_rotation_sequence, persist_initial_device_bootstrap,
    persist_root_identity_rotation_sequence, prepare_pending_root_identity_rotation, KeyStoreError,
    LocalKeyStore, LocalStoreId, MlsDevice, RootIdentityKey, StoreKey, DEFAULT_BATCH_SIZE,
};
use filament_protocol::RotateRootIdentityResponse;
use tauri::ipc::InvokeBody;
use tauri::Manager as _;

use crate::{
    device_registry::{DeviceRegistry, DeviceRegistryError, OsDeviceRegistry},
    native_api::{
        verify_directory_device, verify_directory_root, verify_root_identity_directory,
        NativeApiError, NativeEnrollmentApi, ReqwestNativeEnrollmentApi,
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

struct ProductionDesktopBackend {
    session_store: Arc<dyn SessionCredentialStore>,
    device_registry: Arc<dyn DeviceRegistry>,
    api: Arc<dyn NativeEnrollmentApi>,
    store_factory: Arc<dyn NativeStoreFactory>,
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
        clock: impl Fn() -> Result<i64, DesktopCommandBackendError> + Send + Sync + 'static,
    ) -> Self {
        Self {
            session_store,
            device_registry,
            api,
            store_factory,
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
        let metadata = self
            .session_store
            .store(&request)
            .map_err(map_session_error)?;
        Ok(session_metadata(Some(metadata)))
    }

    fn clear_session(&self) -> Result<(), DesktopCommandBackendError> {
        let result = self.session_store.clear().map_err(map_session_error);
        *self
            .active
            .lock()
            .map_err(|_| DesktopCommandBackendError::Unavailable)? = None;
        result
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
        *active = Some(ActiveE2eeState {
            user_id,
            device_id,
            store,
        });
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
        let active = self
            .active
            .lock()
            .map_err(|_| DesktopCommandBackendError::Unavailable)?;
        let active = active
            .as_ref()
            .ok_or(DesktopCommandBackendError::Unavailable)?;
        let user_id = self
            .api
            .current_user(&session.access_token)
            .map_err(map_native_api_error)?;
        if user_id != active.user_id {
            return Err(DesktopCommandBackendError::Rejected);
        }
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
        let active = self
            .active
            .lock()
            .map_err(|_| DesktopCommandBackendError::Unavailable)?;
        let active = active
            .as_ref()
            .ok_or(DesktopCommandBackendError::Unavailable)?;
        let user_id = self
            .api
            .current_user(&session.access_token)
            .map_err(map_native_api_error)?;
        if user_id != active.user_id {
            return Err(DesktopCommandBackendError::Rejected);
        }
        if let Some(response) = self.reconcile_pending_rotation(&session, active.store.as_ref())? {
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
        let _ = self.flush_pending_keypackages(&session, active.device_id, active.store.as_ref());
        Ok(response)
    }
}

type Clock = dyn Fn() -> Result<i64, DesktopCommandBackendError> + Send + Sync;

struct ActiveE2eeState {
    user_id: UserId,
    device_id: DeviceId,
    store: Arc<dyn LocalKeyStore>,
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
        NativeApiError::Rejected => DesktopCommandBackendError::Rejected,
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

fn system_time_unix() -> Result<i64, DesktopCommandBackendError> {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| DesktopCommandBackendError::Unavailable)?
        .as_secs();
    i64::try_from(seconds).map_err(|_| DesktopCommandBackendError::Unavailable)
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
            let backend = ProductionDesktopBackend::new(app_data_root)
                .map_err(|_| std::io::Error::other("native backend initialization failed"))?;
            app.manage(RuntimeState {
                host: Arc::new(DesktopCommandHost::new(Arc::new(backend))),
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
    use filament_e2ee::{InMemoryKeyStore, PendingKeyPackageUpload};
    use filament_protocol::{
        DeviceInfo, DeviceListResponse, RootIdentityDirectoryResponse, RootIdentityRotationEntry,
        RotateRootIdentityRequest, ROOT_IDENTITY_ROTATION_PROTOCOL_VERSION,
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
