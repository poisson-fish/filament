//! Platform credential-store selection for the shared packaged runtime.
//!
//! `keyring` compatibility mode configures desktop stores lazily, but
//! deliberately leaves iOS and Android without a default store. Mobile hosts
//! install the exact reviewed native store before any session or `SQLCipher` key
//! entry can be created. Failure aborts native startup; there is no fallback.

#[cfg(any(target_os = "android", target_os = "ios"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PlatformCredentialStoreError {
    Unavailable,
}

#[cfg(target_os = "android")]
pub(crate) fn initialize_platform_credential_store() -> Result<(), PlatformCredentialStoreError> {
    let store = android_native_keyring_store::Store::new()
        .map_err(|_| PlatformCredentialStoreError::Unavailable)?;
    keyring_core::set_default_store(store);
    Ok(())
}

#[cfg(target_os = "ios")]
pub(crate) fn initialize_platform_credential_store() -> Result<(), PlatformCredentialStoreError> {
    let store = apple_native_keyring_store::protected::Store::new()
        .map_err(|_| PlatformCredentialStoreError::Unavailable)?;
    keyring_core::set_default_store(store);
    Ok(())
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub(crate) const fn initialize_platform_credential_store() {
    // Desktop `keyring` compatibility mode selects Keychain, Credential
    // Manager, or Secret Service when the first native-only entry is opened.
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    #[test]
    fn desktop_keeps_lazy_native_store_selection() {
        initialize_platform_credential_store();
    }

    #[cfg(any(target_os = "android", target_os = "ios"))]
    #[test]
    fn initialization_failure_is_redacted() {
        assert_eq!(
            format!("{:?}", PlatformCredentialStoreError::Unavailable),
            "Unavailable"
        );
    }
}
