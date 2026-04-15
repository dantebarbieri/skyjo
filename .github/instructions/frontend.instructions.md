---
applyTo: "frontend/src/**/*.ts,frontend/src/**/*.tsx"
---

# Frontend — Review Guidelines

## Stack

React 19 + TypeScript (strict mode) + Tailwind CSS 4 + shadcn/ui, bundled with Vite 6. PWA support via vite-plugin-pwa with Workbox.

## TypeScript Conventions

- **Strict mode is enforced**: `strict: true`, `noFallthroughCasesInSwitch: true` in tsconfig.
- **Zod schemas are the single source of truth** for types. Define the schema first, then infer the TypeScript type: `z.infer<typeof MySchema>`. Never duplicate type definitions manually.
- All API responses and worker messages should be validated via Zod at runtime.
- Use `pnpm lint` (`tsc --noEmit`) to verify — must pass with zero errors.

## Component Architecture

- **UI primitives**: `src/components/ui/` (shadcn/ui library — don't modify these directly)
- **Business components**: `src/components/` (game board, cards, scoring, etc.)
- **Route components**: `src/routes/` (page-level components)
- **Custom hooks**: `src/hooks/` (state management, async flows, WebSocket, Web Worker)
- **Pure logic**: `src/lib/` (replay engine, utilities)
- **Contexts**: `src/contexts/` (WASM provider, auth)
- **Types**: `src/types.ts` (shared interfaces, worker message types, Zod schemas)

## Styling

- Use Tailwind CSS classes exclusively. No inline styles except for critical SVG/canvas positioning.
- Use the `cn()` utility (from `src/lib/utils.ts`) for conditional/dynamic class composition.
- shadcn/ui components provide the design system — prefer them over custom implementations.

## State Management

- Complex state lives in custom hooks (e.g., `use-simulation.ts`, `use-interactive-game.ts`, `use-online-game.ts`).
- Hooks manage async flows: Web Worker communication, WebSocket state, WASM calls.
- Prefer composition over deep prop drilling.

## Performance

- **Heavy computation runs in Web Workers** — simulation uses a dedicated worker (`src/worker.ts`) that loads WASM independently and posts incremental progress every ~50ms.
- Never run batch simulation on the main thread.
- The worker supports pause/resume/stop messages.
- WASM is loaded once via `WasmProvider` context.

## Frontend is a Consumer

- The frontend **never computes game state**. All game logic lives in Rust (via WASM or server).
- The frontend reads histories and stats, reconstructs board visuals from `GameHistory`, and renders UI.
- The replay engine (`src/lib/replay-engine.ts`) reconstructs board state from history for visualization.

## Testing

- **Vitest** (not Jest) with React Testing Library.
- Test files: `src/components/__tests__/component-name.test.tsx` or colocated.
- Query by accessible role/label — avoid querying by implementation details (class names, data-testid when possible).
- Run with `pnpm test` (single run) or `pnpm test:watch` (dev).

## Common Review Flags

- `console.log()` left in production code
- Manual type definitions that should be Zod-inferred
- Inline styles instead of Tailwind classes
- Game logic reimplemented in TypeScript instead of calling WASM
- Blocking the main thread with heavy computation
- Missing error handling for async operations (WASM calls, WebSocket, worker messages)
