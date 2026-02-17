use super::*;

#[tokio::test]
async fn gateway_broadcasts_message_to_subscribed_connection() {
    let state = AppState::new(&AppConfig::default()).unwrap();
    let user_id = UserId::new();
    let username = Username::try_from(String::from("alice_1")).unwrap();
    state.users.write().await.insert(
        username.as_str().to_owned(),
        UserRecord {
            id: user_id,
            username: username.clone(),
            about_markdown: String::new(),
            avatar: None,
            avatar_version: 0,
            password_hash: hash_password("super-secure-password").unwrap(),
            failed_logins: 0,
            locked_until_unix: None,
        },
    );
    state
        .user_ids
        .write()
        .await
        .insert(user_id.to_string(), username.as_str().to_owned());

    let guild_id = String::from("g");
    let channel_id = String::from("c");
    let mut guild = GuildRecord {
        name: String::from("Gateway Test"),
        visibility: GuildVisibility::Private,
        created_by_user_id: user_id,
        members: HashMap::new(),
        banned_members: std::collections::HashSet::new(),
        channels: HashMap::new(),
    };
    guild.members.insert(user_id, Role::Owner);
    guild.channels.insert(
        channel_id.clone(),
        ChannelRecord {
            name: String::from("gateway-room"),
            kind: ChannelKind::Text,
            messages: Vec::new(),
            role_overrides: HashMap::new(),
        },
    );
    state
        .membership_store
        .guilds()
        .write()
        .await
        .insert(guild_id.clone(), guild);

    let (tx, mut rx) = mpsc::channel::<String>(4);
    add_subscription(&state, Uuid::new_v4(), channel_key("g", "c"), tx).await;

    let auth = AuthContext {
        user_id,
        username: username.as_str().to_owned(),
    };
    let result = create_message_internal(
        &state,
        &auth,
        &guild_id,
        &channel_id,
        String::from("hello"),
        Vec::new(),
    )
    .await
    .unwrap();
    assert_eq!(result.content, "hello");

    let event = rx.recv().await.unwrap();
    let value: Value = serde_json::from_str(&event).unwrap();
    assert_eq!(value["t"], "message_create");
    assert_eq!(value["d"]["content"], "hello");
}

#[tokio::test]
async fn channel_broadcast_targets_only_matching_subscription_key() {
    let state = AppState::new(&AppConfig::default()).unwrap();
    let (tx_target, mut rx_target) = mpsc::channel::<String>(2);
    let (tx_other, mut rx_other) = mpsc::channel::<String>(2);
    add_subscription(
        &state,
        Uuid::new_v4(),
        channel_key("g", "c-target"),
        tx_target,
    )
    .await;
    add_subscription(
        &state,
        Uuid::new_v4(),
        channel_key("g", "c-other"),
        tx_other,
    )
    .await;

    let event = gateway_events::subscribed("g", "c-target");
    broadcast_channel_event(&state, &channel_key("g", "c-target"), &event).await;

    let target_payload = rx_target.recv().await.expect("target payload");
    let target_value: Value = serde_json::from_str(&target_payload).unwrap();
    assert_eq!(target_value["d"]["channel_id"], "c-target");
    let other_result = tokio::time::timeout(Duration::from_millis(25), rx_other.recv()).await;
    assert!(
        other_result.is_err(),
        "unexpected event on non-target channel"
    );
}

#[tokio::test]
async fn guild_broadcast_delivers_once_per_connection_and_skips_other_guilds() {
    let state = AppState::new(&AppConfig::default()).unwrap();
    let connection_id = Uuid::new_v4();
    let (tx_target, mut rx_target) = mpsc::channel::<String>(4);
    let (tx_other, mut rx_other) = mpsc::channel::<String>(4);
    add_subscription(
        &state,
        connection_id,
        channel_key("g-main", "c-1"),
        tx_target.clone(),
    )
    .await;
    add_subscription(
        &state,
        connection_id,
        channel_key("g-main", "c-2"),
        tx_target,
    )
    .await;
    add_subscription(
        &state,
        Uuid::new_v4(),
        channel_key("g-other", "c-1"),
        tx_other,
    )
    .await;

    let event = gateway_events::presence_update("g-main", UserId::new(), "online");
    broadcast_guild_event(&state, "g-main", &event).await;

    let first = rx_target.recv().await.expect("guild event");
    let value: Value = serde_json::from_str(&first).unwrap();
    assert_eq!(value["d"]["guild_id"], "g-main");
    let duplicate = tokio::time::timeout(Duration::from_millis(25), rx_target.recv()).await;
    assert!(
        duplicate.is_err(),
        "duplicate event delivered for same connection"
    );
    let other = tokio::time::timeout(Duration::from_millis(25), rx_other.recv()).await;
    assert!(other.is_err(), "event delivered to unrelated guild");
}

#[tokio::test]
async fn user_broadcast_targets_only_requested_authenticated_user() {
    let state = AppState::new(&AppConfig::default()).unwrap();
    let user_a = UserId::new();
    let user_b = UserId::new();
    let connection_a1 = Uuid::new_v4();
    let connection_a2 = Uuid::new_v4();
    let connection_b = Uuid::new_v4();
    let (tx_a1, mut rx_a1) = mpsc::channel::<String>(2);
    let (tx_a2, mut rx_a2) = mpsc::channel::<String>(2);
    let (tx_b, mut rx_b) = mpsc::channel::<String>(2);

    state
        .realtime_registry
        .connection_senders()
        .write()
        .await
        .insert(connection_a1, tx_a1);
    state
        .realtime_registry
        .connection_senders()
        .write()
        .await
        .insert(connection_a2, tx_a2);
    state
        .realtime_registry
        .connection_senders()
        .write()
        .await
        .insert(connection_b, tx_b);
    state.realtime_registry.connection_presence().write().await.insert(
        connection_a1,
        ConnectionPresence {
            user_id: user_a,
            guild_ids: std::collections::HashSet::new(),
        },
    );
    state.realtime_registry.connection_presence().write().await.insert(
        connection_a2,
        ConnectionPresence {
            user_id: user_a,
            guild_ids: std::collections::HashSet::new(),
        },
    );
    state.realtime_registry.connection_presence().write().await.insert(
        connection_b,
        ConnectionPresence {
            user_id: user_b,
            guild_ids: std::collections::HashSet::new(),
        },
    );

    let event = gateway_events::ready(user_a);
    broadcast_user_event(&state, user_a, &event).await;

    let payload_a1 = rx_a1.recv().await.expect("first session");
    let payload_a2 = rx_a2.recv().await.expect("second session");
    let value_a1: Value = serde_json::from_str(&payload_a1).unwrap();
    let value_a2: Value = serde_json::from_str(&payload_a2).unwrap();
    assert_eq!(value_a1["d"]["user_id"], user_a.to_string());
    assert_eq!(value_a2["d"]["user_id"], user_a.to_string());
    let other = tokio::time::timeout(Duration::from_millis(25), rx_b.recv()).await;
    assert!(other.is_err(), "user-scoped event leaked to another user");
}

#[tokio::test]
async fn slow_consumer_signal_is_sent_when_outbound_queue_is_full() {
    let state = AppState::new(&AppConfig {
        gateway_outbound_queue: 1,
        ..AppConfig::default()
    })
    .unwrap();

    let connection_id = Uuid::new_v4();
    let (tx, _rx) = mpsc::channel::<String>(1);
    let (control_tx, control_rx) = watch::channel(ConnectionControl::Open);
    state
        .realtime_registry
        .connection_controls()
        .write()
        .await
        .insert(connection_id, control_tx);
    state
        .realtime_registry
        .subscriptions()
        .write()
        .await
        .entry(channel_key("g", "c"))
        .or_default()
        .insert(connection_id, tx.clone());

    tx.try_send(String::from("first")).unwrap();
    let event = gateway_events::subscribed("g", "c");
    broadcast_channel_event(&state, &channel_key("g", "c"), &event).await;

    assert_eq!(*control_rx.borrow(), ConnectionControl::Close);
}
