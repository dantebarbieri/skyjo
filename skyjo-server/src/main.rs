use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::extract::{ConnectInfo, Extension, Path, Query, State, WebSocketUpgrade};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Json, Response};
use axum::routing::{delete, get, patch, post};
use clap::Parser;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tower_http::compression::CompressionLayer;

use skyjo_server::auth::{self, AuthUser, PermissionLevel};
use skyjo_server::error::ServerError;
use skyjo_server::genetic::{self, GeneticTrainingState};
use skyjo_server::lobby::Lobby;
use skyjo_server::messages::ServerMessage;
use skyjo_server::persistence::Persistence;
use skyjo_server::room::{RoomPhase, RoomSnapshot};
use skyjo_server::ws;
use skyjo_server::{AppState, AppStateInner};

#[derive(Parser)]
#[command(name = "skyjo-server")]
struct Args {
    /// Port to listen on.
    #[arg(long, default_value = "8080")]
    port: u16,

    /// Directory containing static frontend files (for single-binary mode).
    #[arg(long)]
    static_dir: Option<PathBuf>,

    /// Path to the genetic model file.
    #[arg(long)]
    genetic_model_path: Option<PathBuf>,

    /// Directory for persistent data (genetic model files)
    #[arg(long, env = "SKYJO_DATA_DIR", default_value = "./data")]
    data_dir: PathBuf,

    /// PostgreSQL connection URL
    #[arg(long, env = "DATABASE_URL")]
    database_url: String,

    /// JWT signing secret
    #[arg(long, env = "SKYJO_JWT_SECRET")]
    jwt_secret: Option<String>,
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

    // Ensure data directory exists
    std::fs::create_dir_all(&args.data_dir).expect("Failed to create data directory");

    let genetic_model_path = args
        .genetic_model_path
        .unwrap_or_else(|| args.data_dir.join("genetic_model.json"));

    let genetic_state = Arc::new(Mutex::new(GeneticTrainingState::load_or_new(
        genetic_model_path,
    )));

    // Initialize persistence (PostgreSQL)
    let persistence = Persistence::connect(&args.database_url)
        .await
        .expect("Failed to connect to PostgreSQL database");
    tracing::info!("Connected to PostgreSQL database");

    // JWT secret — generate one if not provided or empty (warn about it)
    let jwt_secret = args
        .jwt_secret
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| {
            let secret = uuid::Uuid::new_v4().to_string();
            tracing::warn!(
                "No SKYJO_JWT_SECRET set — generated ephemeral secret. Sessions will not survive restart."
            );
            secret
        });

    // Check setup status
    match auth::user_count(persistence.pool()).await {
        Ok(0) => tracing::info!("No users found — setup wizard will be required on first access"),
        Ok(n) => tracing::info!("Database has {n} user(s)"),
        Err(e) => tracing::error!("Failed to check user count: {e}"),
    }

    let rate_limiter = Arc::new(skyjo_server::rate_limit::RateLimiter::new());

    let app_state = Arc::new(AppStateInner {
        lobby: Lobby::new(100),
        genetic: genetic_state,
        persistence: persistence.clone(),
        rate_limiter,
        jwt_secret,
    });

    // Restore rooms from snapshots (best-effort crash recovery)
    {
        match persistence.load_all_room_snapshots().await {
            Ok(snapshots) => {
                let count = snapshots.len();
                for (code, data) in snapshots {
                    match String::from_utf8(data) {
                        Ok(json) => match serde_json::from_str::<RoomSnapshot>(&json) {
                            Ok(snapshot) => {
                                let room = skyjo_server::room::Room::from_snapshot(snapshot);
                                let shared = Arc::new(tokio::sync::Mutex::new(room));
                                app_state.lobby.rooms.insert(code.clone(), shared);
                                tracing::info!(room = %code, "Restored room from snapshot");
                            }
                            Err(e) => {
                                tracing::warn!(
                                    room = %code,
                                    "Failed to deserialize room snapshot: {e}"
                                );
                            }
                        },
                        Err(e) => {
                            tracing::warn!(
                                room = %code,
                                "Failed to decode room snapshot as UTF-8: {e}"
                            );
                        }
                    }
                    // Clean up snapshot after restoration attempt
                    if let Err(e) = persistence.delete_room_snapshot(&code).await {
                        tracing::warn!(room = %code, "Failed to delete snapshot: {e}");
                    }
                }
                if count > 0 {
                    tracing::info!("Processed {count} room snapshot(s) from previous session");
                }
            }
            Err(e) => {
                tracing::warn!("Failed to load room snapshots: {e}");
            }
        }
    }

    // Spawn room cleanup + periodic snapshot task
    let cleanup_state = app_state.clone();
    let cleanup_persistence = persistence.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(60));
        loop {
            interval.tick().await;
            cleanup_state.lobby.cleanup_stale_rooms(
                Duration::from_secs(300), // 5 min after game over
                Duration::from_secs(600), // 10 min after all disconnect
            );

            // Clean up stale rate limiter entries
            cleanup_state.rate_limiter.cleanup(Duration::from_secs(300));

            // Periodic snapshot of active in-game rooms
            for entry in cleanup_state.lobby.rooms.iter() {
                let code = entry.key().clone();
                if let Ok(room) = entry.value().try_lock()
                    && room.phase == skyjo_server::room::RoomPhase::InGame
                {
                    let snapshot = room.to_snapshot();
                    if let Ok(json) = serde_json::to_string(&snapshot)
                        && let Err(e) = cleanup_persistence.save_room_snapshot(&code, &json).await
                    {
                        tracing::warn!(room = %code, "Failed to save snapshot: {e}");
                    }
                }
            }
        }
    });

    // Genetic mutation routes (require moderator+ auth)
    let genetic_mutation_routes = Router::new()
        .route("/genetic/train", post(genetic_train))
        .route("/genetic/stop", post(genetic_stop))
        .route("/genetic/reset", post(genetic_reset))
        .route("/genetic/load", post(genetic_load))
        .route("/genetic/saved", post(genetic_save))
        .route("/genetic/saved/import", post(genetic_import))
        .route("/genetic/saved/{name}", delete(genetic_saved_delete))
        .layer(axum::middleware::from_fn_with_state(
            app_state.clone(),
            skyjo_server::require_moderator_middleware,
        ));

    // Admin routes (require admin auth)
    let admin_routes = Router::new()
        .route("/admin/users", get(admin_list_users))
        .route("/admin/users", post(admin_create_user))
        .route(
            "/admin/users/{id}/permission",
            patch(admin_update_permission),
        )
        .route("/admin/users/{id}", delete(admin_delete_user))
        .route("/admin/settings", get(admin_get_settings))
        .route("/admin/settings", patch(admin_update_settings))
        .layer(axum::middleware::from_fn_with_state(
            app_state.clone(),
            skyjo_server::require_admin_middleware,
        ));

    // Authenticated user routes
    let user_routes = Router::new()
        .route("/users/me/password", patch(update_my_password))
        .route("/users/me/display-name", patch(update_my_display_name))
        .layer(axum::middleware::from_fn_with_state(
            app_state.clone(),
            skyjo_server::require_auth_middleware,
        ));

    // Auth routes (public)
    let auth_routes = Router::new()
        .route("/auth/login", post(auth_login))
        .route("/auth/refresh", post(auth_refresh))
        .route("/auth/logout", post(auth_logout))
        .route("/auth/setup-status", get(auth_setup_status))
        .route("/auth/setup", post(auth_setup))
        .route(
            "/auth/me",
            get(auth_me).layer(axum::middleware::from_fn_with_state(
                app_state.clone(),
                skyjo_server::require_auth_middleware,
            )),
        );

    // API routes
    let api_routes = Router::new()
        .route("/rooms", post(skyjo_server::create_room))
        .route("/rooms/{code}", get(skyjo_server::room_info))
        .route("/rooms/{code}/join", post(skyjo_server::join_room))
        .route("/rooms/{code}/ws", get(ws_upgrade))
        .route("/genetic/model", get(genetic_model))
        .route("/genetic/status", get(skyjo_server::genetic_status))
        .route("/genetic/saved", get(genetic_saved_list))
        .route("/genetic/saved/{name}/model", get(genetic_saved_model))
        .merge(genetic_mutation_routes)
        .merge(admin_routes)
        .merge(user_routes)
        .merge(auth_routes);

    // Build the app — optionally serve static files for single-binary mode
    let app = if let Some(ref static_dir) = args.static_dir {
        use tower_http::services::{ServeDir, ServeFile};
        let index_path = static_dir.join("index.html");
        let static_service =
            ServeDir::new(static_dir).not_found_service(ServeFile::new(&index_path));

        Router::new()
            .nest("/api", api_routes)
            .fallback_service(static_service)
            .layer(axum::middleware::from_fn_with_state(
                app_state.clone(),
                skyjo_server::rate_limit_middleware,
            ))
            .layer(CompressionLayer::new())
            .with_state(app_state.clone())
    } else {
        Router::new()
            .nest("/api", api_routes)
            .layer(axum::middleware::from_fn_with_state(
                app_state.clone(),
                skyjo_server::rate_limit_middleware,
            ))
            .layer(CompressionLayer::new())
            .with_state(app_state.clone())
    };

    let addr = format!("0.0.0.0:{}", args.port);
    tracing::info!("Starting server on {addr}");
    if let Some(ref static_dir) = args.static_dir {
        tracing::info!("Serving static files from {:?}", static_dir);
    } else {
        tracing::info!("Static file serving disabled (use --static-dir to enable)");
    }

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("Failed to bind");

    // Graceful shutdown: listen for SIGINT (and SIGTERM on Unix)
    let shutdown_lobby = app_state.clone();
    let shutdown_persistence = persistence.clone();
    let shutdown_signal = async move {
        #[cfg(unix)]
        {
            let mut sigterm =
                tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                    .expect("Failed to install SIGTERM handler");

            tokio::select! {
                _ = tokio::signal::ctrl_c() => {
                    tracing::info!("Received SIGINT");
                }
                _ = sigterm.recv() => {
                    tracing::info!("Received SIGTERM");
                }
            }
        }

        #[cfg(not(unix))]
        {
            tokio::signal::ctrl_c()
                .await
                .expect("Failed to install Ctrl+C handler");
            tracing::info!("Received SIGINT");
        }

        tracing::info!("Shutdown signal received, notifying clients and snapshotting rooms...");

        // Broadcast ServerShutdown to all connected clients & snapshot rooms
        let mut snapshot_count = 0u32;
        for entry in shutdown_lobby.lobby.rooms.iter() {
            let code = entry.key().clone();
            let room = entry.value().lock().await;

            // Notify all connected players
            for (i, slot) in room.players.iter().enumerate() {
                if slot.connected {
                    let _ = room.broadcast_tx.send((i, ServerMessage::ServerShutdown));
                }
            }

            // Snapshot rooms that have active players or are in-game
            let has_connected = room.players.iter().any(|p| p.connected);
            let snapshot_json = if room.phase != RoomPhase::Lobby || has_connected {
                serde_json::to_string(&room.to_snapshot()).ok()
            } else {
                None
            };

            drop(room);

            if let Some(json) = snapshot_json {
                match shutdown_persistence.save_room_snapshot(&code, &json).await {
                    Ok(()) => {
                        snapshot_count += 1;
                    }
                    Err(err) => {
                        tracing::warn!(
                            "Failed to persist snapshot for room {} during shutdown: {}",
                            code,
                            err
                        );
                    }
                }
            }
        }
        tracing::info!("Snapshotted {snapshot_count} room(s). Shutdown complete.");
    };

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal)
    .await
    .expect("Server error");
}

// --- REST Handlers ---

#[derive(Deserialize)]
struct WsQuery {
    token: String,
    /// Wire format preference: "json" (default) or "msgpack"
    #[serde(default)]
    format: Option<String>,
}

async fn ws_upgrade(
    State(state): State<AppState>,
    Path(code): Path<String>,
    Query(query): Query<WsQuery>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    ws: WebSocketUpgrade,
) -> Result<Response, ServerError> {
    // Authenticate session token
    let (room_code, player_index) = state
        .lobby
        .get_session(&query.token)
        .ok_or(ServerError::Unauthorized)?;

    if room_code != code {
        return Err(ServerError::Unauthorized);
    }

    let room = state
        .lobby
        .get_room(&code)
        .ok_or(ServerError::RoomNotFound)?;

    // Check IP ban
    {
        let room_guard = room.lock().await;
        let ip = addr.ip().to_string();
        if room_guard.is_ip_banned(&ip) {
            return Err(ServerError::Banned);
        }
    }

    let client_ip = addr.ip().to_string();
    let initial_format = match query.format.as_deref() {
        Some("msgpack" | "messagepack") => skyjo_server::messages::WireFormat::MessagePack,
        _ => skyjo_server::messages::WireFormat::Json,
    };
    Ok(ws.on_upgrade(move |socket| async move {
        ws::handle_ws(
            socket,
            state,
            room,
            room_code,
            player_index,
            client_ip,
            initial_format,
        )
        .await;
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
                generations.min(10_000_000)
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
                (target_generation - current).min(10_000_000)
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
            let default_cap = if allow_unlimited { 10_000_000 } else { 50_000 };
            let max_cap = if allow_unlimited { 10_000_000 } else { 50_000 };
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

// --- Auth Handlers ---

#[derive(Deserialize)]
struct LoginRequest {
    username: String,
    password: String,
}

#[derive(Serialize)]
struct LoginResponse {
    access_token: String,
    user: UserInfo,
}

#[derive(Serialize)]
struct UserInfo {
    id: String,
    username: String,
    display_name: String,
    permission: PermissionLevel,
}

impl From<&auth::User> for UserInfo {
    fn from(u: &auth::User) -> Self {
        Self {
            id: u.id.to_string(),
            username: u.username.clone(),
            display_name: u.display_name.clone(),
            permission: u.permission_level,
        }
    }
}

/// Build a Set-Cookie value for the refresh token.
/// Omits `Secure` flag when not behind HTTPS to support plain HTTP dev/docker setups.
fn refresh_cookie(token: &str, max_age_secs: i64, secure: bool) -> String {
    let secure_flag = if secure { " Secure;" } else { "" };
    format!(
        "refresh_token={token}; HttpOnly;{secure_flag} SameSite=Strict; Path=/api/auth; Max-Age={max_age_secs}"
    )
}

fn clear_refresh_cookie(secure: bool) -> String {
    let secure_flag = if secure { " Secure;" } else { "" };
    format!("refresh_token=; HttpOnly;{secure_flag} SameSite=Strict; Path=/api/auth; Max-Age=0")
}

/// Check if the request was forwarded over HTTPS.
fn is_secure_request(headers: &axum::http::HeaderMap) -> bool {
    headers
        .get("x-forwarded-proto")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|proto| proto.eq_ignore_ascii_case("https"))
}

async fn auth_login(
    State(state): State<AppState>,
    req_headers: axum::http::HeaderMap,
    Json(req): Json<LoginRequest>,
) -> Result<Response, ServerError> {
    let user = auth::find_user_by_username(state.persistence.pool(), &req.username)
        .await?
        .ok_or(ServerError::Unauthorized)?;

    if !auth::verify_password(&req.password, &user.password_hash)? {
        return Err(ServerError::Unauthorized);
    }

    let access_token = auth::create_access_token(&user, &state.jwt_secret)?;

    // Create refresh token
    let refresh_token = auth::generate_refresh_token();
    let token_hash = auth::hash_refresh_token(&refresh_token);
    let expires_at = auth::refresh_token_expiry();
    auth::store_refresh_token(state.persistence.pool(), user.id, &token_hash, expires_at).await?;

    let body = LoginResponse {
        access_token,
        user: UserInfo::from(&user),
    };

    let secure = is_secure_request(&req_headers);
    let mut response = Json(body).into_response();
    let cookie = refresh_cookie(&refresh_token, 7 * 24 * 60 * 60, secure);
    response
        .headers_mut()
        .insert(header::SET_COOKIE, HeaderValue::from_str(&cookie).unwrap());
    Ok(response)
}

async fn auth_refresh(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
) -> Result<Response, ServerError> {
    // Extract refresh token from cookie
    let cookie_header = headers
        .get(axum::http::header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let refresh_token = cookie_header
        .split(';')
        .filter_map(|c| {
            let c = c.trim();
            c.strip_prefix("refresh_token=")
        })
        .next()
        .ok_or(ServerError::Unauthorized)?;

    let token_hash = auth::hash_refresh_token(refresh_token);

    let user_id = auth::validate_refresh_token(state.persistence.pool(), &token_hash)
        .await?
        .ok_or(ServerError::Unauthorized)?;

    // Revoke old refresh token
    auth::revoke_refresh_token(state.persistence.pool(), &token_hash).await?;

    let user = auth::find_user_by_id(state.persistence.pool(), user_id)
        .await?
        .ok_or(ServerError::Unauthorized)?;

    let access_token = auth::create_access_token(&user, &state.jwt_secret)?;

    // Issue new refresh token (rotation)
    let new_refresh_token = auth::generate_refresh_token();
    let new_token_hash = auth::hash_refresh_token(&new_refresh_token);
    let expires_at = auth::refresh_token_expiry();
    auth::store_refresh_token(
        state.persistence.pool(),
        user.id,
        &new_token_hash,
        expires_at,
    )
    .await?;

    let body = LoginResponse {
        access_token,
        user: UserInfo::from(&user),
    };

    let mut response = Json(body).into_response();
    let secure = is_secure_request(&headers);
    let cookie = refresh_cookie(&new_refresh_token, 7 * 24 * 60 * 60, secure);
    response
        .headers_mut()
        .insert(header::SET_COOKIE, HeaderValue::from_str(&cookie).unwrap());
    Ok(response)
}

async fn auth_logout(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
) -> Result<Response, ServerError> {
    let cookie_header = headers
        .get(axum::http::header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    if let Some(refresh_token) = cookie_header
        .split(';')
        .filter_map(|c| {
            let c = c.trim();
            c.strip_prefix("refresh_token=")
        })
        .next()
    {
        let token_hash = auth::hash_refresh_token(refresh_token);
        auth::revoke_refresh_token(state.persistence.pool(), &token_hash).await?;
    }

    let secure = is_secure_request(&headers);
    let mut response = Json(serde_json::json!({"ok": true})).into_response();
    response.headers_mut().insert(
        header::SET_COOKIE,
        HeaderValue::from_str(&clear_refresh_cookie(secure)).unwrap(),
    );
    Ok(response)
}

async fn auth_me(Extension(user): Extension<AuthUser>) -> Json<UserInfo> {
    Json(UserInfo {
        id: user.id.to_string(),
        username: user.username,
        display_name: user.display_name,
        permission: user.permission,
    })
}

// --- Setup Handlers ---

#[derive(Serialize)]
struct SetupStatus {
    needs_setup: bool,
}

async fn auth_setup_status(
    State(state): State<AppState>,
) -> Result<Json<SetupStatus>, ServerError> {
    let count = auth::user_count(state.persistence.pool()).await?;
    Ok(Json(SetupStatus {
        needs_setup: count == 0,
    }))
}

#[derive(Deserialize)]
struct SetupRequest {
    username: String,
    password: String,
    display_name: Option<String>,
}

async fn auth_setup(
    State(state): State<AppState>,
    req_headers: axum::http::HeaderMap,
    Json(req): Json<SetupRequest>,
) -> Result<Response, ServerError> {
    // Only allow setup when no users exist
    let count = auth::user_count(state.persistence.pool()).await?;
    if count > 0 {
        return Err(ServerError::InvalidAction(
            "Setup already completed — an admin account exists".to_string(),
        ));
    }

    let username = req.username.trim().to_string();
    if username.is_empty() {
        return Err(ServerError::InvalidAction(
            "Username cannot be empty".to_string(),
        ));
    }
    if req.password.len() < 8 {
        return Err(ServerError::InvalidAction(
            "Password must be at least 8 characters".to_string(),
        ));
    }

    let display_name = req
        .display_name
        .map(|d| d.trim().to_string())
        .filter(|d| !d.is_empty())
        .unwrap_or_else(|| username.clone());

    let user = auth::create_user(
        state.persistence.pool(),
        &username,
        &req.password,
        &display_name,
        PermissionLevel::Admin,
    )
    .await?;

    tracing::info!("Setup complete: created admin account '{}'", user.username);

    // Auto-login the new admin
    let access_token = auth::create_access_token(&user, &state.jwt_secret)?;
    let refresh_token = auth::generate_refresh_token();
    let token_hash = auth::hash_refresh_token(&refresh_token);
    let expires_at = auth::refresh_token_expiry();
    auth::store_refresh_token(state.persistence.pool(), user.id, &token_hash, expires_at).await?;

    let body = LoginResponse {
        access_token,
        user: UserInfo::from(&user),
    };

    let secure = is_secure_request(&req_headers);
    let mut response = Json(body).into_response();
    let cookie = refresh_cookie(&refresh_token, 7 * 24 * 60 * 60, secure);
    response
        .headers_mut()
        .insert(header::SET_COOKIE, HeaderValue::from_str(&cookie).unwrap());
    Ok(response)
}

// --- Admin Handlers ---

#[derive(Serialize)]
struct AdminUserInfo {
    id: String,
    username: String,
    display_name: String,
    permission: PermissionLevel,
    created_at: String,
}

async fn admin_list_users(
    State(state): State<AppState>,
) -> Result<Json<Vec<AdminUserInfo>>, ServerError> {
    let users = auth::list_all_users(state.persistence.pool()).await?;
    let infos: Vec<AdminUserInfo> = users
        .iter()
        .map(|u| AdminUserInfo {
            id: u.id.to_string(),
            username: u.username.clone(),
            display_name: u.display_name.clone(),
            permission: u.permission_level,
            created_at: u.created_at.to_rfc3339(),
        })
        .collect();
    Ok(Json(infos))
}

#[derive(Deserialize)]
struct CreateUserRequest {
    username: String,
    display_name: Option<String>,
    permission: Option<PermissionLevel>,
}

#[derive(Serialize)]
struct CreateUserResponse {
    user: AdminUserInfo,
    password: String,
}

async fn admin_create_user(
    State(state): State<AppState>,
    Json(req): Json<CreateUserRequest>,
) -> Result<Json<CreateUserResponse>, ServerError> {
    let username = req.username.trim().to_string();
    if username.is_empty() {
        return Err(ServerError::InvalidAction(
            "Username cannot be empty".to_string(),
        ));
    }

    let password = auth::generate_random_password();
    let display_name = req.display_name.unwrap_or_else(|| username.clone());
    let permission = req.permission.unwrap_or(PermissionLevel::User);

    let user = auth::create_user(
        state.persistence.pool(),
        &username,
        &password,
        &display_name,
        permission,
    )
    .await?;

    Ok(Json(CreateUserResponse {
        user: AdminUserInfo {
            id: user.id.to_string(),
            username: user.username,
            display_name: user.display_name,
            permission: user.permission_level,
            created_at: user.created_at.to_rfc3339(),
        },
        password,
    }))
}

#[derive(Deserialize)]
struct UpdatePermissionRequest {
    permission: PermissionLevel,
}

async fn admin_update_permission(
    State(state): State<AppState>,
    Extension(admin): Extension<AuthUser>,
    Path(user_id): Path<uuid::Uuid>,
    Json(req): Json<UpdatePermissionRequest>,
) -> Result<Json<serde_json::Value>, ServerError> {
    if admin.id == user_id {
        return Err(ServerError::InvalidAction(
            "Cannot change your own permissions".to_string(),
        ));
    }

    auth::update_user_permission(state.persistence.pool(), user_id, req.permission).await?;

    Ok(Json(serde_json::json!({"ok": true})))
}

async fn admin_delete_user(
    State(state): State<AppState>,
    Extension(admin): Extension<AuthUser>,
    Path(user_id): Path<uuid::Uuid>,
) -> Result<Json<serde_json::Value>, ServerError> {
    if admin.id == user_id {
        return Err(ServerError::InvalidAction(
            "Cannot delete your own account".to_string(),
        ));
    }

    // Revoke all their tokens first
    auth::revoke_all_user_tokens(state.persistence.pool(), user_id).await?;
    auth::delete_user(state.persistence.pool(), user_id).await?;

    Ok(Json(serde_json::json!({"deleted": user_id.to_string()})))
}

#[derive(Serialize)]
struct AppSettings {
    registration_enabled: bool,
}

async fn admin_get_settings(
    State(state): State<AppState>,
) -> Result<Json<AppSettings>, ServerError> {
    let reg = auth::get_app_setting(state.persistence.pool(), "registration_enabled")
        .await?
        .unwrap_or_else(|| "false".to_string());

    Ok(Json(AppSettings {
        registration_enabled: reg == "true",
    }))
}

#[derive(Deserialize)]
struct UpdateSettingsRequest {
    registration_enabled: Option<bool>,
}

async fn admin_update_settings(
    State(state): State<AppState>,
    Json(req): Json<UpdateSettingsRequest>,
) -> Result<Json<AppSettings>, ServerError> {
    if let Some(reg) = req.registration_enabled {
        auth::set_app_setting(
            state.persistence.pool(),
            "registration_enabled",
            if reg { "true" } else { "false" },
        )
        .await?;
    }

    let reg = auth::get_app_setting(state.persistence.pool(), "registration_enabled")
        .await?
        .unwrap_or_else(|| "false".to_string());

    Ok(Json(AppSettings {
        registration_enabled: reg == "true",
    }))
}

// --- User Self-Service Handlers ---

#[derive(Deserialize)]
struct UpdatePasswordRequest {
    current_password: String,
    new_password: String,
}

async fn update_my_password(
    State(state): State<AppState>,
    Extension(auth_user): Extension<AuthUser>,
    Json(req): Json<UpdatePasswordRequest>,
) -> Result<Json<serde_json::Value>, ServerError> {
    let user = auth::find_user_by_id(state.persistence.pool(), auth_user.id)
        .await?
        .ok_or(ServerError::Unauthorized)?;

    if !auth::verify_password(&req.current_password, &user.password_hash)? {
        return Err(ServerError::InvalidAction(
            "Current password is incorrect".to_string(),
        ));
    }

    if req.new_password.len() < 8 {
        return Err(ServerError::InvalidAction(
            "New password must be at least 8 characters".to_string(),
        ));
    }

    let new_hash = auth::hash_password(&req.new_password)?;
    auth::update_user_password(state.persistence.pool(), auth_user.id, &new_hash).await?;

    // Revoke all refresh tokens (force re-login)
    auth::revoke_all_user_tokens(state.persistence.pool(), auth_user.id).await?;

    Ok(Json(serde_json::json!({"ok": true})))
}

#[derive(Deserialize)]
struct UpdateDisplayNameRequest {
    display_name: String,
}

async fn update_my_display_name(
    State(state): State<AppState>,
    Extension(auth_user): Extension<AuthUser>,
    Json(req): Json<UpdateDisplayNameRequest>,
) -> Result<Json<serde_json::Value>, ServerError> {
    let display_name = req.display_name.trim().to_string();
    if display_name.is_empty() {
        return Err(ServerError::InvalidAction(
            "Display name cannot be empty".to_string(),
        ));
    }
    if display_name.len() > 32 {
        return Err(ServerError::InvalidAction(
            "Display name must be 32 characters or fewer".to_string(),
        ));
    }

    auth::update_user_display_name(state.persistence.pool(), auth_user.id, &display_name).await?;

    Ok(Json(
        serde_json::json!({"ok": true, "display_name": display_name}),
    ))
}
