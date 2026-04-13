# Stage 1: Build WASM from Rust
FROM rust:latest AS wasm-build

RUN curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY skyjo-core/ skyjo-core/
COPY skyjo-wasm/ skyjo-wasm/

RUN cd skyjo-wasm && wasm-pack build --release --target web --out-dir ../frontend/pkg

# Stage 2: Build frontend
FROM node:22-alpine AS frontend-build

ENV PNPM_HOME="/pnpm"
ENV PATH="$PNPM_HOME:$PATH"
RUN corepack enable

WORKDIR /app
COPY frontend/package.json frontend/pnpm-lock.yaml ./
RUN pnpm install --frozen-lockfile

COPY frontend/ .
COPY --from=wasm-build /app/frontend/pkg/ pkg/
RUN pnpm build

# Stage 3: Serve with nginx
FROM nginx:alpine

COPY --from=frontend-build /app/dist/ /usr/share/nginx/html/
COPY frontend/nginx.conf /etc/nginx/conf.d/default.conf

EXPOSE 80
