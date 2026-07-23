//! Concrete Tauri runtime wiring for the packaged Filament clients.
//!
//! The initial runtime is intentionally fail closed: it exposes only the
//! already-audited command names and returns an opaque unavailable result until
//! the production session/network coordinator is injected.

use std::sync::Arc;

use filament_protocol::RotateRootIdentityResponse;
use tauri::ipc::InvokeBody;

use crate::{
    validate_runtime_navigation, DesktopCommandBackend, DesktopCommandBackendError,
    DesktopCommandError, DesktopCommandHost, E2eeStoreStatus, EncryptionSettingsSnapshot,
    RotateIdentityCommandRequest, SessionMetadata, StoreSessionRequest,
    ValidatedStoreSessionRequest,
};

/// Maximum serialized request body accepted at the native IPC boundary.
pub const MAX_TAURI_IPC_REQUEST_BYTES: usize = 16 * 1024;

#[derive(Debug, Default)]
struct UnavailableDesktopBackend;

impl DesktopCommandBackend for UnavailableDesktopBackend {
    fn store_session(
        &self,
        _request: ValidatedStoreSessionRequest,
    ) -> Result<SessionMetadata, DesktopCommandBackendError> {
        Err(DesktopCommandBackendError::Unavailable)
    }

    fn clear_session(&self) -> Result<(), DesktopCommandBackendError> {
        Err(DesktopCommandBackendError::Unavailable)
    }

    fn read_session_metadata(&self) -> Result<SessionMetadata, DesktopCommandBackendError> {
        Err(DesktopCommandBackendError::Unavailable)
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
    let host = Arc::new(DesktopCommandHost::new(Arc::new(UnavailableDesktopBackend)));

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
    use super::*;

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
    fn unavailable_backend_never_claims_native_state_is_ready() {
        let backend = UnavailableDesktopBackend;
        assert_eq!(
            backend.read_session_metadata(),
            Err(DesktopCommandBackendError::Unavailable)
        );
        assert_eq!(
            backend.initialize_e2ee_store(),
            Err(DesktopCommandBackendError::Unavailable)
        );
        assert_eq!(
            backend.read_encryption_settings(),
            Err(DesktopCommandBackendError::Unavailable)
        );
    }
}
