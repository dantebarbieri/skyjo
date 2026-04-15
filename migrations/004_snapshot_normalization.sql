-- Migration 004: Complete room_snapshots normalization
-- Adds missing columns needed for full RoomSnapshot round-trip persistence.

-- room_snapshots: add columns for creator, timers, and game state JSON
ALTER TABLE room_snapshots ADD COLUMN IF NOT EXISTS creator INT NOT NULL DEFAULT 0;
ALTER TABLE room_snapshots ADD COLUMN IF NOT EXISTS turn_timer_secs BIGINT;
ALTER TABLE room_snapshots ADD COLUMN IF NOT EXISTS disconnect_bot_timeout_secs INT;
ALTER TABLE room_snapshots ADD COLUMN IF NOT EXISTS game_state_json TEXT;

-- room_snapshot_players: add was_human flag
ALTER TABLE room_snapshot_players ADD COLUMN IF NOT EXISTS was_human BOOLEAN NOT NULL DEFAULT FALSE;
