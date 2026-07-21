use std::{env, time::Duration};

use axum::{body::Body, http::Request, http::StatusCode, response::Response};
use filament_core::{ConversationId, DeviceId, GroupId, UserId};
use filament_e2ee::{
    create_pairing_transfer, create_root_identity_rotation_proof, generate_key_package_batch,
    generate_last_resort_key_package, verify_root_identity_rotation_proof, MlsConversation,
    MlsDevice, PairingReceiver, PairingTransfer, PinnedUserIdentity, RootIdentityKey,
    RootIdentityRotationProof, ScannedPairingOffer, DEFAULT_PAIRING_TTL_SECS,
};
use filament_protocol::{
    AckE2eeCommitsRequest, AckE2eeCommitsResponse, AckE2eeMessagesRequest, AckE2eeMessagesResponse,
    ClaimKeyPackageRequest, ClaimKeyPackageResponse, CreateMlsConversationRequest,
    DeviceListResponse, E2eeCommitMailboxResponse, E2eeMailboxResponse, GroupInfoResponse,
    KeyPackageEntry, MlsConversationProvisionResponse, PostCommitRequest, PostCommitResponse,
    PostMessageRequest, PostMessageResponse, PublishDeviceCertificateRequest, RemoveDeviceResponse,
    RootIdentityDirectoryResponse, RotateRootIdentityRequest, RotateRootIdentityResponse,
    UpgradeMlsConversationRequest, UploadKeyPackagesRequest, UploadKeyPackagesResponse,
    ROOT_IDENTITY_ROTATION_PROTOCOL_VERSION,
};
use filament_server::{build_router_with_db_bootstrap, AppConfig};
use futures_util::StreamExt;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::json;
use tokio::net::TcpListener;
use tokio_tungstenite::{connect_async, tungstenite::client::IntoClientRequest};
use tower::ServiceExt;
use ulid::Ulid;

#[derive(Debug, Deserialize)]
struct AuthResponse {
    access_token: String,
}

#[tokio::test]
async fn postgres_e2ee_delivery_orders_commits_and_stores_only_padded_opaque_messages() {
    let Some(database_url) = postgres_url() else {
        eprintln!("skipping postgres-backed E2EE test: FILAMENT_TEST_DATABASE_URL is unset");
        return;
    };
    let audit_pool = sqlx::PgPool::connect(&database_url)
        .await
        .expect("delivery audit pool should connect");
    let app = test_app(database_url).await;
    let (alice_auth, alice_user_id) = register_and_login(&app, "203.0.113.181").await;
    let (bob_auth, bob_user_id) = register_and_login(&app, "203.0.113.182").await;
    let alice_root = RootIdentityKey::generate();
    let alice_device_id = DeviceId::new();
    let alice_device = MlsDevice::generate(alice_user_id, alice_device_id, &alice_root)
        .expect("Alice device should generate");
    assert_eq!(
        publish_device(
            &app,
            &alice_auth,
            "203.0.113.181",
            alice_device_id,
            &publish_payload(&alice_device),
        )
        .await
        .status(),
        StatusCode::OK
    );
    let bob_root = RootIdentityKey::generate();
    let bob_device_id = DeviceId::new();
    let bob_device = MlsDevice::generate(bob_user_id, bob_device_id, &bob_root)
        .expect("Bob device should generate");
    assert_eq!(
        publish_device(
            &app,
            &bob_auth,
            "203.0.113.182",
            bob_device_id,
            &publish_payload(&bob_device),
        )
        .await
        .status(),
        StatusCode::OK
    );
    let bob_second_device_id = DeviceId::new();
    let bob_second_device = MlsDevice::generate(bob_user_id, bob_second_device_id, &bob_root)
        .expect("Bob's second device should generate");
    assert_eq!(
        publish_device(
            &app,
            &bob_auth,
            "203.0.113.182",
            bob_second_device_id,
            &publish_payload(&bob_second_device),
        )
        .await
        .status(),
        StatusCode::OK
    );

    let bob_packages = generate_key_package_batch(&bob_device, 1)
        .expect("Bob's initial KeyPackage should generate");
    let upload = UploadKeyPackagesRequest {
        device_id: bob_device_id.to_string(),
        key_packages: bob_packages
            .into_iter()
            .map(|package| KeyPackageEntry {
                key_package_blob: package.blob,
                is_last_resort: false,
            })
            .collect(),
    };
    let upload = send_json(
        &app,
        "POST",
        "/e2ee/keypackages",
        Some(&bob_auth.access_token),
        "203.0.113.182",
        &upload,
    )
    .await;
    assert_eq!(upload.status(), StatusCode::OK);
    let claim = send_json(
        &app,
        "POST",
        "/e2ee/keypackages/claim",
        Some(&alice_auth.access_token),
        "203.0.113.181",
        &ClaimKeyPackageRequest {
            target_user_id: bob_user_id.to_string(),
            target_device_id: Some(bob_device_id.to_string()),
        },
    )
    .await;
    assert_eq!(claim.status(), StatusCode::OK);
    let claim: ClaimKeyPackageResponse = parse_json(claim).await;

    let conversation_id = ConversationId::new();
    let group_id = GroupId::new();
    let (mut alice_conversation, pending) = MlsConversation::create_two_member(
        group_id,
        &alice_device,
        PinnedUserIdentity::new(bob_user_id, bob_root.public_key_bytes()),
        &claim.key_package_blob,
    )
    .expect("Alice should stage the initial Add commit");
    let create_request = CreateMlsConversationRequest {
        conversation_id: conversation_id.to_string(),
        peer_user_id: bob_user_id.to_string(),
        group_id: group_id.to_string(),
        suite_id: pending.suite.as_u16(),
        committer_device_id: pending.committer_device_id.to_string(),
        welcome_device_id: bob_device_id.to_string(),
        commit_blob: pending.commit_blob.clone(),
        welcome_blob: pending
            .welcome_blob
            .clone()
            .expect("initial Add commit must include a Welcome"),
        group_info_blob: pending
            .group_info_blob
            .clone()
            .expect("initial GroupInfo should be present"),
    };
    let create = send_json(
        &app,
        "POST",
        "/e2ee/conversations",
        Some(&alice_auth.access_token),
        "203.0.113.181",
        &create_request,
    )
    .await;
    assert_eq!(create.status(), StatusCode::OK);
    let created: MlsConversationProvisionResponse = parse_json(create).await;
    assert_eq!(created.conversation_id, conversation_id.to_string());
    assert_eq!(created.group_id, group_id.to_string());
    assert_eq!(created.crypto, "mls_v1");
    assert_eq!(created.epoch, 1);

    let retry = send_json(
        &app,
        "POST",
        "/e2ee/conversations",
        Some(&alice_auth.access_token),
        "203.0.113.181",
        &create_request,
    )
    .await;
    assert_eq!(retry.status(), StatusCode::OK);
    let retry: MlsConversationProvisionResponse = parse_json(retry).await;
    assert_eq!(retry, created);

    let bob_initial_commits = Request::builder()
        .method("GET")
        .uri(format!(
            "/e2ee/groups/{group_id}/commits?device_id={bob_device_id}"
        ))
        .header("authorization", format!("Bearer {}", bob_auth.access_token))
        .header("x-forwarded-for", "203.0.113.182")
        .body(Body::empty())
        .expect("commit mailbox request should build");
    let bob_initial_commits = app
        .clone()
        .oneshot(bob_initial_commits)
        .await
        .expect("commit mailbox request should execute");
    assert_eq!(bob_initial_commits.status(), StatusCode::OK);
    let bob_initial_commits: E2eeCommitMailboxResponse = parse_json(bob_initial_commits).await;
    assert_eq!(bob_initial_commits.commits.len(), 1);
    assert_eq!(bob_initial_commits.commits[0].epoch, 1);
    assert_eq!(
        bob_initial_commits.commits[0].welcome_blob.as_deref(),
        Some(create_request.welcome_blob.as_slice())
    );

    let bob_second_initial_commits = Request::builder()
        .method("GET")
        .uri(format!(
            "/e2ee/groups/{group_id}/commits?device_id={bob_second_device_id}"
        ))
        .header("authorization", format!("Bearer {}", bob_auth.access_token))
        .header("x-forwarded-for", "203.0.113.182")
        .body(Body::empty())
        .expect("secondary commit mailbox request should build");
    let bob_second_initial_commits = app
        .clone()
        .oneshot(bob_second_initial_commits)
        .await
        .expect("secondary commit mailbox request should execute");
    assert_eq!(bob_second_initial_commits.status(), StatusCode::OK);
    let bob_second_initial_commits: E2eeCommitMailboxResponse =
        parse_json(bob_second_initial_commits).await;
    assert_eq!(bob_second_initial_commits.commits.len(), 1);
    assert!(bob_second_initial_commits.commits[0].welcome_blob.is_none());
    let conflicting_create = send_json(
        &app,
        "POST",
        "/e2ee/conversations",
        Some(&alice_auth.access_token),
        "203.0.113.181",
        &CreateMlsConversationRequest {
            conversation_id: ConversationId::new().to_string(),
            group_id: GroupId::new().to_string(),
            ..create_request.clone()
        },
    )
    .await;
    assert_eq!(conflicting_create.status(), StatusCode::CONFLICT);
    let conflicting_create: serde_json::Value = parse_json(conflicting_create).await;
    assert_eq!(conflicting_create["error"], "e2ee_conversation_conflict");
    alice_conversation
        .accept_pending_commit(&alice_device)
        .expect("Alice should merge the accepted commit");
    let _bob_conversation = MlsConversation::join_from_welcome(
        group_id,
        &bob_device,
        PinnedUserIdentity::new(alice_user_id, alice_root.public_key_bytes()),
        &create_request.welcome_blob,
    )
    .expect("Bob should join the provisioned group from its Welcome");

    let (charlie_auth, charlie_user_id) = register_and_login(&app, "203.0.113.183").await;
    let capability_request = CreateMlsConversationRequest {
        conversation_id: ConversationId::new().to_string(),
        peer_user_id: charlie_user_id.to_string(),
        group_id: GroupId::new().to_string(),
        ..create_request.clone()
    };
    let capability_failure = send_json(
        &app,
        "POST",
        "/e2ee/conversations",
        Some(&alice_auth.access_token),
        "203.0.113.181",
        &capability_request,
    )
    .await;
    assert_eq!(capability_failure.status(), StatusCode::CONFLICT);
    let capability_failure: serde_json::Value = parse_json(capability_failure).await;
    assert_eq!(capability_failure["error"], "e2ee_capability_required");
    let failed_conversation_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM e2ee_conversations WHERE conversation_id = $1")
            .bind(&capability_request.conversation_id)
            .fetch_one(&audit_pool)
            .await
            .expect("failed capability gate should be auditable");
    assert_eq!(failed_conversation_count, 0);

    let charlie_root = RootIdentityKey::generate();
    let charlie_device_id = DeviceId::new();
    let charlie_device = MlsDevice::generate(charlie_user_id, charlie_device_id, &charlie_root)
        .expect("Charlie's device should generate");
    assert_eq!(
        publish_device(
            &app,
            &charlie_auth,
            "203.0.113.183",
            charlie_device_id,
            &publish_payload(&charlie_device),
        )
        .await
        .status(),
        StatusCode::OK
    );
    let plaintext_conversation_id = ConversationId::new();
    let seeded_at = 1_750_000_000_i64;
    let mut seed = audit_pool
        .begin()
        .await
        .expect("plaintext seed transaction should begin");
    sqlx::query(
        "INSERT INTO e2ee_conversations
            (conversation_id, conversation_crypto, created_by, created_at_unix)
         VALUES ($1, 'plaintext', $2, $3)",
    )
    .bind(plaintext_conversation_id.to_string())
    .bind(alice_user_id.to_string())
    .bind(seeded_at)
    .execute(&mut *seed)
    .await
    .expect("plaintext conversation should seed");
    for user_id in [alice_user_id, charlie_user_id] {
        sqlx::query(
            "INSERT INTO e2ee_conversation_members
                (conversation_id, user_id, joined_at_unix) VALUES ($1, $2, $3)",
        )
        .bind(plaintext_conversation_id.to_string())
        .bind(user_id.to_string())
        .bind(seeded_at)
        .execute(&mut *seed)
        .await
        .expect("plaintext member should seed");
    }
    seed.commit()
        .await
        .expect("plaintext seed transaction should commit");
    let upgrade_request = UpgradeMlsConversationRequest {
        group_id: GroupId::new().to_string(),
        suite_id: 3,
        committer_device_id: alice_device_id.to_string(),
        welcome_device_id: charlie_device_id.to_string(),
        commit_blob: vec![0x31; 128],
        welcome_blob: vec![0x32; 128],
        group_info_blob: vec![0x33; 128],
    };
    let upgrade_uri = format!("/e2ee/conversations/{plaintext_conversation_id}/upgrade");
    let unauthorized_upgrade = send_json(
        &app,
        "POST",
        &upgrade_uri,
        Some(&bob_auth.access_token),
        "203.0.113.182",
        &UpgradeMlsConversationRequest {
            committer_device_id: bob_device_id.to_string(),
            ..upgrade_request.clone()
        },
    )
    .await;
    assert_eq!(unauthorized_upgrade.status(), StatusCode::NOT_FOUND);
    let upgrade = send_json(
        &app,
        "POST",
        &upgrade_uri,
        Some(&alice_auth.access_token),
        "203.0.113.181",
        &upgrade_request,
    )
    .await;
    assert_eq!(upgrade.status(), StatusCode::OK);
    let upgraded: MlsConversationProvisionResponse = parse_json(upgrade).await;
    assert_eq!(
        upgraded.conversation_id,
        plaintext_conversation_id.to_string()
    );
    assert_eq!(upgraded.crypto, "mls_v1");
    let upgrade_retry = send_json(
        &app,
        "POST",
        &upgrade_uri,
        Some(&alice_auth.access_token),
        "203.0.113.181",
        &upgrade_request,
    )
    .await;
    assert_eq!(upgrade_retry.status(), StatusCode::OK);
    assert_eq!(
        parse_json::<MlsConversationProvisionResponse>(upgrade_retry).await,
        upgraded
    );
    let downgrade = sqlx::query(
        "UPDATE e2ee_conversations SET conversation_crypto = 'plaintext'
         WHERE conversation_id = $1",
    )
    .bind(plaintext_conversation_id.to_string())
    .execute(&audit_pool)
    .await;
    assert!(downgrade.is_err(), "database must reject MLS downgrades");

    let first_commit = PostCommitRequest {
        epoch: 2,
        prior_epoch: 1,
        committer_device_id: alice_device_id.to_string(),
        commit_blob: vec![0xA1; 256],
        welcome_blob: Some(vec![0xA2; 192]),
        welcome_device_id: Some(bob_device_id.to_string()),
        group_info_blob: Some(vec![0xA3; 128]),
    };
    let competing_commit = PostCommitRequest {
        commit_blob: vec![0xB1; 256],
        welcome_blob: Some(vec![0xB2; 192]),
        group_info_blob: Some(vec![0xB3; 128]),
        ..first_commit.clone()
    };
    let commit_uri = format!("/e2ee/groups/{group_id}/commits");
    let (first_response, competing_response) = tokio::join!(
        send_json(
            &app,
            "POST",
            &commit_uri,
            Some(&alice_auth.access_token),
            "203.0.113.181",
            &first_commit,
        ),
        send_json(
            &app,
            "POST",
            &commit_uri,
            Some(&alice_auth.access_token),
            "203.0.113.181",
            &competing_commit,
        )
    );
    let statuses = [first_response.status(), competing_response.status()];
    assert_eq!(
        statuses
            .iter()
            .filter(|status| **status == StatusCode::OK)
            .count(),
        1
    );
    assert_eq!(
        statuses
            .iter()
            .filter(|status| **status == StatusCode::CONFLICT)
            .count(),
        1
    );
    let (accepted_response, conflict_response) = if first_response.status() == StatusCode::OK {
        (first_response, competing_response)
    } else {
        (competing_response, first_response)
    };
    let accepted: PostCommitResponse = parse_json(accepted_response).await;
    assert!(accepted.accepted);
    assert_eq!(accepted.epoch, 2);
    let conflict: serde_json::Value = parse_json(conflict_response).await;
    assert_eq!(conflict["error"], "epoch_conflict");
    let stored_commit_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM e2ee_commits WHERE group_id = $1 AND epoch = 2")
            .bind(group_id.to_string())
            .fetch_one(&audit_pool)
            .await
            .expect("accepted commit should be stored once");
    assert_eq!(stored_commit_count, 1);
    let stored_group_info: Vec<u8> =
        sqlx::query_scalar("SELECT group_info_blob FROM e2ee_groups WHERE group_id = $1")
            .bind(group_id.to_string())
            .fetch_one(&audit_pool)
            .await
            .expect("accepted GroupInfo should be stored");

    let info = Request::builder()
        .method("GET")
        .uri(format!("/e2ee/groups/{group_id}/info"))
        .header("authorization", format!("Bearer {}", bob_auth.access_token))
        .header("x-forwarded-for", "203.0.113.182")
        .body(Body::empty())
        .expect("GroupInfo request should build");
    let info = app
        .clone()
        .oneshot(info)
        .await
        .expect("GroupInfo request should execute");
    assert_eq!(info.status(), StatusCode::OK);
    let info: GroupInfoResponse = parse_json(info).await;
    assert_eq!(info.epoch, 2);
    assert_eq!(info.suite_id, 3);
    assert_eq!(info.group_info_blob, stored_group_info);

    let bob_commit_page = Request::builder()
        .method("GET")
        .uri(format!(
            "/e2ee/groups/{group_id}/commits?device_id={bob_device_id}&after_epoch=1&limit=1"
        ))
        .header("authorization", format!("Bearer {}", bob_auth.access_token))
        .header("x-forwarded-for", "203.0.113.182")
        .body(Body::empty())
        .expect("commit cursor request should build");
    let bob_commit_page = app
        .clone()
        .oneshot(bob_commit_page)
        .await
        .expect("commit cursor request should execute");
    assert_eq!(bob_commit_page.status(), StatusCode::OK);
    let bob_commit_page: E2eeCommitMailboxResponse = parse_json(bob_commit_page).await;
    assert_eq!(bob_commit_page.commits.len(), 1);
    assert_eq!(bob_commit_page.commits[0].epoch, 2);
    assert!(bob_commit_page.commits[0].welcome_blob.is_some());
    assert_eq!(bob_commit_page.next_after_epoch, Some(2));

    let first_commit_ack = send_json(
        &app,
        "POST",
        &format!("/e2ee/groups/{group_id}/commits/ack"),
        Some(&bob_auth.access_token),
        "203.0.113.182",
        &AckE2eeCommitsRequest {
            device_id: bob_device_id.to_string(),
            epochs: vec![1, 2],
        },
    )
    .await;
    assert_eq!(first_commit_ack.status(), StatusCode::OK);
    let first_commit_ack: AckE2eeCommitsResponse = parse_json(first_commit_ack).await;
    assert_eq!(first_commit_ack.acknowledged_count, 2);
    assert_eq!(first_commit_ack.deleted_count, 0);

    let final_commit_ack = send_json(
        &app,
        "POST",
        &format!("/e2ee/groups/{group_id}/commits/ack"),
        Some(&bob_auth.access_token),
        "203.0.113.182",
        &AckE2eeCommitsRequest {
            device_id: bob_second_device_id.to_string(),
            epochs: vec![1, 2],
        },
    )
    .await;
    assert_eq!(final_commit_ack.status(), StatusCode::OK);
    let final_commit_ack: AckE2eeCommitsResponse = parse_json(final_commit_ack).await;
    assert_eq!(final_commit_ack.acknowledged_count, 2);
    assert_eq!(final_commit_ack.deleted_count, 2);
    let remaining_commits: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM e2ee_commits WHERE group_id = $1")
            .bind(group_id.to_string())
            .fetch_one(&audit_pool)
            .await
            .expect("commit deletion should be observable");
    assert_eq!(remaining_commits, 0);

    let message_blob = vec![0xCC; 512];
    let message_request = PostMessageRequest {
        epoch: 2,
        suite_id: 3,
        sender_device_id: alice_device_id.to_string(),
        message_blob: message_blob.clone(),
    };
    let message = send_json(
        &app,
        "POST",
        &format!("/e2ee/groups/{group_id}/messages"),
        Some(&alice_auth.access_token),
        "203.0.113.181",
        &message_request,
    )
    .await;
    assert_eq!(message.status(), StatusCode::OK);
    let message: PostMessageResponse = parse_json(message).await;
    let stored_message: (Vec<u8>, String, i64, i64) = sqlx::query_as(
        "SELECT ciphertext_blob, crypto_mode, created_at_unix, expires_at_unix
         FROM e2ee_messages WHERE message_id = $1",
    )
    .bind(&message.message_id)
    .fetch_one(&audit_pool)
    .await
    .expect("opaque message should be stored");
    assert_eq!(stored_message.0, message_blob);
    assert_eq!(stored_message.1, "mls_v1");
    assert!(stored_message.3 > stored_message.2);

    for device_id in [bob_device_id, bob_second_device_id] {
        let mailbox = Request::builder()
            .method("GET")
            .uri(format!(
                "/e2ee/groups/{group_id}/mailbox?device_id={device_id}&limit=20"
            ))
            .header("authorization", format!("Bearer {}", bob_auth.access_token))
            .header("x-forwarded-for", "203.0.113.182")
            .body(Body::empty())
            .expect("mailbox request should build");
        let mailbox = app
            .clone()
            .oneshot(mailbox)
            .await
            .expect("mailbox request should execute");
        assert_eq!(mailbox.status(), StatusCode::OK);
        let mailbox: E2eeMailboxResponse = parse_json(mailbox).await;
        assert_eq!(mailbox.messages.len(), 1);
        assert_eq!(mailbox.messages[0].message_id, message.message_id);
        assert_eq!(mailbox.messages[0].crypto, "mls_v1");
        assert_eq!(mailbox.messages[0].message_blob, message_blob);
    }

    let sender_mailbox = Request::builder()
        .method("GET")
        .uri(format!(
            "/e2ee/groups/{group_id}/mailbox?device_id={alice_device_id}"
        ))
        .header(
            "authorization",
            format!("Bearer {}", alice_auth.access_token),
        )
        .header("x-forwarded-for", "203.0.113.181")
        .body(Body::empty())
        .expect("sender mailbox request should build");
    let sender_mailbox = app
        .clone()
        .oneshot(sender_mailbox)
        .await
        .expect("sender mailbox request should execute");
    assert_eq!(sender_mailbox.status(), StatusCode::OK);
    let sender_mailbox: E2eeMailboxResponse = parse_json(sender_mailbox).await;
    assert!(sender_mailbox.messages.is_empty());

    let first_ack = send_json(
        &app,
        "POST",
        &format!("/e2ee/groups/{group_id}/messages/ack"),
        Some(&bob_auth.access_token),
        "203.0.113.182",
        &AckE2eeMessagesRequest {
            device_id: bob_device_id.to_string(),
            message_ids: vec![message.message_id.clone()],
        },
    )
    .await;
    assert_eq!(first_ack.status(), StatusCode::OK);
    let first_ack: AckE2eeMessagesResponse = parse_json(first_ack).await;
    assert_eq!(first_ack.acknowledged_count, 1);
    assert_eq!(first_ack.deleted_count, 0);
    let retained_after_first_ack: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM e2ee_messages WHERE message_id = $1")
            .bind(&message.message_id)
            .fetch_one(&audit_pool)
            .await
            .expect("message should remain for the second device");
    assert_eq!(retained_after_first_ack, 1);

    let final_ack = send_json(
        &app,
        "POST",
        &format!("/e2ee/groups/{group_id}/messages/ack"),
        Some(&bob_auth.access_token),
        "203.0.113.182",
        &AckE2eeMessagesRequest {
            device_id: bob_second_device_id.to_string(),
            message_ids: vec![message.message_id.clone()],
        },
    )
    .await;
    assert_eq!(final_ack.status(), StatusCode::OK);
    let final_ack: AckE2eeMessagesResponse = parse_json(final_ack).await;
    assert_eq!(final_ack.acknowledged_count, 1);
    assert_eq!(final_ack.deleted_count, 1);
    let deleted_after_all_acks: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM e2ee_messages WHERE message_id = $1")
            .bind(&message.message_id)
            .fetch_one(&audit_pool)
            .await
            .expect("message deletion should be observable");
    assert_eq!(deleted_after_all_acks, 0);

    let expiring = send_json(
        &app,
        "POST",
        &format!("/e2ee/groups/{group_id}/messages"),
        Some(&alice_auth.access_token),
        "203.0.113.181",
        &message_request,
    )
    .await;
    assert_eq!(expiring.status(), StatusCode::OK);
    let expiring: PostMessageResponse = parse_json(expiring).await;
    sqlx::query(
        "UPDATE e2ee_messages SET created_at_unix = 1, expires_at_unix = 2
         WHERE message_id = $1",
    )
    .bind(&expiring.message_id)
    .execute(&audit_pool)
    .await
    .expect("expiry fixture should update");
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let remaining: i64 =
                sqlx::query_scalar("SELECT COUNT(*) FROM e2ee_messages WHERE message_id = $1")
                    .bind(&expiring.message_id)
                    .fetch_one(&audit_pool)
                    .await
                    .expect("expiry state should be queryable");
            if remaining == 0 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("background mailbox GC should delete expired ciphertext");

    let column_names: Vec<String> = sqlx::query_scalar(
        "SELECT column_name FROM information_schema.columns
         WHERE table_schema = current_schema() AND table_name = 'e2ee_messages'",
    )
    .fetch_all(&audit_pool)
    .await
    .expect("message schema should be inspectable");
    assert!(!column_names.iter().any(|column| {
        matches!(
            column.as_str(),
            "content" | "plaintext" | "key" | "key_material"
        )
    }));

    let unpadded = PostMessageRequest {
        message_blob: vec![0xDD; 513],
        ..message_request.clone()
    };
    assert_eq!(
        send_json(
            &app,
            "POST",
            &format!("/e2ee/groups/{group_id}/messages"),
            Some(&alice_auth.access_token),
            "203.0.113.181",
            &unpadded,
        )
        .await
        .status(),
        StatusCode::BAD_REQUEST
    );
    let spoofed_sender = send_json(
        &app,
        "POST",
        &format!("/e2ee/groups/{group_id}/messages"),
        Some(&bob_auth.access_token),
        "203.0.113.182",
        &message_request,
    )
    .await;
    assert_eq!(spoofed_sender.status(), StatusCode::NOT_FOUND);
}

fn postgres_url() -> Option<String> {
    env::var("FILAMENT_TEST_DATABASE_URL").ok()
}

async fn test_app(database_url: String) -> axum::Router {
    test_app_with_e2ee_limits(database_url, 200, 200).await
}

async fn test_app_with_e2ee_limits(
    database_url: String,
    device_publish_per_minute: u32,
    keypackage_claim_per_minute: u32,
) -> axum::Router {
    build_router_with_db_bootstrap(&AppConfig {
        max_body_bytes: 512 * 1024,
        request_timeout: Duration::from_secs(5),
        rate_limit_requests_per_minute: 500,
        auth_route_requests_per_minute: 200,
        e2ee_device_publish_per_minute: device_publish_per_minute,
        e2ee_keypackage_claim_per_minute: keypackage_claim_per_minute,
        e2ee_mailbox_gc_interval: Duration::from_secs(1),
        database_url: Some(database_url),
        ..AppConfig::default()
    })
    .await
    .expect("router should build")
}

async fn parse_json<T: DeserializeOwned>(response: Response) -> T {
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("response body should be readable");
    serde_json::from_slice(&body).expect("response body should be valid JSON")
}

async fn next_gateway_event(
    socket: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
    event_type: &str,
) -> serde_json::Value {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let message = socket
                .next()
                .await
                .expect("gateway should remain connected")
                .expect("gateway event should decode");
            if message.is_ping() || message.is_pong() {
                continue;
            }
            let value: serde_json::Value = serde_json::from_str(
                message
                    .to_text()
                    .expect("gateway event should be a text frame"),
            )
            .expect("gateway event should be valid JSON");
            if value["t"] == event_type {
                return value;
            }
        }
    })
    .await
    .unwrap_or_else(|_| panic!("timed out waiting for gateway event {event_type}"))
}

async fn send_json<T: serde::Serialize>(
    app: &axum::Router,
    method: &str,
    uri: &str,
    token: Option<&str>,
    ip: &str,
    payload: &T,
) -> Response {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .header("x-forwarded-for", ip);
    if let Some(token) = token {
        builder = builder.header("authorization", format!("Bearer {token}"));
    }
    let request = builder
        .body(Body::from(
            serde_json::to_vec(payload).expect("payload should serialize"),
        ))
        .expect("request should build");
    app.clone()
        .oneshot(request)
        .await
        .expect("request should execute")
}

async fn register_and_login(app: &axum::Router, ip: &str) -> (AuthResponse, UserId) {
    let suffix = Ulid::new().to_string().to_lowercase();
    let username = format!("e2ee_{}", &suffix[..20]);
    let password = "e2ee-integration-password";
    let register = send_json(
        app,
        "POST",
        "/auth/register",
        None,
        ip,
        &json!({"username": username, "password": password}),
    )
    .await;
    assert_eq!(register.status(), StatusCode::OK);

    let login = send_json(
        app,
        "POST",
        "/auth/login",
        None,
        ip,
        &json!({"username": username, "password": password}),
    )
    .await;
    assert_eq!(login.status(), StatusCode::OK);
    let auth: AuthResponse = parse_json(login).await;

    let me = Request::builder()
        .method("GET")
        .uri("/auth/me")
        .header("authorization", format!("Bearer {}", auth.access_token))
        .header("x-forwarded-for", ip)
        .body(Body::empty())
        .expect("request should build");
    let me = app
        .clone()
        .oneshot(me)
        .await
        .expect("request should execute");
    assert_eq!(me.status(), StatusCode::OK);
    let body: serde_json::Value = parse_json(me).await;
    let user_id = UserId::try_from(
        body["user_id"]
            .as_str()
            .expect("me response should include user_id")
            .to_owned(),
    )
    .expect("user_id should be valid");
    (auth, user_id)
}

fn publish_payload(device: &MlsDevice) -> PublishDeviceCertificateRequest {
    PublishDeviceCertificateRequest {
        device_signature_pubkey: device.certificate().device_signature_pubkey.clone(),
        root_key_signature: device.certificate().root_key_signature.clone(),
        root_key_pub: device.root_key_public().to_vec(),
    }
}

async fn publish_device(
    app: &axum::Router,
    auth: &AuthResponse,
    ip: &str,
    device_id: DeviceId,
    payload: &PublishDeviceCertificateRequest,
) -> Response {
    send_json(
        app,
        "PUT",
        &format!("/e2ee/devices/{device_id}"),
        Some(&auth.access_token),
        ip,
        payload,
    )
    .await
}

fn claim_request(
    token: &str,
    ip: &str,
    target_user_id: UserId,
    target_device_id: DeviceId,
) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri("/e2ee/keypackages/claim")
        .header("authorization", format!("Bearer {token}"))
        .header("content-type", "application/json")
        .header("x-forwarded-for", ip)
        .body(Body::from(
            serde_json::to_vec(&ClaimKeyPackageRequest {
                target_user_id: target_user_id.to_string(),
                target_device_id: Some(target_device_id.to_string()),
            })
            .expect("claim payload should serialize"),
        ))
        .expect("claim request should build")
}

#[tokio::test]
async fn postgres_e2ee_directory_verifies_identity_and_atomically_claims_once() {
    let Some(database_url) = postgres_url() else {
        eprintln!("skipping postgres-backed E2EE test: FILAMENT_TEST_DATABASE_URL is unset");
        return;
    };
    let audit_pool = sqlx::PgPool::connect(&database_url)
        .await
        .expect("audit verification pool should connect");
    let app = test_app(database_url).await;
    let ip = "203.0.113.91";
    let (auth, user_id) = register_and_login(&app, ip).await;
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("gateway listener should bind");
    let gateway_address = listener
        .local_addr()
        .expect("gateway listener address should be available");
    let gateway_app = app.clone();
    let gateway_server = tokio::spawn(async move {
        axum::serve(listener, gateway_app)
            .await
            .expect("gateway server should run");
    });
    let gateway_url = format!(
        "ws://{gateway_address}/gateway/ws?access_token={}",
        auth.access_token
    );
    let mut gateway_request = gateway_url
        .into_client_request()
        .expect("gateway request should build");
    gateway_request.headers_mut().insert(
        "x-forwarded-for",
        ip.parse()
            .expect("fixture IP should parse as a header value"),
    );
    let (mut gateway, _) = connect_async(gateway_request)
        .await
        .expect("gateway should connect");
    let ready = next_gateway_event(&mut gateway, "ready").await;
    assert_eq!(ready["d"]["user_id"], user_id.to_string());

    let root = RootIdentityKey::generate();
    let device_id = DeviceId::new();
    let device = MlsDevice::generate(user_id, device_id, &root).expect("device should generate");

    let published = publish_device(&app, &auth, ip, device_id, &publish_payload(&device)).await;
    assert_eq!(published.status(), StatusCode::OK);
    let device_added = next_gateway_event(&mut gateway, "device_list_update").await;
    assert_eq!(device_added["d"]["user_id"], user_id.to_string());
    assert_eq!(device_added["d"]["device_count"], 1);

    let paired_device_id = DeviceId::new();
    let pairing_receiver = PairingReceiver::begin(
        user_id,
        paired_device_id,
        1_750_000_000,
        DEFAULT_PAIRING_TTL_SECS,
    )
    .expect("pairing receiver should generate");
    let qr_payload = pairing_receiver
        .qr_payload()
        .expect("QR payload should encode");
    let scanned_offer = ScannedPairingOffer::from_qr_payload(&qr_payload, 1_750_000_000)
        .expect("QR payload should scan");
    let transfer = create_pairing_transfer(&device, &root, &scanned_offer, 1_750_000_000)
        .expect("existing device should authorize pairing");
    let transfer = PairingTransfer::from_payload(
        &transfer
            .to_payload()
            .expect("pairing transfer should encode"),
    )
    .expect("pairing transfer should decode");
    let paired_root = pairing_receiver
        .complete(&transfer, 1_750_000_000)
        .expect("new device should restore root identity");
    let paired_device = MlsDevice::generate(user_id, paired_device_id, paired_root.root_identity())
        .expect("paired device should generate");
    let paired_published = publish_device(
        &app,
        &auth,
        ip,
        paired_device_id,
        &publish_payload(&paired_device),
    )
    .await;
    assert_eq!(paired_published.status(), StatusCode::OK);
    let paired_device_added = next_gateway_event(&mut gateway, "device_list_update").await;
    assert_eq!(paired_device_added["d"]["device_count"], 2);

    let list = Request::builder()
        .method("GET")
        .uri(format!("/e2ee/users/{user_id}/devices"))
        .header("authorization", format!("Bearer {}", auth.access_token))
        .header("x-forwarded-for", ip)
        .body(Body::empty())
        .expect("request should build");
    let list = app
        .clone()
        .oneshot(list)
        .await
        .expect("request should execute");
    assert_eq!(list.status(), StatusCode::OK);
    let listed: DeviceListResponse = parse_json(list).await;
    assert_eq!(listed.user_id, user_id.to_string());
    assert_eq!(listed.devices.len(), 2);
    let first_device = listed
        .devices
        .iter()
        .find(|listed_device| listed_device.device_id == device_id.to_string())
        .expect("first device should be listed");
    assert_eq!(first_device.root_key_pub, root.public_key_bytes());
    let paired_device = listed
        .devices
        .iter()
        .find(|listed_device| listed_device.device_id == paired_device_id.to_string())
        .expect("paired device should be listed");
    assert_eq!(paired_device.root_key_pub, root.public_key_bytes());

    let mut generated = generate_key_package_batch(&device, 2).expect("packages should generate");
    generated
        .push(generate_last_resort_key_package(&device).expect("fallback package should generate"));
    let expected_blobs: Vec<Vec<u8>> = generated
        .iter()
        .map(|package| package.blob.clone())
        .collect();
    let upload = UploadKeyPackagesRequest {
        device_id: device_id.to_string(),
        key_packages: generated
            .into_iter()
            .map(|package| KeyPackageEntry {
                key_package_blob: package.blob,
                is_last_resort: package.is_last_resort,
            })
            .collect(),
    };
    let uploaded = send_json(
        &app,
        "POST",
        "/e2ee/keypackages",
        Some(&auth.access_token),
        ip,
        &upload,
    )
    .await;
    assert_eq!(uploaded.status(), StatusCode::OK);
    let uploaded: UploadKeyPackagesResponse = parse_json(uploaded).await;
    assert_eq!(uploaded.stored_count, 3);

    let first_app = app.clone();
    let second_app = app.clone();
    let first_request = claim_request(&auth.access_token, ip, user_id, device_id);
    let second_request = claim_request(&auth.access_token, ip, user_id, device_id);
    let (first, second) = tokio::join!(
        first_app.oneshot(first_request),
        second_app.oneshot(second_request)
    );
    let first = first.expect("first claim should execute");
    let second = second.expect("second claim should execute");
    assert_eq!(first.status(), StatusCode::OK);
    assert_eq!(second.status(), StatusCode::OK);
    let first: ClaimKeyPackageResponse = parse_json(first).await;
    let second: ClaimKeyPackageResponse = parse_json(second).await;
    assert_ne!(first.key_package_blob, second.key_package_blob);
    assert!(!first.is_last_resort);
    assert!(!second.is_last_resort);
    assert!(expected_blobs.contains(&first.key_package_blob));
    assert!(expected_blobs.contains(&second.key_package_blob));
    let low_pool = next_gateway_event(&mut gateway, "keypackage_low").await;
    assert_eq!(low_pool["d"]["device_id"], device_id.to_string());
    assert!(low_pool["d"]["remaining_count"].as_u64().is_some());
    assert_eq!(low_pool["d"]["water_mark"], 10);

    let fallback = app
        .clone()
        .oneshot(claim_request(&auth.access_token, ip, user_id, device_id))
        .await
        .expect("fallback claim should execute");
    assert_eq!(fallback.status(), StatusCode::OK);
    let fallback: ClaimKeyPackageResponse = parse_json(fallback).await;
    assert!(fallback.is_last_resort);

    let exhausted = app
        .clone()
        .oneshot(claim_request(&auth.access_token, ip, user_id, device_id))
        .await
        .expect("exhausted claim should execute");
    assert_eq!(exhausted.status(), StatusCode::NOT_FOUND);

    let wrong_target = app
        .clone()
        .oneshot(claim_request(
            &auth.access_token,
            ip,
            UserId::new(),
            device_id,
        ))
        .await
        .expect("mismatched claim should execute");
    assert_eq!(wrong_target.status(), StatusCode::NOT_FOUND);

    let forged_device_id = DeviceId::new();
    let forged_device =
        MlsDevice::generate(user_id, forged_device_id, &root).expect("device should generate");
    let mut forged_payload = publish_payload(&forged_device);
    forged_payload.root_key_signature[0] ^= 1;
    let forged = publish_device(&app, &auth, ip, forged_device_id, &forged_payload).await;
    assert_eq!(forged.status(), StatusCode::BAD_REQUEST);

    let replacement_root = RootIdentityKey::generate();
    let replacement_device_id = DeviceId::new();
    let replacement_device = MlsDevice::generate(user_id, replacement_device_id, &replacement_root)
        .expect("replacement device should generate");
    let changed_root = publish_device(
        &app,
        &auth,
        ip,
        replacement_device_id,
        &publish_payload(&replacement_device),
    )
    .await;
    assert_eq!(changed_root.status(), StatusCode::FORBIDDEN);

    let replacement_package =
        generate_key_package_batch(&device, 1).expect("replacement package should generate");
    let replacement_upload = UploadKeyPackagesRequest {
        device_id: device_id.to_string(),
        key_packages: replacement_package
            .into_iter()
            .map(|package| KeyPackageEntry {
                key_package_blob: package.blob,
                is_last_resort: false,
            })
            .collect(),
    };
    let replacement_upload = send_json(
        &app,
        "POST",
        "/e2ee/keypackages",
        Some(&auth.access_token),
        ip,
        &replacement_upload,
    )
    .await;
    assert_eq!(replacement_upload.status(), StatusCode::OK);

    let rotated_device =
        MlsDevice::generate(user_id, device_id, &root).expect("rotated device should generate");
    assert_ne!(
        rotated_device.certificate().device_signature_pubkey,
        device.certificate().device_signature_pubkey
    );
    let rotated = publish_device(
        &app,
        &auth,
        ip,
        device_id,
        &publish_payload(&rotated_device),
    )
    .await;
    assert_eq!(rotated.status(), StatusCode::OK);
    let device_rotated = next_gateway_event(&mut gateway, "device_list_update").await;
    assert_eq!(device_rotated["d"]["user_id"], user_id.to_string());
    assert_eq!(device_rotated["d"]["device_count"], 2);

    let stale_claim = app
        .clone()
        .oneshot(claim_request(&auth.access_token, ip, user_id, device_id))
        .await
        .expect("claim after rotation should execute");
    assert_eq!(stale_claim.status(), StatusCode::NOT_FOUND);

    let list_after_rotation = Request::builder()
        .method("GET")
        .uri(format!("/e2ee/users/{user_id}/devices"))
        .header("authorization", format!("Bearer {}", auth.access_token))
        .header("x-forwarded-for", ip)
        .body(Body::empty())
        .expect("list request should build");
    let list_after_rotation = app
        .clone()
        .oneshot(list_after_rotation)
        .await
        .expect("list after rotation should execute");
    assert_eq!(list_after_rotation.status(), StatusCode::OK);
    let list_after_rotation: DeviceListResponse = parse_json(list_after_rotation).await;
    let rotated_listing = list_after_rotation
        .devices
        .iter()
        .find(|listed_device| listed_device.device_id == device_id.to_string())
        .expect("rotated device should remain listed");
    assert_eq!(
        rotated_listing.device_signature_pubkey,
        rotated_device.certificate().device_signature_pubkey
    );
    let rotation_audit: String = sqlx::query_scalar(
        "SELECT metadata_json::TEXT FROM e2ee_audit_log
         WHERE action = 'device_rotate' AND user_id = $1 AND device_id = $2
         ORDER BY id DESC LIMIT 1",
    )
    .bind(user_id.to_string())
    .bind(device_id.to_string())
    .fetch_one(&audit_pool)
    .await
    .expect("rotation should be audit logged");
    let rotation_audit: serde_json::Value =
        serde_json::from_str(&rotation_audit).expect("rotation audit metadata should be JSON");
    assert_eq!(rotation_audit["deleted_keypackage_count"], 1);

    let post_rotation_package =
        generate_key_package_batch(&rotated_device, 1).expect("package should generate");
    let post_rotation_upload = UploadKeyPackagesRequest {
        device_id: device_id.to_string(),
        key_packages: post_rotation_package
            .into_iter()
            .map(|package| KeyPackageEntry {
                key_package_blob: package.blob,
                is_last_resort: false,
            })
            .collect(),
    };
    let post_rotation_upload = send_json(
        &app,
        "POST",
        "/e2ee/keypackages",
        Some(&auth.access_token),
        ip,
        &post_rotation_upload,
    )
    .await;
    assert_eq!(post_rotation_upload.status(), StatusCode::OK);

    let remove = Request::builder()
        .method("DELETE")
        .uri(format!("/e2ee/devices/{device_id}"))
        .header("authorization", format!("Bearer {}", auth.access_token))
        .header("x-forwarded-for", ip)
        .body(Body::empty())
        .expect("remove request should build");
    let removed = app
        .clone()
        .oneshot(remove)
        .await
        .expect("remove should execute");
    assert_eq!(removed.status(), StatusCode::OK);
    let removed: RemoveDeviceResponse = parse_json(removed).await;
    assert_eq!(removed.device_id, device_id.to_string());
    assert_eq!(removed.deleted_keypackage_count, 1);
    let device_removed = next_gateway_event(&mut gateway, "device_list_update").await;
    assert_eq!(device_removed["d"]["user_id"], user_id.to_string());
    assert_eq!(device_removed["d"]["device_count"], 1);

    let list_after_remove = Request::builder()
        .method("GET")
        .uri(format!("/e2ee/users/{user_id}/devices"))
        .header("authorization", format!("Bearer {}", auth.access_token))
        .header("x-forwarded-for", ip)
        .body(Body::empty())
        .expect("list request should build");
    let listed_after_remove = app
        .clone()
        .oneshot(list_after_remove)
        .await
        .expect("list should execute");
    assert_eq!(listed_after_remove.status(), StatusCode::OK);
    let listed_after_remove: DeviceListResponse = parse_json(listed_after_remove).await;
    assert_eq!(listed_after_remove.devices.len(), 1);
    assert_eq!(
        listed_after_remove.devices[0].device_id,
        paired_device_id.to_string()
    );

    let resurrect = publish_device(&app, &auth, ip, device_id, &publish_payload(&device)).await;
    assert_eq!(resurrect.status(), StatusCode::FORBIDDEN);

    let claim_removed = app
        .clone()
        .oneshot(claim_request(&auth.access_token, ip, user_id, device_id))
        .await
        .expect("claim against removed device should execute");
    assert_eq!(claim_removed.status(), StatusCode::NOT_FOUND);

    let successful_claim_audits: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM e2ee_audit_log
         WHERE action = 'keypackage_claim' AND user_id = $1",
    )
    .bind(user_id.to_string())
    .fetch_one(&audit_pool)
    .await
    .expect("claim audit count should be readable");
    assert_eq!(successful_claim_audits, 3);

    gateway
        .close(None)
        .await
        .expect("gateway should close cleanly");
    gateway_server.abort();
}

#[tokio::test]
async fn postgres_e2ee_publish_and_claim_routes_enforce_specific_rate_limits() {
    let Some(database_url) = postgres_url() else {
        eprintln!("skipping postgres-backed E2EE test: FILAMENT_TEST_DATABASE_URL is unset");
        return;
    };
    let app = test_app_with_e2ee_limits(database_url, 1, 1).await;
    let ip = "203.0.113.92";
    let (auth, user_id) = register_and_login(&app, ip).await;
    let root = RootIdentityKey::generate();
    let device_id = DeviceId::new();
    let device = MlsDevice::generate(user_id, device_id, &root).expect("device should generate");

    let first_publish = publish_device(&app, &auth, ip, device_id, &publish_payload(&device)).await;
    assert_eq!(first_publish.status(), StatusCode::OK);

    let second_device_id = DeviceId::new();
    let second_device = MlsDevice::generate(user_id, second_device_id, &root)
        .expect("second device should generate");
    let limited_publish = publish_device(
        &app,
        &auth,
        ip,
        second_device_id,
        &publish_payload(&second_device),
    )
    .await;
    assert_eq!(limited_publish.status(), StatusCode::TOO_MANY_REQUESTS);

    let packages = generate_key_package_batch(&device, 2).expect("packages should generate");
    let upload = UploadKeyPackagesRequest {
        device_id: device_id.to_string(),
        key_packages: packages
            .into_iter()
            .map(|package| KeyPackageEntry {
                key_package_blob: package.blob,
                is_last_resort: false,
            })
            .collect(),
    };
    let uploaded = send_json(
        &app,
        "POST",
        "/e2ee/keypackages",
        Some(&auth.access_token),
        ip,
        &upload,
    )
    .await;
    assert_eq!(uploaded.status(), StatusCode::OK);

    let first_claim = app
        .clone()
        .oneshot(claim_request(&auth.access_token, ip, user_id, device_id))
        .await
        .expect("first claim should execute");
    assert_eq!(first_claim.status(), StatusCode::OK);
    let limited_claim = app
        .clone()
        .oneshot(claim_request(&auth.access_token, ip, user_id, device_id))
        .await
        .expect("limited claim should execute");
    assert_eq!(limited_claim.status(), StatusCode::TOO_MANY_REQUESTS);
}

#[tokio::test]
async fn postgres_root_identity_rotation_is_dual_signed_atomic_and_replay_safe() {
    let Some(database_url) = postgres_url() else {
        eprintln!("skipping postgres-backed E2EE test: FILAMENT_TEST_DATABASE_URL is unset");
        return;
    };
    let audit_pool = sqlx::PgPool::connect(&database_url)
        .await
        .expect("audit verification pool should connect");
    let app = test_app(database_url).await;
    let ip = "203.0.113.93";
    let (auth, user_id) = register_and_login(&app, ip).await;
    let previous_root = RootIdentityKey::from_secret_bytes(&[0x31; 32]);
    let retained_device_id = DeviceId::new();
    let revoked_device_id = DeviceId::new();
    let retained_device = MlsDevice::generate(user_id, retained_device_id, &previous_root)
        .expect("retained device should generate");
    let revoked_device = MlsDevice::generate(user_id, revoked_device_id, &previous_root)
        .expect("revoked device should generate");
    assert_eq!(
        publish_device(
            &app,
            &auth,
            ip,
            retained_device_id,
            &publish_payload(&retained_device),
        )
        .await
        .status(),
        StatusCode::OK
    );
    assert_eq!(
        publish_device(
            &app,
            &auth,
            ip,
            revoked_device_id,
            &publish_payload(&revoked_device),
        )
        .await
        .status(),
        StatusCode::OK
    );
    for (device_id, device) in [
        (retained_device_id, &retained_device),
        (revoked_device_id, &revoked_device),
    ] {
        let package = generate_key_package_batch(device, 1).expect("package should generate");
        let upload = UploadKeyPackagesRequest {
            device_id: device_id.to_string(),
            key_packages: package
                .into_iter()
                .map(|package| KeyPackageEntry {
                    key_package_blob: package.blob,
                    is_last_resort: false,
                })
                .collect(),
        };
        assert_eq!(
            send_json(
                &app,
                "POST",
                "/e2ee/keypackages",
                Some(&auth.access_token),
                ip,
                &upload,
            )
            .await
            .status(),
            StatusCode::OK
        );
    }

    let replacement_root = RootIdentityKey::from_secret_bytes(&[0x52; 32]);
    let replacement_device = MlsDevice::generate(user_id, retained_device_id, &replacement_root)
        .expect("replacement device should generate");
    let proof = create_root_identity_rotation_proof(&previous_root, &replacement_root, user_id, 1)
        .expect("rotation proof should generate");
    let rotation_request = RotateRootIdentityRequest {
        protocol_version: ROOT_IDENTITY_ROTATION_PROTOCOL_VERSION,
        expected_rotation_sequence: 0,
        device_id: retained_device_id.to_string(),
        new_root_key_pub: proof.new_root_key_pub.to_vec(),
        previous_root_signature: proof.previous_root_signature.to_vec(),
        new_root_signature: proof.new_root_signature.to_vec(),
        new_device_signature_pubkey: replacement_device
            .certificate()
            .device_signature_pubkey
            .clone(),
        new_device_root_signature: replacement_device.certificate().root_key_signature.clone(),
    };
    let mut tampered_request = rotation_request.clone();
    tampered_request.previous_root_signature[0] ^= 1;
    let tampered = send_json(
        &app,
        "POST",
        "/e2ee/identity/rotate",
        Some(&auth.access_token),
        ip,
        &tampered_request,
    )
    .await;
    assert_eq!(tampered.status(), StatusCode::FORBIDDEN);

    let rotated = send_json(
        &app,
        "POST",
        "/e2ee/identity/rotate",
        Some(&auth.access_token),
        ip,
        &rotation_request,
    )
    .await;
    assert_eq!(rotated.status(), StatusCode::OK);
    let rotated: RotateRootIdentityResponse = parse_json(rotated).await;
    assert_eq!(rotated.rotation_sequence, 1);
    assert_eq!(rotated.revoked_device_count, 1);
    assert_eq!(rotated.deleted_keypackage_count, 2);
    assert_eq!(rotated.previous_root_key_pub, proof.previous_root_key_pub);
    assert_eq!(rotated.new_root_key_pub, proof.new_root_key_pub);

    let replay = send_json(
        &app,
        "POST",
        "/e2ee/identity/rotate",
        Some(&auth.access_token),
        ip,
        &rotation_request,
    )
    .await;
    assert_eq!(replay.status(), StatusCode::FORBIDDEN);

    let identity = Request::builder()
        .method("GET")
        .uri(format!("/e2ee/users/{user_id}/identity"))
        .header("authorization", format!("Bearer {}", auth.access_token))
        .header("x-forwarded-for", ip)
        .body(Body::empty())
        .expect("identity request should build");
    let identity = app
        .clone()
        .oneshot(identity)
        .await
        .expect("identity request should execute");
    assert_eq!(identity.status(), StatusCode::OK);
    let identity: RootIdentityDirectoryResponse = parse_json(identity).await;
    assert_eq!(identity.current_root_key_pub, proof.new_root_key_pub);
    assert_eq!(identity.rotation_sequence, 1);
    assert_eq!(identity.rotations.len(), 1);
    let listed_proof = RootIdentityRotationProof {
        sequence: identity.rotations[0].sequence,
        previous_root_key_pub: identity.rotations[0]
            .previous_root_key_pub
            .as_slice()
            .try_into()
            .expect("previous root should have exact length"),
        new_root_key_pub: identity.rotations[0]
            .new_root_key_pub
            .as_slice()
            .try_into()
            .expect("new root should have exact length"),
        previous_root_signature: identity.rotations[0]
            .previous_root_signature
            .as_slice()
            .try_into()
            .expect("previous signature should have exact length"),
        new_root_signature: identity.rotations[0]
            .new_root_signature
            .as_slice()
            .try_into()
            .expect("new signature should have exact length"),
    };
    verify_root_identity_rotation_proof(user_id, &listed_proof)
        .expect("listed continuity proof should verify");

    for device_id in [retained_device_id, revoked_device_id] {
        let claim = app
            .clone()
            .oneshot(claim_request(&auth.access_token, ip, user_id, device_id))
            .await
            .expect("stale claim should execute");
        assert_eq!(claim.status(), StatusCode::NOT_FOUND);
    }
    let revoked_republish = MlsDevice::generate(user_id, revoked_device_id, &replacement_root)
        .expect("revoked replacement should generate");
    assert_eq!(
        publish_device(
            &app,
            &auth,
            ip,
            revoked_device_id,
            &publish_payload(&revoked_republish),
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );

    let audit_metadata: String = sqlx::query_scalar(
        "SELECT metadata_json::TEXT FROM e2ee_audit_log
         WHERE action = 'identity_rotate' AND user_id = $1 ORDER BY id DESC LIMIT 1",
    )
    .bind(user_id.to_string())
    .fetch_one(&audit_pool)
    .await
    .expect("identity rotation should be audit logged");
    let audit_metadata: serde_json::Value =
        serde_json::from_str(&audit_metadata).expect("audit metadata should be JSON");
    assert_eq!(audit_metadata["rotation_sequence"], 1);
    assert_eq!(audit_metadata["revoked_device_count"], 1);
    assert_eq!(audit_metadata["deleted_keypackage_count"], 2);
}
