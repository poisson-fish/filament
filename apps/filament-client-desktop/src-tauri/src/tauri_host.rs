//! Native host boundary for the audited desktop command surface.
//!
//! The host owns the command backend and the clock. Webview requests can select
//! only one of the statically registered commands; native session identity,
//! filesystem paths, credential-store accounts, MLS state, and key material are
//! never accepted as command arguments.

use std::{
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use filament_protocol::RotateRootIdentityResponse;
use serde::Serialize;
use thiserror::Error;

use crate::{
    DesktopCommand, E2eeStoreStatus, EncryptionSettingsSnapshot, RotateIdentityCommandRequest,
    StoreSessionRequest, ValidatedStoreSessionRequest,
};

/// The only commands a future packaged-runtime adapter may register.
#[must_use]
pub const fn registered_desktop_commands() -> [DesktopCommand; 7] {
    DesktopCommand::all()
}

/// Public-only session state returned to the webview.
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SessionMetadata {
    pub stored: bool,
    pub expires_at_unix: Option<i64>,
}

/// Opaque failures produced by a native command backend.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum DesktopCommandBackendError {
    #[error("native command backend is unavailable")]
    Unavailable,
    #[error("native command was rejected")]
    Rejected,
}

/// Stable, non-sensitive IPC failure returned to the packaged webview.
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DesktopCommandError {
    InvalidRequest,
    Unavailable,
    Rejected,
}

impl From<DesktopCommandBackendError> for DesktopCommandError {
    fn from(error: DesktopCommandBackendError) -> Self {
        match error {
            DesktopCommandBackendError::Unavailable => Self::Unavailable,
            DesktopCommandBackendError::Rejected => Self::Rejected,
        }
    }
}

/// Native implementation boundary for the seven audited desktop commands.
///
/// Implementations own platform credential storage, authenticated device
/// identity, the `SQLCipher` store, network submission, and MLS state. None of
/// those capabilities is represented by a general-purpose IPC argument.
pub trait DesktopCommandBackend: Send + Sync + 'static {
    /// # Errors
    /// Returns an opaque backend failure when secure session storage fails.
    fn store_session(
        &self,
        request: ValidatedStoreSessionRequest,
    ) -> Result<SessionMetadata, DesktopCommandBackendError>;

    /// # Errors
    /// Returns an opaque backend failure when secure session deletion fails.
    fn clear_session(&self) -> Result<(), DesktopCommandBackendError>;

    /// # Errors
    /// Returns an opaque backend failure when session metadata is unavailable.
    fn read_session_metadata(&self) -> Result<SessionMetadata, DesktopCommandBackendError>;

    /// # Errors
    /// Returns an opaque backend failure when the encrypted store cannot open.
    fn initialize_e2ee_store(&self) -> Result<E2eeStoreStatus, DesktopCommandBackendError>;

    /// # Errors
    /// Returns an opaque backend failure when store status is unavailable.
    fn read_e2ee_store_status(&self) -> Result<E2eeStoreStatus, DesktopCommandBackendError>;

    /// # Errors
    /// Returns an opaque backend failure when public settings are unavailable.
    fn read_encryption_settings(
        &self,
    ) -> Result<EncryptionSettingsSnapshot, DesktopCommandBackendError>;

    /// # Errors
    /// Returns an opaque backend failure when rotation fails or is rejected.
    fn rotate_root_identity(
        &self,
        request: RotateIdentityCommandRequest,
    ) -> Result<RotateRootIdentityResponse, DesktopCommandBackendError>;
}

type Clock = dyn Fn() -> Result<i64, DesktopCommandError> + Send + Sync;

/// Native command state. Debug output never traverses the backend.
pub struct DesktopCommandHost {
    backend: Arc<dyn DesktopCommandBackend>,
    clock: Arc<Clock>,
}

impl core::fmt::Debug for DesktopCommandHost {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("DesktopCommandHost(<native state redacted>)")
    }
}

impl DesktopCommandHost {
    #[must_use]
    pub fn new(backend: Arc<dyn DesktopCommandBackend>) -> Self {
        Self {
            backend,
            clock: Arc::new(system_time_unix),
        }
    }

    #[cfg(test)]
    fn with_clock(
        backend: Arc<dyn DesktopCommandBackend>,
        clock: impl Fn() -> Result<i64, DesktopCommandError> + Send + Sync + 'static,
    ) -> Self {
        Self {
            backend,
            clock: Arc::new(clock),
        }
    }

    /// Validate and securely store one bounded session.
    ///
    /// # Errors
    /// Returns `InvalidRequest` for malformed or expired tokens and an opaque
    /// failure code for clock or backend failures.
    pub fn store_session(
        &self,
        request: StoreSessionRequest,
    ) -> Result<SessionMetadata, DesktopCommandError> {
        let now_unix = (self.clock)()?;
        let request = ValidatedStoreSessionRequest::try_from_dto(request, now_unix)
            .map_err(|_| DesktopCommandError::InvalidRequest)?;
        self.backend.store_session(request).map_err(Into::into)
    }

    /// Clear the securely stored session.
    ///
    /// # Errors
    /// Returns an opaque backend failure code.
    pub fn clear_session(&self) -> Result<(), DesktopCommandError> {
        self.backend.clear_session().map_err(Into::into)
    }

    /// Read non-sensitive session metadata.
    ///
    /// # Errors
    /// Returns an opaque backend failure code.
    pub fn read_session_metadata(&self) -> Result<SessionMetadata, DesktopCommandError> {
        self.backend.read_session_metadata().map_err(Into::into)
    }

    /// Initialize the encrypted store from native-only identity and path state.
    ///
    /// # Errors
    /// Returns an opaque backend failure code.
    pub fn initialize_e2ee_store(&self) -> Result<E2eeStoreStatus, DesktopCommandError> {
        self.backend.initialize_e2ee_store().map_err(Into::into)
    }

    /// Read fixed, non-sensitive encrypted-store readiness metadata.
    ///
    /// # Errors
    /// Returns an opaque backend failure code.
    pub fn read_e2ee_store_status(&self) -> Result<E2eeStoreStatus, DesktopCommandError> {
        self.backend.read_e2ee_store_status().map_err(Into::into)
    }

    /// Read public encryption settings without exposing key material.
    ///
    /// # Errors
    /// Returns an opaque backend failure code.
    pub fn read_encryption_settings(
        &self,
    ) -> Result<EncryptionSettingsSnapshot, DesktopCommandError> {
        self.backend.read_encryption_settings().map_err(Into::into)
    }

    /// Revalidate the destructive confirmation and delegate native rotation.
    ///
    /// # Errors
    /// Returns `InvalidRequest` for a mismatched confirmation and an opaque
    /// backend failure code otherwise.
    pub fn rotate_root_identity(
        &self,
        request: RotateIdentityCommandRequest,
    ) -> Result<RotateRootIdentityResponse, DesktopCommandError> {
        request
            .validate()
            .map_err(|_| DesktopCommandError::InvalidRequest)?;
        self.backend
            .rotate_root_identity(request)
            .map_err(Into::into)
    }
}

fn system_time_unix() -> Result<i64, DesktopCommandError> {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| DesktopCommandError::Unavailable)?
        .as_secs();
    i64::try_from(seconds).map_err(|_| DesktopCommandError::Unavailable)
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;
    use crate::{
        EncryptionDeviceVerification, EncryptionSettingsDevice, RootIdentityKey,
        ROTATE_IDENTITY_CONFIRMATION,
    };
    use filament_core::DeviceId;

    #[derive(Default)]
    struct RecordingBackend {
        stored_expiry: Mutex<Option<i64>>,
    }

    impl DesktopCommandBackend for RecordingBackend {
        fn store_session(
            &self,
            request: ValidatedStoreSessionRequest,
        ) -> Result<SessionMetadata, DesktopCommandBackendError> {
            let expiry = request.expires_at_unix.as_i64();
            *self.stored_expiry.lock().unwrap() = Some(expiry);
            Ok(SessionMetadata {
                stored: true,
                expires_at_unix: Some(expiry),
            })
        }

        fn clear_session(&self) -> Result<(), DesktopCommandBackendError> {
            *self.stored_expiry.lock().unwrap() = None;
            Ok(())
        }

        fn read_session_metadata(&self) -> Result<SessionMetadata, DesktopCommandBackendError> {
            let expiry = *self.stored_expiry.lock().unwrap();
            Ok(SessionMetadata {
                stored: expiry.is_some(),
                expires_at_unix: expiry,
            })
        }

        fn initialize_e2ee_store(&self) -> Result<E2eeStoreStatus, DesktopCommandBackendError> {
            Ok(store_status())
        }

        fn read_e2ee_store_status(&self) -> Result<E2eeStoreStatus, DesktopCommandBackendError> {
            Ok(store_status())
        }

        fn read_encryption_settings(
            &self,
        ) -> Result<EncryptionSettingsSnapshot, DesktopCommandBackendError> {
            let root = RootIdentityKey::from_secret_bytes(&[0x31; 32]);
            EncryptionSettingsSnapshot::new(
                &root.public_key_bytes(),
                0,
                vec![EncryptionSettingsDevice::new(
                    DeviceId::try_from(String::from("01ARZ3NDEKTSV4RRFFQ69G5FAV")).unwrap(),
                    1_700_000_000,
                    true,
                    EncryptionDeviceVerification::Verified,
                )
                .unwrap()],
                false,
            )
            .map_err(|_| DesktopCommandBackendError::Unavailable)
        }

        fn rotate_root_identity(
            &self,
            _request: RotateIdentityCommandRequest,
        ) -> Result<RotateRootIdentityResponse, DesktopCommandBackendError> {
            Err(DesktopCommandBackendError::Rejected)
        }
    }

    const fn store_status() -> E2eeStoreStatus {
        E2eeStoreStatus {
            ready: true,
            backend: "sqlcipher",
            key_custody: "platform_keystore",
        }
    }

    fn valid_session() -> StoreSessionRequest {
        StoreSessionRequest {
            access_token: "A".repeat(64),
            refresh_token: "B".repeat(64),
            expires_at_unix: 500,
        }
    }

    #[test]
    fn host_validates_sessions_before_the_native_backend() {
        let backend = Arc::new(RecordingBackend::default());
        let host = DesktopCommandHost::with_clock(backend.clone(), || Ok(100));

        assert_eq!(
            host.store_session(valid_session()).unwrap(),
            SessionMetadata {
                stored: true,
                expires_at_unix: Some(500),
            }
        );
        assert_eq!(*backend.stored_expiry.lock().unwrap(), Some(500));

        let mut expired = valid_session();
        expired.expires_at_unix = 100;
        assert_eq!(
            host.store_session(expired),
            Err(DesktopCommandError::InvalidRequest)
        );
        assert_eq!(*backend.stored_expiry.lock().unwrap(), Some(500));
    }

    #[test]
    fn host_revalidates_destructive_confirmation_before_the_backend() {
        let host =
            DesktopCommandHost::with_clock(Arc::new(RecordingBackend::default()), || Ok(100));
        assert_eq!(
            host.rotate_root_identity(RotateIdentityCommandRequest {
                confirmation: String::from("ROTATE"),
            }),
            Err(DesktopCommandError::InvalidRequest)
        );
        assert_eq!(
            host.rotate_root_identity(RotateIdentityCommandRequest {
                confirmation: String::from(ROTATE_IDENTITY_CONFIRMATION),
            }),
            Err(DesktopCommandError::Rejected)
        );
    }

    #[test]
    fn host_and_session_debug_output_are_redacted() {
        let host =
            DesktopCommandHost::with_clock(Arc::new(RecordingBackend::default()), || Ok(100));
        assert_eq!(
            format!("{host:?}"),
            "DesktopCommandHost(<native state redacted>)"
        );
        let request = valid_session();
        let debug = format!("{request:?}");
        assert!(!debug.contains(&request.access_token));
        assert!(!debug.contains(&request.refresh_token));
    }

    #[test]
    fn native_host_exposes_only_the_audited_command_manifest() {
        let registered = registered_desktop_commands();
        assert_eq!(registered, DesktopCommand::all());
        assert_eq!(registered.len(), 7);
        assert!(!registered
            .iter()
            .any(|command| command.to_string() == "read_raw_e2ee_state"));
    }
}
