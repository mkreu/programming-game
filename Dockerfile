# syntax=docker/dockerfile:1.7

FROM rust:1-bookworm AS chef
WORKDIR /app

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
RUN cargo install --locked cargo-chef wasm-bindgen-cli
RUN rustup target add wasm32-unknown-unknown

FROM chef AS planner
WORKDIR /app
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
WORKDIR /app
COPY --from=planner /app/recipe.json recipe.json

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/app/target \
    cargo chef cook --release --recipe-path recipe.json -p botracers-server

COPY . .
RUN mkdir -p /out

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/app/target \
    cargo build -p botracers-server --release --locked \
    && ./scripts/build_web.sh --release \
    && cp /app/target/release/botracers-server /out/botracers-server

FROM debian:bookworm-slim AS runtime
WORKDIR /app

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --gid 10001 botracers \
    && useradd --uid 10001 --gid botracers --home /app --shell /usr/sbin/nologin botracers \
    && mkdir -p /data/botracers_artifacts /opt/botracers \
    && chown -R botracers:botracers /data /opt/botracers

COPY --from=builder /out/botracers-server /usr/local/bin/botracers-server
COPY --from=builder /app/web-dist /opt/botracers/web-dist

ENV BOTRACERS_BIND=0.0.0.0:8787
ENV BOTRACERS_DB_PATH=/data/botracers.db
ENV BOTRACERS_ARTIFACTS_DIR=/data/botracers_artifacts
ENV BOTRACERS_STATIC_DIR=/opt/botracers/web-dist
ENV RUST_LOG=info

VOLUME ["/data"]

EXPOSE 8787

USER botracers
ENTRYPOINT ["/usr/local/bin/botracers-server"]
