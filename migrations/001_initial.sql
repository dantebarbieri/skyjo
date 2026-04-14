-- Migration 001: Initial tables (migrated from SQLite schema)

CREATE TABLE IF NOT EXISTS game_replays (
    id TEXT PRIMARY KEY,
    room_code TEXT NOT NULL,
    players JSONB NOT NULL,
    rules TEXT NOT NULL,
    seed BIGINT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    history BYTEA NOT NULL,
    winner_indices JSONB
);

CREATE INDEX IF NOT EXISTS idx_replays_room ON game_replays(room_code);
CREATE INDEX IF NOT EXISTS idx_replays_created ON game_replays(created_at);

CREATE TABLE IF NOT EXISTS player_stats (
    player_name TEXT NOT NULL,
    rules TEXT NOT NULL,
    games_played INTEGER NOT NULL DEFAULT 0,
    games_won INTEGER NOT NULL DEFAULT 0,
    total_score INTEGER NOT NULL DEFAULT 0,
    best_score INTEGER,
    worst_score INTEGER,
    PRIMARY KEY (player_name, rules)
);

CREATE INDEX IF NOT EXISTS idx_stats_name ON player_stats(player_name);

CREATE TABLE IF NOT EXISTS room_snapshots (
    room_code TEXT PRIMARY KEY,
    snapshot BYTEA NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
