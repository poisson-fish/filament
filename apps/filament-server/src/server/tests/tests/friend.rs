use super::*;

#[allow(clippy::too_many_lines)]
#[tokio::test]
async fn friendship_request_acceptance_and_list_management_work() {
    let app = build_router(&AppConfig::default()).unwrap();
    let alice = register_and_login_as(&app, "alice_friend", "203.0.113.81").await;
    let bob = register_and_login_as(&app, "bob_friend", "203.0.113.82").await;
    let charlie = register_and_login_as(&app, "charlie_friend", "203.0.113.83").await;

    let alice_user_id = user_id_from_me(&app, &alice, "203.0.113.81").await;
    let bob_user_id = user_id_from_me(&app, &bob, "203.0.113.82").await;

    let request_id =
        create_friend_request_for_test(&app, &alice, "203.0.113.81", &bob_user_id).await;

    let (duplicate_status, _) = authed_json_request(
        &app,
        "POST",
        String::from("/friends/requests"),
        &alice.access_token,
        "203.0.113.81",
        Some(json!({ "recipient_user_id": bob_user_id })),
    )
    .await;
    assert_eq!(duplicate_status, StatusCode::BAD_REQUEST);

    let (charlie_accept_status, _) = authed_json_request(
        &app,
        "POST",
        format!("/friends/requests/{request_id}/accept"),
        &charlie.access_token,
        "203.0.113.83",
        None,
    )
    .await;
    assert_eq!(charlie_accept_status, StatusCode::NOT_FOUND);

    let (bob_requests_status, bob_requests_payload) = authed_json_request(
        &app,
        "GET",
        String::from("/friends/requests"),
        &bob.access_token,
        "203.0.113.82",
        None,
    )
    .await;
    assert_eq!(bob_requests_status, StatusCode::OK);
    let bob_requests_payload = bob_requests_payload.unwrap();
    assert_eq!(bob_requests_payload["incoming"].as_array().unwrap().len(), 1);
    assert_eq!(
        bob_requests_payload["incoming"][0]["sender_user_id"]
            .as_str()
            .unwrap(),
        alice_user_id
    );

    let (bob_accept_status, _) = authed_json_request(
        &app,
        "POST",
        format!("/friends/requests/{request_id}/accept"),
        &bob.access_token,
        "203.0.113.82",
        None,
    )
    .await;
    assert_eq!(bob_accept_status, StatusCode::OK);

    let (alice_friends_status, alice_friends_payload) = authed_json_request(
        &app,
        "GET",
        String::from("/friends"),
        &alice.access_token,
        "203.0.113.81",
        None,
    )
    .await;
    assert_eq!(alice_friends_status, StatusCode::OK);
    assert_eq!(alice_friends_payload.unwrap()["friends"].as_array().unwrap().len(), 1);

    let (bob_friends_status, bob_friends_payload) = authed_json_request(
        &app,
        "GET",
        String::from("/friends"),
        &bob.access_token,
        "203.0.113.82",
        None,
    )
    .await;
    assert_eq!(bob_friends_status, StatusCode::OK);
    assert_eq!(
        bob_friends_payload.unwrap()["friends"][0]["user_id"]
            .as_str()
            .unwrap(),
        alice_user_id
    );

    let (remove_status, _) = authed_json_request(
        &app,
        "DELETE",
        format!("/friends/{bob_user_id}"),
        &alice.access_token,
        "203.0.113.81",
        None,
    )
    .await;
    assert_eq!(remove_status, StatusCode::NO_CONTENT);

    let (alice_empty_status, alice_empty_payload) = authed_json_request(
        &app,
        "GET",
        String::from("/friends"),
        &alice.access_token,
        "203.0.113.81",
        None,
    )
    .await;
    assert_eq!(alice_empty_status, StatusCode::OK);
    assert_eq!(
        alice_empty_payload.unwrap()["friends"].as_array().unwrap().len(),
        0
    );
}
