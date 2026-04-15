-- Migration 003: Leaderboard — fully relational game history and normalized schema
-- Replaces: game_replays (BYTEA blob), player_stats (no FK), permission_level ENUM
-- Adds: lookup tables, structured game/round/turn tables, normalized room snapshots, views

--------------------------------------------------------------------------------
-- 1. Lookup tables (seed data inserted immediately)
--------------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS permission_levels (
    id SERIAL PRIMARY KEY,
    name TEXT UNIQUE NOT NULL
);

INSERT INTO permission_levels (name) VALUES ('user'), ('moderator'), ('admin')
ON CONFLICT (name) DO NOTHING;

CREATE TABLE IF NOT EXISTS action_kinds (
    id SERIAL PRIMARY KEY,
    name TEXT UNIQUE NOT NULL
);

INSERT INTO action_kinds (name) VALUES ('drew_deck_kept'), ('drew_deck_flipped'), ('drew_discard')
ON CONFLICT (name) DO NOTHING;

CREATE TABLE IF NOT EXISTS clear_kinds (
    id SERIAL PRIMARY KEY,
    name TEXT UNIQUE NOT NULL
);

INSERT INTO clear_kinds (name) VALUES ('turn'), ('round_end')
ON CONFLICT (name) DO NOTHING;

CREATE TABLE IF NOT EXISTS game_states (
    id SERIAL PRIMARY KEY,
    name TEXT UNIQUE NOT NULL
);

INSERT INTO game_states (name) VALUES ('in_progress'), ('completed'), ('abandoned')
ON CONFLICT (name) DO NOTHING;

CREATE TABLE IF NOT EXISTS room_phases (
    id SERIAL PRIMARY KEY,
    name TEXT UNIQUE NOT NULL
);

INSERT INTO room_phases (name) VALUES ('lobby'), ('in_game'), ('game_over')
ON CONFLICT (name) DO NOTHING;

CREATE TABLE IF NOT EXISTS slot_types (
    id SERIAL PRIMARY KEY,
    name TEXT UNIQUE NOT NULL
);

INSERT INTO slot_types (name) VALUES ('human'), ('bot'), ('empty')
ON CONFLICT (name) DO NOTHING;

--------------------------------------------------------------------------------
-- 2. Core game tables
--------------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS games (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    room_code TEXT,
    rules_name TEXT NOT NULL,
    seed BIGINT,
    game_state_id INT NOT NULL REFERENCES game_states(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_games_created_at ON games(created_at);
CREATE INDEX IF NOT EXISTS idx_games_room_code ON games(room_code);
CREATE INDEX IF NOT EXISTS idx_games_game_state_id ON games(game_state_id);

CREATE TABLE IF NOT EXISTS game_players (
    id SERIAL PRIMARY KEY,
    game_id UUID NOT NULL REFERENCES games(id) ON DELETE CASCADE,
    player_index INT NOT NULL,
    player_name TEXT NOT NULL,
    user_id UUID REFERENCES users(id),
    strategy_name TEXT,
    UNIQUE (game_id, player_index)
);

CREATE INDEX IF NOT EXISTS idx_game_players_user_id ON game_players(user_id);
CREATE INDEX IF NOT EXISTS idx_game_players_game_id ON game_players(game_id);

CREATE TABLE IF NOT EXISTS game_rounds (
    id SERIAL PRIMARY KEY,
    game_id UUID NOT NULL REFERENCES games(id) ON DELETE CASCADE,
    round_number INT NOT NULL,
    starting_player INT NOT NULL,
    going_out_player INT,
    truncated BOOLEAN NOT NULL DEFAULT FALSE,
    UNIQUE (game_id, round_number)
);

CREATE INDEX IF NOT EXISTS idx_game_rounds_game_id ON game_rounds(game_id);

CREATE TABLE IF NOT EXISTS round_scores (
    id SERIAL PRIMARY KEY,
    round_id INT NOT NULL REFERENCES game_rounds(id) ON DELETE CASCADE,
    player_index INT NOT NULL,
    raw_score INT NOT NULL,
    adjusted_score INT NOT NULL,
    UNIQUE (round_id, player_index)
);

CREATE INDEX IF NOT EXISTS idx_round_scores_round_id ON round_scores(round_id);

CREATE TABLE IF NOT EXISTS round_turns (
    id SERIAL PRIMARY KEY,
    round_id INT NOT NULL REFERENCES game_rounds(id) ON DELETE CASCADE,
    turn_index INT NOT NULL,
    player_index INT NOT NULL,
    action_kind_id INT NOT NULL REFERENCES action_kinds(id),
    drawn_card SMALLINT,
    target_position INT,
    replaced_card SMALLINT,
    flipped_card SMALLINT,
    UNIQUE (round_id, turn_index)
);

CREATE INDEX IF NOT EXISTS idx_round_turns_round_id ON round_turns(round_id);

-- clear_kind_id values are fixed seed data: 1 = 'turn', 2 = 'round_end'.
-- The CHECK constraint uses hardcoded IDs because these lookup values are
-- inserted above and will never change.
CREATE TABLE IF NOT EXISTS column_clears (
    id SERIAL PRIMARY KEY,
    round_id INT NOT NULL REFERENCES game_rounds(id) ON DELETE CASCADE,
    clear_kind_id INT NOT NULL REFERENCES clear_kinds(id),
    turn_index INT,
    column_index INT NOT NULL,
    card_value SMALLINT NOT NULL,
    player_index INT NOT NULL,
    cards_cleared INT NOT NULL,
    CHECK (
        (clear_kind_id = 1 AND turn_index IS NOT NULL)
        OR (clear_kind_id = 2 AND turn_index IS NULL)
    )
);

CREATE TABLE IF NOT EXISTS round_initial_deck (
    id SERIAL PRIMARY KEY,
    round_id INT NOT NULL REFERENCES game_rounds(id) ON DELETE CASCADE,
    position INT NOT NULL,
    card_value SMALLINT NOT NULL,
    UNIQUE (round_id, position)
);

CREATE TABLE IF NOT EXISTS round_dealt_cards (
    id SERIAL PRIMARY KEY,
    round_id INT NOT NULL REFERENCES game_rounds(id) ON DELETE CASCADE,
    player_index INT NOT NULL,
    slot_index INT NOT NULL,
    card_value SMALLINT NOT NULL,
    UNIQUE (round_id, player_index, slot_index)
);

CREATE TABLE IF NOT EXISTS round_setup_flips (
    id SERIAL PRIMARY KEY,
    round_id INT NOT NULL REFERENCES game_rounds(id) ON DELETE CASCADE,
    player_index INT NOT NULL,
    flip_index INT NOT NULL,
    slot_index INT NOT NULL,
    UNIQUE (round_id, player_index, flip_index)
);

--------------------------------------------------------------------------------
-- 3. Migrate users.permission_level from ENUM to FK lookup
--------------------------------------------------------------------------------

-- Add the new FK column
ALTER TABLE users ADD COLUMN permission_level_id INT REFERENCES permission_levels(id);

-- Populate from the existing ENUM column
UPDATE users SET permission_level_id = pl.id
FROM permission_levels pl
WHERE pl.name = users.permission_level::TEXT;

-- Set NOT NULL + default after population
-- permission_levels 'user' = id 1 (first inserted above)
ALTER TABLE users ALTER COLUMN permission_level_id SET NOT NULL;
ALTER TABLE users ALTER COLUMN permission_level_id SET DEFAULT 1;

-- Drop the old ENUM column
ALTER TABLE users DROP COLUMN permission_level;

-- Drop the ENUM type
DROP TYPE IF EXISTS permission_level;

--------------------------------------------------------------------------------
-- 4. Normalize room_snapshots (ALTER existing table, add child tables)
--------------------------------------------------------------------------------

-- Add relational columns to existing room_snapshots table
ALTER TABLE room_snapshots ADD COLUMN phase_id INT REFERENCES room_phases(id);
ALTER TABLE room_snapshots ADD COLUMN num_players INT;
ALTER TABLE room_snapshots ADD COLUMN rules_name TEXT;
ALTER TABLE room_snapshots ADD COLUMN created_at TIMESTAMPTZ DEFAULT NOW();
ALTER TABLE room_snapshots ADD COLUMN active_game_id UUID REFERENCES games(id);

-- Drop the old BYTEA blob column
ALTER TABLE room_snapshots DROP COLUMN IF EXISTS snapshot;

-- Child tables for room snapshot details
-- room_snapshots PK is room_code TEXT, so child tables FK to room_code directly.
CREATE TABLE IF NOT EXISTS room_snapshot_players (
    id SERIAL PRIMARY KEY,
    room_code TEXT NOT NULL REFERENCES room_snapshots(room_code) ON DELETE CASCADE,
    slot_index INT NOT NULL,
    slot_type_id INT NOT NULL REFERENCES slot_types(id),
    player_name TEXT,
    strategy_name TEXT,
    session_token TEXT,
    is_connected BOOLEAN NOT NULL DEFAULT TRUE,
    UNIQUE (room_code, slot_index)
);

CREATE TABLE IF NOT EXISTS room_snapshot_banned_ips (
    id SERIAL PRIMARY KEY,
    room_code TEXT NOT NULL REFERENCES room_snapshots(room_code) ON DELETE CASCADE,
    ip_address TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS room_snapshot_last_winners (
    id SERIAL PRIMARY KEY,
    room_code TEXT NOT NULL REFERENCES room_snapshots(room_code) ON DELETE CASCADE,
    player_index INT NOT NULL
);

--------------------------------------------------------------------------------
-- 5. Views
--------------------------------------------------------------------------------

-- Round score details: went_out, was_penalized, and cumulative score per round
CREATE OR REPLACE VIEW round_score_details AS
SELECT
    rs.id AS round_score_id,
    gr.game_id,
    gr.round_number,
    rs.player_index,
    rs.raw_score,
    rs.adjusted_score,
    (gr.going_out_player = rs.player_index) AS went_out,
    (rs.adjusted_score <> rs.raw_score) AS was_penalized,
    SUM(rs.adjusted_score) OVER (
        PARTITION BY gr.game_id, rs.player_index
        ORDER BY gr.round_number
    ) AS cumulative_score
FROM round_scores rs
JOIN game_rounds gr ON gr.id = rs.round_id;

-- Game summary: player count and round count per game
CREATE OR REPLACE VIEW game_summary AS
SELECT
    g.id AS game_id,
    g.rules_name,
    g.seed,
    g.created_at,
    gs.name AS game_state,
    COUNT(DISTINCT gp.id) AS num_players,
    COALESCE(MAX(gr.round_number), 0) AS num_rounds
FROM games g
JOIN game_states gs ON gs.id = g.game_state_id
LEFT JOIN game_players gp ON gp.game_id = g.id
LEFT JOIN game_rounds gr ON gr.game_id = g.id
GROUP BY g.id, g.rules_name, g.seed, g.created_at, gs.name;

-- Game final scores: each player's cumulative score after the last round
CREATE OR REPLACE VIEW game_final_scores AS
SELECT DISTINCT ON (gr.game_id, rs.player_index)
    gr.game_id,
    rs.player_index,
    gp.player_name,
    gp.user_id,
    SUM(rs.adjusted_score) OVER (
        PARTITION BY gr.game_id, rs.player_index
        ORDER BY gr.round_number
    ) AS final_score
FROM round_scores rs
JOIN game_rounds gr ON gr.id = rs.round_id
JOIN game_players gp ON gp.game_id = gr.game_id AND gp.player_index = rs.player_index
ORDER BY gr.game_id, rs.player_index, gr.round_number DESC;

-- Game winners: players with the minimum final score in each game
CREATE OR REPLACE VIEW game_winners AS
SELECT
    gfs.game_id,
    gfs.player_index,
    gfs.player_name,
    gfs.user_id,
    gfs.final_score
FROM game_final_scores gfs
WHERE gfs.final_score = (
    SELECT MIN(gfs2.final_score)
    FROM game_final_scores gfs2
    WHERE gfs2.game_id = gfs.game_id
);

-- Player lifetime stats: aggregate stats per user across all completed games
CREATE OR REPLACE VIEW player_lifetime_stats AS
SELECT
    gfs.user_id,
    COUNT(*) AS games_played,
    COUNT(*) FILTER (WHERE gw.user_id IS NOT NULL) AS games_won,
    SUM(gfs.final_score) AS total_score,
    MIN(gfs.final_score) AS best_score,
    MAX(gfs.final_score) AS worst_score
FROM game_final_scores gfs
JOIN games g ON g.id = gfs.game_id
JOIN game_states gs ON gs.id = g.game_state_id AND gs.name = 'completed'
LEFT JOIN game_winners gw ON gw.game_id = gfs.game_id AND gw.user_id = gfs.user_id
WHERE gfs.user_id IS NOT NULL
GROUP BY gfs.user_id;

--------------------------------------------------------------------------------
-- 6. Drop old tables
--------------------------------------------------------------------------------

DROP TABLE IF EXISTS game_replays;
DROP TABLE IF EXISTS player_stats;
