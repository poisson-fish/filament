use super::*;
use std::time::Instant;

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
            banner: None,
            banner_version: 0,
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
        default_join_role_id: None,
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

    let event =
        gateway_events::try_subscribed("g", "c-target").expect("subscribed event should serialize");
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
    let other_connection_id = Uuid::new_v4();
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
        tx_target.clone(),
    )
    .await;
    add_subscription(
        &state,
        other_connection_id,
        channel_key("g-other", "c-1"),
        tx_other.clone(),
    )
    .await;
    state
        .realtime_registry
        .connection_senders()
        .write()
        .await
        .insert(connection_id, tx_target);
    state
        .realtime_registry
        .connection_senders()
        .write()
        .await
        .insert(other_connection_id, tx_other);

    let event = gateway_events::try_presence_update("g-main", UserId::new(), "online")
        .expect("presence_update should serialize");
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
    state
        .realtime_registry
        .user_connections()
        .write()
        .await
        .insert(
            user_a,
            std::collections::HashSet::from([connection_a1, connection_a2]),
        );
    state
        .realtime_registry
        .user_connections()
        .write()
        .await
        .insert(user_b, std::collections::HashSet::from([connection_b]));

    let event = gateway_events::try_ready(user_a).expect("ready event should serialize");
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
    let event =
        gateway_events::try_subscribed("g", "c").expect("subscribed event should serialize");
    broadcast_channel_event(&state, &channel_key("g", "c"), &event).await;

    assert_eq!(*control_rx.borrow(), ConnectionControl::Close);
}

#[tokio::test]
async fn slow_consumer_signal_is_sent_for_guild_fanout_when_outbound_queue_is_full() {
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
    add_subscription(&state, connection_id, channel_key("g", "c"), tx.clone()).await;
    state
        .realtime_registry
        .connection_senders()
        .write()
        .await
        .insert(connection_id, tx.clone());

    tx.try_send(String::from("first")).unwrap();
    let event = gateway_events::try_presence_update("g", UserId::new(), "online")
        .expect("presence_update should serialize");
    broadcast_guild_event(&state, "g", &event).await;

    assert_eq!(*control_rx.borrow(), ConnectionControl::Close);
}

#[tokio::test]
async fn slow_consumer_signal_is_sent_for_user_fanout_when_outbound_queue_is_full() {
    let state = AppState::new(&AppConfig {
        gateway_outbound_queue: 1,
        ..AppConfig::default()
    })
    .unwrap();

    let user_id = UserId::new();
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
        .connection_senders()
        .write()
        .await
        .insert(connection_id, tx.clone());
    state
        .realtime_registry
        .user_connections()
        .write()
        .await
        .insert(user_id, std::collections::HashSet::from([connection_id]));

    tx.try_send(String::from("first")).unwrap();
    let event = gateway_events::try_ready(user_id).expect("ready event should serialize");
    broadcast_user_event(&state, user_id, &event).await;

    assert_eq!(*control_rx.borrow(), ConnectionControl::Close);
}

fn bench_snapshot_line(
    path: &str,
    listeners: usize,
    iterations: usize,
    elapsed: std::time::Duration,
) {
    let listener_count = u128::try_from(listeners).expect("listener count should fit u128");
    let iteration_count = u128::try_from(iterations).expect("iteration count should fit u128");
    let total_dispatches = listener_count * iteration_count;
    let total_nanos = elapsed.as_nanos();
    let ns_whole = total_nanos / total_dispatches;
    let ns_tenths = ((total_nanos % total_dispatches) * 10) / total_dispatches;
    let total_ms_whole = elapsed.as_millis();
    let total_ms_thousandths = elapsed.as_micros() % 1000;
    println!(
        "fanout_snapshot path={path} listeners={listeners} iterations={iterations} total_ms={total_ms_whole}.{total_ms_thousandths:03} ns_per_dispatch={ns_whole}.{ns_tenths}",
    );
}

#[tokio::test]
#[ignore = "benchmark snapshot"]
async fn fanout_benchmark_snapshot_channel_hot_path() {
    let listener_count = 512usize;
    let iterations = 128usize;
    let queue_capacity = iterations + 4;
    let state = AppState::new(&AppConfig {
        gateway_outbound_queue: queue_capacity,
        ..AppConfig::default()
    })
    .expect("state should build");

    let mut receivers = Vec::with_capacity(listener_count);
    for _ in 0..listener_count {
        let connection_id = Uuid::new_v4();
        let (tx, rx) = mpsc::channel::<String>(queue_capacity);
        add_subscription(&state, connection_id, channel_key("g-bench", "c-bench"), tx).await;
        receivers.push(rx);
    }

    let event = gateway_events::try_subscribed("g-bench", "c-bench")
        .expect("subscribed event should serialize");

    let started = Instant::now();
    for _ in 0..iterations {
        broadcast_channel_event(&state, &channel_key("g-bench", "c-bench"), &event).await;
    }
    let elapsed = started.elapsed();

    bench_snapshot_line("channel", listener_count, iterations, elapsed);
    assert_eq!(receivers.len(), listener_count);
}

#[tokio::test]
#[ignore = "benchmark snapshot"]
async fn fanout_benchmark_snapshot_guild_hot_path() {
    let listener_count = 512usize;
    let iterations = 128usize;
    let queue_capacity = iterations + 4;
    let state = AppState::new(&AppConfig {
        gateway_outbound_queue: queue_capacity,
        ..AppConfig::default()
    })
    .expect("state should build");

    let mut receivers = Vec::with_capacity(listener_count);
    for index in 0..listener_count {
        let connection_id = Uuid::new_v4();
        let (tx, rx) = mpsc::channel::<String>(queue_capacity);
        add_subscription(
            &state,
            connection_id,
            channel_key("g-bench", &format!("c-bench-{index}")),
            tx.clone(),
        )
        .await;
        state
            .realtime_registry
            .connection_senders()
            .write()
            .await
            .insert(connection_id, tx);
        receivers.push(rx);
    }

    let event = gateway_events::try_presence_update("g-bench", UserId::new(), "online")
        .expect("presence_update should serialize");

    let started = Instant::now();
    for _ in 0..iterations {
        broadcast_guild_event(&state, "g-bench", &event).await;
    }
    let elapsed = started.elapsed();

    bench_snapshot_line("guild", listener_count, iterations, elapsed);
    assert_eq!(receivers.len(), listener_count);
}

#[tokio::test]
#[ignore = "benchmark snapshot"]
async fn fanout_benchmark_snapshot_user_hot_path() {
    let listener_count = 512usize;
    let iterations = 128usize;
    let queue_capacity = iterations + 4;
    let state = AppState::new(&AppConfig {
        gateway_outbound_queue: queue_capacity,
        ..AppConfig::default()
    })
    .expect("state should build");

    let user_id = UserId::new();
    let mut connection_ids = std::collections::HashSet::with_capacity(listener_count);
    let mut receivers = Vec::with_capacity(listener_count);
    {
        let mut senders = state.realtime_registry.connection_senders().write().await;
        for _ in 0..listener_count {
            let connection_id = Uuid::new_v4();
            let (tx, rx) = mpsc::channel::<String>(queue_capacity);
            senders.insert(connection_id, tx);
            connection_ids.insert(connection_id);
            receivers.push(rx);
        }
    }
    state
        .realtime_registry
        .user_connections()
        .write()
        .await
        .insert(user_id, connection_ids);

    let event = gateway_events::try_ready(user_id).expect("ready event should serialize");

    let started = Instant::now();
    for _ in 0..iterations {
        broadcast_user_event(&state, user_id, &event).await;
    }
    let elapsed = started.elapsed();

    bench_snapshot_line("user", listener_count, iterations, elapsed);
    assert_eq!(receivers.len(), listener_count);
}
