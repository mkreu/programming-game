# BotRacers

Rust/Bevy racing prototype where cars run RISC-V ELF artifacts fetched from BotRacers.

## Player Onboarding (2 Steps)

1. Register on the BotRacers site (`/register`) when running in server mode (`BOTRACERS_AUTH_MODE=required`).
2. Install the VSCode extension from a packaged `.vsix` artifact (manual install for now).

Recommended workflow after onboarding:
- Run `BotRacers: Initialize Bot Project` once to scaffold a starter `bot` project.
- Use the BotRacers sidebar to build/upload local binaries and manage uploaded artifacts.
- Starter projects keep only minimal local toolchain files (`.cargo/config.toml`, `link.x`, `src/bin/car.rs`) and import bot MMIO/runtime helpers from `botracers-bot-sdk`.

## Run Modes

### 1) Standalone mode (single process, no auth)
Run game + embedded BotRacers in one binary:

```bash
cargo run --bin botracers -- --standalone
```

Behavior:
- Game starts embedded BotRacers on `http://127.0.0.1:8787` (override with `BOTRACERS_STANDALONE_BIND`).
- Auth is disabled.
- Game and VSCode extension can upload/list/download/delete artifacts without login.

### 2) Server mode (separate backend, auth enabled)
Start backend:

```bash
cargo run -p botracers-server
```

Start game in another terminal:

```bash
cargo run --bin botracers
```

Behavior:
- Backend default URL: `http://127.0.0.1:8787` (override via `BOTRACERS_BIND`).
- Backend auth mode defaults to required.
- Native game asks for username/password in terminal on startup.
- Web game uses browser session cookies.
  - Browser/wasm API requests default to same-origin paths (`/api/...`) to keep cookie auth working even when hostnames differ (`localhost` vs `127.0.0.1`).
  - If not logged in, visiting `/` or `/index.html` shows a BotRacers login form.
  - If registration is enabled, login page includes a "Create account" link to `/register`.
  - Successful login redirects back to the requested game page.
- VSCode extension uses bearer token login.

## Backend Environment Variables

- `BOTRACERS_BIND` (default `127.0.0.1:8787`)
- `BOTRACERS_DB_PATH` (default `botracers.db`)
- `BOTRACERS_ARTIFACTS_DIR` (default `botracers_artifacts`)
- `BOTRACERS_AUTH_MODE` (`required` or `disabled`, default `required`)
- `BOTRACERS_COOKIE_SECURE` (`true/false`, default `false`)
- `BOTRACERS_REGISTRATION_ENABLED` (`true/false`, default `true`)
- `BOTRACERS_STATIC_DIR` (default `web-dist`, set empty to disable static serving)

For standalone backend without game:

```bash
BOTRACERS_AUTH_MODE=disabled cargo run -p botracers-server
```

## Docker (BotRacers + Web Game)

Build the production image locally:

```bash
docker build -t botracers:test .
```

Run it with persisted data:

```bash
docker run --rm -p 8787:8787 -v botracers-data:/data ghcr.io/<owner>/botracers:latest
```

The image declares `VOLUME /data`, which is the persistence mount point for both SQLite and uploaded artifacts. Without mounting `/data`, data is ephemeral.

Container defaults:
- `BOTRACERS_BIND=0.0.0.0:8787`
- `BOTRACERS_DB_PATH=/data/botracers.db`
- `BOTRACERS_ARTIFACTS_DIR=/data/botracers_artifacts`
- `BOTRACERS_STATIC_DIR=/opt/botracers/web-dist`

Container image notes:
- OCI-first image: Dockerfile intentionally omits Docker `HEALTHCHECK` metadata to avoid Podman OCI warnings.
- Probe liveness/readiness via `GET /healthz`.
- Container build uses `./scripts/build_web.sh --release` directly and does not run `wasm-opt`.

Quick checks:
- `GET /healthz` returns `ok`
- `GET /index.html` serves the wasm game
- `GET /api/v1/capabilities` serves the backend API

## GHCR Publish Workflow

GitHub Actions workflow: `.github/workflows/publish-botracers-image.yml`

Behavior:
- Pushes container images to `ghcr.io/<owner>/botracers`
- Triggers on:
  - pushes to `main`
  - pushes of tags matching `v*`
- Uses:
  - multi-stage Docker build
  - `cargo-chef` dependency-layer caching
  - BuildKit GitHub Actions cache (`type=gha`)
- Publishes `linux/amd64` images with tags for `latest` (default branch), branch/tag refs, and commit SHA.

## VSCode Extension Build Workflow

GitHub Actions workflow: `.github/workflows/build-vscode-extension.yml`

Behavior:
- Builds and packages the VSCode extension on pushes/PRs to `main` (and manual dispatch).
- Uploads a `.vsix` file as a workflow artifact for manual installation.

## Web Build

Install target and wasm bindgen CLI once:

```bash
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli
```

Build web app:

```bash
./scripts/build_web.sh
```

Release build:

```bash
./scripts/build_web.sh --release
```

What this produces:
- `web-dist/index.html` (generated if missing)
- `web-dist/botracers.js` + `web-dist/botracers_bg.wasm`
- `web-dist/assets/` copied from `botracers-game/assets/`
- Web canvas is configured to fill and dynamically resize with the browser viewport.

Serve web app (same origin as API, so cookie auth works, or optionally with `BOTRACERS_AUTH_MODE=disabled`):

```bash
cargo run -p botracers-server
```

## VSCode Extension (`vscode-extension/`)

Install dependencies and compile:

```bash
cd vscode-extension
npm install
npm run compile
```

Run/debug in VSCode:
- Open `vscode-extension/` in VSCode.
- Press `F5` to launch Extension Development Host.

Available commands:
- `BotRacers: Configure Server URL`
- `BotRacers: Login`
- `BotRacers: Initialize Bot Project`
- `BotRacers: Open Bot Project`

Sidebar (`BotRacers` activity bar):
- `loggedOut`: shows login actions only.
- `needsWorkspace`: shows initialize/open bot project actions only.
- `ready`: shows `Local Binaries` and `Remote Artifacts`.
- Inline actions are minimal (`Build & Upload` for local binaries, `Replace` for owned artifacts). Secondary actions are in context menus.

Auth behavior:
- Server mode (`auth_required=true`): extension uses a custom webview login form and stores bearer token in VSCode secret storage.
- Standalone mode (`auth_required=false`): build/upload/manage works without login.

Server URL behavior:
- Profile-based configuration only:
  - `production` -> `https://botrace.rs` (default)
  - `localhost` -> `http://127.0.0.1:8787`
  - `custom` -> `botracers.customServerUrl`

Replace behavior:
- “Replace artifact” uploads a new build first, then attempts to delete the selected old artifact (best-effort cleanup; no rollback if delete fails).
