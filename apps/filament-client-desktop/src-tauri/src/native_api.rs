//! Bounded native REST transport for packaged-client E2EE bootstrap.
//!
//! The API origin is compile-time policy, never an IPC argument. Redirects are
//! disabled, TLS is mandatory, request/response bodies are capped, and every
//! response is strictly decoded before it can influence native identity state.

use std::{collections::HashSet, io::Read as _, time::Duration};

use filament_core::{DeviceCertificate, DeviceId, UserId, Username};
use filament_e2ee::{MlsDevice, PendingKeyPackageUpload};
use filament_protocol::{
    DeviceListResponse, KeyPackageEntry, PublishDeviceCertificateRequest,
    PublishDeviceCertificateResponse, UploadKeyPackagesRequest, UploadKeyPackagesResponse,
};
use reqwest::{
    blocking::{Client, Response},
    header::{HeaderValue, AUTHORIZATION, CONTENT_LENGTH, CONTENT_TYPE},
    redirect::Policy,
    StatusCode,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use url::Url;
use zeroize::Zeroizing;

use crate::SessionToken;

const DEFAULT_NATIVE_API_ORIGIN: &str = "https://api.filament.local";
const NATIVE_API_TIMEOUT: Duration = Duration::from_secs(7);
const MAX_NATIVE_API_REQUEST_BYTES: usize = 256 * 1024;
const MAX_NATIVE_API_RESPONSE_BYTES: usize = 256 * 1024;
const MAX_PROFILE_MARKDOWN_BYTES: usize = 2_000;
const MAX_PROFILE_MARKDOWN_TOKENS: usize = 4_096;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NativeApiError {
    Unavailable,
    Rejected,
}

pub(crate) trait NativeEnrollmentApi: Send + Sync + 'static {
    fn current_user(&self, access_token: &SessionToken) -> Result<UserId, NativeApiError>;

    fn list_devices(
        &self,
        access_token: &SessionToken,
        user_id: UserId,
    ) -> Result<DeviceListResponse, NativeApiError>;

    fn publish_device(
        &self,
        access_token: &SessionToken,
        device: &MlsDevice,
    ) -> Result<(), NativeApiError>;

    fn upload_keypackages(
        &self,
        access_token: &SessionToken,
        device_id: DeviceId,
        pending: &PendingKeyPackageUpload,
    ) -> Result<(), NativeApiError>;
}

pub(crate) struct ReqwestNativeEnrollmentApi {
    client: Client,
    origin: NativeApiOrigin,
}

impl core::fmt::Debug for ReqwestNativeEnrollmentApi {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("ReqwestNativeEnrollmentApi(<origin and credentials redacted>)")
    }
}

impl ReqwestNativeEnrollmentApi {
    pub(crate) fn from_build_config() -> Result<Self, NativeApiError> {
        let configured =
            option_env!("FILAMENT_NATIVE_API_ORIGIN").unwrap_or(DEFAULT_NATIVE_API_ORIGIN);
        let origin = NativeApiOrigin::parse(configured)?;
        let client = Client::builder()
            .https_only(true)
            .redirect(Policy::none())
            .connect_timeout(NATIVE_API_TIMEOUT)
            .timeout(NATIVE_API_TIMEOUT)
            .build()
            .map_err(|_| NativeApiError::Unavailable)?;
        Ok(Self { client, origin })
    }

    fn get<T: DeserializeOwned>(
        &self,
        access_token: &SessionToken,
        path: &str,
    ) -> Result<T, NativeApiError> {
        let response = self
            .client
            .get(self.origin.endpoint(path)?)
            .header(AUTHORIZATION, bearer_value(access_token)?)
            .send()
            .map_err(|_| NativeApiError::Unavailable)?;
        decode_response(response)
    }

    fn send_json<Request: Serialize, ResponseBody: DeserializeOwned>(
        &self,
        access_token: &SessionToken,
        method: reqwest::Method,
        path: &str,
        request: &Request,
    ) -> Result<ResponseBody, NativeApiError> {
        let encoded = serde_json::to_vec(request).map_err(|_| NativeApiError::Rejected)?;
        if encoded.is_empty() || encoded.len() > MAX_NATIVE_API_REQUEST_BYTES {
            return Err(NativeApiError::Rejected);
        }
        let response = self
            .client
            .request(method, self.origin.endpoint(path)?)
            .header(AUTHORIZATION, bearer_value(access_token)?)
            .header(CONTENT_TYPE, "application/json")
            .body(encoded)
            .send()
            .map_err(|_| NativeApiError::Unavailable)?;
        decode_response(response)
    }
}

impl NativeEnrollmentApi for ReqwestNativeEnrollmentApi {
    fn current_user(&self, access_token: &SessionToken) -> Result<UserId, NativeApiError> {
        let response: NativeMeResponse = self.get(access_token, "/auth/me")?;
        response.validate()
    }

    fn list_devices(
        &self,
        access_token: &SessionToken,
        user_id: UserId,
    ) -> Result<DeviceListResponse, NativeApiError> {
        let response: DeviceListResponse =
            self.get(access_token, &format!("/e2ee/users/{user_id}/devices"))?;
        if response.user_id != user_id.to_string() {
            return Err(NativeApiError::Rejected);
        }
        Ok(response)
    }

    fn publish_device(
        &self,
        access_token: &SessionToken,
        device: &MlsDevice,
    ) -> Result<(), NativeApiError> {
        let certificate = device.certificate();
        let request = PublishDeviceCertificateRequest {
            device_signature_pubkey: certificate.device_signature_pubkey.clone(),
            root_key_signature: certificate.root_key_signature.clone(),
            root_key_pub: device.root_key_public().to_vec(),
        };
        let response: PublishDeviceCertificateResponse = self.send_json(
            access_token,
            reqwest::Method::PUT,
            &format!("/e2ee/devices/{}", device.device_id()),
            &request,
        )?;
        if !response.published || response.device_id != device.device_id().to_string() {
            return Err(NativeApiError::Rejected);
        }
        Ok(())
    }

    fn upload_keypackages(
        &self,
        access_token: &SessionToken,
        device_id: DeviceId,
        pending: &PendingKeyPackageUpload,
    ) -> Result<(), NativeApiError> {
        let request = UploadKeyPackagesRequest {
            device_id: device_id.to_string(),
            key_packages: pending
                .packages
                .iter()
                .map(|package| KeyPackageEntry {
                    key_package_blob: package.blob.clone(),
                    is_last_resort: package.is_last_resort,
                })
                .collect(),
        };
        let response: UploadKeyPackagesResponse = self.send_json(
            access_token,
            reqwest::Method::POST,
            "/e2ee/keypackages",
            &request,
        )?;
        validate_upload_confirmation(&response, pending.packages.len())
    }
}

#[derive(Clone)]
struct NativeApiOrigin(Url);

impl NativeApiOrigin {
    fn parse(value: &str) -> Result<Self, NativeApiError> {
        let parsed = Url::parse(value).map_err(|_| NativeApiError::Rejected)?;
        if parsed.scheme() != "https"
            || parsed.host_str().is_none()
            || !parsed.username().is_empty()
            || parsed.password().is_some()
            || parsed.query().is_some()
            || parsed.fragment().is_some()
            || !matches!(parsed.path(), "" | "/")
        {
            return Err(NativeApiError::Rejected);
        }
        Ok(Self(parsed))
    }

    fn endpoint(&self, path: &str) -> Result<Url, NativeApiError> {
        if !path.starts_with('/') || path.contains('?') || path.contains('#') {
            return Err(NativeApiError::Rejected);
        }
        let mut endpoint = self.0.clone();
        endpoint.set_path(path);
        Ok(endpoint)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NativeMeResponse {
    user_id: String,
    username: String,
    about_markdown: String,
    about_markdown_tokens: Vec<serde_json::Value>,
    avatar_version: i64,
    banner_version: i64,
}

impl NativeMeResponse {
    fn validate(self) -> Result<UserId, NativeApiError> {
        if Username::try_from(self.username).is_err()
            || self.about_markdown.len() > MAX_PROFILE_MARKDOWN_BYTES
            || self.about_markdown_tokens.len() > MAX_PROFILE_MARKDOWN_TOKENS
            || self.avatar_version < 0
            || self.banner_version < 0
        {
            return Err(NativeApiError::Rejected);
        }
        UserId::try_from(self.user_id).map_err(|_| NativeApiError::Rejected)
    }
}

fn bearer_value(access_token: &SessionToken) -> Result<HeaderValue, NativeApiError> {
    let value = Zeroizing::new(format!("Bearer {}", access_token.expose()));
    let mut header = HeaderValue::from_str(&value).map_err(|_| NativeApiError::Rejected)?;
    header.set_sensitive(true);
    Ok(header)
}

fn decode_response<T: DeserializeOwned>(mut response: Response) -> Result<T, NativeApiError> {
    if !response.status().is_success() {
        return Err(map_status(response.status()));
    }
    if response
        .headers()
        .get(CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<usize>().ok())
        .is_some_and(|length| length > MAX_NATIVE_API_RESPONSE_BYTES)
    {
        return Err(NativeApiError::Rejected);
    }
    let limit = u64::try_from(MAX_NATIVE_API_RESPONSE_BYTES + 1)
        .map_err(|_| NativeApiError::Unavailable)?;
    let mut bytes = Vec::new();
    response
        .by_ref()
        .take(limit)
        .read_to_end(&mut bytes)
        .map_err(|_| NativeApiError::Unavailable)?;
    decode_json_bytes(&bytes)
}

fn decode_json_bytes<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, NativeApiError> {
    if bytes.is_empty() || bytes.len() > MAX_NATIVE_API_RESPONSE_BYTES {
        return Err(NativeApiError::Rejected);
    }
    serde_json::from_slice(bytes).map_err(|_| NativeApiError::Rejected)
}

fn map_status(status: StatusCode) -> NativeApiError {
    if status.is_server_error() {
        NativeApiError::Unavailable
    } else {
        NativeApiError::Rejected
    }
}

fn validate_upload_confirmation(
    response: &UploadKeyPackagesResponse,
    requested_count: usize,
) -> Result<(), NativeApiError> {
    if usize::try_from(response.stored_count)
        .is_ok_and(|stored_count| stored_count <= requested_count)
    {
        Ok(())
    } else {
        Err(NativeApiError::Rejected)
    }
}

pub(crate) fn verify_directory_device(
    user_id: UserId,
    device_id: DeviceId,
    expected: &DeviceCertificate,
    expected_root: &[u8; 32],
    response: &DeviceListResponse,
) -> Result<i64, NativeApiError> {
    verify_directory_root(user_id, expected_root, response)?;
    let entry = response
        .devices
        .iter()
        .find(|entry| entry.device_id == device_id.to_string())
        .ok_or(NativeApiError::Rejected)?;
    let signature_key: &[u8; 32] = entry
        .device_signature_pubkey
        .as_slice()
        .try_into()
        .map_err(|_| NativeApiError::Rejected)?;
    let root_signature: &[u8; 64] = entry
        .root_key_signature
        .as_slice()
        .try_into()
        .map_err(|_| NativeApiError::Rejected)?;
    let root_key: &[u8; 32] = entry
        .root_key_pub
        .as_slice()
        .try_into()
        .map_err(|_| NativeApiError::Rejected)?;
    filament_e2ee::verify_device_certificate(
        root_key,
        user_id,
        device_id,
        signature_key,
        root_signature,
    )
    .map_err(|_| NativeApiError::Rejected)?;
    if root_key != expected_root
        || entry.device_signature_pubkey != expected.device_signature_pubkey
        || entry.root_key_signature != expected.root_key_signature
        || entry.created_at_unix < 0
    {
        return Err(NativeApiError::Rejected);
    }
    Ok(entry.created_at_unix)
}

pub(crate) fn verify_directory_root(
    user_id: UserId,
    expected_root: &[u8; 32],
    response: &DeviceListResponse,
) -> Result<(), NativeApiError> {
    if response.user_id != user_id.to_string() || response.devices.is_empty() {
        return Err(NativeApiError::Rejected);
    }
    let mut device_ids = HashSet::with_capacity(response.devices.len());
    for entry in &response.devices {
        let device_id =
            DeviceId::try_from(entry.device_id.clone()).map_err(|_| NativeApiError::Rejected)?;
        if !device_ids.insert(device_id)
            || entry.tombstoned_at_unix.is_some()
            || !(0..=253_402_300_799).contains(&entry.created_at_unix)
        {
            return Err(NativeApiError::Rejected);
        }
        let signature_key: &[u8; 32] = entry
            .device_signature_pubkey
            .as_slice()
            .try_into()
            .map_err(|_| NativeApiError::Rejected)?;
        let root_signature: &[u8; 64] = entry
            .root_key_signature
            .as_slice()
            .try_into()
            .map_err(|_| NativeApiError::Rejected)?;
        let root_key: &[u8; 32] = entry
            .root_key_pub
            .as_slice()
            .try_into()
            .map_err(|_| NativeApiError::Rejected)?;
        if root_key != expected_root || entry.created_at_unix < 0 {
            return Err(NativeApiError::Rejected);
        }
        filament_e2ee::verify_device_certificate(
            root_key,
            user_id,
            device_id,
            signature_key,
            root_signature,
        )
        .map_err(|_| NativeApiError::Rejected)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use filament_e2ee::RootIdentityKey;

    #[test]
    fn native_origin_requires_one_compile_time_https_authority() {
        let origin = NativeApiOrigin::parse("https://chat.example:8443").unwrap();
        assert_eq!(
            origin.endpoint("/auth/me").unwrap().as_str(),
            "https://chat.example:8443/auth/me"
        );
        for invalid in [
            "http://chat.example",
            "https://user@chat.example",
            "https://chat.example/api",
            "https://chat.example?server=other",
            "file:///tmp/socket",
        ] {
            assert!(NativeApiOrigin::parse(invalid).is_err(), "{invalid}");
        }
        assert!(origin.endpoint("https://evil.example").is_err());
    }

    #[test]
    fn me_response_is_strict_and_bounded() {
        let user_id = UserId::new();
        let valid = format!(
            r#"{{"user_id":"{user_id}","username":"alice","about_markdown":"","about_markdown_tokens":[],"avatar_version":0,"banner_version":0}}"#
        );
        let parsed: NativeMeResponse = decode_json_bytes(valid.as_bytes()).unwrap();
        assert_eq!(parsed.validate(), Ok(user_id));

        let unknown = format!(
            r#"{{"user_id":"{user_id}","username":"alice","about_markdown":"","about_markdown_tokens":[],"avatar_version":0,"banner_version":0,"server_origin":"https://evil.example"}}"#
        );
        assert!(decode_json_bytes::<NativeMeResponse>(unknown.as_bytes()).is_err());
        assert!(decode_json_bytes::<NativeMeResponse>(&vec![
            b'A';
            MAX_NATIVE_API_RESPONSE_BYTES + 1
        ])
        .is_err());
    }

    #[test]
    fn api_debug_redacts_origin_and_credentials() {
        let api = ReqwestNativeEnrollmentApi::from_build_config().unwrap();
        assert_eq!(
            format!("{api:?}"),
            "ReqwestNativeEnrollmentApi(<origin and credentials redacted>)"
        );
        let token = SessionToken::new("A".repeat(64)).unwrap();
        assert!(bearer_value(&token).unwrap().is_sensitive());
    }

    #[test]
    fn upload_confirmation_cannot_claim_more_packages_than_requested() {
        for stored_count in [0_u32, 1, 11] {
            assert_eq!(
                validate_upload_confirmation(&UploadKeyPackagesResponse { stored_count }, 11),
                Ok(())
            );
        }
        assert_eq!(
            validate_upload_confirmation(&UploadKeyPackagesResponse { stored_count: 12 }, 11),
            Err(NativeApiError::Rejected)
        );
    }

    #[test]
    fn directory_verification_rejects_a_ghost_device_or_root_replacement() {
        let user_id = UserId::new();
        let root = RootIdentityKey::generate();
        let current = MlsDevice::generate(user_id, DeviceId::new(), &root).unwrap();
        let attacker_root = RootIdentityKey::generate();
        let ghost = MlsDevice::generate(user_id, DeviceId::new(), &attacker_root).unwrap();
        let response = DeviceListResponse {
            user_id: user_id.to_string(),
            devices: vec![device_info(&current), device_info(&ghost)],
        };
        assert_eq!(
            verify_directory_root(user_id, current.root_key_public(), &response),
            Err(NativeApiError::Rejected)
        );

        let duplicate = DeviceListResponse {
            user_id: user_id.to_string(),
            devices: vec![device_info(&current), device_info(&current)],
        };
        assert_eq!(
            verify_directory_root(user_id, current.root_key_public(), &duplicate),
            Err(NativeApiError::Rejected)
        );
    }

    fn device_info(device: &MlsDevice) -> filament_protocol::DeviceInfo {
        filament_protocol::DeviceInfo {
            device_id: device.device_id().to_string(),
            device_signature_pubkey: device.certificate().device_signature_pubkey.clone(),
            root_key_signature: device.certificate().root_key_signature.clone(),
            root_key_pub: device.root_key_public().to_vec(),
            created_at_unix: 1,
            tombstoned_at_unix: None,
        }
    }
}
