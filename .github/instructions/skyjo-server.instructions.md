---
applyTo: "skyjo-server/**/*.rs"
---

# Skyjo Server — Review Guidelines

## Error Handling

- All errors use the `ServerError` enum in `src/error.rs`.
- Each variant maps to an HTTP status code via `status_code()` method.
- `IntoResponse` is implemented to produce JSON: `{ "error": { "code": "...", "message": "..." } }`.
- User-facing messages come from `.message()` — never expose internal details.
- New error variants must include: status code mapping, user-safe message, Display impl, and tests.

## Concurrency

- Room and session state uses `DashMap` for lock-free concurrent access.
- Genetic training state uses `Arc<Mutex<>>`.
- `Persistence` (SQLite via rusqlite) is optional in `AppStateInner`.
- Be careful with lock ordering to avoid deadlocks — never hold a DashMap guard while awaiting.

## WebSocket Protocol

- Supports dual format: JSON (text frames) and MessagePack (binary frames).
- Client format is auto-detected from frame type via `WireFormat` enum.
- MessagePack encoding **must** use `rmp_serde::to_vec_named` (struct-map) because protocol structs use `skip_serializing_if`.
- Client→Server messages: `ConfigureSlot`, `SetNumPlayers`, `SetRules`, `SetTurnTimer`, `KickPlayer`, `BanPlayer`, `PromoteHost`, `StartGame`, `PlayAgain`, `ReturnToLobby`, `Action`, `ContinueRound`, `Ping`.
- Server→Client messages: `RoomState`, `GameState`, `ActionApplied`, `BotAction`, `TimeoutAction`, `PlayerJoined`, `PlayerLeft`, `PlayerReconnected`, `Kicked`, `Error`, `Pong`.

## Room Management

- Rooms support 2–8 players with 6-character codes.
- Player slots: Human/Bot/Empty.
- IP banning is creator-only.
- Turn timers are optional.
- Automatic cleanup: 5 min after game over, 10 min after disconnect.

## Authentication

- Session tokens authenticate WebSocket connections.
- Tokens are passed as query parameters on WebSocket upgrade.

## Rate Limiting

- `RateLimiter` buckets are keyed by `(IpAddr, resource)`.
- Bucket parameters come from the first config used for a key — callers must use distinct resource strings per limit type.

## Persistence

- SQLite (rusqlite bundled) stored under `SKYJO_DATA_DIR` (default `./data`, Docker: `/var/lib/skyjo`).
- Schema: `game_replays`, `player_stats`, `room_snapshots`.
- Migrations tracked in `_migrations` table using `sqlx::raw_sql(include_str!(...))`.

## Testing

- Server tests require the database and run with `--test-threads=1` due to shared DB state.
- Integration tests in `tests/` directory; unit tests inline.
- Use `cargo test --manifest-path skyjo-server/Cargo.toml` to run.

## Genetic Training API

- Population of 100, 30 games per evaluation, 3-way tournament selection.
- Adaptive mutation with stagnation detection.
- BLX-α crossover.
- Parallel evaluation via `rayon`.
- Max 10,000 generations per session (100,000 in `until_fitness` mode).
- Checkpoint interval: 1,000 generations.
