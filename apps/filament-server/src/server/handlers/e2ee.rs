//! E2EE directory and `KeyPackage` Delivery Service endpoints.
//!
//! The server verifies public device certificates and applies shape/rate
//! limits, but never receives private key material or parses MLS interiors.

use std::{collections::HashSet, fmt::Write as _, net::SocketAddr};

use axum::{
    extract::{connect_info::ConnectInfo, Extension, Path, Query, State},
    http::HeaderMap,
    Json,
};
use filament_core::{CiphersuiteId, ConversationCrypto, ConversationId, DeviceId, GroupId, UserId};
use filament_e2ee::{
    verify_device_certificate, verify_root_identity_rotation_proof, RootIdentityRotationProof,
};
use filament_protocol::{
    AckE2eeMessagesRequest, AckE2eeMessagesResponse, ClaimKeyPackageRequest,
    ClaimKeyPackageResponse, CreateMlsConversationRequest, DeviceInfo, DeviceListResponse,
    E2eeMailboxMessage, E2eeMailboxQuery, E2eeMailboxResponse, GroupInfoResponse, MlsCommitEvent,
    MlsConversationProvisionResponse, MlsMessageEvent, MlsWelcomeEvent, PostCommitRequest,
    PostCommitResponse, PostMessageRequest, PostMessageResponse, PublishDeviceCertificateRequest,
    PublishDeviceCertificateResponse, RemoveDeviceResponse, RootIdentityDirectoryResponse,
    RootIdentityRotationEntry, RotateRootIdentityRequest, RotateRootIdentityResponse,
    UpgradeMlsConversationRequest, UploadKeyPackagesRequest, UploadKeyPackagesResponse,
    MAX_E2EE_MAILBOX_PAGE_BLOB_BYTES, MAX_E2EE_MAILBOX_PAGE_SIZE, MAX_E2EE_MESSAGE_ACK_BATCH_SIZE,
    MAX_ROOT_IDENTITY_ROTATIONS, ROOT_IDENTITY_ROTATION_PROTOCOL_VERSION,
};
use serde_json::json;
use sha2::{Digest, Sha256};
use sqlx::Row;

use crate::server::{
    auth::{
        authenticate, enforce_e2ee_device_publish_rate_limit,
        enforce_e2ee_keypackage_claim_rate_limit, enforce_e2ee_transport_rate_limit,
        extract_client_ip, now_unix, E2eeTransportRoute,
    },
    core::AppState,
    errors::AuthFailure,
    gateway_events,
    metrics::record_gateway_event_serialize_error,
    realtime::broadcast_user_event,
};

const KEYPACKAGE_LOW_WATER_MARK: u32 = 10;
const E2EE_MESSAGE_PADDING_BUCKETS: [usize; 4] = [512, 1_024, 4_096, 16_384];
const DEFAULT_E2EE_MAILBOX_PAGE_SIZE: usize = 20;
const MAX_E2EE_1_TO_1_DELIVERY_DEVICES: i64 = 200;
const INITIAL_MLS_EPOCH: u64 = 1;

type CertificateFields = ([u8; 32], [u8; 64], [u8; 32]);
type RotationFields = ([u8; 32], [u8; 64], [u8; 64], [u8; 32], [u8; 64]);

struct GroupAccess {
    conversation_id: String,
    current_epoch: u64,
    suite_id: u16,
    group_info_blob: Option<Vec<u8>>,
}

struct InitialProvision<'a> {
    conversation_id: ConversationId,
    group_id: GroupId,
    suite_id: u16,
    committer_device_id: DeviceId,
    commit_blob: &'a [u8],
    welcome_blob: &'a [u8],
    group_info_blob: &'a [u8],
}

struct ExistingProvision {
    conversation_id: String,
    group_id: String,
    suite_id: u16,
    committer_device_id: String,
    commit_blob: Vec<u8>,
    welcome_blob: Option<Vec<u8>>,
    group_info_blob: Option<Vec<u8>>,
    provisioned_at_unix: i64,
}

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

fn validate_rotation_fields(
    payload: &RotateRootIdentityRequest,
) -> Result<RotationFields, AuthFailure> {
    let new_root_key = payload
        .new_root_key_pub
        .as_slice()
        .try_into()
        .map_err(|_| AuthFailure::InvalidRequest)?;
    let previous_root_signature = payload
        .previous_root_signature
        .as_slice()
        .try_into()
        .map_err(|_| AuthFailure::InvalidRequest)?;
    let new_root_signature = payload
        .new_root_signature
        .as_slice()
        .try_into()
        .map_err(|_| AuthFailure::InvalidRequest)?;
    let new_device_key = payload
        .new_device_signature_pubkey
        .as_slice()
        .try_into()
        .map_err(|_| AuthFailure::InvalidRequest)?;
    let new_device_root_signature = payload
        .new_device_root_signature
        .as_slice()
        .try_into()
        .map_err(|_| AuthFailure::InvalidRequest)?;
    Ok((
        new_root_key,
        previous_root_signature,
        new_root_signature,
        new_device_key,
        new_device_root_signature,
    ))
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

fn is_valid_message_padding_bucket(byte_len: usize) -> bool {
    E2EE_MESSAGE_PADDING_BUCKETS.contains(&byte_len)
}

async fn lock_group_for_member(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    group_id: GroupId,
    user_id: UserId,
) -> Result<GroupAccess, AuthFailure> {
    let row = sqlx::query(
        "SELECT g.conversation_id, g.current_epoch, g.suite_id, g.group_info_blob
         FROM e2ee_groups g
         JOIN e2ee_conversations c ON c.conversation_id = g.conversation_id
         JOIN e2ee_conversation_members m ON m.conversation_id = c.conversation_id
         WHERE g.group_id = $1 AND m.user_id = $2 AND c.conversation_crypto = 'mls_v1'
         FOR UPDATE OF g",
    )
    .bind(group_id.to_string())
    .bind(user_id.to_string())
    .fetch_optional(&mut **transaction)
    .await
    .map_err(|_| AuthFailure::Internal)?
    .ok_or(AuthFailure::NotFound)?;
    let current_epoch: i64 = row
        .try_get("current_epoch")
        .map_err(|_| AuthFailure::Internal)?;
    let suite_id: i32 = row.try_get("suite_id").map_err(|_| AuthFailure::Internal)?;
    Ok(GroupAccess {
        conversation_id: row
            .try_get("conversation_id")
            .map_err(|_| AuthFailure::Internal)?,
        current_epoch: u64::try_from(current_epoch).map_err(|_| AuthFailure::Internal)?,
        suite_id: u16::try_from(suite_id).map_err(|_| AuthFailure::Internal)?,
        group_info_blob: row
            .try_get("group_info_blob")
            .map_err(|_| AuthFailure::Internal)?,
    })
}

async fn get_group_for_member(
    pool: &sqlx::PgPool,
    group_id: GroupId,
    user_id: UserId,
) -> Result<GroupAccess, AuthFailure> {
    let row = sqlx::query(
        "SELECT g.conversation_id, g.current_epoch, g.suite_id, g.group_info_blob
         FROM e2ee_groups g
         JOIN e2ee_conversations c ON c.conversation_id = g.conversation_id
         JOIN e2ee_conversation_members m ON m.conversation_id = c.conversation_id
         WHERE g.group_id = $1 AND m.user_id = $2 AND c.conversation_crypto = 'mls_v1'",
    )
    .bind(group_id.to_string())
    .bind(user_id.to_string())
    .fetch_optional(pool)
    .await
    .map_err(|_| AuthFailure::Internal)?
    .ok_or(AuthFailure::NotFound)?;
    let current_epoch: i64 = row
        .try_get("current_epoch")
        .map_err(|_| AuthFailure::Internal)?;
    let suite_id: i32 = row.try_get("suite_id").map_err(|_| AuthFailure::Internal)?;
    Ok(GroupAccess {
        conversation_id: row
            .try_get("conversation_id")
            .map_err(|_| AuthFailure::Internal)?,
        current_epoch: u64::try_from(current_epoch).map_err(|_| AuthFailure::Internal)?,
        suite_id: u16::try_from(suite_id).map_err(|_| AuthFailure::Internal)?,
        group_info_blob: row
            .try_get("group_info_blob")
            .map_err(|_| AuthFailure::Internal)?,
    })
}

async fn get_group_for_member_in_transaction(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    group_id: GroupId,
    user_id: UserId,
) -> Result<GroupAccess, AuthFailure> {
    let row = sqlx::query(
        "SELECT g.conversation_id, g.current_epoch, g.suite_id, g.group_info_blob
         FROM e2ee_groups g
         JOIN e2ee_conversations c ON c.conversation_id = g.conversation_id
         JOIN e2ee_conversation_members m ON m.conversation_id = c.conversation_id
         WHERE g.group_id = $1 AND m.user_id = $2 AND c.conversation_crypto = 'mls_v1'",
    )
    .bind(group_id.to_string())
    .bind(user_id.to_string())
    .fetch_optional(&mut **transaction)
    .await
    .map_err(|_| AuthFailure::Internal)?
    .ok_or(AuthFailure::NotFound)?;
    let current_epoch: i64 = row
        .try_get("current_epoch")
        .map_err(|_| AuthFailure::Internal)?;
    let suite_id: i32 = row.try_get("suite_id").map_err(|_| AuthFailure::Internal)?;
    Ok(GroupAccess {
        conversation_id: row
            .try_get("conversation_id")
            .map_err(|_| AuthFailure::Internal)?,
        current_epoch: u64::try_from(current_epoch).map_err(|_| AuthFailure::Internal)?,
        suite_id: u16::try_from(suite_id).map_err(|_| AuthFailure::Internal)?,
        group_info_blob: row
            .try_get("group_info_blob")
            .map_err(|_| AuthFailure::Internal)?,
    })
}

async fn require_active_owned_device(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: UserId,
    device_id: DeviceId,
) -> Result<(), AuthFailure> {
    let owned = sqlx::query(
        "SELECT 1 FROM e2ee_device_certificates
         WHERE user_id = $1 AND device_id = $2 AND tombstoned_at_unix IS NULL
         FOR SHARE",
    )
    .bind(user_id.to_string())
    .bind(device_id.to_string())
    .fetch_optional(&mut **transaction)
    .await
    .map_err(|_| AuthFailure::Internal)?;
    owned.map(|_| ()).ok_or(AuthFailure::NotFound)
}

fn canonical_user_pair(first: UserId, second: UserId) -> (String, String) {
    let first = first.to_string();
    let second = second.to_string();
    if first < second {
        (first, second)
    } else {
        (second, first)
    }
}

async fn lock_capable_two_user_membership(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    members: &[UserId; 2],
) -> Result<(), AuthFailure> {
    if members[0] == members[1] {
        return Err(AuthFailure::InvalidRequest);
    }
    let member_ids = [members[0].to_string(), members[1].to_string()];
    let rows: Vec<String> = sqlx::query_scalar(
        "SELECT user_id FROM e2ee_device_certificates
         WHERE user_id = ANY($1::TEXT[]) AND tombstoned_at_unix IS NULL
         ORDER BY user_id, device_id
         FOR SHARE",
    )
    .bind(member_ids.as_slice())
    .fetch_all(&mut **transaction)
    .await
    .map_err(|_| AuthFailure::Internal)?;
    let device_count = i64::try_from(rows.len()).map_err(|_| AuthFailure::Internal)?;
    let capable_users: HashSet<String> = rows.into_iter().collect();
    if capable_users.len() != 2 || device_count > MAX_E2EE_1_TO_1_DELIVERY_DEVICES {
        return Err(AuthFailure::E2eeCapabilityRequired);
    }
    Ok(())
}

fn existing_provision_from_row(
    row: &sqlx::postgres::PgRow,
) -> Result<ExistingProvision, AuthFailure> {
    let suite_id: i32 = row.try_get("suite_id").map_err(|_| AuthFailure::Internal)?;
    Ok(ExistingProvision {
        conversation_id: row
            .try_get("conversation_id")
            .map_err(|_| AuthFailure::Internal)?,
        group_id: row.try_get("group_id").map_err(|_| AuthFailure::Internal)?,
        suite_id: u16::try_from(suite_id).map_err(|_| AuthFailure::Internal)?,
        committer_device_id: row
            .try_get("committer_device_id")
            .map_err(|_| AuthFailure::Internal)?,
        commit_blob: row
            .try_get("commit_blob")
            .map_err(|_| AuthFailure::Internal)?,
        welcome_blob: row
            .try_get("welcome_blob")
            .map_err(|_| AuthFailure::Internal)?,
        group_info_blob: row
            .try_get("group_info_blob")
            .map_err(|_| AuthFailure::Internal)?,
        provisioned_at_unix: row
            .try_get("provisioned_at_unix")
            .map_err(|_| AuthFailure::Internal)?,
    })
}

async fn existing_provision_for_pair(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    lower_user_id: &str,
    upper_user_id: &str,
) -> Result<Option<ExistingProvision>, AuthFailure> {
    let row = sqlx::query(
        "SELECT c.conversation_id, g.group_id, g.suite_id,
                k.committer_device_id, k.commit_blob, k.welcome_blob,
                g.group_info_blob, g.created_at_unix AS provisioned_at_unix
         FROM e2ee_dm_pairs p
         JOIN e2ee_conversations c ON c.conversation_id = p.conversation_id
         JOIN e2ee_groups g ON g.conversation_id = c.conversation_id
         JOIN e2ee_commits k ON k.group_id = g.group_id AND k.epoch = 1
         WHERE p.user_a_id = $1 AND p.user_b_id = $2
           AND c.conversation_crypto = 'mls_v1'",
    )
    .bind(lower_user_id)
    .bind(upper_user_id)
    .fetch_optional(&mut **transaction)
    .await
    .map_err(|_| AuthFailure::Internal)?;
    row.as_ref().map(existing_provision_from_row).transpose()
}

async fn existing_provision_for_conversation(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    conversation_id: ConversationId,
) -> Result<Option<ExistingProvision>, AuthFailure> {
    let row = sqlx::query(
        "SELECT c.conversation_id, g.group_id, g.suite_id,
                k.committer_device_id, k.commit_blob, k.welcome_blob,
                g.group_info_blob, g.created_at_unix AS provisioned_at_unix
         FROM e2ee_conversations c
         JOIN e2ee_groups g ON g.conversation_id = c.conversation_id
         JOIN e2ee_commits k ON k.group_id = g.group_id AND k.epoch = 1
         WHERE c.conversation_id = $1 AND c.conversation_crypto = 'mls_v1'",
    )
    .bind(conversation_id.to_string())
    .fetch_optional(&mut **transaction)
    .await
    .map_err(|_| AuthFailure::Internal)?;
    row.as_ref().map(existing_provision_from_row).transpose()
}

fn provision_matches(existing: &ExistingProvision, expected: &InitialProvision<'_>) -> bool {
    existing.conversation_id == expected.conversation_id.to_string()
        && existing.group_id == expected.group_id.to_string()
        && existing.suite_id == expected.suite_id
        && existing.committer_device_id == expected.committer_device_id.to_string()
        && existing.commit_blob == expected.commit_blob
        && existing.welcome_blob.as_deref() == Some(expected.welcome_blob)
        && existing.group_info_blob.as_deref() == Some(expected.group_info_blob)
}

fn provision_response(
    provision: &InitialProvision<'_>,
    provisioned_at_unix: i64,
) -> MlsConversationProvisionResponse {
    MlsConversationProvisionResponse {
        conversation_id: provision.conversation_id.to_string(),
        group_id: provision.group_id.to_string(),
        crypto: String::from("mls_v1"),
        epoch: INITIAL_MLS_EPOCH,
        suite_id: provision.suite_id,
        provisioned_at_unix,
    }
}

fn map_provision_write_error(error: &sqlx::Error) -> AuthFailure {
    if error
        .as_database_error()
        .and_then(sqlx::error::DatabaseError::code)
        .is_some_and(|code| code.starts_with("23"))
    {
        AuthFailure::E2eeConversationConflict
    } else {
        AuthFailure::Internal
    }
}

async fn insert_initial_group_and_commit(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    provision: &InitialProvision<'_>,
    now: i64,
    expires_at: i64,
) -> Result<(), AuthFailure> {
    sqlx::query(
        "INSERT INTO e2ee_groups
            (group_id, conversation_id, current_epoch, suite_id,
             group_info_blob, created_at_unix)
         VALUES ($1, $2, 1, $3, $4, $5)",
    )
    .bind(provision.group_id.to_string())
    .bind(provision.conversation_id.to_string())
    .bind(i32::from(provision.suite_id))
    .bind(provision.group_info_blob)
    .bind(now)
    .execute(&mut **transaction)
    .await
    .map_err(|error| map_provision_write_error(&error))?;
    sqlx::query(
        "INSERT INTO e2ee_commits
            (group_id, epoch, prior_epoch, committer_device_id,
             commit_blob, welcome_blob, created_at_unix, expires_at_unix)
         VALUES ($1, 1, 0, $2, $3, $4, $5, $6)",
    )
    .bind(provision.group_id.to_string())
    .bind(provision.committer_device_id.to_string())
    .bind(provision.commit_blob)
    .bind(provision.welcome_blob)
    .bind(now)
    .bind(expires_at)
    .execute(&mut **transaction)
    .await
    .map_err(|error| map_provision_write_error(&error))?;
    Ok(())
}

async fn record_conversation_provision_audit(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    action: &'static str,
    user_id: UserId,
    provision: &InitialProvision<'_>,
    now: i64,
) -> Result<(), AuthFailure> {
    sqlx::query(
        "INSERT INTO e2ee_audit_log
            (action, user_id, device_id, metadata_json, created_at_unix)
         VALUES ($1, $2, $3, $4::jsonb, $5)",
    )
    .bind(action)
    .bind(user_id.to_string())
    .bind(provision.committer_device_id.to_string())
    .bind(
        json!({
            "conversation_id": provision.conversation_id.to_string(),
            "group_id": provision.group_id.to_string(),
            "suite_id": provision.suite_id,
        })
        .to_string(),
    )
    .bind(now)
    .execute(&mut **transaction)
    .await
    .map_err(|_| AuthFailure::Internal)?;
    Ok(())
}

fn parse_canonical_ulid(value: &str) -> Result<(), AuthFailure> {
    let parsed = ulid::Ulid::from_string(value).map_err(|_| AuthFailure::InvalidRequest)?;
    if parsed.to_string() != value {
        return Err(AuthFailure::InvalidRequest);
    }
    Ok(())
}

fn mailbox_page_limit(limit: Option<u16>) -> Result<usize, AuthFailure> {
    let limit = limit.map_or(DEFAULT_E2EE_MAILBOX_PAGE_SIZE, usize::from);
    if limit == 0 || limit > MAX_E2EE_MAILBOX_PAGE_SIZE {
        return Err(AuthFailure::InvalidRequest);
    }
    Ok(limit)
}

async fn conversation_member_ids(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    conversation_id: &str,
) -> Result<Vec<UserId>, AuthFailure> {
    let rows: Vec<String> = sqlx::query_scalar(
        "SELECT user_id FROM e2ee_conversation_members
         WHERE conversation_id = $1 ORDER BY user_id ASC",
    )
    .bind(conversation_id)
    .fetch_all(&mut **transaction)
    .await
    .map_err(|_| AuthFailure::Internal)?;
    rows.into_iter()
        .map(|value| UserId::try_from(value).map_err(|_| AuthFailure::Internal))
        .collect()
}

async fn snapshot_message_deliveries(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    conversation_id: &str,
    message_id: &str,
    sender_device_id: DeviceId,
    created_at_unix: i64,
) -> Result<(), AuthFailure> {
    let member_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM e2ee_conversation_members WHERE conversation_id = $1",
    )
    .bind(conversation_id)
    .fetch_one(&mut **transaction)
    .await
    .map_err(|_| AuthFailure::Internal)?;
    let device_rows = sqlx::query(
        "SELECT d.device_id, d.user_id
         FROM e2ee_conversation_members m
         JOIN e2ee_device_certificates d
           ON d.user_id = m.user_id AND d.tombstoned_at_unix IS NULL
         WHERE m.conversation_id = $1
         ORDER BY d.device_id ASC
         FOR SHARE OF d",
    )
    .bind(conversation_id)
    .fetch_all(&mut **transaction)
    .await
    .map_err(|_| AuthFailure::Internal)?;
    let device_count = i64::try_from(device_rows.len()).map_err(|_| AuthFailure::Internal)?;
    let capable_users: HashSet<String> = device_rows
        .iter()
        .map(|row| row.try_get("user_id").map_err(|_| AuthFailure::Internal))
        .collect::<Result<_, _>>()?;
    let capable_member_count =
        i64::try_from(capable_users.len()).map_err(|_| AuthFailure::Internal)?;
    if member_count != 2
        || capable_member_count != member_count
        || device_count < member_count
        || device_count > MAX_E2EE_1_TO_1_DELIVERY_DEVICES
    {
        return Err(AuthFailure::E2eeCapabilityRequired);
    }

    let inserted = sqlx::query(
        "INSERT INTO e2ee_message_acks (message_id, device_id, acked_at_unix)
         SELECT $1, d.device_id,
                CASE WHEN d.device_id = $2 THEN $3 ELSE NULL END
         FROM e2ee_conversation_members m
         JOIN e2ee_device_certificates d
           ON d.user_id = m.user_id AND d.tombstoned_at_unix IS NULL
         WHERE m.conversation_id = $4",
    )
    .bind(message_id)
    .bind(sender_device_id.to_string())
    .bind(created_at_unix)
    .bind(conversation_id)
    .execute(&mut **transaction)
    .await
    .map_err(|_| AuthFailure::Internal)?;
    let inserted = i64::try_from(inserted.rows_affected()).map_err(|_| AuthFailure::Internal)?;
    if inserted != device_count {
        return Err(AuthFailure::Internal);
    }
    Ok(())
}

async fn broadcast_conversation_event(
    state: &AppState,
    member_ids: &[UserId],
    event: &gateway_events::GatewayEvent,
) {
    for user_id in member_ids {
        broadcast_user_event(state, *user_id, event).await;
    }
}

async fn emit_mls_commit_notifications(
    state: &AppState,
    member_ids: &[UserId],
    commit: MlsCommitEvent,
    suite_id: u16,
    has_welcome: bool,
) {
    match gateway_events::try_mls_commit(commit.clone()) {
        Ok(event) => broadcast_conversation_event(state, member_ids, &event).await,
        Err(error) => {
            record_gateway_event_serialize_error("user", gateway_events::MLS_COMMIT_EVENT);
            tracing::error!(
                event = "gateway.mls_commit.serialize_failed",
                error = %error,
                "dropped MLS commit notification because serialization failed"
            );
        }
    }
    if has_welcome {
        match gateway_events::try_mls_welcome(MlsWelcomeEvent {
            group_id: commit.group_id.clone(),
            conversation_id: commit.conversation_id.clone(),
            epoch: commit.epoch,
            suite_id,
            created_at_unix: commit.created_at_unix,
        }) {
            Ok(event) => broadcast_conversation_event(state, member_ids, &event).await,
            Err(error) => {
                record_gateway_event_serialize_error("user", gateway_events::MLS_WELCOME_EVENT);
                tracing::error!(
                    event = "gateway.mls_welcome.serialize_failed",
                    error = %error,
                    "dropped MLS Welcome notification because serialization failed"
                );
            }
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

/// Return the current public root and its bounded, dual-signed continuity chain.
pub(crate) async fn get_root_identity(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(user_id): Path<String>,
) -> Result<Json<RootIdentityDirectoryResponse>, AuthFailure> {
    let _auth = authenticate(&state, &headers).await?;
    let user_id = UserId::try_from(user_id).map_err(|_| AuthFailure::InvalidRequest)?;
    let pool = state.db_pool.as_ref().ok_or(AuthFailure::Internal)?;
    let root = sqlx::query(
        "SELECT root_key_pub, rotation_sequence FROM e2ee_root_identities WHERE user_id = $1",
    )
    .bind(user_id.to_string())
    .fetch_optional(pool)
    .await
    .map_err(|_| AuthFailure::Internal)?
    .ok_or(AuthFailure::NotFound)?;
    let rotation_rows = sqlx::query(
        "SELECT sequence, previous_root_key_pub, new_root_key_pub,
                previous_root_signature, new_root_signature,
                rotating_device_id, rotated_at_unix
         FROM e2ee_root_identity_rotations
         WHERE user_id = $1 ORDER BY sequence ASC LIMIT 100",
    )
    .bind(user_id.to_string())
    .fetch_all(pool)
    .await
    .map_err(|_| AuthFailure::Internal)?;
    let rotations = rotation_rows
        .into_iter()
        .map(|row| {
            let sequence: i64 = row.try_get("sequence").map_err(|_| AuthFailure::Internal)?;
            Ok(RootIdentityRotationEntry {
                sequence: u64::try_from(sequence).map_err(|_| AuthFailure::Internal)?,
                previous_root_key_pub: row
                    .try_get("previous_root_key_pub")
                    .map_err(|_| AuthFailure::Internal)?,
                new_root_key_pub: row
                    .try_get("new_root_key_pub")
                    .map_err(|_| AuthFailure::Internal)?,
                previous_root_signature: row
                    .try_get("previous_root_signature")
                    .map_err(|_| AuthFailure::Internal)?,
                new_root_signature: row
                    .try_get("new_root_signature")
                    .map_err(|_| AuthFailure::Internal)?,
                rotating_device_id: row
                    .try_get("rotating_device_id")
                    .map_err(|_| AuthFailure::Internal)?,
                rotated_at_unix: row
                    .try_get("rotated_at_unix")
                    .map_err(|_| AuthFailure::Internal)?,
            })
        })
        .collect::<Result<Vec<_>, AuthFailure>>()?;
    let rotation_sequence: i64 = root
        .try_get("rotation_sequence")
        .map_err(|_| AuthFailure::Internal)?;
    Ok(Json(RootIdentityDirectoryResponse {
        protocol_version: ROOT_IDENTITY_ROTATION_PROTOCOL_VERSION,
        user_id: user_id.to_string(),
        current_root_key_pub: root
            .try_get("root_key_pub")
            .map_err(|_| AuthFailure::Internal)?,
        rotation_sequence: u64::try_from(rotation_sequence).map_err(|_| AuthFailure::Internal)?,
        rotations,
    }))
}

/// Destructively rotate the authenticated user's pinned root identity.
// Keep the security-critical lock, proof verification, revocation, keypackage
// deletion, continuity append, and audit write together for transaction review.
#[allow(clippy::too_many_lines)]
pub(crate) async fn rotate_root_identity(
    State(state): State<AppState>,
    headers: HeaderMap,
    connect_info: Option<Extension<ConnectInfo<SocketAddr>>>,
    Json(payload): Json<RotateRootIdentityRequest>,
) -> Result<Json<RotateRootIdentityResponse>, AuthFailure> {
    if payload.protocol_version != ROOT_IDENTITY_ROTATION_PROTOCOL_VERSION {
        return Err(AuthFailure::InvalidRequest);
    }
    let client_ip = extract_client_ip(
        &state,
        &headers,
        connect_info.as_ref().map(|value| value.0 .0.ip()),
    );
    let auth = authenticate(&state, &headers).await?;
    enforce_e2ee_device_publish_rate_limit(&state, client_ip, auth.user_id).await?;
    let device_id =
        DeviceId::try_from(payload.device_id.clone()).map_err(|_| AuthFailure::InvalidRequest)?;
    let next_sequence = payload
        .expected_rotation_sequence
        .checked_add(1)
        .filter(|value| {
            usize::try_from(*value).is_ok_and(|value| value <= MAX_ROOT_IDENTITY_ROTATIONS)
        })
        .ok_or(AuthFailure::InvalidRequest)?;
    let (
        new_root_key,
        previous_root_signature,
        new_root_signature,
        new_device_key,
        new_device_root_signature,
    ) = validate_rotation_fields(&payload)?;

    let pool = state.db_pool.as_ref().ok_or(AuthFailure::Internal)?;
    let now = now_unix();
    let mut transaction = pool.begin().await.map_err(|_| AuthFailure::Internal)?;
    let current = sqlx::query(
        "SELECT root_key_pub, rotation_sequence FROM e2ee_root_identities
         WHERE user_id = $1 FOR UPDATE",
    )
    .bind(auth.user_id.to_string())
    .fetch_optional(&mut *transaction)
    .await
    .map_err(|_| AuthFailure::Internal)?
    .ok_or(AuthFailure::NotFound)?;
    let previous_root_key: Vec<u8> = current
        .try_get("root_key_pub")
        .map_err(|_| AuthFailure::Internal)?;
    let current_sequence: i64 = current
        .try_get("rotation_sequence")
        .map_err(|_| AuthFailure::Internal)?;
    if u64::try_from(current_sequence).map_err(|_| AuthFailure::Internal)?
        != payload.expected_rotation_sequence
    {
        return Err(AuthFailure::Forbidden);
    }
    let previous_root_key_array: [u8; 32] = previous_root_key
        .as_slice()
        .try_into()
        .map_err(|_| AuthFailure::Internal)?;
    let proof = RootIdentityRotationProof {
        sequence: next_sequence,
        previous_root_key_pub: previous_root_key_array,
        new_root_key_pub: new_root_key,
        previous_root_signature,
        new_root_signature,
    };
    verify_root_identity_rotation_proof(auth.user_id, &proof)
        .map_err(|_| AuthFailure::Forbidden)?;
    verify_device_certificate(
        &new_root_key,
        auth.user_id,
        device_id,
        &new_device_key,
        &new_device_root_signature,
    )
    .map_err(|_| AuthFailure::InvalidRequest)?;

    let retained = sqlx::query(
        "SELECT 1 FROM e2ee_device_certificates
         WHERE user_id = $1 AND device_id = $2 AND tombstoned_at_unix IS NULL FOR UPDATE",
    )
    .bind(auth.user_id.to_string())
    .bind(device_id.to_string())
    .fetch_optional(&mut *transaction)
    .await
    .map_err(|_| AuthFailure::Internal)?;
    if retained.is_none() {
        return Err(AuthFailure::NotFound);
    }
    let revoked = sqlx::query(
        "UPDATE e2ee_device_certificates SET tombstoned_at_unix = $3
         WHERE user_id = $1 AND device_id <> $2 AND tombstoned_at_unix IS NULL",
    )
    .bind(auth.user_id.to_string())
    .bind(device_id.to_string())
    .bind(now)
    .execute(&mut *transaction)
    .await
    .map_err(|_| AuthFailure::Internal)?;
    let revoked_device_count =
        u32::try_from(revoked.rows_affected()).map_err(|_| AuthFailure::Internal)?;
    sqlx::query(
        "UPDATE e2ee_device_certificates SET
            device_sig_pubkey = $3, root_key_sig = $4, root_key_pub = $5, created_at_unix = $6
         WHERE user_id = $1 AND device_id = $2 AND tombstoned_at_unix IS NULL",
    )
    .bind(auth.user_id.to_string())
    .bind(device_id.to_string())
    .bind(new_device_key.as_slice())
    .bind(new_device_root_signature.as_slice())
    .bind(new_root_key.as_slice())
    .bind(now)
    .execute(&mut *transaction)
    .await
    .map_err(|_| AuthFailure::Internal)?;
    let deleted = sqlx::query(
        "DELETE FROM e2ee_keypackages kp USING e2ee_device_certificates dc
         WHERE kp.device_id = dc.device_id AND dc.user_id = $1
           AND kp.claimed_at_unix IS NULL",
    )
    .bind(auth.user_id.to_string())
    .execute(&mut *transaction)
    .await
    .map_err(|_| AuthFailure::Internal)?;
    let deleted_keypackage_count =
        u32::try_from(deleted.rows_affected()).map_err(|_| AuthFailure::Internal)?;
    let next_sequence_i64 = i64::try_from(next_sequence).map_err(|_| AuthFailure::Internal)?;
    sqlx::query(
        "INSERT INTO e2ee_root_identity_rotations
            (user_id, sequence, previous_root_key_pub, new_root_key_pub,
             previous_root_signature, new_root_signature, rotating_device_id, rotated_at_unix)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
    )
    .bind(auth.user_id.to_string())
    .bind(next_sequence_i64)
    .bind(previous_root_key.as_slice())
    .bind(new_root_key.as_slice())
    .bind(previous_root_signature.as_slice())
    .bind(new_root_signature.as_slice())
    .bind(device_id.to_string())
    .bind(now)
    .execute(&mut *transaction)
    .await
    .map_err(|_| AuthFailure::Internal)?;
    sqlx::query(
        "UPDATE e2ee_root_identities SET root_key_pub = $2, rotation_sequence = $3
         WHERE user_id = $1",
    )
    .bind(auth.user_id.to_string())
    .bind(new_root_key.as_slice())
    .bind(next_sequence_i64)
    .execute(&mut *transaction)
    .await
    .map_err(|_| AuthFailure::Internal)?;
    sqlx::query(
        "INSERT INTO e2ee_audit_log
            (action, user_id, device_id, metadata_json, created_at_unix)
         VALUES ('identity_rotate', $1, $2, $3::jsonb, $4)",
    )
    .bind(auth.user_id.to_string())
    .bind(device_id.to_string())
    .bind(
        json!({
            "rotation_sequence": next_sequence,
            "revoked_device_count": revoked_device_count,
            "deleted_keypackage_count": deleted_keypackage_count,
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

    emit_device_list_update(&state, auth.user_id, 1, now).await;
    Ok(Json(RotateRootIdentityResponse {
        protocol_version: ROOT_IDENTITY_ROTATION_PROTOCOL_VERSION,
        user_id: auth.user_id.to_string(),
        device_id: device_id.to_string(),
        rotation_sequence: next_sequence,
        previous_root_key_pub: previous_root_key,
        new_root_key_pub: new_root_key.to_vec(),
        revoked_device_count,
        deleted_keypackage_count,
        rotated_at_unix: now,
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

/// Atomically create a two-user MLS conversation with its initial Add commit.
#[allow(clippy::too_many_lines)]
pub(crate) async fn create_mls_conversation(
    State(state): State<AppState>,
    headers: HeaderMap,
    connect_info: Option<Extension<ConnectInfo<SocketAddr>>>,
    Json(payload): Json<CreateMlsConversationRequest>,
) -> Result<Json<MlsConversationProvisionResponse>, AuthFailure> {
    let client_ip = extract_client_ip(
        &state,
        &headers,
        connect_info.as_ref().map(|value| value.0 .0.ip()),
    );
    let auth = authenticate(&state, &headers).await?;
    parse_canonical_ulid(&payload.conversation_id)?;
    parse_canonical_ulid(&payload.peer_user_id)?;
    parse_canonical_ulid(&payload.group_id)?;
    parse_canonical_ulid(&payload.committer_device_id)?;
    let conversation_id = ConversationId::try_from(payload.conversation_id.clone())
        .map_err(|_| AuthFailure::InvalidRequest)?;
    let peer_user_id =
        UserId::try_from(payload.peer_user_id).map_err(|_| AuthFailure::InvalidRequest)?;
    if peer_user_id == auth.user_id {
        return Err(AuthFailure::InvalidRequest);
    }
    let group_id =
        GroupId::try_from(payload.group_id.clone()).map_err(|_| AuthFailure::InvalidRequest)?;
    let committer_device_id = DeviceId::try_from(payload.committer_device_id.clone())
        .map_err(|_| AuthFailure::InvalidRequest)?;
    CiphersuiteId::try_from(payload.suite_id).map_err(|_| AuthFailure::InvalidRequest)?;
    enforce_e2ee_transport_rate_limit(
        &state,
        client_ip,
        auth.user_id,
        committer_device_id,
        group_id,
        E2eeTransportRoute::Provision,
    )
    .await?;

    let provision = InitialProvision {
        conversation_id,
        group_id,
        suite_id: payload.suite_id,
        committer_device_id,
        commit_blob: &payload.commit_blob,
        welcome_blob: &payload.welcome_blob,
        group_info_blob: &payload.group_info_blob,
    };
    let (lower_user_id, upper_user_id) = canonical_user_pair(auth.user_id, peer_user_id);
    let pool = state.db_pool.as_ref().ok_or(AuthFailure::Internal)?;
    let now = now_unix();
    let ttl = i64::try_from(state.runtime.e2ee_mailbox_ttl.as_secs())
        .map_err(|_| AuthFailure::Internal)?;
    let expires_at = now.checked_add(ttl).ok_or(AuthFailure::Internal)?;
    let mut transaction = pool.begin().await.map_err(|_| AuthFailure::Internal)?;
    require_active_owned_device(&mut transaction, auth.user_id, committer_device_id).await?;
    let pair_lock_key = format!("{lower_user_id}:{upper_user_id}");
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
        .bind(pair_lock_key)
        .execute(&mut *transaction)
        .await
        .map_err(|_| AuthFailure::Internal)?;

    if let Some(existing) =
        existing_provision_for_pair(&mut transaction, &lower_user_id, &upper_user_id).await?
    {
        if !provision_matches(&existing, &provision) {
            return Err(AuthFailure::E2eeConversationConflict);
        }
        transaction
            .commit()
            .await
            .map_err(|_| AuthFailure::Internal)?;
        return Ok(Json(provision_response(
            &provision,
            existing.provisioned_at_unix,
        )));
    }

    lock_capable_two_user_membership(&mut transaction, &[auth.user_id, peer_user_id]).await?;
    sqlx::query(
        "INSERT INTO e2ee_conversations
            (conversation_id, conversation_crypto, created_by, created_at_unix)
         VALUES ($1, 'mls_v1', $2, $3)",
    )
    .bind(conversation_id.to_string())
    .bind(auth.user_id.to_string())
    .bind(now)
    .execute(&mut *transaction)
    .await
    .map_err(|error| map_provision_write_error(&error))?;
    for member_id in [auth.user_id, peer_user_id] {
        sqlx::query(
            "INSERT INTO e2ee_conversation_members
                (conversation_id, user_id, joined_at_unix)
             VALUES ($1, $2, $3)",
        )
        .bind(conversation_id.to_string())
        .bind(member_id.to_string())
        .bind(now)
        .execute(&mut *transaction)
        .await
        .map_err(|error| map_provision_write_error(&error))?;
    }
    sqlx::query(
        "INSERT INTO e2ee_dm_pairs (conversation_id, user_a_id, user_b_id)
         VALUES ($1, $2, $3)",
    )
    .bind(conversation_id.to_string())
    .bind(&lower_user_id)
    .bind(&upper_user_id)
    .execute(&mut *transaction)
    .await
    .map_err(|error| map_provision_write_error(&error))?;
    insert_initial_group_and_commit(&mut transaction, &provision, now, expires_at).await?;
    record_conversation_provision_audit(
        &mut transaction,
        "conversation_create",
        auth.user_id,
        &provision,
        now,
    )
    .await?;
    transaction
        .commit()
        .await
        .map_err(|_| AuthFailure::Internal)?;

    emit_mls_commit_notifications(
        &state,
        &[auth.user_id, peer_user_id],
        MlsCommitEvent {
            group_id: group_id.to_string(),
            conversation_id: conversation_id.to_string(),
            epoch: INITIAL_MLS_EPOCH,
            prior_epoch: 0,
            committer_device_id: committer_device_id.to_string(),
            created_at_unix: now,
        },
        payload.suite_id,
        true,
    )
    .await;
    Ok(Json(provision_response(&provision, now)))
}

/// Explicitly upgrade an existing two-user plaintext conversation to MLS v1.
#[allow(clippy::too_many_lines)]
pub(crate) async fn upgrade_mls_conversation(
    State(state): State<AppState>,
    headers: HeaderMap,
    connect_info: Option<Extension<ConnectInfo<SocketAddr>>>,
    Path(conversation_id): Path<String>,
    Json(payload): Json<UpgradeMlsConversationRequest>,
) -> Result<Json<MlsConversationProvisionResponse>, AuthFailure> {
    let client_ip = extract_client_ip(
        &state,
        &headers,
        connect_info.as_ref().map(|value| value.0 .0.ip()),
    );
    let auth = authenticate(&state, &headers).await?;
    parse_canonical_ulid(&conversation_id)?;
    parse_canonical_ulid(&payload.group_id)?;
    parse_canonical_ulid(&payload.committer_device_id)?;
    let conversation_id =
        ConversationId::try_from(conversation_id).map_err(|_| AuthFailure::InvalidRequest)?;
    let group_id =
        GroupId::try_from(payload.group_id.clone()).map_err(|_| AuthFailure::InvalidRequest)?;
    let committer_device_id = DeviceId::try_from(payload.committer_device_id.clone())
        .map_err(|_| AuthFailure::InvalidRequest)?;
    CiphersuiteId::try_from(payload.suite_id).map_err(|_| AuthFailure::InvalidRequest)?;
    enforce_e2ee_transport_rate_limit(
        &state,
        client_ip,
        auth.user_id,
        committer_device_id,
        group_id,
        E2eeTransportRoute::Provision,
    )
    .await?;
    let provision = InitialProvision {
        conversation_id,
        group_id,
        suite_id: payload.suite_id,
        committer_device_id,
        commit_blob: &payload.commit_blob,
        welcome_blob: &payload.welcome_blob,
        group_info_blob: &payload.group_info_blob,
    };

    let pool = state.db_pool.as_ref().ok_or(AuthFailure::Internal)?;
    let now = now_unix();
    let ttl = i64::try_from(state.runtime.e2ee_mailbox_ttl.as_secs())
        .map_err(|_| AuthFailure::Internal)?;
    let expires_at = now.checked_add(ttl).ok_or(AuthFailure::Internal)?;
    let mut transaction = pool.begin().await.map_err(|_| AuthFailure::Internal)?;
    require_active_owned_device(&mut transaction, auth.user_id, committer_device_id).await?;
    let crypto_mode: String = sqlx::query_scalar(
        "SELECT c.conversation_crypto
         FROM e2ee_conversations c
         JOIN e2ee_conversation_members m ON m.conversation_id = c.conversation_id
         WHERE c.conversation_id = $1 AND m.user_id = $2
         FOR UPDATE OF c",
    )
    .bind(conversation_id.to_string())
    .bind(auth.user_id.to_string())
    .fetch_optional(&mut *transaction)
    .await
    .map_err(|_| AuthFailure::Internal)?
    .ok_or(AuthFailure::NotFound)?;

    let crypto_mode =
        ConversationCrypto::try_from(crypto_mode).map_err(|_| AuthFailure::Internal)?;
    if crypto_mode == ConversationCrypto::MlsV1 {
        let existing = existing_provision_for_conversation(&mut transaction, conversation_id)
            .await?
            .ok_or(AuthFailure::E2eeConversationConflict)?;
        if !provision_matches(&existing, &provision) {
            return Err(AuthFailure::E2eeConversationConflict);
        }
        transaction
            .commit()
            .await
            .map_err(|_| AuthFailure::Internal)?;
        return Ok(Json(provision_response(
            &provision,
            existing.provisioned_at_unix,
        )));
    }
    if crypto_mode != ConversationCrypto::Plaintext {
        return Err(AuthFailure::E2eeConversationConflict);
    }

    let member_ids =
        conversation_member_ids(&mut transaction, &conversation_id.to_string()).await?;
    let members: [UserId; 2] = member_ids
        .try_into()
        .map_err(|_| AuthFailure::E2eeConversationConflict)?;
    lock_capable_two_user_membership(&mut transaction, &members).await?;
    let (lower_user_id, upper_user_id) = canonical_user_pair(members[0], members[1]);
    let pair_lock_key = format!("{lower_user_id}:{upper_user_id}");
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
        .bind(pair_lock_key)
        .execute(&mut *transaction)
        .await
        .map_err(|_| AuthFailure::Internal)?;
    if existing_provision_for_pair(&mut transaction, &lower_user_id, &upper_user_id)
        .await?
        .is_some()
    {
        return Err(AuthFailure::E2eeConversationConflict);
    }

    sqlx::query(
        "UPDATE e2ee_conversations SET conversation_crypto = 'mls_v1'
         WHERE conversation_id = $1 AND conversation_crypto = 'plaintext'",
    )
    .bind(conversation_id.to_string())
    .execute(&mut *transaction)
    .await
    .map_err(|_| AuthFailure::Internal)?;
    sqlx::query(
        "INSERT INTO e2ee_dm_pairs (conversation_id, user_a_id, user_b_id)
         VALUES ($1, $2, $3)",
    )
    .bind(conversation_id.to_string())
    .bind(&lower_user_id)
    .bind(&upper_user_id)
    .execute(&mut *transaction)
    .await
    .map_err(|error| map_provision_write_error(&error))?;
    insert_initial_group_and_commit(&mut transaction, &provision, now, expires_at).await?;
    record_conversation_provision_audit(
        &mut transaction,
        "conversation_upgrade",
        auth.user_id,
        &provision,
        now,
    )
    .await?;
    transaction
        .commit()
        .await
        .map_err(|_| AuthFailure::Internal)?;

    emit_mls_commit_notifications(
        &state,
        &members,
        MlsCommitEvent {
            group_id: group_id.to_string(),
            conversation_id: conversation_id.to_string(),
            epoch: INITIAL_MLS_EPOCH,
            prior_epoch: 0,
            committer_device_id: committer_device_id.to_string(),
            created_at_unix: now,
        },
        payload.suite_id,
        true,
    )
    .await;
    Ok(Json(provision_response(&provision, now)))
}

/// Return the latest opaque `GroupInfo` for a locally authorized conversation member.
pub(crate) async fn get_group_info(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(group_id): Path<String>,
) -> Result<Json<GroupInfoResponse>, AuthFailure> {
    let auth = authenticate(&state, &headers).await?;
    let group_id = GroupId::try_from(group_id).map_err(|_| AuthFailure::InvalidRequest)?;
    let pool = state.db_pool.as_ref().ok_or(AuthFailure::Internal)?;
    let group = get_group_for_member(pool, group_id, auth.user_id).await?;
    let group_info_blob = group.group_info_blob.ok_or(AuthFailure::NotFound)?;
    Ok(Json(GroupInfoResponse {
        group_id: group_id.to_string(),
        epoch: group.current_epoch,
        suite_id: group.suite_id,
        group_info_blob,
    }))
}

/// Return a bounded page of opaque messages pending for one active device.
pub(crate) async fn get_group_mailbox(
    State(state): State<AppState>,
    headers: HeaderMap,
    connect_info: Option<Extension<ConnectInfo<SocketAddr>>>,
    Path(group_id): Path<String>,
    Query(query): Query<E2eeMailboxQuery>,
) -> Result<Json<E2eeMailboxResponse>, AuthFailure> {
    let client_ip = extract_client_ip(
        &state,
        &headers,
        connect_info.as_ref().map(|value| value.0 .0.ip()),
    );
    let auth = authenticate(&state, &headers).await?;
    let group_id = GroupId::try_from(group_id).map_err(|_| AuthFailure::InvalidRequest)?;
    let device_id = DeviceId::try_from(query.device_id).map_err(|_| AuthFailure::InvalidRequest)?;
    let limit = mailbox_page_limit(query.limit)?;
    if let Some(cursor) = query.after_message_id.as_deref() {
        parse_canonical_ulid(cursor)?;
    }
    enforce_e2ee_transport_rate_limit(
        &state,
        client_ip,
        auth.user_id,
        device_id,
        group_id,
        E2eeTransportRoute::MailboxRead,
    )
    .await?;

    let pool = state.db_pool.as_ref().ok_or(AuthFailure::Internal)?;
    let now = now_unix();
    let mut transaction = pool.begin().await.map_err(|_| AuthFailure::Internal)?;
    require_active_owned_device(&mut transaction, auth.user_id, device_id).await?;
    let _group =
        get_group_for_member_in_transaction(&mut transaction, group_id, auth.user_id).await?;
    let sql_limit = i64::try_from(limit).map_err(|_| AuthFailure::Internal)?;
    let rows = sqlx::query(
        "SELECT m.message_id, m.crypto_mode, m.epoch, m.suite_id,
                m.sender_device_id, m.ciphertext_blob, m.created_at_unix,
                m.expires_at_unix
         FROM e2ee_message_acks a
         JOIN e2ee_messages m ON m.message_id = a.message_id
         WHERE a.device_id = $1
           AND a.acked_at_unix IS NULL
           AND m.group_id = $2
           AND m.expires_at_unix > $3
           AND ($4::TEXT IS NULL OR m.message_id > $4)
         ORDER BY m.message_id ASC
         LIMIT $5",
    )
    .bind(device_id.to_string())
    .bind(group_id.to_string())
    .bind(now)
    .bind(query.after_message_id.as_deref())
    .bind(sql_limit)
    .fetch_all(&mut *transaction)
    .await
    .map_err(|_| AuthFailure::Internal)?;

    let mut aggregate_bytes = 0_usize;
    let mut messages = Vec::with_capacity(rows.len());
    for row in rows {
        let message_blob: Vec<u8> = row
            .try_get("ciphertext_blob")
            .map_err(|_| AuthFailure::Internal)?;
        let next_aggregate = aggregate_bytes
            .checked_add(message_blob.len())
            .ok_or(AuthFailure::Internal)?;
        if next_aggregate > MAX_E2EE_MAILBOX_PAGE_BLOB_BYTES {
            break;
        }
        let epoch: i64 = row.try_get("epoch").map_err(|_| AuthFailure::Internal)?;
        let suite_id: i32 = row.try_get("suite_id").map_err(|_| AuthFailure::Internal)?;
        messages.push(E2eeMailboxMessage {
            message_id: row
                .try_get("message_id")
                .map_err(|_| AuthFailure::Internal)?,
            crypto: row
                .try_get("crypto_mode")
                .map_err(|_| AuthFailure::Internal)?,
            epoch: u64::try_from(epoch).map_err(|_| AuthFailure::Internal)?,
            suite_id: u16::try_from(suite_id).map_err(|_| AuthFailure::Internal)?,
            sender_device_id: row
                .try_get("sender_device_id")
                .map_err(|_| AuthFailure::Internal)?,
            message_blob,
            created_at_unix: row
                .try_get("created_at_unix")
                .map_err(|_| AuthFailure::Internal)?,
            expires_at_unix: row
                .try_get("expires_at_unix")
                .map_err(|_| AuthFailure::Internal)?,
        });
        aggregate_bytes = next_aggregate;
    }
    transaction
        .commit()
        .await
        .map_err(|_| AuthFailure::Internal)?;
    let next_after_message_id = messages.last().map(|message| message.message_id.clone());
    Ok(Json(E2eeMailboxResponse {
        messages,
        next_after_message_id,
    }))
}

/// Acknowledge messages only after one active device decrypts them successfully.
pub(crate) async fn ack_group_messages(
    State(state): State<AppState>,
    headers: HeaderMap,
    connect_info: Option<Extension<ConnectInfo<SocketAddr>>>,
    Path(group_id): Path<String>,
    Json(payload): Json<AckE2eeMessagesRequest>,
) -> Result<Json<AckE2eeMessagesResponse>, AuthFailure> {
    if payload.message_ids.is_empty() || payload.message_ids.len() > MAX_E2EE_MESSAGE_ACK_BATCH_SIZE
    {
        return Err(AuthFailure::InvalidRequest);
    }
    let mut unique_ids = HashSet::with_capacity(payload.message_ids.len());
    for message_id in &payload.message_ids {
        parse_canonical_ulid(message_id)?;
        if !unique_ids.insert(message_id.as_str()) {
            return Err(AuthFailure::InvalidRequest);
        }
    }
    let client_ip = extract_client_ip(
        &state,
        &headers,
        connect_info.as_ref().map(|value| value.0 .0.ip()),
    );
    let auth = authenticate(&state, &headers).await?;
    let group_id = GroupId::try_from(group_id).map_err(|_| AuthFailure::InvalidRequest)?;
    let device_id =
        DeviceId::try_from(payload.device_id).map_err(|_| AuthFailure::InvalidRequest)?;
    enforce_e2ee_transport_rate_limit(
        &state,
        client_ip,
        auth.user_id,
        device_id,
        group_id,
        E2eeTransportRoute::MailboxAck,
    )
    .await?;

    let pool = state.db_pool.as_ref().ok_or(AuthFailure::Internal)?;
    let now = now_unix();
    let mut transaction = pool.begin().await.map_err(|_| AuthFailure::Internal)?;
    require_active_owned_device(&mut transaction, auth.user_id, device_id).await?;
    let _group = lock_group_for_member(&mut transaction, group_id, auth.user_id).await?;
    let acknowledged_ids: Vec<String> = sqlx::query_scalar(
        "UPDATE e2ee_message_acks a
         SET acked_at_unix = COALESCE(a.acked_at_unix, $1)
         FROM e2ee_messages m
         WHERE m.message_id = a.message_id
           AND m.group_id = $2
           AND m.expires_at_unix > $1
           AND a.device_id = $3
           AND a.message_id = ANY($4::TEXT[])
         RETURNING a.message_id",
    )
    .bind(now)
    .bind(group_id.to_string())
    .bind(device_id.to_string())
    .bind(&payload.message_ids)
    .fetch_all(&mut *transaction)
    .await
    .map_err(|_| AuthFailure::Internal)?;

    let deleted_count = if acknowledged_ids.is_empty() {
        0
    } else {
        sqlx::query(
            "DELETE FROM e2ee_messages m
             WHERE m.group_id = $1
               AND m.message_id = ANY($2::TEXT[])
               AND NOT EXISTS (
                   SELECT 1 FROM e2ee_message_acks pending
                   WHERE pending.message_id = m.message_id
                     AND pending.acked_at_unix IS NULL
               )",
        )
        .bind(group_id.to_string())
        .bind(&acknowledged_ids)
        .execute(&mut *transaction)
        .await
        .map_err(|_| AuthFailure::Internal)?
        .rows_affected()
    };
    transaction
        .commit()
        .await
        .map_err(|_| AuthFailure::Internal)?;
    Ok(Json(AckE2eeMessagesResponse {
        acknowledged_count: u32::try_from(acknowledged_ids.len())
            .map_err(|_| AuthFailure::Internal)?,
        deleted_count: u32::try_from(deleted_count).map_err(|_| AuthFailure::Internal)?,
    }))
}

/// Atomically order and persist one opaque MLS commit.
pub(crate) async fn post_group_commit(
    State(state): State<AppState>,
    headers: HeaderMap,
    connect_info: Option<Extension<ConnectInfo<SocketAddr>>>,
    Path(group_id): Path<String>,
    Json(payload): Json<PostCommitRequest>,
) -> Result<Json<PostCommitResponse>, AuthFailure> {
    let client_ip = extract_client_ip(
        &state,
        &headers,
        connect_info.as_ref().map(|value| value.0 .0.ip()),
    );
    let auth = authenticate(&state, &headers).await?;
    let group_id = GroupId::try_from(group_id).map_err(|_| AuthFailure::InvalidRequest)?;
    let committer_device_id = DeviceId::try_from(payload.committer_device_id.clone())
        .map_err(|_| AuthFailure::InvalidRequest)?;
    if payload.prior_epoch.checked_add(1) != Some(payload.epoch) {
        return Err(AuthFailure::InvalidRequest);
    }
    enforce_e2ee_transport_rate_limit(
        &state,
        client_ip,
        auth.user_id,
        committer_device_id,
        group_id,
        E2eeTransportRoute::Commit,
    )
    .await?;

    let epoch = i64::try_from(payload.epoch).map_err(|_| AuthFailure::InvalidRequest)?;
    let prior_epoch =
        i64::try_from(payload.prior_epoch).map_err(|_| AuthFailure::InvalidRequest)?;
    let pool = state.db_pool.as_ref().ok_or(AuthFailure::Internal)?;
    let now = now_unix();
    let ttl = i64::try_from(state.runtime.e2ee_mailbox_ttl.as_secs())
        .map_err(|_| AuthFailure::Internal)?;
    let expires_at = now.checked_add(ttl).ok_or(AuthFailure::Internal)?;
    let mut transaction = pool.begin().await.map_err(|_| AuthFailure::Internal)?;
    require_active_owned_device(&mut transaction, auth.user_id, committer_device_id).await?;
    let group = lock_group_for_member(&mut transaction, group_id, auth.user_id).await?;
    if group.current_epoch != payload.prior_epoch {
        return Err(AuthFailure::EpochConflict);
    }

    sqlx::query(
        "INSERT INTO e2ee_commits
            (group_id, epoch, prior_epoch, committer_device_id,
             commit_blob, welcome_blob, created_at_unix, expires_at_unix)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
    )
    .bind(group_id.to_string())
    .bind(epoch)
    .bind(prior_epoch)
    .bind(committer_device_id.to_string())
    .bind(&payload.commit_blob)
    .bind(&payload.welcome_blob)
    .bind(now)
    .bind(expires_at)
    .execute(&mut *transaction)
    .await
    .map_err(|error| {
        if error
            .as_database_error()
            .and_then(sqlx::error::DatabaseError::constraint)
            .is_some_and(|constraint| constraint.contains("e2ee_commits_pkey"))
        {
            AuthFailure::EpochConflict
        } else {
            AuthFailure::Internal
        }
    })?;
    sqlx::query(
        "UPDATE e2ee_groups
         SET current_epoch = $2, group_info_blob = COALESCE($3, group_info_blob)
         WHERE group_id = $1",
    )
    .bind(group_id.to_string())
    .bind(epoch)
    .bind(&payload.group_info_blob)
    .execute(&mut *transaction)
    .await
    .map_err(|_| AuthFailure::Internal)?;
    let member_ids = conversation_member_ids(&mut transaction, &group.conversation_id).await?;
    transaction
        .commit()
        .await
        .map_err(|_| AuthFailure::Internal)?;

    emit_mls_commit_notifications(
        &state,
        &member_ids,
        MlsCommitEvent {
            group_id: group_id.to_string(),
            conversation_id: group.conversation_id,
            epoch: payload.epoch,
            prior_epoch: payload.prior_epoch,
            committer_device_id: committer_device_id.to_string(),
            created_at_unix: now,
        },
        group.suite_id,
        payload.welcome_blob.is_some(),
    )
    .await;

    Ok(Json(PostCommitResponse {
        accepted: true,
        epoch: payload.epoch,
    }))
}

/// Persist one padded MLS `PrivateMessage` without parsing its interior.
pub(crate) async fn post_group_message(
    State(state): State<AppState>,
    headers: HeaderMap,
    connect_info: Option<Extension<ConnectInfo<SocketAddr>>>,
    Path(group_id): Path<String>,
    Json(payload): Json<PostMessageRequest>,
) -> Result<Json<PostMessageResponse>, AuthFailure> {
    if !is_valid_message_padding_bucket(payload.message_blob.len()) {
        return Err(AuthFailure::InvalidRequest);
    }
    let client_ip = extract_client_ip(
        &state,
        &headers,
        connect_info.as_ref().map(|value| value.0 .0.ip()),
    );
    let auth = authenticate(&state, &headers).await?;
    let group_id = GroupId::try_from(group_id).map_err(|_| AuthFailure::InvalidRequest)?;
    let sender_device_id = DeviceId::try_from(payload.sender_device_id.clone())
        .map_err(|_| AuthFailure::InvalidRequest)?;
    CiphersuiteId::try_from(payload.suite_id).map_err(|_| AuthFailure::InvalidRequest)?;
    enforce_e2ee_transport_rate_limit(
        &state,
        client_ip,
        auth.user_id,
        sender_device_id,
        group_id,
        E2eeTransportRoute::Message,
    )
    .await?;

    let epoch = i64::try_from(payload.epoch).map_err(|_| AuthFailure::InvalidRequest)?;
    let pool = state.db_pool.as_ref().ok_or(AuthFailure::Internal)?;
    let now = now_unix();
    let ttl = i64::try_from(state.runtime.e2ee_mailbox_ttl.as_secs())
        .map_err(|_| AuthFailure::Internal)?;
    let expires_at = now.checked_add(ttl).ok_or(AuthFailure::Internal)?;
    let mut transaction = pool.begin().await.map_err(|_| AuthFailure::Internal)?;
    require_active_owned_device(&mut transaction, auth.user_id, sender_device_id).await?;
    let group = lock_group_for_member(&mut transaction, group_id, auth.user_id).await?;
    if group.current_epoch != payload.epoch {
        return Err(AuthFailure::EpochConflict);
    }
    if group.suite_id != payload.suite_id {
        return Err(AuthFailure::InvalidRequest);
    }

    let message_id = ulid::Ulid::new().to_string();
    sqlx::query(
        "INSERT INTO e2ee_messages
            (message_id, group_id, sender_device_id, epoch, suite_id,
             ciphertext_blob, created_at_unix, expires_at_unix)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
    )
    .bind(&message_id)
    .bind(group_id.to_string())
    .bind(sender_device_id.to_string())
    .bind(epoch)
    .bind(i32::from(payload.suite_id))
    .bind(&payload.message_blob)
    .bind(now)
    .bind(expires_at)
    .execute(&mut *transaction)
    .await
    .map_err(|_| AuthFailure::Internal)?;
    snapshot_message_deliveries(
        &mut transaction,
        &group.conversation_id,
        &message_id,
        sender_device_id,
        now,
    )
    .await?;
    let member_ids = conversation_member_ids(&mut transaction, &group.conversation_id).await?;
    transaction
        .commit()
        .await
        .map_err(|_| AuthFailure::Internal)?;

    match gateway_events::try_mls_message(MlsMessageEvent {
        group_id: group_id.to_string(),
        conversation_id: group.conversation_id,
        message_id: message_id.clone(),
        epoch: payload.epoch,
        suite_id: payload.suite_id,
        sender_device_id: sender_device_id.to_string(),
        created_at_unix: now,
    }) {
        Ok(event) => broadcast_conversation_event(&state, &member_ids, &event).await,
        Err(error) => {
            record_gateway_event_serialize_error("user", gateway_events::MLS_MESSAGE_EVENT);
            tracing::error!(
                event = "gateway.mls_message.serialize_failed",
                error = %error,
                "dropped MLS message notification because serialization failed"
            );
        }
    }

    Ok(Json(PostMessageResponse {
        message_id,
        created_at_unix: now,
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

    #[test]
    fn private_message_padding_accepts_only_protocol_buckets() {
        for bucket in E2EE_MESSAGE_PADDING_BUCKETS {
            assert!(is_valid_message_padding_bucket(bucket));
        }
        for invalid in [0, 511, 513, 2_048, 16_385, 65_536] {
            assert!(!is_valid_message_padding_bucket(invalid));
        }
    }

    #[test]
    fn mailbox_pagination_requires_canonical_bounded_inputs() {
        assert_eq!(mailbox_page_limit(None).unwrap(), 20);
        assert_eq!(mailbox_page_limit(Some(50)).unwrap(), 50);
        assert!(mailbox_page_limit(Some(0)).is_err());
        assert!(mailbox_page_limit(Some(51)).is_err());

        let canonical = ulid::Ulid::new().to_string();
        assert!(parse_canonical_ulid(&canonical).is_ok());
        assert!(parse_canonical_ulid(&canonical.to_lowercase()).is_err());
        assert!(parse_canonical_ulid("not-a-ulid").is_err());
    }
}
