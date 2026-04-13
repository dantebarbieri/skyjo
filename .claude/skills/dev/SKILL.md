---
name: dev
description: Launch the full dev environment — Vite dev server with HMR plus automatic WASM rebuild on Rust changes.
user-invocable: true
allowed-tools: Bash(cargo:*) Bash(wasm-pack:*) Bash(pnpm:*) Bash(kill:*) Bash(lsof:*)
---

# Dev Environment

Launch the Skyjo development environment with hot module reload for frontend changes and automatic WASM rebuild when Rust source files change.

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

### 4. Start both processes in background

Start the Vite dev server:

```bash
cd frontend && pnpm dev
```

Run this in the background using `run_in_background: true`, then start cargo-watch:

```bash
cd skyjo-wasm && cargo watch -w ../skyjo-core/src -w src -s "wasm-pack build --target web --out-dir ../frontend/pkg"
```

Run this in the background too using `run_in_background: true`.

### 5. Confirm

After launching both processes, report the Vite URL (usually http://localhost:5173) and confirm that:
- Vite is serving the frontend with HMR
- cargo-watch is monitoring `skyjo-core/src/` and `skyjo-wasm/src/` for Rust changes
- Any `.rs` file change triggers a WASM rebuild, and Vite picks up the new `pkg/` output

## Arguments

- `/dev` — full setup (default)
- `/dev wasm` — only rebuild WASM once, no watch
- `/dev frontend` — only start Vite, skip Rust watching
