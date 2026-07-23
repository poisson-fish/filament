#![forbid(unsafe_code)]

use std::{fmt, fs, path::Path};

use filament_e2ee::{
    create_root_identity_rotation_proof, persist_root_identity, safety_number, KeyStoreError,
    LocalKeyStore, LocalStoreId, MlsDevice, RootIdentityKey, SqlCipherKeyStore, StoreKey,
    StoreKeyProvider, STORE_ENCRYPTION_KEY_BYTES,
};
use filament_protocol::{
    RotateRootIdentityRequest, RotateRootIdentityResponse, ROOT_IDENTITY_ROTATION_PROTOCOL_VERSION,
};
use keyring::Entry;
use rand::{rngs::OsRng, RngCore as _};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use url::Url;
use zeroize::{Zeroize as _, Zeroizing};

mod runtime;
mod session_store;
mod tauri_host;

pub use runtime::{run, MAX_TAURI_IPC_REQUEST_BYTES};

pub use tauri_host::{
    registered_desktop_commands, DesktopCommandBackend, DesktopCommandBackendError,
    DesktopCommandError, DesktopCommandHost, SessionMetadata,
};

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
    #[error("identity rotation confirmation is invalid")]
    InvalidIdentityRotationConfirmation,
    #[error("identity rotation response is invalid")]
    InvalidIdentityRotationResponse,
    #[error("identity rotation preparation failed")]
    IdentityRotationUnavailable,
}

#[derive(PartialEq, Eq)]
pub struct SessionToken(String);

impl fmt::Debug for SessionToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SessionToken([REDACTED])")
    }
}

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

impl Drop for SessionToken {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct UnixExpiry(i64);

impl UnixExpiry {
    /// Returns an expiry timestamp validated to be in the future.
    ///
    /// # Errors
    ///
    /// Returns [`SecurityError::InvalidExpiry`] when `expires_at_unix <= now_unix`.
    pub fn new(expires_at_unix: i64, now_unix: i64) -> Result<Self, SecurityError> {
        if expires_at_unix <= now_unix || expires_at_unix > 253_402_300_799 {
            return Err(SecurityError::InvalidExpiry);
        }

        Ok(Self(expires_at_unix))
    }

    #[must_use]
    pub fn as_i64(&self) -> i64 {
        self.0
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StoreSessionRequest {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at_unix: i64,
}

impl Drop for StoreSessionRequest {
    fn drop(&mut self) {
        self.access_token.zeroize();
        self.refresh_token.zeroize();
    }
}

impl fmt::Debug for StoreSessionRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StoreSessionRequest")
            .field("access_token", &"[REDACTED]")
            .field("refresh_token", &"[REDACTED]")
            .field("expires_at_unix", &self.expires_at_unix)
            .finish()
    }
}

#[derive(PartialEq, Eq)]
pub struct ValidatedStoreSessionRequest {
    pub access_token: SessionToken,
    pub refresh_token: SessionToken,
    pub expires_at_unix: UnixExpiry,
}

impl fmt::Debug for ValidatedStoreSessionRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ValidatedStoreSessionRequest")
            .field("access_token", &"[REDACTED]")
            .field("refresh_token", &"[REDACTED]")
            .field("expires_at_unix", &self.expires_at_unix)
            .finish()
    }
}

impl ValidatedStoreSessionRequest {
    /// Converts an IPC DTO into invariant-checked domain values.
    ///
    /// # Errors
    ///
    /// Returns any [`SecurityError`] raised by token or expiry validation.
    pub fn try_from_dto(
        mut dto: StoreSessionRequest,
        now_unix: i64,
    ) -> Result<Self, SecurityError> {
        Ok(Self {
            access_token: SessionToken::new(core::mem::take(&mut dto.access_token))?,
            refresh_token: SessionToken::new(core::mem::take(&mut dto.refresh_token))?,
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
    ReadEncryptionSettings,
    RotateRootIdentity,
}

impl DesktopCommand {
    #[must_use]
    pub const fn all() -> [Self; 7] {
        [
            Self::StoreSession,
            Self::ClearSession,
            Self::ReadSessionMetadata,
            Self::InitializeE2eeStore,
            Self::ReadE2eeStoreStatus,
            Self::ReadEncryptionSettings,
            Self::RotateRootIdentity,
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

/// Exact typed confirmation required by the destructive native command.
pub const ROTATE_IDENTITY_CONFIRMATION: &str = "ROTATE MY IDENTITY";

/// Strict webview request for the destructive native rotation action.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RotateIdentityCommandRequest {
    pub confirmation: String,
}

impl RotateIdentityCommandRequest {
    /// Validate the exact destructive confirmation without accepting aliases.
    ///
    /// # Errors
    /// Returns [`SecurityError::InvalidIdentityRotationConfirmation`] unless
    /// the bounded input exactly matches [`ROTATE_IDENTITY_CONFIRMATION`].
    pub fn validate(&self) -> Result<(), SecurityError> {
        if self.confirmation.len() > ROTATE_IDENTITY_CONFIRMATION.len()
            || self.confirmation != ROTATE_IDENTITY_CONFIRMATION
        {
            return Err(SecurityError::InvalidIdentityRotationConfirmation);
        }
        Ok(())
    }
}

/// Public device metadata safe to display in the packaged webview.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct EncryptionSettingsDevice {
    device_id: String,
    added_at_unix: i64,
    is_current_device: bool,
    verification: EncryptionDeviceVerification,
}

/// Closed verification state for the settings presentation model.
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EncryptionDeviceVerification {
    Verified,
    Unverified,
}

impl EncryptionSettingsDevice {
    /// Build bounded public device metadata from a validated domain ID.
    ///
    /// # Errors
    /// Rejects negative or out-of-range timestamps received from an untrusted
    /// server.
    pub fn new(
        device_id: filament_core::DeviceId,
        added_at_unix: i64,
        is_current_device: bool,
        verification: EncryptionDeviceVerification,
    ) -> Result<Self, SecurityError> {
        if !(0..=253_402_300_799).contains(&added_at_unix) {
            return Err(SecurityError::InvalidIdentityRotationResponse);
        }
        Ok(Self {
            device_id: device_id.to_string(),
            added_at_unix,
            is_current_device,
            verification,
        })
    }
}

/// Redacted encryption-settings presentation model.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct EncryptionSettingsSnapshot {
    pub ready: bool,
    pub safety_number: String,
    pub rotation_sequence: u64,
    pub devices: Vec<EncryptionSettingsDevice>,
    pub backup_enrolled: bool,
}

impl EncryptionSettingsSnapshot {
    /// Construct a public-only settings snapshot with bounded device metadata.
    ///
    /// # Errors
    /// Returns [`SecurityError::InvalidIdentityRotationResponse`] when more
    /// than 100 devices are supplied.
    pub fn new(
        root_public_key: &[u8; 32],
        rotation_sequence: u64,
        devices: Vec<EncryptionSettingsDevice>,
        backup_enrolled: bool,
    ) -> Result<Self, SecurityError> {
        if devices.len() > 100 {
            return Err(SecurityError::InvalidIdentityRotationResponse);
        }
        Ok(Self {
            ready: true,
            safety_number: safety_number(root_public_key),
            rotation_sequence,
            devices,
            backup_enrolled,
        })
    }
}

/// Native-only pending root rotation. Replacement secrets never serialize or
/// cross into the webview.
pub struct PreparedRootIdentityRotation {
    user_id: filament_core::UserId,
    device_id: filament_core::DeviceId,
    previous_root_key_pub: [u8; 32],
    replacement_root: RootIdentityKey,
    replacement_device: MlsDevice,
    request: RotateRootIdentityRequest,
}

impl core::fmt::Debug for PreparedRootIdentityRotation {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("PreparedRootIdentityRotation(<native secret state>)")
    }
}

impl PreparedRootIdentityRotation {
    /// Prepare public wire material while retaining all replacement secrets in Rust.
    ///
    /// Authenticated IDs and the previous root must come from native session and
    /// encrypted-store state, never from IPC fields.
    ///
    /// # Errors
    /// Returns an opaque error if proof or device generation fails.
    pub fn prepare(
        confirmation: &RotateIdentityCommandRequest,
        user_id: filament_core::UserId,
        device_id: filament_core::DeviceId,
        expected_rotation_sequence: u64,
        previous_root: &RootIdentityKey,
    ) -> Result<Self, SecurityError> {
        confirmation.validate()?;
        let replacement_root = RootIdentityKey::generate();
        let next_sequence = expected_rotation_sequence
            .checked_add(1)
            .ok_or(SecurityError::IdentityRotationUnavailable)?;
        let proof = create_root_identity_rotation_proof(
            previous_root,
            &replacement_root,
            user_id,
            next_sequence,
        )
        .map_err(|_| SecurityError::IdentityRotationUnavailable)?;
        let replacement_device = MlsDevice::generate(user_id, device_id, &replacement_root)
            .map_err(|_| SecurityError::IdentityRotationUnavailable)?;
        let request = RotateRootIdentityRequest {
            protocol_version: ROOT_IDENTITY_ROTATION_PROTOCOL_VERSION,
            expected_rotation_sequence,
            device_id: device_id.to_string(),
            new_root_key_pub: proof.new_root_key_pub.to_vec(),
            previous_root_signature: proof.previous_root_signature.to_vec(),
            new_root_signature: proof.new_root_signature.to_vec(),
            new_device_signature_pubkey: replacement_device
                .certificate()
                .device_signature_pubkey
                .clone(),
            new_device_root_signature: replacement_device.certificate().root_key_signature.clone(),
        };
        Ok(Self {
            user_id,
            device_id,
            previous_root_key_pub: previous_root.public_key_bytes(),
            replacement_root,
            replacement_device,
            request,
        })
    }

    /// Public-only request that the native network boundary may submit.
    #[must_use]
    pub const fn wire_request(&self) -> &RotateRootIdentityRequest {
        &self.request
    }

    /// Validate the authenticated server result, then atomically replace the
    /// locally persisted root identity.
    ///
    /// # Errors
    /// Rejects any response that differs from the prepared transition or any
    /// encrypted-store persistence failure.
    pub fn commit(
        self,
        response: &RotateRootIdentityResponse,
        store: &dyn LocalKeyStore,
    ) -> Result<MlsDevice, SecurityError> {
        let expected_sequence = self
            .request
            .expected_rotation_sequence
            .checked_add(1)
            .ok_or(SecurityError::InvalidIdentityRotationResponse)?;
        if response.protocol_version != ROOT_IDENTITY_ROTATION_PROTOCOL_VERSION
            || response.user_id != self.user_id.to_string()
            || response.device_id != self.device_id.to_string()
            || response.rotation_sequence != expected_sequence
            || response.previous_root_key_pub != self.previous_root_key_pub
            || response.new_root_key_pub != self.replacement_root.public_key_bytes()
        {
            return Err(SecurityError::InvalidIdentityRotationResponse);
        }
        persist_root_identity(store, StoreKey::root_identity(), &self.replacement_root)
            .map_err(|_| SecurityError::E2eeStoreUnavailable)?;
        Ok(self.replacement_device)
    }
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

/// Readiness of the only permitted desktop E2EE media backend.
///
/// The webview cannot claim readiness: this value must be supplied by the
/// native host after it has established the MLS-bound `LiveKit` GCM room.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeE2eeMediaReadiness {
    Ready,
    Unavailable,
}

/// The sole media path authorized for an E2EE desktop call.
///
/// Deliberately having no webview or plaintext variant makes an accidental
/// degraded fallback unrepresentable at this boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum E2eeMediaPath {
    NativeLiveKitGcm,
}

/// Stable failure returned when the native encrypted-media backend cannot be
/// established. Callers must leave call controls disabled.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum E2eeMediaPolicyError {
    #[error("encrypted calls are unavailable")]
    NativeMediaUnavailable,
}

/// Select the desktop E2EE media path without consulting webview capability.
///
/// Filament keeps MLS exporter material and decoded media in the Rust host on
/// every desktop target. `RTCRtpScriptTransform` probing is therefore
/// diagnostic only and can never authorize an alternate call path.
///
/// # Errors
/// Returns [`E2eeMediaPolicyError::NativeMediaUnavailable`] when the native
/// MLS-bound GCM backend is not ready. There is no unencrypted fallback.
pub const fn select_e2ee_media_path(
    _target: OsTarget,
    readiness: NativeE2eeMediaReadiness,
) -> Result<E2eeMediaPath, E2eeMediaPolicyError> {
    match readiness {
        NativeE2eeMediaReadiness::Ready => Ok(E2eeMediaPath::NativeLiveKitGcm),
        NativeE2eeMediaReadiness::Unavailable => Err(E2eeMediaPolicyError::NativeMediaUnavailable),
    }
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
    validate_runtime_navigation(url, true)
}

/// Validate a packaged-runtime navigation, allowing the development origin
/// only when explicitly requested by the native host.
///
/// # Errors
/// Returns a closed navigation-policy error for malformed, remote, credentialed,
/// or otherwise non-allowlisted URLs.
pub fn validate_runtime_navigation(
    url: &str,
    allow_development_origin: bool,
) -> Result<(), SecurityError> {
    let parsed = Url::parse(url).map_err(|_| SecurityError::InvalidNavigationUrl)?;
    let scheme = parsed.scheme();

    if !parsed.username().is_empty() || parsed.password().is_some() || parsed.port().is_some() {
        return Err(SecurityError::ForbiddenNavigationHost);
    }

    if scheme == "tauri" {
        return if parsed.host_str() == Some("localhost") {
            Ok(())
        } else {
            Err(SecurityError::ForbiddenNavigationHost)
        };
    }

    if matches!(scheme, "http" | "https") && parsed.host_str() == Some("tauri.localhost") {
        return Ok(());
    }

    if scheme != "https" {
        return Err(SecurityError::ForbiddenNavigationScheme);
    }

    if allow_development_origin && parsed.host_str() == Some("app.filament.local") {
        Ok(())
    } else {
        Err(SecurityError::ForbiddenNavigationHost)
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
            Self::ReadEncryptionSettings => "read_encryption_settings",
            Self::RotateRootIdentity => "rotate_root_identity",
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
    fn store_session_validation_rejects_out_of_range_expiry() {
        let request = StoreSessionRequest {
            access_token: "A".repeat(64),
            refresh_token: "B".repeat(64),
            expires_at_unix: 253_402_300_800,
        };

        let result = ValidatedStoreSessionRequest::try_from_dto(request, 100);
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
        assert_eq!(commands.len(), 7);
        assert_eq!(commands[0].to_string(), "store_session");
        assert_eq!(commands[1].to_string(), "clear_session");
        assert_eq!(commands[2].to_string(), "read_session_metadata");
        assert_eq!(commands[3].to_string(), "initialize_e2ee_store");
        assert_eq!(commands[4].to_string(), "read_e2ee_store_status");
        assert_eq!(commands[5].to_string(), "read_encryption_settings");
        assert_eq!(commands[6].to_string(), "rotate_root_identity");
    }

    #[test]
    fn navigation_policy_blocks_remote_hosts_and_http() {
        assert!(validate_desktop_navigation("tauri://localhost/index.html").is_ok());
        assert!(validate_runtime_navigation("https://tauri.localhost/index.html", false).is_ok());
        assert!(validate_runtime_navigation("http://tauri.localhost/index.html", false).is_ok());
        assert!(validate_desktop_navigation("https://app.filament.local/channels").is_ok());
        assert_eq!(
            validate_runtime_navigation("https://app.filament.local/channels", false),
            Err(SecurityError::ForbiddenNavigationHost)
        );
        assert_eq!(
            validate_desktop_navigation("tauri://evil.example/index.html"),
            Err(SecurityError::ForbiddenNavigationHost)
        );
        assert_eq!(
            validate_desktop_navigation("https://user@tauri.localhost/index.html"),
            Err(SecurityError::ForbiddenNavigationHost)
        );
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
    fn every_desktop_target_requires_the_native_gcm_media_path() {
        for target in [OsTarget::MacOs, OsTarget::Windows, OsTarget::Linux] {
            assert_eq!(
                select_e2ee_media_path(target, NativeE2eeMediaReadiness::Ready),
                Ok(E2eeMediaPath::NativeLiveKitGcm)
            );
            assert_eq!(
                select_e2ee_media_path(target, NativeE2eeMediaReadiness::Unavailable),
                Err(E2eeMediaPolicyError::NativeMediaUnavailable)
            );
        }
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

    #[test]
    fn encryption_settings_snapshot_contains_public_presentation_data_only() {
        let root = RootIdentityKey::from_secret_bytes(&[0x61; 32]);
        let snapshot = EncryptionSettingsSnapshot::new(
            &root.public_key_bytes(),
            2,
            vec![EncryptionSettingsDevice::new(
                DeviceId::try_from(String::from("01ARZ3NDEKTSV4RRFFQ69G5FAV")).unwrap(),
                1_700_000_000,
                true,
                EncryptionDeviceVerification::Verified,
            )
            .unwrap()],
            false,
        )
        .unwrap();
        let serialized = serde_json::to_string(&snapshot).unwrap();
        assert!(serialized.contains(&safety_number(&root.public_key_bytes())));
        for forbidden in ["root_key", "signature", "secret", "private", "path"] {
            assert!(!serialized.contains(forbidden));
        }
    }

    #[test]
    fn destructive_rotation_requires_exact_typed_confirmation() {
        for invalid in ["", "rotate my identity", "ROTATE MY IDENTITY ", "ROTATE"] {
            assert_eq!(
                RotateIdentityCommandRequest {
                    confirmation: String::from(invalid),
                }
                .validate(),
                Err(SecurityError::InvalidIdentityRotationConfirmation)
            );
        }
        RotateIdentityCommandRequest {
            confirmation: String::from(ROTATE_IDENTITY_CONFIRMATION),
        }
        .validate()
        .unwrap();
    }

    #[test]
    fn prepared_rotation_keeps_secrets_native_and_persists_only_after_exact_response() {
        use filament_e2ee::{load_root_identity, InMemoryKeyStore};

        let user_id = UserId::new();
        let device_id = DeviceId::new();
        let previous = RootIdentityKey::from_secret_bytes(&[0x71; 32]);
        let confirmation = RotateIdentityCommandRequest {
            confirmation: String::from(ROTATE_IDENTITY_CONFIRMATION),
        };
        let prepared =
            PreparedRootIdentityRotation::prepare(&confirmation, user_id, device_id, 3, &previous)
                .unwrap();
        assert_eq!(
            format!("{prepared:?}"),
            "PreparedRootIdentityRotation(<native secret state>)"
        );
        let request = prepared.wire_request().clone();
        let serialized = serde_json::to_string(&request).unwrap();
        for forbidden in ["secret", "private", "seed", "path"] {
            assert!(!serialized.contains(forbidden));
        }
        let response = RotateRootIdentityResponse {
            protocol_version: ROOT_IDENTITY_ROTATION_PROTOCOL_VERSION,
            user_id: user_id.to_string(),
            device_id: device_id.to_string(),
            rotation_sequence: 4,
            previous_root_key_pub: previous.public_key_bytes().to_vec(),
            new_root_key_pub: request.new_root_key_pub.clone(),
            revoked_device_count: 0,
            deleted_keypackage_count: 0,
            rotated_at_unix: 1_700_000_000,
        };
        let store = InMemoryKeyStore::new();
        let replacement_device = prepared.commit(&response, &store).unwrap();
        let persisted = load_root_identity(&store, &StoreKey::root_identity()).unwrap();
        assert_eq!(
            persisted.public_key_bytes(),
            request.new_root_key_pub.as_slice()
        );
        assert_eq!(
            replacement_device.root_key_public(),
            &persisted.public_key_bytes()
        );
    }
}
