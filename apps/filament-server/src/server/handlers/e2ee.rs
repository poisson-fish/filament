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
    PublishDeviceCertificateRequest, PublishDeviceCertificateResponse, RemoveDeviceResponse,
    UploadKeyPackagesRequest, UploadKeyPackagesResponse,
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
    gateway_events,
    metrics::record_gateway_event_serialize_error,
    realtime::broadcast_user_event,
};

const KEYPACKAGE_LOW_WATER_MARK: u32 = 10;

type CertificateFields = ([u8; 32], [u8; 64], [u8; 32]);

fn validate_certificate_fields(
    payload: &PublishDeviceCertificateRequest,
) -> Result<CertificateFields, AuthFailure> {
    let device_key = payload
        .device_signature_pubkey
        .as_slice()
        .try_into()
        .map_err(|_| AuthFailure::InvalidRequest)?;
    let root_signature = payload
        .root_key_signature
        .as_slice()
        .try_into()
        .map_err(|_| AuthFailure::InvalidRequest)?;
    let root_key = payload
        .root_key_pub
        .as_slice()
        .try_into()
        .map_err(|_| AuthFailure::InvalidRequest)?;
    Ok((device_key, root_signature, root_key))
}

fn keypackage_low_water_mark(max_pool_size: usize) -> Result<u32, AuthFailure> {
    let max_pool_size = u32::try_from(max_pool_size).map_err(|_| AuthFailure::Internal)?;
    Ok(KEYPACKAGE_LOW_WATER_MARK.min(max_pool_size))
}

async fn active_device_count(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: UserId,
) -> Result<u32, AuthFailure> {
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM e2ee_device_certificates
         WHERE user_id = $1 AND tombstoned_at_unix IS NULL",
    )
    .bind(user_id.to_string())
    .fetch_one(&mut **transaction)
    .await
    .map_err(|_| AuthFailure::Internal)?;
    u32::try_from(count).map_err(|_| AuthFailure::Internal)
}

async fn unclaimed_keypackage_count(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    device_id: &str,
) -> Result<u32, AuthFailure> {
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM e2ee_keypackages
         WHERE device_id = $1 AND claimed_at_unix IS NULL",
    )
    .bind(device_id)
    .fetch_one(&mut **transaction)
    .await
    .map_err(|_| AuthFailure::Internal)?;
    u32::try_from(count).map_err(|_| AuthFailure::Internal)
}

async fn record_device_publish_audit(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: UserId,
    device_id: DeviceId,
    created_at_unix: i64,
) -> Result<(), AuthFailure> {
    sqlx::query(
        "INSERT INTO e2ee_audit_log (action, user_id, device_id, created_at_unix)
         VALUES ('device_publish', $1, $2, $3)",
    )
    .bind(user_id.to_string())
    .bind(device_id.to_string())
    .bind(created_at_unix)
    .execute(&mut **transaction)
    .await
    .map_err(|_| AuthFailure::Internal)?;
    Ok(())
}

async fn record_device_rotation_audit(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: UserId,
    device_id: DeviceId,
    deleted_keypackage_count: u32,
    created_at_unix: i64,
) -> Result<(), AuthFailure> {
    sqlx::query(
        "INSERT INTO e2ee_audit_log
            (action, user_id, device_id, metadata_json, created_at_unix)
         VALUES ('device_rotate', $1, $2, $3::jsonb, $4)",
    )
    .bind(user_id.to_string())
    .bind(device_id.to_string())
    .bind(json!({ "deleted_keypackage_count": deleted_keypackage_count }).to_string())
    .bind(created_at_unix)
    .execute(&mut **transaction)
    .await
    .map_err(|_| AuthFailure::Internal)?;
    Ok(())
}

async fn current_device_key_for_update(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: UserId,
    device_id: DeviceId,
) -> Result<Option<Vec<u8>>, AuthFailure> {
    sqlx::query_scalar(
        "SELECT device_sig_pubkey FROM e2ee_device_certificates
         WHERE device_id = $1
           AND user_id = $2
           AND tombstoned_at_unix IS NULL
         FOR UPDATE",
    )
    .bind(device_id.to_string())
    .bind(user_id.to_string())
    .fetch_optional(&mut **transaction)
    .await
    .map_err(|_| AuthFailure::Internal)
}

async fn invalidate_rotated_device_keypackages(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: UserId,
    device_id: DeviceId,
    previous_device_key: Option<&[u8]>,
    device_key: &[u8; 32],
    created_at_unix: i64,
) -> Result<(), AuthFailure> {
    if previous_device_key.is_none_or(|previous| previous == device_key) {
        return Ok(());
    }

    // A KeyPackage credential is bound to the device signing key. Once that
    // key rotates, every package created under the previous key must become
    // unclaimable in the same transaction as the certificate swap.
    let deleted = sqlx::query(
        "DELETE FROM e2ee_keypackages
         WHERE device_id = $1 AND claimed_at_unix IS NULL",
    )
    .bind(device_id.to_string())
    .execute(&mut **transaction)
    .await
    .map_err(|_| AuthFailure::Internal)?;
    let deleted_keypackage_count =
        u32::try_from(deleted.rows_affected()).map_err(|_| AuthFailure::Internal)?;
    record_device_rotation_audit(
        transaction,
        user_id,
        device_id,
        deleted_keypackage_count,
        created_at_unix,
    )
    .await
}

async fn emit_device_list_update(
    state: &AppState,
    user_id: UserId,
    device_count: u32,
    created_at_unix: i64,
) {
    match gateway_events::try_device_list_update(user_id, device_count, created_at_unix) {
        Ok(event) => broadcast_user_event(state, user_id, &event).await,
        Err(error) => {
            record_gateway_event_serialize_error("user", gateway_events::DEVICE_LIST_UPDATE_EVENT);
            tracing::error!(
                event = "gateway.device_list_update.serialize_failed",
                event_type = gateway_events::DEVICE_LIST_UPDATE_EVENT,
                error = %error,
                "dropped device-list update because serialization failed"
            );
        }
    }
}

async fn emit_keypackage_low(
    state: &AppState,
    user_id: UserId,
    device_id: DeviceId,
    remaining_count: u32,
    water_mark: u32,
    created_at_unix: i64,
) {
    match gateway_events::try_keypackage_low(
        device_id,
        remaining_count,
        water_mark,
        created_at_unix,
    ) {
        Ok(event) => broadcast_user_event(state, user_id, &event).await,
        Err(error) => {
            record_gateway_event_serialize_error("user", gateway_events::KEYPACKAGE_LOW_EVENT);
            tracing::error!(
                event = "gateway.keypackage_low.serialize_failed",
                event_type = gateway_events::KEYPACKAGE_LOW_EVENT,
                error = %error,
                "dropped KeyPackage low-water alert because serialization failed"
            );
        }
    }
}

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
    let (device_key, root_signature, root_key) = validate_certificate_fields(&payload)?;
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

    let previous_device_key =
        current_device_key_for_update(&mut transaction, auth.user_id, device_id).await?;

    let inserted = sqlx::query(
        "INSERT INTO e2ee_device_certificates
            (device_id, user_id, device_sig_pubkey, root_key_sig, root_key_pub, created_at_unix)
         VALUES ($1, $2, $3, $4, $5, $6)
         ON CONFLICT (device_id) DO UPDATE SET
            device_sig_pubkey = EXCLUDED.device_sig_pubkey,
            root_key_sig = EXCLUDED.root_key_sig,
            root_key_pub = EXCLUDED.root_key_pub,
            created_at_unix = EXCLUDED.created_at_unix
         WHERE e2ee_device_certificates.user_id = EXCLUDED.user_id
           AND e2ee_device_certificates.tombstoned_at_unix IS NULL",
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
    record_device_publish_audit(&mut transaction, auth.user_id, device_id, now).await?;
    invalidate_rotated_device_keypackages(
        &mut transaction,
        auth.user_id,
        device_id,
        previous_device_key.as_deref(),
        &device_key,
        now,
    )
    .await?;
    let active_device_count = active_device_count(&mut transaction, auth.user_id).await?;
    transaction
        .commit()
        .await
        .map_err(|_| AuthFailure::Internal)?;

    emit_device_list_update(&state, auth.user_id, active_device_count, now).await;

    Ok(Json(PublishDeviceCertificateResponse {
        device_id: device_id.to_string(),
        published: true,
    }))
}

/// Irreversibly tombstone an owned device and destroy its unclaimed `KeyPackages`.
pub(crate) async fn remove_device(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(device_id): Path<String>,
) -> Result<Json<RemoveDeviceResponse>, AuthFailure> {
    let auth = authenticate(&state, &headers).await?;
    let device_id = DeviceId::try_from(device_id).map_err(|_| AuthFailure::InvalidRequest)?;
    let pool = state.db_pool.as_ref().ok_or(AuthFailure::Internal)?;
    let now = now_unix();
    let mut transaction = pool.begin().await.map_err(|_| AuthFailure::Internal)?;

    let removed = sqlx::query(
        "UPDATE e2ee_device_certificates
         SET tombstoned_at_unix = $3
         WHERE user_id = $1 AND device_id = $2 AND tombstoned_at_unix IS NULL",
    )
    .bind(auth.user_id.to_string())
    .bind(device_id.to_string())
    .bind(now)
    .execute(&mut *transaction)
    .await
    .map_err(|_| AuthFailure::Internal)?;
    if removed.rows_affected() != 1 {
        return Err(AuthFailure::NotFound);
    }

    let deleted = sqlx::query(
        "DELETE FROM e2ee_keypackages
         WHERE device_id = $1 AND claimed_at_unix IS NULL",
    )
    .bind(device_id.to_string())
    .execute(&mut *transaction)
    .await
    .map_err(|_| AuthFailure::Internal)?;
    let deleted_keypackage_count =
        u32::try_from(deleted.rows_affected()).map_err(|_| AuthFailure::Internal)?;
    sqlx::query(
        "INSERT INTO e2ee_audit_log
            (action, user_id, device_id, metadata_json, created_at_unix)
         VALUES ('device_remove', $1, $2, $3::jsonb, $4)",
    )
    .bind(auth.user_id.to_string())
    .bind(device_id.to_string())
    .bind(json!({ "deleted_keypackage_count": deleted_keypackage_count }).to_string())
    .bind(now)
    .execute(&mut *transaction)
    .await
    .map_err(|_| AuthFailure::Internal)?;
    let active_device_count = active_device_count(&mut transaction, auth.user_id).await?;
    transaction
        .commit()
        .await
        .map_err(|_| AuthFailure::Internal)?;

    emit_device_list_update(&state, auth.user_id, active_device_count, now).await;

    Ok(Json(RemoveDeviceResponse {
        device_id: device_id.to_string(),
        tombstoned_at_unix: now,
        deleted_keypackage_count,
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
    let remaining_count = unclaimed_keypackage_count(&mut transaction, &device_id).await?;
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
    let claimed_device_id =
        DeviceId::try_from(device_id.clone()).map_err(|_| AuthFailure::Internal)?;
    let water_mark = keypackage_low_water_mark(state.runtime.e2ee_max_keypackage_pool_size)?;
    if remaining_count < water_mark {
        emit_keypackage_low(
            &state,
            target_user_id,
            claimed_device_id,
            remaining_count,
            water_mark,
            now,
        )
        .await;
    }
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

    #[test]
    fn low_water_mark_never_exceeds_configured_pool_cap() {
        assert_eq!(keypackage_low_water_mark(100).unwrap(), 10);
        assert_eq!(keypackage_low_water_mark(4).unwrap(), 4);
    }
}
