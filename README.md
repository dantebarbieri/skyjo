# Skyjo Simulator

[![Rust CI](https://github.com/dantebarbieri/skyjo/actions/workflows/rust.yml/badge.svg)](https://github.com/dantebarbieri/skyjo/actions/workflows/rust.yml)
[![Server CI](https://github.com/dantebarbieri/skyjo/actions/workflows/server.yml/badge.svg)](https://github.com/dantebarbieri/skyjo/actions/workflows/server.yml)
[![Frontend CI](https://github.com/dantebarbieri/skyjo/actions/workflows/frontend.yml/badge.svg)](https://github.com/dantebarbieri/skyjo/actions/workflows/frontend.yml)
[![Docker Build](https://github.com/dantebarbieri/skyjo/actions/workflows/docker.yml/badge.svg)](https://github.com/dantebarbieri/skyjo/actions/workflows/docker.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

A web-based [Skyjo](https://magilano.com/en/products/skyjo) card game — play it in your browser or simulate thousands of games in seconds. Powered by a Rust engine compiled to WebAssembly.

**[Live Demo →](https://skyjo.danteb.com)**

## Features

### Play
- **Local multiplayer** — play with friends on the same device (2–8 players)
- **Play vs bots** — challenge AI opponents using any of 11 strategies, or mix humans and bots
- **Online multiplayer** — create or join rooms by code, play in real time over WebSocket
- **Save & resume** — save your game to a file and pick up where you left off
- **PWA support** — install as an app, works offline

### Simulate
- **Batch simulation** — run up to millions of games and view aggregate statistics (win rates, score distributions, average rounds)
- **11 AI strategies** — Random, Greedy, Gambler, Rusher, Defensive, Clearer, Mimic, Saboteur, Survivor, Statistician, and Genetic (neural network)
- **Extensible rules** — the engine supports alternate rule sets (currently Standard rules; the architecture allows adding variants like "Aunt Janet Rules")
- **Game replay** — step through any game turn-by-turn with an animated board
- **Live visualization** — watch games play out in real time with adjustable speed
- **Deterministic** — every simulation is seeded and fully reproducible

### Train
- **Genetic algorithm** — evolve neural network strategies via server-side training
- **Save & load generations** — persist and resume training, import/export genomes

## Architecture

```
skyjo-core/     Rust library — game engine, simulation, 11 AI strategies
skyjo-wasm/     wasm-bindgen wrapper exposing the engine to JavaScript
skyjo-server/   Axum game server — WebSocket multiplayer, genetic training, static file serving
frontend/       React + TypeScript + Tailwind CSS (Vite)
```

All game logic lives in Rust. The frontend is a pure consumer — it reads histories and stats from the WASM module (local play, simulation) or the game server (online multiplayer). Simulations run in a Web Worker so the UI stays responsive.

## Getting Started

### Docker (recommended)

Build and run the production image:

```bash
docker build -t skyjo .
docker run -p 8080:8080 skyjo
```

Then open [http://localhost:8080](http://localhost:8080).

### Docker Compose

```bash
docker compose up --build
```

The app is served on port **8080** by default. Override with `PORT=3000 docker compose up --build`.

### Development

Prerequisites: [Rust](https://rustup.rs/), [wasm-pack](https://rustwasm.github.io/wasm-pack/installer/), [Node.js ≥ 22](https://nodejs.org/), and [pnpm](https://pnpm.io/).

1. **Build the WASM module**

   ```bash
   cd skyjo-wasm
   wasm-pack build --target web --out-dir ../frontend/pkg
   ```

2. **Install frontend dependencies**

   ```bash
   cd frontend
   pnpm install
   ```

3. **Start the game server** (for online multiplayer / genetic training)

   ```bash
   cargo run --package skyjo-server -- --port 3001
   ```

4. **Start the dev server**

   ```bash
   cd frontend
   pnpm dev
   ```

   Vite will start at [http://localhost:5173](http://localhost:5173) with hot-module replacement. API requests are proxied to the game server on port 3001.

To rebuild WASM after Rust changes, re-run the `wasm-pack build` command above, or use `pnpm dev` from the repo root if the [dev skill](.copilot/skills/dev.md) is configured (it watches for Rust changes automatically).

### Lint & Test

```bash
# Rust
cargo fmt --all --check                        # format check (CI enforced)
cargo clippy --manifest-path skyjo-core/Cargo.toml -- -D warnings
cargo clippy --manifest-path skyjo-server/Cargo.toml -- -D warnings
cargo test --manifest-path skyjo-core/Cargo.toml
cargo test --manifest-path skyjo-server/Cargo.toml

# Frontend
cd frontend && pnpm lint                       # tsc --noEmit
cd frontend && pnpm test                       # Vitest
```

## License

MIT — see [LICENSE](LICENSE).

This project is not affiliated with Magilano GmbH.
