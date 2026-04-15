---
applyTo: "skyjo-core/**/*.rs"
---

# Skyjo Core — Game Engine Review Guidelines

## Purity Constraints

`skyjo-core` is a pure library crate. It must have:
- **No async code** — no tokio, no futures
- **No I/O** — no file system, no network, no stdin/stdout
- **No non-determinism** — all randomness via seeded `StdRng` (ChaCha12)
- **No platform-specific code** — must compile to both native (64-bit) and WASM (32-bit)

## Card Representation

- Cards are `i8` values from -2 to 12.
- Board slots: `enum Slot { Hidden(i8), Revealed(i8), Cleared }`
- Strategies see `VisibleSlot` — the `Hidden` variant carries no value (information hiding).
- Grid indexing is column-major: `index = col * num_rows + row`.
- Grid size is determined by `Rules::num_rows()` and `Rules::num_cols()` (standard: 3×4 = 12 slots).

## Strategy Implementation

- Each strategy lives in its own file under `src/strategies/`.
- Shared utility functions (card counting, expected value, column analysis) live in `strategies/common.rs`.
- Strategies receive a `StrategyView` containing all public knowledge: own board, opponent boards, full discard pile contents, deck size, cumulative scores, final turn flag.
- Strategies must never access hidden card values — only `VisibleSlot` data.
- New strategies must implement `Strategy::name()` returning a unique identifier and have exactly 4 phase descriptions.

## Game Rules

- The `Rules` trait controls: grid dimensions, initial flips, discard pile behavior, scoring penalties, column-clearing rules, end conditions, tiebreak policies.
- Rules can support multiple discard piles via `discard_pile_count()`, `drawable_piles()`, `discard_target()`.
- Currently one implementation: `StandardRules` (3×4 grid, 2 initial flips, 100-point threshold).
- Changes to game logic must preserve backward compatibility with existing `GameHistory` replays.

## GameHistory and Replay

- `GameHistory` records the complete game: initial deck order and all player actions per turn.
- It must remain serializable via serde and sufficient to deterministically reconstruct any game state.
- Changes to action types or history format require careful migration consideration.

## Genetic Strategy

- Neural network architecture: feedforward (62→64→32→39) with ReLU activations.
- Architecture is versioned (currently v2). Changes require version bumps.
- `GENOME_SIZE` is derived from layer dimensions — do not hardcode.

## Key Review Checks

- Verify no hidden card values leak into `StrategyView` or `VisibleSlot`
- Verify column clearing logic: first discard the completing card, then discard all column cards
- Verify going-out penalty: only applied to positive scores, requires solo lowest (ties don't count)
- Verify turn order: round 1 by highest initial flip sum; subsequent rounds by who went out
- Verify deck exhaustion: shuffle discard pile into new draw pile, flip top card for new discard
