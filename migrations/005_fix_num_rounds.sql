-- Migration 005: Fix off-by-one in game_summary num_rounds
-- round_number is 0-indexed in game_rounds, so MAX(round_number) undercounts by 1.
-- Use COUNT(DISTINCT round_number) instead.

CREATE OR REPLACE VIEW game_summary AS
SELECT
    g.id AS game_id,
    g.rules_name,
    g.seed,
    g.created_at,
    gs.name AS game_state,
    COUNT(DISTINCT gp.id) AS num_players,
    COUNT(DISTINCT gr.round_number) AS num_rounds
FROM games g
JOIN game_states gs ON gs.id = g.game_state_id
LEFT JOIN game_players gp ON gp.game_id = g.id
LEFT JOIN game_rounds gr ON gr.game_id = g.id
GROUP BY g.id, g.rules_name, g.seed, g.created_at, gs.name;
