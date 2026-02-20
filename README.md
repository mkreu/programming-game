# Programming Game

Rust/Bevy racing prototype where cars run RISC-V ELF artifacts fetched from RaceHub.

## Run Modes

### 1) Standalone mode (single process, no auth)
Run game + embedded RaceHub in one binary:

```bash
cargo run --bin racing -- --standalone
```

Behavior:
- Game starts embedded RaceHub on `http://127.0.0.1:8787` (override with `RACEHUB_STANDALONE_BIND`).
- Auth is disabled.
- Game and VSCode extension can upload/list/download artifacts without login.

### 2) Server mode (separate backend, auth enabled)
Start backend:

```bash
cargo run -p racehub
```

Start game in another terminal:

```bash
cargo run --bin racing
```

Behavior:
- Backend default URL: `http://127.0.0.1:8787` (override via `RACEHUB_BIND`).
- Backend auth mode defaults to required.
- Native game asks for username/password in terminal on startup.
- Web game uses browser session cookies (log in through backend HTTP API / website flow).
- VSCode extension uses bearer token login.

## Backend Environment Variables

- `RACEHUB_BIND` (default `127.0.0.1:8787`)
- `RACEHUB_DB_PATH` (default `racehub.db`)
- `RACEHUB_ARTIFACTS_DIR` (default `racehub_artifacts`)
- `RACEHUB_AUTH_MODE` (`required` or `disabled`, default `required`)
- `RACEHUB_COOKIE_SECURE` (`true/false`, default `false`)
- `RACEHUB_STATIC_DIR` (default `web-dist`, set empty to disable static serving)

For standalone backend without game:

```bash
RACEHUB_AUTH_MODE=disabled cargo run -p racehub
```

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
- `web-dist/racing.js` + `web-dist/racing_bg.wasm`
- `web-dist/assets/` copied from `racing/assets/`

Serve web app (same origin as API, so cookie auth works):

```bash
./scripts/serve_web.sh
```

Useful options:
- `./scripts/serve_web.sh --release` builds release wasm before serving.
- `NO_BUILD=1 ./scripts/serve_web.sh` serves existing `web-dist/` without rebuilding.
- `RACEHUB_AUTH_MODE=disabled ./scripts/serve_web.sh` serves in standalone/no-auth backend mode.

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
- `RaceHub: Configure Server URL`
- `RaceHub: Login`
- `RaceHub: Upload Artifact File`

Auth behavior:
- Server mode (`auth_required=true`): extension requires login and stores bearer token in VSCode secret storage.
- Standalone mode (`auth_required=false`): extension uploads without login.
