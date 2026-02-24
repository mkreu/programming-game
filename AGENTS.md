# AGENTS.md — AI Agent Guide

## Project Overview

A racing game prototype where cars can be driven by AI implemented as **RISC-V programs** running inside an emulator. The goal is to let users write bare-metal RISC-V code (in Rust, `no_std`) that controls a car via memory-mapped I/O, while the game handles physics, rendering, and track management.

## Workspace Structure

```
Cargo.toml            # Workspace root — members: botracers-game, emulator, botracers-protocol, botracers-server, botracers-bot-sdk. Excludes bot.
Dockerfile            # Multi-stage production image build (botracers-server binary + web-dist wasm build)
/.github/workflows/   # CI workflows, including GHCR container publish and VSCode .vsix artifact build
├── botracers-game/           # Bevy game — physics, rendering, car spawning, AI systems
├── emulator/         # Use-case-agnostic RISC-V emulator (RV32IMAFC + RV32C/Zcf) with Bevy integration
├── botracers-protocol/    # Shared API DTOs used by backend/client/game/extension
├── botracers-server/   # Minimal backend (auth + artifact storage/list/download/delete)
├── botracers-bot-sdk/  # Shared no_std bot SDK (MMIO bindings, log device, optional panic+allocator runtime)
├── vscode-extension/ # VSCode extension for bot bootstrap + build/upload + artifact management (not a Cargo crate)
└── bot/              # no_std RISC-V programs compiled to bare-metal ELF (separate target)
```

**Important**: `bot/` is excluded from the workspace because it cross-compiles to `riscv32imafc-unknown-none-elf`.

**Important**: Always keep this file up to date when changing code.

## Build & Run

```bash
# Run the racing game (from workspace root)
cargo run --bin botracers [-- path/to/track.toml]
# Default track: botracers-game/assets/track1.toml

# Run BotRacers in standalone mode (embedded botracers-server, auth disabled)
cargo run --bin botracers -- --standalone

# Run the single-node backend (default bind: 127.0.0.1:8787)
cargo run -p botracers-server

# Build web artifacts into web-dist/
./scripts/build_web.sh [--release]

# Build + serve web app through botracers-server
./scripts/serve_web.sh [--release]

# Build production container (botracers-server + web-dist)
docker build -t botracers:test .

# Run container with persistent data volume
docker run --rm -p 8787:8787 -v botracers-data:/data ghcr.io/<owner>/botracers:latest
```

Cars are spawned from ELF artifacts fetched from `botracers-server` (or uploaded manually in-game). Local runtime bot compilation/discovery is intentionally removed from `botracers-game`.

## Crate Details

### `emulator/` — RISC-V Emulator

**Must remain use-case agnostic.** No car/racing-specific code belongs here.

- **`cpu.rs`** — Core emulator: `Hart` (32 GPRs, 32 FPRs, PC, LR/SC reservation), `Dram` (ELF-backed memory with stack headroom), `Mmu` (routes memory accesses to DRAM or devices), `LogDevice` (buffered char output with `drain_output()` and `output()` methods)
- **`bevy.rs`** — `CpuComponent` holds only CPU core state (`Hart`, `Dram`, instruction budget). MMIO devices are first-class Bevy components on the same entity. Slot mapping is provided by consumer-defined `CpuConfig` (`slot -> device component`) and consumed by generic `cpu_system::<Config>`. Use `CpuComponent::new(elf, instructions_per_update)` to create and register `cpu_system::<YourCpuConfig>` in `FixedUpdate`. For less boilerplate, use `emulator::define_cpu_config!`.
- **`lib.rs`** — `CpuBuilder` helper

**`Device` trait** (`cpu.rs`) — The memory interface for devices:
```rust
pub trait Device: Send + Sync {
    fn load(&self, addr: u32, size: u32) -> Result<u32, ()>;
    fn store(&mut self, addr: u32, size: u32, value: u32) -> Result<(), ()>;
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}
```

**Device address mapping** (handled by `Mmu`):
| Address Range   | Device Index | Typical Use     |
|-----------------|-------------|-----------------|
| `0x000–0x0FF`   | (error)     | Reserved        |
| `0x100–0x1FF`   | 0           | LogDevice       |
| `0x200–0x2FF`   | 1           | CarStateDevice  |
| `0x300–0x3FF`   | 2           | CarControlsDevice |
| `0x400–0x4FF`   | 3           | SplineDevice    |
| `0x500–0x5FF`   | 4           | TrackRadarDevice |
| `0x600–0x6FF`   | 5           | CarRadarDevice  |
| `≥ 0x1000`      | —           | DRAM            |

Devices receive **offset-relative addresses** (i.e., `addr & 0xFF`), not absolute addresses.

**Device ECS access** — Query device components directly from Bevy systems (e.g. `Query<(&mut CarStateDevice, &mut TrackRadarDevice)>`). Do not store devices in `CpuComponent`.

### `bot/` — RISC-V Bot Programs

- Target: `riscv32imafc-unknown-none-elf` (configured in `bot/.cargo/config.toml`)
- Linker script `link.x` places `.text` at `0x1000` (start of DRAM)
- Depends on `botracers-bot-sdk` for slot constants, MMIO bindings (`CarState`, `CarControls`, `SplineQuery`, `TrackRadar`, `CarRadar`), log writer, and default runtime (`panic-handler` + `global-allocator` features)
- `.cargo/config.toml` and local `link.x` stay in each bot repo; target/linker wiring is crate-local on stable Rust
- `bin/car.rs` — The car AI: infinite loop reading state, querying spline, computing steering/braking, writing controls
- `bin/car_radar.rs` — Radar-only car AI using `TrackRadar` (no spline-following dependency)
- `bin/bottles.rs` — Test program (99 bottles of beer via log device)

### `botracers-bot-sdk/` — Shared Bot Runtime + MMIO API

- `no_std` crate used by local `bot/` and VSCode-initialized bot repos
- Exposes `pub mod driving`, `pub mod log`, slot constants (`SLOT1..SLOT6`), and `log()`
- Feature flags:
  - `panic-handler` — provides a default panic handler that logs panic text to slot `0x100`
  - `global-allocator` — installs bump allocator as `#[global_allocator]`
  - `allocator-4k` — heap size profile for `global-allocator` (default 4 KiB)
- Consumers can disable runtime features to provide custom panic/allocator implementations

**CarState layout** (SLOT2, 0x200, read by bot):
| Offset | Field       | Type |
|--------|-------------|------|
| 0x00   | speed       | f32  |
| 0x04   | position_x  | f32  |
| 0x08   | position_y  | f32  |
| 0x0C   | forward_x   | f32  |
| 0x10   | forward_y   | f32  |

**CarControls layout** (SLOT3, 0x300, written by bot):
| Offset | Field       | Type |
|--------|-------------|------|
| 0x00   | accelerator | f32  |
| 0x04   | brake       | f32  |
| 0x08   | steering    | f32  |

**SplineQuery layout** (SLOT4, 0x400, read/write by bot):
| Offset | Field       | Type | Access |
|--------|-------------|------|--------|
| 0x00   | t           | f32  | write  |
| 0x04   | x           | f32  | read   |
| 0x08   | y           | f32  | read   |
| 0x0C   | t_max       | f32  | read   |

**SplineQuery protocol**: Bot writes a `t` parameter (spline position) to offset 0x00, device evaluates the spline at that point, then bot reads the resulting x/y coordinates from offsets 0x04/0x08. The `t_max` value (domain end) is read-only.

**TrackRadar layout** (SLOT5, 0x500, read by bot):
| Offset | Field            | Type |
|--------|------------------|------|
| 0x00   | ray_0_distance   | f32  |
| 0x04   | ray_1_distance   | f32  |
| 0x08   | ray_2_distance   | f32  |
| 0x0C   | ray_3_distance   | f32  |
| 0x10   | ray_4_distance   | f32  |
| 0x14   | ray_5_distance   | f32  |
| 0x18   | ray_6_distance   | f32  |

Rays are cast in a forward cone (currently 7 rays over 90°). Distances are nearest border-hit distances in world units; no-hit rays are encoded as `NaN`.

**CarRadar layout** (SLOT6, 0x600, read by bot):
| Offset | Field   | Type |
|--------|---------|------|
| 0x00   | car0_x  | f32  |
| 0x04   | car0_y  | f32  |
| 0x08   | car1_x  | f32  |
| 0x0C   | car1_y  | f32  |
| 0x10   | car2_x  | f32  |
| 0x14   | car2_y  | f32  |
| 0x18   | car3_x  | f32  |
| 0x1C   | car3_y  | f32  |

Entries are absolute world positions of nearest cars, strictly nearest-first and excluding self. Missing entries are encoded as `NaN` pairs.

### `botracers-protocol/` — Shared API Types

- Shared request/response DTOs for backend/client/game/extension.
- Defines minimal v1 payloads for auth, capabilities, artifact metadata (including owner username, visibility, and ownership flags), artifact visibility updates, and artifact upload.
- Keep this crate transport-agnostic and serde-only.

### `botracers-server/` — Single-Executable Backend

- One Axum HTTP process with SQLite (`BOTRACERS_DB_PATH`, default `botracers.db`) and filesystem artifact store (`BOTRACERS_ARTIFACTS_DIR`, default `botracers_artifacts/`).
- Browser web routes:
  - `GET /` and `GET /index.html` serve the web game entry.
  - In `required` auth mode, unauthenticated access to `/` or `/index.html` renders a login page first.
  - `GET /login` serves login form HTML and `POST /login` authenticates then redirects back to `next` (default `/`).
  - `GET /register` serves registration form HTML and `POST /register` creates an account and logs in, then redirects back to `next` (default `/`).
  - `BOTRACERS_REGISTRATION_ENABLED=false` disables registration (API and web flow).
- API endpoints:
  - `GET /api/v1/capabilities`
  - `POST /api/v1/auth/register`
  - `POST /api/v1/auth/login`
  - `POST /api/v1/auth/logout`
  - `GET /api/v1/me`
  - `GET /api/v1/artifacts`
  - `POST /api/v1/artifacts`
  - `GET /api/v1/artifacts/{id}`
  - `DELETE /api/v1/artifacts/{id}`
  - `PATCH /api/v1/artifacts/{id}/visibility`
- Artifact visibility model:
  - uploads are private by default
  - in `required` auth mode, list/download access includes own artifacts plus other users' public artifacts
  - only owners can delete or change visibility
- Uses session tokens stored in SQLite and accepts either:
  - `Authorization: Bearer <token>` (VSCode extension / native clients)
  - `botracers_session` cookie (browser/web game flow)
- Supports auth modes via `BOTRACERS_AUTH_MODE`:
  - `required` (normal server mode)
  - `disabled` (standalone mode, implicit local user)
- `BOTRACERS_COOKIE_SECURE` controls whether the session cookie is marked `Secure`.
- `BOTRACERS_REGISTRATION_ENABLED` controls whether account registration endpoints/UI are enabled (default `true`).
- `BOTRACERS_STATIC_DIR` controls which static directory is served (default `web-dist`; empty disables static serving).
- Server uses graceful shutdown on process signals (`SIGINT`/`SIGTERM` on Unix, `Ctrl-C` elsewhere).
- `botracers-server` emits concise tracing logs for startup/shutdown, static serving mode, login failures, and artifact upload/delete actions.
- Backend scope is intentionally minimal: auth + artifact storage/list/download/delete.
- Production container image is built by the root `Dockerfile` and includes:
  - release `botracers-server` binary
  - release wasm game bundle in `/opt/botracers/web-dist`
  - `VOLUME /data` for persisted SQLite + artifact storage mounts
  - OCI-first metadata: no Dockerfile `HEALTHCHECK` directive (use `/healthz` endpoint for probes)
  - no `wasm-opt` step in container builds; web artifacts are produced by `./scripts/build_web.sh --release`
  - defaults: `BOTRACERS_BIND=0.0.0.0:8787`, `BOTRACERS_DB_PATH=/data/botracers.db`, `BOTRACERS_ARTIFACTS_DIR=/data/botracers_artifacts`, `BOTRACERS_STATIC_DIR=/opt/botracers/web-dist`
- GHCR publish workflow: `.github/workflows/publish-botracers-image.yml`
  - triggers on pushes to `main` and tags `v*`
  - publishes `linux/amd64` images to `ghcr.io/<owner>/botracers`
  - uses Docker BuildKit `type=gha` cache and `cargo-chef` dependency-layer caching
- VSCode extension build workflow: `.github/workflows/build-vscode-extension.yml`
  - triggers on pushes and pull requests to `main` plus manual dispatch
  - compiles `vscode-extension`, packages a `.vsix`, uploads it as a workflow artifact
  - no marketplace publish step (artifact-only distribution)

### `vscode-extension/` — Bot Workflow + Artifact Connector

- TypeScript VSCode extension.
- Commands:
  - `BotRacers: Configure Server URL`
  - `BotRacers: Login` (webview form)
  - `BotRacers: Initialize Bot Project`
  - `BotRacers: Open Bot Project`
- Server URL is profile-only (no raw server URL setting):
  - `production` -> `https://botrace.rs` (default)
  - `localhost` -> `http://127.0.0.1:8787`
  - `custom` -> `botracers.customServerUrl`
- `BotRacers` tree view is contributed directly to the built-in Explorer sidebar with explicit states:
  - `loggedOut`: rendered through VS Code Welcome View content (login/server actions with context-specific variants like session expiry or request errors)
  - `needsWorkspace`: rendered through VS Code Welcome View content (initialize/open actions with context-specific variants like missing workspace or no binaries)
  - `ready`: local binaries + remote artifacts
- Action density policy:
  - BotRacers tree inline icon actions for local binaries: `Build & Upload`, `Build`, `Reveal ELF Path`
  - BotRacers tree inline icon actions on owned artifacts: `Replace`, `Toggle Visibility`, `Delete`
  - the same owned-artifact actions are also available in the context menu
- Local bin discovery uses `Cargo.toml` (`[[bin]]` including optional `path`) and `src/bin/*.rs`.
- Bootstrap template assets: `vscode-extension/templates/bot-starter/` (`Cargo.toml`, `.cargo/config.toml`, `link.x`, `src/bin/car.rs`)
- Starter template imports `botracers-bot-sdk` from git (`branch = "main"`) and relies on SDK defaults for panic handler + allocator.
- Template rule: keep local linker/target files minimal (`.cargo/config.toml`, `link.x`) and treat `botracers-bot-sdk` as the source of truth for bot MMIO/log/runtime helpers.
- Replacement semantics are best-effort cleanup: upload new artifact first, then delete selected old artifact if owned.
- Detects server capabilities and skips auth flow automatically when `auth_required=false`.

### `botracers-game/` — The Game

- **`main.rs`** — Bevy app setup, game state management (`SimState`), event-based car spawning, physics, free camera with follow-on-select, two AI systems
- **`ui.rs`** — `RaceUiPlugin`: right-side panel with persistent server status dialog, refresh/upload artifact buttons, artifact list with owner + visibility labels and per-artifact actions (spawn for all visible artifacts, delete/visibility toggle for owned artifacts), start/pause/reset, spawned-car controls (follow/gizmos/remove), and console output.
- **`devices.rs`** — `CarStateDevice`, `CarControlsDevice`, `SplineDevice`, `TrackRadarDevice`, and `CarRadarDevice` implementing `Device` (host-side counterparts to the bot's volatile pointers and their uptate systems for bevy logic)
- **`track.rs`** — `TrackSpline` resource, spline construction, track/kerb mesh generation
- **`track_format.rs`** — TOML-based track file format (`TrackFile`)
- **`bin/editor.rs`** — Track editor tool
- Web API integration in `main.rs`/`ui.rs` now supports:
  - capability checks against `botracers-server`
  - native CLI credential prompt (non-wasm) and login when required
  - browser-cookie-based auth for wasm/web builds (no in-game login fields)
  - same-origin API URL default in wasm/web builds (relative `/api/...` requests) to avoid cookie loss across hostname mismatches
  - wasm canvas autosizing via `Window.fit_canvas_to_parent = true` (fills and tracks browser viewport with matching `index.html` CSS)
  - loading artifact lists
  - manual artifact upload from file chooser (native + web)
  - deleting artifacts from BotRacers storage
  - toggling artifact visibility (`public`/`private`) for owned artifacts
  - spawning cars directly from artifact list rows (`DriverType::RemoteArtifact`) by downloading ELF via HTTP

**Key components:**
- `Car` — steering, accelerator, brake state (used by physics)
- `EmulatorDriver` — marker component for RISC-V-emulator-driven cars
- `CpuComponent` (from emulator crate) — attached to emulator-driven cars
- `LogDevice`, `CarStateDevice`, `CarControlsDevice`, `SplineDevice`, `TrackRadarDevice`, `CarRadarDevice` — MMIO device components attached to emulator-driven cars
- `CarLabel` — name label for each car
- `DebugGizmos` — marker; when present on a car, debug gizmos are drawn (off by default)
- `FrontWheel` — visual wheel rotation marker

**Key resources:**
- `RaceManager` — tracks all spawned cars (`Vec<CarEntry>`), next car ID, and per-car console output
- `FollowCar` — optional entity to follow with the camera
- `SimState` — state machine: `PreRace` (add/remove cars) → `Racing` (simulation active) → `Paused` (toggle)

**Key messages (Bevy 0.18 `Message` trait, not `Event`):**
- `SpawnCarRequest { driver: DriverType }` — sent by artifact-row "Spawn" button, consumed by `handle_spawn_car_event`

**System execution order:**
1. `Update`: `handle_car_input` (keyboard → `Car`, excludes AI/emulator cars)
2. `FixedUpdate` (in order, only in `Racing` state):
    - `update_car_state_device` — writes physics state (position, velocity, forward direction) into `CarStateDevice` (**before** CPU execution system)
    - `update_track_radar_device` — updates `TrackRadarDevice` border ray distances (**before** CPU execution system)
    - `update_car_radar_device` — updates `CarRadarDevice` nearest-car absolute positions (**before** CPU execution system)
    - CPU execution system (`cpu_system::<YourCpuConfig>`) — runs N RISC-V instructions per tick; bot queries `SplineDevice` and computes controls
   - `apply_emulator_controls` — reads `CarControlsDevice` → `Car` (**after** CPU execution system)
   - `apply_car_forces` — applies `Car` state to physics forces
3. `Update` (always): UI systems (car list rebuild, button handlers, console output drain), `update_camera`, `draw_gizmos`, `update_fps_counter`

**Car spawning** — Event-driven via `SpawnCarRequest` message. The artifact list UI sends a `SpawnCarRequest` with a `DriverType`; `handle_spawn_car_event` creates the car entity with staggered grid positioning. Cars can only be added/removed in `PreRace` state. Each emulator car gets its own isolated CPU (`CpuComponent`) and isolated MMIO device components; each car has its own `SplineDevice` with a cloned copy of the track spline.

**Camera** — Free camera by default (no cars spawned at startup). Middle/right-mouse drag to pan, scroll to zoom. When a car is selected via the UI "follow" button, the camera snaps to it; clicking again unfollows.

**Physics model** — Bicycle-ish 4-wheel model: acceleration/braking along forward vector, lateral grip forces per wheel computed from slip angle. Uses `avian2d` for rigid body simulation. Fixed timestep at 200 Hz.

## Key Architectural Decisions

0. **No compatibility stubs unless requested** — When refactoring APIs, do not keep unused backward-compatibility code paths, deprecated wrappers, or dead shim layers unless explicitly requested. Prefer deleting old forms and updating all call sites.

1. **Emulator is use-case agnostic** — Car-specific devices (`CarStateDevice`, `CarControlsDevice`, `SplineDevice`) live in `botracers-game/`, not in `emulator/`. The emulator only provides `Device`, `Mmu`, `LogDevice` (buffered), `CpuComponent`, and the plugin.

2. **Each emulator car is fully isolated** — Separate `Hart`, `Dram`, and device-component instances per car entity. No shared state between emulator instances. Each car has its own `SplineDevice` with a cloned copy of the track spline.

3. **Device addressing** — The `Mmu` strips the high bits and passes offset-relative addresses (`addr & 0xFF`) to devices. Devices don't need to know their absolute slot address.

4. **Driver source is artifact-only** — Cars are spawned from artifacts served by `botracers-server`; local runtime bot compilation/discovery is intentionally removed from `botracers-game`.

5. **Instruction budget matters** — The `instructions_per_update` value (currently 10000) must be high enough for each bot loop iteration to make progress, but low enough to avoid burning host CPU.

6. **Spline logic is bot-side** — The bot implements full autonomous navigation (window search, dynamic lookahead, spline walking, curvature-based braking) using the `SplineDevice` query interface. The engine only provides basic physics state; all pathfinding intelligence runs in emulated RISC-V code.

7. **Strict compressed decode** — Compressed instruction decode is intentionally strict RV32C(+Zcf). Illegal encodings must trap/panic; do not add permissive fallbacks.

8. **Stack/DRAM alignment invariants** — DRAM allocation is rounded to 16-byte alignment with explicit stack headroom, and `sp` is set to a 16-byte aligned top-of-memory minus 16. Keep this when changing loader/builder code.

9. **Pending artifact downloads are state-gated** — If an artifact download finishes after leaving `PreRace`, the result is discarded and no car is spawned.

## Common Pitfalls

- **Web artifact flow may require auth** — In server mode: native uses bearer token after CLI login, web uses browser session cookie. In standalone mode auth is disabled.
- **Embedded standalone startup race** — Initial capability fetch can fail if embedded `botracers-server` has not yet bound; retry from the UI.
- **Device index vs slot address** — Device index 0 = address 0x100, index 1 = 0x200, etc. Off-by-one errors here will silently read zeros or fail.
- **Mmu passes offsets, not absolute addresses** — If you implement a new device, your `load`/`store` will receive `addr & 0xFF`, not the full address.
- **`instructions_per_update` tuning** — Too low and the bot can't complete a loop iteration per tick. Too high and it burns CPU time.
- **Bump allocator in SDK defaults** — `botracers-bot-sdk` default features provide a 4 KiB bump allocator that never frees. Allocating in a loop will eventually OOM. Current bot code doesn't allocate in its hot loop, but be careful adding features that do.
- **Compressed immediates are easy to misdecode** — For `C.ADDI/C.LI/C.LUI/C.ANDI`, immediate sign comes from `inst[12]` mapped to imm bit 5. Missing that sign bit causes silent control-flow/data corruption.
