mod lobby;
mod messages;
mod room;
mod session;
mod ws;

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::{ConnectInfo, Path, Query, State, WebSocketUpgrade};
use axum::http::StatusCode;
use axum::response::{Json, Response};
use axum::routing::{get, post};
use axum::Router;
use clap::Parser;
use serde::Deserialize;
use tower_http::compression::CompressionLayer;
use tower_http::services::{ServeDir, ServeFile};

use lobby::{
    CreateRoomRequest, CreateRoomResponse, JoinRoomRequest, JoinRoomResponse, Lobby,
    RoomInfoResponse,
};

#[derive(Parser)]
#[command(name = "skyjo-server")]
struct Args {
    /// Port to listen on.
    #[arg(long, default_value = "8080")]
    port: u16,

    /// Directory containing static frontend files.
    #[arg(long, default_value = "./static")]
    static_dir: PathBuf,
}

type AppState = Arc<Lobby>;

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "skyjo_server=info,tower_http=info".parse().unwrap()),
        )
        .init();

    let args = Args::parse();

    let lobby = Arc::new(Lobby::new(100));

    // Spawn room cleanup task
    let lobby_cleanup = lobby.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(60));
        loop {
            interval.tick().await;
            lobby_cleanup.cleanup_stale_rooms(
                Duration::from_secs(300),  // 5 min after game over
                Duration::from_secs(600),  // 10 min after all disconnect
            );
        }
    });

    // API routes
    let api_routes = Router::new()
        .route("/rooms", post(create_room))
        .route("/rooms/{code}", get(room_info))
        .route("/rooms/{code}/join", post(join_room))
        .route("/rooms/{code}/ws", get(ws_upgrade));

    // SPA fallback: serve index.html for any non-file route
    let index_path = args.static_dir.join("index.html");
    let static_service = ServeDir::new(&args.static_dir)
        .not_found_service(ServeFile::new(&index_path));

    let app = Router::new()
        .nest("/api", api_routes)
        .fallback_service(static_service)
        .layer(CompressionLayer::new())
        .with_state(lobby);

    let addr = format!("0.0.0.0:{}", args.port);
    tracing::info!("Starting server on {addr}");
    tracing::info!("Serving static files from {:?}", args.static_dir);

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("Failed to bind");

    axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>())
        .await
        .expect("Server error");
}

// --- REST Handlers ---

async fn create_room(
    State(lobby): State<AppState>,
    Json(req): Json<CreateRoomRequest>,
) -> Result<Json<CreateRoomResponse>, (StatusCode, String)> {
    let (code, token, player_index) = lobby
        .create_room(req.player_name, req.num_players, req.rules)
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    Ok(Json(CreateRoomResponse {
        room_code: code,
        session_token: token.to_string(),
        player_index,
    }))
}

async fn room_info(
    State(lobby): State<AppState>,
    Path(code): Path<String>,
) -> Result<Json<RoomInfoResponse>, (StatusCode, String)> {
    let room_ref = lobby
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

async fn join_room(
    State(lobby): State<AppState>,
    Path(code): Path<String>,
    Json(req): Json<JoinRoomRequest>,
) -> Result<Json<JoinRoomResponse>, (StatusCode, String)> {
    let (token, player_index) = lobby
        .join_room(&code, req.player_name)
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    Ok(Json(JoinRoomResponse {
        session_token: token.to_string(),
        player_index,
    }))
}

#[derive(Deserialize)]
struct WsQuery {
    token: String,
}

async fn ws_upgrade(
    State(lobby): State<AppState>,
    Path(code): Path<String>,
    Query(query): Query<WsQuery>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    ws: WebSocketUpgrade,
) -> Result<Response, (StatusCode, String)> {
    // Authenticate session token
    let (room_code, player_index) = lobby
        .get_session(&query.token)
        .ok_or((StatusCode::UNAUTHORIZED, "Invalid session token".to_string()))?;

    if room_code != code {
        return Err((
            StatusCode::FORBIDDEN,
            "Token does not match this room".to_string(),
        ));
    }

    let room = lobby
        .get_room(&code)
        .ok_or((StatusCode::NOT_FOUND, "Room not found".to_string()))?;

    // Check IP ban
    {
        let room_guard = room.lock().await;
        let ip = addr.ip().to_string();
        if room_guard.is_ip_banned(&ip) {
            return Err((
                StatusCode::FORBIDDEN,
                "You are banned from this room".to_string(),
            ));
        }
    }

    let client_ip = addr.ip().to_string();

    Ok(ws.on_upgrade(move |socket| async move {
        ws::handle_ws(socket, lobby, room, room_code, player_index, client_ip).await;
    }))
}
