use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::routing::{get, post};
use http_body_util::BodyExt;
use tokio::sync::Mutex;
use tower::ServiceExt;

use skyjo_server::{AppState, AppStateInner};

fn test_app() -> Router {
    let model_path =
        std::env::temp_dir().join(format!("skyjo_test_model_{}.json", std::process::id()));
    let genetic_state = Arc::new(Mutex::new(
        skyjo_server::genetic::GeneticTrainingState::load_or_new(model_path),
    ));
    let state: AppState = Arc::new(AppStateInner {
        lobby: skyjo_server::lobby::Lobby::new(100),
        genetic: genetic_state,
    });

    // Mirror the routes from main.rs
    let api_routes = Router::new()
        .route("/rooms", post(skyjo_server::create_room))
        .route("/rooms/{code}", get(skyjo_server::room_info))
        .route("/rooms/{code}/join", post(skyjo_server::join_room))
        .route("/genetic/status", get(skyjo_server::genetic_status));

    Router::new().nest("/api", api_routes).with_state(state)
}

async fn body_json(response: axum::http::Response<Body>) -> serde_json::Value {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

// --- Room creation tests ---

#[tokio::test]
async fn create_room_returns_200_with_code_and_token() {
    let app = test_app();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/rooms")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"player_name":"Alice","num_players":2}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = body_json(response).await;
    assert!(json["room_code"].is_string());
    assert_eq!(json["room_code"].as_str().unwrap().len(), 6);
    assert!(json["session_token"].is_string());
    assert!(!json["session_token"].as_str().unwrap().is_empty());
    assert_eq!(json["player_index"], 0);
}

#[tokio::test]
async fn create_room_with_invalid_player_count_returns_400() {
    let app = test_app();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/rooms")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"player_name":"Alice","num_players":1}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn create_room_with_too_many_players_returns_400() {
    let app = test_app();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/rooms")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"player_name":"Alice","num_players":9}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

// --- Room info tests ---

#[tokio::test]
async fn get_room_info_for_valid_room() {
    let app = test_app();

    // Create a room first
    let create_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/rooms")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"player_name":"Alice","num_players":3}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(create_resp.status(), StatusCode::OK);
    let create_json = body_json(create_resp).await;
    let code = create_json["room_code"].as_str().unwrap();

    // Get room info
    let info_resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/rooms/{code}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(info_resp.status(), StatusCode::OK);
    let info_json = body_json(info_resp).await;
    assert_eq!(info_json["room_code"].as_str().unwrap(), code);
    assert_eq!(info_json["num_players"], 3);
    assert_eq!(info_json["players_joined"], 1);
    assert_eq!(info_json["phase"], "lobby");
}

#[tokio::test]
async fn get_room_info_for_nonexistent_room_returns_404() {
    let app = test_app();

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/rooms/ZZZZZZ")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

// --- Join room tests ---

#[tokio::test]
async fn join_room_succeeds_with_valid_code() {
    let app = test_app();

    // Create a room
    let create_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/rooms")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"player_name":"Alice","num_players":2}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    let create_json = body_json(create_resp).await;
    let code = create_json["room_code"].as_str().unwrap();

    // Join the room
    let join_resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/rooms/{code}/join"))
                .header("content-type", "application/json")
                .body(Body::from(r#"{"player_name":"Bob"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(join_resp.status(), StatusCode::OK);
    let join_json = body_json(join_resp).await;
    assert!(join_json["session_token"].is_string());
    assert!(!join_json["session_token"].as_str().unwrap().is_empty());
    assert_eq!(join_json["player_index"], 1);
}

#[tokio::test]
async fn join_room_fails_with_invalid_code() {
    let app = test_app();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/rooms/BADCOD/join")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"player_name":"Bob"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

// --- Genetic status test ---

#[tokio::test]
async fn genetic_status_returns_valid_response() {
    let app = test_app();

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/genetic/status")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = body_json(response).await;
    assert_eq!(json["is_training"], false);
    assert!(json["generation"].is_number());
}
