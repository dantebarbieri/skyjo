# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Skyjo board game simulator. Rust core compiled to WebAssembly, with a TypeScript/HTML/CSS frontend for visualization. The simulator runs configurable game simulations and records full game histories for replay and statistical analysis.

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
frontend/        — TypeScript + HTML + CSS (visualization, replay, stats)
```

### Rust Core (`skyjo-core`)

The core is designed around **trait-based extensibility** for both player strategies and rule variants:

- **`Rules` trait** — Abstracts game rules (e.g., standard rules vs "Aunt Janet Rules" with per-player discard piles). Implementations control discard pile behavior, end-of-round scoring penalties, column-clearing rules, etc. New rule variants are added by implementing this trait — no conditionals scattered through game logic.
- **`Strategy` trait** — Defines player decision-making: which cards to flip initially, whether to draw from deck or discard, whether to keep or discard a drawn card, which board position to place/reveal. Strategies receive a `StrategyView` with all public knowledge: own board, opponent boards, full discard pile contents (for card counting), deck size, cumulative scores, and whether it's the final turn.
- **`Game` struct** — Orchestrates rounds, turns, and scoring. Parameterized by a `Rules` implementation. Records a full `GameHistory` of every action for deterministic replay.
- **`GameHistory`** — Complete record of a game: initial deck order, all player actions per turn. Sufficient to reconstruct and replay any game state. Serializable (serde) for passing to the frontend.
- **`Simulator`** — Runs batches of games with given strategies/rules, collects aggregate statistics (score distributions, win rates, average game length, etc.).

### WASM Bridge (`skyjo-wasm`)

Thin wasm-bindgen layer exposing simulation and replay to JS via JSON strings. Six exported functions:
- `simulate(config_json)` — batch simulation, returns `AggregateStats` as JSON
- `simulate_with_histories(config_json)` — batch simulation, returns `{ stats, histories }` as JSON
- `simulate_one(config_json)` — single game, returns `GameStats` as JSON (used by Web Worker for incremental progress)
- `simulate_one_with_history(config_json)` — single game, returns `{ stats, history }` as JSON
- `get_available_strategies()` — returns strategy name list as JSON
- `get_available_rules()` — returns rule set name list as JSON

Batch config: `{ num_games, seed, strategies: string[], rules?: string }`. Single game config: `{ seed, strategies: string[], rules?: string }`. Strategy/rule names are mapped to concrete types via match statements in `lib.rs`. All serialization uses `serde_json` (string-based JSON, not `serde-wasm-bindgen`).

The WASM module is built to `frontend/pkg/` via `wasm-pack build --target web --out-dir ../frontend/pkg`. The `pkg/` directory is gitignored in `frontend/.gitignore`.

### Frontend (`frontend`)

- **React + TypeScript + Tailwind CSS + shadcn/ui**, bundled with Vite
- **Component architecture**: React components in `src/components/`, custom hooks in `src/hooks/`, pure logic in `src/lib/`
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
- Key files: `src/App.tsx` (root), `src/hooks/use-simulation.ts` (worker management), `src/hooks/use-replay.ts` (replay state), `src/lib/replay-engine.ts` (board reconstruction), `src/components/skyjo-card.tsx` (card rendering), `src/types.ts` (TypeScript interfaces + worker message types), `src/worker.ts` (WASM simulation loop)

## Build Commands

```bash
# Build Rust core (check/test without WASM)
cd skyjo-core && cargo build
cd skyjo-core && cargo test
cd skyjo-core && cargo test <test_name>    # single test

# Build WASM (outputs to frontend/pkg/)
cd skyjo-wasm && wasm-pack build --target web --out-dir ../frontend/pkg

# Frontend dev
cd frontend && npm install
cd frontend && npm run dev                  # dev server (Vite)
cd frontend && npm run build                # production build

# Full rebuild (WASM + frontend)
cd skyjo-wasm && wasm-pack build --target web --out-dir ../frontend/pkg && cd ../frontend && npm run build

# Lint
cd skyjo-core && cargo clippy -- -D warnings
cd skyjo-wasm && cargo clippy -- -D warnings
cd frontend && npm run lint                 # tsc --noEmit
```

## Key Design Principles

- **Trait objects for hot-swappability**: `Rules` and `Strategy` are trait objects (`Box<dyn Rules>`, `Box<dyn Strategy>`), allowing runtime selection without recompilation. This is the mechanism for rule/strategy variations.
- **Deterministic replay**: All randomness flows through a seedable RNG. Combined with `GameHistory`, any game can be reproduced exactly.
- **Parameterized grid**: Grid dimensions (rows × columns) are defined by the Rules trait, not hardcoded. Board state uses `Vec<Slot>` sized to `rows * cols`. Column operations are index-based.
- **Frontend is purely a consumer**: All game logic lives in Rust. The frontend never computes game state — it reads histories and stats from WASM.
- **Serialization boundary**: The WASM↔JS boundary passes JSON-serialized structs. No complex bindings — keep the interface narrow.

## Card Representation

Cards use a compact representation: `i8` value (-2 to 12), with board slots tracked as `enum Slot { Hidden(i8), Revealed(i8), Cleared }`. The hidden value exists in the slot but is not visible to strategies (strategies see `VisibleSlot` — `Hidden` variant has no value). Grid size is determined by `Rules::num_rows()` and `Rules::num_cols()`; standard is 3×4=12 slots.
