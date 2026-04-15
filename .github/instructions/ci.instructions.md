---
applyTo: ".github/workflows/**/*.yml"
---

# CI Workflow — Review Guidelines

## Existing Workflows

- **`rust.yml`** — Rust core: format check (`cargo fmt --all --check`), clippy (`-D warnings`), tests (`skyjo-core`)
- **`server.yml`** — Server: format check, clippy, tests (`skyjo-server`, `--test-threads=1`)
- **`frontend.yml`** — Frontend: WASM build, `pnpm install --frozen-lockfile`, type check (`tsc --noEmit`), tests (`pnpm test`), production build (`pnpm build`)
- **`release.yml`** — Release pipeline
- **`docker.yml`** — Docker image build

## Review Checklist for Workflow Changes

- Ensure all existing checks continue to run — do not remove or weaken quality gates.
- `cargo clippy` must use `-D warnings` (deny all warnings). Never downgrade to allow warnings.
- `cargo fmt` must use `--check` flag. Never auto-format in CI without checking first.
- Server tests must use `--test-threads=1` due to shared database state.
- Frontend must use `--frozen-lockfile` to ensure lockfile integrity.
- New dependencies should be added to the appropriate cache keys.
- Workflow changes should be tested on a branch before merging to main.

## Adding New Checks

If adding new CI checks:
- Document what the check validates and why it's needed.
- Ensure the check runs on both `push` and `pull_request` triggers as appropriate.
- Use consistent naming conventions with existing workflows.
- Pin action versions to specific SHA or major version tags for security.
