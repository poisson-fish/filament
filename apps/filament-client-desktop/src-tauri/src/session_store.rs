//! Native session custody for packaged clients.
//!
//! Tokens are stored as one bounded credential so a process interruption
//! cannot expose a mismatched access/refresh pair. The fixed service and
//! account names are native policy; neither is selectable over IPC.

use core::mem;

use keyring::Entry;
use serde::{Deserialize, Serialize};
use zeroize::{Zeroize as _, Zeroizing};

use crate::{SecurityError, SessionToken, UnixExpiry, ValidatedStoreSessionRequest};

const SESSION_CREDENTIAL_SERVICE: &str = "com.filament.desktop.session";
const SESSION_CREDENTIAL_ACCOUNT: &str = "active-session-v1";
const MAX_SESSION_CREDENTIAL_BYTES: usize = 12 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct StoredSessionMetadata {
    pub(crate) expires_at_unix: i64,
}

pub(crate) struct StoredSession {
    pub(crate) access_token: SessionToken,
    pub(crate) expires_at_unix: i64,
}

impl core::fmt::Debug for StoredSession {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("StoredSession")
            .field("access_token", &"[REDACTED]")
            .field("expires_at_unix", &self.expires_at_unix)
            .finish()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SessionCredentialError {
    Unavailable,
    Rejected,
}

pub(crate) trait SessionCredentialStore: Send + Sync + 'static {
    fn store(
        &self,
        request: &ValidatedStoreSessionRequest,
    ) -> Result<StoredSessionMetadata, SessionCredentialError>;

    fn clear(&self) -> Result<(), SessionCredentialError>;

    fn load(&self) -> Result<Option<StoredSession>, SessionCredentialError>;

    fn metadata(&self) -> Result<Option<StoredSessionMetadata>, SessionCredentialError>;
}

pub(crate) struct OsSessionCredentialStore;

impl core::fmt::Debug for OsSessionCredentialStore {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("OsSessionCredentialStore(<credential metadata redacted>)")
    }
}

impl SessionCredentialStore for OsSessionCredentialStore {
    fn store(
        &self,
        request: &ValidatedStoreSessionRequest,
    ) -> Result<StoredSessionMetadata, SessionCredentialError> {
        let serialized = encode_session(request)?;
        session_entry()?
            .set_secret(&serialized)
            .map_err(|_| SessionCredentialError::Unavailable)?;
        Ok(StoredSessionMetadata {
            expires_at_unix: request.expires_at_unix.as_i64(),
        })
    }

    fn clear(&self) -> Result<(), SessionCredentialError> {
        match session_entry()?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(_) => Err(SessionCredentialError::Unavailable),
        }
    }

    fn metadata(&self) -> Result<Option<StoredSessionMetadata>, SessionCredentialError> {
        self.load().map(|session| {
            session.map(|session| StoredSessionMetadata {
                expires_at_unix: session.expires_at_unix,
            })
        })
    }

    fn load(&self) -> Result<Option<StoredSession>, SessionCredentialError> {
        let serialized = match session_entry()?.get_secret() {
            Ok(secret) => Zeroizing::new(secret),
            Err(keyring::Error::NoEntry) => return Ok(None),
            Err(_) => return Err(SessionCredentialError::Unavailable),
        };
        decode_session(&serialized).map(Some)
    }
}

fn session_entry() -> Result<Entry, SessionCredentialError> {
    Entry::new(SESSION_CREDENTIAL_SERVICE, SESSION_CREDENTIAL_ACCOUNT)
        .map_err(|_| SessionCredentialError::Unavailable)
}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct SessionRecordRef<'a> {
    version: u8,
    access_token: &'a str,
    refresh_token: &'a str,
    expires_at_unix: i64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SessionRecord {
    version: u8,
    access_token: String,
    refresh_token: String,
    expires_at_unix: i64,
}

impl Drop for SessionRecord {
    fn drop(&mut self) {
        self.access_token.zeroize();
        self.refresh_token.zeroize();
    }
}

fn encode_session(
    request: &ValidatedStoreSessionRequest,
) -> Result<Zeroizing<Vec<u8>>, SessionCredentialError> {
    let record = SessionRecordRef {
        version: 1,
        access_token: request.access_token.expose(),
        refresh_token: request.refresh_token.expose(),
        expires_at_unix: request.expires_at_unix.as_i64(),
    };
    let serialized = Zeroizing::new(
        serde_json::to_vec(&record).map_err(|_| SessionCredentialError::Unavailable)?,
    );
    if serialized.len() > MAX_SESSION_CREDENTIAL_BYTES {
        return Err(SessionCredentialError::Rejected);
    }
    Ok(serialized)
}

fn decode_session(serialized: &[u8]) -> Result<StoredSession, SessionCredentialError> {
    if serialized.len() > MAX_SESSION_CREDENTIAL_BYTES {
        return Err(SessionCredentialError::Rejected);
    }
    let mut record: SessionRecord =
        serde_json::from_slice(serialized).map_err(|_| SessionCredentialError::Rejected)?;
    if record.version != 1 {
        return Err(SessionCredentialError::Rejected);
    }

    let access_token =
        SessionToken::new(mem::take(&mut record.access_token)).map_err(map_validation_error)?;
    let refresh_token =
        SessionToken::new(mem::take(&mut record.refresh_token)).map_err(map_validation_error)?;
    let expiry = UnixExpiry::new(record.expires_at_unix, 0).map_err(map_validation_error)?;
    drop(refresh_token);

    Ok(StoredSession {
        access_token,
        expires_at_unix: expiry.as_i64(),
    })
}

const fn map_validation_error(_error: SecurityError) -> SessionCredentialError {
    SessionCredentialError::Rejected
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::StoreSessionRequest;

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
    fn session_record_round_trip_exposes_metadata_only() {
        let request = valid_session();
        let serialized = encode_session(&request).unwrap();
        let decoded = decode_session(&serialized).unwrap();
        assert_eq!(decoded.expires_at_unix, 500);
        assert_eq!(decoded.access_token.expose(), "A".repeat(64));
        assert!(serialized.len() <= MAX_SESSION_CREDENTIAL_BYTES);
    }

    #[test]
    fn session_record_rejects_unknown_version_fields_and_oversize() {
        let unknown_version = br#"{"version":2,"access_token":"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA","refresh_token":"BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB","expires_at_unix":500}"#;
        assert_eq!(
            decode_session(unknown_version).map(|_| ()),
            Err(SessionCredentialError::Rejected)
        );

        let unknown_field = br#"{"version":1,"access_token":"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA","refresh_token":"BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB","expires_at_unix":500,"server":"https://evil.example"}"#;
        assert_eq!(
            decode_session(unknown_field).map(|_| ()),
            Err(SessionCredentialError::Rejected)
        );

        assert_eq!(
            decode_session(&vec![b'A'; MAX_SESSION_CREDENTIAL_BYTES + 1]).map(|_| ()),
            Err(SessionCredentialError::Rejected)
        );
    }

    #[test]
    fn session_record_revalidates_tokens_and_timestamp_after_keyring_load() {
        let malformed_token = br#"{"version":1,"access_token":"short","refresh_token":"BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB","expires_at_unix":500}"#;
        assert_eq!(
            decode_session(malformed_token).map(|_| ()),
            Err(SessionCredentialError::Rejected)
        );

        let invalid_expiry = br#"{"version":1,"access_token":"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA","refresh_token":"BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB","expires_at_unix":253402300800}"#;
        assert_eq!(
            decode_session(invalid_expiry).map(|_| ()),
            Err(SessionCredentialError::Rejected)
        );
    }

    #[test]
    fn os_store_debug_never_exposes_credential_identifiers() {
        assert_eq!(
            format!("{OsSessionCredentialStore:?}"),
            "OsSessionCredentialStore(<credential metadata redacted>)"
        );
    }
}
