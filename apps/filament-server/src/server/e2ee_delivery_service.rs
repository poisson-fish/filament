//! Stable key custody for the MLS Delivery Service external sender.

use std::{io::Read, path::Path, sync::Arc};

use anyhow::{anyhow, Context};
use filament_e2ee::{DeliveryServiceSigner, DELIVERY_SERVICE_SEED_BYTES};

/// Load a stable raw Ed25519 seed without following a final-component symlink.
///
/// On Unix, the already-open file descriptor is checked for regular-file
/// type, single-link ownership, current effective UID, private permissions,
/// and exact length. This avoids validating one path and reading another.
#[cfg(unix)]
pub(crate) fn load_delivery_service_signer(
    path: &Path,
) -> anyhow::Result<Arc<DeliveryServiceSigner>> {
    use std::os::unix::fs::MetadataExt;

    use rustix::fs::{open, Mode, OFlags};

    if !path.is_absolute() {
        return Err(anyhow!(
            "E2EE Delivery Service key file path must be absolute"
        ));
    }
    let fd = open(
        path,
        OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
        Mode::empty(),
    )
    .with_context(|| {
        format!(
            "failed to open E2EE Delivery Service key file {}",
            path.display()
        )
    })?;
    let mut file = std::fs::File::from(fd);
    let metadata = file
        .metadata()
        .context("failed to inspect E2EE Delivery Service key file")?;
    if !metadata.file_type().is_file()
        || metadata.nlink() != 1
        || metadata.uid() != rustix::process::geteuid().as_raw()
        || metadata.mode() & 0o177 != 0
        || metadata.len() != DELIVERY_SERVICE_SEED_BYTES as u64
    {
        return Err(anyhow!(
            "E2EE Delivery Service key must be a current-user-owned, single-link, 0400/0600 regular file containing exactly {DELIVERY_SERVICE_SEED_BYTES} raw bytes"
        ));
    }
    let mut seed = [0_u8; DELIVERY_SERVICE_SEED_BYTES];
    file.read_exact(&mut seed)
        .context("failed to read E2EE Delivery Service key file")?;
    DeliveryServiceSigner::from_seed(seed)
        .map(Arc::new)
        .map_err(|_| anyhow!("E2EE Delivery Service key is invalid"))
}

/// Secure external-sender key files currently require Unix descriptor and
/// permission semantics. Fail closed on other server targets.
#[cfg(not(unix))]
pub(crate) fn load_delivery_service_signer(
    _path: &Path,
) -> anyhow::Result<Arc<DeliveryServiceSigner>> {
    Err(anyhow!(
        "E2EE Delivery Service key custody is unsupported on this server platform"
    ))
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::{
        fs,
        os::unix::fs::{symlink, PermissionsExt},
    };
    use ulid::Ulid;

    fn test_path(suffix: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("filament-e2ee-ds-{}-{suffix}", Ulid::new()))
    }

    fn write_key(path: &Path, bytes: &[u8], mode: u32) {
        fs::write(path, bytes).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(mode)).unwrap();
    }

    #[test]
    fn loads_only_private_exact_single_link_key_files() {
        let path = test_path("key");
        write_key(&path, &[0x55; DELIVERY_SERVICE_SEED_BYTES], 0o600);
        let first = load_delivery_service_signer(&path).unwrap();
        let second = load_delivery_service_signer(&path).unwrap();
        assert_eq!(first.identity(), second.identity());

        fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();
        assert!(load_delivery_service_signer(&path).is_err());
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn rejects_wrong_length_symlinks_and_hardlinks() {
        assert!(load_delivery_service_signer(Path::new("relative-key")).is_err());

        let short = test_path("short");
        write_key(&short, &[0x11; DELIVERY_SERVICE_SEED_BYTES - 1], 0o600);
        assert!(load_delivery_service_signer(&short).is_err());
        fs::remove_file(short).unwrap();

        let target = test_path("target");
        let symlink_path = test_path("symlink");
        write_key(&target, &[0x22; DELIVERY_SERVICE_SEED_BYTES], 0o600);
        symlink(&target, &symlink_path).unwrap();
        assert!(load_delivery_service_signer(&symlink_path).is_err());
        fs::remove_file(symlink_path).unwrap();

        let hardlink = test_path("hardlink");
        fs::hard_link(&target, &hardlink).unwrap();
        assert!(load_delivery_service_signer(&target).is_err());
        fs::remove_file(hardlink).unwrap();
        fs::remove_file(target).unwrap();
    }
}
