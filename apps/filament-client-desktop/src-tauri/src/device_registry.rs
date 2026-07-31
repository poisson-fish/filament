//! Native-only account-to-device bindings for packaged clients.
//!
//! The registry contains public ULIDs only, but it is integrity-sensitive:
//! webview input must never select which encrypted store or device identity is
//! opened. One bounded, versioned record is therefore kept under fixed
//! platform-credential identifiers.

use filament_core::{DeviceId, UserId};
use keyring::Entry;
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

const DEVICE_REGISTRY_SERVICE: &str = "com.filament.desktop.device-registry";
const DEVICE_REGISTRY_ACCOUNT: &str = "account-device-bindings-v1";
const DEVICE_REGISTRY_VERSION: u8 = 1;
const MAX_DEVICE_REGISTRY_ACCOUNTS: usize = 16;
const MAX_DEVICE_REGISTRY_BYTES: usize = 2 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DeviceRegistryError {
    Unavailable,
    Rejected,
}

pub(crate) trait DeviceRegistry: Send + Sync + 'static {
    fn device_for(&self, user_id: UserId) -> Result<Option<DeviceId>, DeviceRegistryError>;

    fn bind(&self, user_id: UserId, device_id: DeviceId) -> Result<(), DeviceRegistryError>;
}

pub(crate) struct OsDeviceRegistry;

impl core::fmt::Debug for OsDeviceRegistry {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("OsDeviceRegistry(<credential metadata redacted>)")
    }
}

impl DeviceRegistry for OsDeviceRegistry {
    fn device_for(&self, user_id: UserId) -> Result<Option<DeviceId>, DeviceRegistryError> {
        let registry = load_registry()?;
        Ok(registry
            .bindings
            .iter()
            .find(|binding| binding.user_id == user_id)
            .map(|binding| binding.device_id))
    }

    fn bind(&self, user_id: UserId, device_id: DeviceId) -> Result<(), DeviceRegistryError> {
        let mut registry = load_registry()?;
        if let Some(existing) = registry
            .bindings
            .iter()
            .find(|binding| binding.user_id == user_id)
        {
            return if existing.device_id == device_id {
                Ok(())
            } else {
                Err(DeviceRegistryError::Rejected)
            };
        }
        if registry.bindings.len() >= MAX_DEVICE_REGISTRY_ACCOUNTS {
            return Err(DeviceRegistryError::Rejected);
        }
        registry.bindings.push(DeviceBinding { user_id, device_id });
        registry
            .bindings
            .sort_by_key(|binding| binding.user_id.to_string());
        store_registry(&registry)
    }
}

#[derive(Default)]
struct DeviceRegistryRecord {
    bindings: Vec<DeviceBinding>,
}

#[derive(Clone, Copy)]
struct DeviceBinding {
    user_id: UserId,
    device_id: DeviceId,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DeviceRegistryWire {
    version: u8,
    bindings: Vec<DeviceBindingWire>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DeviceBindingWire {
    user_id: String,
    device_id: String,
}

fn registry_entry() -> Result<Entry, DeviceRegistryError> {
    Entry::new(DEVICE_REGISTRY_SERVICE, DEVICE_REGISTRY_ACCOUNT)
        .map_err(|_| DeviceRegistryError::Unavailable)
}

fn load_registry() -> Result<DeviceRegistryRecord, DeviceRegistryError> {
    let encoded = match registry_entry()?.get_secret() {
        Ok(encoded) => Zeroizing::new(encoded),
        Err(keyring::Error::NoEntry) => return Ok(DeviceRegistryRecord::default()),
        Err(_) => return Err(DeviceRegistryError::Unavailable),
    };
    decode_registry(&encoded)
}

fn store_registry(registry: &DeviceRegistryRecord) -> Result<(), DeviceRegistryError> {
    let wire = DeviceRegistryWire {
        version: DEVICE_REGISTRY_VERSION,
        bindings: registry
            .bindings
            .iter()
            .map(|binding| DeviceBindingWire {
                user_id: binding.user_id.to_string(),
                device_id: binding.device_id.to_string(),
            })
            .collect(),
    };
    let encoded =
        Zeroizing::new(serde_json::to_vec(&wire).map_err(|_| DeviceRegistryError::Unavailable)?);
    if encoded.is_empty() || encoded.len() > MAX_DEVICE_REGISTRY_BYTES {
        return Err(DeviceRegistryError::Rejected);
    }
    registry_entry()?
        .set_secret(&encoded)
        .map_err(|_| DeviceRegistryError::Unavailable)
}

fn decode_registry(encoded: &[u8]) -> Result<DeviceRegistryRecord, DeviceRegistryError> {
    if encoded.is_empty() || encoded.len() > MAX_DEVICE_REGISTRY_BYTES {
        return Err(DeviceRegistryError::Rejected);
    }
    let wire: DeviceRegistryWire =
        serde_json::from_slice(encoded).map_err(|_| DeviceRegistryError::Rejected)?;
    if wire.version != DEVICE_REGISTRY_VERSION || wire.bindings.len() > MAX_DEVICE_REGISTRY_ACCOUNTS
    {
        return Err(DeviceRegistryError::Rejected);
    }
    let mut bindings = Vec::with_capacity(wire.bindings.len());
    for binding in wire.bindings {
        let user_id =
            UserId::try_from(binding.user_id).map_err(|_| DeviceRegistryError::Rejected)?;
        let device_id =
            DeviceId::try_from(binding.device_id).map_err(|_| DeviceRegistryError::Rejected)?;
        if bindings
            .iter()
            .any(|existing: &DeviceBinding| existing.user_id == user_id)
        {
            return Err(DeviceRegistryError::Rejected);
        }
        bindings.push(DeviceBinding { user_id, device_id });
    }
    Ok(DeviceRegistryRecord { bindings })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_round_trip_is_strict_bounded_and_canonical() {
        let user_id = UserId::new();
        let device_id = DeviceId::new();
        let record = DeviceRegistryWire {
            version: DEVICE_REGISTRY_VERSION,
            bindings: vec![DeviceBindingWire {
                user_id: user_id.to_string(),
                device_id: device_id.to_string(),
            }],
        };
        let encoded = serde_json::to_vec(&record).unwrap();
        let decoded = decode_registry(&encoded).unwrap();
        assert_eq!(decoded.bindings[0].user_id, user_id);
        assert_eq!(decoded.bindings[0].device_id, device_id);

        assert_eq!(
            decode_registry(
                br#"{"version":1,"bindings":[],"credential_account":"attacker-selected"}"#
            )
            .map(|_| ()),
            Err(DeviceRegistryError::Rejected)
        );
        assert_eq!(
            decode_registry(&vec![b'A'; MAX_DEVICE_REGISTRY_BYTES + 1]).map(|_| ()),
            Err(DeviceRegistryError::Rejected)
        );
    }

    #[test]
    fn registry_rejects_duplicate_users_and_unknown_versions() {
        let user_id = UserId::new();
        let first = DeviceId::new();
        let second = DeviceId::new();
        let duplicate = serde_json::to_vec(&DeviceRegistryWire {
            version: DEVICE_REGISTRY_VERSION,
            bindings: vec![
                DeviceBindingWire {
                    user_id: user_id.to_string(),
                    device_id: first.to_string(),
                },
                DeviceBindingWire {
                    user_id: user_id.to_string(),
                    device_id: second.to_string(),
                },
            ],
        })
        .unwrap();
        assert_eq!(
            decode_registry(&duplicate).map(|_| ()),
            Err(DeviceRegistryError::Rejected)
        );
        assert_eq!(
            decode_registry(br#"{"version":2,"bindings":[]}"#).map(|_| ()),
            Err(DeviceRegistryError::Rejected)
        );
    }

    #[test]
    fn os_registry_debug_redacts_fixed_credential_identifiers() {
        assert_eq!(
            format!("{OsDeviceRegistry:?}"),
            "OsDeviceRegistry(<credential metadata redacted>)"
        );
    }
}
