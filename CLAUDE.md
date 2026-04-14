# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Skyjo board game simulator, playable web app, and online multiplayer server. Rust core compiled to WebAssembly, with a TypeScript/React frontend and an Axum-based game server. Users can **play Skyjo** (local multiplayer, vs bots, or online multiplayer), **run simulations** (batch games with configurable AI strategies, full replay, and statistical analysis), or **train genetic AI** via an evolutionary algorithm.

### Game Rules (Skyjo)

- **Deck**: 150 cards total — 5×(-2), 10×(-1), 15×(0), 10× each of [1–12]
- **Setup**: Each player gets cards in a grid (standard: 4 columns × 3 rows = 12 cards), all hidden. Each player flips 2 cards to start. Grid dimensions are configurable via the Rules trait.
- **Turn**: Draw from deck OR discard pile.
  - **Deck draw**: Keep it (replace any board card, discarding the replaced card) OR discard it and flip one hidden card.
  - **Discard pile draw**: Must keep it — replace any board card (hidden or revealed), discarding the replaced card. Cannot discard the drawn card.
- **Column clearing**: When all cards in a column are revealed and match, first discard the card that completed the match, then discard all cards in the column. The last discarded card becomes the new top of the discard pile. Column clearing also applies during the final reveal step at end of round.
- **Deck exhaustion**: Shuffle the discard pile to form a new draw pile, flip its top card to start a new discard pile (standard rule).
- **Going out**: When a player reveals all cards, their turn ends. Remaining players each get 1 final turn in order until play would return to the going-out player. The going-out player does NOT get an additional turn.
- **Going out penalty**: The going-out player must have the **solo lowest** score (ties do NOT count as lowest). If their positive score is not solo lowest, it is doubled. Scores ≤ 0 are never penalized.
- **Scoring**: Each player's score = sum of all their remaining cards (cleared columns count as 0). Cumulative across rounds.
- **Game end**: After a round ends, if any player's cumulative score ≥ 100, the game is over. Lowest cumulative score wins. Ties are possible (multiple winners).
- **Turn order**: Round 1 — highest sum of initially flipped cards goes first (tiebreak: first in player order). Subsequent rounds — the player who went out last round goes first.
- **Tiebreak policies**: Two abstracted tiebreak situations: (1) who starts a round when initial flip sums tie, (2) who wins when final cumulative scores tie. Both configurable via Rules trait.

## Architecture

### Separation of Concerns

```
skyjo-core/      — Rust library crate (game engine, simulation, strategies)
skyjo-wasm/      — Rust crate that wraps skyjo-core with wasm-bindgen exports
skyjo-server/    — Rust binary crate (Axum game server: WebSocket multiplayer, genetic training, static file serving)
frontend/        — React + TypeScript + Tailwind CSS (UI for play, simulation, replay)
```

### Rust Core (`skyjo-core`)

The core is designed around **trait-based extensibility** for both player strategies and rule variants:

- **`Rules` trait** — Abstracts game rules (e.g., standard rules vs "Aunt Janet Rules" with per-player discard piles). Implementations control discard pile behavior, end-of-round scoring penalties, column-clearing rules, etc. New rule variants are added by implementing this trait — no conditionals scattered through game logic. Currently one implementation: `StandardRules` (3×4 grid, 2 initial flips, 100-point threshold).
- **`Strategy` trait** — Defines player decision-making: which cards to flip initially, whether to draw from deck or discard, whether to keep or discard a drawn card, which board position to place/reveal. Strategies receive a `StrategyView` with all public knowledge: own board, opponent boards, full discard pile contents (for card counting), deck size, cumulative scores, and whether it's the final turn.
- **`Game` struct** — Orchestrates rounds, turns, and scoring. Parameterized by a `Rules` implementation. Records a full `GameHistory` of every action for deterministic replay.
- **`InteractiveGame`** — Turn-by-turn game control for interactive play. Exposes `ActionNeeded` (which decision the current player must make) and `PlayerAction` (the response). Used by both WASM interactive API and the game server.
- **`GameHistory`** — Complete record of a game: initial deck order, all player actions per turn. Sufficient to reconstruct and replay any game state. Serializable (serde) for passing to the frontend.
- **`Simulator`** — Runs batches of games with given strategies/rules, collects aggregate statistics (score distributions, win rates, average game length, etc.).

#### Strategies (11 total)

| Strategy | Complexity | Description |
|----------|-----------|-------------|
| Random | Trivial | Completely random decisions |
| Greedy | Low | Locally optimal card value comparisons |
| Gambler | Low | High-variance, aggressive hidden card flipping |
| Rusher | Low | Ends rounds fast, speed over optimization |
| Defensive | Medium | Opponent denial via discard pile control |
| Clearer | Medium | Column clearing as primary mechanic |
| Mimic | Medium | Copies the leader's board patterns |
| Saboteur | Medium | Poisons discard pile for next player |
| Survivor | Medium | Adaptive risk based on cumulative score |
| Statistician | High | Expected value calculations + card counting |
| Genetic | High | Feedforward neural network (48→32→39) with evolved weights |

Shared strategy utilities live in `strategies/common.rs`: card counting (`deck_distribution`, `count_visible`, `count_remaining`), expected value calculations (`average_unknown_value`, `expected_score`), column analysis (`column_analysis`), and opponent denial scoring (`card_usefulness_to_player`).

### WASM Bridge (`skyjo-wasm`)

Thin wasm-bindgen layer exposing simulation, interactive play, and genetic model loading to JS via JSON strings. Fifteen exported functions:

**Simulation:**
- `simulate(config_json)` — batch simulation, returns `AggregateStats` as JSON
- `simulate_with_histories(config_json)` — batch simulation, returns `{ stats, histories }` as JSON
- `simulate_one(config_json)` — single game, returns `GameStats` as JSON (used by Web Worker for incremental progress)
- `simulate_one_with_history(config_json)` — single game, returns `{ stats, history }` as JSON

**Strategy & Rules info:**
- `get_available_strategies()` — returns strategy name list as JSON
- `get_available_rules()` — returns rule set name list as JSON
- `get_strategy_descriptions()` — returns strategy metadata + common concepts as JSON
- `get_rules_info(rules_name)` — returns grid size, initial flips, penalties, etc. as JSON

**Interactive game (turn-by-turn):**
- `create_interactive_game(config_json)` — creates a new game, returns game ID + initial state
- `get_game_state(game_id, player_index)` — query game state (player-specific view)
- `apply_action(game_id, action_json)` — apply a human player's action
- `apply_bot_action(game_id, strategy_name)` — bot plays its turn using named strategy
- `destroy_interactive_game(game_id)` — clean up game from memory

**Genetic model:**
- `set_genetic_genome(model_json)` — load a genome for the Genetic strategy to use
- `is_genetic_loaded()` — check if a genome is loaded

Config structs: `WasmSimConfig { num_games, seed, strategies[], rules? }`, `SingleGameConfig { seed, strategies[], rules?, max_turns_per_round? }`, `InteractiveGameConfig { num_players, player_names[], rules?, seed }`. Strategy/rule names are mapped to concrete types via match statements in `lib.rs`. All serialization uses `serde_json` (string-based JSON, not `serde-wasm-bindgen`). All functions return JSON; errors are `{"error": "message"}`.

Interactive games are stored in thread-local `HashMap<u32, InteractiveGame>` with auto-incrementing IDs. The genetic genome is cached in thread-local storage.

The WASM module is built to `frontend/pkg/` via `wasm-pack build --target web --out-dir ../frontend/pkg`. The `pkg/` directory is gitignored in `frontend/.gitignore`.

### Game Server (`skyjo-server`)

Axum-based multiplayer game server providing room management, real-time WebSocket gameplay, genetic algorithm training, and static file serving.

**Core state:** `AppState` wraps a `Lobby` (room management) and `GeneticTrainingState` (background training). Concurrent access uses `DashMap` for rooms/sessions and `Arc<Mutex<>>` for genetic state.

**REST API:**
- `POST /api/rooms` — create a room (returns room code + session token)
- `GET /api/rooms/{code}` — room info
- `POST /api/rooms/{code}/join` — join a room (returns session token)
- `GET /api/rooms/{code}/ws?token=...` — WebSocket upgrade (authenticated via session token)

**Genetic training API:**
- `GET /api/genetic/model` — current best genome
- `POST /api/genetic/train` — start training (configurable generations, fitness threshold)
- `POST /api/genetic/stop` — stop training
- `POST /api/genetic/reset` — reset population
- `POST /api/genetic/load` — load saved generation
- `GET /api/genetic/status` — training progress
- `GET /api/genetic/saved` — list saved generations
- `POST /api/genetic/saved` — save current generation
- `POST /api/genetic/saved/import` — import external genome
- `DELETE /api/genetic/saved/{name}` — delete a saved generation
- `GET /api/genetic/saved/{name}/model` — get a specific saved genome

**WebSocket protocol (client → server):** `ConfigureSlot`, `SetNumPlayers`, `SetRules`, `SetTurnTimer`, `KickPlayer`, `BanPlayer`, `PromoteHost`, `StartGame`, `PlayAgain`, `ReturnToLobby`, `Action`, `ContinueRound`, `Ping`

**WebSocket protocol (server → client):** `RoomState`, `GameState`, `ActionApplied`, `BotAction`, `TimeoutAction`, `PlayerJoined`, `PlayerLeft`, `PlayerReconnected`, `Kicked`, `Error`, `Pong`

**Room features:** 2–8 player rooms with 6-character codes, player slots (Human/Bot/Empty), IP banning (creator-only), optional turn timers, automatic cleanup (5min after game over, 10min after disconnect).

**Genetic training:** Population of 50, 10 games per individual, 3-way tournament selection, 5% mutation rate (σ=0.3), 0.5% reset rate, parallel evaluation via `rayon`, persistent save/load with lineage tracking, max 10,000 generations per session (100,000 in `until_fitness` mode).

**CLI args:** `--port` (default 8080), `--static-dir` (default `./static`), `--genetic-model-path` (default `./genetic_model.json`).

**Dependencies:** axum (HTTP + WebSocket), tokio (async runtime), tower-http (static files, gzip compression), dashmap (concurrent state), rayon (parallel genetic eval), clap (CLI), tracing (structured logging).

### Frontend (`frontend`)

- **React 19 + TypeScript + Tailwind CSS 4 + shadcn/ui**, bundled with Vite 6
- **PWA support**: vite-plugin-pwa with Workbox for offline caching and install prompts
- **Component architecture**: React components in `src/components/`, custom hooks in `src/hooks/`, pure logic in `src/lib/`, route components in `src/routes/`
- **WASM context**: `WasmProvider` (`src/contexts/wasm-context.tsx`) loads and provides the WASM module to the component tree
- **Async simulation via Web Worker**: Simulations run in a dedicated Web Worker (`src/worker.ts`) to avoid blocking the main thread. The worker loads WASM independently, runs games one at a time via `simulate_one()`, accumulates stats incrementally, and posts progress updates to the main thread every ~50ms.
- **Live progress**: Progress bar, elapsed time, ETA, games/sec, and live-updating stats table that updates as games complete.
- **Pause/Resume/Stop**: Worker supports pause/resume/stop messages. Main thread controls via buttons.
- **Configuration UI**: Number of players (2–8), strategy per player, rule variant, game count, seed — all using shadcn/ui Select/Input/Button components
- **Stats table**: Per-player win rates, average/min/max scores, average rounds/turns per game — shadcn Table, live-updating during simulation
- **Game list**: Paginated game histories table (shadcn Table + Pagination), with per-game scoring sheet view
- **Scoring sheet**: Round-by-round scorepad (rows=rounds, cols=players) matching the physical Skyjo scorepad
- **Game replay**: Step through `GameHistory` turn-by-turn. Board state is reconstructed in TypeScript from the history (deal → flips → turns → end-of-round). Column-major board layout (index = col * numRows + row).
- **Skyjo card component**: CSS-only cards replicating real Skyjo aesthetics — correct color scheme (purple/blue/green/yellow/red), hexagonal mosaic pattern overlay, corner numbers in white circles
- **Real-time visualization**: Live game view during simulation with speed controls
- **Interactive play mode** (`src/routes/play.tsx`): Full playable Skyjo — local multiplayer (multiple humans on one device), vs bots (any AI strategy), or mixed. Features: configurable player count (2–8), per-player human/bot assignment, bot speed control, save/resume/import/export game state, end-of-round scoring sheet, and play-again flow. Bot turns are automated via `src/hooks/use-bot-turns.ts`; human turns use click-based interaction (flip cards, draw from deck/discard, place/swap). Game state is driven by `src/hooks/use-interactive-game.ts` which calls into WASM for action validation and state transitions.
- **Online multiplayer** (`src/routes/play-online.tsx`): WebSocket-based multiplayer — create/join rooms by code, lobby with player slot configuration (Human/Bot/Empty), real-time game play, reconnection support, session persistence via `sessionStorage`. Game hook: `src/hooks/use-online-game.ts`.
- **Strategy documentation** (`src/routes/strategies.tsx`): Browse strategy descriptions, decision logic per phase, and common concepts. Includes per-strategy detail view at `/rules/strategies/:name`.
- **Genetic algorithm manager** (`src/routes/genetic-manage.tsx`): Train, save/load, import/export genetic models. Neural network visualization (`src/components/neural-network-viz.tsx`).
- **Simulation cache**: Cache simulation results for quick retrieval (`src/cache.ts`, `src/hooks/use-cache.ts`, `src/components/cache-panel.tsx`).

**Routes:**
| Path | Component | Purpose |
|------|-----------|---------|
| `/` | redirect | → `/rules` |
| `/rules` | RulesRoute | Game rules documentation |
| `/rules/strategies` | StrategiesRoute | Strategy list and details |
| `/rules/strategies/:name` | StrategiesRoute | Individual strategy deep-dive |
| `/rules/strategies/Genetic/manage` | GeneticManageRoute | Genetic algorithm training UI |
| `/simulator` | SimulatorRoute | Batch simulation with AI strategies |
| `/play` | PlayRoute | Local multiplayer + vs bots |
| `/play/online` | PlayOnlineRoute | Online multiplayer lobby |
| `/play/online/:roomCode` | PlayOnlineRoute | Join existing room |

**Key files:** `src/App.tsx` (root), `src/main.tsx` (router + bootstrap), `src/contexts/wasm-context.tsx` (WASM provider), `src/hooks/use-simulation.ts` (worker management), `src/hooks/use-replay.ts` (replay state), `src/hooks/use-interactive-game.ts` (play mode state), `src/hooks/use-online-game.ts` (online multiplayer), `src/hooks/use-bot-turns.ts` (bot automation), `src/lib/replay-engine.ts` (board reconstruction), `src/components/skyjo-card.tsx` (card rendering), `src/types.ts` (TypeScript interfaces + worker message types), `src/worker.ts` (WASM simulation loop)

## Build Commands

```bash
# Build Rust core (check/test without WASM)
cd skyjo-core && cargo build
cd skyjo-core && cargo test
cd skyjo-core && cargo test <test_name>    # single test

# Build WASM (outputs to frontend/pkg/)
cd skyjo-wasm && wasm-pack build --target web --out-dir ../frontend/pkg

# Build/run game server
cd skyjo-server && cargo build
cd skyjo-server && cargo run -- --port 3001 --static-dir ../frontend/dist

# Frontend dev
cd frontend && pnpm install
cd frontend && pnpm dev                     # dev server (Vite, proxies /api → localhost:3001)
cd frontend && pnpm build                   # production build

# Full rebuild (WASM + frontend)
cd skyjo-wasm && wasm-pack build --target web --out-dir ../frontend/pkg && cd ../frontend && pnpm build

# Format
cargo fmt --all --check                     # check formatting (CI enforced)
cargo fmt --all                             # auto-fix formatting

# Lint
cd skyjo-core && cargo clippy -- -D warnings
cd skyjo-wasm && cargo clippy -- -D warnings
cd skyjo-server && cargo clippy -- -D warnings
cd frontend && pnpm lint                    # tsc --noEmit

# Test
cargo test --manifest-path skyjo-core/Cargo.toml
cargo test --manifest-path skyjo-server/Cargo.toml
cd frontend && pnpm test                    # Vitest (single run)
cd frontend && pnpm test:watch              # Vitest (watch mode)
cd frontend && pnpm test:coverage           # Vitest with coverage
```

## Key Design Principles

- **Trait objects for hot-swappability**: `Rules` and `Strategy` are trait objects (`Box<dyn Rules>`, `Box<dyn Strategy>`), allowing runtime selection without recompilation. This is the mechanism for rule/strategy variations.
- **Deterministic replay**: All randomness flows through a seedable RNG. Combined with `GameHistory`, any game can be reproduced exactly.
- **Parameterized grid**: Grid dimensions (rows × columns) are defined by the Rules trait, not hardcoded. Board state uses `Vec<Slot>` sized to `rows * cols`. Column operations are index-based.
- **Frontend is purely a consumer**: All game logic lives in Rust. The frontend never computes game state — it reads histories and stats from WASM (local play/simulation) or from the server (online multiplayer).
- **Serialization boundary**: The WASM↔JS boundary passes JSON-serialized structs. No complex bindings — keep the interface narrow. The server↔client boundary uses JSON over WebSocket.
- **Server is authoritative for online play**: The game server owns game state for multiplayer rooms. Clients send actions, server validates and broadcasts state updates.

## Card Representation

Cards use a compact representation: `i8` value (-2 to 12), with board slots tracked as `enum Slot { Hidden(i8), Revealed(i8), Cleared }`. The hidden value exists in the slot but is not visible to strategies (strategies see `VisibleSlot` — `Hidden` variant has no value). Grid size is determined by `Rules::num_rows()` and `Rules::num_cols()`; standard is 3×4=12 slots.
