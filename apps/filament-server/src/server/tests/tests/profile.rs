use super::*;

#[tokio::test]
async fn profile_update_changes_username_and_about() {
    let app = build_router(&AppConfig::default()).unwrap();
    let auth = register_and_login_as(&app, "profile_owner", "203.0.113.141").await;

    let (update_status, update_payload) = authed_json_request(
        &app,
        "PATCH",
        String::from("/users/me/profile"),
        &auth.access_token,
        "203.0.113.141",
        Some(json!({
            "username":"profile_owner_next",
            "about_markdown":"hello **team**"
        })),
    )
    .await;
    assert_eq!(update_status, StatusCode::OK);
    let updated = update_payload.expect("profile update payload");
    assert_eq!(updated["username"], "profile_owner_next");
    assert_eq!(updated["about_markdown"], "hello **team**");
    assert!(updated["about_markdown_tokens"]
        .as_array()
        .is_some_and(|tokens| !tokens.is_empty()));

    let (me_status, me_payload) = authed_json_request(
        &app,
        "GET",
        String::from("/auth/me"),
        &auth.access_token,
        "203.0.113.141",
        None,
    )
    .await;
    assert_eq!(me_status, StatusCode::OK);
    let me = me_payload.expect("me payload");
    assert_eq!(me["username"], "profile_owner_next");
    assert_eq!(me["about_markdown"], "hello **team**");
    assert!(me["about_markdown_tokens"]
        .as_array()
        .is_some_and(|tokens| !tokens.is_empty()));

    let old_login = Request::builder()
        .method("POST")
        .uri("/auth/login")
        .header("content-type", "application/json")
        .header("x-forwarded-for", "203.0.113.141")
        .body(Body::from(
            json!({"username":"profile_owner","password":"super-secure-password"}).to_string(),
        ))
        .unwrap();
    let old_login_response = app.clone().oneshot(old_login).await.unwrap();
    assert_eq!(old_login_response.status(), StatusCode::UNAUTHORIZED);

    let new_login = Request::builder()
        .method("POST")
        .uri("/auth/login")
        .header("content-type", "application/json")
        .header("x-forwarded-for", "203.0.113.141")
        .body(Body::from(
            json!({"username":"profile_owner_next","password":"super-secure-password"})
                .to_string(),
        ))
        .unwrap();
    let new_login_response = app.oneshot(new_login).await.unwrap();
    assert_eq!(new_login_response.status(), StatusCode::OK);
}

#[tokio::test]
async fn profile_avatar_upload_and_download_round_trip() {
    const PNG_1X1: &[u8] = &[
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48,
        0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x04, 0x00, 0x00,
        0x00, 0xB5, 0x1C, 0x0C, 0x02, 0x00, 0x00, 0x00, 0x0B, 0x49, 0x44, 0x41, 0x54, 0x78,
        0xDA, 0x63, 0xFC, 0x5F, 0x0F, 0x00, 0x02, 0x7F, 0x01, 0xF5, 0x87, 0xCB, 0xD9, 0x1F,
        0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
    ];

    let app = build_router(&AppConfig::default()).unwrap();
    let auth = register_and_login_as(&app, "avatar_owner", "203.0.113.142").await;
    let user_id = user_id_from_me(&app, &auth, "203.0.113.142").await;

    let upload = Request::builder()
        .method("POST")
        .uri("/users/me/profile/avatar")
        .header("authorization", format!("Bearer {}", auth.access_token))
        .header("content-type", "image/png")
        .header("x-forwarded-for", "203.0.113.142")
        .body(Body::from(PNG_1X1.to_vec()))
        .unwrap();
    let upload_response = app.clone().oneshot(upload).await.unwrap();
    assert_eq!(upload_response.status(), StatusCode::OK);
    let upload_body = axum::body::to_bytes(upload_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let upload_json: Value = serde_json::from_slice(&upload_body).unwrap();
    assert!(upload_json["avatar_version"]
        .as_i64()
        .is_some_and(|value| value > 0));

    let download = Request::builder()
        .method("GET")
        .uri(format!("/users/{user_id}/avatar"))
        .header("x-forwarded-for", "203.0.113.142")
        .body(Body::empty())
        .unwrap();
    let download_response = app.clone().oneshot(download).await.unwrap();
    assert_eq!(download_response.status(), StatusCode::OK);
    assert_eq!(
        download_response
            .headers()
            .get("content-type")
            .and_then(|value| value.to_str().ok()),
        Some("image/png")
    );
    let bytes = axum::body::to_bytes(download_response.into_body(), usize::MAX)
        .await
        .unwrap();
    assert_eq!(bytes.as_ref(), PNG_1X1);

    let bad_upload = Request::builder()
        .method("POST")
        .uri("/users/me/profile/avatar")
        .header("authorization", format!("Bearer {}", auth.access_token))
        .header("content-type", "text/plain")
        .header("x-forwarded-for", "203.0.113.142")
        .body(Body::from("not-an-image"))
        .unwrap();
    let bad_response = app.oneshot(bad_upload).await.unwrap();
    assert_eq!(bad_response.status(), StatusCode::BAD_REQUEST);
}
