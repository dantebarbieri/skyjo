---
applyTo: "**/*.rs"
---

# Rust Code Review Guidelines

## Error Handling

- Use typed error enums, never `String` errors in public APIs. `skyjo-core` uses `SkyjoError`; `skyjo-server` uses `ServerError`.
- Every error variant must implement `Display` with a meaningful, distinct message.
- Error types should derive `Debug`, `Clone`, `PartialEq`, `Eq` where appropriate.
- Implement `std::error::Error` for all error types.
- Use `Result<T, Error>` with type aliases. Use the `?` operator for propagation.
- **No `unwrap()` in production code** unless there is a clear, documented invariant that guarantees `Some`/`Ok`. Prefer `.expect("reason")` with context if an invariant truly holds, or propagate with `?`.
- Every error variant should have a corresponding test.

## Trait-Based Extensibility

- Game rules are abstracted via the `Rules` trait; player AI via the `Strategy` trait. Both are used as trait objects (`Box<dyn Rules>`, `Box<dyn Strategy>`).
- New rule variants or strategies are added by implementing these traits — never scatter conditionals through game logic.
- Grid dimensions come from `Rules::num_rows()` and `Rules::num_cols()`, not hardcoded constants.

## Formatting and Linting

- All code must pass `cargo fmt --all --check` (CI-enforced).
- All code must pass `cargo clippy -- -D warnings` (zero warnings allowed).
- Rust edition is 2024. Use edition 2024 idioms.

## Testing Patterns

- Unit tests use inline `#[cfg(test)]` modules in the same file.
- Test error variants, Display messages, and edge cases — not just happy paths.
- Strategy tests should verify name consistency and serde round-trip serialization.
- Server integration tests run with `--test-threads=1` due to shared database state.

## Serialization

- Use `serde` with `#[derive(Serialize, Deserialize)]` for all types crossing boundaries.
- WebSocket MessagePack must use `rmp_serde::to_vec_named` (struct-map) to preserve field names with `skip_serializing_if`.
- WASM boundary uses JSON strings via `serde_json`, not `serde-wasm-bindgen`.

## Performance

- Release builds use `opt-level = "s"` (size-optimized) with LTO.
- Avoid unnecessary allocations in hot paths (game simulation, genetic evaluation).
- Genetic algorithm evaluation uses `rayon` for parallelism.

## Common Review Flags

- `unwrap()` or `expect()` without clear invariant justification
- `thread_rng()` or non-seeded randomness in game logic
- Hardcoded grid dimensions instead of using `Rules` trait methods
- Missing `#[cfg(test)]` test module for new logic
- Clippy suppressions (`#[allow(...)]`) without explanation
