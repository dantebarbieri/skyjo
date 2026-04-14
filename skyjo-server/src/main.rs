use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::extract::{ConnectInfo, Path, Query, State, WebSocketUpgrade};
use axum::http::StatusCode;
use axum::response::{Json, Response};
use axum::routing::{get, post};
use clap::Parser;
use serde::Deserialize;
use tokio::sync::Mutex;
use tower_http::compression::CompressionLayer;
use tower_http::services::{ServeDir, ServeFile};

use skyjo_server::genetic::{self, GeneticTrainingState};
use skyjo_server::lobby::Lobby;
use skyjo_server::ws;
use skyjo_server::{AppState, AppStateInner};

#[derive(Parser)]
#[command(name = "skyjo-server")]
struct Args {
    /// Port to listen on.
    #[arg(long, default_value = "8080")]
    port: u16,

    /// Directory containing static frontend files.
    #[arg(long, default_value = "./static")]
    static_dir: PathBuf,

    /// Path to the genetic model file.
    #[arg(long, default_value = "./genetic_model.json")]
    genetic_model_path: PathBuf,
}

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

    let genetic_state = Arc::new(Mutex::new(GeneticTrainingState::load_or_new(
        args.genetic_model_path,
    )));

    let app_state = Arc::new(AppStateInner {
        lobby: Lobby::new(100),
        genetic: genetic_state,
    });

    // Spawn room cleanup task
    let cleanup_state = app_state.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(60));
        loop {
            interval.tick().await;
            cleanup_state.lobby.cleanup_stale_rooms(
                Duration::from_secs(300), // 5 min after game over
                Duration::from_secs(600), // 10 min after all disconnect
            );
        }
    });

    // API routes
    let api_routes = Router::new()
        .route("/rooms", post(skyjo_server::create_room))
        .route("/rooms/{code}", get(skyjo_server::room_info))
        .route("/rooms/{code}/join", post(skyjo_server::join_room))
        .route("/rooms/{code}/ws", get(ws_upgrade))
        .route("/genetic/model", get(genetic_model))
        .route("/genetic/train", post(genetic_train))
        .route("/genetic/stop", post(genetic_stop))
        .route("/genetic/reset", post(genetic_reset))
        .route("/genetic/load", post(genetic_load))
        .route("/genetic/status", get(skyjo_server::genetic_status))
        .route("/genetic/saved", get(genetic_saved_list).post(genetic_save))
        .route("/genetic/saved/import", post(genetic_import))
        .route(
            "/genetic/saved/{name}",
            axum::routing::delete(genetic_saved_delete),
        )
        .route("/genetic/saved/{name}/model", get(genetic_saved_model));

    // SPA fallback: serve index.html for any non-file route
    let index_path = args.static_dir.join("index.html");
    let static_service =
        ServeDir::new(&args.static_dir).not_found_service(ServeFile::new(&index_path));

    let app = Router::new()
        .nest("/api", api_routes)
        .fallback_service(static_service)
        .layer(CompressionLayer::new())
        .with_state(app_state);

    let addr = format!("0.0.0.0:{}", args.port);
    tracing::info!("Starting server on {addr}");
    tracing::info!("Serving static files from {:?}", args.static_dir);

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("Failed to bind");

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .expect("Server error");
}

// --- REST Handlers ---

#[derive(Deserialize)]
struct WsQuery {
    token: String,
}

async fn ws_upgrade(
    State(state): State<AppState>,
    Path(code): Path<String>,
    Query(query): Query<WsQuery>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    ws: WebSocketUpgrade,
) -> Result<Response, (StatusCode, String)> {
    // Authenticate session token
    let (room_code, player_index) = state.lobby.get_session(&query.token).ok_or((
        StatusCode::UNAUTHORIZED,
        "Invalid session token".to_string(),
    ))?;

    if room_code != code {
        return Err((
            StatusCode::FORBIDDEN,
            "Token does not match this room".to_string(),
        ));
    }

    let room = state
        .lobby
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
        ws::handle_ws(socket, state, room, room_code, player_index, client_ip).await;
    }))
}

// --- Genetic API handlers ---

async fn genetic_model(State(state): State<AppState>) -> Json<genetic::GeneticModelData> {
    let s = state.genetic.lock().await;
    Json(s.model_data())
}

#[derive(Deserialize)]
#[serde(tag = "mode")]
enum TrainRequest {
    #[serde(rename = "generations")]
    ForGenerations {
        generations: usize,
        #[serde(default)]
        unlimited: bool,
    },
    #[serde(rename = "until_generation")]
    UntilGeneration {
        target_generation: usize,
        #[serde(default)]
        unlimited: bool,
    },
    #[serde(rename = "until_fitness")]
    UntilFitness {
        target_fitness: f64,
        max_generations: Option<usize>,
        #[serde(default)]
        unlimited: bool,
    },
}

async fn genetic_train(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Json(req): Json<TrainRequest>,
) -> Result<Json<genetic::TrainingStatus>, (StatusCode, String)> {
    let mut s = state.genetic.lock().await;
    if s.is_training {
        return Ok(Json(s.status()));
    }

    // Only allow unlimited training from localhost
    let is_local = addr.ip().is_loopback();

    let (generations, mode, target_fitness) = match req {
        TrainRequest::ForGenerations {
            generations,
            unlimited,
        } => {
            let gens = if unlimited && is_local {
                generations
            } else {
                generations.min(50_000)
            };
            (gens, "generations".to_string(), 0.0)
        }
        TrainRequest::UntilGeneration {
            target_generation,
            unlimited,
        } => {
            let current = s.generation;
            if target_generation <= current {
                return Err((
                    StatusCode::BAD_REQUEST,
                    format!(
                        "Target generation {target_generation} must be greater than current generation {current}"
                    ),
                ));
            }
            let gens = if unlimited && is_local {
                target_generation - current
            } else {
                (target_generation - current).min(50_000)
            };
            (gens, "until_generation".to_string(), 0.0)
        }
        TrainRequest::UntilFitness {
            target_fitness,
            max_generations,
            unlimited,
        } => {
            let allow_unlimited = unlimited && is_local;
            let default_cap = if allow_unlimited {
                10_000_000
            } else {
                50_000
            };
            let max_cap = if allow_unlimited {
                10_000_000
            } else {
                50_000
            };
            let cap = max_generations.unwrap_or(default_cap).min(max_cap);
            (cap, "until_fitness".to_string(), target_fitness)
        }
    };

    s.is_training = true;
    s.training_start_generation = s.generation;
    s.training_target_generation = s.generation.saturating_add(generations);
    s.training_mode = mode;
    s.training_target_fitness = target_fitness;
    s.training_start_fitness = if s.best_fitness.is_finite() {
        s.best_fitness
    } else {
        0.0
    };
    s.training_started_at = Some(std::time::Instant::now());
    s.training_last_gen_elapsed_ms = 0;
    let status = s.status();
    drop(s);

    let genetic = state.genetic.clone();
    tokio::spawn(async move {
        genetic::train_generations(genetic, generations).await;
    });

    Ok(Json(status))
}

async fn genetic_stop(State(state): State<AppState>) -> Json<genetic::TrainingStatus> {
    let mut s = state.genetic.lock().await;
    if s.is_training {
        s.is_training = false;
        // training_started_at is cleared by the training loop when it detects is_training=false
    }
    Json(s.status())
}

async fn genetic_reset(
    State(state): State<AppState>,
) -> Result<Json<genetic::TrainingStatus>, (StatusCode, String)> {
    let mut s = state.genetic.lock().await;
    if s.is_training {
        return Err((
            StatusCode::CONFLICT,
            "Cannot reset while training is in progress. Stop training first.".to_string(),
        ));
    }
    s.reset();
    Ok(Json(s.status()))
}

async fn genetic_load(
    State(state): State<AppState>,
    Json(req): Json<SaveRequest>,
) -> Result<Json<genetic::TrainingStatus>, (StatusCode, String)> {
    let mut s = state.genetic.lock().await;
    if s.is_training {
        return Err((
            StatusCode::CONFLICT,
            "Cannot load while training is in progress. Stop training first.".to_string(),
        ));
    }
    let name = req
        .name
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "name is required".to_string()))?;
    s.load_saved(&name)
        .map_err(|e| (StatusCode::NOT_FOUND, e))?;
    Ok(Json(s.status()))
}

async fn genetic_saved_list(
    State(state): State<AppState>,
) -> Json<Vec<genetic::SavedGenerationInfo>> {
    let s = state.genetic.lock().await;
    Json(s.list_saved_generations())
}

#[derive(Deserialize)]
struct SaveRequest {
    name: Option<String>,
}

async fn genetic_save(
    State(state): State<AppState>,
    Json(req): Json<SaveRequest>,
) -> Result<Json<genetic::SavedGenerationInfo>, (StatusCode, String)> {
    let mut s = state.genetic.lock().await;
    s.save_generation(req.name)
        .map(Json)
        .map_err(|e| (StatusCode::BAD_REQUEST, e))
}

async fn genetic_saved_delete(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let mut s = state.genetic.lock().await;
    s.delete_saved_generation(&name)
        .map(|()| Json(serde_json::json!({ "deleted": name })))
        .map_err(|e| (StatusCode::NOT_FOUND, e))
}

#[derive(Deserialize)]
struct ImportRequest {
    name: String,
    genome: Vec<f32>,
    generation: Option<usize>,
    total_games_trained: Option<usize>,
    best_fitness: Option<f64>,
    lineage_hash: Option<String>,
    architecture_version: Option<u32>,
}

async fn genetic_import(
    State(state): State<AppState>,
    Json(req): Json<ImportRequest>,
) -> Result<Json<genetic::SavedGenerationInfo>, (StatusCode, String)> {
    let mut s = state.genetic.lock().await;
    s.import_generation(
        req.name,
        req.genome,
        req.generation.unwrap_or(0),
        req.total_games_trained.unwrap_or(0),
        req.best_fitness.unwrap_or(0.0),
        req.lineage_hash,
        req.architecture_version,
    )
    .map(Json)
    .map_err(|e| (StatusCode::BAD_REQUEST, e))
}

async fn genetic_saved_model(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Json<genetic::GeneticModelData>, (StatusCode, String)> {
    let s = state.genetic.lock().await;
    s.get_saved_generation_model(&name)
        .map(Json)
        .map_err(|e| (StatusCode::NOT_FOUND, e))
}
