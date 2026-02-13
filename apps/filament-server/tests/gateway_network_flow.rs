use std::{net::SocketAddr, time::Duration};

use axum::{body::Body, extract::connect_info::ConnectInfo, http::Request, http::StatusCode};
use filament_server::{build_router, AppConfig};
use futures_util::{SinkExt, StreamExt};
use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use tokio::net::TcpListener;
use tokio_tungstenite::{
    connect_async,
    tungstenite::{client::IntoClientRequest, protocol::Message},
};
use tower::ServiceExt;

#[derive(Debug, serde::Deserialize)]
struct AuthResponse {
    access_token: String,
}

#[derive(Debug)]
struct ChannelRef {
    guild_id: String,
    channel_id: String,
}

async fn parse_json_body<T: DeserializeOwned>(response: axum::response::Response) -> T {
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("response body should be readable");
    serde_json::from_slice(&body).expect("response body should be valid json")
}

async fn register_and_login_as(app: &axum::Router, username: &str, ip: &str) -> AuthResponse {
    let register = Request::builder()
        .method("POST")
        .uri("/auth/register")
        .header("content-type", "application/json")
        .header("x-forwarded-for", ip)
        .body(Body::from(
            json!({"username":username,"password":"super-secure-password"}).to_string(),
        ))
        .expect("register request should build");
    let register_response = app
        .clone()
        .oneshot(register)
        .await
        .expect("register request should execute");
    assert_eq!(register_response.status(), StatusCode::OK);

    let login = Request::builder()
        .method("POST")
        .uri("/auth/login")
        .header("content-type", "application/json")
        .header("x-forwarded-for", ip)
        .body(Body::from(
            json!({"username":username,"password":"super-secure-password"}).to_string(),
        ))
        .expect("login request should build");
    let login_response = app
        .clone()
        .oneshot(login)
        .await
        .expect("login request should execute");
    assert_eq!(login_response.status(), StatusCode::OK);

    parse_json_body(login_response).await
}

async fn register_and_login(app: &axum::Router, ip: &str) -> AuthResponse {
    register_and_login_as(app, "network_test_user", ip).await
}

async fn create_channel_context(app: &axum::Router, auth: &AuthResponse, ip: &str) -> ChannelRef {
    let create_guild = Request::builder()
        .method("POST")
        .uri("/guilds")
        .header("authorization", format!("Bearer {}", auth.access_token))
        .header("content-type", "application/json")
        .header("x-forwarded-for", ip)
        .body(Body::from(json!({"name":"Network Guild"}).to_string()))
        .expect("create guild request should build");
    let guild_response = app
        .clone()
        .oneshot(create_guild)
        .await
        .expect("create guild request should execute");
    assert_eq!(guild_response.status(), StatusCode::OK);
    let guild_json: Value = parse_json_body(guild_response).await;
    let guild_id = guild_json["guild_id"]
        .as_str()
        .expect("guild id should exist")
        .to_owned();

    let create_channel = Request::builder()
        .method("POST")
        .uri(format!("/guilds/{guild_id}/channels"))
        .header("authorization", format!("Bearer {}", auth.access_token))
        .header("content-type", "application/json")
        .header("x-forwarded-for", ip)
        .body(Body::from(json!({"name":"network-chat"}).to_string()))
        .expect("create channel request should build");
    let channel_response = app
        .clone()
        .oneshot(create_channel)
        .await
        .expect("create channel request should execute");
    assert_eq!(channel_response.status(), StatusCode::OK);
    let channel_json: Value = parse_json_body(channel_response).await;
    let channel_id = channel_json["channel_id"]
        .as_str()
        .expect("channel id should exist")
        .to_owned();

    ChannelRef {
        guild_id,
        channel_id,
    }
}

async fn user_id_from_me(app: &axum::Router, auth: &AuthResponse, ip: &str) -> String {
    let me = Request::builder()
        .method("GET")
        .uri("/auth/me")
        .header("authorization", format!("Bearer {}", auth.access_token))
        .header("x-forwarded-for", ip)
        .body(Body::empty())
        .expect("me request should build");
    let me_response = app
        .clone()
        .oneshot(me)
        .await
        .expect("me request should execute");
    assert_eq!(me_response.status(), StatusCode::OK);
    let me_json: Value = parse_json_body(me_response).await;
    me_json["user_id"]
        .as_str()
        .expect("user id should exist")
        .to_owned()
}

async fn add_member(
    app: &axum::Router,
    actor_access_token: &str,
    ip: &str,
    guild_id: &str,
    target_user_id: &str,
) {
    let add_member = Request::builder()
        .method("POST")
        .uri(format!("/guilds/{guild_id}/members/{target_user_id}"))
        .header("authorization", format!("Bearer {actor_access_token}"))
        .header("x-forwarded-for", ip)
        .body(Body::empty())
        .expect("add member request should build");
    let add_member_response = app
        .clone()
        .oneshot(add_member)
        .await
        .expect("add member request should execute");
    assert_eq!(add_member_response.status(), StatusCode::OK);
}

fn test_app() -> axum::Router {
    build_router(&AppConfig {
        max_body_bytes: 1024 * 32,
        request_timeout: Duration::from_secs(2),
        rate_limit_requests_per_minute: 200,
        auth_route_requests_per_minute: 200,
        gateway_ingress_events_per_window: 20,
        gateway_ingress_window: Duration::from_secs(10),
        gateway_outbound_queue: 256,
        max_gateway_event_bytes: AppConfig::default().max_gateway_event_bytes,
        ..AppConfig::default()
    })
    .expect("router should build")
}

async fn next_text_event(
    socket: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
) -> Value {
    loop {
        let event = socket
            .next()
            .await
            .expect("event should be emitted")
            .expect("event should decode");
        if event.is_ping() || event.is_pong() {
            continue;
        }
        let text = event.into_text().expect("event should be text");
        return serde_json::from_str(&text).expect("event should be valid json");
    }
}

async fn next_event_of_type(
    socket: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
    event_type: &str,
) -> Value {
    for _ in 0..32 {
        let event = tokio::time::timeout(Duration::from_secs(1), next_text_event(socket))
            .await
            .unwrap_or_else(|_| panic!("timed out waiting for event type {event_type}"));
        if event["t"] == event_type {
            return event;
        }
    }
    panic!("expected event type {event_type}");
}

async fn maybe_next_event_of_type(
    socket: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
    event_type: &str,
    timeout: Duration,
) -> Option<Value> {
    let result = tokio::time::timeout(timeout, async {
        for _ in 0..32 {
            let Some(raw) = socket.next().await else {
                return None;
            };
            let Ok(message) = raw else {
                return None;
            };
            let Ok(text) = message.into_text() else {
                continue;
            };
            let Ok(event) = serde_json::from_str::<Value>(&text) else {
                continue;
            };
            if event["t"] == event_type {
                return Some(event);
            }
        }
        None
    })
    .await;
    result.unwrap_or_default()
}

async fn create_voice_channel(
    app: &axum::Router,
    auth: &AuthResponse,
    ip: &str,
    guild_id: &str,
) -> String {
    let create_channel = Request::builder()
        .method("POST")
        .uri(format!("/guilds/{guild_id}/channels"))
        .header("authorization", format!("Bearer {}", auth.access_token))
        .header("content-type", "application/json")
        .header("x-forwarded-for", ip)
        .body(Body::from(
            json!({"name":"voice-sync","kind":"voice"}).to_string(),
        ))
        .expect("create voice channel request should build");
    let response = app
        .clone()
        .oneshot(create_channel)
        .await
        .expect("create voice channel request should execute");
    assert_eq!(response.status(), StatusCode::OK);
    let payload: Value = parse_json_body(response).await;
    payload["channel_id"]
        .as_str()
        .expect("voice channel id should exist")
        .to_owned()
}

async fn metrics_text(app: &axum::Router) -> String {
    let metrics_request = Request::builder()
        .method("GET")
        .uri("/metrics")
        .header("x-forwarded-for", "198.51.100.250")
        .body(Body::empty())
        .expect("metrics request should build");
    let metrics_response = app
        .clone()
        .oneshot(metrics_request)
        .await
        .expect("metrics request should execute");
    assert_eq!(metrics_response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(metrics_response.into_body(), usize::MAX)
        .await
        .expect("metrics body should be readable");
    String::from_utf8(body.to_vec()).expect("metrics should be utf-8")
}

fn contains_ip_field(value: &Value) -> bool {
    match value {
        Value::Object(map) => map.iter().any(|(key, value)| {
            key == "ip"
                || key == "ip_cidr"
                || key == "ip_network"
                || key == "source_ip"
                || key == "address"
                || contains_ip_field(value)
        }),
        Value::Array(values) => values.iter().any(contains_ip_field),
        _ => false,
    }
}

async fn subscribe_to_channel(
    socket: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
    channel: &ChannelRef,
) {
    let subscribe = json!({
        "v": 1,
        "t": "subscribe",
        "d": {
            "guild_id": channel.guild_id,
            "channel_id": channel.channel_id
        }
    });
    socket
        .send(Message::Text(subscribe.to_string()))
        .await
        .expect("subscribe event should send");
    let subscribed_json = next_event_of_type(socket, "subscribed").await;
    assert_eq!(subscribed_json["t"], "subscribed");
}

#[tokio::test]
async fn websocket_handshake_and_message_flow_work_over_network() {
    let app = test_app();

    let auth = register_and_login(&app, "203.0.113.44").await;
    let channel = create_channel_context(&app, &auth, "203.0.113.44").await;

    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener should bind");
    let addr = listener
        .local_addr()
        .expect("listener addr should be readable");
    let server = tokio::spawn(async move {
        axum::serve(listener, app)
            .await
            .expect("server should run without errors");
    });

    let ws_url = format!("ws://{addr}/gateway/ws?access_token={}", auth.access_token);
    let mut ws_request = ws_url
        .into_client_request()
        .expect("websocket request should build");
    ws_request.headers_mut().insert(
        "x-forwarded-for",
        http::HeaderValue::from_static("203.0.113.44"),
    );

    let (mut socket, _response) = connect_async(ws_request)
        .await
        .expect("websocket handshake should succeed");

    let ready = socket
        .next()
        .await
        .expect("ready event should be emitted")
        .expect("ready event should decode");
    let ready_text = ready.into_text().expect("ready event should be text");
    let ready_json: Value = serde_json::from_str(&ready_text).expect("ready event should be json");
    assert_eq!(ready_json["t"], "ready");

    subscribe_to_channel(&mut socket, &channel).await;

    let message_create = json!({
        "v": 1,
        "t": "message_create",
        "d": {
            "guild_id": channel.guild_id,
            "channel_id": channel.channel_id,
            "content": "hello over network"
        }
    });
    socket
        .send(Message::Text(message_create.to_string()))
        .await
        .expect("message create event should send");

    let broadcast_json = next_event_of_type(&mut socket, "message_create").await;
    assert_eq!(broadcast_json["t"], "message_create");
    assert_eq!(broadcast_json["d"]["content"], "hello over network");

    socket
        .close(None)
        .await
        .expect("socket close should succeed");
    server.abort();
}

#[tokio::test]
async fn gateway_ingress_rejections_and_unknown_events_are_counted_in_metrics() {
    let app = test_app();
    let auth = register_and_login(&app, "198.51.100.31").await;
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener should bind");
    let addr = listener.local_addr().expect("listener should expose addr");
    let app_clone = app.clone();
    tokio::spawn(async move {
        axum::serve(
            listener,
            app_clone.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
        .expect("server should run");
    });

    let ws_url = format!("ws://{addr}/gateway/ws?access_token={}", auth.access_token);

    let mut malformed_request = ws_url
        .as_str()
        .into_client_request()
        .expect("malformed ws request should build");
    malformed_request.headers_mut().insert(
        "x-forwarded-for",
        "198.51.100.31".parse().expect("valid ip header"),
    );
    let (mut malformed_socket, _) = connect_async(malformed_request)
        .await
        .expect("ws should connect");
    let _ = next_event_of_type(&mut malformed_socket, "ready").await;
    malformed_socket
        .send(Message::Text(String::from("not-json").into()))
        .await
        .expect("invalid envelope should send");
    let _ = tokio::time::timeout(Duration::from_secs(1), malformed_socket.next()).await;

    let mut unknown_request = ws_url
        .as_str()
        .into_client_request()
        .expect("unknown ws request should build");
    unknown_request.headers_mut().insert(
        "x-forwarded-for",
        "198.51.100.31".parse().expect("valid ip header"),
    );
    let (mut unknown_socket, _) = connect_async(unknown_request)
        .await
        .expect("ws should connect");
    let _ = next_event_of_type(&mut unknown_socket, "ready").await;
    unknown_socket
        .send(Message::Text(
            json!({
                "v": 1,
                "t": "unknown_ingress_event",
                "d": {}
            })
            .to_string()
            .into(),
        ))
        .await
        .expect("unknown event should send");
    let _ = tokio::time::timeout(Duration::from_secs(1), unknown_socket.next()).await;

    let metrics = metrics_text(&app).await;
    assert!(metrics.contains("filament_gateway_events_unknown_received_total"));
    assert!(metrics.contains("filament_gateway_events_parse_rejected_total"));
    assert!(metrics.contains(
        "filament_gateway_events_unknown_received_total{scope=\"ingress\",event_type=\"unknown_ingress_event\"}",
    ));
    assert!(metrics.contains(
        "filament_gateway_events_parse_rejected_total{scope=\"ingress\",reason=\"invalid_envelope\"}",
    ));
}

#[tokio::test]
async fn websocket_disconnect_does_not_block_rest_message_create() {
    let app = test_app();

    let auth = register_and_login(&app, "203.0.113.55").await;
    let channel = create_channel_context(&app, &auth, "203.0.113.55").await;

    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener should bind");
    let addr = listener
        .local_addr()
        .expect("listener addr should be readable");
    let server_app = app.clone();
    let server = tokio::spawn(async move {
        axum::serve(listener, server_app)
            .await
            .expect("server should run without errors");
    });

    let ws_url = format!("ws://{addr}/gateway/ws?access_token={}", auth.access_token);
    let mut ws_request = ws_url
        .into_client_request()
        .expect("websocket request should build");
    ws_request.headers_mut().insert(
        "x-forwarded-for",
        http::HeaderValue::from_static("203.0.113.55"),
    );
    let (mut socket, _response) = connect_async(ws_request)
        .await
        .expect("websocket handshake should succeed");

    let ready_json = next_text_event(&mut socket).await;
    assert_eq!(ready_json["t"], "ready");

    subscribe_to_channel(&mut socket, &channel).await;

    socket
        .close(None)
        .await
        .expect("socket close should succeed");
    let _ = tokio::time::timeout(Duration::from_millis(250), socket.next()).await;

    let create_message = Request::builder()
        .method("POST")
        .uri(format!(
            "/guilds/{}/channels/{}/messages",
            channel.guild_id, channel.channel_id
        ))
        .header("authorization", format!("Bearer {}", auth.access_token))
        .header("content-type", "application/json")
        .header("x-forwarded-for", "203.0.113.55")
        .body(Body::from(
            json!({"content":"message after websocket disconnect"}).to_string(),
        ))
        .expect("create message request should build");
    let message_response = app
        .clone()
        .oneshot(create_message)
        .await
        .expect("create message request should execute");
    assert_eq!(message_response.status(), StatusCode::OK);

    server.abort();
}

#[tokio::test]
async fn websocket_subscription_receives_reaction_updates_from_rest() {
    let app = test_app();

    let auth = register_and_login(&app, "203.0.113.56").await;
    let channel = create_channel_context(&app, &auth, "203.0.113.56").await;

    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener should bind");
    let addr = listener
        .local_addr()
        .expect("listener addr should be readable");
    let server_app = app.clone();
    let server = tokio::spawn(async move {
        axum::serve(listener, server_app)
            .await
            .expect("server should run without errors");
    });

    let ws_url = format!("ws://{addr}/gateway/ws?access_token={}", auth.access_token);
    let mut ws_request = ws_url
        .into_client_request()
        .expect("websocket request should build");
    ws_request.headers_mut().insert(
        "x-forwarded-for",
        http::HeaderValue::from_static("203.0.113.56"),
    );
    let (mut socket, _response) = connect_async(ws_request)
        .await
        .expect("websocket handshake should succeed");

    let ready_json = next_text_event(&mut socket).await;
    assert_eq!(ready_json["t"], "ready");

    subscribe_to_channel(&mut socket, &channel).await;

    let create_message = Request::builder()
        .method("POST")
        .uri(format!(
            "/guilds/{}/channels/{}/messages",
            channel.guild_id, channel.channel_id
        ))
        .header("authorization", format!("Bearer {}", auth.access_token))
        .header("content-type", "application/json")
        .header("x-forwarded-for", "203.0.113.56")
        .body(Body::from(json!({"content":"reaction target"}).to_string()))
        .expect("create message request should build");
    let create_message_response = app
        .clone()
        .oneshot(create_message)
        .await
        .expect("create message request should execute");
    assert_eq!(create_message_response.status(), StatusCode::OK);
    let created_json: Value = parse_json_body(create_message_response).await;
    let message_id = created_json["message_id"]
        .as_str()
        .expect("message id should be present")
        .to_owned();

    let message_event = next_event_of_type(&mut socket, "message_create").await;
    assert_eq!(message_event["d"]["message_id"], message_id);

    let add_reaction = Request::builder()
        .method("POST")
        .uri(format!(
            "/guilds/{}/channels/{}/messages/{}/reactions/%F0%9F%91%8D",
            channel.guild_id, channel.channel_id, message_id
        ))
        .header("authorization", format!("Bearer {}", auth.access_token))
        .header("x-forwarded-for", "203.0.113.56")
        .body(Body::empty())
        .expect("add reaction request should build");
    let add_reaction_response = app
        .clone()
        .oneshot(add_reaction)
        .await
        .expect("add reaction request should execute");
    assert_eq!(add_reaction_response.status(), StatusCode::OK);

    let add_event = next_event_of_type(&mut socket, "message_reaction").await;
    assert_eq!(add_event["d"]["message_id"], message_id);
    assert_eq!(add_event["d"]["emoji"], "👍");
    assert_eq!(add_event["d"]["count"], 1);

    let remove_reaction = Request::builder()
        .method("DELETE")
        .uri(format!(
            "/guilds/{}/channels/{}/messages/{}/reactions/%F0%9F%91%8D",
            channel.guild_id, channel.channel_id, message_id
        ))
        .header("authorization", format!("Bearer {}", auth.access_token))
        .header("x-forwarded-for", "203.0.113.56")
        .body(Body::empty())
        .expect("remove reaction request should build");
    let remove_reaction_response = app
        .oneshot(remove_reaction)
        .await
        .expect("remove reaction request should execute");
    assert_eq!(remove_reaction_response.status(), StatusCode::OK);

    let remove_event = next_event_of_type(&mut socket, "message_reaction").await;
    assert_eq!(remove_event["d"]["message_id"], message_id);
    assert_eq!(remove_event["d"]["emoji"], "👍");
    assert_eq!(remove_event["d"]["count"], 0);

    socket
        .close(None)
        .await
        .expect("socket close should succeed");
    server.abort();
}

#[tokio::test]
async fn websocket_subscription_receives_message_lifecycle_updates_from_rest() {
    let app = test_app();

    let auth = register_and_login(&app, "203.0.113.58").await;
    let channel = create_channel_context(&app, &auth, "203.0.113.58").await;

    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener should bind");
    let addr = listener
        .local_addr()
        .expect("listener addr should be readable");
    let server_app = app.clone();
    let server = tokio::spawn(async move {
        axum::serve(listener, server_app)
            .await
            .expect("server should run without errors");
    });

    let ws_url = format!("ws://{addr}/gateway/ws?access_token={}", auth.access_token);

    let mut ws_a_request = ws_url
        .clone()
        .into_client_request()
        .expect("websocket request should build");
    ws_a_request.headers_mut().insert(
        "x-forwarded-for",
        http::HeaderValue::from_static("203.0.113.58"),
    );
    let (mut socket_a, _response) = connect_async(ws_a_request)
        .await
        .expect("websocket handshake should succeed");
    let ready_a = next_text_event(&mut socket_a).await;
    assert_eq!(ready_a["t"], "ready");
    subscribe_to_channel(&mut socket_a, &channel).await;

    let mut ws_b_request = ws_url
        .into_client_request()
        .expect("websocket request should build");
    ws_b_request.headers_mut().insert(
        "x-forwarded-for",
        http::HeaderValue::from_static("203.0.113.58"),
    );
    let (mut socket_b, _response) = connect_async(ws_b_request)
        .await
        .expect("websocket handshake should succeed");
    let ready_b = next_text_event(&mut socket_b).await;
    assert_eq!(ready_b["t"], "ready");
    subscribe_to_channel(&mut socket_b, &channel).await;

    let create_message = Request::builder()
        .method("POST")
        .uri(format!(
            "/guilds/{}/channels/{}/messages",
            channel.guild_id, channel.channel_id
        ))
        .header("authorization", format!("Bearer {}", auth.access_token))
        .header("content-type", "application/json")
        .header("x-forwarded-for", "203.0.113.58")
        .body(Body::from(json!({"content":"before edit"}).to_string()))
        .expect("create message request should build");
    let create_message_response = app
        .clone()
        .oneshot(create_message)
        .await
        .expect("create message request should execute");
    assert_eq!(create_message_response.status(), StatusCode::OK);
    let created_json: Value = parse_json_body(create_message_response).await;
    let message_id = created_json["message_id"]
        .as_str()
        .expect("message id should be present")
        .to_owned();

    let create_event_a = next_event_of_type(&mut socket_a, "message_create").await;
    assert_eq!(create_event_a["d"]["message_id"], message_id);
    let create_event_b = next_event_of_type(&mut socket_b, "message_create").await;
    assert_eq!(create_event_b["d"]["message_id"], message_id);

    let edit_message = Request::builder()
        .method("PATCH")
        .uri(format!(
            "/guilds/{}/channels/{}/messages/{}",
            channel.guild_id, channel.channel_id, message_id
        ))
        .header("authorization", format!("Bearer {}", auth.access_token))
        .header("content-type", "application/json")
        .header("x-forwarded-for", "203.0.113.58")
        .body(Body::from(json!({"content":"after edit"}).to_string()))
        .expect("edit message request should build");
    let edit_message_response = app
        .clone()
        .oneshot(edit_message)
        .await
        .expect("edit message request should execute");
    assert_eq!(edit_message_response.status(), StatusCode::OK);

    let update_event_a = next_event_of_type(&mut socket_a, "message_update").await;
    assert_eq!(update_event_a["d"]["message_id"], message_id);
    assert_eq!(
        update_event_a["d"]["updated_fields"]["content"],
        Value::String("after edit".to_owned())
    );
    let update_event_b = next_event_of_type(&mut socket_b, "message_update").await;
    assert_eq!(update_event_b["d"]["message_id"], message_id);
    assert_eq!(
        update_event_b["d"]["updated_fields"]["content"],
        Value::String("after edit".to_owned())
    );

    let delete_message = Request::builder()
        .method("DELETE")
        .uri(format!(
            "/guilds/{}/channels/{}/messages/{}",
            channel.guild_id, channel.channel_id, message_id
        ))
        .header("authorization", format!("Bearer {}", auth.access_token))
        .header("x-forwarded-for", "203.0.113.58")
        .body(Body::empty())
        .expect("delete message request should build");
    let delete_message_response = app
        .clone()
        .oneshot(delete_message)
        .await
        .expect("delete message request should execute");
    assert_eq!(delete_message_response.status(), StatusCode::NO_CONTENT);

    let delete_event_a = next_event_of_type(&mut socket_a, "message_delete").await;
    assert_eq!(delete_event_a["d"]["message_id"], message_id);
    let delete_event_b = next_event_of_type(&mut socket_b, "message_delete").await;
    assert_eq!(delete_event_b["d"]["message_id"], message_id);

    socket_a
        .close(None)
        .await
        .expect("socket close should succeed");
    socket_b
        .close(None)
        .await
        .expect("socket close should succeed");
    server.abort();
}

#[tokio::test]
async fn websocket_subscription_receives_channel_create_updates_from_rest() {
    let app = test_app();

    let auth = register_and_login(&app, "203.0.113.57").await;
    let channel = create_channel_context(&app, &auth, "203.0.113.57").await;

    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener should bind");
    let addr = listener
        .local_addr()
        .expect("listener addr should be readable");
    let server_app = app.clone();
    let server = tokio::spawn(async move {
        axum::serve(listener, server_app)
            .await
            .expect("server should run without errors");
    });

    let ws_url = format!("ws://{addr}/gateway/ws?access_token={}", auth.access_token);

    let mut ws_a_request = ws_url
        .clone()
        .into_client_request()
        .expect("websocket request should build");
    ws_a_request.headers_mut().insert(
        "x-forwarded-for",
        http::HeaderValue::from_static("203.0.113.57"),
    );
    let (mut socket_a, _response) = connect_async(ws_a_request)
        .await
        .expect("websocket handshake should succeed");
    let ready_a = next_text_event(&mut socket_a).await;
    assert_eq!(ready_a["t"], "ready");
    subscribe_to_channel(&mut socket_a, &channel).await;

    let mut ws_b_request = ws_url
        .into_client_request()
        .expect("websocket request should build");
    ws_b_request.headers_mut().insert(
        "x-forwarded-for",
        http::HeaderValue::from_static("203.0.113.57"),
    );
    let (mut socket_b, _response) = connect_async(ws_b_request)
        .await
        .expect("websocket handshake should succeed");
    let ready_b = next_text_event(&mut socket_b).await;
    assert_eq!(ready_b["t"], "ready");
    subscribe_to_channel(&mut socket_b, &channel).await;

    let create_channel = Request::builder()
        .method("POST")
        .uri(format!("/guilds/{}/channels", channel.guild_id))
        .header("authorization", format!("Bearer {}", auth.access_token))
        .header("content-type", "application/json")
        .header("x-forwarded-for", "203.0.113.57")
        .body(Body::from(
            json!({"name":"bridge-call","kind":"voice"}).to_string(),
        ))
        .expect("create channel request should build");
    let create_channel_response = app
        .clone()
        .oneshot(create_channel)
        .await
        .expect("create channel request should execute");
    assert_eq!(create_channel_response.status(), StatusCode::OK);
    let created_json: Value = parse_json_body(create_channel_response).await;
    let created_channel_id = created_json["channel_id"]
        .as_str()
        .expect("channel id should be present")
        .to_owned();

    let event_a = next_event_of_type(&mut socket_a, "channel_create").await;
    assert_eq!(event_a["d"]["guild_id"], channel.guild_id);
    assert_eq!(event_a["d"]["channel"]["channel_id"], created_channel_id);
    assert_eq!(event_a["d"]["channel"]["kind"], "voice");

    let event_b = next_event_of_type(&mut socket_b, "channel_create").await;
    assert_eq!(event_b["d"]["guild_id"], channel.guild_id);
    assert_eq!(event_b["d"]["channel"]["channel_id"], created_channel_id);
    assert_eq!(event_b["d"]["channel"]["kind"], "voice");

    socket_a
        .close(None)
        .await
        .expect("socket close should succeed");
    socket_b
        .close(None)
        .await
        .expect("socket close should succeed");
    server.abort();
}

#[tokio::test]
async fn websocket_subscription_receives_workspace_update_from_rest() {
    let app = test_app();

    let owner = register_and_login_as(&app, "workspace_owner_a", "203.0.113.71").await;
    let member = register_and_login_as(&app, "workspace_member_a", "203.0.113.72").await;
    let channel = create_channel_context(&app, &owner, "203.0.113.71").await;
    let member_id = user_id_from_me(&app, &member, "203.0.113.72").await;
    add_member(
        &app,
        &owner.access_token,
        "203.0.113.71",
        &channel.guild_id,
        &member_id,
    )
    .await;

    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener should bind");
    let addr = listener
        .local_addr()
        .expect("listener addr should be readable");
    let server_app = app.clone();
    let server = tokio::spawn(async move {
        axum::serve(listener, server_app)
            .await
            .expect("server should run without errors");
    });

    let owner_ws_url = format!("ws://{addr}/gateway/ws?access_token={}", owner.access_token);
    let mut owner_request = owner_ws_url
        .into_client_request()
        .expect("owner websocket request should build");
    owner_request.headers_mut().insert(
        "x-forwarded-for",
        http::HeaderValue::from_static("203.0.113.71"),
    );
    let (mut owner_socket, _response) = connect_async(owner_request)
        .await
        .expect("owner websocket handshake should succeed");
    let owner_ready = next_text_event(&mut owner_socket).await;
    assert_eq!(owner_ready["t"], "ready");
    subscribe_to_channel(&mut owner_socket, &channel).await;

    let member_ws_url = format!(
        "ws://{addr}/gateway/ws?access_token={}",
        member.access_token
    );
    let mut member_request = member_ws_url
        .into_client_request()
        .expect("member websocket request should build");
    member_request.headers_mut().insert(
        "x-forwarded-for",
        http::HeaderValue::from_static("203.0.113.72"),
    );
    let (mut member_socket, _response) = connect_async(member_request)
        .await
        .expect("member websocket handshake should succeed");
    let member_ready = next_text_event(&mut member_socket).await;
    assert_eq!(member_ready["t"], "ready");
    subscribe_to_channel(&mut member_socket, &channel).await;

    let update_workspace = Request::builder()
        .method("PATCH")
        .uri(format!("/guilds/{}", channel.guild_id))
        .header("authorization", format!("Bearer {}", owner.access_token))
        .header("content-type", "application/json")
        .header("x-forwarded-for", "203.0.113.71")
        .body(Body::from(
            json!({"name":"Network Guild Prime","visibility":"public"}).to_string(),
        ))
        .expect("update workspace request should build");
    let update_workspace_response = app
        .clone()
        .oneshot(update_workspace)
        .await
        .expect("update workspace request should execute");
    assert_eq!(update_workspace_response.status(), StatusCode::OK);

    let owner_event = next_event_of_type(&mut owner_socket, "workspace_update").await;
    assert_eq!(owner_event["d"]["guild_id"], channel.guild_id);
    assert_eq!(
        owner_event["d"]["updated_fields"]["name"],
        Value::String("Network Guild Prime".to_owned())
    );
    assert_eq!(
        owner_event["d"]["updated_fields"]["visibility"],
        Value::String("public".to_owned())
    );
    let member_event = next_event_of_type(&mut member_socket, "workspace_update").await;
    assert_eq!(member_event["d"]["guild_id"], channel.guild_id);
    assert_eq!(
        member_event["d"]["updated_fields"]["name"],
        Value::String("Network Guild Prime".to_owned())
    );
    assert_eq!(
        member_event["d"]["updated_fields"]["visibility"],
        Value::String("public".to_owned())
    );

    owner_socket
        .close(None)
        .await
        .expect("owner socket close should succeed");
    member_socket
        .close(None)
        .await
        .expect("member socket close should succeed");
    server.abort();
}

#[tokio::test]
async fn websocket_voice_participant_sync_repairs_and_disconnect_cleanup() {
    let app = build_router(&AppConfig {
        max_body_bytes: 1024 * 32,
        request_timeout: Duration::from_secs(2),
        rate_limit_requests_per_minute: 200,
        auth_route_requests_per_minute: 200,
        gateway_ingress_events_per_window: 20,
        gateway_ingress_window: Duration::from_secs(10),
        gateway_outbound_queue: 256,
        max_gateway_event_bytes: AppConfig::default().max_gateway_event_bytes,
        livekit_url: String::from("ws://livekit.test:7880"),
        livekit_api_key: Some(String::from("phase8-key")),
        livekit_api_secret: Some(String::from("phase8-secret")),
        ..AppConfig::default()
    })
    .expect("voice test router should build");
    let owner = register_and_login_as(&app, "phase8_voice_owner", "203.0.113.141").await;
    let member = register_and_login_as(&app, "phase8_voice_member", "203.0.113.142").await;
    let outsider = register_and_login_as(&app, "phase8_voice_outsider", "203.0.113.143").await;
    let channel = create_channel_context(&app, &owner, "203.0.113.141").await;
    let member_id = user_id_from_me(&app, &member, "203.0.113.142").await;
    add_member(
        &app,
        &owner.access_token,
        "203.0.113.141",
        &channel.guild_id,
        &member_id,
    )
    .await;
    let voice_channel_id =
        create_voice_channel(&app, &owner, "203.0.113.141", &channel.guild_id).await;
    let voice_channel = ChannelRef {
        guild_id: channel.guild_id.clone(),
        channel_id: voice_channel_id.clone(),
    };

    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener should bind");
    let addr = listener
        .local_addr()
        .expect("listener addr should be readable");
    let server_app = app.clone();
    let server = tokio::spawn(async move {
        axum::serve(listener, server_app)
            .await
            .expect("server should run without errors");
    });

    let ws_url = |token: &str| format!("ws://{addr}/gateway/ws?access_token={token}");

    let mut owner_req = ws_url(&owner.access_token)
        .into_client_request()
        .expect("owner ws request should build");
    owner_req.headers_mut().insert(
        "x-forwarded-for",
        http::HeaderValue::from_static("203.0.113.141"),
    );
    let (mut owner_socket, _) = connect_async(owner_req)
        .await
        .expect("owner ws should connect");
    let _ = next_event_of_type(&mut owner_socket, "ready").await;
    subscribe_to_channel(&mut owner_socket, &voice_channel).await;
    let initial_sync = next_event_of_type(&mut owner_socket, "voice_participant_sync").await;
    assert_eq!(
        initial_sync["d"]["participants"]
            .as_array()
            .expect("participants array")
            .len(),
        0
    );

    let mut member_req = ws_url(&member.access_token)
        .into_client_request()
        .expect("member ws request should build");
    member_req.headers_mut().insert(
        "x-forwarded-for",
        http::HeaderValue::from_static("203.0.113.142"),
    );
    let (mut member_socket, _) = connect_async(member_req)
        .await
        .expect("member ws should connect");
    let _ = next_event_of_type(&mut member_socket, "ready").await;
    subscribe_to_channel(&mut member_socket, &voice_channel).await;
    let _ = next_event_of_type(&mut member_socket, "voice_participant_sync").await;

    let mut outsider_req = ws_url(&outsider.access_token)
        .into_client_request()
        .expect("outsider ws request should build");
    outsider_req.headers_mut().insert(
        "x-forwarded-for",
        http::HeaderValue::from_static("203.0.113.143"),
    );
    let (mut outsider_socket, _) = connect_async(outsider_req)
        .await
        .expect("outsider ws should connect");
    let _ = next_event_of_type(&mut outsider_socket, "ready").await;
    outsider_socket
        .send(Message::Text(
            json!({
                "v": 1,
                "t": "subscribe",
                "d": {
                    "guild_id": voice_channel.guild_id,
                    "channel_id": voice_channel.channel_id,
                }
            })
            .to_string()
            .into(),
        ))
        .await
        .expect("outsider subscribe should send");
    let _ = tokio::time::timeout(Duration::from_secs(1), outsider_socket.next()).await;

    let issue_token = Request::builder()
        .method("POST")
        .uri(format!(
            "/guilds/{}/channels/{}/voice/token",
            voice_channel.guild_id, voice_channel.channel_id
        ))
        .header("authorization", format!("Bearer {}", owner.access_token))
        .header("content-type", "application/json")
        .header("x-forwarded-for", "203.0.113.141")
        .body(Body::from(
            json!({
                "can_publish": true,
                "can_subscribe": true,
                "publish_sources": ["microphone", "camera"]
            })
            .to_string(),
        ))
        .expect("voice token request should build");
    let issue_token_response = app
        .clone()
        .oneshot(issue_token)
        .await
        .expect("voice token request should execute");
    assert_eq!(issue_token_response.status(), StatusCode::OK);

    let owner_join = next_event_of_type(&mut owner_socket, "voice_participant_join").await;
    let member_join = next_event_of_type(&mut member_socket, "voice_participant_join").await;
    assert_eq!(
        owner_join["d"]["participant"]["identity"],
        member_join["d"]["participant"]["identity"]
    );
    let joined_identity = owner_join["d"]["participant"]["identity"]
        .as_str()
        .expect("joined identity should exist")
        .to_owned();

    let member_publish = next_event_of_type(&mut member_socket, "voice_stream_publish").await;
    assert_eq!(member_publish["d"]["identity"], joined_identity);

    let mut member_second_req = ws_url(&member.access_token)
        .into_client_request()
        .expect("member second ws request should build");
    member_second_req.headers_mut().insert(
        "x-forwarded-for",
        http::HeaderValue::from_static("203.0.113.144"),
    );
    let (mut member_second_socket, _) = connect_async(member_second_req)
        .await
        .expect("member second ws should connect");
    let _ = next_event_of_type(&mut member_second_socket, "ready").await;
    subscribe_to_channel(&mut member_second_socket, &voice_channel).await;
    let repaired_sync =
        next_event_of_type(&mut member_second_socket, "voice_participant_sync").await;
    assert_eq!(
        repaired_sync["d"]["participants"][0]["identity"],
        joined_identity
    );

    owner_socket
        .close(None)
        .await
        .expect("owner close should succeed");
    let leave_for_member = next_event_of_type(&mut member_socket, "voice_participant_leave").await;
    assert_eq!(leave_for_member["d"]["identity"], joined_identity);
    let leave_for_member_second =
        next_event_of_type(&mut member_second_socket, "voice_participant_leave").await;
    assert_eq!(leave_for_member_second["d"]["identity"], joined_identity);

    assert!(
        maybe_next_event_of_type(
            &mut outsider_socket,
            "voice_participant_join",
            Duration::from_millis(250),
        )
        .await
        .is_none(),
        "outsider must not observe voice participant events",
    );

    member_socket
        .close(None)
        .await
        .expect("member close should succeed");
    member_second_socket
        .close(None)
        .await
        .expect("member second close should succeed");
    let _ = outsider_socket.close(None).await;
    server.abort();
}

#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn websocket_profile_and_friendship_events_sync_across_sessions() {
    let app = test_app();
    let app_http = app.clone();

    let alice = register_and_login_as(&app_http, "phase5_alice", "203.0.113.120").await;
    let bob = register_and_login_as(&app_http, "phase5_bob", "203.0.113.121").await;
    let outsider = register_and_login_as(&app_http, "phase5_outsider", "203.0.113.122").await;
    let bob_id = user_id_from_me(&app_http, &bob, "203.0.113.121").await;
    let shared_guild = create_channel_context(&app_http, &alice, "203.0.113.120").await;
    add_member(
        &app_http,
        &alice.access_token,
        "203.0.113.120",
        &shared_guild.guild_id,
        &bob_id,
    )
    .await;

    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener should bind");
    let addr = listener
        .local_addr()
        .expect("listener addr should be readable");
    let server = tokio::spawn(async move {
        axum::serve(listener, app)
            .await
            .expect("server should run without errors");
    });

    let build_ws_request = |token: &str, ip: &'static str| {
        let ws_url = format!("ws://{addr}/gateway/ws?access_token={token}");
        let mut request = ws_url
            .into_client_request()
            .expect("websocket request should build");
        request
            .headers_mut()
            .insert("x-forwarded-for", http::HeaderValue::from_static(ip));
        request
    };

    let (mut alice_socket_a, _response) =
        connect_async(build_ws_request(&alice.access_token, "203.0.113.120"))
            .await
            .expect("alice socket a should connect");
    let (mut alice_socket_b, _response) =
        connect_async(build_ws_request(&alice.access_token, "203.0.113.123"))
            .await
            .expect("alice socket b should connect");
    let (mut bob_socket, _response) =
        connect_async(build_ws_request(&bob.access_token, "203.0.113.121"))
            .await
            .expect("bob socket should connect");
    let (mut outsider_socket, _response) =
        connect_async(build_ws_request(&outsider.access_token, "203.0.113.122"))
            .await
            .expect("outsider socket should connect");

    let _ = next_event_of_type(&mut alice_socket_a, "ready").await;
    let _ = next_event_of_type(&mut alice_socket_b, "ready").await;
    let _ = next_event_of_type(&mut bob_socket, "ready").await;
    let _ = next_event_of_type(&mut outsider_socket, "ready").await;

    let update_profile = Request::builder()
        .method("PATCH")
        .uri("/users/me/profile")
        .header("authorization", format!("Bearer {}", alice.access_token))
        .header("content-type", "application/json")
        .header("x-forwarded-for", "203.0.113.120")
        .body(Body::from(
            json!({"username":"phase5_alice_renamed","about_markdown":"phase5-about"}).to_string(),
        ))
        .expect("profile update request should build");
    let update_profile_response = app_http
        .clone()
        .oneshot(update_profile)
        .await
        .expect("profile update request should execute");
    assert_eq!(update_profile_response.status(), StatusCode::OK);

    let profile_update_a = next_event_of_type(&mut alice_socket_a, "profile_update").await;
    let profile_update_b = next_event_of_type(&mut alice_socket_b, "profile_update").await;
    assert_eq!(
        profile_update_a["d"]["updated_fields"]["username"],
        Value::String("phase5_alice_renamed".to_owned())
    );
    assert_eq!(profile_update_b["d"], profile_update_a["d"]);
    let profile_update_bob = next_event_of_type(&mut bob_socket, "profile_update").await;
    assert_eq!(
        profile_update_bob["d"]["updated_fields"]["username"],
        Value::String("phase5_alice_renamed".to_owned())
    );
    assert!(
        tokio::time::timeout(
            Duration::from_millis(200),
            next_event_of_type(&mut outsider_socket, "profile_update"),
        )
        .await
        .is_err(),
        "outsider should not receive profile updates",
    );

    let create_request = Request::builder()
        .method("POST")
        .uri("/friends/requests")
        .header("authorization", format!("Bearer {}", alice.access_token))
        .header("content-type", "application/json")
        .header("x-forwarded-for", "203.0.113.120")
        .body(Body::from(json!({"recipient_user_id":bob_id}).to_string()))
        .expect("friend request create should build");
    let create_response = app_http
        .clone()
        .oneshot(create_request)
        .await
        .expect("friend request create should execute");
    assert_eq!(create_response.status(), StatusCode::OK);
    let create_json: Value = parse_json_body(create_response).await;
    let request_id = create_json["request_id"]
        .as_str()
        .expect("request id should exist")
        .to_owned();

    let friend_create_alice =
        next_event_of_type(&mut alice_socket_a, "friend_request_create").await;
    let friend_create_bob = next_event_of_type(&mut bob_socket, "friend_request_create").await;
    assert_eq!(
        friend_create_alice["d"]["request_id"],
        Value::String(request_id.clone())
    );
    assert_eq!(
        friend_create_bob["d"]["request_id"],
        Value::String(request_id.clone())
    );

    let accept_request = Request::builder()
        .method("POST")
        .uri(format!("/friends/requests/{request_id}/accept"))
        .header("authorization", format!("Bearer {}", bob.access_token))
        .header("x-forwarded-for", "203.0.113.121")
        .body(Body::empty())
        .expect("friend request accept should build");
    let accept_response = app_http
        .clone()
        .oneshot(accept_request)
        .await
        .expect("friend request accept should execute");
    assert_eq!(accept_response.status(), StatusCode::OK);

    let friend_update_alice =
        next_event_of_type(&mut alice_socket_a, "friend_request_update").await;
    let friend_update_bob = next_event_of_type(&mut bob_socket, "friend_request_update").await;
    assert_eq!(
        friend_update_alice["d"]["state"],
        Value::String("accepted".to_owned())
    );
    assert_eq!(
        friend_update_bob["d"]["state"],
        Value::String("accepted".to_owned())
    );

    let second_profile_update = Request::builder()
        .method("PATCH")
        .uri("/users/me/profile")
        .header("authorization", format!("Bearer {}", alice.access_token))
        .header("content-type", "application/json")
        .header("x-forwarded-for", "203.0.113.120")
        .body(Body::from(
            json!({"about_markdown":"phase5-about-updated"}).to_string(),
        ))
        .expect("second profile update request should build");
    let second_profile_response = app_http
        .clone()
        .oneshot(second_profile_update)
        .await
        .expect("second profile update should execute");
    assert_eq!(second_profile_response.status(), StatusCode::OK);

    let friend_profile_update_bob = next_event_of_type(&mut bob_socket, "profile_update").await;
    assert_eq!(
        friend_profile_update_bob["d"]["updated_fields"]["about_markdown"],
        Value::String("phase5-about-updated".to_owned())
    );

    let upload_avatar = Request::builder()
        .method("POST")
        .uri("/users/me/profile/avatar")
        .header("authorization", format!("Bearer {}", alice.access_token))
        .header("content-type", "image/png")
        .header("x-forwarded-for", "203.0.113.120")
        .body(Body::from(vec![137, 80, 78, 71, 13, 10, 26, 10]))
        .expect("avatar upload should build");
    let upload_avatar_response = app_http
        .clone()
        .oneshot(upload_avatar)
        .await
        .expect("avatar upload should execute");
    assert_eq!(upload_avatar_response.status(), StatusCode::OK);

    let avatar_update_alice =
        next_event_of_type(&mut alice_socket_b, "profile_avatar_update").await;
    let avatar_update_bob = next_event_of_type(&mut bob_socket, "profile_avatar_update").await;
    assert_eq!(
        avatar_update_alice["d"]["avatar_version"].as_i64(),
        avatar_update_bob["d"]["avatar_version"].as_i64()
    );
    assert!(
        tokio::time::timeout(
            Duration::from_millis(200),
            next_event_of_type(&mut outsider_socket, "profile_avatar_update"),
        )
        .await
        .is_err(),
        "outsider should not receive profile avatar updates",
    );

    let remove_friend = Request::builder()
        .method("DELETE")
        .uri(format!("/friends/{bob_id}"))
        .header("authorization", format!("Bearer {}", alice.access_token))
        .header("x-forwarded-for", "203.0.113.120")
        .body(Body::empty())
        .expect("remove friend request should build");
    let remove_friend_response = app_http
        .clone()
        .oneshot(remove_friend)
        .await
        .expect("remove friend request should execute");
    assert_eq!(remove_friend_response.status(), StatusCode::NO_CONTENT);

    let friend_remove_alice = next_event_of_type(&mut alice_socket_a, "friend_remove").await;
    let friend_remove_bob = next_event_of_type(&mut bob_socket, "friend_remove").await;
    assert_eq!(
        friend_remove_alice["d"]["friend_user_id"],
        friend_remove_bob["d"]["user_id"]
    );

    assert!(
        tokio::time::timeout(
            Duration::from_millis(200),
            next_event_of_type(&mut outsider_socket, "friend_request_create"),
        )
        .await
        .is_err(),
        "outsider should not receive friendship events"
    );

    alice_socket_a
        .close(None)
        .await
        .expect("alice socket a close should succeed");
    alice_socket_b
        .close(None)
        .await
        .expect("alice socket b close should succeed");
    bob_socket
        .close(None)
        .await
        .expect("bob socket close should succeed");
    outsider_socket
        .close(None)
        .await
        .expect("outsider socket close should succeed");
    server.abort();
}

#[tokio::test]
async fn websocket_subscription_receives_workspace_membership_transitions() {
    let app = test_app();

    let owner = register_and_login_as(&app, "workspace_owner_b", "203.0.113.81").await;
    let moderator = register_and_login_as(&app, "workspace_member_b", "203.0.113.82").await;
    let target = register_and_login_as(&app, "workspace_target_b", "203.0.113.83").await;
    let channel = create_channel_context(&app, &owner, "203.0.113.81").await;
    let moderator_id = user_id_from_me(&app, &moderator, "203.0.113.82").await;
    let target_id = user_id_from_me(&app, &target, "203.0.113.83").await;
    add_member(
        &app,
        &owner.access_token,
        "203.0.113.81",
        &channel.guild_id,
        &moderator_id,
    )
    .await;

    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener should bind");
    let addr = listener
        .local_addr()
        .expect("listener addr should be readable");
    let server_app = app.clone();
    let server = tokio::spawn(async move {
        axum::serve(listener, server_app)
            .await
            .expect("server should run without errors");
    });

    let owner_ws_url = format!("ws://{addr}/gateway/ws?access_token={}", owner.access_token);
    let mut owner_request = owner_ws_url
        .into_client_request()
        .expect("owner websocket request should build");
    owner_request.headers_mut().insert(
        "x-forwarded-for",
        http::HeaderValue::from_static("203.0.113.81"),
    );
    let (mut owner_socket, _response) = connect_async(owner_request)
        .await
        .expect("owner websocket handshake should succeed");
    let owner_ready = next_text_event(&mut owner_socket).await;
    assert_eq!(owner_ready["t"], "ready");
    subscribe_to_channel(&mut owner_socket, &channel).await;

    let moderator_ws_url = format!(
        "ws://{addr}/gateway/ws?access_token={}",
        moderator.access_token
    );
    let mut moderator_request = moderator_ws_url
        .into_client_request()
        .expect("moderator websocket request should build");
    moderator_request.headers_mut().insert(
        "x-forwarded-for",
        http::HeaderValue::from_static("203.0.113.82"),
    );
    let (mut moderator_socket, _response) = connect_async(moderator_request)
        .await
        .expect("moderator websocket handshake should succeed");
    let moderator_ready = next_text_event(&mut moderator_socket).await;
    assert_eq!(moderator_ready["t"], "ready");
    subscribe_to_channel(&mut moderator_socket, &channel).await;

    add_member(
        &app,
        &owner.access_token,
        "203.0.113.81",
        &channel.guild_id,
        &target_id,
    )
    .await;
    let add_event_owner = next_event_of_type(&mut owner_socket, "workspace_member_add").await;
    assert_eq!(add_event_owner["d"]["guild_id"], channel.guild_id);
    assert_eq!(add_event_owner["d"]["user_id"], target_id);
    assert_eq!(add_event_owner["d"]["role"], "member");
    let add_event_moderator =
        next_event_of_type(&mut moderator_socket, "workspace_member_add").await;
    assert_eq!(add_event_moderator["d"]["user_id"], target_id);

    let update_role = Request::builder()
        .method("PATCH")
        .uri(format!("/guilds/{}/members/{target_id}", channel.guild_id))
        .header("authorization", format!("Bearer {}", owner.access_token))
        .header("content-type", "application/json")
        .header("x-forwarded-for", "203.0.113.81")
        .body(Body::from(json!({"role":"moderator"}).to_string()))
        .expect("update member role request should build");
    let update_role_response = app
        .clone()
        .oneshot(update_role)
        .await
        .expect("update member role request should execute");
    assert_eq!(update_role_response.status(), StatusCode::OK);
    let update_event_owner = next_event_of_type(&mut owner_socket, "workspace_member_update").await;
    assert_eq!(update_event_owner["d"]["user_id"], target_id);
    assert_eq!(
        update_event_owner["d"]["updated_fields"]["role"],
        Value::String("moderator".to_owned())
    );
    let update_event_moderator =
        next_event_of_type(&mut moderator_socket, "workspace_member_update").await;
    assert_eq!(update_event_moderator["d"]["user_id"], target_id);

    let kick_member = Request::builder()
        .method("POST")
        .uri(format!(
            "/guilds/{}/members/{target_id}/kick",
            channel.guild_id
        ))
        .header("authorization", format!("Bearer {}", owner.access_token))
        .header("x-forwarded-for", "203.0.113.81")
        .body(Body::empty())
        .expect("kick member request should build");
    let kick_member_response = app
        .clone()
        .oneshot(kick_member)
        .await
        .expect("kick member request should execute");
    assert_eq!(kick_member_response.status(), StatusCode::OK);
    let remove_event_owner = next_event_of_type(&mut owner_socket, "workspace_member_remove").await;
    assert_eq!(remove_event_owner["d"]["user_id"], target_id);
    assert_eq!(remove_event_owner["d"]["reason"], "kick");
    let remove_event_moderator =
        next_event_of_type(&mut moderator_socket, "workspace_member_remove").await;
    assert_eq!(remove_event_moderator["d"]["user_id"], target_id);
    assert_eq!(remove_event_moderator["d"]["reason"], "kick");

    let ban_member = Request::builder()
        .method("POST")
        .uri(format!(
            "/guilds/{}/members/{moderator_id}/ban",
            channel.guild_id
        ))
        .header("authorization", format!("Bearer {}", owner.access_token))
        .header("x-forwarded-for", "203.0.113.81")
        .body(Body::empty())
        .expect("ban member request should build");
    let ban_member_response = app
        .clone()
        .oneshot(ban_member)
        .await
        .expect("ban member request should execute");
    assert_eq!(ban_member_response.status(), StatusCode::OK);
    let ban_event_owner = next_event_of_type(&mut owner_socket, "workspace_member_ban").await;
    assert_eq!(ban_event_owner["d"]["user_id"], moderator_id);
    let ban_remove_owner = next_event_of_type(&mut owner_socket, "workspace_member_remove").await;
    assert_eq!(ban_remove_owner["d"]["user_id"], moderator_id);
    assert_eq!(ban_remove_owner["d"]["reason"], "ban");

    owner_socket
        .close(None)
        .await
        .expect("owner socket close should succeed");
    moderator_socket
        .close(None)
        .await
        .expect("moderator socket close should succeed");
    server.abort();
}

#[tokio::test]
async fn websocket_subscription_receives_phase4_permission_and_moderation_events() {
    let app = test_app();

    let owner = register_and_login_as(&app, "workspace_owner_phase4", "203.0.113.91").await;
    let moderator = register_and_login_as(&app, "workspace_moderator_phase4", "203.0.113.92").await;
    let member = register_and_login_as(&app, "workspace_member_phase4", "203.0.113.93").await;
    let target = register_and_login_as(&app, "workspace_target_phase4", "203.0.113.94").await;

    let channel = create_channel_context(&app, &owner, "203.0.113.91").await;
    let moderator_id = user_id_from_me(&app, &moderator, "203.0.113.92").await;
    let member_id = user_id_from_me(&app, &member, "203.0.113.93").await;
    let target_id = user_id_from_me(&app, &target, "203.0.113.94").await;
    add_member(
        &app,
        &owner.access_token,
        "203.0.113.91",
        &channel.guild_id,
        &moderator_id,
    )
    .await;
    add_member(
        &app,
        &owner.access_token,
        "203.0.113.91",
        &channel.guild_id,
        &member_id,
    )
    .await;

    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener should bind");
    let addr = listener
        .local_addr()
        .expect("listener addr should be readable");
    let server_app = app.clone();
    let server = tokio::spawn(async move {
        axum::serve(listener, server_app)
            .await
            .expect("server should run without errors");
    });

    let owner_ws_url = format!("ws://{addr}/gateway/ws?access_token={}", owner.access_token);
    let mut owner_request = owner_ws_url
        .into_client_request()
        .expect("owner websocket request should build");
    owner_request.headers_mut().insert(
        "x-forwarded-for",
        http::HeaderValue::from_static("203.0.113.91"),
    );
    let (mut owner_socket, _response) = connect_async(owner_request)
        .await
        .expect("owner websocket handshake should succeed");
    let owner_ready = next_text_event(&mut owner_socket).await;
    assert_eq!(owner_ready["t"], "ready");
    subscribe_to_channel(&mut owner_socket, &channel).await;

    let moderator_ws_url = format!(
        "ws://{addr}/gateway/ws?access_token={}",
        moderator.access_token
    );
    let mut moderator_request = moderator_ws_url
        .into_client_request()
        .expect("moderator websocket request should build");
    moderator_request.headers_mut().insert(
        "x-forwarded-for",
        http::HeaderValue::from_static("203.0.113.92"),
    );
    let (mut moderator_socket, _response) = connect_async(moderator_request)
        .await
        .expect("moderator websocket handshake should succeed");
    let moderator_ready = next_text_event(&mut moderator_socket).await;
    assert_eq!(moderator_ready["t"], "ready");
    subscribe_to_channel(&mut moderator_socket, &channel).await;

    let member_ws_url = format!(
        "ws://{addr}/gateway/ws?access_token={}",
        member.access_token
    );
    let mut member_request = member_ws_url
        .into_client_request()
        .expect("member websocket request should build");
    member_request.headers_mut().insert(
        "x-forwarded-for",
        http::HeaderValue::from_static("203.0.113.93"),
    );
    let (mut member_socket, _response) = connect_async(member_request)
        .await
        .expect("member websocket handshake should succeed");
    let member_ready = next_text_event(&mut member_socket).await;
    assert_eq!(member_ready["t"], "ready");
    subscribe_to_channel(&mut member_socket, &channel).await;

    let create_role = Request::builder()
        .method("POST")
        .uri(format!("/guilds/{}/roles", channel.guild_id))
        .header("authorization", format!("Bearer {}", owner.access_token))
        .header("content-type", "application/json")
        .header("x-forwarded-for", "203.0.113.91")
        .body(Body::from(
            json!({"name":"ops_admin","permissions":["manage_roles"],"position":90}).to_string(),
        ))
        .expect("create role request should build");
    let create_role_response = app
        .clone()
        .oneshot(create_role)
        .await
        .expect("create role request should execute");
    assert_eq!(create_role_response.status(), StatusCode::OK);
    let created_role: Value = parse_json_body(create_role_response).await;
    let role_id = created_role["role_id"]
        .as_str()
        .expect("role id should exist")
        .to_owned();

    let create_event_owner = next_event_of_type(&mut owner_socket, "workspace_role_create").await;
    assert_eq!(create_event_owner["d"]["guild_id"], channel.guild_id);
    assert_eq!(create_event_owner["d"]["role"]["role_id"], role_id);
    let create_event_member = next_event_of_type(&mut member_socket, "workspace_role_create").await;
    assert_eq!(create_event_member["d"]["role"]["role_id"], role_id);
    let create_event_moderator =
        next_event_of_type(&mut moderator_socket, "workspace_role_create").await;
    assert_eq!(create_event_moderator["d"]["role"]["role_id"], role_id);

    let update_role = Request::builder()
        .method("PATCH")
        .uri(format!("/guilds/{}/roles/{role_id}", channel.guild_id))
        .header("authorization", format!("Bearer {}", owner.access_token))
        .header("content-type", "application/json")
        .header("x-forwarded-for", "203.0.113.91")
        .body(Body::from(json!({"name":"ops_admin_v2"}).to_string()))
        .expect("update role request should build");
    let update_role_response = app
        .clone()
        .oneshot(update_role)
        .await
        .expect("update role request should execute");
    assert_eq!(update_role_response.status(), StatusCode::OK);
    let update_event = next_event_of_type(&mut owner_socket, "workspace_role_update").await;
    assert_eq!(update_event["d"]["role_id"], role_id);

    let assign_role = Request::builder()
        .method("POST")
        .uri(format!(
            "/guilds/{}/roles/{role_id}/members/{member_id}",
            channel.guild_id
        ))
        .header("authorization", format!("Bearer {}", owner.access_token))
        .header("x-forwarded-for", "203.0.113.91")
        .body(Body::empty())
        .expect("assign role request should build");
    let assign_role_response = app
        .clone()
        .oneshot(assign_role)
        .await
        .expect("assign role request should execute");
    assert_eq!(assign_role_response.status(), StatusCode::OK);
    let assignment_add_event =
        next_event_of_type(&mut owner_socket, "workspace_role_assignment_add").await;
    assert_eq!(assignment_add_event["d"]["role_id"], role_id);
    assert_eq!(assignment_add_event["d"]["user_id"], member_id);

    let unassign_role = Request::builder()
        .method("DELETE")
        .uri(format!(
            "/guilds/{}/roles/{role_id}/members/{member_id}",
            channel.guild_id
        ))
        .header("authorization", format!("Bearer {}", owner.access_token))
        .header("x-forwarded-for", "203.0.113.91")
        .body(Body::empty())
        .expect("unassign role request should build");
    let unassign_role_response = app
        .clone()
        .oneshot(unassign_role)
        .await
        .expect("unassign role request should execute");
    assert_eq!(unassign_role_response.status(), StatusCode::OK);
    let assignment_remove_event =
        next_event_of_type(&mut owner_socket, "workspace_role_assignment_remove").await;
    assert_eq!(assignment_remove_event["d"]["role_id"], role_id);

    let reorder_roles = Request::builder()
        .method("POST")
        .uri(format!("/guilds/{}/roles/reorder", channel.guild_id))
        .header("authorization", format!("Bearer {}", owner.access_token))
        .header("content-type", "application/json")
        .header("x-forwarded-for", "203.0.113.91")
        .body(Body::from(json!({"role_ids":[role_id]}).to_string()))
        .expect("reorder roles request should build");
    let reorder_roles_response = app
        .clone()
        .oneshot(reorder_roles)
        .await
        .expect("reorder roles request should execute");
    assert_eq!(reorder_roles_response.status(), StatusCode::OK);
    let reorder_event = next_event_of_type(&mut owner_socket, "workspace_role_reorder").await;
    assert_eq!(reorder_event["d"]["role_ids"], json!([role_id]));

    let override_update = Request::builder()
        .method("POST")
        .uri(format!(
            "/guilds/{}/channels/{}/overrides/moderator",
            channel.guild_id, channel.channel_id
        ))
        .header("authorization", format!("Bearer {}", owner.access_token))
        .header("content-type", "application/json")
        .header("x-forwarded-for", "203.0.113.91")
        .body(Body::from(
            json!({"allow":["create_message"],"deny":["ban_member"]}).to_string(),
        ))
        .expect("override update request should build");
    let override_update_response = app
        .clone()
        .oneshot(override_update)
        .await
        .expect("override update request should execute");
    assert_eq!(override_update_response.status(), StatusCode::OK);
    let override_event =
        next_event_of_type(&mut owner_socket, "workspace_channel_override_update").await;
    assert_eq!(override_event["d"]["channel_id"], channel.channel_id);
    assert_eq!(
        override_event["d"]["updated_fields"]["allow"],
        json!(["create_message"])
    );
    assert_eq!(
        override_event["d"]["updated_fields"]["deny"],
        json!(["ban_member"])
    );

    let delete_role = Request::builder()
        .method("DELETE")
        .uri(format!("/guilds/{}/roles/{role_id}", channel.guild_id))
        .header("authorization", format!("Bearer {}", owner.access_token))
        .header("x-forwarded-for", "203.0.113.91")
        .body(Body::empty())
        .expect("delete role request should build");
    let delete_role_response = app
        .clone()
        .oneshot(delete_role)
        .await
        .expect("delete role request should execute");
    assert_eq!(delete_role_response.status(), StatusCode::OK);
    let delete_event = next_event_of_type(&mut owner_socket, "workspace_role_delete").await;
    assert_eq!(delete_event["d"]["role_id"], role_id);

    let make_public = Request::builder()
        .method("PATCH")
        .uri(format!("/guilds/{}", channel.guild_id))
        .header("authorization", format!("Bearer {}", owner.access_token))
        .header("content-type", "application/json")
        .header("x-forwarded-for", "203.0.113.91")
        .body(Body::from(json!({"visibility":"public"}).to_string()))
        .expect("workspace update request should build");
    let make_public_response = app
        .clone()
        .oneshot(make_public)
        .await
        .expect("workspace update request should execute");
    assert_eq!(make_public_response.status(), StatusCode::OK);

    let mut join_public = Request::builder()
        .method("POST")
        .uri(format!("/guilds/{}/join", channel.guild_id))
        .header("authorization", format!("Bearer {}", target.access_token))
        .header("x-forwarded-for", "203.0.113.94")
        .body(Body::empty())
        .expect("join request should build");
    join_public
        .extensions_mut()
        .insert(ConnectInfo(SocketAddr::from(([203, 0, 113, 94], 40_194))));
    let join_public_response = app
        .clone()
        .oneshot(join_public)
        .await
        .expect("join request should execute");
    assert_eq!(join_public_response.status(), StatusCode::OK);

    let add_ip_bans = Request::builder()
        .method("POST")
        .uri(format!("/guilds/{}/ip-bans/by-user", channel.guild_id))
        .header("authorization", format!("Bearer {}", owner.access_token))
        .header("content-type", "application/json")
        .header("x-forwarded-for", "203.0.113.91")
        .body(Body::from(
            json!({"target_user_id":target_id,"reason":"abuse"}).to_string(),
        ))
        .expect("add ip bans request should build");
    let add_ip_bans_response = app
        .clone()
        .oneshot(add_ip_bans)
        .await
        .expect("add ip bans request should execute");
    assert_eq!(add_ip_bans_response.status(), StatusCode::OK);
    let add_ip_bans_json: Value = parse_json_body(add_ip_bans_response).await;
    assert_eq!(add_ip_bans_json["created_count"], json!(1));
    let created_ban_id = add_ip_bans_json["ban_ids"][0]
        .as_str()
        .expect("ban id should exist")
        .to_owned();

    let ip_ban_sync_owner = next_event_of_type(&mut owner_socket, "workspace_ip_ban_sync").await;
    assert_eq!(ip_ban_sync_owner["d"]["summary"]["action"], "upsert");
    assert_eq!(ip_ban_sync_owner["d"]["summary"]["changed_count"], json!(1));
    assert!(!contains_ip_field(&ip_ban_sync_owner["d"]));
    let ip_ban_sync_moderator =
        next_event_of_type(&mut moderator_socket, "workspace_ip_ban_sync").await;
    assert_eq!(ip_ban_sync_moderator["d"]["summary"]["action"], "upsert");
    assert!(!contains_ip_field(&ip_ban_sync_moderator["d"]));
    let ip_ban_sync_member = next_event_of_type(&mut member_socket, "workspace_ip_ban_sync").await;
    assert_eq!(ip_ban_sync_member["d"]["summary"]["action"], "upsert");
    assert!(!contains_ip_field(&ip_ban_sync_member["d"]));

    let remove_ip_ban = Request::builder()
        .method("DELETE")
        .uri(format!(
            "/guilds/{}/ip-bans/{created_ban_id}",
            channel.guild_id
        ))
        .header("authorization", format!("Bearer {}", owner.access_token))
        .header("x-forwarded-for", "203.0.113.91")
        .body(Body::empty())
        .expect("remove ip ban request should build");
    let remove_ip_ban_response = app
        .clone()
        .oneshot(remove_ip_ban)
        .await
        .expect("remove ip ban request should execute");
    assert_eq!(remove_ip_ban_response.status(), StatusCode::OK);

    let ip_ban_remove_owner = next_event_of_type(&mut owner_socket, "workspace_ip_ban_sync").await;
    assert_eq!(ip_ban_remove_owner["d"]["summary"]["action"], "remove");
    assert_eq!(
        ip_ban_remove_owner["d"]["summary"]["changed_count"],
        json!(1)
    );
    assert!(!contains_ip_field(&ip_ban_remove_owner["d"]));
    let ip_ban_remove_moderator =
        next_event_of_type(&mut moderator_socket, "workspace_ip_ban_sync").await;
    assert_eq!(ip_ban_remove_moderator["d"]["summary"]["action"], "remove");
    assert!(!contains_ip_field(&ip_ban_remove_moderator["d"]));
    let ip_ban_remove_member =
        next_event_of_type(&mut member_socket, "workspace_ip_ban_sync").await;
    assert_eq!(ip_ban_remove_member["d"]["summary"]["action"], "remove");
    assert!(!contains_ip_field(&ip_ban_remove_member["d"]));

    owner_socket
        .close(None)
        .await
        .expect("owner socket close should succeed");
    moderator_socket
        .close(None)
        .await
        .expect("moderator socket close should succeed");
    member_socket
        .close(None)
        .await
        .expect("member socket close should succeed");
    server.abort();
}
