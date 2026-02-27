#![allow(clippy::too_many_lines)]

use std::time::Duration;

use axum::{body::Body, http::Request, http::StatusCode};
use filament_server::{build_router, AppConfig};
use serde::de::DeserializeOwned;
use serde_json::{json, Value};
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
        .body(Body::from(json!({"name":"phase7-chat"}).to_string()))
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

fn find_role(roles: &[Value], name: &str) -> Value {
    roles
        .iter()
        .find(|role| role["name"].as_str() == Some(name))
        .cloned()
        .expect("role should exist")
}

#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn role_crud_assignment_and_reorder_respect_phase7_hierarchy_rules() {
    let app = test_app();
    let owner = register_and_login(&app, "phase7_owner", "203.0.113.201").await;
    let moderator = register_and_login(&app, "phase7_mod", "203.0.113.202").await;
    let member = register_and_login(&app, "phase7_member", "203.0.113.203").await;
    let moderator_id = user_id_from_me(&app, &moderator, "203.0.113.202").await;
    let member_id = user_id_from_me(&app, &member, "203.0.113.203").await;
    let channel = create_channel_context(&app, &owner, "203.0.113.201", "Phase 7 Guild").await;

    for user_id in [&moderator_id, &member_id] {
        let add_member = Request::builder()
            .method("POST")
            .uri(format!("/guilds/{}/members/{user_id}", channel.guild_id))
            .header("authorization", format!("Bearer {}", owner.access_token))
            .header("x-forwarded-for", "203.0.113.201")
            .body(Body::empty())
            .expect("add member request should build");
        let add_member_response = app.clone().oneshot(add_member).await.unwrap();
        assert_eq!(add_member_response.status(), StatusCode::OK);
    }

    let member_list_roles = Request::builder()
        .method("GET")
        .uri(format!("/guilds/{}/roles", channel.guild_id))
        .header("authorization", format!("Bearer {}", member.access_token))
        .header("x-forwarded-for", "203.0.113.203")
        .body(Body::empty())
        .expect("member list roles request should build");
    let member_list_roles_response = app.clone().oneshot(member_list_roles).await.unwrap();
    assert_eq!(member_list_roles_response.status(), StatusCode::OK);
    let member_roles_json: Value = parse_json_body(member_list_roles_response).await;
    assert!(member_roles_json["roles"].as_array().is_some());

    let member_list_members = Request::builder()
        .method("GET")
        .uri(format!("/guilds/{}/members?limit=10", channel.guild_id))
        .header("authorization", format!("Bearer {}", member.access_token))
        .header("x-forwarded-for", "203.0.113.203")
        .body(Body::empty())
        .expect("member list members request should build");
    let member_list_members_response = app.clone().oneshot(member_list_members).await.unwrap();
    assert_eq!(member_list_members_response.status(), StatusCode::OK);
    let member_list_members_json: Value = parse_json_body(member_list_members_response).await;
    let listed_member_ids = member_list_members_json["members"]
        .as_array()
        .expect("members should be an array")
        .iter()
        .filter_map(|entry| entry["user_id"].as_str())
        .collect::<Vec<_>>();
    assert!(listed_member_ids.contains(&member_id.as_str()));

    let promote_mod = Request::builder()
        .method("PATCH")
        .uri(format!(
            "/guilds/{}/members/{}",
            channel.guild_id, moderator_id
        ))
        .header("authorization", format!("Bearer {}", owner.access_token))
        .header("content-type", "application/json")
        .header("x-forwarded-for", "203.0.113.201")
        .body(Body::from(json!({"role":"moderator"}).to_string()))
        .expect("promote request should build");
    let promote_response = app.clone().oneshot(promote_mod).await.unwrap();
    assert_eq!(promote_response.status(), StatusCode::OK);

    let moderator_create_role = Request::builder()
        .method("POST")
        .uri(format!("/guilds/{}/roles", channel.guild_id))
        .header(
            "authorization",
            format!("Bearer {}", moderator.access_token),
        )
        .header("content-type", "application/json")
        .header("x-forwarded-for", "203.0.113.202")
        .body(Body::from(
            json!({"name":"helpers","permissions":["delete_message"],"position":50}).to_string(),
        ))
        .expect("moderator create role request should build");
    let moderator_create_role_response = app.clone().oneshot(moderator_create_role).await.unwrap();
    assert_eq!(
        moderator_create_role_response.status(),
        StatusCode::FORBIDDEN
    );

    let owner_create_helpers_role = Request::builder()
        .method("POST")
        .uri(format!("/guilds/{}/roles", channel.guild_id))
        .header("authorization", format!("Bearer {}", owner.access_token))
        .header("content-type", "application/json")
        .header("x-forwarded-for", "203.0.113.201")
        .body(Body::from(
            json!({"name":"helpers","permissions":["delete_message"],"position":50}).to_string(),
        ))
        .expect("owner create role request should build");
    let owner_create_helpers_response = app
        .clone()
        .oneshot(owner_create_helpers_role)
        .await
        .unwrap();
    assert_eq!(owner_create_helpers_response.status(), StatusCode::OK);
    let helpers_role_json: Value = parse_json_body(owner_create_helpers_response).await;
    let helpers_role_id = helpers_role_json["role_id"].as_str().unwrap().to_owned();

    let owner_create_ops_role = Request::builder()
        .method("POST")
        .uri(format!("/guilds/{}/roles", channel.guild_id))
        .header("authorization", format!("Bearer {}", owner.access_token))
        .header("content-type", "application/json")
        .header("x-forwarded-for", "203.0.113.201")
        .body(Body::from(
            json!({"name":"ops","permissions":["create_message"],"position":45}).to_string(),
        ))
        .expect("owner create role request should build");
    let owner_create_ops_response = app.clone().oneshot(owner_create_ops_role).await.unwrap();
    assert_eq!(owner_create_ops_response.status(), StatusCode::OK);
    let ops_role_json: Value = parse_json_body(owner_create_ops_response).await;
    let ops_role_id = ops_role_json["role_id"].as_str().unwrap().to_owned();

    let assign_helpers = Request::builder()
        .method("POST")
        .uri(format!(
            "/guilds/{}/roles/{}/members/{}",
            channel.guild_id, helpers_role_id, member_id
        ))
        .header("authorization", format!("Bearer {}", owner.access_token))
        .header("x-forwarded-for", "203.0.113.201")
        .body(Body::empty())
        .expect("assign helpers request should build");
    let assign_helpers_response = app.clone().oneshot(assign_helpers).await.unwrap();
    assert_eq!(assign_helpers_response.status(), StatusCode::OK);

    let owner_message = Request::builder()
        .method("POST")
        .uri(format!(
            "/guilds/{}/channels/{}/messages",
            channel.guild_id, channel.channel_id
        ))
        .header("authorization", format!("Bearer {}", owner.access_token))
        .header("content-type", "application/json")
        .header("x-forwarded-for", "203.0.113.201")
        .body(Body::from(
            json!({"content":"owner message one"}).to_string(),
        ))
        .expect("owner message request should build");
    let owner_message_response = app.clone().oneshot(owner_message).await.unwrap();
    assert_eq!(owner_message_response.status(), StatusCode::OK);
    let owner_message_json: Value = parse_json_body(owner_message_response).await;
    let owner_message_id = owner_message_json["message_id"]
        .as_str()
        .unwrap()
        .to_owned();

    let member_delete_with_role = Request::builder()
        .method("DELETE")
        .uri(format!(
            "/guilds/{}/channels/{}/messages/{}",
            channel.guild_id, channel.channel_id, owner_message_id
        ))
        .header("authorization", format!("Bearer {}", member.access_token))
        .header("x-forwarded-for", "203.0.113.203")
        .body(Body::empty())
        .expect("member delete request should build");
    let member_delete_with_role_response =
        app.clone().oneshot(member_delete_with_role).await.unwrap();
    assert_eq!(
        member_delete_with_role_response.status(),
        StatusCode::NO_CONTENT
    );

    let second_owner_message = Request::builder()
        .method("POST")
        .uri(format!(
            "/guilds/{}/channels/{}/messages",
            channel.guild_id, channel.channel_id
        ))
        .header("authorization", format!("Bearer {}", owner.access_token))
        .header("content-type", "application/json")
        .header("x-forwarded-for", "203.0.113.201")
        .body(Body::from(
            json!({"content":"owner message two"}).to_string(),
        ))
        .expect("second owner message request should build");
    let second_owner_message_response = app.clone().oneshot(second_owner_message).await.unwrap();
    assert_eq!(second_owner_message_response.status(), StatusCode::OK);
    let second_owner_message_json: Value = parse_json_body(second_owner_message_response).await;
    let second_owner_message_id = second_owner_message_json["message_id"]
        .as_str()
        .unwrap()
        .to_owned();

    let unassign_helpers = Request::builder()
        .method("DELETE")
        .uri(format!(
            "/guilds/{}/roles/{}/members/{}",
            channel.guild_id, helpers_role_id, member_id
        ))
        .header("authorization", format!("Bearer {}", owner.access_token))
        .header("x-forwarded-for", "203.0.113.201")
        .body(Body::empty())
        .expect("unassign helpers request should build");
    let unassign_helpers_response = app.clone().oneshot(unassign_helpers).await.unwrap();
    assert_eq!(unassign_helpers_response.status(), StatusCode::OK);

    let member_delete_without_role = Request::builder()
        .method("DELETE")
        .uri(format!(
            "/guilds/{}/channels/{}/messages/{}",
            channel.guild_id, channel.channel_id, second_owner_message_id
        ))
        .header("authorization", format!("Bearer {}", member.access_token))
        .header("x-forwarded-for", "203.0.113.203")
        .body(Body::empty())
        .expect("member delete request should build");
    let member_delete_without_role_response = app
        .clone()
        .oneshot(member_delete_without_role)
        .await
        .unwrap();
    assert_eq!(
        member_delete_without_role_response.status(),
        StatusCode::FORBIDDEN
    );

    let reorder_roles = Request::builder()
        .method("POST")
        .uri(format!("/guilds/{}/roles/reorder", channel.guild_id))
        .header("authorization", format!("Bearer {}", owner.access_token))
        .header("content-type", "application/json")
        .header("x-forwarded-for", "203.0.113.201")
        .body(Body::from(
            json!({"role_ids":[ops_role_id.clone(), helpers_role_id.clone()]}).to_string(),
        ))
        .expect("reorder roles request should build");
    let reorder_roles_response = app.clone().oneshot(reorder_roles).await.unwrap();
    assert_eq!(reorder_roles_response.status(), StatusCode::OK);

    let list_roles = Request::builder()
        .method("GET")
        .uri(format!("/guilds/{}/roles", channel.guild_id))
        .header("authorization", format!("Bearer {}", owner.access_token))
        .header("x-forwarded-for", "203.0.113.201")
        .body(Body::empty())
        .expect("list roles request should build");
    let list_roles_response = app.clone().oneshot(list_roles).await.unwrap();
    assert_eq!(list_roles_response.status(), StatusCode::OK);
    let roles_json: Value = parse_json_body(list_roles_response).await;
    let roles = roles_json["roles"]
        .as_array()
        .expect("roles should be an array");
    let helpers_role = find_role(roles, "helpers");
    let ops_role = find_role(roles, "ops");
    let helpers_position = helpers_role["position"].as_i64().unwrap();
    let ops_position = ops_role["position"].as_i64().unwrap();
    assert!(ops_position > helpers_position);
}

#[tokio::test]
async fn system_role_guards_block_workspace_owner_escalation_and_invalid_permission_input() {
    let app = test_app();
    let owner = register_and_login(&app, "phase7_owner_guard", "203.0.113.211").await;
    let moderator = register_and_login(&app, "phase7_mod_guard", "203.0.113.212").await;
    let owner_id = user_id_from_me(&app, &owner, "203.0.113.211").await;
    let moderator_id = user_id_from_me(&app, &moderator, "203.0.113.212").await;
    let channel =
        create_channel_context(&app, &owner, "203.0.113.211", "Phase 7 Guard Guild").await;

    let add_moderator = Request::builder()
        .method("POST")
        .uri(format!(
            "/guilds/{}/members/{}",
            channel.guild_id, moderator_id
        ))
        .header("authorization", format!("Bearer {}", owner.access_token))
        .header("x-forwarded-for", "203.0.113.211")
        .body(Body::empty())
        .expect("add moderator request should build");
    let add_moderator_response = app.clone().oneshot(add_moderator).await.unwrap();
    assert_eq!(add_moderator_response.status(), StatusCode::OK);

    let promote_mod = Request::builder()
        .method("PATCH")
        .uri(format!(
            "/guilds/{}/members/{}",
            channel.guild_id, moderator_id
        ))
        .header("authorization", format!("Bearer {}", owner.access_token))
        .header("content-type", "application/json")
        .header("x-forwarded-for", "203.0.113.211")
        .body(Body::from(json!({"role":"moderator"}).to_string()))
        .expect("promote request should build");
    let promote_response = app.clone().oneshot(promote_mod).await.unwrap();
    assert_eq!(promote_response.status(), StatusCode::OK);

    let list_roles = Request::builder()
        .method("GET")
        .uri(format!("/guilds/{}/roles", channel.guild_id))
        .header("authorization", format!("Bearer {}", owner.access_token))
        .header("x-forwarded-for", "203.0.113.211")
        .body(Body::empty())
        .expect("list roles request should build");
    let list_roles_response = app.clone().oneshot(list_roles).await.unwrap();
    assert_eq!(list_roles_response.status(), StatusCode::OK);
    let roles_json: Value = parse_json_body(list_roles_response).await;
    let roles = roles_json["roles"]
        .as_array()
        .expect("roles should be an array");
    let workspace_owner_role_id = find_role(roles, "workspace_owner")["role_id"]
        .as_str()
        .unwrap()
        .to_owned();

    let assign_workspace_owner_by_mod = Request::builder()
        .method("POST")
        .uri(format!(
            "/guilds/{}/roles/{}/members/{}",
            channel.guild_id, workspace_owner_role_id, moderator_id
        ))
        .header(
            "authorization",
            format!("Bearer {}", moderator.access_token),
        )
        .header("x-forwarded-for", "203.0.113.212")
        .body(Body::empty())
        .expect("assign workspace owner request should build");
    let assign_workspace_owner_by_mod_response = app
        .clone()
        .oneshot(assign_workspace_owner_by_mod)
        .await
        .unwrap();
    assert_eq!(
        assign_workspace_owner_by_mod_response.status(),
        StatusCode::FORBIDDEN
    );

    let delete_workspace_owner_role = Request::builder()
        .method("DELETE")
        .uri(format!(
            "/guilds/{}/roles/{}",
            channel.guild_id, workspace_owner_role_id
        ))
        .header("authorization", format!("Bearer {}", owner.access_token))
        .header("x-forwarded-for", "203.0.113.211")
        .body(Body::empty())
        .expect("delete workspace owner role request should build");
    let delete_workspace_owner_role_response = app
        .clone()
        .oneshot(delete_workspace_owner_role)
        .await
        .unwrap();
    assert_eq!(
        delete_workspace_owner_role_response.status(),
        StatusCode::FORBIDDEN
    );

    let unassign_last_workspace_owner = Request::builder()
        .method("DELETE")
        .uri(format!(
            "/guilds/{}/roles/{}/members/{}",
            channel.guild_id, workspace_owner_role_id, owner_id
        ))
        .header("authorization", format!("Bearer {}", owner.access_token))
        .header("x-forwarded-for", "203.0.113.211")
        .body(Body::empty())
        .expect("unassign workspace owner request should build");
    let unassign_last_workspace_owner_response = app
        .clone()
        .oneshot(unassign_last_workspace_owner)
        .await
        .unwrap();
    assert_eq!(
        unassign_last_workspace_owner_response.status(),
        StatusCode::FORBIDDEN
    );

    let invalid_permission_role = Request::builder()
        .method("POST")
        .uri(format!("/guilds/{}/roles", channel.guild_id))
        .header("authorization", format!("Bearer {}", owner.access_token))
        .header("content-type", "application/json")
        .header("x-forwarded-for", "203.0.113.211")
        .body(Body::from(
            json!({"name":"bad-perms","permissions":["not_a_permission"]}).to_string(),
        ))
        .expect("invalid permission request should build");
    let invalid_permission_role_response =
        app.clone().oneshot(invalid_permission_role).await.unwrap();
    assert_eq!(
        invalid_permission_role_response.status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
}
