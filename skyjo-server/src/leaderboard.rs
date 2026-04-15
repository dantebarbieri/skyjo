use axum::extract::{Path, Query, State};
use axum::response::Json;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::AppState;
use crate::auth::{self, AuthUser};
use crate::error::ServerError;
use crate::persistence::{GameDetail, GameListParams, GameSummary};

// ── Optional auth extractor ─────────────────────────────────────────

/// Extracts an authenticated user from the JWT `Authorization` header,
/// returning `None` if the header is missing or invalid instead of
/// rejecting the request.
pub struct OptionalAuth(pub Option<AuthUser>);

impl axum::extract::FromRequestParts<AppState> for OptionalAuth {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let user = parts
            .headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .and_then(|h| h.strip_prefix("Bearer "))
            .and_then(|token| auth::validate_access_token(token, &state.jwt_secret).ok());

        Ok(OptionalAuth(user))
    }
}

// ── Query params ────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ListGamesQuery {
    pub page: Option<i32>,
    pub per_page: Option<i32>,
    pub sort_by: Option<String>,
    pub sort_order: Option<String>,
    pub player_name: Option<String>,
    pub user_id: Option<Uuid>,
    pub min_players: Option<i32>,
    pub max_players: Option<i32>,
    pub rules: Option<String>,
}

// ── Response types ──────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct GameListApiResponse {
    pub games: Vec<GameSummaryWithScore>,
    pub total: i64,
    pub page: i32,
    pub per_page: i32,
}

#[derive(Debug, Serialize)]
pub struct GameSummaryWithScore {
    #[serde(flatten)]
    pub game: GameSummary,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub your_score: Option<i32>,
}

// ── Handlers ────────────────────────────────────────────────────────

/// `GET /api/games` — List completed games with optional auth annotation.
pub async fn list_games(
    State(state): State<AppState>,
    optional_auth: OptionalAuth,
    Query(query): Query<ListGamesQuery>,
) -> Result<Json<GameListApiResponse>, ServerError> {
    // user_id filtering requires authentication and is restricted to the caller's own ID.
    let user_id_filter = match query.user_id {
        Some(requested_id) => {
            let auth_user = optional_auth.0.as_ref().ok_or(ServerError::Unauthorized)?;
            if auth_user.id != requested_id {
                return Err(ServerError::Forbidden);
            }
            Some(requested_id)
        }
        None => None,
    };

    let params = GameListParams {
        page: query.page,
        per_page: query.per_page,
        sort_by: query.sort_by,
        sort_order: query.sort_order,
        player_name: query.player_name,
        user_id: user_id_filter,
        min_players: query.min_players,
        max_players: query.max_players,
        rules: query.rules,
    };

    let response = state
        .persistence
        .list_games(&params)
        .await
        .map_err(|e| ServerError::InternalError(format!("persistence error: {e}")))?;

    let user_id = optional_auth.0.map(|u| u.id);

    let games = annotate_games_with_scores(&state, response.games, user_id).await;

    Ok(Json(GameListApiResponse {
        games,
        total: response.total,
        page: response.page,
        per_page: response.per_page,
    }))
}

/// `GET /api/games/:id` — Game detail with round-by-round scores.
pub async fn get_game_detail(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<GameDetail>, ServerError> {
    let detail = state
        .persistence
        .get_game_detail(id)
        .await
        .map_err(persistence_error_to_server_error)?;

    Ok(Json(detail))
}

/// `GET /api/games/:id/replay` — Full game history for client-side replay.
pub async fn get_game_replay(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<skyjo_core::history::GameHistory>, ServerError> {
    let history = state
        .persistence
        .reconstruct_game_history(id)
        .await
        .map_err(persistence_error_to_server_error)?;

    Ok(Json(history))
}

// ── Helpers ─────────────────────────────────────────────────────────

/// Convert `PersistenceError` to `ServerError`, mapping `NotFound` to 404.
fn persistence_error_to_server_error(e: crate::persistence::PersistenceError) -> ServerError {
    match e {
        crate::persistence::PersistenceError::NotFound(_) => ServerError::GameNotFound,
        other => ServerError::InternalError(format!("persistence error: {other}")),
    }
}

/// For each game in the list, look up the authenticated user's score
/// (if any) and attach it as `your_score`.
// TODO: N+1 query — batch into a single query with WHERE game_id = ANY($1)
async fn annotate_games_with_scores(
    state: &AppState,
    games: Vec<GameSummary>,
    user_id: Option<Uuid>,
) -> Vec<GameSummaryWithScore> {
    let Some(uid) = user_id else {
        return games
            .into_iter()
            .map(|game| GameSummaryWithScore {
                game,
                your_score: None,
            })
            .collect();
    };

    let mut result = Vec::with_capacity(games.len());
    for game in games {
        let your_score = state
            .persistence
            .get_user_score_for_game(game.id, uid)
            .await
            .ok()
            .flatten();
        result.push(GameSummaryWithScore { game, your_score });
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn persistence_not_found_maps_to_game_not_found() {
        let err = persistence_error_to_server_error(
            crate::persistence::PersistenceError::NotFound("game xyz".into()),
        );
        assert_eq!(err, ServerError::GameNotFound);
    }

    #[test]
    fn persistence_internal_maps_to_internal_error() {
        let err = persistence_error_to_server_error(crate::persistence::PersistenceError::Json(
            serde_json::from_str::<()>("bad").unwrap_err(),
        ));
        assert!(matches!(err, ServerError::InternalError(_)));
    }
}
