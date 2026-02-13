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

fn test_app() -> axum::Router {
    build_router(&AppConfig {
        max_body_bytes: 1024 * 64,
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

async fn parse_json_body<T: DeserializeOwned>(response: axum::response::Response) -> T {
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("response body should be readable");
    serde_json::from_slice(&body).expect("response body should be valid json")
}

async fn register_and_login(app: &axum::Router, username: &str, ip: &str) -> AuthResponse {
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

async fn user_id_from_me(app: &axum::Router, auth: &AuthResponse, ip: &str) -> String {
    let me = Request::builder()
        .method("GET")
        .uri("/auth/me")
        .header("authorization", format!("Bearer {}", auth.access_token))
        .header("x-forwarded-for", ip)
        .body(Body::empty())
        .expect("me request should build");
    let me_response = app.clone().oneshot(me).await.expect("me should execute");
    assert_eq!(me_response.status(), StatusCode::OK);
    let me_json: Value = parse_json_body(me_response).await;
    me_json["user_id"]
        .as_str()
        .expect("user id should exist")
        .to_owned()
}

async fn create_channel_context(
    app: &axum::Router,
    auth: &AuthResponse,
    ip: &str,
    name: &str,
) -> ChannelRef {
    let create_guild = Request::builder()
        .method("POST")
        .uri("/guilds")
        .header("authorization", format!("Bearer {}", auth.access_token))
        .header("content-type", "application/json")
        .header("x-forwarded-for", ip)
        .body(Body::from(json!({"name":name}).to_string()))
        .expect("create guild request should build");
    let guild_response = app
        .clone()
        .oneshot(create_guild)
        .await
        .expect("create guild should execute");
    assert_eq!(guild_response.status(), StatusCode::OK);
    let guild_json: Value = parse_json_body(guild_response).await;
    let guild_id = guild_json["guild_id"].as_str().unwrap().to_owned();

    let create_channel = Request::builder()
        .method("POST")
        .uri(format!("/guilds/{guild_id}/channels"))
        .header("authorization", format!("Bearer {}", auth.access_token))
        .header("content-type", "application/json")
        .header("x-forwarded-for", ip)
        .body(Body::from(json!({"name":"phase4-chat"}).to_string()))
        .expect("create channel request should build");
    let channel_response = app
        .clone()
        .oneshot(create_channel)
        .await
        .expect("create channel should execute");
    assert_eq!(channel_response.status(), StatusCode::OK);
    let channel_json: Value = parse_json_body(channel_response).await;

    ChannelRef {
        guild_id,
        channel_id: channel_json["channel_id"].as_str().unwrap().to_owned(),
    }
}

async fn ws_next_text(
    socket: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
) -> Value {
    loop {
        let event = socket
            .next()
            .await
            .expect("socket should yield event")
            .expect("socket event should decode");
        if event.is_ping() || event.is_pong() {
            continue;
        }
        let text = event.into_text().expect("event should be text");
        return serde_json::from_str(&text).expect("event should be valid json");
    }
}

async fn ws_wait_for_type(
    socket: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
    event_type: &str,
) -> Value {
    for _ in 0..8 {
        let event = ws_next_text(socket).await;
        if event["t"] == event_type {
            return event;
        }
    }
    panic!("missing expected event type {event_type}");
}

#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn roles_overrides_and_reactions_enforce_privilege_boundaries() {
    let app = test_app();
    let owner = register_and_login(&app, "phase4_owner", "203.0.113.91").await;
    let moderator = register_and_login(&app, "phase4_mod", "203.0.113.92").await;
    let member = register_and_login(&app, "phase4_member", "203.0.113.93").await;

    let owner_id = user_id_from_me(&app, &owner, "203.0.113.91").await;
    let moderator_id = user_id_from_me(&app, &moderator, "203.0.113.92").await;
    let member_id = user_id_from_me(&app, &member, "203.0.113.93").await;
    let channel = create_channel_context(&app, &owner, "203.0.113.91", "Phase 4 Guild").await;

    for user_id in [&moderator_id, &member_id] {
        let add_member = Request::builder()
            .method("POST")
            .uri(format!("/guilds/{}/members/{user_id}", channel.guild_id))
            .header("authorization", format!("Bearer {}", owner.access_token))
            .header("x-forwarded-for", "203.0.113.91")
            .body(Body::empty())
            .expect("add member request should build");
        let add_member_response = app.clone().oneshot(add_member).await.unwrap();
        assert_eq!(add_member_response.status(), StatusCode::OK);
    }

    let promote_mod = Request::builder()
        .method("PATCH")
        .uri(format!(
            "/guilds/{}/members/{}",
            channel.guild_id, moderator_id
        ))
        .header("authorization", format!("Bearer {}", owner.access_token))
        .header("content-type", "application/json")
        .header("x-forwarded-for", "203.0.113.91")
        .body(Body::from(json!({"role":"moderator"}).to_string()))
        .expect("promote request should build");
    let promote_response = app.clone().oneshot(promote_mod).await.unwrap();
    assert_eq!(promote_response.status(), StatusCode::OK);

    let ban_owner_by_mod = Request::builder()
        .method("POST")
        .uri(format!(
            "/guilds/{}/members/{}/ban",
            channel.guild_id, owner_id
        ))
        .header(
            "authorization",
            format!("Bearer {}", moderator.access_token),
        )
        .header("x-forwarded-for", "203.0.113.92")
        .body(Body::empty())
        .expect("ban request should build");
    let ban_owner_response = app.clone().oneshot(ban_owner_by_mod).await.unwrap();
    assert_eq!(ban_owner_response.status(), StatusCode::FORBIDDEN);

    let deny_member_write = Request::builder()
        .method("POST")
        .uri(format!(
            "/guilds/{}/channels/{}/overrides/member",
            channel.guild_id, channel.channel_id
        ))
        .header("authorization", format!("Bearer {}", owner.access_token))
        .header("content-type", "application/json")
        .header("x-forwarded-for", "203.0.113.91")
        .body(Body::from(
            json!({"allow":[],"deny":["create_message"]}).to_string(),
        ))
        .expect("override request should build");
    let deny_member_response = app.clone().oneshot(deny_member_write).await.unwrap();
    assert_eq!(deny_member_response.status(), StatusCode::OK);

    let member_message = Request::builder()
        .method("POST")
        .uri(format!(
            "/guilds/{}/channels/{}/messages",
            channel.guild_id, channel.channel_id
        ))
        .header("authorization", format!("Bearer {}", member.access_token))
        .header("content-type", "application/json")
        .header("x-forwarded-for", "203.0.113.93")
        .body(Body::from(json!({"content":"blocked"}).to_string()))
        .expect("member message request should build");
    let member_message_response = app.clone().oneshot(member_message).await.unwrap();
    assert_eq!(member_message_response.status(), StatusCode::FORBIDDEN);

    let moderator_message = Request::builder()
        .method("POST")
        .uri(format!(
            "/guilds/{}/channels/{}/messages",
            channel.guild_id, channel.channel_id
        ))
        .header(
            "authorization",
            format!("Bearer {}", moderator.access_token),
        )
        .header("content-type", "application/json")
        .header("x-forwarded-for", "203.0.113.92")
        .body(Body::from(json!({"content":"allowed"}).to_string()))
        .expect("moderator message request should build");
    let moderator_message_response = app.clone().oneshot(moderator_message).await.unwrap();
    assert_eq!(moderator_message_response.status(), StatusCode::OK);
    let message_json: Value = parse_json_body(moderator_message_response).await;
    let message_id = message_json["message_id"].as_str().unwrap().to_owned();

    let member_reaction = Request::builder()
        .method("POST")
        .uri(format!(
            "/guilds/{}/channels/{}/messages/{}/reactions/%F0%9F%91%8D",
            channel.guild_id, channel.channel_id, message_id
        ))
        .header("authorization", format!("Bearer {}", member.access_token))
        .header("x-forwarded-for", "203.0.113.93")
        .body(Body::empty())
        .expect("member reaction request should build");
    let member_reaction_response = app.clone().oneshot(member_reaction).await.unwrap();
    assert_eq!(member_reaction_response.status(), StatusCode::FORBIDDEN);

    let add_reaction = Request::builder()
        .method("POST")
        .uri(format!(
            "/guilds/{}/channels/{}/messages/{}/reactions/%F0%9F%91%8D",
            channel.guild_id, channel.channel_id, message_id
        ))
        .header(
            "authorization",
            format!("Bearer {}", moderator.access_token),
        )
        .header("x-forwarded-for", "203.0.113.92")
        .body(Body::empty())
        .expect("reaction add request should build");
    let add_reaction_response = app.clone().oneshot(add_reaction).await.unwrap();
    assert_eq!(add_reaction_response.status(), StatusCode::OK);
    let reaction_json: Value = parse_json_body(add_reaction_response).await;
    assert_eq!(reaction_json["count"], 1);

    let remove_reaction = Request::builder()
        .method("DELETE")
        .uri(format!(
            "/guilds/{}/channels/{}/messages/{}/reactions/%F0%9F%91%8D",
            channel.guild_id, channel.channel_id, message_id
        ))
        .header(
            "authorization",
            format!("Bearer {}", moderator.access_token),
        )
        .header("x-forwarded-for", "203.0.113.92")
        .body(Body::empty())
        .expect("reaction delete request should build");
    let remove_reaction_response = app.oneshot(remove_reaction).await.unwrap();
    assert_eq!(remove_reaction_response.status(), StatusCode::OK);
    let reaction_deleted_json: Value = parse_json_body(remove_reaction_response).await;
    assert_eq!(reaction_deleted_json["count"], 0);
}

#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn presence_events_do_not_leak_across_guilds() {
    let app = test_app();

    let owner = register_and_login(&app, "phase4_presence_owner", "203.0.113.94").await;
    let mod_user = register_and_login(&app, "phase4_presence_mod", "203.0.113.95").await;
    let outsider = register_and_login(&app, "phase4_presence_out", "203.0.113.96").await;

    let mod_id = user_id_from_me(&app, &mod_user, "203.0.113.95").await;
    let guild_a = create_channel_context(&app, &owner, "203.0.113.94", "Presence Guild A").await;
    let guild_b = create_channel_context(&app, &outsider, "203.0.113.96", "Presence Guild B").await;

    let add_mod = Request::builder()
        .method("POST")
        .uri(format!("/guilds/{}/members/{mod_id}", guild_a.guild_id))
        .header("authorization", format!("Bearer {}", owner.access_token))
        .header("x-forwarded-for", "203.0.113.94")
        .body(Body::empty())
        .expect("add mod request should build");
    let add_mod_response = app.clone().oneshot(add_mod).await.unwrap();
    assert_eq!(add_mod_response.status(), StatusCode::OK);

    let promote_mod = Request::builder()
        .method("PATCH")
        .uri(format!("/guilds/{}/members/{mod_id}", guild_a.guild_id))
        .header("authorization", format!("Bearer {}", owner.access_token))
        .header("content-type", "application/json")
        .header("x-forwarded-for", "203.0.113.94")
        .body(Body::from(json!({"role":"moderator"}).to_string()))
        .expect("promote mod request should build");
    let promote_mod_response = app.clone().oneshot(promote_mod).await.unwrap();
    assert_eq!(promote_mod_response.status(), StatusCode::OK);

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server =
        tokio::spawn(async move { axum::serve(listener, app).await.expect("server should run") });

    let owner_url = format!("ws://{addr}/gateway/ws?access_token={}", owner.access_token);
    let outsider_url = format!(
        "ws://{addr}/gateway/ws?access_token={}",
        outsider.access_token
    );
    let mod_url = format!(
        "ws://{addr}/gateway/ws?access_token={}",
        mod_user.access_token
    );

    let mut owner_req = owner_url.into_client_request().unwrap();
    owner_req.headers_mut().insert(
        "x-forwarded-for",
        http::HeaderValue::from_static("203.0.113.94"),
    );
    let (mut owner_ws, _) = connect_async(owner_req)
        .await
        .expect("owner ws should connect");
    let _ = ws_wait_for_type(&mut owner_ws, "ready").await;
    owner_ws
        .send(Message::Text(
            json!({
                "v":1,
                "t":"subscribe",
                "d":{"guild_id":guild_a.guild_id,"channel_id":guild_a.channel_id}
            })
            .to_string(),
        ))
        .await
        .unwrap();
    let _ = ws_wait_for_type(&mut owner_ws, "subscribed").await;

    let mut outsider_req = outsider_url.into_client_request().unwrap();
    outsider_req.headers_mut().insert(
        "x-forwarded-for",
        http::HeaderValue::from_static("203.0.113.96"),
    );
    let (mut outsider_ws, _) = connect_async(outsider_req)
        .await
        .expect("outsider ws should connect");
    let _ = ws_wait_for_type(&mut outsider_ws, "ready").await;
    outsider_ws
        .send(Message::Text(
            json!({
                "v":1,
                "t":"subscribe",
                "d":{"guild_id":guild_b.guild_id,"channel_id":guild_b.channel_id}
            })
            .to_string(),
        ))
        .await
        .unwrap();
    let _ = ws_wait_for_type(&mut outsider_ws, "subscribed").await;

    let mut mod_req = mod_url.into_client_request().unwrap();
    mod_req.headers_mut().insert(
        "x-forwarded-for",
        http::HeaderValue::from_static("203.0.113.95"),
    );
    let (mut mod_ws, _) = connect_async(mod_req).await.expect("mod ws should connect");
    let _ = ws_wait_for_type(&mut mod_ws, "ready").await;
    mod_ws
        .send(Message::Text(
            json!({
                "v":1,
                "t":"subscribe",
                "d":{"guild_id":guild_a.guild_id,"channel_id":guild_a.channel_id}
            })
            .to_string(),
        ))
        .await
        .unwrap();

    let owner_presence = ws_wait_for_type(&mut owner_ws, "presence_update").await;
    assert_eq!(owner_presence["d"]["guild_id"], guild_a.guild_id);
    assert_eq!(owner_presence["d"]["status"], "online");

    let outsider_next = tokio::time::timeout(Duration::from_millis(300), outsider_ws.next()).await;
    if let Ok(Some(Ok(message))) = outsider_next {
        if message.is_text() {
            let payload: Value = serde_json::from_str(message.to_text().unwrap()).unwrap();
            assert_ne!(payload["t"], "presence_update");
        }
    }

    let _ = mod_ws.close(None).await;
    let _ = owner_ws.close(None).await;
    let _ = outsider_ws.close(None).await;
    server.abort();
}
