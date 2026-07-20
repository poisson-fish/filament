//! E2EE directory and `KeyPackage` Delivery Service endpoints.
//!
//! The server verifies public device certificates and applies shape/rate
//! limits, but never receives private key material or parses MLS interiors.

use std::{fmt::Write as _, net::SocketAddr};

use axum::{
    extract::{connect_info::ConnectInfo, Extension, Path, State},
    http::HeaderMap,
    Json,
};
use filament_core::{DeviceId, UserId};
use filament_e2ee::verify_device_certificate;
use filament_protocol::{
    ClaimKeyPackageRequest, ClaimKeyPackageResponse, DeviceInfo, DeviceListResponse,
    PublishDeviceCertificateRequest, PublishDeviceCertificateResponse, UploadKeyPackagesRequest,
    UploadKeyPackagesResponse,
};
use serde_json::json;
use sha2::{Digest, Sha256};
use sqlx::Row;

use crate::server::{
    auth::{
        authenticate, enforce_e2ee_device_publish_rate_limit,
        enforce_e2ee_keypackage_claim_rate_limit, extract_client_ip, now_unix,
    },
    core::AppState,
    errors::AuthFailure,
};

/// Publish a root-key-certified device for the authenticated user.
pub(crate) async fn publish_device_certificate(
    State(state): State<AppState>,
    headers: HeaderMap,
    connect_info: Option<Extension<ConnectInfo<SocketAddr>>>,
    Path(device_id): Path<String>,
    Json(payload): Json<PublishDeviceCertificateRequest>,
) -> Result<Json<PublishDeviceCertificateResponse>, AuthFailure> {
    let client_ip = extract_client_ip(
        &state,
        &headers,
        connect_info.as_ref().map(|value| value.0 .0.ip()),
    );
    let auth = authenticate(&state, &headers).await?;
    enforce_e2ee_device_publish_rate_limit(&state, client_ip, auth.user_id).await?;
    let device_id = DeviceId::try_from(device_id).map_err(|_| AuthFailure::InvalidRequest)?;
    let device_key: [u8; 32] = payload
        .device_signature_pubkey
        .as_slice()
        .try_into()
        .map_err(|_| AuthFailure::InvalidRequest)?;
    let root_signature: [u8; 64] = payload
        .root_key_signature
        .as_slice()
        .try_into()
        .map_err(|_| AuthFailure::InvalidRequest)?;
    let root_key: [u8; 32] = payload
        .root_key_pub
        .as_slice()
        .try_into()
        .map_err(|_| AuthFailure::InvalidRequest)?;
    verify_device_certificate(
        &root_key,
        auth.user_id,
        device_id,
        &device_key,
        &root_signature,
    )
    .map_err(|_| AuthFailure::InvalidRequest)?;

    let pool = state.db_pool.as_ref().ok_or(AuthFailure::Internal)?;
    let now = now_unix();
    let mut transaction = pool.begin().await.map_err(|_| AuthFailure::Internal)?;
    sqlx::query(
        "INSERT INTO e2ee_root_identities (user_id, root_key_pub, created_at_unix)
         VALUES ($1, $2, $3)
         ON CONFLICT (user_id) DO NOTHING",
    )
    .bind(auth.user_id.to_string())
    .bind(root_key.as_slice())
    .bind(now)
    .execute(&mut *transaction)
    .await
    .map_err(|_| AuthFailure::Internal)?;
    let pinned_root: Vec<u8> = sqlx::query_scalar(
        "SELECT root_key_pub FROM e2ee_root_identities WHERE user_id = $1 FOR UPDATE",
    )
    .bind(auth.user_id.to_string())
    .fetch_one(&mut *transaction)
    .await
    .map_err(|_| AuthFailure::Internal)?;
    if pinned_root.as_slice() != root_key {
        return Err(AuthFailure::Forbidden);
    }

    let inserted = sqlx::query(
        "INSERT INTO e2ee_device_certificates
            (device_id, user_id, device_sig_pubkey, root_key_sig, root_key_pub, created_at_unix)
         VALUES ($1, $2, $3, $4, $5, $6)
         ON CONFLICT (device_id) DO UPDATE SET
            device_sig_pubkey = EXCLUDED.device_sig_pubkey,
            root_key_sig = EXCLUDED.root_key_sig,
            root_key_pub = EXCLUDED.root_key_pub,
            created_at_unix = EXCLUDED.created_at_unix,
            tombstoned_at_unix = NULL
         WHERE e2ee_device_certificates.user_id = EXCLUDED.user_id",
    )
    .bind(device_id.to_string())
    .bind(auth.user_id.to_string())
    .bind(device_key.as_slice())
    .bind(root_signature.as_slice())
    .bind(root_key.as_slice())
    .bind(now)
    .execute(&mut *transaction)
    .await
    .map_err(|_| AuthFailure::Internal)?;
    if inserted.rows_affected() != 1 {
        return Err(AuthFailure::Forbidden);
    }
    sqlx::query(
        "INSERT INTO e2ee_audit_log (action, user_id, device_id, created_at_unix)
         VALUES ('device_publish', $1, $2, $3)",
    )
    .bind(auth.user_id.to_string())
    .bind(device_id.to_string())
    .bind(now)
    .execute(&mut *transaction)
    .await
    .map_err(|_| AuthFailure::Internal)?;
    transaction
        .commit()
        .await
        .map_err(|_| AuthFailure::Internal)?;

    Ok(Json(PublishDeviceCertificateResponse {
        device_id: device_id.to_string(),
        published: true,
    }))
}

/// List active public device certificates for a user.
pub(crate) async fn list_user_devices(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(user_id): Path<String>,
) -> Result<Json<DeviceListResponse>, AuthFailure> {
    let _auth = authenticate(&state, &headers).await?;
    let user_id = UserId::try_from(user_id).map_err(|_| AuthFailure::InvalidRequest)?;
    let pool = state.db_pool.as_ref().ok_or(AuthFailure::Internal)?;
    let rows = sqlx::query(
        "SELECT device_id, device_sig_pubkey, root_key_sig, root_key_pub, created_at_unix
         FROM e2ee_device_certificates
         WHERE user_id = $1 AND tombstoned_at_unix IS NULL
         ORDER BY created_at_unix ASC, device_id ASC
         LIMIT 100",
    )
    .bind(user_id.to_string())
    .fetch_all(pool)
    .await
    .map_err(|_| AuthFailure::Internal)?;
    let devices = rows
        .into_iter()
        .map(|row| {
            Ok(DeviceInfo {
                device_id: row
                    .try_get("device_id")
                    .map_err(|_| AuthFailure::Internal)?,
                device_signature_pubkey: row
                    .try_get("device_sig_pubkey")
                    .map_err(|_| AuthFailure::Internal)?,
                root_key_signature: row
                    .try_get("root_key_sig")
                    .map_err(|_| AuthFailure::Internal)?,
                root_key_pub: row
                    .try_get("root_key_pub")
                    .map_err(|_| AuthFailure::Internal)?,
                created_at_unix: row
                    .try_get("created_at_unix")
                    .map_err(|_| AuthFailure::Internal)?,
                tombstoned_at_unix: None,
            })
        })
        .collect::<Result<Vec<_>, AuthFailure>>()?;
    Ok(Json(DeviceListResponse {
        user_id: user_id.to_string(),
        devices,
    }))
}

/// Upload a bounded batch of opaque `KeyPackage` blobs for an owned device.
pub(crate) async fn upload_keypackages(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<UploadKeyPackagesRequest>,
) -> Result<Json<UploadKeyPackagesResponse>, AuthFailure> {
    let auth = authenticate(&state, &headers).await?;
    let device_id =
        DeviceId::try_from(payload.device_id).map_err(|_| AuthFailure::InvalidRequest)?;
    if payload
        .key_packages
        .iter()
        .filter(|package| package.is_last_resort)
        .count()
        > 1
    {
        return Err(AuthFailure::InvalidRequest);
    }
    let pool = state.db_pool.as_ref().ok_or(AuthFailure::Internal)?;
    let now = now_unix();
    let mut transaction = pool.begin().await.map_err(|_| AuthFailure::Internal)?;
    let owns_device = sqlx::query(
        "SELECT 1 FROM e2ee_device_certificates
         WHERE user_id = $1 AND device_id = $2 AND tombstoned_at_unix IS NULL
         FOR UPDATE",
    )
    .bind(auth.user_id.to_string())
    .bind(device_id.to_string())
    .fetch_optional(&mut *transaction)
    .await
    .map_err(|_| AuthFailure::Internal)?;
    if owns_device.is_none() {
        return Err(AuthFailure::NotFound);
    }
    let current_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM e2ee_keypackages
         WHERE device_id = $1 AND claimed_at_unix IS NULL",
    )
    .bind(device_id.to_string())
    .fetch_one(&mut *transaction)
    .await
    .map_err(|_| AuthFailure::Internal)?;
    let requested = i64::try_from(payload.key_packages.len()).map_err(|_| AuthFailure::Internal)?;
    let max_pool = i64::try_from(state.runtime.e2ee_max_keypackage_pool_size)
        .map_err(|_| AuthFailure::Internal)?;
    if current_count.saturating_add(requested) > max_pool {
        return Err(AuthFailure::QuotaExceeded);
    }

    let mut stored_count = 0_u32;
    for package in payload.key_packages {
        let inserted = sqlx::query(
            "INSERT INTO e2ee_keypackages
                (device_id, key_package_hash, key_package_blob, is_last_resort, created_at_unix)
             VALUES ($1, $2, $3, $4, $5)
             ON CONFLICT (device_id, key_package_hash) DO NOTHING",
        )
        .bind(device_id.to_string())
        .bind(sha256_hex(&package.key_package_blob))
        .bind(package.key_package_blob)
        .bind(package.is_last_resort)
        .bind(now)
        .execute(&mut *transaction)
        .await
        .map_err(|_| AuthFailure::InvalidRequest)?;
        stored_count = stored_count.saturating_add(
            u32::try_from(inserted.rows_affected()).map_err(|_| AuthFailure::Internal)?,
        );
    }
    sqlx::query(
        "INSERT INTO e2ee_audit_log
            (action, user_id, device_id, metadata_json, created_at_unix)
         VALUES ('keypackage_upload', $1, $2, $3::jsonb, $4)",
    )
    .bind(auth.user_id.to_string())
    .bind(device_id.to_string())
    .bind(json!({ "stored_count": stored_count }).to_string())
    .bind(now)
    .execute(&mut *transaction)
    .await
    .map_err(|_| AuthFailure::Internal)?;
    transaction
        .commit()
        .await
        .map_err(|_| AuthFailure::Internal)?;
    Ok(Json(UploadKeyPackagesResponse { stored_count }))
}

/// Atomically claim one opaque `KeyPackage` for a certified target device.
pub(crate) async fn claim_keypackage(
    State(state): State<AppState>,
    headers: HeaderMap,
    connect_info: Option<Extension<ConnectInfo<SocketAddr>>>,
    Json(payload): Json<ClaimKeyPackageRequest>,
) -> Result<Json<ClaimKeyPackageResponse>, AuthFailure> {
    let client_ip = extract_client_ip(
        &state,
        &headers,
        connect_info.as_ref().map(|value| value.0 .0.ip()),
    );
    let auth = authenticate(&state, &headers).await?;
    let target_user_id =
        UserId::try_from(payload.target_user_id).map_err(|_| AuthFailure::InvalidRequest)?;
    let target_device_id = payload
        .target_device_id
        .map(DeviceId::try_from)
        .transpose()
        .map_err(|_| AuthFailure::InvalidRequest)?;
    enforce_e2ee_keypackage_claim_rate_limit(&state, client_ip, auth.user_id, target_device_id)
        .await?;
    let pool = state.db_pool.as_ref().ok_or(AuthFailure::Internal)?;
    let now = now_unix();
    let mut transaction = pool.begin().await.map_err(|_| AuthFailure::Internal)?;
    let target_device = target_device_id.map(|value| value.to_string());
    let row = sqlx::query(
        "WITH candidate AS (
            SELECT kp.device_id, kp.key_package_hash
            FROM e2ee_keypackages kp
            JOIN e2ee_device_certificates dc ON dc.device_id = kp.device_id
            WHERE dc.user_id = $1
              AND dc.tombstoned_at_unix IS NULL
              AND kp.claimed_at_unix IS NULL
              AND ($2::TEXT IS NULL OR kp.device_id = $2)
            ORDER BY kp.is_last_resort ASC, kp.created_at_unix ASC
            FOR UPDATE OF kp SKIP LOCKED
            LIMIT 1
         )
         UPDATE e2ee_keypackages kp
         SET claimed_at_unix = $3
         FROM candidate
         WHERE kp.device_id = candidate.device_id
           AND kp.key_package_hash = candidate.key_package_hash
           AND kp.claimed_at_unix IS NULL
         RETURNING kp.device_id, kp.key_package_blob, kp.is_last_resort",
    )
    .bind(target_user_id.to_string())
    .bind(target_device)
    .bind(now)
    .fetch_optional(&mut *transaction)
    .await
    .map_err(|_| AuthFailure::Internal)?
    .ok_or(AuthFailure::NotFound)?;
    let device_id: String = row
        .try_get("device_id")
        .map_err(|_| AuthFailure::Internal)?;
    let key_package_blob = row
        .try_get("key_package_blob")
        .map_err(|_| AuthFailure::Internal)?;
    let is_last_resort = row
        .try_get("is_last_resort")
        .map_err(|_| AuthFailure::Internal)?;
    sqlx::query(
        "INSERT INTO e2ee_audit_log
            (action, user_id, device_id, metadata_json, created_at_unix)
         VALUES ('keypackage_claim', $1, $2, $3::jsonb, $4)",
    )
    .bind(auth.user_id.to_string())
    .bind(&device_id)
    .bind(
        json!({
            "target_user_id": target_user_id.to_string(),
            "is_last_resort": is_last_resort,
        })
        .to_string(),
    )
    .bind(now)
    .execute(&mut *transaction)
    .await
    .map_err(|_| AuthFailure::Internal)?;
    transaction
        .commit()
        .await
        .map_err(|_| AuthFailure::Internal)?;
    Ok(Json(ClaimKeyPackageResponse {
        device_id,
        key_package_blob,
        is_last_resort,
    }))
}

fn sha256_hex(bytes: &[u8]) -> String {
    let hash = Sha256::digest(bytes);
    let mut encoded = String::with_capacity(hash.len() * 2);
    for byte in hash {
        let _ = write!(encoded, "{byte:02x}");
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_hex_is_stable_and_fixed_length() {
        assert_eq!(
            sha256_hex(b"test"),
            "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08"
        );
    }
}
