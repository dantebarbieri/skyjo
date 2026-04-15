pub mod auth;
pub mod error;
pub mod genetic;
pub mod leaderboard;
pub mod lobby;
pub mod messages;
pub mod persistence;
pub mod rate_limit;
pub mod room;
pub mod session;
pub mod ws;

use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::Mutex;

use axum::extract::{ConnectInfo, Path, State};
use axum::middleware::Next;
use axum::response::{IntoResponse, Json, Response};

use error::ServerError;
use genetic::GeneticTrainingState;
use lobby::{
    CreateRoomRequest, CreateRoomResponse, JoinRoomRequest, JoinRoomResponse, Lobby,
    RoomInfoResponse,
};
use persistence::Persistence;

pub struct AppStateInner {
    pub lobby: Lobby,
    pub genetic: Arc<Mutex<GeneticTrainingState>>,
    pub persistence: Persistence,
    pub rate_limiter: Arc<crate::rate_limit::RateLimiter>,
    pub jwt_secret: String,
}

pub type AppState = Arc<AppStateInner>;

// --- Auth Middleware ---

/// Middleware that requires a valid JWT with moderator or admin permission.
/// Used to protect genetic mutation endpoints and other privileged routes.
pub async fn require_moderator_middleware(
    State(state): State<AppState>,
    mut req: axum::extract::Request,
    next: Next,
) -> Response {
    let auth_header = req
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok());

    let token = match auth_header {
        Some(h) => match h.strip_prefix("Bearer ") {
            Some(t) => t,
            None => return ServerError::Unauthorized.into_response(),
        },
        None => return ServerError::Unauthorized.into_response(),
    };

    match auth::validate_access_token(token, &state.jwt_secret) {
        Ok(user) => {
            if user.permission == auth::PermissionLevel::User {
                return ServerError::Forbidden.into_response();
            }
            req.extensions_mut().insert(user);
            next.run(req).await
        }
        Err(_) => ServerError::Unauthorized.into_response(),
    }
}

/// Middleware that requires a valid JWT (any permission level).
pub async fn require_auth_middleware(
    State(state): State<AppState>,
    mut req: axum::extract::Request,
    next: Next,
) -> Response {
    let auth_header = req
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok());

    let token = match auth_header {
        Some(h) => match h.strip_prefix("Bearer ") {
            Some(t) => t,
            None => return ServerError::Unauthorized.into_response(),
        },
        None => return ServerError::Unauthorized.into_response(),
    };

    match auth::validate_access_token(token, &state.jwt_secret) {
        Ok(user) => {
            req.extensions_mut().insert(user);
            next.run(req).await
        }
        Err(_) => ServerError::Unauthorized.into_response(),
    }
}

/// Middleware that requires admin permission.
pub async fn require_admin_middleware(
    State(state): State<AppState>,
    mut req: axum::extract::Request,
    next: Next,
) -> Response {
    let auth_header = req
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok());

    let token = match auth_header {
        Some(h) => match h.strip_prefix("Bearer ") {
            Some(t) => t,
            None => return ServerError::Unauthorized.into_response(),
        },
        None => return ServerError::Unauthorized.into_response(),
    };

    match auth::validate_access_token(token, &state.jwt_secret) {
        Ok(user) => {
            if user.permission != auth::PermissionLevel::Admin {
                return ServerError::Forbidden.into_response();
            }
            req.extensions_mut().insert(user);
            next.run(req).await
        }
        Err(_) => ServerError::Unauthorized.into_response(),
    }
}

// --- REST Handlers (public for integration tests) ---

pub async fn create_room(
    State(state): State<AppState>,
    Json(req): Json<CreateRoomRequest>,
) -> Result<Json<CreateRoomResponse>, ServerError> {
    let (genetic_games, genetic_gen) = {
        let g = state.genetic.lock().await;
        (g.total_games_trained, g.generation)
    };
    let (code, token, player_index) = state.lobby.create_room(
        req.player_name,
        req.num_players,
        req.rules,
        genetic_games,
        genetic_gen,
    )?;

    Ok(Json(CreateRoomResponse {
        room_code: code,
        session_token: token.to_string(),
        player_index,
    }))
}

pub async fn room_info(
    State(state): State<AppState>,
    Path(code): Path<String>,
) -> Result<Json<RoomInfoResponse>, ServerError> {
    let room_ref = state
        .lobby
        .get_room(&code)
        .ok_or(ServerError::RoomNotFound)?;

    let room = room_ref.lock().await;
    let players_joined = room
        .players
        .iter()
        .filter(|p| p.slot_type != messages::PlayerSlotType::Empty)
        .count();

    let phase = match room.phase {
        room::RoomPhase::Lobby => "lobby",
        room::RoomPhase::InGame => "in_game",
        room::RoomPhase::GameOver => "game_over",
    };

    Ok(Json(RoomInfoResponse {
        room_code: room.code.clone(),
        num_players: room.num_players,
        rules: room.rules_name.clone(),
        players_joined,
        phase: phase.to_string(),
    }))
}

pub async fn join_room(
    State(state): State<AppState>,
    Path(code): Path<String>,
    Json(req): Json<JoinRoomRequest>,
) -> Result<Json<JoinRoomResponse>, ServerError> {
    let (token, player_index) = state.lobby.join_room(&code, req.player_name).await?;

    Ok(Json(JoinRoomResponse {
        session_token: token.to_string(),
        player_index,
    }))
}

/// Lightweight session validation endpoint.
/// Returns 200 if the session token is valid for the given room, 401 otherwise.
/// Used by clients to check session validity before attempting WebSocket reconnection.
pub async fn validate_session(
    State(state): State<AppState>,
    Path(code): Path<String>,
    axum::extract::Query(query): axum::extract::Query<lobby::ValidateSessionQuery>,
) -> Result<Json<serde_json::Value>, ServerError> {
    let (room_code, _player_index) = state
        .lobby
        .get_session(&query.token)
        .ok_or(ServerError::Unauthorized)?;

    if room_code != code {
        return Err(ServerError::Unauthorized);
    }

    // Verify the room still exists
    state
        .lobby
        .get_room(&code)
        .ok_or(ServerError::RoomNotFound)?;

    Ok(Json(serde_json::json!({ "valid": true })))
}

pub async fn genetic_status(State(state): State<AppState>) -> Json<genetic::TrainingStatus> {
    let s = state.genetic.lock().await;
    Json(s.status())
}

// --- Rate Limit Middleware ---

pub async fn rate_limit_middleware(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(state): State<AppState>,
    req: axum::extract::Request,
    next: Next,
) -> Response {
    let (resource, config) = match (req.uri().path(), req.method()) {
        (p, &axum::http::Method::POST) if p.starts_with("/api/rooms") && !p.contains("/join") => {
            ("room_create", &rate_limit::limits::ROOM_CREATION)
        }
        (p, &axum::http::Method::POST) if p.contains("/join") => {
            ("room_join", &rate_limit::limits::ROOM_JOIN)
        }
        (p, &axum::http::Method::POST) if p.starts_with("/api/genetic/") => {
            ("genetic", &rate_limit::limits::GENETIC_API)
        }
        _ => return next.run(req).await,
    };

    if !state.rate_limiter.check(addr.ip(), resource, config) {
        return error::ServerError::RateLimited.into_response();
    }

    next.run(req).await
}
