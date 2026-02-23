# Programming Game RaceHub VSCode Extension

Integrated workflow for RaceHub onboarding, bot bootstrap, build/upload, and artifact management.

## Commands

- `RaceHub: Configure Server URL`
- `RaceHub: Login` (webview form)
- `RaceHub: Initialize Bot Project`
- `RaceHub: Open Bot Project`

## Server URL Profiles

The extension no longer uses a raw `serverUrl` setting.

- `production` (default): `https://racers.mlkr.eu`
- `localhost`: `http://127.0.0.1:8787`
- `custom`: uses `racehub.customServerUrl`

Use `RaceHub: Configure Server URL` to switch profiles.

## Explorer View States

The `RaceHub` tree appears inside the built-in Explorer sidebar and has three explicit states:

- `loggedOut`
  - Shows login and server configuration actions.
- `needsWorkspace`
  - Shows initialize/open bot project actions.
- `ready`
  - Shows `Local Binaries` and `Remote Artifacts`.

## Explorer Workflow (`ready` state)

- `Local Binaries`
  - Discovers binaries from `Cargo.toml` (`[[bin]]`, including explicit `path`) and `src/bin/*.rs`.
  - Inline icon actions: `Build & Upload`, `Build Binary`, `Reveal ELF Path`.
- `Remote Artifacts`
  - Lists artifacts from `GET /api/v1/artifacts`.
  - Owned artifacts inline icon actions: `Replace`, `Toggle Visibility`, `Delete`.
  - The same owned-artifact actions are also available in the right-click menu.

Replace semantics:
- Upload new build first.
- Delete old artifact after upload (best effort).
- If delete fails, new artifact is kept.

## Bootstrap Template Parity

The starter template intentionally mirrors bot MMIO interfaces exactly:

- `src/lib.rs` == `bot/src/lib.rs`
- `src/log.rs` == `bot/src/log.rs`
- `src/driving.rs` == `bot/src/driving.rs`

Template files include:
- `Cargo.toml`
- `.cargo/config.toml`
- `link.x`
- `src/lib.rs`
- `src/log.rs`
- `src/driving.rs`
- `src/bin/car.rs`

## Auth Behavior

- If server reports `auth_required=true`, the extension requires webview login and uses bearer token auth.
- If server reports `auth_required=false` (standalone mode), artifact operations work without login.
