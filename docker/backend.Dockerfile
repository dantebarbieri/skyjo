# Stage 1: Build server binary
FROM rust:1.88 AS server-build

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY skyjo-core/ skyjo-core/
COPY skyjo-server/ skyjo-server/
COPY migrations/ migrations/
# Stub skyjo-wasm to satisfy workspace
COPY skyjo-wasm/Cargo.toml skyjo-wasm/Cargo.toml
RUN mkdir -p skyjo-wasm/src && echo "" > skyjo-wasm/src/lib.rs

RUN cargo build --release --package skyjo-server

# Stage 2: Final image — Rust server only (no static files)
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y ca-certificates curl && rm -rf /var/lib/apt/lists/*

COPY --from=server-build /app/target/release/skyjo-server /usr/local/bin/

EXPOSE 8080

VOLUME ["/var/lib/skyjo"]
ENV SKYJO_DATA_DIR=/var/lib/skyjo

HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD curl --fail --silent --show-error --max-time 2 http://localhost:8080/api/health || exit 1

CMD ["skyjo-server", "--port", "8080", "--data-dir", "/var/lib/skyjo"]
