pub mod genetic;
pub mod lobby;
pub mod messages;
pub mod room;
pub mod session;
pub mod ws;

use std::sync::Arc;
use tokio::sync::Mutex;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::Json;

use genetic::GeneticTrainingState;
use lobby::{
    CreateRoomRequest, CreateRoomResponse, JoinRoomRequest, JoinRoomResponse, Lobby,
    RoomInfoResponse,
};

pub struct AppStateInner {
    pub lobby: Lobby,
    pub genetic: Arc<Mutex<GeneticTrainingState>>,
}

pub type AppState = Arc<AppStateInner>;

// --- REST Handlers (public for integration tests) ---

pub async fn create_room(
    State(state): State<AppState>,
    Json(req): Json<CreateRoomRequest>,
) -> Result<Json<CreateRoomResponse>, (StatusCode, String)> {
    let (genetic_games, genetic_gen) = {
        let g = state.genetic.lock().await;
        (g.total_games_trained, g.generation)
    };
    let (code, token, player_index) = state
        .lobby
        .create_room(
            req.player_name,
            req.num_players,
            req.rules,
            genetic_games,
            genetic_gen,
        )
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    Ok(Json(CreateRoomResponse {
        room_code: code,
        session_token: token.to_string(),
        player_index,
    }))
}

pub async fn room_info(
    State(state): State<AppState>,
    Path(code): Path<String>,
) -> Result<Json<RoomInfoResponse>, (StatusCode, String)> {
    let room_ref = state
        .lobby
        .get_room(&code)
        .ok_or((StatusCode::NOT_FOUND, "Room not found".to_string()))?;

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
) -> Result<Json<JoinRoomResponse>, (StatusCode, String)> {
    let (token, player_index) = state
        .lobby
        .join_room(&code, req.player_name)
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    Ok(Json(JoinRoomResponse {
        session_token: token.to_string(),
        player_index,
    }))
}

pub async fn genetic_status(State(state): State<AppState>) -> Json<genetic::TrainingStatus> {
    let s = state.genetic.lock().await;
    Json(s.status())
}
