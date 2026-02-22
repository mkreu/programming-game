# syntax=docker/dockerfile:1.7

FROM rust:1-bookworm AS chef
WORKDIR /app

RUN apt-get update \
    && apt-get install -y --no-install-recommends binaryen ca-certificates \
    && rm -rf /var/lib/apt/lists/*
RUN cargo install --locked cargo-chef wasm-bindgen-cli
RUN rustup target add wasm32-unknown-unknown

FROM chef AS planner
WORKDIR /app
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
WORKDIR /app
ARG ENABLE_WASM_OPT=1
COPY --from=planner /app/recipe.json recipe.json

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/app/target \
    cargo chef cook --release --recipe-path recipe.json -p racehub

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/app/target \
    cargo chef cook --release --recipe-path recipe.json \
    -p racing --bin racing --target wasm32-unknown-unknown

COPY . .

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/app/target \
    cargo build -p racehub --release --locked \
    && cp /app/target/release/racehub /app/racehub

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/app/target \
    ./scripts/build_web.sh --release \
    && if [ "$ENABLE_WASM_OPT" = "1" ] && command -v wasm-opt >/dev/null 2>&1; then \
      echo "Running wasm-opt -Oz on web-dist/racing_bg.wasm..."; \
      wasm-opt -Oz /app/web-dist/racing_bg.wasm -o /app/web-dist/racing_bg.wasm; \
      echo "wasm-opt completed."; \
    else \
      echo "Skipping wasm-opt (ENABLE_WASM_OPT=$ENABLE_WASM_OPT)."; \
    fi

FROM debian:bookworm-slim AS runtime
WORKDIR /app

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates wget \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --gid 10001 racehub \
    && useradd --uid 10001 --gid racehub --home /app --shell /usr/sbin/nologin racehub \
    && mkdir -p /data/racehub_artifacts /opt/racehub \
    && chown -R racehub:racehub /data /opt/racehub

COPY --from=builder /app/racehub /usr/local/bin/racehub
COPY --from=builder /app/web-dist /opt/racehub/web-dist

ENV RACEHUB_BIND=0.0.0.0:8787
ENV RACEHUB_DB_PATH=/data/racehub.db
ENV RACEHUB_ARTIFACTS_DIR=/data/racehub_artifacts
ENV RACEHUB_STATIC_DIR=/opt/racehub/web-dist

EXPOSE 8787

HEALTHCHECK --interval=30s --timeout=3s --start-period=10s --retries=3 \
  CMD wget -qO- http://127.0.0.1:8787/healthz || exit 1

USER racehub
ENTRYPOINT ["/usr/local/bin/racehub"]
