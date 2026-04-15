---
applyTo: "skyjo-wasm/**/*.rs"
---

# Skyjo WASM Bridge — Review Guidelines

## Design Principle

`skyjo-wasm` is a **thin wrapper** over `skyjo-core`. It must not contain game logic — only serialization, deserialization, and delegation to core types.

## Serialization

- All WASM↔JS communication uses JSON strings via `serde_json`.
- Do **not** use `serde-wasm-bindgen` — keep the boundary as JSON strings for simplicity and debuggability.
- All exported functions return JSON strings except `is_genetic_loaded()` which returns a raw `bool`.
- Error responses use the format: `{"error": "message"}`.

## Exported Functions (15 total)

Simulation: `simulate`, `simulate_with_histories`, `simulate_one`, `simulate_one_with_history`
Strategy/Rules info: `get_available_strategies`, `get_available_rules`, `get_strategy_descriptions`, `get_rules_info`
Interactive game: `create_interactive_game`, `get_game_state`, `apply_action`, `apply_bot_action`, `destroy_interactive_game`
Genetic model: `set_genetic_genome`, `is_genetic_loaded`

When adding new exports, follow the same pattern: accept JSON string config, return JSON string result, handle errors as `{"error": "..."}`.

## State Management

- Interactive games are stored in a **thread-local** `HashMap<u32, InteractiveGame>` with auto-incrementing IDs.
- The genetic genome is cached in **thread-local** storage.
- Always clean up: `destroy_interactive_game` must remove the game from the map.

## Config Types

- `WasmSimConfig { num_games, seed, strategies[], rules? }`
- `SingleGameConfig { seed, strategies[], rules?, max_turns_per_round? }`
- `InteractiveGameConfig { num_players, player_names[], rules?, seed }`
- Strategy and rule names are mapped to concrete types via match statements in `lib.rs`.

## Build

- Built with `wasm-pack build --target web --out-dir ../frontend/pkg`.
- Output directory `frontend/pkg/` is gitignored.
- Must compile for `wasm32-unknown-unknown` target — no platform-specific code.
