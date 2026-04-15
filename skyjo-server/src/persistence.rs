use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use skyjo_core::history::{ColumnClearEvent, GameHistory, RoundHistory, TurnAction, TurnRecord};
use skyjo_core::strategy::DeckDrawAction;
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use uuid::Uuid;

#[derive(Debug)]
pub enum PersistenceError {
    Sqlx(sqlx::Error),
    Io(std::io::Error),
    Json(serde_json::Error),
    NotFound(String),
}

impl std::fmt::Display for PersistenceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Sqlx(e) => write!(f, "database error: {e}"),
            Self::Io(e) => write!(f, "IO error: {e}"),
            Self::Json(e) => write!(f, "JSON error: {e}"),
            Self::NotFound(msg) => write!(f, "not found: {msg}"),
        }
    }
}

impl std::error::Error for PersistenceError {}

impl From<sqlx::Error> for PersistenceError {
    fn from(e: sqlx::Error) -> Self {
        Self::Sqlx(e)
    }
}
impl From<std::io::Error> for PersistenceError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}
impl From<serde_json::Error> for PersistenceError {
    fn from(e: serde_json::Error) -> Self {
        Self::Json(e)
    }
}

/// Persistent storage layer backed by PostgreSQL.
#[derive(Clone)]
pub struct Persistence {
    pool: PgPool,
}

impl Persistence {
    /// Connect to PostgreSQL and run migrations.
    pub async fn connect(database_url: &str) -> Result<Self, PersistenceError> {
        let pool = PgPoolOptions::new()
            .max_connections(10)
            .connect(database_url)
            .await?;

        let persistence = Self { pool };
        persistence.migrate().await?;
        Ok(persistence)
    }

    /// Get a reference to the connection pool.
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Run schema migrations.
    async fn migrate(&self) -> Result<(), PersistenceError> {
        // Create a migrations tracking table
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS _migrations (
                name TEXT PRIMARY KEY,
                applied_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            )",
        )
        .execute(&self.pool)
        .await?;

        let migrations: &[(&str, &str)] = &[
            (
                "001_initial",
                include_str!("../../migrations/001_initial.sql"),
            ),
            (
                "002_users_auth",
                include_str!("../../migrations/002_users_auth.sql"),
            ),
            (
                "003_leaderboard",
                include_str!("../../migrations/003_leaderboard.sql"),
            ),
        ];

        for (name, sql) in migrations {
            let applied: bool =
                sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM _migrations WHERE name = $1)")
                    .bind(name)
                    .fetch_one(&self.pool)
                    .await?;

            if !applied {
                // Wrap migration + tracking insert in a transaction for atomicity
                let mut tx = self.pool.begin().await?;
                sqlx::raw_sql(sql).execute(&mut *tx).await?;
                sqlx::query("INSERT INTO _migrations (name) VALUES ($1)")
                    .bind(name)
                    .execute(&mut *tx)
                    .await?;
                tx.commit().await?;
                tracing::info!("Applied migration: {name}");
            }
        }

        Ok(())
    }

    /// Store a game replay (compressed with zstd).
    #[allow(clippy::too_many_arguments)]
    pub async fn save_replay(
        &self,
        id: &str,
        room_code: &str,
        players: &[String],
        rules: &str,
        seed: u64,
        history_json: &str,
        winner_indices: &[usize],
    ) -> Result<(), PersistenceError> {
        let compressed = zstd::encode_all(history_json.as_bytes(), 3)?;
        let players_json = serde_json::to_value(players)?;
        let winners_json = serde_json::to_value(winner_indices)?;

        sqlx::query(
            "INSERT INTO game_replays (id, room_code, players, rules, seed, history, winner_indices)
             VALUES ($1, $2, $3, $4, $5, $6, $7)
             ON CONFLICT (id) DO UPDATE SET
                room_code = EXCLUDED.room_code,
                players = EXCLUDED.players,
                rules = EXCLUDED.rules,
                seed = EXCLUDED.seed,
                history = EXCLUDED.history,
                winner_indices = EXCLUDED.winner_indices",
        )
        .bind(id)
        .bind(room_code)
        .bind(&players_json)
        .bind(rules)
        .bind(seed as i64)
        .bind(&compressed)
        .bind(&winners_json)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Load a game replay by ID, decompressing the history.
    pub async fn load_replay(
        &self,
        id: &str,
    ) -> Result<Option<(String, Vec<u8>)>, PersistenceError> {
        let row: Option<(serde_json::Value, Vec<u8>)> =
            sqlx::query_as("SELECT players, history FROM game_replays WHERE id = $1")
                .bind(id)
                .fetch_optional(&self.pool)
                .await?;

        match row {
            Some((players, compressed)) => {
                let decompressed = zstd::decode_all(compressed.as_slice())?;
                Ok(Some((players.to_string(), decompressed)))
            }
            None => Ok(None),
        }
    }

    /// Update player statistics after a game.
    pub async fn update_player_stats(
        &self,
        player_name: &str,
        rules: &str,
        score: i32,
        won: bool,
    ) -> Result<(), PersistenceError> {
        let won_int = if won { 1 } else { 0 };
        sqlx::query(
            "INSERT INTO player_stats (player_name, rules, games_played, games_won, total_score, best_score, worst_score)
             VALUES ($1, $2, 1, $3, $4, $4, $4)
             ON CONFLICT(player_name, rules) DO UPDATE SET
                games_played = player_stats.games_played + 1,
                games_won = player_stats.games_won + $3,
                total_score = player_stats.total_score + $4,
                best_score = LEAST(player_stats.best_score, $4),
                worst_score = GREATEST(player_stats.worst_score, $4)",
        )
        .bind(player_name)
        .bind(rules)
        .bind(won_int)
        .bind(score)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Get player stats.
    pub async fn get_player_stats(
        &self,
        player_name: &str,
    ) -> Result<Vec<PlayerStatsRow>, PersistenceError> {
        let rows: Vec<PlayerStatsRow> = sqlx::query_as(
            "SELECT player_name, rules, games_played, games_won, total_score, best_score, worst_score
             FROM player_stats WHERE player_name = $1",
        )
        .bind(player_name)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    /// Save a room snapshot (compressed).
    pub async fn save_room_snapshot(
        &self,
        room_code: &str,
        snapshot_json: &str,
    ) -> Result<(), PersistenceError> {
        let compressed = zstd::encode_all(snapshot_json.as_bytes(), 3)?;

        sqlx::query(
            "INSERT INTO room_snapshots (room_code, snapshot, updated_at)
             VALUES ($1, $2, NOW())
             ON CONFLICT (room_code) DO UPDATE SET
                snapshot = EXCLUDED.snapshot,
                updated_at = NOW()",
        )
        .bind(room_code)
        .bind(&compressed)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Load a room snapshot, decompressing.
    pub async fn load_room_snapshot(
        &self,
        room_code: &str,
    ) -> Result<Option<Vec<u8>>, PersistenceError> {
        let row: Option<(Vec<u8>,)> =
            sqlx::query_as("SELECT snapshot FROM room_snapshots WHERE room_code = $1")
                .bind(room_code)
                .fetch_optional(&self.pool)
                .await?;

        match row {
            Some((compressed,)) => {
                let decompressed = zstd::decode_all(compressed.as_slice())?;
                Ok(Some(decompressed))
            }
            None => Ok(None),
        }
    }

    /// Load all room snapshots (for crash recovery on startup).
    pub async fn load_all_room_snapshots(
        &self,
    ) -> Result<Vec<(String, Vec<u8>)>, PersistenceError> {
        let rows: Vec<(String, Vec<u8>)> =
            sqlx::query_as("SELECT room_code, snapshot FROM room_snapshots")
                .fetch_all(&self.pool)
                .await?;

        let mut results = Vec::new();
        for (code, compressed) in rows {
            let decompressed = zstd::decode_all(compressed.as_slice())?;
            results.push((code, decompressed));
        }
        Ok(results)
    }

    /// Delete a room snapshot (when room is cleaned up).
    pub async fn delete_room_snapshot(&self, room_code: &str) -> Result<(), PersistenceError> {
        sqlx::query("DELETE FROM room_snapshots WHERE room_code = $1")
            .bind(room_code)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // ── Leaderboard: WRITE methods ──────────────────────────────────────

    /// Insert a new game row with state = in_progress.
    pub async fn save_game(
        &self,
        game_id: Uuid,
        room_code: &str,
        rules_name: &str,
        seed: Option<i64>,
    ) -> Result<(), PersistenceError> {
        sqlx::query(
            "INSERT INTO games (id, room_code, rules_name, seed, game_state_id)
             VALUES ($1, $2, $3, $4, (SELECT id FROM game_states WHERE name = 'in_progress'))",
        )
        .bind(game_id)
        .bind(room_code)
        .bind(rules_name)
        .bind(seed)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Insert players for a game.
    /// Each tuple: (player_index, player_name, user_id, strategy_name).
    pub async fn save_game_players(
        &self,
        game_id: Uuid,
        players: &[(usize, &str, Option<Uuid>, Option<&str>)],
    ) -> Result<(), PersistenceError> {
        for &(idx, name, user_id, strategy) in players {
            sqlx::query(
                "INSERT INTO game_players (game_id, player_index, player_name, user_id, strategy_name)
                 VALUES ($1, $2, $3, $4, $5)",
            )
            .bind(game_id)
            .bind(idx as i32)
            .bind(name)
            .bind(user_id)
            .bind(strategy)
            .execute(&self.pool)
            .await?;
        }
        Ok(())
    }

    /// Insert a round and its associated initial deck, dealt cards, and setup flips.
    /// Returns the auto-generated round_id.
    ///
    /// `dealt_cards`: (player_index, slot_index, card_value)
    /// `setup_flips`: (player_index, flip_index, slot_index)
    #[allow(clippy::too_many_arguments)]
    pub async fn save_game_round(
        &self,
        game_id: Uuid,
        round_number: i32,
        starting_player: i32,
        going_out_player: Option<i32>,
        truncated: bool,
        initial_deck: &[i8],
        dealt_cards: &[(i32, i32, i8)],
        setup_flips: &[(i32, i32, i32)],
    ) -> Result<i32, PersistenceError> {
        let round_id: i32 = sqlx::query_scalar(
            "INSERT INTO game_rounds (game_id, round_number, starting_player, going_out_player, truncated)
             VALUES ($1, $2, $3, $4, $5) RETURNING id",
        )
        .bind(game_id)
        .bind(round_number)
        .bind(starting_player)
        .bind(going_out_player)
        .bind(truncated)
        .fetch_one(&self.pool)
        .await?;

        // Initial deck order
        for (pos, &card) in initial_deck.iter().enumerate() {
            sqlx::query(
                "INSERT INTO round_initial_deck (round_id, position, card_value)
                 VALUES ($1, $2, $3)",
            )
            .bind(round_id)
            .bind(pos as i32)
            .bind(card as i16)
            .execute(&self.pool)
            .await?;
        }

        // Dealt cards
        for &(player_idx, slot_idx, card_val) in dealt_cards {
            sqlx::query(
                "INSERT INTO round_dealt_cards (round_id, player_index, slot_index, card_value)
                 VALUES ($1, $2, $3, $4)",
            )
            .bind(round_id)
            .bind(player_idx)
            .bind(slot_idx)
            .bind(card_val as i16)
            .execute(&self.pool)
            .await?;
        }

        // Setup flips
        for &(player_idx, flip_idx, slot_idx) in setup_flips {
            sqlx::query(
                "INSERT INTO round_setup_flips (round_id, player_index, flip_index, slot_index)
                 VALUES ($1, $2, $3, $4)",
            )
            .bind(round_id)
            .bind(player_idx)
            .bind(flip_idx)
            .bind(slot_idx)
            .execute(&self.pool)
            .await?;
        }

        Ok(round_id)
    }

    /// Insert round scores. Each tuple: (player_index, raw_score, adjusted_score).
    pub async fn save_round_scores(
        &self,
        round_id: i32,
        scores: &[(i32, i32, i32)],
    ) -> Result<(), PersistenceError> {
        for &(player_idx, raw, adjusted) in scores {
            sqlx::query(
                "INSERT INTO round_scores (round_id, player_index, raw_score, adjusted_score)
                 VALUES ($1, $2, $3, $4)",
            )
            .bind(round_id)
            .bind(player_idx)
            .bind(raw)
            .bind(adjusted)
            .execute(&self.pool)
            .await?;
        }
        Ok(())
    }

    /// Insert round turns.
    pub async fn save_round_turns(
        &self,
        round_id: i32,
        turns: &[RoundTurnRow],
    ) -> Result<(), PersistenceError> {
        for t in turns {
            sqlx::query(
                "INSERT INTO round_turns (round_id, turn_index, player_index, action_kind_id,
                    drawn_card, target_position, replaced_card, flipped_card)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
            )
            .bind(round_id)
            .bind(t.turn_index)
            .bind(t.player_index)
            .bind(t.action_kind_id)
            .bind(t.drawn_card)
            .bind(t.target_position)
            .bind(t.replaced_card)
            .bind(t.flipped_card)
            .execute(&self.pool)
            .await?;
        }
        Ok(())
    }

    /// Insert column clears for a round.
    pub async fn save_column_clears(
        &self,
        round_id: i32,
        clears: &[ColumnClearRow],
    ) -> Result<(), PersistenceError> {
        for c in clears {
            sqlx::query(
                "INSERT INTO column_clears (round_id, clear_kind_id, turn_index,
                    column_index, card_value, player_index, cards_cleared)
                 VALUES ($1, $2, $3, $4, $5, $6, $7)",
            )
            .bind(round_id)
            .bind(c.clear_kind_id)
            .bind(c.turn_index)
            .bind(c.column_index)
            .bind(c.card_value)
            .bind(c.player_index)
            .bind(c.cards_cleared)
            .execute(&self.pool)
            .await?;
        }
        Ok(())
    }

    /// Update game state (e.g. "completed", "abandoned").
    pub async fn update_game_state(
        &self,
        game_id: Uuid,
        state_name: &str,
    ) -> Result<(), PersistenceError> {
        sqlx::query(
            "UPDATE games SET game_state_id = (SELECT id FROM game_states WHERE name = $1)
             WHERE id = $2",
        )
        .bind(state_name)
        .bind(game_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Save a complete game from a `GameHistory` in a single transaction.
    /// This is the main entry point called from game-over handling.
    pub async fn save_complete_game(
        &self,
        game_id: Uuid,
        room_code: &str,
        rules_name: &str,
        seed: Option<i64>,
        history: &GameHistory,
        players: &[(usize, &str, Option<Uuid>, Option<&str>)],
    ) -> Result<(), PersistenceError> {
        let num_rows = num_rows_for_rules(rules_name);
        let mut tx = self.pool.begin().await?;

        // 1. Create game row (completed)
        sqlx::query(
            "INSERT INTO games (id, room_code, rules_name, seed, game_state_id)
             VALUES ($1, $2, $3, $4, (SELECT id FROM game_states WHERE name = 'completed'))",
        )
        .bind(game_id)
        .bind(room_code)
        .bind(rules_name)
        .bind(seed)
        .execute(&mut *tx)
        .await?;

        // 2. Save players
        for &(idx, name, user_id, strategy) in players {
            sqlx::query(
                "INSERT INTO game_players (game_id, player_index, player_name, user_id, strategy_name)
                 VALUES ($1, $2, $3, $4, $5)",
            )
            .bind(game_id)
            .bind(idx as i32)
            .bind(name)
            .bind(user_id)
            .bind(strategy)
            .execute(&mut *tx)
            .await?;
        }

        // 3. Save each round
        for round in &history.rounds {
            let round_id = self
                .save_round_in_tx(&mut tx, game_id, round, num_rows)
                .await?;
            let _ = round_id;
        }

        tx.commit().await?;
        Ok(())
    }

    /// Save a single round within an existing transaction.
    async fn save_round_in_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        game_id: Uuid,
        round: &RoundHistory,
        num_rows: usize,
    ) -> Result<i32, PersistenceError> {
        let round_id: i32 = sqlx::query_scalar(
            "INSERT INTO game_rounds (game_id, round_number, starting_player, going_out_player, truncated)
             VALUES ($1, $2, $3, $4, $5) RETURNING id",
        )
        .bind(game_id)
        .bind(round.round_number as i32)
        .bind(round.starting_player as i32)
        .bind(round.going_out_player.map(|p| p as i32))
        .bind(round.truncated)
        .fetch_one(&mut **tx)
        .await?;

        // Initial deck order
        for (pos, &card) in round.initial_deck_order.iter().enumerate() {
            sqlx::query(
                "INSERT INTO round_initial_deck (round_id, position, card_value)
                 VALUES ($1, $2, $3)",
            )
            .bind(round_id)
            .bind(pos as i32)
            .bind(card as i16)
            .execute(&mut **tx)
            .await?;
        }

        // Dealt cards: dealt_hands is Vec<Vec<CardValue>> — one vec per player
        for (player_idx, hand) in round.dealt_hands.iter().enumerate() {
            for (slot_idx, &card) in hand.iter().enumerate() {
                sqlx::query(
                    "INSERT INTO round_dealt_cards (round_id, player_index, slot_index, card_value)
                     VALUES ($1, $2, $3, $4)",
                )
                .bind(round_id)
                .bind(player_idx as i32)
                .bind(slot_idx as i32)
                .bind(card as i16)
                .execute(&mut **tx)
                .await?;
            }
        }

        // Setup flips: setup_flips is Vec<Vec<usize>> — one vec of slot indices per player
        for (player_idx, flips) in round.setup_flips.iter().enumerate() {
            for (flip_idx, &slot_idx) in flips.iter().enumerate() {
                sqlx::query(
                    "INSERT INTO round_setup_flips (round_id, player_index, flip_index, slot_index)
                     VALUES ($1, $2, $3, $4)",
                )
                .bind(round_id)
                .bind(player_idx as i32)
                .bind(flip_idx as i32)
                .bind(slot_idx as i32)
                .execute(&mut **tx)
                .await?;
            }
        }

        // Turns: build board state to compute flipped_card values
        let mut boards = build_initial_boards(&round.dealt_hands);
        let mut all_clears: Vec<ColumnClearRow> = Vec::new();

        for (turn_idx, turn) in round.turns.iter().enumerate() {
            let (action_kind_id, drawn_card, target_pos, replaced_card, flipped_card) =
                decompose_turn_action(&turn.action, turn.player_index, &mut boards);

            sqlx::query(
                "INSERT INTO round_turns (round_id, turn_index, player_index, action_kind_id,
                    drawn_card, target_position, replaced_card, flipped_card)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
            )
            .bind(round_id)
            .bind(turn_idx as i32)
            .bind(turn.player_index as i32)
            .bind(action_kind_id)
            .bind(drawn_card)
            .bind(target_pos)
            .bind(replaced_card)
            .bind(flipped_card)
            .execute(&mut **tx)
            .await?;

            // Column clears during turns (clear_kind_id = 1 = 'turn')
            for clear in &turn.column_clears {
                all_clears.push(ColumnClearRow {
                    clear_kind_id: 1,
                    turn_index: Some(turn_idx as i32),
                    column_index: clear.column as i32,
                    card_value: clear.card_value as i16,
                    player_index: clear.player_index as i32,
                    cards_cleared: num_rows as i32,
                });
                mark_column_cleared(&mut boards, clear.player_index, clear.column, num_rows);
            }
        }

        // End-of-round clears (clear_kind_id = 2 = 'round_end')
        for clear in &round.end_of_round_clears {
            all_clears.push(ColumnClearRow {
                clear_kind_id: 2,
                turn_index: None,
                column_index: clear.column as i32,
                card_value: clear.card_value as i16,
                player_index: clear.player_index as i32,
                cards_cleared: num_rows as i32,
            });
        }

        // Write all column clears
        for c in &all_clears {
            sqlx::query(
                "INSERT INTO column_clears (round_id, clear_kind_id, turn_index,
                    column_index, card_value, player_index, cards_cleared)
                 VALUES ($1, $2, $3, $4, $5, $6, $7)",
            )
            .bind(round_id)
            .bind(c.clear_kind_id)
            .bind(c.turn_index)
            .bind(c.column_index)
            .bind(c.card_value)
            .bind(c.player_index)
            .bind(c.cards_cleared)
            .execute(&mut **tx)
            .await?;
        }

        // Round scores: GameHistory stores post-penalty scores in round_scores.
        // We store them as both raw_score and adjusted_score.
        // The going-out penalty is already applied in round_scores.
        for (player_idx, &score) in round.round_scores.iter().enumerate() {
            let (raw, adjusted) = compute_raw_adjusted(
                score,
                player_idx,
                round.going_out_player,
                &round.round_scores,
            );
            sqlx::query(
                "INSERT INTO round_scores (round_id, player_index, raw_score, adjusted_score)
                 VALUES ($1, $2, $3, $4)",
            )
            .bind(round_id)
            .bind(player_idx as i32)
            .bind(raw)
            .bind(adjusted)
            .execute(&mut **tx)
            .await?;
        }

        Ok(round_id)
    }

    // ── Leaderboard: READ methods ───────────────────────────────────────

    /// List completed games with pagination, filtering, and sorting.
    pub async fn list_games(
        &self,
        params: &GameListParams,
    ) -> Result<GameListResponse, PersistenceError> {
        let page = params.page.unwrap_or(1).max(1);
        let per_page = params.per_page.unwrap_or(20).clamp(1, 100);
        let offset = (page - 1) * per_page;

        let sort_col = match params.sort_by.as_deref() {
            Some("num_players") => "gs.num_players",
            Some("num_rounds") => "gs.num_rounds",
            Some("best_winner") => "best_winner_score",
            Some("worst_winner") => "worst_winner_score",
            Some("best_loser") => "best_loser_score",
            Some("worst_loser") => "worst_loser_score",
            _ => "gs.created_at",
        };
        let sort_dir = match params.sort_order.as_deref() {
            Some("asc") => "ASC",
            _ => "DESC",
        };

        // Build WHERE clause with positional params starting at $1
        let mut conditions = vec!["gs.game_state = 'completed'".to_string()];
        let mut param_idx = 0u32;

        if params.player_name.is_some() {
            param_idx += 1;
            conditions.push(format!(
                "EXISTS (SELECT 1 FROM game_players gp2 WHERE gp2.game_id = gs.game_id AND gp2.player_name = ${param_idx})"
            ));
        }
        if params.rules.is_some() {
            param_idx += 1;
            conditions.push(format!("gs.rules_name = ${param_idx}"));
        }
        if params.min_players.is_some() {
            param_idx += 1;
            conditions.push(format!("gs.num_players >= ${param_idx}"));
        }
        if params.max_players.is_some() {
            param_idx += 1;
            conditions.push(format!("gs.num_players <= ${param_idx}"));
        }

        let where_clause = conditions.join(" AND ");

        // Count query (uses same param positions)
        let total: i64 = self.count_games_internal(&where_clause, params).await?;

        // Fetch game list with LIMIT/OFFSET using param positions after the filter params
        let limit_param = param_idx + 1;
        let offset_param = param_idx + 2;

        let games_rows: Vec<GameSummaryRow> = self
            .list_games_internal(
                &where_clause,
                sort_col,
                sort_dir,
                per_page,
                offset,
                limit_param,
                offset_param,
                params,
            )
            .await?;

        // Fetch players for each game
        let mut games = Vec::with_capacity(games_rows.len());
        for row in &games_rows {
            let player_rows: Vec<GamePlayerSummaryRow> = sqlx::query_as(
                "SELECT
                    gfs.player_name AS name,
                    gfs.final_score,
                    (gw.player_index IS NOT NULL) AS is_winner,
                    (gp.strategy_name IS NOT NULL) AS is_bot
                 FROM game_final_scores gfs
                 JOIN game_players gp ON gp.game_id = gfs.game_id AND gp.player_index = gfs.player_index
                 LEFT JOIN game_winners gw ON gw.game_id = gfs.game_id AND gw.player_index = gfs.player_index
                 WHERE gfs.game_id = $1
                 ORDER BY gfs.player_index",
            )
            .bind(row.game_id)
            .fetch_all(&self.pool)
            .await?;

            let room_code: Option<String> =
                sqlx::query_scalar("SELECT room_code FROM games WHERE id = $1")
                    .bind(row.game_id)
                    .fetch_optional(&self.pool)
                    .await?
                    .flatten();

            games.push(GameSummary {
                id: row.game_id,
                room_code,
                rules: row.rules_name.clone(),
                num_players: row.num_players,
                num_rounds: row.num_rounds,
                created_at: row.created_at,
                players: player_rows
                    .into_iter()
                    .map(|p| GamePlayerSummary {
                        name: p.name,
                        final_score: p.final_score,
                        is_winner: p.is_winner,
                        is_bot: p.is_bot,
                    })
                    .collect(),
            });
        }

        Ok(GameListResponse {
            games,
            total,
            page,
            per_page,
        })
    }

    /// Internal: count games matching filters.
    async fn count_games_internal(
        &self,
        where_clause: &str,
        params: &GameListParams,
    ) -> Result<i64, PersistenceError> {
        let sql =
            format!("SELECT COUNT(DISTINCT gs.game_id) FROM game_summary gs WHERE {where_clause}");
        let mut query = sqlx::query_scalar::<_, i64>(&sql);
        query = bind_game_list_filters(query, params);
        let count = query.fetch_one(&self.pool).await?;
        Ok(count)
    }

    /// Internal: fetch game list rows matching filters.
    #[allow(clippy::too_many_arguments)]
    async fn list_games_internal(
        &self,
        where_clause: &str,
        sort_col: &str,
        sort_dir: &str,
        per_page: i32,
        offset: i32,
        limit_param: u32,
        offset_param: u32,
        params: &GameListParams,
    ) -> Result<Vec<GameSummaryRow>, PersistenceError> {
        let sql = format!(
            "WITH game_data AS (
                SELECT
                    gs.game_id,
                    gs.rules_name,
                    gs.num_players::INT AS num_players,
                    gs.num_rounds::INT AS num_rounds,
                    gs.created_at,
                    MIN(gw.final_score) AS best_winner_score,
                    MAX(gw.final_score) AS worst_winner_score,
                    MIN(CASE WHEN gw2.player_index IS NULL THEN gfs.final_score END) AS best_loser_score,
                    MAX(CASE WHEN gw2.player_index IS NULL THEN gfs.final_score END) AS worst_loser_score
                FROM game_summary gs
                LEFT JOIN game_winners gw ON gw.game_id = gs.game_id
                LEFT JOIN game_final_scores gfs ON gfs.game_id = gs.game_id
                LEFT JOIN game_winners gw2 ON gw2.game_id = gfs.game_id AND gw2.player_index = gfs.player_index
                WHERE {where_clause}
                GROUP BY gs.game_id, gs.rules_name, gs.num_players, gs.num_rounds, gs.created_at
            )
            SELECT game_id, rules_name, num_players::INT AS num_players, num_rounds::INT AS num_rounds, created_at
            FROM game_data
            ORDER BY {sort_col} {sort_dir} NULLS LAST, game_id
            LIMIT ${limit_param} OFFSET ${offset_param}",
        );
        let mut query = sqlx::query_as::<_, GameSummaryRow>(&sql);
        query = bind_game_list_filters_query_as(query, params);
        query = query.bind(per_page).bind(offset);
        let rows = query.fetch_all(&self.pool).await?;
        Ok(rows)
    }

    /// Get detailed information about a single game.
    pub async fn get_game_detail(&self, game_id: Uuid) -> Result<GameDetail, PersistenceError> {
        // Game summary
        let summary: Option<GameDetailRow> = sqlx::query_as(
            "SELECT
                g.id AS game_id,
                g.room_code,
                g.rules_name,
                gs.num_players::INT AS num_players,
                gs.num_rounds::INT AS num_rounds,
                g.created_at
             FROM games g
             JOIN game_summary gs ON gs.game_id = g.id
             WHERE g.id = $1",
        )
        .bind(game_id)
        .fetch_optional(&self.pool)
        .await?;

        let summary =
            summary.ok_or_else(|| PersistenceError::NotFound(format!("game {game_id}")))?;

        // Players
        let player_rows: Vec<GamePlayerDetailRow> = sqlx::query_as(
            "SELECT
                gfs.player_name AS name,
                gfs.final_score,
                (gw.player_index IS NOT NULL) AS is_winner,
                (gp.strategy_name IS NOT NULL) AS is_bot,
                gp.user_id
             FROM game_final_scores gfs
             JOIN game_players gp ON gp.game_id = gfs.game_id AND gp.player_index = gfs.player_index
             LEFT JOIN game_winners gw ON gw.game_id = gfs.game_id AND gw.player_index = gfs.player_index
             WHERE gfs.game_id = $1
             ORDER BY gfs.player_index",
        )
        .bind(game_id)
        .fetch_all(&self.pool)
        .await?;

        // Rounds with scores
        let score_rows: Vec<RoundScoreRow> = sqlx::query_as(
            "SELECT
                rsd.round_number,
                rsd.player_index,
                rsd.raw_score,
                rsd.adjusted_score,
                rsd.cumulative_score,
                rsd.went_out,
                rsd.was_penalized
             FROM round_score_details rsd
             WHERE rsd.game_id = $1
             ORDER BY rsd.round_number, rsd.player_index",
        )
        .bind(game_id)
        .fetch_all(&self.pool)
        .await?;

        // Group scores by round
        let mut rounds_map: std::collections::BTreeMap<i32, Vec<RoundScoreDetail>> =
            std::collections::BTreeMap::new();
        for row in &score_rows {
            rounds_map
                .entry(row.round_number)
                .or_default()
                .push(RoundScoreDetail {
                    player_index: row.player_index,
                    raw_score: row.raw_score,
                    adjusted_score: row.adjusted_score,
                    cumulative_score: row.cumulative_score,
                    went_out: row.went_out,
                    was_penalized: row.was_penalized,
                });
        }

        let rounds = rounds_map
            .into_iter()
            .map(|(round_number, scores)| RoundDetail {
                round_number,
                scores,
            })
            .collect();

        Ok(GameDetail {
            id: summary.game_id,
            room_code: summary.room_code,
            rules: summary.rules_name,
            num_players: summary.num_players,
            num_rounds: summary.num_rounds,
            created_at: summary.created_at,
            players: player_rows
                .into_iter()
                .map(|p| GamePlayerDetail {
                    name: p.name,
                    final_score: p.final_score,
                    is_winner: p.is_winner,
                    is_bot: p.is_bot,
                    user_id: p.user_id,
                })
                .collect(),
            rounds,
        })
    }

    /// Reconstruct a `GameHistory` from relational tables.
    /// This is the inverse of `save_complete_game`.
    pub async fn reconstruct_game_history(
        &self,
        game_id: Uuid,
    ) -> Result<GameHistory, PersistenceError> {
        // Game metadata
        let game_row: Option<(String, i64)> =
            sqlx::query_as("SELECT rules_name, COALESCE(seed, 0) FROM games WHERE id = $1")
                .bind(game_id)
                .fetch_optional(&self.pool)
                .await?;

        let (rules_name, seed) =
            game_row.ok_or_else(|| PersistenceError::NotFound(format!("game {game_id}")))?;

        // Players
        let player_rows: Vec<(i32, String, Option<String>)> = sqlx::query_as(
            "SELECT player_index, player_name, strategy_name
             FROM game_players WHERE game_id = $1 ORDER BY player_index",
        )
        .bind(game_id)
        .fetch_all(&self.pool)
        .await?;

        let num_players = player_rows.len();
        let strategy_names: Vec<String> = player_rows
            .iter()
            .map(|(_, name, strategy)| strategy.as_deref().unwrap_or(name.as_str()).to_string())
            .collect();

        // Rounds
        let round_rows: Vec<(i32, i32, i32, Option<i32>, bool)> = sqlx::query_as(
            "SELECT id, round_number, starting_player, going_out_player, truncated
             FROM game_rounds WHERE game_id = $1 ORDER BY round_number",
        )
        .bind(game_id)
        .fetch_all(&self.pool)
        .await?;

        let mut rounds = Vec::with_capacity(round_rows.len());
        let mut cumulative_scores = vec![0i32; num_players];

        for (round_id, round_number, starting_player, going_out_player, truncated) in &round_rows {
            // Initial deck order
            let deck_rows: Vec<(i32, i16)> = sqlx::query_as(
                "SELECT position, card_value FROM round_initial_deck
                 WHERE round_id = $1 ORDER BY position",
            )
            .bind(round_id)
            .fetch_all(&self.pool)
            .await?;
            let initial_deck_order: Vec<i8> = deck_rows.iter().map(|(_, v)| *v as i8).collect();

            // Dealt cards
            let dealt_rows: Vec<(i32, i32, i16)> = sqlx::query_as(
                "SELECT player_index, slot_index, card_value FROM round_dealt_cards
                 WHERE round_id = $1 ORDER BY player_index, slot_index",
            )
            .bind(round_id)
            .fetch_all(&self.pool)
            .await?;
            let mut dealt_hands: Vec<Vec<i8>> = vec![Vec::new(); num_players];
            for (pi, _si, cv) in &dealt_rows {
                dealt_hands[*pi as usize].push(*cv as i8);
            }

            // Setup flips
            let flip_rows: Vec<(i32, i32, i32)> = sqlx::query_as(
                "SELECT player_index, flip_index, slot_index FROM round_setup_flips
                 WHERE round_id = $1 ORDER BY player_index, flip_index",
            )
            .bind(round_id)
            .fetch_all(&self.pool)
            .await?;
            let mut setup_flips: Vec<Vec<usize>> = vec![Vec::new(); num_players];
            for (pi, _fi, si) in &flip_rows {
                setup_flips[*pi as usize].push(*si as usize);
            }

            // Turns
            let turn_rows: Vec<TurnDbRow> = sqlx::query_as(
                "SELECT turn_index, player_index, action_kind_id, drawn_card,
                        target_position, replaced_card, flipped_card
                 FROM round_turns WHERE round_id = $1 ORDER BY turn_index",
            )
            .bind(round_id)
            .fetch_all(&self.pool)
            .await?;

            // Column clears (turn-based)
            let clear_rows: Vec<ColumnClearDbRow> = sqlx::query_as(
                "SELECT clear_kind_id, turn_index, column_index, card_value, player_index
                 FROM column_clears WHERE round_id = $1 ORDER BY id",
            )
            .bind(round_id)
            .fetch_all(&self.pool)
            .await?;

            // Reconstruct turns with their column clears
            let mut boards = build_initial_boards(&dealt_hands);
            let mut turns = Vec::with_capacity(turn_rows.len());
            for turn_row in &turn_rows {
                let action = reconstruct_turn_action(turn_row, &boards);

                // Collect clears for this turn (clear_kind_id = 1)
                let turn_clears: Vec<ColumnClearEvent> = clear_rows
                    .iter()
                    .filter(|c| c.clear_kind_id == 1 && c.turn_index == Some(turn_row.turn_index))
                    .map(|c| ColumnClearEvent {
                        player_index: c.player_index as usize,
                        column: c.column_index as usize,
                        card_value: c.card_value as i8,
                        displaced_card: match &action {
                            TurnAction::DrewFromDeck { displaced_card, .. } => *displaced_card,
                            TurnAction::DrewFromDiscard { displaced_card, .. } => {
                                Some(*displaced_card)
                            }
                        },
                    })
                    .collect();

                // Update boards based on action
                apply_action_to_boards(&action, turn_row.player_index as usize, &mut boards);
                let num_rows = num_rows_for_rules(&rules_name);
                for clear in &turn_clears {
                    mark_column_cleared(&mut boards, clear.player_index, clear.column, num_rows);
                }

                // The going_out player's first turn matching their index has went_out = true.
                let is_going_out_turn = going_out_player
                    .map(|g| g == turn_row.player_index)
                    .unwrap_or(false)
                    && !turns.iter().any(|t: &TurnRecord| t.went_out);

                turns.push(TurnRecord {
                    player_index: turn_row.player_index as usize,
                    action,
                    column_clears: turn_clears,
                    went_out: is_going_out_turn,
                });
            }

            // End-of-round clears (clear_kind_id = 2)
            let end_of_round_clears: Vec<ColumnClearEvent> = clear_rows
                .iter()
                .filter(|c| c.clear_kind_id == 2)
                .map(|c| ColumnClearEvent {
                    player_index: c.player_index as usize,
                    column: c.column_index as usize,
                    card_value: c.card_value as i8,
                    displaced_card: None,
                })
                .collect();

            // Scores
            let score_rows: Vec<(i32, i32)> = sqlx::query_as(
                "SELECT player_index, adjusted_score FROM round_scores
                 WHERE round_id = $1 ORDER BY player_index",
            )
            .bind(round_id)
            .fetch_all(&self.pool)
            .await?;
            let round_scores: Vec<i32> = score_rows.iter().map(|(_, s)| *s).collect();

            for (i, &s) in round_scores.iter().enumerate() {
                if i < cumulative_scores.len() {
                    cumulative_scores[i] += s;
                }
            }

            rounds.push(RoundHistory {
                round_number: *round_number as usize,
                initial_deck_order,
                dealt_hands,
                setup_flips,
                starting_player: *starting_player as usize,
                turns,
                going_out_player: going_out_player.map(|p| p as usize),
                end_of_round_clears,
                round_scores,
                cumulative_scores: cumulative_scores.clone(),
                truncated: *truncated,
            });
        }

        // Final scores and winners
        let final_score_rows: Vec<(i32, i64)> = sqlx::query_as(
            "SELECT player_index, final_score FROM game_final_scores
             WHERE game_id = $1 ORDER BY player_index",
        )
        .bind(game_id)
        .fetch_all(&self.pool)
        .await?;
        let final_scores: Vec<i32> = final_score_rows.iter().map(|(_, s)| *s as i32).collect();

        let winner_rows: Vec<(i32,)> = sqlx::query_as(
            "SELECT player_index FROM game_winners WHERE game_id = $1 ORDER BY player_index",
        )
        .bind(game_id)
        .fetch_all(&self.pool)
        .await?;
        let winners: Vec<usize> = winner_rows.iter().map(|(i,)| *i as usize).collect();

        Ok(GameHistory {
            seed: seed as u64,
            num_players,
            strategy_names,
            rules_name,
            rounds,
            final_scores,
            winners,
        })
    }

    /// Find games stuck in 'in_progress' state (for cleanup on server restart).
    pub async fn find_orphaned_in_progress_games(&self) -> Result<Vec<Uuid>, PersistenceError> {
        let rows: Vec<(Uuid,)> = sqlx::query_as(
            "SELECT g.id FROM games g
             JOIN game_states gs ON gs.id = g.game_state_id
             WHERE gs.name = 'in_progress'",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(|(id,)| id).collect())
    }

    /// Get a user's final score for a specific game.
    pub async fn get_user_score_for_game(
        &self,
        game_id: Uuid,
        user_id: Uuid,
    ) -> Result<Option<i32>, PersistenceError> {
        let score: Option<i64> = sqlx::query_scalar(
            "SELECT final_score FROM game_final_scores
             WHERE game_id = $1 AND user_id = $2",
        )
        .bind(game_id)
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(score.map(|s| s as i32))
    }
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PlayerStatsRow {
    pub player_name: String,
    pub rules: String,
    pub games_played: i32,
    pub games_won: i32,
    pub total_score: i32,
    pub best_score: Option<i32>,
    pub worst_score: Option<i32>,
}

// ── Leaderboard response types ──────────────────────────────────────

/// Input row for `save_round_turns`.
#[derive(Debug, Clone)]
pub struct RoundTurnRow {
    pub turn_index: i32,
    pub player_index: i32,
    pub action_kind_id: i32,
    pub drawn_card: Option<i16>,
    pub target_position: Option<i32>,
    pub replaced_card: Option<i16>,
    pub flipped_card: Option<i16>,
}

/// Input row for `save_column_clears`.
#[derive(Debug, Clone)]
pub struct ColumnClearRow {
    pub clear_kind_id: i32,
    pub turn_index: Option<i32>,
    pub column_index: i32,
    pub card_value: i16,
    pub player_index: i32,
    pub cards_cleared: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameListParams {
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameListResponse {
    pub games: Vec<GameSummary>,
    pub total: i64,
    pub page: i32,
    pub per_page: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameSummary {
    pub id: Uuid,
    pub room_code: Option<String>,
    pub rules: String,
    pub num_players: i32,
    pub num_rounds: i32,
    pub created_at: DateTime<Utc>,
    pub players: Vec<GamePlayerSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GamePlayerSummary {
    pub name: String,
    pub final_score: i64,
    pub is_winner: bool,
    pub is_bot: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameDetail {
    pub id: Uuid,
    pub room_code: Option<String>,
    pub rules: String,
    pub num_players: i32,
    pub num_rounds: i32,
    pub created_at: DateTime<Utc>,
    pub players: Vec<GamePlayerDetail>,
    pub rounds: Vec<RoundDetail>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GamePlayerDetail {
    pub name: String,
    pub final_score: i64,
    pub is_winner: bool,
    pub is_bot: bool,
    pub user_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoundDetail {
    pub round_number: i32,
    pub scores: Vec<RoundScoreDetail>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoundScoreDetail {
    pub player_index: i32,
    pub raw_score: i32,
    pub adjusted_score: i32,
    pub cumulative_score: i64,
    pub went_out: bool,
    pub was_penalized: bool,
}

// ── Internal row types for sqlx ─────────────────────────────────────

#[derive(Debug, sqlx::FromRow)]
struct GameSummaryRow {
    game_id: Uuid,
    rules_name: String,
    num_players: i32,
    num_rounds: i32,
    created_at: DateTime<Utc>,
}

#[derive(Debug, sqlx::FromRow)]
struct GamePlayerSummaryRow {
    name: String,
    final_score: i64,
    is_winner: bool,
    is_bot: bool,
}

#[derive(Debug, sqlx::FromRow)]
struct GameDetailRow {
    game_id: Uuid,
    room_code: Option<String>,
    rules_name: String,
    num_players: i32,
    num_rounds: i32,
    created_at: DateTime<Utc>,
}

#[derive(Debug, sqlx::FromRow)]
struct GamePlayerDetailRow {
    name: String,
    final_score: i64,
    is_winner: bool,
    is_bot: bool,
    user_id: Option<Uuid>,
}

#[derive(Debug, sqlx::FromRow)]
struct RoundScoreRow {
    round_number: i32,
    player_index: i32,
    raw_score: i32,
    adjusted_score: i32,
    cumulative_score: i64,
    went_out: bool,
    was_penalized: bool,
}

#[derive(Debug, sqlx::FromRow)]
struct TurnDbRow {
    turn_index: i32,
    player_index: i32,
    action_kind_id: i32,
    drawn_card: Option<i16>,
    target_position: Option<i32>,
    replaced_card: Option<i16>,
    flipped_card: Option<i16>,
}

#[derive(Debug, sqlx::FromRow)]
struct ColumnClearDbRow {
    clear_kind_id: i32,
    turn_index: Option<i32>,
    column_index: i32,
    card_value: i16,
    player_index: i32,
}

// ── Helper functions ────────────────────────────────────────────────

/// Map rules name to number of rows per player grid.
fn num_rows_for_rules(rules_name: &str) -> usize {
    match rules_name {
        "Standard" => 3,
        // Default to 3 for unknown rules
        _ => 3,
    }
}

/// Build initial per-player board state from dealt hands.
/// `boards[player][slot]` = card value (i8). Uses `i16::MIN` as sentinel for cleared.
fn build_initial_boards(dealt_hands: &[Vec<i8>]) -> Vec<Vec<i16>> {
    dealt_hands
        .iter()
        .map(|hand| hand.iter().map(|&c| c as i16).collect())
        .collect()
}

/// Mark a column as cleared in the board state.
fn mark_column_cleared(boards: &mut [Vec<i16>], player: usize, column: usize, num_rows: usize) {
    for row in 0..num_rows {
        let idx = column * num_rows + row;
        if idx < boards[player].len() {
            boards[player][idx] = i16::MIN; // sentinel for cleared
        }
    }
}

/// Apply a turn action to the board state tracker.
fn apply_action_to_boards(action: &TurnAction, player: usize, boards: &mut [Vec<i16>]) {
    match action {
        TurnAction::DrewFromDeck {
            drawn_card, action, ..
        } => match action {
            DeckDrawAction::Keep(pos) => {
                boards[player][*pos] = *drawn_card as i16;
            }
            DeckDrawAction::DiscardAndFlip(_pos) => {
                // Card stays; just revealed (no value change in our tracker)
            }
        },
        TurnAction::DrewFromDiscard {
            drawn_card,
            placement,
            ..
        } => {
            boards[player][*placement] = *drawn_card as i16;
        }
    }
}

/// Decompose a TurnAction into the relational columns, updating board state.
/// Returns (action_kind_id, drawn_card, target_position, replaced_card, flipped_card).
fn decompose_turn_action(
    action: &TurnAction,
    player_index: usize,
    boards: &mut [Vec<i16>],
) -> (i32, Option<i16>, Option<i32>, Option<i16>, Option<i16>) {
    match action {
        TurnAction::DrewFromDeck {
            drawn_card,
            action: deck_action,
            displaced_card,
        } => match deck_action {
            DeckDrawAction::Keep(pos) => {
                let replaced = displaced_card.map(|c| c as i16);
                boards[player_index][*pos] = *drawn_card as i16;
                // action_kind_id = 1 = 'drew_deck_kept'
                (
                    1,
                    Some(*drawn_card as i16),
                    Some(*pos as i32),
                    replaced,
                    None,
                )
            }
            DeckDrawAction::DiscardAndFlip(pos) => {
                let flipped = boards[player_index][*pos];
                // action_kind_id = 2 = 'drew_deck_flipped'
                (
                    2,
                    Some(*drawn_card as i16),
                    Some(*pos as i32),
                    None,
                    Some(flipped),
                )
            }
        },
        TurnAction::DrewFromDiscard {
            drawn_card,
            placement,
            displaced_card,
            ..
        } => {
            boards[player_index][*placement] = *drawn_card as i16;
            // action_kind_id = 3 = 'drew_discard'
            (
                3,
                Some(*drawn_card as i16),
                Some(*placement as i32),
                Some(*displaced_card as i16),
                None,
            )
        }
    }
}

/// Reconstruct a TurnAction from database row and board state.
fn reconstruct_turn_action(row: &TurnDbRow, boards: &[Vec<i16>]) -> TurnAction {
    let player = row.player_index as usize;
    match row.action_kind_id {
        // 1 = drew_deck_kept
        1 => TurnAction::DrewFromDeck {
            drawn_card: row.drawn_card.unwrap_or(0) as i8,
            action: DeckDrawAction::Keep(row.target_position.unwrap_or(0) as usize),
            displaced_card: row.replaced_card.map(|c| c as i8),
        },
        // 2 = drew_deck_flipped
        2 => {
            let pos = row.target_position.unwrap_or(0) as usize;
            let _flipped_value = row
                .flipped_card
                .unwrap_or_else(|| boards[player].get(pos).copied().unwrap_or(0));
            TurnAction::DrewFromDeck {
                drawn_card: row.drawn_card.unwrap_or(0) as i8,
                action: DeckDrawAction::DiscardAndFlip(pos),
                displaced_card: None,
            }
        }
        // 3 = drew_discard
        _ => TurnAction::DrewFromDiscard {
            pile_index: 0,
            drawn_card: row.drawn_card.unwrap_or(0) as i8,
            placement: row.target_position.unwrap_or(0) as usize,
            displaced_card: row.replaced_card.unwrap_or(0) as i8,
        },
    }
}

/// Compute raw and adjusted scores for a player.
/// `round_scores` from GameHistory already has penalties baked in.
/// For the going-out player with a positive score that was penalized,
/// raw = adjusted / 2. For everyone else, raw = adjusted.
fn compute_raw_adjusted(
    adjusted: i32,
    player_idx: usize,
    going_out_player: Option<usize>,
    all_scores: &[i32],
) -> (i32, i32) {
    let is_goer = going_out_player == Some(player_idx);
    if !is_goer || adjusted <= 0 {
        return (adjusted, adjusted);
    }

    // Check if the goer's score might have been doubled.
    // If adjusted/2 would NOT be solo lowest among other players, then penalty was applied.
    let half = adjusted / 2;
    let is_half_solo_lowest = all_scores
        .iter()
        .enumerate()
        .all(|(i, &s)| i == player_idx || s > half);

    if is_half_solo_lowest {
        // goer was solo lowest at half → no penalty was applied → raw = adjusted
        (adjusted, adjusted)
    } else {
        // goer was NOT solo lowest → penalty was applied → raw = adjusted/2
        (half, adjusted)
    }
}

/// Helper to bind filter params to a scalar count query.
fn bind_game_list_filters<'q>(
    mut query: sqlx::query::QueryScalar<'q, sqlx::Postgres, i64, sqlx::postgres::PgArguments>,
    params: &'q GameListParams,
) -> sqlx::query::QueryScalar<'q, sqlx::Postgres, i64, sqlx::postgres::PgArguments> {
    if let Some(ref name) = params.player_name {
        query = query.bind(name.as_str());
    }
    if let Some(ref rules) = params.rules {
        query = query.bind(rules.as_str());
    }
    if let Some(min_p) = params.min_players {
        query = query.bind(min_p as i64);
    }
    if let Some(max_p) = params.max_players {
        query = query.bind(max_p as i64);
    }
    query
}

/// Helper to bind filter params to a query_as for game summaries.
fn bind_game_list_filters_query_as<'q>(
    mut query: sqlx::query::QueryAs<
        'q,
        sqlx::Postgres,
        GameSummaryRow,
        sqlx::postgres::PgArguments,
    >,
    params: &'q GameListParams,
) -> sqlx::query::QueryAs<'q, sqlx::Postgres, GameSummaryRow, sqlx::postgres::PgArguments> {
    if let Some(ref name) = params.player_name {
        query = query.bind(name.as_str());
    }
    if let Some(ref rules) = params.rules {
        query = query.bind(rules.as_str());
    }
    if let Some(min_p) = params.min_players {
        query = query.bind(min_p as i64);
    }
    if let Some(max_p) = params.max_players {
        query = query.bind(max_p as i64);
    }
    query
}
