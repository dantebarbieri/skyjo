---
name: dev
description: Launch the full dev environment — Vite dev server with HMR, automatic WASM rebuild on Rust changes, and the game server for online play.
user-invocable: true
allowed-tools: Bash(cargo:*) Bash(wasm-pack:*) Bash(pnpm:*) Bash(kill:*) Bash(lsof:*)
---

# Dev Environment

Launch the Skyjo development environment with hot module reload for frontend changes, automatic WASM rebuild when Rust source files change, and the game server for online play.

## Steps

### 1. Ensure cargo-watch is installed

```bash
cargo install cargo-watch
```

Skip if already installed (cargo will report it's up to date).

### 2. Build WASM once to ensure pkg/ is fresh

```bash
cd skyjo-wasm && wasm-pack build --target web --out-dir ../frontend/pkg
```

### 3. Install frontend dependencies if needed

```bash
cd frontend && pnpm install
```

### 4. Start all three processes in background

Start the game server (for online play API + WebSocket):

```bash
cd skyjo-server && cargo run -- --port 3001 --static-dir ../frontend/dist
```

Run this in the background using `run_in_background: true`.

Start the Vite dev server (proxies /api to the game server on port 3001):

```bash
cd frontend && pnpm dev
```

Run this in the background using `run_in_background: true`, then start cargo-watch:

```bash
cd skyjo-wasm && cargo watch -w ../skyjo-core/src -w src -s "wasm-pack build --target web --out-dir ../frontend/pkg"
```

Run this in the background too using `run_in_background: true`.

### 5. Confirm

After launching all three processes, report the Vite URL (usually http://localhost:5173) and confirm that:
- Vite is serving the frontend with HMR
- Vite proxies `/api` requests to the game server on port 3001
- cargo-watch is monitoring `skyjo-core/src/` and `skyjo-wasm/src/` for Rust changes
- Any `.rs` file change triggers a WASM rebuild, and Vite picks up the new `pkg/` output
- The game server is running for online play (create/join rooms via `/play/online`)

## Arguments

- `/dev` — full setup (default)
- `/dev wasm` — only rebuild WASM once, no watch
- `/dev frontend` — only start Vite, skip Rust watching
