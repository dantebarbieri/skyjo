# Skyjo Simulator

[![Rust CI](https://github.com/dantebarbieri/skyjo/actions/workflows/rust.yml/badge.svg)](https://github.com/dantebarbieri/skyjo/actions/workflows/rust.yml)
[![Frontend CI](https://github.com/dantebarbieri/skyjo/actions/workflows/frontend.yml/badge.svg)](https://github.com/dantebarbieri/skyjo/actions/workflows/frontend.yml)
[![Docker Build](https://github.com/dantebarbieri/skyjo/actions/workflows/docker.yml/badge.svg)](https://github.com/dantebarbieri/skyjo/actions/workflows/docker.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

A web-based [Skyjo](https://magilano.com/en/products/skyjo) card game — play it in your browser or simulate thousands of games in seconds. Powered by a Rust engine compiled to WebAssembly.

**[Live Demo →](https://skyjo.danteb.com)**

## Features

### Play
- **Local multiplayer** — play with friends on the same device (2–8 players)
- **Play vs bots** — challenge AI opponents using any available strategy, or mix humans and bots
- **Save & resume** — save your game to a file and pick up where you left off
- **Networked multiplayer** — planned for a future release

### Simulate
- **Batch simulation** — run up to millions of games and view aggregate statistics (win rates, score distributions, average rounds)
- **Configurable strategies** — pit different AI strategies against each other
- **Rule variants** — swap in alternate rule sets (e.g., "Aunt Janet Rules")
- **Game replay** — step through any game turn-by-turn with an animated board
- **Live visualization** — watch games play out in real time with adjustable speed
- **Deterministic** — every simulation is seeded and fully reproducible

## Architecture

```
skyjo-core/   Rust library — game engine, simulation, strategies
skyjo-wasm/   wasm-bindgen wrapper exposing the engine to JavaScript
frontend/     React + TypeScript + Tailwind CSS (Vite)
```

All game logic lives in Rust. The frontend is a pure consumer — it reads histories and stats from the WASM module and never computes game state itself. Simulations run in a Web Worker so the UI stays responsive.

## Getting Started

### Docker (recommended)

Build and run the production image:

```bash
docker build -t skyjo .
docker run -p 8080:80 skyjo
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

3. **Start the dev server**

   ```bash
   pnpm dev
   ```

   Vite will start at [http://localhost:5173](http://localhost:5173) with hot-module replacement.

To rebuild WASM after Rust changes, re-run the `wasm-pack build` command above, or use `pnpm dev` from the repo root if the [dev skill](.copilot/skills/dev.md) is configured (it watches for Rust changes automatically).

### Lint & Test

```bash
# Rust
cd skyjo-core && cargo clippy -- -D warnings
cd skyjo-core && cargo test

# Frontend type-check
cd frontend && pnpm lint
```

## License

MIT — see [LICENSE](LICENSE).

This project is not affiliated with Magilano GmbH.
