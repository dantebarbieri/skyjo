# Stage 1: Build WASM from Rust
FROM rust:1.87 AS wasm-build

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

# Stage 3: Nginx serves static files
FROM nginx:alpine

COPY docker/nginx.conf /etc/nginx/conf.d/default.conf
COPY --from=frontend-build /app/dist/ /usr/share/nginx/html/

EXPOSE 80

HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD wget -qO- http://localhost/ || exit 1
