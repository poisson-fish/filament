use std::{env, time::Duration};

use axum::{body::Body, http::Request, http::StatusCode, response::Response};
use filament_core::{ConversationId, DeviceId, GroupId, UserId};
use filament_e2ee::{
    create_pairing_transfer, create_root_identity_rotation_proof, decrypt_attachment,
    encrypt_attachment, generate_key_package_batch, generate_last_resort_key_package,
    process_message_mailbox, verify_root_identity_rotation_proof, EncryptedAttachment,
    MlsConversation, MlsDevice, PairingReceiver, PairingTransfer, PinnedUserIdentity,
    RootIdentityKey, RootIdentityRotationProof, ScannedPairingOffer, DEFAULT_PAIRING_TTL_SECS,
};
use filament_protocol::{
    AckE2eeAttachmentsRequest, AckE2eeAttachmentsResponse, AckE2eeCommitsRequest,
    AckE2eeCommitsResponse, AckE2eeMessagesRequest, AckE2eeMessagesResponse,
    AckE2eeProposalsRequest, AckE2eeProposalsResponse, ClaimKeyPackageRequest,
    ClaimKeyPackageResponse, CreateMlsConversationRequest, CreateMlsEncryptedChannelRequest,
    CreateMlsGroupConversationRequest, DeviceListResponse, E2eeCommitMailboxResponse,
    E2eeMailboxResponse, E2eeProposalMailboxResponse, E2eeRetentionSeconds, GroupInfoResponse,
    KeyPackageEntry, MlsConversationProvisionResponse, MlsEncryptedChannelKind,
    MlsEncryptedChannelProvisionResponse, MlsEncryptedChannelType, MlsGroupInvite, MlsLeafRouting,
    MlsMembershipChange, PostCommitRequest, PostCommitResponse, PostMessageRequest,
    PostMessageResponse, PostProposalRequest, PostProposalResponse,
    PublishDeviceCertificateRequest, PutE2eeAttachmentResponse, RemoveDeviceResponse,
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
#[allow(clippy::too_many_lines)]
async fn postgres_e2ee_delivery_orders_commits_and_relays_opaque_mailboxes() {
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
    assert!(
        bob_second_initial_commits.commits.is_empty(),
        "an active certified device that is not an MLS leaf must not receive commits"
    );
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
    let mut bob_conversation = MlsConversation::join_from_welcome(
        group_id,
        &bob_device,
        PinnedUserIdentity::new(alice_user_id, alice_root.public_key_bytes()),
        &create_request.welcome_blob,
    )
    .expect("Bob should join the provisioned group from its Welcome");

    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("native gateway smoke listener should bind");
    let gateway_address = listener
        .local_addr()
        .expect("native gateway smoke address should be available");
    let gateway_app = app.clone();
    let gateway_server = tokio::spawn(async move {
        axum::serve(listener, gateway_app)
            .await
            .expect("native gateway smoke server should run");
    });
    let gateway_url = format!("ws://{gateway_address}/gateway/ws");
    let mut gateway_request = gateway_url
        .into_client_request()
        .expect("native gateway smoke request should build");
    gateway_request.headers_mut().insert(
        "authorization",
        format!("Bearer {}", bob_auth.access_token)
            .parse()
            .expect("bearer token should parse as a header value"),
    );
    gateway_request.headers_mut().insert(
        "x-forwarded-for",
        "203.0.113.182"
            .parse()
            .expect("fixture IP should parse as a header value"),
    );
    let (mut gateway, _) = connect_async(gateway_request)
        .await
        .expect("native bearer-header gateway should connect");
    let ready = next_gateway_event(&mut gateway, "ready").await;
    assert_eq!(ready["d"]["user_id"], bob_user_id.to_string());

    let online_plaintext = b"native real-server immediate receive";
    let online_encrypted = alice_conversation
        .encrypt_application_message(&alice_device, online_plaintext)
        .expect("online smoke message should encrypt");
    let online_request = PostMessageRequest {
        epoch: online_encrypted.epoch,
        suite_id: online_encrypted.suite.as_u16(),
        sender_device_id: online_encrypted.sender_device_id.to_string(),
        retention_secs: None,
        message_blob: online_encrypted.message_blob,
    };
    let online_message = send_json(
        &app,
        "POST",
        &format!("/e2ee/groups/{group_id}/messages"),
        Some(&alice_auth.access_token),
        "203.0.113.181",
        &online_request,
    )
    .await;
    assert_eq!(online_message.status(), StatusCode::OK);
    let online_message: PostMessageResponse = parse_json(online_message).await;
    let online_wake = next_gateway_event(&mut gateway, "mls_message").await;
    assert_eq!(online_wake["d"]["group_id"], group_id.to_string());
    assert_eq!(
        online_wake["d"]["conversation_id"],
        conversation_id.to_string()
    );
    assert_eq!(online_wake["d"]["message_id"], online_message.message_id);
    assert_eq!(
        online_wake["d"]["sender_device_id"],
        alice_device_id.to_string()
    );
    assert_eq!(online_wake["d"]["epoch"], online_request.epoch);
    assert_eq!(online_wake["d"]["suite_id"], online_request.suite_id);
    assert!(
        online_wake["d"].get("message_blob").is_none(),
        "gateway wake must not carry ciphertext"
    );

    let online_mailbox = Request::builder()
        .method("GET")
        .uri(format!(
            "/e2ee/groups/{group_id}/mailbox?device_id={bob_device_id}"
        ))
        .header("authorization", format!("Bearer {}", bob_auth.access_token))
        .header("x-forwarded-for", "203.0.113.182")
        .body(Body::empty())
        .expect("online mailbox request should build");
    let online_mailbox = app
        .clone()
        .oneshot(online_mailbox)
        .await
        .expect("online mailbox request should execute");
    assert_eq!(online_mailbox.status(), StatusCode::OK);
    let online_mailbox: E2eeMailboxResponse = parse_json(online_mailbox).await;
    let online_batch = process_message_mailbox(&mut bob_conversation, &bob_device, online_mailbox)
        .expect("online mailbox should authenticate and decrypt");
    assert!(online_batch.rejected_messages.is_empty());
    assert_eq!(online_batch.ready_messages.len(), 1);
    assert_eq!(
        online_batch.ready_messages[0].plaintext.as_slice(),
        online_plaintext
    );
    let online_ack = online_batch
        .pending_acknowledgment
        .expect("authenticated online message should be acknowledged");
    assert_eq!(
        online_ack.message_ids,
        vec![online_message.message_id.clone()]
    );
    let online_ack_response = send_json(
        &app,
        "POST",
        &format!("/e2ee/groups/{group_id}/messages/ack"),
        Some(&bob_auth.access_token),
        "203.0.113.182",
        &online_ack,
    )
    .await;
    assert_eq!(online_ack_response.status(), StatusCode::OK);
    let online_deleted: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM e2ee_messages WHERE message_id = $1")
            .bind(&online_message.message_id)
            .fetch_one(&audit_pool)
            .await
            .expect("online smoke deletion should be queryable");
    assert_eq!(online_deleted, 0);

    gateway
        .close(None)
        .await
        .expect("native gateway should close cleanly");
    gateway_server.abort();

    let offline_plaintext = b"native real-server offline reconciliation";
    let offline_encrypted = alice_conversation
        .encrypt_application_message(&alice_device, offline_plaintext)
        .expect("offline smoke message should encrypt");
    let offline_request = PostMessageRequest {
        epoch: offline_encrypted.epoch,
        suite_id: offline_encrypted.suite.as_u16(),
        sender_device_id: offline_encrypted.sender_device_id.to_string(),
        retention_secs: None,
        message_blob: offline_encrypted.message_blob,
    };
    let offline_message = send_json(
        &app,
        "POST",
        &format!("/e2ee/groups/{group_id}/messages"),
        Some(&alice_auth.access_token),
        "203.0.113.181",
        &offline_request,
    )
    .await;
    assert_eq!(offline_message.status(), StatusCode::OK);
    let offline_message: PostMessageResponse = parse_json(offline_message).await;
    let offline_mailbox = Request::builder()
        .method("GET")
        .uri(format!(
            "/e2ee/groups/{group_id}/mailbox?device_id={bob_device_id}"
        ))
        .header("authorization", format!("Bearer {}", bob_auth.access_token))
        .header("x-forwarded-for", "203.0.113.182")
        .body(Body::empty())
        .expect("offline mailbox request should build");
    let offline_mailbox = app
        .clone()
        .oneshot(offline_mailbox)
        .await
        .expect("offline mailbox request should execute");
    assert_eq!(offline_mailbox.status(), StatusCode::OK);
    let offline_mailbox: E2eeMailboxResponse = parse_json(offline_mailbox).await;
    let offline_batch =
        process_message_mailbox(&mut bob_conversation, &bob_device, offline_mailbox)
            .expect("offline mailbox should authenticate and decrypt");
    assert!(offline_batch.rejected_messages.is_empty());
    assert_eq!(offline_batch.ready_messages.len(), 1);
    assert_eq!(
        offline_batch.ready_messages[0].plaintext.as_slice(),
        offline_plaintext
    );
    let offline_ack = offline_batch
        .pending_acknowledgment
        .expect("authenticated offline message should be acknowledged");
    assert_eq!(
        offline_ack.message_ids,
        vec![offline_message.message_id.clone()]
    );
    let offline_ack_response = send_json(
        &app,
        "POST",
        &format!("/e2ee/groups/{group_id}/messages/ack"),
        Some(&bob_auth.access_token),
        "203.0.113.182",
        &offline_ack,
    )
    .await;
    assert_eq!(offline_ack_response.status(), StatusCode::OK);
    let offline_deleted: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM e2ee_messages WHERE message_id = $1")
            .bind(&offline_message.message_id)
            .fetch_one(&audit_pool)
            .await
            .expect("offline smoke deletion should be queryable");
    assert_eq!(offline_deleted, 0);

    let plaintext_fallback_rows: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM messages WHERE content = ANY($1::TEXT[])")
            .bind(vec![
                String::from_utf8(online_plaintext.to_vec()).expect("fixture should be UTF-8"),
                String::from_utf8(offline_plaintext.to_vec()).expect("fixture should be UTF-8"),
            ])
            .fetch_one(&audit_pool)
            .await
            .expect("plaintext fallback absence should be queryable");
    assert_eq!(
        plaintext_fallback_rows, 0,
        "native E2EE smoke messages must never enter the plaintext message table"
    );
    let smoke_conversation_crypto: String = sqlx::query_scalar(
        "SELECT conversation_crypto FROM e2ee_conversations WHERE conversation_id = $1",
    )
    .bind(conversation_id.to_string())
    .fetch_one(&audit_pool)
    .await
    .expect("smoke conversation crypto mode should be queryable");
    assert_eq!(smoke_conversation_crypto, "mls_v1");

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

    let provisioned_group_request = CreateMlsGroupConversationRequest {
        conversation_id: ConversationId::new().to_string(),
        group_id: GroupId::new().to_string(),
        suite_id: 3,
        committer_device_id: alice_device_id.to_string(),
        invitees: vec![
            MlsGroupInvite {
                user_id: bob_user_id.to_string(),
                welcome_device_id: bob_device_id.to_string(),
                leaf_index: 1,
            },
            MlsGroupInvite {
                user_id: charlie_user_id.to_string(),
                welcome_device_id: charlie_device_id.to_string(),
                leaf_index: 2,
            },
        ],
        commit_blob: vec![0x68; 256],
        welcome_blob: vec![0x69; 192],
        group_info_blob: vec![0x6A; 128],
    };
    let provisioned_group = send_json(
        &app,
        "POST",
        "/e2ee/group-conversations",
        Some(&alice_auth.access_token),
        "203.0.113.181",
        &provisioned_group_request,
    )
    .await;
    assert_eq!(provisioned_group.status(), StatusCode::OK);
    let provisioned_group: MlsConversationProvisionResponse = parse_json(provisioned_group).await;
    assert_eq!(provisioned_group.epoch, 1);
    let provisioned_group_retry = send_json(
        &app,
        "POST",
        "/e2ee/group-conversations",
        Some(&alice_auth.access_token),
        "203.0.113.181",
        &provisioned_group_request,
    )
    .await;
    assert_eq!(provisioned_group_retry.status(), StatusCode::OK);
    assert_eq!(
        parse_json::<MlsConversationProvisionResponse>(provisioned_group_retry).await,
        provisioned_group
    );

    let encrypted_workspace = send_json(
        &app,
        "POST",
        "/guilds",
        Some(&alice_auth.access_token),
        "203.0.113.181",
        &json!({"name": "Phase 6 Atomic Channel"}),
    )
    .await;
    assert_eq!(encrypted_workspace.status(), StatusCode::OK);
    let encrypted_workspace: serde_json::Value = parse_json(encrypted_workspace).await;
    let encrypted_workspace_id = encrypted_workspace["guild_id"]
        .as_str()
        .expect("created workspace should include an ID");
    for member_id in [bob_user_id, charlie_user_id] {
        let member = send_json(
            &app,
            "POST",
            &format!("/guilds/{encrypted_workspace_id}/members/{member_id}"),
            Some(&alice_auth.access_token),
            "203.0.113.181",
            &json!({}),
        )
        .await;
        assert_eq!(member.status(), StatusCode::OK);
    }
    let policy = send_json(
        &app,
        "PATCH",
        &format!("/guilds/{encrypted_workspace_id}"),
        Some(&alice_auth.access_token),
        "203.0.113.181",
        &json!({"encrypted_channel_policy": "require_moderator_membership"}),
    )
    .await;
    assert_eq!(policy.status(), StatusCode::OK);

    let encrypted_channel_request = CreateMlsEncryptedChannelRequest {
        channel_id: Ulid::new().to_string(),
        channel_name: String::from("sealed-team"),
        conversation_id: ConversationId::new().to_string(),
        group_id: GroupId::new().to_string(),
        suite_id: 3,
        committer_device_id: alice_device_id.to_string(),
        invitees: vec![
            MlsGroupInvite {
                user_id: bob_user_id.to_string(),
                welcome_device_id: bob_device_id.to_string(),
                leaf_index: 1,
            },
            MlsGroupInvite {
                user_id: charlie_user_id.to_string(),
                welcome_device_id: charlie_device_id.to_string(),
                leaf_index: 2,
            },
        ],
        commit_blob: vec![0x71; 256],
        welcome_blob: vec![0x72; 192],
        group_info_blob: vec![0x73; 128],
    };
    let encrypted_channel = send_json(
        &app,
        "POST",
        &format!("/guilds/{encrypted_workspace_id}/e2ee/channels"),
        Some(&alice_auth.access_token),
        "203.0.113.181",
        &encrypted_channel_request,
    )
    .await;
    assert_eq!(encrypted_channel.status(), StatusCode::OK);
    let encrypted_channel: MlsEncryptedChannelProvisionResponse =
        parse_json(encrypted_channel).await;
    assert_eq!(
        encrypted_channel.channel_id,
        encrypted_channel_request.channel_id
    );
    assert_eq!(
        encrypted_channel.channel_type,
        MlsEncryptedChannelType::Encrypted
    );
    assert_eq!(encrypted_channel.kind, MlsEncryptedChannelKind::Text);
    assert_eq!(encrypted_channel.crypto, "mls_v1");
    assert_eq!(encrypted_channel.epoch, 1);

    let encrypted_channel_retry = send_json(
        &app,
        "POST",
        &format!("/guilds/{encrypted_workspace_id}/e2ee/channels"),
        Some(&alice_auth.access_token),
        "203.0.113.181",
        &encrypted_channel_request,
    )
    .await;
    assert_eq!(encrypted_channel_retry.status(), StatusCode::OK);
    assert_eq!(
        parse_json::<MlsEncryptedChannelProvisionResponse>(encrypted_channel_retry).await,
        encrypted_channel
    );

    let mut altered_channel_retry = encrypted_channel_request.clone();
    altered_channel_retry.channel_name = String::from("substituted");
    let altered_channel_retry = send_json(
        &app,
        "POST",
        &format!("/guilds/{encrypted_workspace_id}/e2ee/channels"),
        Some(&alice_auth.access_token),
        "203.0.113.181",
        &altered_channel_retry,
    )
    .await;
    assert_eq!(altered_channel_retry.status(), StatusCode::CONFLICT);
    let altered_channel_retry: serde_json::Value = parse_json(altered_channel_retry).await;
    assert_eq!(
        altered_channel_retry["error"],
        serde_json::Value::from("e2ee_conversation_conflict")
    );

    let channel_binding: (String, String, String) = sqlx::query_as(
        "SELECT guild_id, conversation_id, group_id
         FROM e2ee_channel_groups
         WHERE channel_id = $1",
    )
    .bind(&encrypted_channel.channel_id)
    .fetch_one(&audit_pool)
    .await
    .expect("encrypted channel should have one durable MLS binding");
    assert_eq!(channel_binding.0, encrypted_workspace_id);
    assert_eq!(channel_binding.1, encrypted_channel_request.conversation_id);
    assert_eq!(channel_binding.2, encrypted_channel_request.group_id);

    // Seed the server-side routing view for a three-user group. MLS interiors
    // remain opaque here; this exercises only bounded Delivery Service fanout.
    let group_conversation_id = ConversationId::new();
    let group_delivery_id = GroupId::new();
    let group_seeded_at = 1_750_000_001_i64;
    let mut group_seed = audit_pool
        .begin()
        .await
        .expect("group delivery seed transaction should begin");
    sqlx::query(
        "INSERT INTO e2ee_conversations
            (conversation_id, conversation_crypto, created_by, created_at_unix)
         VALUES ($1, 'mls_v1', $2, $3)",
    )
    .bind(group_conversation_id.to_string())
    .bind(alice_user_id.to_string())
    .bind(group_seeded_at)
    .execute(&mut *group_seed)
    .await
    .expect("group delivery conversation should seed");
    for user_id in [alice_user_id, bob_user_id, charlie_user_id] {
        sqlx::query(
            "INSERT INTO e2ee_conversation_members
                (conversation_id, user_id, joined_at_unix) VALUES ($1, $2, $3)",
        )
        .bind(group_conversation_id.to_string())
        .bind(user_id.to_string())
        .bind(group_seeded_at)
        .execute(&mut *group_seed)
        .await
        .expect("group delivery member should seed");
    }
    sqlx::query(
        "INSERT INTO e2ee_groups
            (group_id, conversation_id, current_epoch, suite_id,
             group_info_blob, created_at_unix)
         VALUES ($1, $2, 1, 3, $3, $4)",
    )
    .bind(group_delivery_id.to_string())
    .bind(group_conversation_id.to_string())
    .bind(vec![0x71_u8; 128])
    .bind(group_seeded_at)
    .execute(&mut *group_seed)
    .await
    .expect("group delivery MLS group should seed");
    for (leaf_index, user_id, device_id) in [
        (0_i32, alice_user_id, alice_device_id),
        (1_i32, bob_user_id, bob_device_id),
        (2_i32, charlie_user_id, charlie_device_id),
    ] {
        sqlx::query(
            "INSERT INTO e2ee_group_leaves
                (group_id, leaf_index, user_id, device_id, added_epoch)
             VALUES ($1, $2, $3, $4, 1)",
        )
        .bind(group_delivery_id.to_string())
        .bind(leaf_index)
        .bind(user_id.to_string())
        .bind(device_id.to_string())
        .execute(&mut *group_seed)
        .await
        .expect("group delivery leaf should seed");
    }
    group_seed
        .commit()
        .await
        .expect("group delivery seed transaction should commit");

    let group_message = send_json(
        &app,
        "POST",
        &format!("/e2ee/groups/{group_delivery_id}/messages"),
        Some(&alice_auth.access_token),
        "203.0.113.181",
        &PostMessageRequest {
            epoch: 1,
            suite_id: 3,
            sender_device_id: alice_device_id.to_string(),
            retention_secs: None,
            message_blob: vec![0x72; 512],
        },
    )
    .await;
    assert_eq!(group_message.status(), StatusCode::OK);
    let group_message: PostMessageResponse = parse_json(group_message).await;
    let charlie_group_mailbox = Request::builder()
        .method("GET")
        .uri(format!(
            "/e2ee/groups/{group_delivery_id}/mailbox?device_id={charlie_device_id}"
        ))
        .header(
            "authorization",
            format!("Bearer {}", charlie_auth.access_token),
        )
        .header("x-forwarded-for", "203.0.113.183")
        .body(Body::empty())
        .expect("group message mailbox request should build");
    let charlie_group_mailbox = app
        .clone()
        .oneshot(charlie_group_mailbox)
        .await
        .expect("group message mailbox request should execute");
    assert_eq!(charlie_group_mailbox.status(), StatusCode::OK);
    let charlie_group_mailbox: E2eeMailboxResponse = parse_json(charlie_group_mailbox).await;
    assert_eq!(charlie_group_mailbox.messages.len(), 1);
    assert_eq!(
        charlie_group_mailbox.messages[0].message_id,
        group_message.message_id
    );

    let group_commit = send_json(
        &app,
        "POST",
        &format!("/e2ee/groups/{group_delivery_id}/commits"),
        Some(&alice_auth.access_token),
        "203.0.113.181",
        &PostCommitRequest {
            epoch: 2,
            prior_epoch: 1,
            committer_device_id: alice_device_id.to_string(),
            commit_blob: vec![0x73; 256],
            welcome_blob: None,
            welcome_device_id: None,
            group_info_blob: Some(vec![0x74; 128]),
            membership_change: None,
        },
    )
    .await;
    assert_eq!(group_commit.status(), StatusCode::OK);
    let charlie_group_commits = Request::builder()
        .method("GET")
        .uri(format!(
            "/e2ee/groups/{group_delivery_id}/commits?device_id={charlie_device_id}"
        ))
        .header(
            "authorization",
            format!("Bearer {}", charlie_auth.access_token),
        )
        .header("x-forwarded-for", "203.0.113.183")
        .body(Body::empty())
        .expect("group commit mailbox request should build");
    let charlie_group_commits = app
        .clone()
        .oneshot(charlie_group_commits)
        .await
        .expect("group commit mailbox request should execute");
    assert_eq!(charlie_group_commits.status(), StatusCode::OK);
    let charlie_group_commits: E2eeCommitMailboxResponse = parse_json(charlie_group_commits).await;
    assert_eq!(charlie_group_commits.commits.len(), 1);
    assert_eq!(charlie_group_commits.commits[0].epoch, 2);

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
        membership_change: None,
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
    let (accepted_response, conflict_response, accepted_request) =
        if first_response.status() == StatusCode::OK {
            (first_response, competing_response, first_commit)
        } else {
            (competing_response, first_response, competing_commit)
        };
    let accepted: PostCommitResponse = parse_json(accepted_response).await;
    assert!(accepted.accepted);
    assert_eq!(accepted.epoch, 2);
    let conflict: serde_json::Value = parse_json(conflict_response).await;
    assert_eq!(conflict["error"], "epoch_conflict");

    let exact_retry = send_json(
        &app,
        "POST",
        &commit_uri,
        Some(&alice_auth.access_token),
        "203.0.113.181",
        &accepted_request,
    )
    .await;
    assert_eq!(exact_retry.status(), StatusCode::OK);
    assert_eq!(
        parse_json::<PostCommitResponse>(exact_retry).await,
        accepted
    );
    let conflicting_retry = send_json(
        &app,
        "POST",
        &commit_uri,
        Some(&alice_auth.access_token),
        "203.0.113.181",
        &PostCommitRequest {
            group_info_blob: Some(vec![0xC3; 128]),
            ..accepted_request.clone()
        },
    )
    .await;
    assert_eq!(conflicting_retry.status(), StatusCode::CONFLICT);
    assert_eq!(
        parse_json::<serde_json::Value>(conflicting_retry).await["error"],
        "epoch_conflict"
    );
    let stored_commit_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM e2ee_commits WHERE group_id = $1 AND epoch = 2")
            .bind(group_id.to_string())
            .fetch_one(&audit_pool)
            .await
            .expect("accepted commit should be stored once");
    assert_eq!(stored_commit_count, 1);
    let stored_receipts: Vec<Vec<u8>> = sqlx::query_scalar(
        "SELECT request_sha256 FROM e2ee_commit_receipts
         WHERE group_id = $1 AND epoch = 2",
    )
    .bind(group_id.to_string())
    .fetch_all(&audit_pool)
    .await
    .expect("accepted commit should retain one retry receipt");
    assert_eq!(stored_receipts.len(), 1);
    assert_eq!(stored_receipts[0].len(), 32);
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

    let proposal_blob = vec![0x91; 384];
    let proposal_request = PostProposalRequest {
        epoch: 2,
        proposer_device_id: alice_device_id.to_string(),
        proposal_blob: proposal_blob.clone(),
    };
    let proposal = send_json(
        &app,
        "POST",
        &format!("/e2ee/groups/{group_id}/proposals"),
        Some(&alice_auth.access_token),
        "203.0.113.181",
        &proposal_request,
    )
    .await;
    assert_eq!(proposal.status(), StatusCode::OK);
    let proposal: PostProposalResponse = parse_json(proposal).await;
    let stored_proposal: (Vec<u8>, i64, i64) = sqlx::query_as(
        "SELECT proposal_blob, created_at_unix, expires_at_unix
         FROM e2ee_proposals WHERE proposal_id = $1",
    )
    .bind(&proposal.proposal_id)
    .fetch_one(&audit_pool)
    .await
    .expect("opaque proposal should be stored");
    assert_eq!(stored_proposal.0, proposal_blob);
    assert!(stored_proposal.2 > stored_proposal.1);

    let bob_proposal_mailbox = Request::builder()
        .method("GET")
        .uri(format!(
            "/e2ee/groups/{group_id}/proposals?device_id={bob_device_id}&limit=20"
        ))
        .header("authorization", format!("Bearer {}", bob_auth.access_token))
        .header("x-forwarded-for", "203.0.113.182")
        .body(Body::empty())
        .expect("proposal mailbox request should build");
    let bob_proposal_mailbox = app
        .clone()
        .oneshot(bob_proposal_mailbox)
        .await
        .expect("proposal mailbox request should execute");
    assert_eq!(bob_proposal_mailbox.status(), StatusCode::OK);
    let bob_proposal_mailbox: E2eeProposalMailboxResponse = parse_json(bob_proposal_mailbox).await;
    assert_eq!(bob_proposal_mailbox.proposals.len(), 1);
    assert_eq!(
        bob_proposal_mailbox.proposals[0].proposal_id,
        proposal.proposal_id
    );
    assert_eq!(
        bob_proposal_mailbox.proposals[0].proposal_blob,
        proposal_blob
    );
    assert_eq!(
        bob_proposal_mailbox.next_after_proposal_id.as_deref(),
        Some(proposal.proposal_id.as_str())
    );

    let non_leaf_proposal_mailbox = Request::builder()
        .method("GET")
        .uri(format!(
            "/e2ee/groups/{group_id}/proposals?device_id={bob_second_device_id}"
        ))
        .header("authorization", format!("Bearer {}", bob_auth.access_token))
        .header("x-forwarded-for", "203.0.113.182")
        .body(Body::empty())
        .expect("non-leaf proposal mailbox request should build");
    let non_leaf_proposal_mailbox = app
        .clone()
        .oneshot(non_leaf_proposal_mailbox)
        .await
        .expect("non-leaf proposal mailbox request should execute");
    assert_eq!(non_leaf_proposal_mailbox.status(), StatusCode::OK);
    assert!(
        parse_json::<E2eeProposalMailboxResponse>(non_leaf_proposal_mailbox)
            .await
            .proposals
            .is_empty(),
        "a certified non-leaf device must not receive MLS proposals"
    );

    let proposer_mailbox = Request::builder()
        .method("GET")
        .uri(format!(
            "/e2ee/groups/{group_id}/proposals?device_id={alice_device_id}"
        ))
        .header(
            "authorization",
            format!("Bearer {}", alice_auth.access_token),
        )
        .header("x-forwarded-for", "203.0.113.181")
        .body(Body::empty())
        .expect("proposer mailbox request should build");
    let proposer_mailbox = app
        .clone()
        .oneshot(proposer_mailbox)
        .await
        .expect("proposer mailbox request should execute");
    assert_eq!(proposer_mailbox.status(), StatusCode::OK);
    assert!(parse_json::<E2eeProposalMailboxResponse>(proposer_mailbox)
        .await
        .proposals
        .is_empty());

    let stale_proposal = send_json(
        &app,
        "POST",
        &format!("/e2ee/groups/{group_id}/proposals"),
        Some(&alice_auth.access_token),
        "203.0.113.181",
        &PostProposalRequest {
            epoch: 1,
            ..proposal_request.clone()
        },
    )
    .await;
    assert_eq!(stale_proposal.status(), StatusCode::CONFLICT);
    let spoofed_proposer = send_json(
        &app,
        "POST",
        &format!("/e2ee/groups/{group_id}/proposals"),
        Some(&bob_auth.access_token),
        "203.0.113.182",
        &proposal_request,
    )
    .await;
    assert_eq!(spoofed_proposer.status(), StatusCode::NOT_FOUND);

    let first_proposal_ack = send_json(
        &app,
        "POST",
        &format!("/e2ee/groups/{group_id}/proposals/ack"),
        Some(&bob_auth.access_token),
        "203.0.113.182",
        &AckE2eeProposalsRequest {
            device_id: bob_device_id.to_string(),
            proposal_ids: vec![proposal.proposal_id.clone()],
        },
    )
    .await;
    assert_eq!(first_proposal_ack.status(), StatusCode::OK);
    let first_proposal_ack: AckE2eeProposalsResponse = parse_json(first_proposal_ack).await;
    assert_eq!(first_proposal_ack.acknowledged_count, 1);
    assert_eq!(first_proposal_ack.deleted_count, 1);
    let remaining_proposals: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM e2ee_proposals WHERE proposal_id = $1")
            .bind(&proposal.proposal_id)
            .fetch_one(&audit_pool)
            .await
            .expect("proposal deletion should be observable");
    assert_eq!(remaining_proposals, 0);

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
    assert_eq!(first_commit_ack.deleted_count, 2);
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
        retention_secs: Some(E2eeRetentionSeconds::new(60).unwrap()),
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
    let exact_retry = send_json(
        &app,
        "POST",
        &format!("/e2ee/groups/{group_id}/messages"),
        Some(&alice_auth.access_token),
        "203.0.113.181",
        &message_request,
    )
    .await;
    assert_eq!(exact_retry.status(), StatusCode::OK);
    let exact_retry: PostMessageResponse = parse_json(exact_retry).await;
    assert_eq!(exact_retry, message);
    let altered_replay = send_json(
        &app,
        "POST",
        &format!("/e2ee/groups/{group_id}/messages"),
        Some(&alice_auth.access_token),
        "203.0.113.181",
        &PostMessageRequest {
            retention_secs: None,
            ..message_request.clone()
        },
    )
    .await;
    assert_eq!(altered_replay.status(), StatusCode::BAD_REQUEST);
    let stored_message_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM e2ee_messages WHERE group_id = $1")
            .bind(group_id.to_string())
            .fetch_one(&audit_pool)
            .await
            .expect("exact retry must not duplicate ciphertext");
    assert_eq!(stored_message_count, 1);
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
    assert_eq!(stored_message.3, stored_message.2 + 60);

    let bob_mailbox = Request::builder()
        .method("GET")
        .uri(format!(
            "/e2ee/groups/{group_id}/mailbox?device_id={bob_device_id}&limit=20"
        ))
        .header("authorization", format!("Bearer {}", bob_auth.access_token))
        .header("x-forwarded-for", "203.0.113.182")
        .body(Body::empty())
        .expect("mailbox request should build");
    let bob_mailbox = app
        .clone()
        .oneshot(bob_mailbox)
        .await
        .expect("mailbox request should execute");
    assert_eq!(bob_mailbox.status(), StatusCode::OK);
    let bob_mailbox: E2eeMailboxResponse = parse_json(bob_mailbox).await;
    assert_eq!(bob_mailbox.messages.len(), 1);
    assert_eq!(bob_mailbox.messages[0].message_id, message.message_id);
    assert_eq!(bob_mailbox.messages[0].crypto, "mls_v1");
    assert_eq!(bob_mailbox.messages[0].message_blob, message_blob);

    let non_leaf_mailbox = Request::builder()
        .method("GET")
        .uri(format!(
            "/e2ee/groups/{group_id}/mailbox?device_id={bob_second_device_id}"
        ))
        .header("authorization", format!("Bearer {}", bob_auth.access_token))
        .header("x-forwarded-for", "203.0.113.182")
        .body(Body::empty())
        .expect("non-leaf message mailbox request should build");
    let non_leaf_mailbox = app
        .clone()
        .oneshot(non_leaf_mailbox)
        .await
        .expect("non-leaf message mailbox request should execute");
    assert_eq!(non_leaf_mailbox.status(), StatusCode::OK);
    assert!(
        parse_json::<E2eeMailboxResponse>(non_leaf_mailbox)
            .await
            .messages
            .is_empty(),
        "a certified non-leaf device must not receive MLS application ciphertext"
    );

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
    assert_eq!(first_ack.deleted_count, 1);
    let deleted_after_all_acks: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM e2ee_messages WHERE message_id = $1")
            .bind(&message.message_id)
            .fetch_one(&audit_pool)
            .await
            .expect("message deletion should be observable");
    assert_eq!(deleted_after_all_acks, 0);

    let retry_after_ciphertext_deletion = send_json(
        &app,
        "POST",
        &format!("/e2ee/groups/{group_id}/messages"),
        Some(&alice_auth.access_token),
        "203.0.113.181",
        &message_request,
    )
    .await;
    assert_eq!(retry_after_ciphertext_deletion.status(), StatusCode::OK);
    let retry_after_ciphertext_deletion: PostMessageResponse =
        parse_json(retry_after_ciphertext_deletion).await;
    assert_eq!(retry_after_ciphertext_deletion, message);

    let expiring_request = PostMessageRequest {
        message_blob: vec![0xCD; 512],
        ..message_request.clone()
    };
    let expiring = send_json(
        &app,
        "POST",
        &format!("/e2ee/groups/{group_id}/messages"),
        Some(&alice_auth.access_token),
        "203.0.113.181",
        &expiring_request,
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

    let remove_charlie = Request::builder()
        .method("DELETE")
        .uri(format!("/e2ee/devices/{charlie_device_id}"))
        .header(
            "authorization",
            format!("Bearer {}", charlie_auth.access_token),
        )
        .header("x-forwarded-for", "203.0.113.183")
        .body(Body::empty())
        .expect("device removal request should build");
    let remove_charlie = app
        .clone()
        .oneshot(remove_charlie)
        .await
        .expect("device removal should execute");
    assert_eq!(remove_charlie.status(), StatusCode::OK);
    let external_proposals = Request::builder()
        .method("GET")
        .uri(format!(
            "/e2ee/groups/{}/proposals?device_id={alice_device_id}",
            provisioned_group.group_id
        ))
        .header(
            "authorization",
            format!("Bearer {}", alice_auth.access_token),
        )
        .header("x-forwarded-for", "203.0.113.181")
        .body(Body::empty())
        .expect("external proposal mailbox request should build");
    let external_proposals = app
        .clone()
        .oneshot(external_proposals)
        .await
        .expect("external proposal mailbox should execute");
    assert_eq!(external_proposals.status(), StatusCode::OK);
    let external_proposals: E2eeProposalMailboxResponse = parse_json(external_proposals).await;
    assert_eq!(external_proposals.proposals.len(), 1);
    assert_eq!(external_proposals.proposals[0].proposer_device_id, None);
    assert_eq!(
        external_proposals.proposals[0].external_sender_index,
        Some(0)
    );
    assert!(external_proposals.proposals[0]
        .reconciliation_deadline_unix
        .is_some());
    let blocked_message = send_json(
        &app,
        "POST",
        &format!("/e2ee/groups/{}/messages", provisioned_group.group_id),
        Some(&alice_auth.access_token),
        "203.0.113.181",
        &PostMessageRequest {
            epoch: 1,
            suite_id: 3,
            sender_device_id: alice_device_id.to_string(),
            retention_secs: None,
            message_blob: vec![0xEE; 512],
        },
    )
    .await;
    assert_eq!(blocked_message.status(), StatusCode::CONFLICT);
    assert_eq!(
        parse_json::<serde_json::Value>(blocked_message).await["error"],
        "e2ee_membership_reconciliation_pending"
    );
    let eviction = send_json(
        &app,
        "POST",
        &format!("/e2ee/groups/{}/commits", provisioned_group.group_id),
        Some(&alice_auth.access_token),
        "203.0.113.181",
        &PostCommitRequest {
            epoch: 2,
            prior_epoch: 1,
            committer_device_id: alice_device_id.to_string(),
            commit_blob: vec![0xEF; 256],
            welcome_blob: None,
            welcome_device_id: None,
            group_info_blob: Some(vec![0xF0; 128]),
            membership_change: Some(MlsMembershipChange::Remove {
                leaves: vec![MlsLeafRouting {
                    leaf_index: 2,
                    user_id: charlie_user_id.to_string(),
                    device_id: charlie_device_id.to_string(),
                }],
            }),
        },
    )
    .await;
    assert_eq!(eviction.status(), StatusCode::OK);
    let completed_reconciliations: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM e2ee_membership_reconciliations
         WHERE group_id = $1 AND completed_epoch = 2",
    )
    .bind(&provisioned_group.group_id)
    .fetch_one(&audit_pool)
    .await
    .expect("completed reconciliation should be queryable");
    assert_eq!(completed_reconciliations, 1);
    let resumed_message = send_json(
        &app,
        "POST",
        &format!("/e2ee/groups/{}/messages", provisioned_group.group_id),
        Some(&alice_auth.access_token),
        "203.0.113.181",
        &PostMessageRequest {
            epoch: 2,
            suite_id: 3,
            sender_device_id: alice_device_id.to_string(),
            retention_secs: None,
            message_blob: vec![0xF1; 512],
        },
    )
    .await;
    assert_eq!(resumed_message.status(), StatusCode::OK);

    let attachment_plaintext = b"private attachment contents";
    let (descriptor, encrypted) =
        encrypt_attachment("private.txt", attachment_plaintext).expect("attachment should encrypt");
    let attachment_id = descriptor.attachment_id.to_string();
    let attachment_uri =
        format!("/e2ee/groups/{group_id}/attachments/{attachment_id}?device_id={alice_device_id}");
    let upload = send_opaque_attachment(
        &app,
        "PUT",
        &attachment_uri,
        &alice_auth.access_token,
        "203.0.113.181",
        encrypted.ciphertext.clone(),
    )
    .await;
    assert_eq!(upload.status(), StatusCode::OK);
    let uploaded: PutE2eeAttachmentResponse = parse_json(upload).await;
    assert_eq!(uploaded.attachment_id, attachment_id);
    assert_eq!(uploaded.ciphertext_bytes, 65_536);

    let retry = send_opaque_attachment(
        &app,
        "PUT",
        &attachment_uri,
        &alice_auth.access_token,
        "203.0.113.181",
        encrypted.ciphertext.clone(),
    )
    .await;
    assert_eq!(retry.status(), StatusCode::OK);
    assert_eq!(
        parse_json::<PutE2eeAttachmentResponse>(retry).await,
        uploaded
    );
    let conflicting = send_opaque_attachment(
        &app,
        "PUT",
        &attachment_uri,
        &alice_auth.access_token,
        "203.0.113.181",
        vec![0xA7; 65_536],
    )
    .await;
    assert_eq!(conflicting.status(), StatusCode::CONFLICT);
    assert_eq!(
        parse_json::<serde_json::Value>(conflicting).await["error"],
        "e2ee_attachment_conflict"
    );

    let stored_attachment: (Vec<u8>, i32, String, String) = sqlx::query_as(
        "SELECT ciphertext_blob, octet_length(ciphertext_blob), owner_user_id, group_id
         FROM e2ee_attachment_blobs WHERE attachment_id = $1",
    )
    .bind(&attachment_id)
    .fetch_one(&audit_pool)
    .await
    .expect("opaque attachment should be stored");
    assert_eq!(stored_attachment.0, encrypted.ciphertext);
    assert_eq!(stored_attachment.1, 65_536);
    assert_eq!(stored_attachment.2, alice_user_id.to_string());
    assert_eq!(stored_attachment.3, group_id.to_string());
    let private_metadata_columns: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM information_schema.columns
         WHERE table_name = 'e2ee_attachment_blobs'
           AND column_name = ANY($1::TEXT[])",
    )
    .bind(vec!["filename", "mime_type", "content_hash", "content_key"])
    .fetch_one(&audit_pool)
    .await
    .expect("attachment schema should be inspectable");
    assert_eq!(private_metadata_columns, 0);

    let removed_member_download = Request::builder()
        .method("GET")
        .uri(format!(
            "/e2ee/groups/{group_id}/attachments/{attachment_id}?device_id={charlie_device_id}"
        ))
        .header(
            "authorization",
            format!("Bearer {}", charlie_auth.access_token),
        )
        .header("x-forwarded-for", "203.0.113.183")
        .body(Body::empty())
        .expect("removed member download should build");
    let removed_member_download = app
        .clone()
        .oneshot(removed_member_download)
        .await
        .expect("removed member download should execute");
    assert_eq!(removed_member_download.status(), StatusCode::NOT_FOUND);

    let pending_devices: Vec<String> = sqlx::query_scalar(
        "SELECT device_id FROM e2ee_attachment_deliveries
         WHERE attachment_id = $1 AND acked_at_unix IS NULL ORDER BY device_id",
    )
    .bind(&attachment_id)
    .fetch_all(&audit_pool)
    .await
    .expect("pending attachment devices should be queryable");
    assert_eq!(pending_devices, vec![bob_device_id.to_string()]);

    let non_leaf_download = Request::builder()
        .method("GET")
        .uri(format!(
            "/e2ee/groups/{group_id}/attachments/{attachment_id}?device_id={bob_second_device_id}"
        ))
        .header("authorization", format!("Bearer {}", bob_auth.access_token))
        .header("x-forwarded-for", "203.0.113.182")
        .body(Body::empty())
        .expect("non-leaf attachment download should build");
    let non_leaf_download = app
        .clone()
        .oneshot(non_leaf_download)
        .await
        .expect("non-leaf attachment download should execute");
    assert_eq!(non_leaf_download.status(), StatusCode::NOT_FOUND);

    let download = Request::builder()
        .method("GET")
        .uri(format!(
            "/e2ee/groups/{group_id}/attachments/{attachment_id}?device_id={bob_device_id}"
        ))
        .header("authorization", format!("Bearer {}", bob_auth.access_token))
        .header("x-forwarded-for", "203.0.113.182")
        .body(Body::empty())
        .expect("attachment download should build");
    let download = app
        .clone()
        .oneshot(download)
        .await
        .expect("attachment download should execute");
    assert_eq!(download.status(), StatusCode::OK);
    assert_eq!(
        download.headers()["content-type"],
        "application/octet-stream"
    );
    assert_eq!(download.headers()["cache-control"], "private, no-store");
    let downloaded_bytes = axum::body::to_bytes(download.into_body(), 65_536)
        .await
        .expect("attachment body should be bounded and readable")
        .to_vec();
    let downloaded = EncryptedAttachment {
        attachment_id: descriptor.attachment_id,
        ciphertext: downloaded_bytes,
    };
    let content = decrypt_attachment(&descriptor, &downloaded)
        .expect("recipient should authenticate and decrypt attachment");
    assert_eq!(content.bytes.as_slice(), attachment_plaintext);

    let ack = send_json(
        &app,
        "POST",
        &format!("/e2ee/groups/{group_id}/attachments/ack"),
        Some(&bob_auth.access_token),
        "203.0.113.182",
        &AckE2eeAttachmentsRequest {
            device_id: bob_device_id.to_string(),
            attachment_ids: vec![attachment_id.clone()],
        },
    )
    .await;
    assert_eq!(ack.status(), StatusCode::OK);
    let ack: AckE2eeAttachmentsResponse = parse_json(ack).await;
    assert_eq!(ack.acknowledged_count, 1);
    assert_eq!(ack.deleted_count, 1);
    let deleted_after_all_device_ack: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM e2ee_attachment_blobs WHERE attachment_id = $1")
            .bind(&attachment_id)
            .fetch_one(&audit_pool)
            .await
            .expect("attachment deletion should be queryable");
    assert_eq!(deleted_after_all_device_ack, 0);

    let (_, expiring) = encrypt_attachment("expiring.bin", b"expires from the mailbox")
        .expect("expiring attachment should encrypt");
    let expiring_id = expiring.attachment_id.to_string();
    let expiring_upload = send_opaque_attachment(
        &app,
        "PUT",
        &format!("/e2ee/groups/{group_id}/attachments/{expiring_id}?device_id={alice_device_id}"),
        &alice_auth.access_token,
        "203.0.113.181",
        expiring.ciphertext,
    )
    .await;
    assert_eq!(expiring_upload.status(), StatusCode::OK);
    sqlx::query(
        "UPDATE e2ee_attachment_blobs
         SET created_at_unix = 0, expires_at_unix = 1 WHERE attachment_id = $1",
    )
    .bind(&expiring_id)
    .execute(&audit_pool)
    .await
    .expect("attachment expiry should be adjustable for GC coverage");
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let remaining: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM e2ee_attachment_blobs WHERE attachment_id = $1",
            )
            .bind(&expiring_id)
            .fetch_one(&audit_pool)
            .await
            .expect("expired attachment deletion should be queryable");
            if remaining == 0 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("background mailbox GC should delete expired attachments");
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
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;

    let delivery_service_key_path =
        env::temp_dir().join(format!("filament-e2ee-integration-ds-{}", Ulid::new()));
    std::fs::write(&delivery_service_key_path, [0x5A_u8; 32])
        .expect("delivery service test key should write");
    #[cfg(unix)]
    std::fs::set_permissions(
        &delivery_service_key_path,
        std::fs::Permissions::from_mode(0o600),
    )
    .expect("delivery service test key permissions should be private");
    let config = AppConfig {
        max_body_bytes: 512 * 1024,
        request_timeout: Duration::from_secs(5),
        rate_limit_requests_per_minute: 500,
        auth_route_requests_per_minute: 200,
        e2ee_device_publish_per_minute: device_publish_per_minute,
        e2ee_keypackage_claim_per_minute: keypackage_claim_per_minute,
        e2ee_mailbox_gc_interval: Duration::from_secs(1),
        e2ee_delivery_service_key_file: Some(delivery_service_key_path.clone()),
        database_url: Some(database_url),
        ..AppConfig::default()
    };
    let router = build_router_with_db_bootstrap(&config)
        .await
        .expect("router should build");
    std::fs::remove_file(delivery_service_key_path)
        .expect("delivery service test key should be removed");
    router
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

async fn send_opaque_attachment(
    app: &axum::Router,
    method: &str,
    uri: &str,
    token: &str,
    ip: &str,
    ciphertext: Vec<u8>,
) -> Response {
    let request = Request::builder()
        .method(method)
        .uri(uri)
        .header("authorization", format!("Bearer {token}"))
        .header("content-type", "application/octet-stream")
        .header("x-forwarded-for", ip)
        .body(Body::from(ciphertext))
        .expect("opaque attachment request should build");
    app.clone()
        .oneshot(request)
        .await
        .expect("opaque attachment request should execute")
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
#[allow(clippy::too_many_lines)]
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

    let final_remove_request = Request::builder()
        .method("DELETE")
        .uri(format!("/e2ee/devices/{paired_device_id}"))
        .header("authorization", format!("Bearer {}", auth.access_token))
        .header("x-forwarded-for", ip)
        .body(Body::empty())
        .expect("final-device remove request should build");
    let final_remove_response = app
        .clone()
        .oneshot(final_remove_request)
        .await
        .expect("final-device remove should execute");
    assert_eq!(final_remove_response.status(), StatusCode::OK);
    let final_device_removed = next_gateway_event(&mut gateway, "device_list_update").await;
    assert_eq!(final_device_removed["d"]["user_id"], user_id.to_string());
    assert_eq!(final_device_removed["d"]["device_count"], 0);

    let list_after_final_remove = Request::builder()
        .method("GET")
        .uri(format!("/e2ee/users/{user_id}/devices"))
        .header("authorization", format!("Bearer {}", auth.access_token))
        .header("x-forwarded-for", ip)
        .body(Body::empty())
        .expect("final empty-list request should build");
    let listed_after_final_remove = app
        .clone()
        .oneshot(list_after_final_remove)
        .await
        .expect("final empty list should execute");
    assert_eq!(listed_after_final_remove.status(), StatusCode::OK);
    let listed_after_final_remove: DeviceListResponse = parse_json(listed_after_final_remove).await;
    assert!(listed_after_final_remove.devices.is_empty());

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
#[allow(clippy::too_many_lines)]
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
    assert_eq!(replay.status(), StatusCode::OK);
    let replay: RotateRootIdentityResponse = parse_json(replay).await;
    assert_eq!(replay, rotated);

    let mut conflicting_replay = rotation_request.clone();
    conflicting_replay.new_device_signature_pubkey[0] ^= 1;
    let conflicting_replay = send_json(
        &app,
        "POST",
        "/e2ee/identity/rotate",
        Some(&auth.access_token),
        ip,
        &conflicting_replay,
    )
    .await;
    assert_eq!(conflicting_replay.status(), StatusCode::FORBIDDEN);

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
