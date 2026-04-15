---
applyTo: "**"
---

# Skyjo Repository — General Review Guidelines

## Project Overview

Skyjo is a board game simulator with a Rust core compiled to WebAssembly, a TypeScript/React frontend, and an Axum-based multiplayer game server. It supports local play, bot play, online multiplayer, batch simulation with AI strategies, and genetic algorithm training.

## Architecture Boundaries (Critical)

The codebase is split into four components with strict separation of concerns:

- **`skyjo-core/`** — Pure Rust library. Game engine, simulation, and AI strategies. No async, no I/O, fully deterministic. This is the source of truth for all game logic.
- **`skyjo-wasm/`** — Thin wasm-bindgen wrapper over `skyjo-core`. Exposes functions to JavaScript via JSON strings. Must remain a thin layer — no game logic here.
- **`skyjo-server/`** — Axum binary. WebSocket multiplayer, REST API, genetic training, static file serving. Server is authoritative for online play.
- **`frontend/`** — React + TypeScript UI. **Purely a consumer** — never computes game state. Reads histories and stats from WASM (local) or server (online).

When reviewing changes, verify they respect these boundaries:
- Game logic changes belong in `skyjo-core`, not in the frontend or server
- The frontend must not duplicate or reimplement game rules
- The WASM layer must stay thin — just serialization and delegation
- The server owns online game state; clients send actions, server validates

## Deterministic Replay

All randomness flows through a seedable RNG (`StdRng` / ChaCha12). Combined with `GameHistory`, any game must be reproducible exactly. When reviewing:
- Verify new randomness uses seeded RNG, never `thread_rng()` or other non-deterministic sources
- `GameHistory` must remain serializable and sufficient to reconstruct any game state
- Changes to game logic must not break replay determinism

## Serialization Boundaries

- **WASM ↔ JS**: JSON strings via `serde_json` (not `serde-wasm-bindgen`). Keep the interface narrow.
- **Server ↔ Client**: JSON over REST, JSON + MessagePack over WebSocket.
- **MessagePack**: Must use `rmp_serde::to_vec_named` (struct-map format) because protocol structs use `skip_serializing_if`.

## CI Requirements

All PRs must pass these checks before merge:
- `cargo fmt --all --check` — Rust formatting (zero tolerance)
- `cargo clippy --workspace --locked -- -D warnings` — Rust linting (deny all warnings)
- `cargo test` — Rust tests for `skyjo-core` and `skyjo-server`
- `pnpm lint` (`tsc --noEmit`) — TypeScript type checking
- `pnpm test` — Frontend tests (Vitest)
- `pnpm build` — Frontend production build
- WASM build via `wasm-pack`

## PR Quality Expectations

- Changes should be scoped to a single concern
- No leftover debug code (`dbg!()`, `console.log()`, `TODO` without issue reference)
- Documentation updated if public APIs or behavior change
- New features should include tests
- Error paths should be tested, not just happy paths
