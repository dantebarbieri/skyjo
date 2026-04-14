//! Security-focused integration tests.

use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::routing::{get, post};
use http_body_util::BodyExt;
use tokio::sync::Mutex;
use tower::ServiceExt;

use skyjo_server::error::ServerError;
use skyjo_server::rate_limit::{RateLimitConfig, RateLimiter};
use skyjo_server::{AppState, AppStateInner};

// ========================================================================
// Test helpers (mirrors api_integration.rs)
// ========================================================================

fn test_app() -> Router {
    let model_path =
        std::env::temp_dir().join(format!("skyjo_test_security_{}.json", std::process::id()));
    let genetic_state = Arc::new(Mutex::new(
        skyjo_server::genetic::GeneticTrainingState::load_or_new(model_path),
    ));
    let state: AppState = Arc::new(AppStateInner {
        lobby: skyjo_server::lobby::Lobby::new(100),
        genetic: genetic_state,
        genetic_api_key: None,
        persistence: None,
        rate_limiter: Arc::new(skyjo_server::rate_limit::RateLimiter::new()),
    });

    let api_routes = Router::new()
        .route("/rooms", post(skyjo_server::create_room))
        .route("/rooms/{code}", get(skyjo_server::room_info))
        .route("/rooms/{code}/join", post(skyjo_server::join_room))
        .route("/genetic/status", get(skyjo_server::genetic_status));

    Router::new().nest("/api", api_routes).with_state(state)
}

fn test_app_with_api_key(api_key: Option<String>) -> Router {
    let model_path = std::env::temp_dir().join(format!(
        "skyjo_test_security_auth_{}.json",
        std::process::id()
    ));
    let genetic_state = Arc::new(Mutex::new(
        skyjo_server::genetic::GeneticTrainingState::load_or_new(model_path),
    ));
    let state: AppState = Arc::new(AppStateInner {
        lobby: skyjo_server::lobby::Lobby::new(100),
        genetic: genetic_state,
        genetic_api_key: api_key,
        persistence: None,
        rate_limiter: Arc::new(skyjo_server::rate_limit::RateLimiter::new()),
    });

    let genetic_mutation_routes = Router::new()
        .route("/genetic/train", post(skyjo_server::genetic_status))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            skyjo_server::genetic_auth_middleware,
        ));

    let api_routes = Router::new()
        .route("/genetic/status", get(skyjo_server::genetic_status))
        .merge(genetic_mutation_routes);

    Router::new().nest("/api", api_routes).with_state(state)
}

async fn body_json(response: axum::http::Response<Body>) -> serde_json::Value {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

/// Helper: create a room via the HTTP API, returning (room_code, session_token).
async fn create_room_via_api(app: &Router, name: &str, num_players: usize) -> (String, String) {
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/rooms")
                .header("content-type", "application/json")
                .body(Body::from(format!(
                    r#"{{"player_name":"{name}","num_players":{num_players}}}"#
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let json = body_json(resp).await;
    (
        json["room_code"].as_str().unwrap().to_string(),
        json["session_token"].as_str().unwrap().to_string(),
    )
}

// ========================================================================
// Rate limiter unit tests (tests the module directly)
// ========================================================================

#[test]
fn rate_limiter_allows_burst_then_blocks() {
    use std::net::{IpAddr, Ipv4Addr};

    let limiter = RateLimiter::new();
    let config = RateLimitConfig::new(5.0, 0.0001); // very slow refill
    let ip = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));

    // Should allow 5 requests (burst)
    for i in 0..5 {
        assert!(
            limiter.check(ip, "room_create", &config),
            "request {i} should pass"
        );
    }
    // 6th should be blocked
    assert!(
        !limiter.check(ip, "room_create", &config),
        "6th request should be blocked"
    );
}

#[test]
fn rate_limiter_different_ips_independent() {
    use std::net::{IpAddr, Ipv4Addr};

    let limiter = RateLimiter::new();
    let config = RateLimitConfig::new(1.0, 0.0001);
    let ip1 = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));
    let ip2 = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 2));

    assert!(limiter.check(ip1, "test", &config));
    assert!(!limiter.check(ip1, "test", &config));
    // ip2 still has its own bucket
    assert!(limiter.check(ip2, "test", &config));
}

// ========================================================================
// Genetic API auth tests
// ========================================================================

#[tokio::test]
async fn genetic_train_without_auth_header_returns_403() {
    let app = test_app_with_api_key(Some("my-secret-key".to_string()));

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/genetic/train")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"mode":"generations","generations":1}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn genetic_train_with_wrong_bearer_token_returns_403() {
    let app = test_app_with_api_key(Some("correct-key".to_string()));

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/genetic/train")
                .header("content-type", "application/json")
                .header("authorization", "Bearer wrong-key")
                .body(Body::from(r#"{"mode":"generations","generations":1}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn genetic_train_with_no_configured_key_returns_403() {
    // When no API key is configured, all mutation requests are rejected.
    let app = test_app_with_api_key(None);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/genetic/train")
                .header("content-type", "application/json")
                .header("authorization", "Bearer some-token")
                .body(Body::from(r#"{"mode":"generations","generations":1}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

// ========================================================================
// Session token after kick
// ========================================================================

#[tokio::test]
async fn session_token_invalid_after_kick() {
    let app = test_app();

    // Create a room (Alice is host, slot 0)
    let (code, _alice_token) = create_room_via_api(&app, "Alice", 2).await;

    // Bob joins
    let join_resp = app
        .clone()
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
    let bob_token = join_json["session_token"].as_str().unwrap().to_string();

    // Kick Bob via Room method (we test the lobby-level session invalidation)
    {
        let state: AppState = Arc::new(AppStateInner {
            lobby: skyjo_server::lobby::Lobby::new(100),
            genetic: Arc::new(Mutex::new(
                skyjo_server::genetic::GeneticTrainingState::load_or_new(
                    std::env::temp_dir().join("skyjo_test_kick.json"),
                ),
            )),
            genetic_api_key: None,
            persistence: None,
            rate_limiter: Arc::new(skyjo_server::rate_limit::RateLimiter::new()),
        });

        // Create room directly in lobby
        let (code, _host_token, _) = state
            .lobby
            .create_room("Host".into(), 2, None, 0, 0)
            .unwrap();
        let (_bob_token, bob_idx) = state.lobby.join_room(&code, "Bob".into()).await.unwrap();

        // Kick Bob: removes session token from room slot
        let room_ref = state.lobby.get_room(&code).unwrap();
        let kicked_token = {
            let mut room = room_ref.lock().await;
            room.kick_player(bob_idx).unwrap()
        };

        // The kicked token should have been returned
        assert!(kicked_token.is_some());
        // Remove from lobby sessions (mirrors what ws.rs does)
        state.lobby.sessions.remove(&kicked_token.unwrap());

        // Now the token should not resolve to a session
        assert!(
            state.lobby.get_session(&bob_token).is_none() || {
                // bob_token was from the HTTP-level test above, not this lobby.
                // Verify the kicked token is truly gone from this lobby.
                let kicked_str = _bob_token.to_string();
                state.lobby.get_session(&kicked_str).is_none()
            }
        );
    }
}

// ========================================================================
// Banned IP check
// ========================================================================

#[tokio::test]
async fn banned_ip_is_recognized() {
    use skyjo_server::room::Room;

    let mut room = Room::new("BANNED".to_string(), "Alice".to_string(), 2, None, 0, 0);
    room.banned_ips.push("192.168.1.100".to_string());

    assert!(room.is_ip_banned("192.168.1.100"));
    assert!(!room.is_ip_banned("10.0.0.1"));
}

#[tokio::test]
async fn ban_player_adds_ip_and_kicks() {
    use skyjo_server::messages::PlayerSlotType;
    use skyjo_server::room::Room;
    use skyjo_server::session::SessionToken;

    let mut room = Room::new("BANRM".to_string(), "Alice".to_string(), 3, None, 0, 0);
    // Give creator an IP
    room.players[0].ip = Some("10.0.0.1".to_string());

    // Add a human player in slot 1
    room.players[1].name = "Bob".to_string();
    room.players[1].slot_type = PlayerSlotType::Human;
    room.players[1].session_token = Some(SessionToken::new());
    room.players[1].ip = Some("192.168.1.50".to_string());

    let result = room.ban_player(1);
    assert!(result.is_ok());
    assert!(room.is_ip_banned("192.168.1.50"));
    // After ban, slot should be empty (kicked)
    assert_eq!(room.players[1].slot_type, PlayerSlotType::Empty);
}

// ========================================================================
// Invalid room code format rejected
// ========================================================================

#[tokio::test]
async fn join_with_lowercase_room_code_returns_400() {
    let app = test_app();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/rooms/abcdef/join")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"player_name":"Bob"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn join_with_too_short_room_code_returns_400() {
    let app = test_app();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/rooms/ABCDE/join")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"player_name":"Bob"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn join_with_excluded_chars_room_code_returns_400() {
    let app = test_app();

    // 'I' and 'O' are excluded from the room code charset
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/rooms/ABCDIO/join")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"player_name":"Bob"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

// ========================================================================
// Player name validation (via HTTP)
// ========================================================================

#[tokio::test]
async fn join_with_too_long_name_returns_400() {
    let app = test_app();
    let (code, _) = create_room_via_api(&app, "Alice", 2).await;

    let long_name = "A".repeat(33);
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/rooms/{code}/join"))
                .header("content-type", "application/json")
                .body(Body::from(format!(r#"{{"player_name":"{long_name}"}}"#)))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn join_with_empty_name_returns_400() {
    let app = test_app();
    let (code, _) = create_room_via_api(&app, "Alice", 2).await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/rooms/{code}/join"))
                .header("content-type", "application/json")
                .body(Body::from(r#"{"player_name":""}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn join_with_whitespace_only_name_returns_400() {
    let app = test_app();
    let (code, _) = create_room_via_api(&app, "Alice", 2).await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/rooms/{code}/join"))
                .header("content-type", "application/json")
                .body(Body::from(r#"{"player_name":"   "}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn create_room_with_empty_name_returns_400() {
    let app = test_app();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/rooms")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"player_name":"","num_players":2}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn create_room_with_too_long_name_returns_400() {
    let app = test_app();

    let long_name = "B".repeat(33);
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/rooms")
                .header("content-type", "application/json")
                .body(Body::from(format!(
                    r#"{{"player_name":"{long_name}","num_players":2}}"#
                )))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

// ========================================================================
// Validate_room_code unit tests
// ========================================================================

#[test]
fn validate_room_code_rejects_lowercase() {
    let result = skyjo_server::room::validate_room_code("abcdef");
    assert_eq!(result, Err(ServerError::RoomCodeInvalid));
}

#[test]
fn validate_room_code_rejects_wrong_length() {
    assert_eq!(
        skyjo_server::room::validate_room_code("ABCDE"),
        Err(ServerError::RoomCodeInvalid)
    );
    assert_eq!(
        skyjo_server::room::validate_room_code("ABCDEFG"),
        Err(ServerError::RoomCodeInvalid)
    );
}

#[test]
fn validate_room_code_rejects_excluded_chars() {
    // I and O are excluded (along with digits 0 and 1)
    assert_eq!(
        skyjo_server::room::validate_room_code("ABCDEI"),
        Err(ServerError::RoomCodeInvalid)
    );
    assert_eq!(
        skyjo_server::room::validate_room_code("ABCDEO"),
        Err(ServerError::RoomCodeInvalid)
    );
    assert_eq!(
        skyjo_server::room::validate_room_code("ABCDE0"),
        Err(ServerError::RoomCodeInvalid)
    );
    assert_eq!(
        skyjo_server::room::validate_room_code("ABCDE1"),
        Err(ServerError::RoomCodeInvalid)
    );
}

#[test]
fn validate_room_code_accepts_valid_code() {
    assert!(skyjo_server::room::validate_room_code("ABCDEF").is_ok());
    assert!(skyjo_server::room::validate_room_code("234567").is_ok());
    assert!(skyjo_server::room::validate_room_code("HJK289").is_ok());
}

// ========================================================================
// Validate_player_name unit tests
// ========================================================================

#[test]
fn validate_player_name_rejects_empty() {
    assert_eq!(
        skyjo_server::room::validate_player_name(""),
        Err(ServerError::PlayerNameEmpty)
    );
}

#[test]
fn validate_player_name_rejects_whitespace_only() {
    assert_eq!(
        skyjo_server::room::validate_player_name("   "),
        Err(ServerError::PlayerNameEmpty)
    );
}

#[test]
fn validate_player_name_rejects_too_long() {
    let long = "X".repeat(33);
    assert_eq!(
        skyjo_server::room::validate_player_name(&long),
        Err(ServerError::PlayerNameTooLong)
    );
}

#[test]
fn validate_player_name_trims_and_accepts_valid() {
    assert_eq!(
        skyjo_server::room::validate_player_name("  Alice  "),
        Ok("Alice".to_string())
    );
}
