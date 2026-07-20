use std::{env, time::Duration};

use axum::{body::Body, http::Request, http::StatusCode, response::Response};
use filament_core::{DeviceId, UserId};
use filament_e2ee::{
    generate_key_package_batch, generate_last_resort_key_package, MlsDevice, RootIdentityKey,
};
use filament_protocol::{
    ClaimKeyPackageRequest, ClaimKeyPackageResponse, DeviceListResponse, KeyPackageEntry,
    PublishDeviceCertificateRequest, RemoveDeviceResponse, UploadKeyPackagesRequest,
    UploadKeyPackagesResponse,
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

fn postgres_url() -> Option<String> {
    env::var("FILAMENT_TEST_DATABASE_URL").ok()
}

async fn test_app(database_url: String) -> axum::Router {
    build_router_with_db_bootstrap(&AppConfig {
        max_body_bytes: 512 * 1024,
        request_timeout: Duration::from_secs(5),
        rate_limit_requests_per_minute: 500,
        auth_route_requests_per_minute: 200,
        e2ee_device_publish_per_minute: 200,
        e2ee_keypackage_claim_per_minute: 200,
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
    assert_eq!(listed.devices.len(), 1);
    assert_eq!(listed.devices[0].device_id, device_id.to_string());
    assert_eq!(listed.devices[0].root_key_pub, root.public_key_bytes());

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
    assert_eq!(device_removed["d"]["device_count"], 0);

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
    assert!(listed_after_remove.devices.is_empty());

    let resurrect = publish_device(&app, &auth, ip, device_id, &publish_payload(&device)).await;
    assert_eq!(resurrect.status(), StatusCode::FORBIDDEN);

    let claim_removed = app
        .clone()
        .oneshot(claim_request(&auth.access_token, ip, user_id, device_id))
        .await
        .expect("claim against removed device should execute");
    assert_eq!(claim_removed.status(), StatusCode::NOT_FOUND);

    gateway
        .close(None)
        .await
        .expect("gateway should close cleanly");
    gateway_server.abort();
}
