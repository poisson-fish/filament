//! E2EE directory and `KeyPackage` Delivery Service endpoints.
//!
//! The server verifies public device certificates and applies shape/rate
//! limits, but never receives private key material or parses MLS interiors.

use std::{collections::HashSet, fmt::Write as _, net::SocketAddr};

use axum::{
    body::{Body, Bytes},
    extract::{connect_info::ConnectInfo, Extension, Path, Query, State},
    http::{header::CONTENT_LENGTH, header::CONTENT_TYPE, HeaderMap, HeaderName, HeaderValue},
    response::Response,
    Json,
};
use filament_core::{
    CiphersuiteId, ConversationCrypto, ConversationId, DeviceId, GroupId, ProposalId, UserId,
};
use filament_e2ee::{
    verify_device_certificate, verify_root_identity_rotation_proof, AttachmentId,
    RootIdentityRotationProof, MAX_MLS_GROUP_LEAVES, MAX_MLS_GROUP_USERS,
};
use filament_protocol::{
    AckE2eeAttachmentsRequest, AckE2eeAttachmentsResponse, AckE2eeCommitsRequest,
    AckE2eeCommitsResponse, AckE2eeMessagesRequest, AckE2eeMessagesResponse,
    AckE2eeProposalsRequest, AckE2eeProposalsResponse, ClaimKeyPackageRequest,
    ClaimKeyPackageResponse, CreateMlsConversationRequest, CreateMlsGroupConversationRequest,
    DeliveryServiceIdentityResponse, DeviceInfo, DeviceListResponse, E2eeCommitMailboxEntry,
    E2eeCommitMailboxQuery, E2eeCommitMailboxResponse, E2eeMailboxMessage, E2eeMailboxQuery,
    E2eeMailboxResponse, E2eeProposalMailboxEntry, E2eeProposalMailboxQuery,
    E2eeProposalMailboxResponse, GetE2eeAttachmentQuery, GroupInfoResponse, MlsCommitEvent,
    MlsConversationProvisionResponse, MlsLeafRouting, MlsMembershipChange,
    MlsMembershipChangeEvent, MlsMessageEvent, MlsProposalEvent, MlsWelcomeEvent,
    PostCommitRequest, PostCommitResponse, PostMessageRequest, PostMessageResponse,
    PostProposalRequest, PostProposalResponse, PublishDeviceCertificateRequest,
    PublishDeviceCertificateResponse, PutE2eeAttachmentQuery, PutE2eeAttachmentResponse,
    RemoveDeviceResponse, RootIdentityDirectoryResponse, RootIdentityRotationEntry,
    RotateRootIdentityRequest, RotateRootIdentityResponse, UpgradeMlsConversationRequest,
    UploadKeyPackagesRequest, UploadKeyPackagesResponse, DELIVERY_SERVICE_EXTERNAL_SENDER_INDEX,
    DELIVERY_SERVICE_IDENTITY_PROTOCOL_VERSION, E2EE_ATTACHMENT_CIPHERTEXT_BUCKETS,
    MAX_E2EE_ATTACHMENT_ACK_BATCH_SIZE, MAX_E2EE_COMMIT_ACK_BATCH_SIZE,
    MAX_E2EE_COMMIT_MAILBOX_PAGE_BLOB_BYTES, MAX_E2EE_COMMIT_MAILBOX_PAGE_SIZE,
    MAX_E2EE_MAILBOX_PAGE_BLOB_BYTES, MAX_E2EE_MAILBOX_PAGE_SIZE, MAX_E2EE_MESSAGE_ACK_BATCH_SIZE,
    MAX_E2EE_PROPOSAL_ACK_BATCH_SIZE, MAX_E2EE_PROPOSAL_MAILBOX_PAGE_BLOB_BYTES,
    MAX_E2EE_PROPOSAL_MAILBOX_PAGE_SIZE, MAX_ROOT_IDENTITY_ROTATIONS,
    ROOT_IDENTITY_ROTATION_PROTOCOL_VERSION,
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
const INITIAL_MLS_EPOCH: u64 = 1;

type CertificateFields = ([u8; 32], [u8; 64], [u8; 32]);
type RotationFields = ([u8; 32], [u8; 64], [u8; 64], [u8; 32], [u8; 64]);

/// Return authenticated public configuration for the server's stable MLS
/// external sender. The private key never crosses this boundary.
pub(crate) async fn get_delivery_service_identity(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<DeliveryServiceIdentityResponse>, AuthFailure> {
    let _auth = authenticate(&state, &headers).await?;
    let signer = state
        .e2ee_delivery_service
        .as_ref()
        .ok_or(AuthFailure::E2eeCapabilityRequired)?;
    Ok(Json(DeliveryServiceIdentityResponse {
        protocol_version: DELIVERY_SERVICE_IDENTITY_PROTOCOL_VERSION,
        external_sender_index: DELIVERY_SERVICE_EXTERNAL_SENDER_INDEX,
        signature_key: signer.identity().signature_key().to_vec(),
    }))
}

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
    welcome_device_id: DeviceId,
    commit_blob: &'a [u8],
    welcome_blob: &'a [u8],
    group_info_blob: &'a [u8],
}

struct ExistingProvision {
    conversation_id: String,
    group_id: String,
    suite_id: u16,
    committer_device_id: String,
    welcome_device_id: Option<String>,
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

fn is_valid_attachment_padding_bucket(byte_len: usize) -> bool {
    E2EE_ATTACHMENT_CIPHERTEXT_BUCKETS.contains(&byte_len)
}

fn fits_attachment_quota(current_usage: u64, new_bytes: u64, quota_bytes: u64) -> bool {
    current_usage
        .checked_add(new_bytes)
        .is_some_and(|total| total <= quota_bytes)
}

async fn locked_attachment_usage_for_user(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: UserId,
) -> Result<u64, AuthFailure> {
    let owner_id = user_id.to_string();
    let lock_key = format!("attachment-quota:{owner_id}");
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
        .bind(lock_key)
        .execute(&mut **transaction)
        .await
        .map_err(|_| AuthFailure::Internal)?;
    let plaintext_usage: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(size_bytes), 0)::BIGINT
         FROM attachments WHERE owner_id = $1",
    )
    .bind(&owner_id)
    .fetch_one(&mut **transaction)
    .await
    .map_err(|_| AuthFailure::Internal)?;
    let encrypted_usage: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(octet_length(ciphertext_blob)), 0)::BIGINT
         FROM e2ee_attachment_blobs WHERE owner_user_id = $1",
    )
    .bind(owner_id)
    .fetch_one(&mut **transaction)
    .await
    .map_err(|_| AuthFailure::Internal)?;
    let plaintext_usage = u64::try_from(plaintext_usage).map_err(|_| AuthFailure::Internal)?;
    let encrypted_usage = u64::try_from(encrypted_usage).map_err(|_| AuthFailure::Internal)?;
    plaintext_usage
        .checked_add(encrypted_usage)
        .ok_or(AuthFailure::Internal)
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

async fn require_current_group_leaf(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    group_id: GroupId,
    user_id: UserId,
    device_id: DeviceId,
) -> Result<(), AuthFailure> {
    let present = sqlx::query(
        "SELECT 1 FROM e2ee_group_leaves
         WHERE group_id = $1 AND user_id = $2 AND device_id = $3
         FOR SHARE",
    )
    .bind(group_id.to_string())
    .bind(user_id.to_string())
    .bind(device_id.to_string())
    .fetch_optional(&mut **transaction)
    .await
    .map_err(|_| AuthFailure::Internal)?;
    present.map(|_| ()).ok_or(AuthFailure::NotFound)
}

async fn active_device_owner(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    device_id: DeviceId,
) -> Result<UserId, AuthFailure> {
    let owner: String = sqlx::query_scalar(
        "SELECT user_id FROM e2ee_device_certificates
         WHERE device_id = $1 AND tombstoned_at_unix IS NULL
         FOR SHARE",
    )
    .bind(device_id.to_string())
    .fetch_optional(&mut **transaction)
    .await
    .map_err(|_| AuthFailure::Internal)?
    .ok_or(AuthFailure::NotFound)?;
    UserId::try_from(owner).map_err(|_| AuthFailure::Internal)
}

async fn require_welcome_target_for_conversation(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    conversation_id: &str,
    committer_device_id: DeviceId,
    welcome_device_id: DeviceId,
) -> Result<UserId, AuthFailure> {
    if committer_device_id == welcome_device_id {
        return Err(AuthFailure::InvalidRequest);
    }
    let owner = active_device_owner(transaction, welcome_device_id).await?;
    let is_member = sqlx::query(
        "SELECT 1 FROM e2ee_conversation_members
         WHERE conversation_id = $1 AND user_id = $2",
    )
    .bind(conversation_id)
    .bind(owner.to_string())
    .fetch_optional(&mut **transaction)
    .await
    .map_err(|_| AuthFailure::Internal)?
    .is_some();
    if !is_member {
        return Err(AuthFailure::InvalidRequest);
    }
    Ok(owner)
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
    let max_device_count =
        i64::try_from(MAX_MLS_GROUP_LEAVES).map_err(|_| AuthFailure::Internal)?;
    if capable_users.len() != 2 || device_count > max_device_count {
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
        welcome_device_id: row
            .try_get("welcome_device_id")
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
                k.committer_device_id, k.welcome_device_id, k.commit_blob, k.welcome_blob,
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
                k.committer_device_id, k.welcome_device_id, k.commit_blob, k.welcome_blob,
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
        && existing
            .welcome_device_id
            .as_ref()
            .is_some_and(|device_id| device_id == &expected.welcome_device_id.to_string())
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
             commit_blob, welcome_blob, welcome_device_id, created_at_unix, expires_at_unix)
         VALUES ($1, 1, 0, $2, $3, $4, $5, $6, $7)",
    )
    .bind(provision.group_id.to_string())
    .bind(provision.committer_device_id.to_string())
    .bind(provision.commit_blob)
    .bind(provision.welcome_blob)
    .bind(provision.welcome_device_id.to_string())
    .bind(now)
    .bind(expires_at)
    .execute(&mut **transaction)
    .await
    .map_err(|error| map_provision_write_error(&error))?;
    Ok(())
}

async fn insert_group_leaf(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    group_id: GroupId,
    leaf: &MlsLeafRouting,
    added_epoch: u64,
) -> Result<(), AuthFailure> {
    let leaf_index = i32::try_from(leaf.leaf_index).map_err(|_| AuthFailure::InvalidRequest)?;
    let added_epoch = i64::try_from(added_epoch).map_err(|_| AuthFailure::InvalidRequest)?;
    sqlx::query(
        "INSERT INTO e2ee_group_leaves
            (group_id, leaf_index, user_id, device_id, added_epoch)
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(group_id.to_string())
    .bind(leaf_index)
    .bind(&leaf.user_id)
    .bind(&leaf.device_id)
    .bind(added_epoch)
    .execute(&mut **transaction)
    .await
    .map_err(|error| map_provision_write_error(&error))?;
    Ok(())
}

async fn insert_welcome_recipients(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    group_id: GroupId,
    epoch: u64,
    device_ids: &[DeviceId],
) -> Result<(), AuthFailure> {
    let epoch = i64::try_from(epoch).map_err(|_| AuthFailure::Internal)?;
    for device_id in device_ids {
        sqlx::query(
            "INSERT INTO e2ee_commit_welcome_recipients (group_id, epoch, device_id)
             VALUES ($1, $2, $3)
             ON CONFLICT (group_id, epoch, device_id) DO NOTHING",
        )
        .bind(group_id.to_string())
        .bind(epoch)
        .bind(device_id.to_string())
        .execute(&mut **transaction)
        .await
        .map_err(|_| AuthFailure::Internal)?;
    }
    Ok(())
}

#[allow(clippy::too_many_lines)]
async fn apply_membership_change(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    conversation_id: &str,
    group_id: GroupId,
    epoch: u64,
    change: &MlsMembershipChange,
    welcome_device_id: Option<DeviceId>,
    now: i64,
) -> Result<Vec<UserId>, AuthFailure> {
    match change {
        MlsMembershipChange::Add { leaf } => {
            if leaf.leaf_index >= u32::try_from(MAX_MLS_GROUP_LEAVES).unwrap_or(u32::MAX) {
                return Err(AuthFailure::InvalidRequest);
            }
            parse_canonical_ulid(&leaf.user_id)?;
            parse_canonical_ulid(&leaf.device_id)?;
            let user_id =
                UserId::try_from(leaf.user_id.clone()).map_err(|_| AuthFailure::InvalidRequest)?;
            let device_id = DeviceId::try_from(leaf.device_id.clone())
                .map_err(|_| AuthFailure::InvalidRequest)?;
            if welcome_device_id != Some(device_id) {
                return Err(AuthFailure::InvalidRequest);
            }
            require_active_owned_device(transaction, user_id, device_id).await?;
            let (leaf_count, user_count): (i64, i64) = sqlx::query_as(
                "SELECT COUNT(*), COUNT(DISTINCT user_id)
                 FROM e2ee_group_leaves WHERE group_id = $1",
            )
            .bind(group_id.to_string())
            .fetch_one(&mut **transaction)
            .await
            .map_err(|_| AuthFailure::Internal)?;
            let existing_user: bool = sqlx::query_scalar(
                "SELECT EXISTS (
                    SELECT 1 FROM e2ee_group_leaves WHERE group_id = $1 AND user_id = $2
                 )",
            )
            .bind(group_id.to_string())
            .bind(user_id.to_string())
            .fetch_one(&mut **transaction)
            .await
            .map_err(|_| AuthFailure::Internal)?;
            let max_leaves =
                i64::try_from(MAX_MLS_GROUP_LEAVES).map_err(|_| AuthFailure::Internal)?;
            let max_users =
                i64::try_from(MAX_MLS_GROUP_USERS).map_err(|_| AuthFailure::Internal)?;
            if leaf_count >= max_leaves || (!existing_user && user_count >= max_users) {
                return Err(AuthFailure::E2eeCapabilityRequired);
            }
            insert_group_leaf(transaction, group_id, leaf, epoch).await?;
            sqlx::query(
                "INSERT INTO e2ee_conversation_members (conversation_id, user_id, joined_at_unix)
                 VALUES ($1, $2, $3)
                 ON CONFLICT (conversation_id, user_id) DO NOTHING",
            )
            .bind(conversation_id)
            .bind(user_id.to_string())
            .bind(now)
            .execute(&mut **transaction)
            .await
            .map_err(|_| AuthFailure::Internal)?;
            Ok(vec![user_id])
        }
        MlsMembershipChange::Remove { leaves } => {
            if welcome_device_id.is_some() {
                return Err(AuthFailure::InvalidRequest);
            }
            let mut exact = HashSet::with_capacity(leaves.len());
            let mut affected_users = HashSet::with_capacity(leaves.len());
            for leaf in leaves {
                parse_canonical_ulid(&leaf.user_id)?;
                parse_canonical_ulid(&leaf.device_id)?;
                let user_id = UserId::try_from(leaf.user_id.clone())
                    .map_err(|_| AuthFailure::InvalidRequest)?;
                let device_id = DeviceId::try_from(leaf.device_id.clone())
                    .map_err(|_| AuthFailure::InvalidRequest)?;
                if !exact.insert((leaf.leaf_index, user_id, device_id)) {
                    return Err(AuthFailure::InvalidRequest);
                }
                let deleted = sqlx::query(
                    "DELETE FROM e2ee_group_leaves
                     WHERE group_id = $1 AND leaf_index = $2 AND user_id = $3 AND device_id = $4",
                )
                .bind(group_id.to_string())
                .bind(i32::try_from(leaf.leaf_index).map_err(|_| AuthFailure::InvalidRequest)?)
                .bind(user_id.to_string())
                .bind(device_id.to_string())
                .execute(&mut **transaction)
                .await
                .map_err(|_| AuthFailure::Internal)?;
                if deleted.rows_affected() != 1 {
                    return Err(AuthFailure::InvalidRequest);
                }
                affected_users.insert(user_id);
                sqlx::query(
                    "UPDATE e2ee_membership_reconciliations
                     SET completed_epoch = $4
                     WHERE group_id = $1 AND target_device_id = $2 AND leaf_index = $3
                       AND completed_epoch IS NULL",
                )
                .bind(group_id.to_string())
                .bind(device_id.to_string())
                .bind(i32::try_from(leaf.leaf_index).map_err(|_| AuthFailure::InvalidRequest)?)
                .bind(i64::try_from(epoch).map_err(|_| AuthFailure::Internal)?)
                .execute(&mut **transaction)
                .await
                .map_err(|_| AuthFailure::Internal)?;
            }
            let (leaf_count, user_count): (i64, i64) = sqlx::query_as(
                "SELECT COUNT(*), COUNT(DISTINCT user_id)
                 FROM e2ee_group_leaves WHERE group_id = $1",
            )
            .bind(group_id.to_string())
            .fetch_one(&mut **transaction)
            .await
            .map_err(|_| AuthFailure::Internal)?;
            if leaf_count < 2 || user_count < 2 {
                return Err(AuthFailure::InvalidRequest);
            }
            for user_id in affected_users {
                sqlx::query(
                    "DELETE FROM e2ee_conversation_members m
                     WHERE m.conversation_id = $1 AND m.user_id = $2
                       AND NOT EXISTS (
                           SELECT 1 FROM e2ee_group_leaves l
                           WHERE l.group_id = $3 AND l.user_id = m.user_id
                       )",
                )
                .bind(conversation_id)
                .bind(user_id.to_string())
                .bind(group_id.to_string())
                .execute(&mut **transaction)
                .await
                .map_err(|_| AuthFailure::Internal)?;
            }
            sqlx::query(
                "DELETE FROM e2ee_proposals p
                 USING e2ee_membership_reconciliations r
                 WHERE p.reconciliation_id = r.reconciliation_id
                   AND r.group_id = $1 AND r.completed_epoch = $2",
            )
            .bind(group_id.to_string())
            .bind(i64::try_from(epoch).map_err(|_| AuthFailure::Internal)?)
            .execute(&mut **transaction)
            .await
            .map_err(|_| AuthFailure::Internal)?;
            Ok(Vec::new())
        }
    }
}

struct PendingPolicyProposal {
    member_ids: Vec<UserId>,
    event: MlsProposalEvent,
}

#[allow(clippy::too_many_lines)]
async fn queue_policy_removals_for_device(
    state: &AppState,
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: UserId,
    device_id: DeviceId,
    now: i64,
) -> Result<Vec<PendingPolicyProposal>, AuthFailure> {
    let leaves = sqlx::query(
        "SELECT l.group_id, l.leaf_index, g.current_epoch, g.conversation_id
         FROM e2ee_group_leaves l
         JOIN e2ee_groups g ON g.group_id = l.group_id
         WHERE l.user_id = $1 AND l.device_id = $2
         ORDER BY l.group_id
         FOR UPDATE OF g",
    )
    .bind(user_id.to_string())
    .bind(device_id.to_string())
    .fetch_all(&mut **transaction)
    .await
    .map_err(|_| AuthFailure::Internal)?;
    if leaves.is_empty() {
        return Ok(Vec::new());
    }
    let signer = state
        .e2ee_delivery_service
        .as_ref()
        .ok_or(AuthFailure::E2eeCapabilityRequired)?;
    let reconciliation_window = i64::try_from(
        state
            .runtime
            .e2ee_membership_reconciliation_window
            .as_secs(),
    )
    .map_err(|_| AuthFailure::Internal)?;
    let deadline = now
        .checked_add(reconciliation_window)
        .ok_or(AuthFailure::Internal)?;
    let mailbox_ttl = i64::try_from(state.runtime.e2ee_mailbox_ttl.as_secs())
        .map_err(|_| AuthFailure::Internal)?;
    let expires_at = now.checked_add(mailbox_ttl).ok_or(AuthFailure::Internal)?;
    let mut pending = Vec::with_capacity(leaves.len());
    for row in leaves {
        let group_id: String = row.try_get("group_id").map_err(|_| AuthFailure::Internal)?;
        let group_id = GroupId::try_from(group_id).map_err(|_| AuthFailure::Internal)?;
        let leaf_index: i32 = row
            .try_get("leaf_index")
            .map_err(|_| AuthFailure::Internal)?;
        let leaf_index = u32::try_from(leaf_index).map_err(|_| AuthFailure::Internal)?;
        let epoch: i64 = row
            .try_get("current_epoch")
            .map_err(|_| AuthFailure::Internal)?;
        let epoch = u64::try_from(epoch).map_err(|_| AuthFailure::Internal)?;
        let conversation_id: String = row
            .try_get("conversation_id")
            .map_err(|_| AuthFailure::Internal)?;
        let reconciliation_id = ulid::Ulid::new().to_string();
        let inserted = sqlx::query(
            "INSERT INTO e2ee_membership_reconciliations
                (reconciliation_id, group_id, target_user_id, target_device_id,
                 leaf_index, requested_at_unix, deadline_unix)
             VALUES ($1, $2, $3, $4, $5, $6, $7)
             ON CONFLICT (group_id, target_device_id, leaf_index) DO NOTHING",
        )
        .bind(&reconciliation_id)
        .bind(group_id.to_string())
        .bind(user_id.to_string())
        .bind(device_id.to_string())
        .bind(i32::try_from(leaf_index).map_err(|_| AuthFailure::Internal)?)
        .bind(now)
        .bind(deadline)
        .execute(&mut **transaction)
        .await
        .map_err(|_| AuthFailure::Internal)?;
        if inserted.rows_affected() == 0 {
            continue;
        }
        let proposal = signer
            .sign_remove(group_id, epoch, leaf_index)
            .map_err(|_| AuthFailure::Internal)?;
        let proposal_id = ProposalId::new();
        sqlx::query(
            "INSERT INTO e2ee_proposals
                (proposal_id, group_id, epoch, proposer_device_id, external_sender_index,
                 reconciliation_id, reconciliation_deadline_unix, proposal_blob,
                 created_at_unix, expires_at_unix)
             VALUES ($1, $2, $3, NULL, $4, $5, $6, $7, $8, $9)",
        )
        .bind(proposal_id.to_string())
        .bind(group_id.to_string())
        .bind(i64::try_from(epoch).map_err(|_| AuthFailure::Internal)?)
        .bind(
            i32::try_from(DELIVERY_SERVICE_EXTERNAL_SENDER_INDEX)
                .map_err(|_| AuthFailure::Internal)?,
        )
        .bind(&reconciliation_id)
        .bind(deadline)
        .bind(&proposal.proposal_blob)
        .bind(now)
        .bind(expires_at)
        .execute(&mut **transaction)
        .await
        .map_err(|_| AuthFailure::Internal)?;
        snapshot_proposal_deliveries(
            transaction,
            &conversation_id,
            group_id,
            proposal_id,
            None,
            now,
        )
        .await?;
        let member_ids = conversation_member_ids(transaction, &conversation_id).await?;
        pending.push(PendingPolicyProposal {
            member_ids,
            event: MlsProposalEvent {
                group_id: group_id.to_string(),
                conversation_id,
                proposal_id: proposal_id.to_string(),
                epoch,
                proposer_device_id: None,
                external_sender_index: Some(DELIVERY_SERVICE_EXTERNAL_SENDER_INDEX),
                reconciliation_deadline_unix: Some(deadline),
                created_at_unix: now,
            },
        });
    }
    Ok(pending)
}

async fn emit_policy_proposals(state: &AppState, pending: Vec<PendingPolicyProposal>) {
    for proposal in pending {
        match gateway_events::try_mls_proposal(proposal.event) {
            Ok(event) => broadcast_conversation_event(state, &proposal.member_ids, &event).await,
            Err(error) => {
                record_gateway_event_serialize_error("user", gateway_events::MLS_PROPOSAL_EVENT);
                tracing::error!(
                    event = "gateway.mls_policy_proposal.serialize_failed",
                    error = %error,
                    "dropped policy Remove notification because serialization failed"
                );
            }
        }
    }
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

fn commit_mailbox_page_limit(limit: Option<u16>) -> Result<usize, AuthFailure> {
    let limit = limit.map_or(DEFAULT_E2EE_MAILBOX_PAGE_SIZE, usize::from);
    if limit == 0 || limit > MAX_E2EE_COMMIT_MAILBOX_PAGE_SIZE {
        return Err(AuthFailure::InvalidRequest);
    }
    Ok(limit)
}

fn proposal_mailbox_page_limit(limit: Option<u16>) -> Result<usize, AuthFailure> {
    let limit = limit.map_or(DEFAULT_E2EE_MAILBOX_PAGE_SIZE, usize::from);
    if limit == 0 || limit > MAX_E2EE_PROPOSAL_MAILBOX_PAGE_SIZE {
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

fn validate_delivery_audience_counts(
    member_count: i64,
    capable_member_count: i64,
    device_count: i64,
) -> Result<(), AuthFailure> {
    let max_member_count = i64::try_from(MAX_MLS_GROUP_USERS).map_err(|_| AuthFailure::Internal)?;
    let max_device_count =
        i64::try_from(MAX_MLS_GROUP_LEAVES).map_err(|_| AuthFailure::Internal)?;
    if !(2..=max_member_count).contains(&member_count)
        || capable_member_count != member_count
        || !(member_count..=max_device_count).contains(&device_count)
    {
        return Err(AuthFailure::E2eeCapabilityRequired);
    }
    Ok(())
}

async fn snapshot_message_deliveries(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    conversation_id: &str,
    group_id: GroupId,
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
        "SELECT l.device_id, l.user_id
         FROM e2ee_group_leaves l
         JOIN e2ee_device_certificates d
           ON d.device_id = l.device_id AND d.user_id = l.user_id
          AND d.tombstoned_at_unix IS NULL
         WHERE l.group_id = $1
         ORDER BY l.leaf_index ASC
         FOR SHARE OF d",
    )
    .bind(group_id.to_string())
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
    validate_delivery_audience_counts(member_count, capable_member_count, device_count)?;

    let inserted = sqlx::query(
        "INSERT INTO e2ee_message_acks (message_id, device_id, acked_at_unix)
         SELECT $1, l.device_id,
                CASE WHEN l.device_id = $2 THEN $3 ELSE NULL END
         FROM e2ee_group_leaves l
         JOIN e2ee_device_certificates d
           ON d.device_id = l.device_id AND d.user_id = l.user_id
          AND d.tombstoned_at_unix IS NULL
         WHERE l.group_id = $4",
    )
    .bind(message_id)
    .bind(sender_device_id.to_string())
    .bind(created_at_unix)
    .bind(group_id.to_string())
    .execute(&mut **transaction)
    .await
    .map_err(|_| AuthFailure::Internal)?;
    let inserted = i64::try_from(inserted.rows_affected()).map_err(|_| AuthFailure::Internal)?;
    if inserted != device_count {
        return Err(AuthFailure::Internal);
    }
    Ok(())
}

async fn snapshot_attachment_deliveries(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    conversation_id: &str,
    group_id: GroupId,
    attachment_id: AttachmentId,
    uploader_device_id: DeviceId,
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
        "SELECT l.device_id, l.user_id
         FROM e2ee_group_leaves l
         JOIN e2ee_device_certificates d
           ON d.device_id = l.device_id AND d.user_id = l.user_id
          AND d.tombstoned_at_unix IS NULL
         WHERE l.group_id = $1
         ORDER BY l.leaf_index ASC
         FOR SHARE OF d",
    )
    .bind(group_id.to_string())
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
    validate_delivery_audience_counts(member_count, capable_member_count, device_count)?;

    let inserted = sqlx::query(
        "INSERT INTO e2ee_attachment_deliveries
            (attachment_id, device_id, acked_at_unix)
         SELECT $1, l.device_id,
                CASE WHEN l.device_id = $2 THEN $3 ELSE NULL END
         FROM e2ee_group_leaves l
         JOIN e2ee_device_certificates d
           ON d.device_id = l.device_id AND d.user_id = l.user_id
          AND d.tombstoned_at_unix IS NULL
         WHERE l.group_id = $4",
    )
    .bind(attachment_id.to_string())
    .bind(uploader_device_id.to_string())
    .bind(created_at_unix)
    .bind(group_id.to_string())
    .execute(&mut **transaction)
    .await
    .map_err(|_| AuthFailure::Internal)?;
    let inserted = i64::try_from(inserted.rows_affected()).map_err(|_| AuthFailure::Internal)?;
    if inserted != device_count {
        return Err(AuthFailure::Internal);
    }
    Ok(())
}

async fn snapshot_commit_deliveries(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    conversation_id: &str,
    group_id: GroupId,
    epoch: u64,
    committer_device_id: DeviceId,
    created_at_unix: i64,
) -> Result<(), AuthFailure> {
    let epoch = i64::try_from(epoch).map_err(|_| AuthFailure::Internal)?;
    let member_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM e2ee_conversation_members WHERE conversation_id = $1",
    )
    .bind(conversation_id)
    .fetch_one(&mut **transaction)
    .await
    .map_err(|_| AuthFailure::Internal)?;
    let device_rows = sqlx::query(
        "SELECT l.device_id, l.user_id
         FROM e2ee_group_leaves l
         JOIN e2ee_device_certificates d
           ON d.device_id = l.device_id AND d.user_id = l.user_id
          AND d.tombstoned_at_unix IS NULL
         WHERE l.group_id = $1
         ORDER BY l.leaf_index ASC
         FOR SHARE OF d",
    )
    .bind(group_id.to_string())
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
    validate_delivery_audience_counts(member_count, capable_member_count, device_count)?;
    let inserted = sqlx::query(
        "INSERT INTO e2ee_commit_deliveries
            (group_id, epoch, device_id, acked_at_unix)
         SELECT $1, $2, l.device_id,
                CASE WHEN l.device_id = $3 THEN $4 ELSE NULL END
         FROM e2ee_group_leaves l
         JOIN e2ee_device_certificates d
           ON d.device_id = l.device_id AND d.user_id = l.user_id
          AND d.tombstoned_at_unix IS NULL
         WHERE l.group_id = $5",
    )
    .bind(group_id.to_string())
    .bind(epoch)
    .bind(committer_device_id.to_string())
    .bind(created_at_unix)
    .bind(group_id.to_string())
    .execute(&mut **transaction)
    .await
    .map_err(|_| AuthFailure::Internal)?;
    let inserted = i64::try_from(inserted.rows_affected()).map_err(|_| AuthFailure::Internal)?;
    if inserted != device_count {
        return Err(AuthFailure::Internal);
    }
    Ok(())
}

async fn snapshot_proposal_deliveries(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    conversation_id: &str,
    group_id: GroupId,
    proposal_id: ProposalId,
    proposer_device_id: Option<DeviceId>,
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
        "SELECT l.device_id, l.user_id
         FROM e2ee_group_leaves l
         JOIN e2ee_device_certificates d
           ON d.device_id = l.device_id AND d.user_id = l.user_id
          AND d.tombstoned_at_unix IS NULL
         WHERE l.group_id = $1
         ORDER BY l.leaf_index ASC
         FOR SHARE OF d",
    )
    .bind(group_id.to_string())
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
    if proposer_device_id.is_some() {
        validate_delivery_audience_counts(member_count, capable_member_count, device_count)?;
    } else {
        let max_device_count =
            i64::try_from(MAX_MLS_GROUP_LEAVES).map_err(|_| AuthFailure::Internal)?;
        if !(1..=max_device_count).contains(&device_count) {
            return Err(AuthFailure::E2eeCapabilityRequired);
        }
    }

    let inserted = sqlx::query(
        "INSERT INTO e2ee_proposal_deliveries
            (proposal_id, device_id, acked_at_unix)
         SELECT $1, l.device_id,
                CASE WHEN l.device_id = $2 THEN $3 ELSE NULL END
         FROM e2ee_group_leaves l
         JOIN e2ee_device_certificates d
           ON d.device_id = l.device_id AND d.user_id = l.user_id
          AND d.tombstoned_at_unix IS NULL
         WHERE l.group_id = $4",
    )
    .bind(proposal_id.to_string())
    .bind(proposer_device_id.map(|device_id| device_id.to_string()))
    .bind(created_at_unix)
    .bind(group_id.to_string())
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
    welcome_user_ids: &[UserId],
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
    for welcome_user_id in welcome_user_ids {
        match gateway_events::try_mls_welcome(MlsWelcomeEvent {
            group_id: commit.group_id.clone(),
            conversation_id: commit.conversation_id.clone(),
            epoch: commit.epoch,
            suite_id,
            created_at_unix: commit.created_at_unix,
        }) {
            Ok(event) => broadcast_user_event(state, *welcome_user_id, &event).await,
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
    let pending_policy_proposals =
        queue_policy_removals_for_device(&state, &mut transaction, auth.user_id, device_id, now)
            .await?;
    transaction
        .commit()
        .await
        .map_err(|_| AuthFailure::Internal)?;

    emit_device_list_update(&state, auth.user_id, active_device_count, now).await;
    emit_policy_proposals(&state, pending_policy_proposals).await;

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
         WHERE user_id = $1 AND device_id <> $2 AND tombstoned_at_unix IS NULL
         RETURNING device_id",
    )
    .bind(auth.user_id.to_string())
    .bind(device_id.to_string())
    .bind(now)
    .fetch_all(&mut *transaction)
    .await
    .map_err(|_| AuthFailure::Internal)?;
    let revoked_device_count = u32::try_from(revoked.len()).map_err(|_| AuthFailure::Internal)?;
    let revoked_device_ids = revoked
        .into_iter()
        .map(|row| {
            let value: String = row
                .try_get("device_id")
                .map_err(|_| AuthFailure::Internal)?;
            DeviceId::try_from(value).map_err(|_| AuthFailure::Internal)
        })
        .collect::<Result<Vec<_>, _>>()?;
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
    let mut pending_policy_proposals = Vec::new();
    for revoked_device_id in revoked_device_ids {
        pending_policy_proposals.extend(
            queue_policy_removals_for_device(
                &state,
                &mut transaction,
                auth.user_id,
                revoked_device_id,
                now,
            )
            .await?,
        );
    }
    transaction
        .commit()
        .await
        .map_err(|_| AuthFailure::Internal)?;

    emit_device_list_update(&state, auth.user_id, 1, now).await;
    emit_policy_proposals(&state, pending_policy_proposals).await;
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

/// Atomically create a bounded group DM with one shared multi-recipient Welcome.
#[allow(clippy::too_many_lines)]
pub(crate) async fn create_mls_group_conversation(
    State(state): State<AppState>,
    headers: HeaderMap,
    connect_info: Option<Extension<ConnectInfo<SocketAddr>>>,
    Json(payload): Json<CreateMlsGroupConversationRequest>,
) -> Result<Json<MlsConversationProvisionResponse>, AuthFailure> {
    let client_ip = extract_client_ip(
        &state,
        &headers,
        connect_info.as_ref().map(|value| value.0 .0.ip()),
    );
    let auth = authenticate(&state, &headers).await?;
    if state.e2ee_delivery_service.is_none() {
        return Err(AuthFailure::E2eeCapabilityRequired);
    }
    for value in [
        &payload.conversation_id,
        &payload.group_id,
        &payload.committer_device_id,
    ] {
        parse_canonical_ulid(value)?;
    }
    let conversation_id = ConversationId::try_from(payload.conversation_id.clone())
        .map_err(|_| AuthFailure::InvalidRequest)?;
    let group_id =
        GroupId::try_from(payload.group_id.clone()).map_err(|_| AuthFailure::InvalidRequest)?;
    let committer_device_id = DeviceId::try_from(payload.committer_device_id.clone())
        .map_err(|_| AuthFailure::InvalidRequest)?;
    CiphersuiteId::try_from(payload.suite_id).map_err(|_| AuthFailure::InvalidRequest)?;

    let mut invitees = Vec::with_capacity(payload.invitees.len());
    let mut user_ids = HashSet::with_capacity(payload.invitees.len() + 1);
    let mut device_ids = HashSet::with_capacity(payload.invitees.len() + 1);
    user_ids.insert(auth.user_id);
    device_ids.insert(committer_device_id);
    for (offset, invitee) in payload.invitees.iter().enumerate() {
        parse_canonical_ulid(&invitee.user_id)?;
        parse_canonical_ulid(&invitee.welcome_device_id)?;
        let expected_leaf_index =
            u32::try_from(offset + 1).map_err(|_| AuthFailure::InvalidRequest)?;
        if invitee.leaf_index != expected_leaf_index {
            return Err(AuthFailure::InvalidRequest);
        }
        let user_id =
            UserId::try_from(invitee.user_id.clone()).map_err(|_| AuthFailure::InvalidRequest)?;
        let device_id = DeviceId::try_from(invitee.welcome_device_id.clone())
            .map_err(|_| AuthFailure::InvalidRequest)?;
        if !user_ids.insert(user_id) || !device_ids.insert(device_id) {
            return Err(AuthFailure::InvalidRequest);
        }
        invitees.push((user_id, device_id, invitee.leaf_index));
    }
    enforce_e2ee_transport_rate_limit(
        &state,
        client_ip,
        auth.user_id,
        committer_device_id,
        group_id,
        E2eeTransportRoute::Provision,
    )
    .await?;

    let pool = state.db_pool.as_ref().ok_or(AuthFailure::Internal)?;
    let now = now_unix();
    let ttl = i64::try_from(state.runtime.e2ee_mailbox_ttl.as_secs())
        .map_err(|_| AuthFailure::Internal)?;
    let expires_at = now.checked_add(ttl).ok_or(AuthFailure::Internal)?;
    let mut transaction = pool.begin().await.map_err(|_| AuthFailure::Internal)?;
    require_active_owned_device(&mut transaction, auth.user_id, committer_device_id).await?;
    for (user_id, device_id, _) in &invitees {
        require_active_owned_device(&mut transaction, *user_id, *device_id).await?;
    }
    let lock_key = format!("group-provision:{conversation_id}:{group_id}");
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
        .bind(lock_key)
        .execute(&mut *transaction)
        .await
        .map_err(|_| AuthFailure::Internal)?;

    let existing = sqlx::query(
        "SELECT g.conversation_id, g.group_id, g.suite_id, k.committer_device_id,
                k.commit_blob, k.welcome_blob, g.group_info_blob, g.created_at_unix
         FROM e2ee_groups g
         JOIN e2ee_commits k ON k.group_id = g.group_id AND k.epoch = 1
         WHERE g.group_id = $1 OR g.conversation_id = $2
         FOR UPDATE OF g",
    )
    .bind(group_id.to_string())
    .bind(conversation_id.to_string())
    .fetch_optional(&mut *transaction)
    .await
    .map_err(|_| AuthFailure::Internal)?;
    if let Some(existing) = existing {
        let existing_group: String = existing
            .try_get("group_id")
            .map_err(|_| AuthFailure::Internal)?;
        let leaves: Vec<(i32, String, String)> = sqlx::query_as(
            "SELECT leaf_index, user_id, device_id FROM e2ee_group_leaves
             WHERE group_id = $1 ORDER BY leaf_index",
        )
        .bind(&existing_group)
        .fetch_all(&mut *transaction)
        .await
        .map_err(|_| AuthFailure::Internal)?;
        let expected_leaves = std::iter::once((
            0_i32,
            auth.user_id.to_string(),
            committer_device_id.to_string(),
        ))
        .chain(invitees.iter().map(|(user_id, device_id, leaf_index)| {
            (
                i32::try_from(*leaf_index).unwrap_or(i32::MAX),
                user_id.to_string(),
                device_id.to_string(),
            )
        }))
        .collect::<Vec<_>>();
        let exact_retry = existing
            .try_get::<String, _>("conversation_id")
            .is_ok_and(|value| value == conversation_id.to_string())
            && existing_group == group_id.to_string()
            && existing
                .try_get::<i32, _>("suite_id")
                .is_ok_and(|value| value == i32::from(payload.suite_id))
            && existing
                .try_get::<String, _>("committer_device_id")
                .is_ok_and(|value| value == committer_device_id.to_string())
            && existing
                .try_get::<Vec<u8>, _>("commit_blob")
                .is_ok_and(|value| value == payload.commit_blob)
            && existing
                .try_get::<Option<Vec<u8>>, _>("welcome_blob")
                .is_ok_and(|value| value.as_deref() == Some(payload.welcome_blob.as_slice()))
            && existing
                .try_get::<Option<Vec<u8>>, _>("group_info_blob")
                .is_ok_and(|value| value.as_deref() == Some(payload.group_info_blob.as_slice()))
            && leaves == expected_leaves;
        if !exact_retry {
            return Err(AuthFailure::E2eeConversationConflict);
        }
        let provisioned_at_unix = existing
            .try_get("created_at_unix")
            .map_err(|_| AuthFailure::Internal)?;
        transaction
            .commit()
            .await
            .map_err(|_| AuthFailure::Internal)?;
        return Ok(Json(MlsConversationProvisionResponse {
            conversation_id: conversation_id.to_string(),
            group_id: group_id.to_string(),
            crypto: String::from("mls_v1"),
            epoch: INITIAL_MLS_EPOCH,
            suite_id: payload.suite_id,
            provisioned_at_unix,
        }));
    }

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
    for user_id in std::iter::once(auth.user_id).chain(invitees.iter().map(|value| value.0)) {
        sqlx::query(
            "INSERT INTO e2ee_conversation_members (conversation_id, user_id, joined_at_unix)
             VALUES ($1, $2, $3)",
        )
        .bind(conversation_id.to_string())
        .bind(user_id.to_string())
        .bind(now)
        .execute(&mut *transaction)
        .await
        .map_err(|error| map_provision_write_error(&error))?;
    }
    sqlx::query(
        "INSERT INTO e2ee_groups
            (group_id, conversation_id, current_epoch, suite_id, group_info_blob, created_at_unix)
         VALUES ($1, $2, 1, $3, $4, $5)",
    )
    .bind(group_id.to_string())
    .bind(conversation_id.to_string())
    .bind(i32::from(payload.suite_id))
    .bind(&payload.group_info_blob)
    .bind(now)
    .execute(&mut *transaction)
    .await
    .map_err(|error| map_provision_write_error(&error))?;
    sqlx::query(
        "INSERT INTO e2ee_commits
            (group_id, epoch, prior_epoch, committer_device_id, commit_blob,
             welcome_blob, welcome_device_id, created_at_unix, expires_at_unix)
         VALUES ($1, 1, 0, $2, $3, $4, $5, $6, $7)",
    )
    .bind(group_id.to_string())
    .bind(committer_device_id.to_string())
    .bind(&payload.commit_blob)
    .bind(&payload.welcome_blob)
    .bind(invitees[0].1.to_string())
    .bind(now)
    .bind(expires_at)
    .execute(&mut *transaction)
    .await
    .map_err(|error| map_provision_write_error(&error))?;
    insert_group_leaf(
        &mut transaction,
        group_id,
        &MlsLeafRouting {
            leaf_index: 0,
            user_id: auth.user_id.to_string(),
            device_id: committer_device_id.to_string(),
        },
        INITIAL_MLS_EPOCH,
    )
    .await?;
    let mut welcome_devices = Vec::with_capacity(invitees.len());
    for (user_id, device_id, leaf_index) in &invitees {
        insert_group_leaf(
            &mut transaction,
            group_id,
            &MlsLeafRouting {
                leaf_index: *leaf_index,
                user_id: user_id.to_string(),
                device_id: device_id.to_string(),
            },
            INITIAL_MLS_EPOCH,
        )
        .await?;
        welcome_devices.push(*device_id);
    }
    insert_welcome_recipients(
        &mut transaction,
        group_id,
        INITIAL_MLS_EPOCH,
        &welcome_devices,
    )
    .await?;
    snapshot_commit_deliveries(
        &mut transaction,
        &conversation_id.to_string(),
        group_id,
        INITIAL_MLS_EPOCH,
        committer_device_id,
        now,
    )
    .await?;
    let member_ids = std::iter::once(auth.user_id)
        .chain(invitees.iter().map(|value| value.0))
        .collect::<Vec<_>>();
    let welcome_user_ids = invitees.iter().map(|value| value.0).collect::<Vec<_>>();
    transaction
        .commit()
        .await
        .map_err(|_| AuthFailure::Internal)?;
    emit_mls_commit_notifications(
        &state,
        &member_ids,
        MlsCommitEvent {
            group_id: group_id.to_string(),
            conversation_id: conversation_id.to_string(),
            epoch: INITIAL_MLS_EPOCH,
            prior_epoch: 0,
            committer_device_id: committer_device_id.to_string(),
            created_at_unix: now,
        },
        payload.suite_id,
        &welcome_user_ids,
    )
    .await;
    Ok(Json(MlsConversationProvisionResponse {
        conversation_id: conversation_id.to_string(),
        group_id: group_id.to_string(),
        crypto: String::from("mls_v1"),
        epoch: INITIAL_MLS_EPOCH,
        suite_id: payload.suite_id,
        provisioned_at_unix: now,
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
    parse_canonical_ulid(&payload.welcome_device_id)?;
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
    let welcome_device_id = DeviceId::try_from(payload.welcome_device_id.clone())
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
        welcome_device_id,
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
    require_active_owned_device(&mut transaction, peer_user_id, welcome_device_id).await?;
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
    insert_group_leaf(
        &mut transaction,
        group_id,
        &MlsLeafRouting {
            leaf_index: 0,
            user_id: auth.user_id.to_string(),
            device_id: committer_device_id.to_string(),
        },
        INITIAL_MLS_EPOCH,
    )
    .await?;
    insert_group_leaf(
        &mut transaction,
        group_id,
        &MlsLeafRouting {
            leaf_index: 1,
            user_id: peer_user_id.to_string(),
            device_id: welcome_device_id.to_string(),
        },
        INITIAL_MLS_EPOCH,
    )
    .await?;
    insert_welcome_recipients(
        &mut transaction,
        group_id,
        INITIAL_MLS_EPOCH,
        &[welcome_device_id],
    )
    .await?;
    snapshot_commit_deliveries(
        &mut transaction,
        &conversation_id.to_string(),
        group_id,
        INITIAL_MLS_EPOCH,
        committer_device_id,
        now,
    )
    .await?;
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
        &[peer_user_id],
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
    parse_canonical_ulid(&payload.welcome_device_id)?;
    let conversation_id =
        ConversationId::try_from(conversation_id).map_err(|_| AuthFailure::InvalidRequest)?;
    let group_id =
        GroupId::try_from(payload.group_id.clone()).map_err(|_| AuthFailure::InvalidRequest)?;
    let committer_device_id = DeviceId::try_from(payload.committer_device_id.clone())
        .map_err(|_| AuthFailure::InvalidRequest)?;
    let welcome_device_id = DeviceId::try_from(payload.welcome_device_id.clone())
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
        welcome_device_id,
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
    let welcome_user_id = require_welcome_target_for_conversation(
        &mut transaction,
        &conversation_id.to_string(),
        committer_device_id,
        welcome_device_id,
    )
    .await?;
    if welcome_user_id == auth.user_id {
        return Err(AuthFailure::InvalidRequest);
    }
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
    insert_group_leaf(
        &mut transaction,
        group_id,
        &MlsLeafRouting {
            leaf_index: 0,
            user_id: auth.user_id.to_string(),
            device_id: committer_device_id.to_string(),
        },
        INITIAL_MLS_EPOCH,
    )
    .await?;
    insert_group_leaf(
        &mut transaction,
        group_id,
        &MlsLeafRouting {
            leaf_index: 1,
            user_id: welcome_user_id.to_string(),
            device_id: welcome_device_id.to_string(),
        },
        INITIAL_MLS_EPOCH,
    )
    .await?;
    insert_welcome_recipients(
        &mut transaction,
        group_id,
        INITIAL_MLS_EPOCH,
        &[welcome_device_id],
    )
    .await?;
    snapshot_commit_deliveries(
        &mut transaction,
        &conversation_id.to_string(),
        group_id,
        INITIAL_MLS_EPOCH,
        committer_device_id,
        now,
    )
    .await?;
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
        &[welcome_user_id],
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

/// Store one exact-bucket encrypted attachment without parsing its interior.
#[allow(clippy::too_many_lines)]
pub(crate) async fn put_group_attachment(
    State(state): State<AppState>,
    headers: HeaderMap,
    connect_info: Option<Extension<ConnectInfo<SocketAddr>>>,
    Path((group_id, attachment_id)): Path<(String, String)>,
    Query(query): Query<PutE2eeAttachmentQuery>,
    ciphertext: Bytes,
) -> Result<Json<PutE2eeAttachmentResponse>, AuthFailure> {
    if !is_valid_attachment_padding_bucket(ciphertext.len()) {
        return Err(AuthFailure::InvalidRequest);
    }
    let client_ip = extract_client_ip(
        &state,
        &headers,
        connect_info.as_ref().map(|value| value.0 .0.ip()),
    );
    let auth = authenticate(&state, &headers).await?;
    let group_id = GroupId::try_from(group_id).map_err(|_| AuthFailure::InvalidRequest)?;
    let attachment_id =
        AttachmentId::try_from(attachment_id).map_err(|_| AuthFailure::InvalidRequest)?;
    let uploader_device_id =
        DeviceId::try_from(query.device_id).map_err(|_| AuthFailure::InvalidRequest)?;
    enforce_e2ee_transport_rate_limit(
        &state,
        client_ip,
        auth.user_id,
        uploader_device_id,
        group_id,
        E2eeTransportRoute::AttachmentUpload,
    )
    .await?;

    let ciphertext_bytes =
        u64::try_from(ciphertext.len()).map_err(|_| AuthFailure::PayloadTooLarge)?;
    let pool = state.db_pool.as_ref().ok_or(AuthFailure::Internal)?;
    let now = now_unix();
    let ttl = i64::try_from(state.runtime.e2ee_mailbox_ttl.as_secs())
        .map_err(|_| AuthFailure::Internal)?;
    let expires_at = now.checked_add(ttl).ok_or(AuthFailure::Internal)?;
    let mut transaction = pool.begin().await.map_err(|_| AuthFailure::Internal)?;
    require_active_owned_device(&mut transaction, auth.user_id, uploader_device_id).await?;
    let group = lock_group_for_member(&mut transaction, group_id, auth.user_id).await?;
    require_current_group_leaf(&mut transaction, group_id, auth.user_id, uploader_device_id)
        .await?;
    let reconciliation_pending: bool = sqlx::query_scalar(
        "SELECT EXISTS (
            SELECT 1 FROM e2ee_membership_reconciliations
            WHERE group_id = $1 AND completed_epoch IS NULL
         )",
    )
    .bind(group_id.to_string())
    .fetch_one(&mut *transaction)
    .await
    .map_err(|_| AuthFailure::Internal)?;
    if reconciliation_pending {
        return Err(AuthFailure::E2eeMembershipReconciliationPending);
    }

    if let Some(existing) = sqlx::query(
        "SELECT group_id, owner_user_id, uploader_device_id, ciphertext_blob,
                expires_at_unix
         FROM e2ee_attachment_blobs WHERE attachment_id = $1 FOR UPDATE",
    )
    .bind(attachment_id.to_string())
    .fetch_optional(&mut *transaction)
    .await
    .map_err(|_| AuthFailure::Internal)?
    {
        let existing_expires_at: i64 = existing
            .try_get("expires_at_unix")
            .map_err(|_| AuthFailure::Internal)?;
        if existing_expires_at <= now {
            sqlx::query("DELETE FROM e2ee_attachment_blobs WHERE attachment_id = $1")
                .bind(attachment_id.to_string())
                .execute(&mut *transaction)
                .await
                .map_err(|_| AuthFailure::Internal)?;
        } else {
            let exact_retry = existing
                .try_get::<String, _>("group_id")
                .is_ok_and(|value| value == group_id.to_string())
                && existing
                    .try_get::<String, _>("owner_user_id")
                    .is_ok_and(|value| value == auth.user_id.to_string())
                && existing
                    .try_get::<String, _>("uploader_device_id")
                    .is_ok_and(|value| value == uploader_device_id.to_string())
                && existing
                    .try_get::<Vec<u8>, _>("ciphertext_blob")
                    .is_ok_and(|value| value.as_slice() == ciphertext.as_ref());
            if !exact_retry {
                return Err(AuthFailure::E2eeAttachmentConflict);
            }
            transaction
                .commit()
                .await
                .map_err(|_| AuthFailure::Internal)?;
            return Ok(Json(PutE2eeAttachmentResponse {
                attachment_id: attachment_id.to_string(),
                ciphertext_bytes,
                expires_at_unix: existing_expires_at,
            }));
        }
    }

    let usage = locked_attachment_usage_for_user(&mut transaction, auth.user_id).await?;
    if !fits_attachment_quota(
        usage,
        ciphertext_bytes,
        state.runtime.user_attachment_quota_bytes,
    ) {
        return Err(AuthFailure::QuotaExceeded);
    }
    sqlx::query(
        "INSERT INTO e2ee_attachment_blobs
            (attachment_id, group_id, owner_user_id, uploader_device_id,
             ciphertext_blob, created_at_unix, expires_at_unix)
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(attachment_id.to_string())
    .bind(group_id.to_string())
    .bind(auth.user_id.to_string())
    .bind(uploader_device_id.to_string())
    .bind(ciphertext.as_ref())
    .bind(now)
    .bind(expires_at)
    .execute(&mut *transaction)
    .await
    .map_err(|error| {
        if error
            .as_database_error()
            .and_then(sqlx::error::DatabaseError::constraint)
            .is_some_and(|constraint| constraint.contains("e2ee_attachment_blobs_pkey"))
        {
            AuthFailure::E2eeAttachmentConflict
        } else {
            AuthFailure::Internal
        }
    })?;
    snapshot_attachment_deliveries(
        &mut transaction,
        &group.conversation_id,
        group_id,
        attachment_id,
        uploader_device_id,
        now,
    )
    .await?;
    transaction
        .commit()
        .await
        .map_err(|_| AuthFailure::Internal)?;
    Ok(Json(PutE2eeAttachmentResponse {
        attachment_id: attachment_id.to_string(),
        ciphertext_bytes,
        expires_at_unix: expires_at,
    }))
}

/// Return one opaque attachment only to a snapshotted active group device.
pub(crate) async fn get_group_attachment(
    State(state): State<AppState>,
    headers: HeaderMap,
    connect_info: Option<Extension<ConnectInfo<SocketAddr>>>,
    Path((group_id, attachment_id)): Path<(String, String)>,
    Query(query): Query<GetE2eeAttachmentQuery>,
) -> Result<Response, AuthFailure> {
    let client_ip = extract_client_ip(
        &state,
        &headers,
        connect_info.as_ref().map(|value| value.0 .0.ip()),
    );
    let auth = authenticate(&state, &headers).await?;
    let group_id = GroupId::try_from(group_id).map_err(|_| AuthFailure::InvalidRequest)?;
    let attachment_id =
        AttachmentId::try_from(attachment_id).map_err(|_| AuthFailure::InvalidRequest)?;
    let device_id = DeviceId::try_from(query.device_id).map_err(|_| AuthFailure::InvalidRequest)?;
    enforce_e2ee_transport_rate_limit(
        &state,
        client_ip,
        auth.user_id,
        device_id,
        group_id,
        E2eeTransportRoute::AttachmentRead,
    )
    .await?;

    let pool = state.db_pool.as_ref().ok_or(AuthFailure::Internal)?;
    let now = now_unix();
    let mut transaction = pool.begin().await.map_err(|_| AuthFailure::Internal)?;
    require_active_owned_device(&mut transaction, auth.user_id, device_id).await?;
    let _group =
        get_group_for_member_in_transaction(&mut transaction, group_id, auth.user_id).await?;
    require_current_group_leaf(&mut transaction, group_id, auth.user_id, device_id).await?;
    let ciphertext: Vec<u8> = sqlx::query_scalar(
        "SELECT b.ciphertext_blob
         FROM e2ee_attachment_deliveries d
         JOIN e2ee_attachment_blobs b ON b.attachment_id = d.attachment_id
         WHERE d.attachment_id = $1 AND d.device_id = $2
           AND b.group_id = $3 AND b.expires_at_unix > $4",
    )
    .bind(attachment_id.to_string())
    .bind(device_id.to_string())
    .bind(group_id.to_string())
    .bind(now)
    .fetch_optional(&mut *transaction)
    .await
    .map_err(|_| AuthFailure::Internal)?
    .ok_or(AuthFailure::NotFound)?;
    if !is_valid_attachment_padding_bucket(ciphertext.len()) {
        return Err(AuthFailure::Internal);
    }
    transaction
        .commit()
        .await
        .map_err(|_| AuthFailure::Internal)?;

    let ciphertext_len = ciphertext.len();
    let mut response = Response::new(Body::from(ciphertext));
    response.headers_mut().insert(
        CONTENT_TYPE,
        HeaderValue::from_static("application/octet-stream"),
    );
    let content_length =
        HeaderValue::from_str(&ciphertext_len.to_string()).map_err(|_| AuthFailure::Internal)?;
    response
        .headers_mut()
        .insert(CONTENT_LENGTH, content_length);
    response.headers_mut().insert(
        HeaderName::from_static("x-content-type-options"),
        HeaderValue::from_static("nosniff"),
    );
    response.headers_mut().insert(
        HeaderName::from_static("cache-control"),
        HeaderValue::from_static("private, no-store"),
    );
    Ok(response)
}

/// Hard-delete blobs after every snapshotted device durably verifies them.
pub(crate) async fn ack_group_attachments(
    State(state): State<AppState>,
    headers: HeaderMap,
    connect_info: Option<Extension<ConnectInfo<SocketAddr>>>,
    Path(group_id): Path<String>,
    Json(payload): Json<AckE2eeAttachmentsRequest>,
) -> Result<Json<AckE2eeAttachmentsResponse>, AuthFailure> {
    if payload.attachment_ids.is_empty()
        || payload.attachment_ids.len() > MAX_E2EE_ATTACHMENT_ACK_BATCH_SIZE
    {
        return Err(AuthFailure::InvalidRequest);
    }
    let mut unique_ids = HashSet::with_capacity(payload.attachment_ids.len());
    let attachment_ids = payload
        .attachment_ids
        .iter()
        .map(|attachment_id| {
            let attachment_id = AttachmentId::try_from(attachment_id.clone())
                .map_err(|_| AuthFailure::InvalidRequest)?;
            if !unique_ids.insert(attachment_id) {
                return Err(AuthFailure::InvalidRequest);
            }
            Ok(attachment_id.to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
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
        E2eeTransportRoute::AttachmentAck,
    )
    .await?;

    let pool = state.db_pool.as_ref().ok_or(AuthFailure::Internal)?;
    let now = now_unix();
    let mut transaction = pool.begin().await.map_err(|_| AuthFailure::Internal)?;
    require_active_owned_device(&mut transaction, auth.user_id, device_id).await?;
    let _group = lock_group_for_member(&mut transaction, group_id, auth.user_id).await?;
    require_current_group_leaf(&mut transaction, group_id, auth.user_id, device_id).await?;
    let acknowledged_ids: Vec<String> = sqlx::query_scalar(
        "UPDATE e2ee_attachment_deliveries d
         SET acked_at_unix = COALESCE(d.acked_at_unix, $1)
         FROM e2ee_attachment_blobs b
         WHERE b.attachment_id = d.attachment_id
           AND b.group_id = $2 AND b.expires_at_unix > $1
           AND d.device_id = $3
           AND d.attachment_id = ANY($4::TEXT[])
         RETURNING d.attachment_id",
    )
    .bind(now)
    .bind(group_id.to_string())
    .bind(device_id.to_string())
    .bind(&attachment_ids)
    .fetch_all(&mut *transaction)
    .await
    .map_err(|_| AuthFailure::Internal)?;
    let deleted_count = if acknowledged_ids.is_empty() {
        0
    } else {
        sqlx::query(
            "DELETE FROM e2ee_attachment_blobs b
             WHERE b.group_id = $1
               AND b.attachment_id = ANY($2::TEXT[])
               AND NOT EXISTS (
                   SELECT 1 FROM e2ee_attachment_deliveries pending
                   WHERE pending.attachment_id = b.attachment_id
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
    Ok(Json(AckE2eeAttachmentsResponse {
        acknowledged_count: u32::try_from(acknowledged_ids.len())
            .map_err(|_| AuthFailure::Internal)?,
        deleted_count: u32::try_from(deleted_count).map_err(|_| AuthFailure::Internal)?,
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

/// Return a bounded page of opaque commits pending for one active device.
#[allow(clippy::too_many_lines)]
pub(crate) async fn get_group_commit_mailbox(
    State(state): State<AppState>,
    headers: HeaderMap,
    connect_info: Option<Extension<ConnectInfo<SocketAddr>>>,
    Path(group_id): Path<String>,
    Query(query): Query<E2eeCommitMailboxQuery>,
) -> Result<Json<E2eeCommitMailboxResponse>, AuthFailure> {
    let client_ip = extract_client_ip(
        &state,
        &headers,
        connect_info.as_ref().map(|value| value.0 .0.ip()),
    );
    let auth = authenticate(&state, &headers).await?;
    let group_id = GroupId::try_from(group_id).map_err(|_| AuthFailure::InvalidRequest)?;
    let device_id = DeviceId::try_from(query.device_id).map_err(|_| AuthFailure::InvalidRequest)?;
    let limit = commit_mailbox_page_limit(query.limit)?;
    enforce_e2ee_transport_rate_limit(
        &state,
        client_ip,
        auth.user_id,
        device_id,
        group_id,
        E2eeTransportRoute::MailboxRead,
    )
    .await?;

    let after_epoch =
        i64::try_from(query.after_epoch.unwrap_or(0)).map_err(|_| AuthFailure::InvalidRequest)?;
    let sql_limit = i64::try_from(limit).map_err(|_| AuthFailure::Internal)?;
    let pool = state.db_pool.as_ref().ok_or(AuthFailure::Internal)?;
    let now = now_unix();
    let mut transaction = pool.begin().await.map_err(|_| AuthFailure::Internal)?;
    require_active_owned_device(&mut transaction, auth.user_id, device_id).await?;
    let _group =
        get_group_for_member_in_transaction(&mut transaction, group_id, auth.user_id).await?;
    let rows = sqlx::query(
        "SELECT k.epoch, k.prior_epoch, k.committer_device_id, k.commit_blob,
                CASE WHEN EXISTS (
                    SELECT 1 FROM e2ee_commit_welcome_recipients wr
                    WHERE wr.group_id = k.group_id AND wr.epoch = k.epoch
                      AND wr.device_id = $1
                ) THEN k.welcome_blob ELSE NULL END AS welcome_blob,
                k.membership_change_json, k.created_at_unix, k.expires_at_unix
         FROM e2ee_commit_deliveries d
         JOIN e2ee_commits k ON k.group_id = d.group_id AND k.epoch = d.epoch
         WHERE d.device_id = $1
           AND d.acked_at_unix IS NULL
           AND k.group_id = $2
           AND k.expires_at_unix > $3
           AND k.epoch > $4
         ORDER BY k.epoch ASC
         LIMIT $5",
    )
    .bind(device_id.to_string())
    .bind(group_id.to_string())
    .bind(now)
    .bind(after_epoch)
    .bind(sql_limit)
    .fetch_all(&mut *transaction)
    .await
    .map_err(|_| AuthFailure::Internal)?;

    let mut aggregate_bytes = 0_usize;
    let mut commits = Vec::with_capacity(rows.len());
    for row in rows {
        let commit_blob: Vec<u8> = row
            .try_get("commit_blob")
            .map_err(|_| AuthFailure::Internal)?;
        let welcome_blob: Option<Vec<u8>> = row
            .try_get("welcome_blob")
            .map_err(|_| AuthFailure::Internal)?;
        let next_aggregate = aggregate_bytes
            .checked_add(commit_blob.len())
            .and_then(|value| value.checked_add(welcome_blob.as_ref().map_or(0, Vec::len)))
            .ok_or(AuthFailure::Internal)?;
        if next_aggregate > MAX_E2EE_COMMIT_MAILBOX_PAGE_BLOB_BYTES {
            break;
        }
        let epoch: i64 = row.try_get("epoch").map_err(|_| AuthFailure::Internal)?;
        let prior_epoch: i64 = row
            .try_get("prior_epoch")
            .map_err(|_| AuthFailure::Internal)?;
        commits.push(E2eeCommitMailboxEntry {
            epoch: u64::try_from(epoch).map_err(|_| AuthFailure::Internal)?,
            prior_epoch: u64::try_from(prior_epoch).map_err(|_| AuthFailure::Internal)?,
            committer_device_id: row
                .try_get("committer_device_id")
                .map_err(|_| AuthFailure::Internal)?,
            commit_blob,
            welcome_blob,
            membership_change: row
                .try_get::<Option<serde_json::Value>, _>("membership_change_json")
                .map_err(|_| AuthFailure::Internal)?
                .map(serde_json::from_value)
                .transpose()
                .map_err(|_| AuthFailure::Internal)?,
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
    let next_after_epoch = commits.last().map(|commit| commit.epoch);
    Ok(Json(E2eeCommitMailboxResponse {
        commits,
        next_after_epoch,
    }))
}

/// Acknowledge commits only after one active device processes them successfully.
pub(crate) async fn ack_group_commits(
    State(state): State<AppState>,
    headers: HeaderMap,
    connect_info: Option<Extension<ConnectInfo<SocketAddr>>>,
    Path(group_id): Path<String>,
    Json(payload): Json<AckE2eeCommitsRequest>,
) -> Result<Json<AckE2eeCommitsResponse>, AuthFailure> {
    if payload.epochs.is_empty() || payload.epochs.len() > MAX_E2EE_COMMIT_ACK_BATCH_SIZE {
        return Err(AuthFailure::InvalidRequest);
    }
    let mut unique_epochs = HashSet::with_capacity(payload.epochs.len());
    let epochs = payload
        .epochs
        .iter()
        .map(|epoch| {
            if *epoch == 0 || !unique_epochs.insert(*epoch) {
                return Err(AuthFailure::InvalidRequest);
            }
            i64::try_from(*epoch).map_err(|_| AuthFailure::InvalidRequest)
        })
        .collect::<Result<Vec<_>, _>>()?;
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
    let acknowledged_epochs: Vec<i64> = sqlx::query_scalar(
        "UPDATE e2ee_commit_deliveries d
         SET acked_at_unix = COALESCE(d.acked_at_unix, $1)
         FROM e2ee_commits k
         WHERE k.group_id = d.group_id AND k.epoch = d.epoch
           AND k.group_id = $2
           AND k.expires_at_unix > $1
           AND d.device_id = $3
           AND d.epoch = ANY($4::BIGINT[])
         RETURNING d.epoch",
    )
    .bind(now)
    .bind(group_id.to_string())
    .bind(device_id.to_string())
    .bind(&epochs)
    .fetch_all(&mut *transaction)
    .await
    .map_err(|_| AuthFailure::Internal)?;
    let deleted_count = if acknowledged_epochs.is_empty() {
        0
    } else {
        sqlx::query(
            "DELETE FROM e2ee_commits k
             WHERE k.group_id = $1
               AND k.epoch = ANY($2::BIGINT[])
               AND NOT EXISTS (
                   SELECT 1 FROM e2ee_commit_deliveries pending
                   WHERE pending.group_id = k.group_id
                     AND pending.epoch = k.epoch
                     AND pending.acked_at_unix IS NULL
               )",
        )
        .bind(group_id.to_string())
        .bind(&acknowledged_epochs)
        .execute(&mut *transaction)
        .await
        .map_err(|_| AuthFailure::Internal)?
        .rows_affected()
    };
    transaction
        .commit()
        .await
        .map_err(|_| AuthFailure::Internal)?;
    Ok(Json(AckE2eeCommitsResponse {
        acknowledged_count: u32::try_from(acknowledged_epochs.len())
            .map_err(|_| AuthFailure::Internal)?,
        deleted_count: u32::try_from(deleted_count).map_err(|_| AuthFailure::Internal)?,
    }))
}

/// Return a bounded page of opaque MLS proposals pending for one active device.
#[allow(clippy::too_many_lines)]
pub(crate) async fn get_group_proposal_mailbox(
    State(state): State<AppState>,
    headers: HeaderMap,
    connect_info: Option<Extension<ConnectInfo<SocketAddr>>>,
    Path(group_id): Path<String>,
    Query(query): Query<E2eeProposalMailboxQuery>,
) -> Result<Json<E2eeProposalMailboxResponse>, AuthFailure> {
    let client_ip = extract_client_ip(
        &state,
        &headers,
        connect_info.as_ref().map(|value| value.0 .0.ip()),
    );
    let auth = authenticate(&state, &headers).await?;
    let group_id = GroupId::try_from(group_id).map_err(|_| AuthFailure::InvalidRequest)?;
    let device_id = DeviceId::try_from(query.device_id).map_err(|_| AuthFailure::InvalidRequest)?;
    let after_proposal_id = query
        .after_proposal_id
        .map(ProposalId::try_from)
        .transpose()
        .map_err(|_| AuthFailure::InvalidRequest)?;
    let limit = proposal_mailbox_page_limit(query.limit)?;
    enforce_e2ee_transport_rate_limit(
        &state,
        client_ip,
        auth.user_id,
        device_id,
        group_id,
        E2eeTransportRoute::MailboxRead,
    )
    .await?;

    let sql_limit = i64::try_from(limit).map_err(|_| AuthFailure::Internal)?;
    let pool = state.db_pool.as_ref().ok_or(AuthFailure::Internal)?;
    let now = now_unix();
    let mut transaction = pool.begin().await.map_err(|_| AuthFailure::Internal)?;
    require_active_owned_device(&mut transaction, auth.user_id, device_id).await?;
    let _group =
        get_group_for_member_in_transaction(&mut transaction, group_id, auth.user_id).await?;
    let rows = sqlx::query(
        "SELECT p.proposal_id, p.epoch, p.proposer_device_id,
                p.external_sender_index, p.reconciliation_deadline_unix,
                p.proposal_blob, p.created_at_unix, p.expires_at_unix
         FROM e2ee_proposal_deliveries d
         JOIN e2ee_proposals p ON p.proposal_id = d.proposal_id
         WHERE d.device_id = $1
           AND d.acked_at_unix IS NULL
           AND p.group_id = $2
           AND p.expires_at_unix > $3
           AND p.proposal_id > $4
         ORDER BY p.proposal_id ASC
         LIMIT $5",
    )
    .bind(device_id.to_string())
    .bind(group_id.to_string())
    .bind(now)
    .bind(after_proposal_id.map_or_else(String::new, |value| value.to_string()))
    .bind(sql_limit)
    .fetch_all(&mut *transaction)
    .await
    .map_err(|_| AuthFailure::Internal)?;

    let mut aggregate_bytes = 0_usize;
    let mut proposals = Vec::with_capacity(rows.len());
    for row in rows {
        let proposal_blob: Vec<u8> = row
            .try_get("proposal_blob")
            .map_err(|_| AuthFailure::Internal)?;
        let next_aggregate = aggregate_bytes
            .checked_add(proposal_blob.len())
            .ok_or(AuthFailure::Internal)?;
        if next_aggregate > MAX_E2EE_PROPOSAL_MAILBOX_PAGE_BLOB_BYTES {
            break;
        }
        let proposal_id: String = row
            .try_get("proposal_id")
            .map_err(|_| AuthFailure::Internal)?;
        ProposalId::try_from(proposal_id.clone()).map_err(|_| AuthFailure::Internal)?;
        let epoch: i64 = row.try_get("epoch").map_err(|_| AuthFailure::Internal)?;
        proposals.push(E2eeProposalMailboxEntry {
            proposal_id,
            epoch: u64::try_from(epoch).map_err(|_| AuthFailure::Internal)?,
            proposer_device_id: row
                .try_get("proposer_device_id")
                .map_err(|_| AuthFailure::Internal)?,
            external_sender_index: row
                .try_get::<Option<i32>, _>("external_sender_index")
                .map_err(|_| AuthFailure::Internal)?
                .map(u32::try_from)
                .transpose()
                .map_err(|_| AuthFailure::Internal)?,
            reconciliation_deadline_unix: row
                .try_get("reconciliation_deadline_unix")
                .map_err(|_| AuthFailure::Internal)?,
            proposal_blob,
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
    let next_after_proposal_id = proposals
        .last()
        .map(|proposal| proposal.proposal_id.clone());
    Ok(Json(E2eeProposalMailboxResponse {
        proposals,
        next_after_proposal_id,
    }))
}

/// Acknowledge proposals only after one active device authenticates and stores them.
pub(crate) async fn ack_group_proposals(
    State(state): State<AppState>,
    headers: HeaderMap,
    connect_info: Option<Extension<ConnectInfo<SocketAddr>>>,
    Path(group_id): Path<String>,
    Json(payload): Json<AckE2eeProposalsRequest>,
) -> Result<Json<AckE2eeProposalsResponse>, AuthFailure> {
    if payload.proposal_ids.is_empty()
        || payload.proposal_ids.len() > MAX_E2EE_PROPOSAL_ACK_BATCH_SIZE
    {
        return Err(AuthFailure::InvalidRequest);
    }
    let mut unique_ids = HashSet::with_capacity(payload.proposal_ids.len());
    let proposal_ids = payload
        .proposal_ids
        .into_iter()
        .map(|value| {
            let proposal_id =
                ProposalId::try_from(value).map_err(|_| AuthFailure::InvalidRequest)?;
            if !unique_ids.insert(proposal_id) {
                return Err(AuthFailure::InvalidRequest);
            }
            Ok(proposal_id.to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
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
        "UPDATE e2ee_proposal_deliveries d
         SET acked_at_unix = COALESCE(d.acked_at_unix, $1)
         FROM e2ee_proposals p
         WHERE p.proposal_id = d.proposal_id
           AND p.group_id = $2
           AND p.expires_at_unix > $1
           AND d.device_id = $3
           AND d.proposal_id = ANY($4::TEXT[])
         RETURNING d.proposal_id",
    )
    .bind(now)
    .bind(group_id.to_string())
    .bind(device_id.to_string())
    .bind(&proposal_ids)
    .fetch_all(&mut *transaction)
    .await
    .map_err(|_| AuthFailure::Internal)?;
    let deleted_count = if acknowledged_ids.is_empty() {
        0
    } else {
        sqlx::query(
            "DELETE FROM e2ee_proposals p
             WHERE p.group_id = $1
               AND p.proposal_id = ANY($2::TEXT[])
               AND NOT EXISTS (
                   SELECT 1 FROM e2ee_proposal_deliveries pending
                   WHERE pending.proposal_id = p.proposal_id
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
    Ok(Json(AckE2eeProposalsResponse {
        acknowledged_count: u32::try_from(acknowledged_ids.len())
            .map_err(|_| AuthFailure::Internal)?,
        deleted_count: u32::try_from(deleted_count).map_err(|_| AuthFailure::Internal)?,
    }))
}

/// Persist and fan out one member-authored opaque MLS proposal.
pub(crate) async fn post_group_proposal(
    State(state): State<AppState>,
    headers: HeaderMap,
    connect_info: Option<Extension<ConnectInfo<SocketAddr>>>,
    Path(group_id): Path<String>,
    Json(payload): Json<PostProposalRequest>,
) -> Result<Json<PostProposalResponse>, AuthFailure> {
    let client_ip = extract_client_ip(
        &state,
        &headers,
        connect_info.as_ref().map(|value| value.0 .0.ip()),
    );
    let auth = authenticate(&state, &headers).await?;
    let group_id = GroupId::try_from(group_id).map_err(|_| AuthFailure::InvalidRequest)?;
    let proposer_device_id = DeviceId::try_from(payload.proposer_device_id.clone())
        .map_err(|_| AuthFailure::InvalidRequest)?;
    enforce_e2ee_transport_rate_limit(
        &state,
        client_ip,
        auth.user_id,
        proposer_device_id,
        group_id,
        E2eeTransportRoute::Proposal,
    )
    .await?;

    let epoch = i64::try_from(payload.epoch).map_err(|_| AuthFailure::InvalidRequest)?;
    let pool = state.db_pool.as_ref().ok_or(AuthFailure::Internal)?;
    let now = now_unix();
    let ttl = i64::try_from(state.runtime.e2ee_mailbox_ttl.as_secs())
        .map_err(|_| AuthFailure::Internal)?;
    let expires_at = now.checked_add(ttl).ok_or(AuthFailure::Internal)?;
    let proposal_id = ProposalId::new();
    let mut transaction = pool.begin().await.map_err(|_| AuthFailure::Internal)?;
    require_active_owned_device(&mut transaction, auth.user_id, proposer_device_id).await?;
    let group = lock_group_for_member(&mut transaction, group_id, auth.user_id).await?;
    require_current_group_leaf(&mut transaction, group_id, auth.user_id, proposer_device_id)
        .await?;
    if group.current_epoch != payload.epoch {
        return Err(AuthFailure::EpochConflict);
    }
    sqlx::query(
        "INSERT INTO e2ee_proposals
            (proposal_id, group_id, epoch, proposer_device_id, proposal_blob,
             created_at_unix, expires_at_unix)
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(proposal_id.to_string())
    .bind(group_id.to_string())
    .bind(epoch)
    .bind(proposer_device_id.to_string())
    .bind(&payload.proposal_blob)
    .bind(now)
    .bind(expires_at)
    .execute(&mut *transaction)
    .await
    .map_err(|_| AuthFailure::Internal)?;
    snapshot_proposal_deliveries(
        &mut transaction,
        &group.conversation_id,
        group_id,
        proposal_id,
        Some(proposer_device_id),
        now,
    )
    .await?;
    let member_ids = conversation_member_ids(&mut transaction, &group.conversation_id).await?;
    transaction
        .commit()
        .await
        .map_err(|_| AuthFailure::Internal)?;

    match gateway_events::try_mls_proposal(MlsProposalEvent {
        group_id: group_id.to_string(),
        conversation_id: group.conversation_id,
        proposal_id: proposal_id.to_string(),
        epoch: payload.epoch,
        proposer_device_id: Some(proposer_device_id.to_string()),
        external_sender_index: None,
        reconciliation_deadline_unix: None,
        created_at_unix: now,
    }) {
        Ok(event) => broadcast_conversation_event(&state, &member_ids, &event).await,
        Err(error) => {
            record_gateway_event_serialize_error("user", gateway_events::MLS_PROPOSAL_EVENT);
            tracing::error!(
                event = "gateway.mls_proposal.serialize_failed",
                error = %error,
                "dropped MLS proposal notification because serialization failed"
            );
        }
    }

    Ok(Json(PostProposalResponse {
        proposal_id: proposal_id.to_string(),
        created_at_unix: now,
    }))
}

/// Atomically order and persist one opaque MLS commit.
#[allow(clippy::too_many_lines)]
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
    let welcome_device_id = match (&payload.welcome_blob, &payload.welcome_device_id) {
        (Some(_), Some(device_id)) => {
            Some(DeviceId::try_from(device_id.clone()).map_err(|_| AuthFailure::InvalidRequest)?)
        }
        (None, None) => None,
        _ => return Err(AuthFailure::InvalidRequest),
    };
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
    require_current_group_leaf(
        &mut transaction,
        group_id,
        auth.user_id,
        committer_device_id,
    )
    .await?;
    if group.current_epoch != payload.prior_epoch {
        return Err(AuthFailure::EpochConflict);
    }
    let pending_reconciliations: Vec<(i32, String)> = sqlx::query_as(
        "SELECT leaf_index, target_device_id
         FROM e2ee_membership_reconciliations
         WHERE group_id = $1 AND completed_epoch IS NULL
         ORDER BY leaf_index",
    )
    .bind(group_id.to_string())
    .fetch_all(&mut *transaction)
    .await
    .map_err(|_| AuthFailure::Internal)?;
    if !pending_reconciliations.is_empty() {
        let satisfies_policy = match &payload.membership_change {
            Some(MlsMembershipChange::Remove { leaves }) => leaves.iter().any(|leaf| {
                i32::try_from(leaf.leaf_index).is_ok_and(|leaf_index| {
                    pending_reconciliations
                        .iter()
                        .any(|(pending_index, pending_device)| {
                            *pending_index == leaf_index && *pending_device == leaf.device_id
                        })
                })
            }),
            _ => false,
        };
        if !satisfies_policy {
            return Err(AuthFailure::E2eeMembershipReconciliationPending);
        }
    }
    let member_ids_before =
        conversation_member_ids(&mut transaction, &group.conversation_id).await?;
    let welcome_user_id = if let Some(welcome_device_id) = welcome_device_id {
        if matches!(
            payload.membership_change,
            Some(MlsMembershipChange::Add { .. })
        ) {
            Some(active_device_owner(&mut transaction, welcome_device_id).await?)
        } else {
            Some(
                require_welcome_target_for_conversation(
                    &mut transaction,
                    &group.conversation_id,
                    committer_device_id,
                    welcome_device_id,
                )
                .await?,
            )
        }
    } else {
        None
    };

    sqlx::query(
        "INSERT INTO e2ee_commits
            (group_id, epoch, prior_epoch, committer_device_id,
             commit_blob, welcome_blob, welcome_device_id, membership_change_json,
             created_at_unix, expires_at_unix)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
    )
    .bind(group_id.to_string())
    .bind(epoch)
    .bind(prior_epoch)
    .bind(committer_device_id.to_string())
    .bind(&payload.commit_blob)
    .bind(&payload.welcome_blob)
    .bind(welcome_device_id.map(|device_id| device_id.to_string()))
    .bind(
        payload
            .membership_change
            .as_ref()
            .map(serde_json::to_value)
            .transpose()
            .map_err(|_| AuthFailure::InvalidRequest)?,
    )
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
    if let Some(change) = &payload.membership_change {
        apply_membership_change(
            &mut transaction,
            &group.conversation_id,
            group_id,
            payload.epoch,
            change,
            welcome_device_id,
            now,
        )
        .await?;
    }
    if let Some(welcome_device_id) = welcome_device_id {
        insert_welcome_recipients(
            &mut transaction,
            group_id,
            payload.epoch,
            &[welcome_device_id],
        )
        .await?;
    }
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
    snapshot_commit_deliveries(
        &mut transaction,
        &group.conversation_id,
        group_id,
        payload.epoch,
        committer_device_id,
        now,
    )
    .await?;
    let member_ids_after =
        conversation_member_ids(&mut transaction, &group.conversation_id).await?;
    let mut member_ids = member_ids_before;
    for member_id in member_ids_after {
        if !member_ids.contains(&member_id) {
            member_ids.push(member_id);
        }
    }
    transaction
        .commit()
        .await
        .map_err(|_| AuthFailure::Internal)?;

    emit_mls_commit_notifications(
        &state,
        &member_ids,
        MlsCommitEvent {
            group_id: group_id.to_string(),
            conversation_id: group.conversation_id.clone(),
            epoch: payload.epoch,
            prior_epoch: payload.prior_epoch,
            committer_device_id: committer_device_id.to_string(),
            created_at_unix: now,
        },
        group.suite_id,
        &welcome_user_id.into_iter().collect::<Vec<_>>(),
    )
    .await;

    if let Some(membership_change) = payload.membership_change {
        match gateway_events::try_mls_membership_change(MlsMembershipChangeEvent {
            group_id: group_id.to_string(),
            conversation_id: group.conversation_id,
            epoch: payload.epoch,
            committer_device_id: committer_device_id.to_string(),
            membership_change,
            created_at_unix: now,
        }) {
            Ok(event) => broadcast_conversation_event(&state, &member_ids, &event).await,
            Err(error) => {
                record_gateway_event_serialize_error(
                    "user",
                    gateway_events::MLS_MEMBERSHIP_CHANGE_EVENT,
                );
                tracing::error!(
                    event = "gateway.mls_membership_change.serialize_failed",
                    error = %error,
                    "dropped MLS membership notification because serialization failed"
                );
            }
        }
    }

    Ok(Json(PostCommitResponse {
        accepted: true,
        epoch: payload.epoch,
    }))
}

/// Persist one padded MLS `PrivateMessage` without parsing its interior.
#[allow(clippy::too_many_lines)]
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
    let configured_ttl = state.runtime.e2ee_mailbox_ttl.as_secs();
    let requested_ttl = payload
        .retention_secs
        .map(filament_protocol::E2eeRetentionSeconds::as_u64);
    if requested_ttl.is_some_and(|ttl| ttl > configured_ttl) {
        return Err(AuthFailure::InvalidRequest);
    }
    let ttl = i64::try_from(requested_ttl.unwrap_or(configured_ttl))
        .map_err(|_| AuthFailure::Internal)?;
    let expires_at = now.checked_add(ttl).ok_or(AuthFailure::Internal)?;
    let mut transaction = pool.begin().await.map_err(|_| AuthFailure::Internal)?;
    require_active_owned_device(&mut transaction, auth.user_id, sender_device_id).await?;
    let group = lock_group_for_member(&mut transaction, group_id, auth.user_id).await?;
    require_current_group_leaf(&mut transaction, group_id, auth.user_id, sender_device_id).await?;
    if group.current_epoch != payload.epoch {
        return Err(AuthFailure::EpochConflict);
    }
    if group.suite_id != payload.suite_id {
        return Err(AuthFailure::InvalidRequest);
    }
    let reconciliation_pending: bool = sqlx::query_scalar(
        "SELECT EXISTS (
            SELECT 1 FROM e2ee_membership_reconciliations
            WHERE group_id = $1 AND completed_epoch IS NULL
         )",
    )
    .bind(group_id.to_string())
    .fetch_one(&mut *transaction)
    .await
    .map_err(|_| AuthFailure::Internal)?;
    if reconciliation_pending {
        return Err(AuthFailure::E2eeMembershipReconciliationPending);
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
        group_id,
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
    fn attachment_padding_accepts_only_protocol_buckets() {
        for bucket in E2EE_ATTACHMENT_CIPHERTEXT_BUCKETS {
            assert!(is_valid_attachment_padding_bucket(bucket));
        }
        for invalid in [0, 65_535, 65_537, 2 * 1_024 * 1_024, 33 * 1_024 * 1_024] {
            assert!(!is_valid_attachment_padding_bucket(invalid));
        }
        assert!(fits_attachment_quota(64, 32, 96));
        assert!(!fits_attachment_quota(65, 32, 96));
        assert!(!fits_attachment_quota(u64::MAX, 1, u64::MAX));
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

    #[test]
    fn delivery_audiences_accept_bounded_group_dms() {
        assert!(validate_delivery_audience_counts(2, 2, 2).is_ok());
        assert!(validate_delivery_audience_counts(20, 20, 40).is_ok());
        assert!(validate_delivery_audience_counts(
            i64::try_from(MAX_MLS_GROUP_USERS).unwrap(),
            i64::try_from(MAX_MLS_GROUP_USERS).unwrap(),
            i64::try_from(MAX_MLS_GROUP_LEAVES).unwrap(),
        )
        .is_ok());
    }

    #[test]
    fn delivery_audiences_fail_closed_outside_group_bounds() {
        let max_members = i64::try_from(MAX_MLS_GROUP_USERS).unwrap();
        let max_devices = i64::try_from(MAX_MLS_GROUP_LEAVES).unwrap();
        for (members, capable_members, devices) in [
            (1, 1, 1),
            (3, 2, 3),
            (3, 3, 2),
            (max_members + 1, max_members + 1, max_members + 1),
            (max_members, max_members, max_devices + 1),
        ] {
            assert!(matches!(
                validate_delivery_audience_counts(members, capable_members, devices),
                Err(AuthFailure::E2eeCapabilityRequired)
            ));
        }
    }
}
