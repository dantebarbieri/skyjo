pub mod error;
pub mod genetic;
pub mod lobby;
pub mod messages;
pub mod persistence;
pub mod rate_limit;
pub mod room;
pub mod session;
pub mod ws;

use std::sync::Arc;
use tokio::sync::Mutex;

use axum::extract::{Path, State};
use axum::response::Json;

use error::ServerError;
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
) -> Result<Json<CreateRoomResponse>, ServerError> {
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
    let (token, player_index) = state
        .lobby
        .join_room(&code, req.player_name)
        .await?;

    Ok(Json(JoinRoomResponse {
        session_token: token.to_string(),
        player_index,
    }))
}

pub async fn genetic_status(State(state): State<AppState>) -> Json<genetic::TrainingStatus> {
    let s = state.genetic.lock().await;
    Json(s.status())
}
