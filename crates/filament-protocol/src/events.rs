use std::{collections::BTreeSet, sync::OnceLock};

use serde::{Deserialize, Serialize};

use crate::validate_event_type;

const GATEWAY_EVENT_MANIFEST_JSON: &str = include_str!("events/gateway_events_manifest.json");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GatewayEventScope {
    Connection,
    Channel,
    Guild,
    User,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GatewayEventLifecycle {
    #[default]
    Active,
    Deprecated,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GatewayEventManifestEntry {
    pub event_type: String,
    pub schema_version: u16,
    pub scope: GatewayEventScope,
    #[serde(default)]
    pub lifecycle: GatewayEventLifecycle,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub migration: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GatewayEventManifest {
    pub events: Vec<GatewayEventManifestEntry>,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum GatewayEventManifestError {
    #[error("invalid manifest json")]
    InvalidJson,
    #[error("gateway event type `{0}` failed identifier validation")]
    InvalidEventType(String),
    #[error("duplicate gateway event type `{0}` in manifest")]
    DuplicateEventType(String),
    #[error("gateway event `{event_type}` has invalid schema version {schema_version}")]
    InvalidSchemaVersion {
        event_type: String,
        schema_version: u16,
    },
    #[error("gateway event `{event_type}` is deprecated and must include a migration note")]
    MissingDeprecatedMigration { event_type: String },
    #[error("gateway event `{event_type}` is active but includes migration note")]
    UnexpectedActiveMigration { event_type: String },
}

impl From<serde_json::Error> for GatewayEventManifestError {
    fn from(_: serde_json::Error) -> Self {
        Self::InvalidJson
    }
}

/// Parse and validate the machine-readable gateway event manifest.
///
/// # Errors
/// Returns [`GatewayEventManifestError`] when JSON is invalid or when manifest
/// invariants fail (identifier format, duplicates, schema version, lifecycle metadata).
pub fn parse_gateway_event_manifest(
    json: &str,
) -> Result<GatewayEventManifest, GatewayEventManifestError> {
    let manifest: GatewayEventManifest = serde_json::from_str(json)?;
    validate_gateway_event_manifest(&manifest)?;
    Ok(manifest)
}

/// Return the embedded protocol gateway event manifest.
///
/// # Panics
/// Panics if the embedded manifest file is invalid. This is a startup-time
/// invariant and should be prevented by tests and CI parity checks.
#[must_use]
pub fn gateway_event_manifest() -> &'static GatewayEventManifest {
    static MANIFEST: OnceLock<GatewayEventManifest> = OnceLock::new();
    MANIFEST.get_or_init(|| {
        parse_gateway_event_manifest(GATEWAY_EVENT_MANIFEST_JSON)
            .expect("gateway event manifest must parse and validate")
    })
}

fn validate_gateway_event_manifest(
    manifest: &GatewayEventManifest,
) -> Result<(), GatewayEventManifestError> {
    let mut seen = BTreeSet::new();

    for entry in &manifest.events {
        validate_event_type(&entry.event_type)
            .map_err(|_| GatewayEventManifestError::InvalidEventType(entry.event_type.clone()))?;

        if entry.schema_version == 0 {
            return Err(GatewayEventManifestError::InvalidSchemaVersion {
                event_type: entry.event_type.clone(),
                schema_version: entry.schema_version,
            });
        }

        if !seen.insert(entry.event_type.clone()) {
            return Err(GatewayEventManifestError::DuplicateEventType(
                entry.event_type.clone(),
            ));
        }

        match entry.lifecycle {
            GatewayEventLifecycle::Active if entry.migration.is_some() => {
                return Err(GatewayEventManifestError::UnexpectedActiveMigration {
                    event_type: entry.event_type.clone(),
                });
            }
            GatewayEventLifecycle::Deprecated
                if entry
                    .migration
                    .as_ref()
                    .is_none_or(|value| value.trim().is_empty()) =>
            {
                return Err(GatewayEventManifestError::MissingDeprecatedMigration {
                    event_type: entry.event_type.clone(),
                });
            }
            GatewayEventLifecycle::Active | GatewayEventLifecycle::Deprecated => {}
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{gateway_event_manifest, parse_gateway_event_manifest, GatewayEventManifestError};

    #[test]
    fn embedded_manifest_parses_and_is_non_empty() {
        let manifest = gateway_event_manifest();
        assert!(!manifest.events.is_empty());
    }

    #[test]
    fn parse_rejects_deprecated_event_without_migration() {
        let error = parse_gateway_event_manifest(
            r#"{"events":[{"event_type":"message_create","schema_version":1,"scope":"channel","lifecycle":"deprecated"}]}"#,
        )
        .expect_err("deprecated event without migration must fail");

        assert_eq!(
            error,
            GatewayEventManifestError::MissingDeprecatedMigration {
                event_type: String::from("message_create"),
            }
        );
    }

    #[test]
    fn parse_rejects_duplicate_event_types() {
        let error = parse_gateway_event_manifest(
            r#"{"events":[{"event_type":"message_create","schema_version":1,"scope":"channel"},{"event_type":"message_create","schema_version":1,"scope":"channel"}]}"#,
        )
        .expect_err("duplicate event types must fail");

        assert_eq!(
            error,
            GatewayEventManifestError::DuplicateEventType(String::from("message_create"))
        );
    }
}
