#![forbid(unsafe_code)]

use std::fmt;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use url::Url;

pub const DESKTOP_CSP: &str = "default-src 'none'; base-uri 'none'; object-src 'none'; frame-ancestors 'none'; img-src 'self' data:; style-src 'self'; script-src 'self'; connect-src 'self' https://api.filament.local; font-src 'self'; form-action 'none'; media-src 'self' blob:;";
pub const WEB_CSP: &str = "default-src 'none'; base-uri 'none'; object-src 'none'; frame-ancestors 'none'; img-src 'self' data:; style-src 'self'; script-src 'self'; connect-src 'self' https://api.filament.local wss://api.filament.local; font-src 'self'; form-action 'none'; media-src 'self' blob:;";

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
}

impl DesktopCommand {
    #[must_use]
    pub const fn all() -> [Self; 3] {
        [
            Self::StoreSession,
            Self::ClearSession,
            Self::ReadSessionMetadata,
        ]
    }
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
        };
        f.write_str(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(commands.len(), 3);
        assert_eq!(commands[0].to_string(), "store_session");
        assert_eq!(commands[1].to_string(), "clear_session");
        assert_eq!(commands[2].to_string(), "read_session_metadata");
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
}
