use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;

#[derive(Debug)]
pub enum PersistenceError {
    Sqlx(sqlx::Error),
    Io(std::io::Error),
    Json(serde_json::Error),
}

impl std::fmt::Display for PersistenceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Sqlx(e) => write!(f, "database error: {e}"),
            Self::Io(e) => write!(f, "IO error: {e}"),
            Self::Json(e) => write!(f, "JSON error: {e}"),
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
        ];

        for (name, sql) in migrations {
            let applied: bool =
                sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM _migrations WHERE name = $1)")
                    .bind(name)
                    .fetch_one(&self.pool)
                    .await?;

            if !applied {
                sqlx::query(sql).execute(&self.pool).await?;
                sqlx::query("INSERT INTO _migrations (name) VALUES ($1)")
                    .bind(name)
                    .execute(&self.pool)
                    .await?;
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
