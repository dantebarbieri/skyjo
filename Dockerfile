# Stage 1: Build WASM from Rust
FROM rust:latest AS wasm-build

RUN curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY skyjo-core/ skyjo-core/
COPY skyjo-wasm/ skyjo-wasm/
# Stub skyjo-server to satisfy workspace
COPY skyjo-server/Cargo.toml skyjo-server/Cargo.toml
RUN mkdir -p skyjo-server/src && echo "fn main() {}" > skyjo-server/src/main.rs

RUN cd skyjo-wasm && wasm-pack build --release --target web --out-dir ../frontend/pkg

# Stage 2: Build frontend
FROM node:22-alpine AS frontend-build

ENV PNPM_HOME="/pnpm"
ENV PATH="$PNPM_HOME:$PATH"
RUN corepack enable

WORKDIR /app
COPY frontend/package.json frontend/pnpm-lock.yaml frontend/pnpm-workspace.yaml ./
RUN pnpm install --frozen-lockfile

COPY frontend/ .
COPY --from=wasm-build /app/frontend/pkg/ pkg/
RUN pnpm build

# Stage 3: Build server binary
FROM rust:latest AS server-build

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY skyjo-core/ skyjo-core/
COPY skyjo-server/ skyjo-server/
COPY migrations/ migrations/
# Stub skyjo-wasm to satisfy workspace
COPY skyjo-wasm/Cargo.toml skyjo-wasm/Cargo.toml
RUN mkdir -p skyjo-wasm/src && echo "" > skyjo-wasm/src/lib.rs

RUN cargo build --release --package skyjo-server

# Stage 4: Final image — Rust server serves static files + WebSocket
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

COPY --from=server-build /app/target/release/skyjo-server /usr/local/bin/
COPY --from=frontend-build /app/dist/ /var/www/static/

EXPOSE 8080

VOLUME ["/var/lib/skyjo"]
ENV SKYJO_DATA_DIR=/var/lib/skyjo

HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD timeout 3 bash -c 'cat < /dev/null > /dev/tcp/localhost/8080' || exit 1

CMD ["skyjo-server", "--static-dir", "/var/www/static", "--port", "8080", "--data-dir", "/var/lib/skyjo"]
