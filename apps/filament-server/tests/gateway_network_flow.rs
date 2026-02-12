use std::time::Duration;

use axum::{body::Body, http::Request, http::StatusCode};
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
    let event = socket
        .next()
        .await
        .expect("event should be emitted")
        .expect("event should decode");
    let text = event.into_text().expect("event should be text");
    serde_json::from_str(&text).expect("event should be valid json")
}

async fn next_event_of_type(
    socket: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
    event_type: &str,
) -> Value {
    for _ in 0..8 {
        let event = next_text_event(socket).await;
        if event["t"] == event_type {
            return event;
        }
    }
    panic!("expected event type {event_type}");
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

    let broadcast_json = next_text_event(&mut socket).await;
    assert_eq!(broadcast_json["t"], "message_create");
    assert_eq!(broadcast_json["d"]["content"], "hello over network");

    socket
        .close(None)
        .await
        .expect("socket close should succeed");
    server.abort();
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
