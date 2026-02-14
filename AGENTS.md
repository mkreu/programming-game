# AGENTS.md — AI Agent Guide

## Project Overview

A racing game prototype where cars can be driven by AI implemented as **RISC-V programs** running inside an emulator. The goal is to let users write bare-metal RISC-V code (in Rust, `no_std`) that controls a car via memory-mapped I/O, while the game handles physics, rendering, and track management.

## Workspace Structure

```
Cargo.toml            # Workspace root — members: racing, emulator. Excludes bot.
├── racing/           # Bevy game — physics, rendering, car spawning, AI systems
├── emulator/         # Use-case-agnostic RISC-V emulator (RV32IMAFC + RV32C/Zcf) with Bevy integration
└── bot/              # no_std RISC-V programs compiled to bare-metal ELF (separate target)
```

**Important**: `bot/` is excluded from the workspace because it cross-compiles to `riscv32imafc-unknown-none-elf`.

**Important**: Always keep this file up to date when changing code.

## Build & Run

```bash
# Run the racing game (from workspace root)
cargo run --bin racing [-- path/to/track.toml]
# Default track: racing/assets/track1.toml
```

Bot binaries are now discovered at runtime via `cargo metadata` in `bot/`. When adding a bot-driven car, the game compiles the selected bot binary on demand and loads the resulting ELF.

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
- `lib.rs` defines slot constants (`SLOT1`=0x100, `SLOT2`=0x200, `SLOT3`=0x300, `SLOT4`=0x400, `SLOT5`=0x500, `SLOT6`=0x600) and a bump allocator (`#[global_allocator]`)
- `driving.rs` — `CarState` (reads from SLOT2), `CarControls` (writes to SLOT3), `SplineQuery` (queries SLOT4), `TrackRadar` (reads SLOT5), and `CarRadar` (reads SLOT6) via volatile pointer access
- `bin/car.rs` — The car AI: infinite loop reading state, querying spline, computing steering/braking, writing controls
- `bin/car_radar.rs` — Radar-only car AI using `TrackRadar` (no spline-following dependency)
- `bin/bottles.rs` — Test program (99 bottles of beer via log device)
- Uses `bevy_math` with `default-features = false, features = ["libm"]`

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

### `racing/` — The Game

- **`main.rs`** — Bevy app setup, game state management (`SimState`), event-based car spawning, physics, free camera with follow-on-select, two AI systems
- **`ui.rs`** — `RaceUiPlugin`: right-side panel with driver type selector, add/remove car buttons, start/pause/reset, per-car debug gizmo toggles, per-car follow camera button, scrollable console output (drains `LogDevice` buffers)
- **`bot_runtime.rs`** — Runtime bot integration helpers: discovers available `bot` binaries via `cargo metadata`, compiles selected binaries (`cargo build --release --bin ... --target riscv32imafc-unknown-none-elf`), and loads produced ELF bytes.
- **`devices.rs`** — `CarStateDevice`, `CarControlsDevice`, `SplineDevice`, `TrackRadarDevice`, and `CarRadarDevice` implementing `Device` (host-side counterparts to the bot's volatile pointers)
- **`track.rs`** — `TrackSpline` resource, spline construction, track/kerb mesh generation
- **`track_format.rs`** — TOML-based track file format (`TrackFile`)
- **`bin/editor.rs`** — Track editor tool

**Key components:**
- `Car` — steering, accelerator, brake state (used by physics)
- `EmulatorDriver` — marker component for RISC-V-emulator-driven cars
- `CpuComponent` (from emulator crate) — attached to emulator-driven cars
- `LogDevice`, `CarStateDevice`, `CarControlsDevice`, `SplineDevice`, `TrackRadarDevice`, `CarRadarDevice` — MMIO device components attached to emulator-driven cars
- `CarLabel` — name label for each car
- `DebugGizmos` — marker; when present on a car, debug gizmos are drawn (off by default)
- `FrontWheel` — visual wheel rotation marker

**Key resources:**
- `RaceManager` — tracks all spawned cars (`Vec<CarEntry>`), selected driver type, next car ID, and per-car console output
- `FollowCar` — optional entity to follow with the camera
- `SimState` — state machine: `PreRace` (add/remove cars) → `Racing` (simulation active) → `Paused` (toggle)

**Key messages (Bevy 0.18 `Message` trait, not `Event`):**
- `SpawnCarRequest { driver: DriverType }` — sent by UI "Add Car" button, consumed by `handle_spawn_car_event`

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

**Car spawning** — Event-driven via `SpawnCarRequest` message. The UI sends a `SpawnCarRequest` with a `DriverType`; `handle_spawn_car_event` creates the car entity with staggered grid positioning. Cars can only be added/removed in `PreRace` state. Each emulator car gets its own isolated CPU (`CpuComponent`) and isolated MMIO device components; each car has its own `SplineDevice` with a cloned copy of the track spline.

**Camera** — Free camera by default (no cars spawned at startup). Middle/right-mouse drag to pan, scroll to zoom. When a car is selected via the UI "follow" button, the camera snaps to it; clicking again unfollows.

**Physics model** — Bicycle-ish 4-wheel model: acceleration/braking along forward vector, lateral grip forces per wheel computed from slip angle. Uses `avian2d` for rigid body simulation. Fixed timestep at 200 Hz.

## Key Architectural Decisions

0. **No compatibility stubs unless requested** — When refactoring APIs, do not keep unused backward-compatibility code paths, deprecated wrappers, or dead shim layers unless explicitly requested. Prefer deleting old forms and updating all call sites.

1. **Emulator is use-case agnostic** — Car-specific devices (`CarStateDevice`, `CarControlsDevice`, `SplineDevice`) live in `racing/`, not in `emulator/`. The emulator only provides `Device`, `Mmu`, `LogDevice` (buffered), `CpuComponent`, and the plugin.

2. **Each emulator car is fully isolated** — Separate `Hart`, `Dram`, and device-component instances per car entity. No shared state between emulator instances. Each car has its own `SplineDevice` with a cloned copy of the track spline.

3. **Device addressing** — The `Mmu` strips the high bits and passes offset-relative addresses (`addr & 0xFF`) to devices. Devices don't need to know their absolute slot address.

4. **Bot programs are runtime-compiled** — The game discovers bot binaries via `cargo metadata` and compiles the selected binary when the user adds a bot-driven car.

5. **Instruction budget matters** — The `instructions_per_update` value (currently 5000) must be high enough for each bot loop iteration to make progress, but low enough to avoid burning host CPU. The bot now performs window search (50 samples), lookahead walking, and curvature detection each frame.

6. **Spline logic is bot-side** — The bot implements full autonomous navigation (window search, dynamic lookahead, spline walking, curvature-based braking) using the `SplineDevice` query interface. The engine only provides basic physics state; all pathfinding intelligence runs in emulated RISC-V code.

7. **Strict compressed decode** — Compressed instruction decode is intentionally strict RV32C(+Zcf). Illegal encodings must trap/panic; do not add permissive fallbacks.

8. **Stack/DRAM alignment invariants** — DRAM allocation is rounded to 16-byte alignment with explicit stack headroom, and `sp` is set to a 16-byte aligned top-of-memory minus 16. Keep this when changing loader/builder code.

9. **Pending bot builds are state-gated** — If a bot compile finishes after leaving `PreRace`, the result is discarded and no car is spawned.

## Common Pitfalls

- **Compile latency for bot cars** — Adding a bot-driven car now triggers a build. Failures are surfaced in the UI status line and the car is not spawned on failure.
- **Device index vs slot address** — Device index 0 = address 0x100, index 1 = 0x200, etc. Off-by-one errors here will silently read zeros or fail.
- **Mmu passes offsets, not absolute addresses** — If you implement a new device, your `load`/`store` will receive `addr & 0xFF`, not the full address.
- **`instructions_per_update` tuning** — Too low and the bot can't complete a loop iteration per tick. Too high and it burns CPU time.
- **Bump allocator in bot** — The bot has a 4 KiB heap that never frees. Allocating in a loop will eventually OOM. Current bot code doesn't allocate in its hot loop, but be careful adding features that do.
- **Compressed immediates are easy to misdecode** — For `C.ADDI/C.LI/C.LUI/C.ANDI`, immediate sign comes from `inst[12]` mapped to imm bit 5. Missing that sign bit causes silent control-flow/data corruption.
