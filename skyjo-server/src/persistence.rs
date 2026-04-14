use rusqlite::{Connection, params};
use std::path::Path;
use std::sync::Mutex;

#[derive(Debug)]
pub enum PersistenceError {
    Sqlite(rusqlite::Error),
    Io(std::io::Error),
    Json(serde_json::Error),
    LockPoisoned,
}

impl std::fmt::Display for PersistenceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Sqlite(e) => write!(f, "SQLite error: {e}"),
            Self::Io(e) => write!(f, "IO error: {e}"),
            Self::Json(e) => write!(f, "JSON error: {e}"),
            Self::LockPoisoned => write!(f, "database connection mutex poisoned"),
        }
    }
}

impl std::error::Error for PersistenceError {}

impl From<rusqlite::Error> for PersistenceError {
    fn from(e: rusqlite::Error) -> Self {
        Self::Sqlite(e)
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

/// Persistent storage layer backed by SQLite.
pub struct Persistence {
    conn: Mutex<Connection>,
}

impl Persistence {
    /// Open or create the SQLite database at the given path.
    pub fn open(path: &Path) -> Result<Self, PersistenceError> {
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;")?;
        let persistence = Self {
            conn: Mutex::new(conn),
        };
        persistence.migrate()?;
        Ok(persistence)
    }

    /// Run schema migrations.
    fn migrate(&self) -> Result<(), PersistenceError> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| PersistenceError::LockPoisoned)?;
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS game_replays (
                id TEXT PRIMARY KEY,
                room_code TEXT NOT NULL,
                players TEXT NOT NULL,
                rules TEXT NOT NULL,
                seed INTEGER NOT NULL,
                created_at TEXT NOT NULL,
                history BLOB NOT NULL,
                winner_indices TEXT
            );

            CREATE TABLE IF NOT EXISTS player_stats (
                player_name TEXT NOT NULL,
                rules TEXT NOT NULL,
                games_played INTEGER DEFAULT 0,
                games_won INTEGER DEFAULT 0,
                total_score INTEGER DEFAULT 0,
                best_score INTEGER,
                worst_score INTEGER,
                PRIMARY KEY (player_name, rules)
            );

            CREATE TABLE IF NOT EXISTS room_snapshots (
                room_code TEXT PRIMARY KEY,
                snapshot BLOB NOT NULL,
                updated_at TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_replays_room ON game_replays(room_code);
            CREATE INDEX IF NOT EXISTS idx_replays_created ON game_replays(created_at);
            CREATE INDEX IF NOT EXISTS idx_stats_name ON player_stats(player_name);
        ",
        )?;
        Ok(())
    }

    /// Store a game replay (compressed with zstd).
    #[allow(clippy::too_many_arguments)]
    pub fn save_replay(
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
        let players_json = serde_json::to_string(players)?;
        let winners_json = serde_json::to_string(winner_indices)?;
        let now = chrono::Utc::now().to_rfc3339();

        let conn = self
            .conn
            .lock()
            .map_err(|_| PersistenceError::LockPoisoned)?;
        conn.execute(
            "INSERT OR REPLACE INTO game_replays (id, room_code, players, rules, seed, created_at, history, winner_indices)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![id, room_code, players_json, rules, seed as i64, now, compressed, winners_json],
        )?;
        Ok(())
    }

    /// Load a game replay by ID, decompressing the history.
    pub fn load_replay(&self, id: &str) -> Result<Option<(String, Vec<u8>)>, PersistenceError> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| PersistenceError::LockPoisoned)?;
        let mut stmt = conn.prepare("SELECT players, history FROM game_replays WHERE id = ?1")?;
        let result = stmt.query_row(params![id], |row| {
            let players: String = row.get(0)?;
            let compressed: Vec<u8> = row.get(1)?;
            Ok((players, compressed))
        });
        match result {
            Ok((players, compressed)) => {
                let decompressed = zstd::decode_all(compressed.as_slice())?;
                Ok(Some((players, decompressed)))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Update player statistics after a game.
    pub fn update_player_stats(
        &self,
        player_name: &str,
        rules: &str,
        score: i32,
        won: bool,
    ) -> Result<(), PersistenceError> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| PersistenceError::LockPoisoned)?;
        conn.execute(
            "INSERT INTO player_stats (player_name, rules, games_played, games_won, total_score, best_score, worst_score)
             VALUES (?1, ?2, 1, ?3, ?4, ?4, ?4)
             ON CONFLICT(player_name, rules) DO UPDATE SET
                games_played = games_played + 1,
                games_won = games_won + ?3,
                total_score = total_score + ?4,
                best_score = MIN(best_score, ?4),
                worst_score = MAX(worst_score, ?4)",
            params![player_name, rules, won as i32, score],
        )?;
        Ok(())
    }

    /// Get player stats.
    pub fn get_player_stats(
        &self,
        player_name: &str,
    ) -> Result<Vec<PlayerStatsRow>, PersistenceError> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| PersistenceError::LockPoisoned)?;
        let mut stmt = conn.prepare(
            "SELECT player_name, rules, games_played, games_won, total_score, best_score, worst_score
             FROM player_stats WHERE player_name = ?1",
        )?;
        let rows = stmt.query_map(params![player_name], |row| {
            Ok(PlayerStatsRow {
                player_name: row.get(0)?,
                rules: row.get(1)?,
                games_played: row.get(2)?,
                games_won: row.get(3)?,
                total_score: row.get(4)?,
                best_score: row.get(5)?,
                worst_score: row.get(6)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    /// Save a room snapshot (compressed).
    pub fn save_room_snapshot(
        &self,
        room_code: &str,
        snapshot_json: &str,
    ) -> Result<(), PersistenceError> {
        let compressed = zstd::encode_all(snapshot_json.as_bytes(), 3)?;
        let now = chrono::Utc::now().to_rfc3339();
        let conn = self
            .conn
            .lock()
            .map_err(|_| PersistenceError::LockPoisoned)?;
        conn.execute(
            "INSERT OR REPLACE INTO room_snapshots (room_code, snapshot, updated_at)
             VALUES (?1, ?2, ?3)",
            params![room_code, compressed, now],
        )?;
        Ok(())
    }

    /// Load a room snapshot, decompressing.
    pub fn load_room_snapshot(&self, room_code: &str) -> Result<Option<Vec<u8>>, PersistenceError> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| PersistenceError::LockPoisoned)?;
        let mut stmt = conn.prepare("SELECT snapshot FROM room_snapshots WHERE room_code = ?1")?;
        let result = stmt.query_row(params![room_code], |row| {
            let compressed: Vec<u8> = row.get(0)?;
            Ok(compressed)
        });
        match result {
            Ok(compressed) => {
                let decompressed = zstd::decode_all(compressed.as_slice())?;
                Ok(Some(decompressed))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Load all room snapshots (for crash recovery on startup).
    pub fn load_all_room_snapshots(&self) -> Result<Vec<(String, Vec<u8>)>, PersistenceError> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| PersistenceError::LockPoisoned)?;
        let mut stmt = conn.prepare("SELECT room_code, snapshot FROM room_snapshots")?;
        let rows = stmt.query_map([], |row| {
            let code: String = row.get(0)?;
            let compressed: Vec<u8> = row.get(1)?;
            Ok((code, compressed))
        })?;
        let mut results = Vec::new();
        for row in rows {
            let (code, compressed) = row?;
            let decompressed = zstd::decode_all(compressed.as_slice())?;
            results.push((code, decompressed));
        }
        Ok(results)
    }

    /// Delete a room snapshot (when room is cleaned up).
    pub fn delete_room_snapshot(&self, room_code: &str) -> Result<(), PersistenceError> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| PersistenceError::LockPoisoned)?;
        conn.execute(
            "DELETE FROM room_snapshots WHERE room_code = ?1",
            params![room_code],
        )?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct PlayerStatsRow {
    pub player_name: String,
    pub rules: String,
    pub games_played: i32,
    pub games_won: i32,
    pub total_score: i32,
    pub best_score: Option<i32>,
    pub worst_score: Option<i32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_db() -> Persistence {
        Persistence::open(Path::new(":memory:")).unwrap()
    }

    #[test]
    fn create_tables_idempotent() {
        let db = test_db();
        // Running migrate again should be fine (IF NOT EXISTS)
        db.migrate().unwrap();
        db.migrate().unwrap();
    }

    #[test]
    fn insert_and_query_replay() {
        let db = test_db();
        let history = r#"{"rounds":[]}"#;
        db.save_replay(
            "r1",
            "ABCDEF",
            &["Alice".into(), "Bob".into()],
            "Standard",
            42,
            history,
            &[0],
        )
        .unwrap();

        let (players, data) = db.load_replay("r1").unwrap().unwrap();
        assert!(players.contains("Alice"));
        assert_eq!(String::from_utf8(data).unwrap(), history);
    }

    #[test]
    fn load_missing_replay_returns_none() {
        let db = test_db();
        assert!(db.load_replay("nonexistent").unwrap().is_none());
    }

    #[test]
    fn update_player_stats() {
        let db = test_db();
        db.update_player_stats("Alice", "Standard", 45, true)
            .unwrap();
        db.update_player_stats("Alice", "Standard", 60, false)
            .unwrap();

        let stats = db.get_player_stats("Alice").unwrap();
        assert_eq!(stats.len(), 1);
        assert_eq!(stats[0].games_played, 2);
        assert_eq!(stats[0].games_won, 1);
        assert_eq!(stats[0].total_score, 105);
        assert_eq!(stats[0].best_score, Some(45));
        assert_eq!(stats[0].worst_score, Some(60));
    }

    #[test]
    fn snapshot_and_restore_room() {
        let db = test_db();
        let snapshot = r#"{"phase":"InGame","players":[]}"#;
        db.save_room_snapshot("ABC123", snapshot).unwrap();

        let loaded = db.load_room_snapshot("ABC123").unwrap().unwrap();
        assert_eq!(String::from_utf8(loaded).unwrap(), snapshot);
    }

    #[test]
    fn load_all_snapshots() {
        let db = test_db();
        db.save_room_snapshot("ROOM1", "snap1").unwrap();
        db.save_room_snapshot("ROOM2", "snap2").unwrap();

        let all = db.load_all_room_snapshots().unwrap();
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn delete_snapshot() {
        let db = test_db();
        db.save_room_snapshot("ROOM1", "snap1").unwrap();
        db.delete_room_snapshot("ROOM1").unwrap();
        assert!(db.load_room_snapshot("ROOM1").unwrap().is_none());
    }

    #[test]
    fn compressed_replay_round_trip() {
        let db = test_db();
        // Large-ish history to test compression
        let history = "{\"data\":\"".to_string() + &"x".repeat(10000) + "\"}";
        db.save_replay("big", "ROOM", &["P1".into()], "Standard", 1, &history, &[])
            .unwrap();
        let (_, data) = db.load_replay("big").unwrap().unwrap();
        assert_eq!(String::from_utf8(data).unwrap(), history);
    }
}
