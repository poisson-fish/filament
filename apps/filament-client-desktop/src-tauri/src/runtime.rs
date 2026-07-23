//! Concrete Tauri runtime wiring for the packaged Filament clients.
//!
//! The runtime persists the bounded session in the platform credential store.
//! E2EE store, settings, rotation, and network coordination remain fail closed
//! until authenticated device enrollment is injected.

use std::sync::Arc;

use filament_protocol::RotateRootIdentityResponse;
use tauri::ipc::InvokeBody;

use crate::{
    session_store::{
        OsSessionCredentialStore, SessionCredentialError, SessionCredentialStore,
        StoredSessionMetadata,
    },
    validate_runtime_navigation, DesktopCommandBackend, DesktopCommandBackendError,
    DesktopCommandError, DesktopCommandHost, E2eeStoreStatus, EncryptionSettingsSnapshot,
    RotateIdentityCommandRequest, SessionMetadata, StoreSessionRequest,
    ValidatedStoreSessionRequest,
};

/// Maximum serialized request body accepted at the native IPC boundary.
pub const MAX_TAURI_IPC_REQUEST_BYTES: usize = 16 * 1024;

struct ProductionDesktopBackend {
    session_store: Arc<dyn SessionCredentialStore>,
}

impl core::fmt::Debug for ProductionDesktopBackend {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("ProductionDesktopBackend(<native state redacted>)")
    }
}

impl Default for ProductionDesktopBackend {
    fn default() -> Self {
        Self {
            session_store: Arc::new(OsSessionCredentialStore),
        }
    }
}

impl ProductionDesktopBackend {
    #[cfg(test)]
    fn with_session_store(session_store: Arc<dyn SessionCredentialStore>) -> Self {
        Self { session_store }
    }
}

impl DesktopCommandBackend for ProductionDesktopBackend {
    fn store_session(
        &self,
        request: ValidatedStoreSessionRequest,
    ) -> Result<SessionMetadata, DesktopCommandBackendError> {
        let metadata = self
            .session_store
            .store(&request)
            .map_err(map_session_error)?;
        Ok(session_metadata(Some(metadata)))
    }

    fn clear_session(&self) -> Result<(), DesktopCommandBackendError> {
        self.session_store.clear().map_err(map_session_error)
    }

    fn read_session_metadata(&self) -> Result<SessionMetadata, DesktopCommandBackendError> {
        self.session_store
            .metadata()
            .map(session_metadata)
            .map_err(map_session_error)
    }

    fn initialize_e2ee_store(&self) -> Result<E2eeStoreStatus, DesktopCommandBackendError> {
        Err(DesktopCommandBackendError::Unavailable)
    }

    fn read_e2ee_store_status(&self) -> Result<E2eeStoreStatus, DesktopCommandBackendError> {
        Err(DesktopCommandBackendError::Unavailable)
    }

    fn read_encryption_settings(
        &self,
    ) -> Result<EncryptionSettingsSnapshot, DesktopCommandBackendError> {
        Err(DesktopCommandBackendError::Unavailable)
    }

    fn rotate_root_identity(
        &self,
        _request: RotateIdentityCommandRequest,
    ) -> Result<RotateRootIdentityResponse, DesktopCommandBackendError> {
        Err(DesktopCommandBackendError::Unavailable)
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
    let host = Arc::new(DesktopCommandHost::new(Arc::new(
        ProductionDesktopBackend::default(),
    )));

    tauri::Builder::default()
        .manage(RuntimeState { host })
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
    use std::sync::Mutex;

    use super::*;

    #[derive(Default)]
    struct MemorySessionStore {
        expiry: Mutex<Option<i64>>,
    }

    impl SessionCredentialStore for MemorySessionStore {
        fn store(
            &self,
            request: &ValidatedStoreSessionRequest,
        ) -> Result<StoredSessionMetadata, SessionCredentialError> {
            let expiry = request.expires_at_unix.as_i64();
            *self.expiry.lock().unwrap() = Some(expiry);
            Ok(StoredSessionMetadata {
                expires_at_unix: expiry,
            })
        }

        fn clear(&self) -> Result<(), SessionCredentialError> {
            *self.expiry.lock().unwrap() = None;
            Ok(())
        }

        fn metadata(&self) -> Result<Option<StoredSessionMetadata>, SessionCredentialError> {
            Ok(self
                .expiry
                .lock()
                .unwrap()
                .map(|expires_at_unix| StoredSessionMetadata { expires_at_unix }))
        }
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
    fn production_backend_persists_session_but_keeps_unwired_e2ee_state_closed() {
        let session_store = Arc::new(MemorySessionStore::default());
        let backend = ProductionDesktopBackend::with_session_store(session_store);
        let request = ValidatedStoreSessionRequest::try_from_dto(
            StoreSessionRequest {
                access_token: "A".repeat(64),
                refresh_token: "B".repeat(64),
                expires_at_unix: 500,
            },
            100,
        )
        .unwrap();

        assert_eq!(
            backend.store_session(request),
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
        assert_eq!(backend.clear_session(), Ok(()));
        assert_eq!(
            backend.read_session_metadata(),
            Ok(SessionMetadata {
                stored: false,
                expires_at_unix: None,
            })
        );
        assert_eq!(
            backend.initialize_e2ee_store(),
            Err(DesktopCommandBackendError::Unavailable)
        );
        assert_eq!(
            backend.read_encryption_settings(),
            Err(DesktopCommandBackendError::Unavailable)
        );
        assert_eq!(
            format!("{backend:?}"),
            "ProductionDesktopBackend(<native state redacted>)"
        );
    }
}
