#![forbid(unsafe_code)]

use std::{fmt, fs, path::Path};

use filament_e2ee::{
    KeyStoreError, LocalKeyStore, LocalStoreId, SqlCipherKeyStore, StoreKeyProvider,
    STORE_ENCRYPTION_KEY_BYTES,
};
use keyring::Entry;
use rand::{rngs::OsRng, RngCore as _};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use url::Url;
use zeroize::Zeroizing;

pub const DESKTOP_CSP: &str = "default-src 'none'; base-uri 'none'; object-src 'none'; frame-ancestors 'none'; img-src 'self' data: blob:; style-src 'self'; script-src 'self'; connect-src 'self' https://api.filament.local; font-src 'self'; form-action 'none'; media-src 'self' blob:;";
pub const WEB_CSP: &str = "default-src 'none'; base-uri 'none'; object-src 'none'; frame-ancestors 'none'; img-src 'self' data: blob:; style-src 'self'; script-src 'self'; connect-src 'self' https://api.filament.local wss://api.filament.local; font-src 'self'; form-action 'none'; media-src 'self' blob:;";

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SecurityError {
    #[error("token length is out of bounds")]
    InvalidTokenLength,
    #[error("token contains non-printable ASCII")]
    InvalidTokenCharset,
    #[error("expires_at_unix must be in the future")]
    InvalidExpiry,
    #[error("navigation URL is invalid")]
    InvalidNavigationUrl,
    #[error("navigation URL scheme is not allowed")]
    ForbiddenNavigationScheme,
    #[error("navigation host is not allowed")]
    ForbiddenNavigationHost,
    #[error("encrypted local store is unavailable")]
    E2eeStoreUnavailable,
    #[error("encrypted local store root is invalid")]
    InvalidE2eeStoreRoot,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionToken(String);

impl SessionToken {
    /// Returns a bounded-printable token value.
    ///
    /// # Errors
    ///
    /// Returns [`SecurityError::InvalidTokenLength`] when token length is outside
    /// `32..=4096`, or [`SecurityError::InvalidTokenCharset`] when bytes are not
    /// printable ASCII.
    pub fn new(value: String) -> Result<Self, SecurityError> {
        let len = value.len();
        if !(32..=4096).contains(&len) {
            return Err(SecurityError::InvalidTokenLength);
        }

        if !value.bytes().all(|byte| (0x21..=0x7e).contains(&byte)) {
            return Err(SecurityError::InvalidTokenCharset);
        }

        Ok(Self(value))
    }

    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnixExpiry(i64);

impl UnixExpiry {
    /// Returns an expiry timestamp validated to be in the future.
    ///
    /// # Errors
    ///
    /// Returns [`SecurityError::InvalidExpiry`] when `expires_at_unix <= now_unix`.
    pub fn new(expires_at_unix: i64, now_unix: i64) -> Result<Self, SecurityError> {
        if expires_at_unix <= now_unix {
            return Err(SecurityError::InvalidExpiry);
        }

        Ok(Self(expires_at_unix))
    }

    #[must_use]
    pub fn as_i64(&self) -> i64 {
        self.0
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct StoreSessionRequest {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at_unix: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValidatedStoreSessionRequest {
    pub access_token: SessionToken,
    pub refresh_token: SessionToken,
    pub expires_at_unix: UnixExpiry,
}

impl ValidatedStoreSessionRequest {
    /// Converts an IPC DTO into invariant-checked domain values.
    ///
    /// # Errors
    ///
    /// Returns any [`SecurityError`] raised by token or expiry validation.
    pub fn try_from_dto(dto: StoreSessionRequest, now_unix: i64) -> Result<Self, SecurityError> {
        Ok(Self {
            access_token: SessionToken::new(dto.access_token)?,
            refresh_token: SessionToken::new(dto.refresh_token)?,
            expires_at_unix: UnixExpiry::new(dto.expires_at_unix, now_unix)?,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DesktopCommand {
    StoreSession,
    ClearSession,
    ReadSessionMetadata,
    InitializeE2eeStore,
    ReadE2eeStoreStatus,
}

impl DesktopCommand {
    #[must_use]
    pub const fn all() -> [Self; 5] {
        [
            Self::StoreSession,
            Self::ClearSession,
            Self::ReadSessionMetadata,
            Self::InitializeE2eeStore,
            Self::ReadE2eeStoreStatus,
        ]
    }
}

/// Non-sensitive IPC response for the native encrypted-store boundary.
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct E2eeStoreStatus {
    /// Whether the native host successfully opened the store.
    pub ready: bool,
    /// Fixed backend identifier; never contains a filesystem path.
    pub backend: &'static str,
    /// Fixed key-custody identifier; never contains key bytes or an account ID.
    pub key_custody: &'static str,
}

/// Native-only desktop encrypted store.
///
/// The host derives [`LocalStoreId`] from its authenticated native session.
/// The webview never supplies user/device IDs, filesystem paths, or key bytes.
pub struct DesktopE2eeStore {
    store: SqlCipherKeyStore,
}

impl core::fmt::Debug for DesktopE2eeStore {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("DesktopE2eeStore(<native encrypted state>)")
    }
}

impl DesktopE2eeStore {
    /// Open the authenticated device's `SQLCipher` store using the OS keyring.
    ///
    /// `app_data_root` must come from the Tauri host, not IPC. The directory
    /// must already exist and resolve to a regular directory. The database
    /// filename is derived only from validated domain IDs.
    ///
    /// # Errors
    /// Returns an opaque error when the root, OS keyring, or `SQLCipher` store is
    /// unavailable. No path or key material is included in the error.
    pub fn open(app_data_root: &Path, store_id: LocalStoreId) -> Result<Self, SecurityError> {
        Self::open_with_provider(app_data_root, store_id, &OsStoreKeyProvider)
    }

    fn open_with_provider(
        app_data_root: &Path,
        store_id: LocalStoreId,
        provider: &dyn StoreKeyProvider,
    ) -> Result<Self, SecurityError> {
        let store_directory = prepare_store_directory(app_data_root)?;
        let filename = format!("{}-{}.db", store_id.user_id(), store_id.device_id());
        let database_path = store_directory.join(filename);
        let store = SqlCipherKeyStore::open(&database_path, &store_id, provider)
            .map_err(|_| SecurityError::E2eeStoreUnavailable)?;
        Ok(Self { store })
    }

    /// Non-sensitive state that may cross the narrow IPC boundary.
    #[must_use]
    pub const fn status(&self) -> E2eeStoreStatus {
        E2eeStoreStatus {
            ready: true,
            backend: "sqlcipher",
            key_custody: "platform_keystore",
        }
    }

    /// Native E2EE core access. This object is never serializable and must not
    /// be returned from a Tauri command.
    #[must_use]
    pub const fn native_store(&self) -> &dyn LocalKeyStore {
        &self.store
    }
}

/// OS Keychain / Credential Manager / Secret Service provider.
pub struct OsStoreKeyProvider;

impl core::fmt::Debug for OsStoreKeyProvider {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("OsStoreKeyProvider(<credential metadata redacted>)")
    }
}

impl StoreKeyProvider for OsStoreKeyProvider {
    fn load_or_create_key(
        &self,
        store_id: &LocalStoreId,
    ) -> Result<Zeroizing<Vec<u8>>, KeyStoreError> {
        const SERVICE: &str = "com.filament.desktop.e2ee-store";

        let account = store_id.credential_account();
        let entry = Entry::new(SERVICE, &account).map_err(|_| KeyStoreError::KeyUnavailable)?;
        let secret = Zeroizing::new(match entry.get_secret() {
            Ok(secret) => secret,
            Err(keyring::Error::NoEntry) => {
                let mut generated = Zeroizing::new(vec![0_u8; STORE_ENCRYPTION_KEY_BYTES]);
                OsRng
                    .try_fill_bytes(&mut generated)
                    .map_err(|_| KeyStoreError::KeyUnavailable)?;
                entry
                    .set_secret(&generated)
                    .map_err(|_| KeyStoreError::KeyUnavailable)?;
                entry
                    .get_secret()
                    .map_err(|_| KeyStoreError::KeyUnavailable)?
            }
            Err(_) => return Err(KeyStoreError::KeyUnavailable),
        });
        if secret.len() != STORE_ENCRYPTION_KEY_BYTES {
            return Err(KeyStoreError::InvalidValue);
        }
        Ok(secret)
    }
}

fn prepare_store_directory(app_data_root: &Path) -> Result<std::path::PathBuf, SecurityError> {
    if !app_data_root.is_absolute() {
        return Err(SecurityError::InvalidE2eeStoreRoot);
    }
    let root = app_data_root
        .canonicalize()
        .map_err(|_| SecurityError::InvalidE2eeStoreRoot)?;
    if !root.is_dir() {
        return Err(SecurityError::InvalidE2eeStoreRoot);
    }
    let store_directory = root.join("e2ee");
    match fs::symlink_metadata(&store_directory) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() || !metadata.is_dir() {
                return Err(SecurityError::InvalidE2eeStoreRoot);
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            create_private_store_directory(&store_directory)?;
        }
        Err(_) => return Err(SecurityError::InvalidE2eeStoreRoot),
    }
    enforce_store_directory_permissions(&store_directory)?;
    Ok(store_directory)
}

#[cfg(unix)]
fn create_private_store_directory(path: &Path) -> Result<(), SecurityError> {
    use std::os::unix::fs::DirBuilderExt as _;

    let mut builder = fs::DirBuilder::new();
    builder.mode(0o700);
    builder
        .create(path)
        .map_err(|_| SecurityError::InvalidE2eeStoreRoot)
}

#[cfg(not(unix))]
fn create_private_store_directory(path: &Path) -> Result<(), SecurityError> {
    fs::create_dir(path).map_err(|_| SecurityError::InvalidE2eeStoreRoot)
}

#[cfg(unix)]
fn enforce_store_directory_permissions(path: &Path) -> Result<(), SecurityError> {
    use std::os::unix::fs::PermissionsExt as _;

    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
        .map_err(|_| SecurityError::InvalidE2eeStoreRoot)
}

#[cfg(not(unix))]
fn enforce_store_directory_permissions(_path: &Path) -> Result<(), SecurityError> {
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OsTarget {
    MacOs,
    Windows,
    Linux,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TokenStoragePolicy {
    pub backend: &'static str,
    pub service: &'static str,
    pub account_prefix: &'static str,
}

impl TokenStoragePolicy {
    #[must_use]
    pub const fn for_target(target: OsTarget) -> Self {
        match target {
            OsTarget::MacOs => Self {
                backend: "macos-keychain",
                service: "com.filament.desktop",
                account_prefix: "filament-user-",
            },
            OsTarget::Windows => Self {
                backend: "windows-credential-manager",
                service: "FilamentDesktop",
                account_prefix: "filament-user-",
            },
            OsTarget::Linux => Self {
                backend: "secret-service",
                service: "com.filament.desktop",
                account_prefix: "filament-user-",
            },
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct CrashLogEvent {
    pub event: &'static str,
    pub user_id: Option<String>,
    pub reason: &'static str,
    pub access_token: Option<&'static str>,
    pub refresh_token: Option<&'static str>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct RedactedCrashLogEvent {
    pub event: &'static str,
    pub user_id: Option<String>,
    pub reason: &'static str,
    pub access_token: &'static str,
    pub refresh_token: &'static str,
}

#[must_use]
pub fn redact_crash_log(event: CrashLogEvent) -> RedactedCrashLogEvent {
    let _ = event.access_token;
    let _ = event.refresh_token;

    RedactedCrashLogEvent {
        event: event.event,
        user_id: event.user_id,
        reason: event.reason,
        access_token: "[REDACTED]",
        refresh_token: "[REDACTED]",
    }
}

/// Validates a desktop navigation target against an allowlist.
///
/// # Errors
///
/// Returns [`SecurityError::InvalidNavigationUrl`] when parsing fails,
/// [`SecurityError::ForbiddenNavigationScheme`] for non-`tauri`/`https` schemes,
/// and [`SecurityError::ForbiddenNavigationHost`] for non-allowlisted `https` hosts.
pub fn validate_desktop_navigation(url: &str) -> Result<(), SecurityError> {
    let parsed = Url::parse(url).map_err(|_| SecurityError::InvalidNavigationUrl)?;
    let scheme = parsed.scheme();

    if scheme == "tauri" {
        return Ok(());
    }

    if scheme != "https" {
        return Err(SecurityError::ForbiddenNavigationScheme);
    }

    match parsed.host_str() {
        Some("app.filament.local") => Ok(()),
        _ => Err(SecurityError::ForbiddenNavigationHost),
    }
}

#[must_use]
pub fn csp_has_forbidden_tokens(csp: &str) -> bool {
    ["unsafe-inline", "unsafe-eval", "http://", "data:text/html"]
        .iter()
        .any(|token| csp.contains(token))
}

impl fmt::Display for DesktopCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::StoreSession => "store_session",
            Self::ClearSession => "clear_session",
            Self::ReadSessionMetadata => "read_session_metadata",
            Self::InitializeE2eeStore => "initialize_e2ee_store",
            Self::ReadE2eeStoreStatus => "read_e2ee_store_status",
        };
        f.write_str(value)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use filament_core::{DeviceId, UserId};
    use tempfile::tempdir;

    use super::*;

    struct FixedStoreKeyProvider {
        calls: AtomicUsize,
    }

    impl StoreKeyProvider for FixedStoreKeyProvider {
        fn load_or_create_key(
            &self,
            _store_id: &LocalStoreId,
        ) -> Result<Zeroizing<Vec<u8>>, KeyStoreError> {
            self.calls.fetch_add(1, Ordering::Relaxed);
            Ok(Zeroizing::new(vec![0x42; STORE_ENCRYPTION_KEY_BYTES]))
        }
    }

    #[test]
    fn store_session_validation_rejects_expired_tokens() {
        let request = StoreSessionRequest {
            access_token: "A".repeat(64),
            refresh_token: "B".repeat(64),
            expires_at_unix: 100,
        };

        let result = ValidatedStoreSessionRequest::try_from_dto(request, 101);
        assert_eq!(result, Err(SecurityError::InvalidExpiry));
    }

    #[test]
    fn store_session_validation_rejects_non_printable_tokens() {
        let request = StoreSessionRequest {
            access_token: format!("{}{}", "A".repeat(31), '\n'),
            refresh_token: "B".repeat(64),
            expires_at_unix: 500,
        };

        let result = ValidatedStoreSessionRequest::try_from_dto(request, 100);
        assert_eq!(result, Err(SecurityError::InvalidTokenCharset));
    }

    #[test]
    fn store_session_validation_accepts_valid_payload() {
        let request = StoreSessionRequest {
            access_token: "A".repeat(64),
            refresh_token: "B".repeat(64),
            expires_at_unix: 500,
        };

        let validated = ValidatedStoreSessionRequest::try_from_dto(request, 100)
            .expect("valid payload should pass");
        assert_eq!(validated.access_token.expose().len(), 64);
        assert_eq!(validated.refresh_token.expose().len(), 64);
        assert_eq!(validated.expires_at_unix.as_i64(), 500);
    }

    #[test]
    fn desktop_commands_are_strictly_bounded() {
        let commands = DesktopCommand::all();
        assert_eq!(commands.len(), 5);
        assert_eq!(commands[0].to_string(), "store_session");
        assert_eq!(commands[1].to_string(), "clear_session");
        assert_eq!(commands[2].to_string(), "read_session_metadata");
        assert_eq!(commands[3].to_string(), "initialize_e2ee_store");
        assert_eq!(commands[4].to_string(), "read_e2ee_store_status");
    }

    #[test]
    fn navigation_policy_blocks_remote_hosts_and_http() {
        assert!(validate_desktop_navigation("tauri://localhost/index.html").is_ok());
        assert!(validate_desktop_navigation("https://app.filament.local/channels").is_ok());
        assert_eq!(
            validate_desktop_navigation("https://evil.example/phish"),
            Err(SecurityError::ForbiddenNavigationHost)
        );
        assert_eq!(
            validate_desktop_navigation("http://app.filament.local/channels"),
            Err(SecurityError::ForbiddenNavigationScheme)
        );
    }

    #[test]
    fn token_storage_policy_exists_for_all_targets() {
        let mac = TokenStoragePolicy::for_target(OsTarget::MacOs);
        let windows = TokenStoragePolicy::for_target(OsTarget::Windows);
        let linux = TokenStoragePolicy::for_target(OsTarget::Linux);

        assert_eq!(mac.backend, "macos-keychain");
        assert_eq!(windows.backend, "windows-credential-manager");
        assert_eq!(linux.backend, "secret-service");
        assert_eq!(mac.account_prefix, windows.account_prefix);
        assert_eq!(windows.account_prefix, linux.account_prefix);
    }

    #[test]
    fn crash_logs_are_redacted() {
        let redacted = redact_crash_log(CrashLogEvent {
            event: "client_panic",
            user_id: Some(String::from("01HXY")),
            reason: "webview panicked",
            access_token: Some("secret-access"),
            refresh_token: Some("secret-refresh"),
        });

        assert_eq!(redacted.access_token, "[REDACTED]");
        assert_eq!(redacted.refresh_token, "[REDACTED]");
    }

    #[test]
    fn csp_constants_disallow_unsafe_tokens() {
        assert!(!csp_has_forbidden_tokens(DESKTOP_CSP));
        assert!(!csp_has_forbidden_tokens(WEB_CSP));
    }

    #[test]
    fn encrypted_store_is_native_only_and_ipc_status_has_no_sensitive_fields() {
        let app_data = tempdir().unwrap();
        let store_id = LocalStoreId::new(UserId::new(), DeviceId::new());
        let provider = FixedStoreKeyProvider {
            calls: AtomicUsize::new(0),
        };
        let desktop =
            DesktopE2eeStore::open_with_provider(app_data.path(), store_id, &provider).unwrap();
        let serialized = serde_json::to_string(&desktop.status()).unwrap();

        assert_eq!(provider.calls.load(Ordering::Relaxed), 1);
        assert_eq!(
            serialized,
            r#"{"ready":true,"backend":"sqlcipher","key_custody":"platform_keystore"}"#
        );
        for forbidden in ["secret", "private", "path", "user_id", "device_id"] {
            assert!(!serialized.contains(forbidden));
        }
        assert_eq!(
            format!("{desktop:?}"),
            "DesktopE2eeStore(<native encrypted state>)"
        );
    }

    #[test]
    fn encrypted_store_rejects_relative_and_symlink_roots() {
        let store_id = LocalStoreId::new(UserId::new(), DeviceId::new());
        let provider = FixedStoreKeyProvider {
            calls: AtomicUsize::new(0),
        };
        assert_eq!(
            DesktopE2eeStore::open_with_provider(Path::new("relative"), store_id, &provider)
                .unwrap_err(),
            SecurityError::InvalidE2eeStoreRoot
        );

        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;

            let parent = tempdir().unwrap();
            let target = tempdir().unwrap();
            symlink(target.path(), parent.path().join("e2ee")).unwrap();
            assert_eq!(
                DesktopE2eeStore::open_with_provider(parent.path(), store_id, &provider)
                    .unwrap_err(),
                SecurityError::InvalidE2eeStoreRoot
            );
        }
    }

    #[test]
    fn keyring_provider_debug_redacts_credential_metadata() {
        assert_eq!(
            format!("{OsStoreKeyProvider:?}"),
            "OsStoreKeyProvider(<credential metadata redacted>)"
        );
    }
}
