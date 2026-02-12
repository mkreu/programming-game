# AGENTS.md — AI Agent Guide

## Project Overview

A racing game prototype where cars can be driven by AI implemented as **RISC-V programs** running inside an emulator. The goal is to let users write bare-metal RISC-V code (in Rust, `no_std`) that controls a car via memory-mapped I/O, while the game handles physics, rendering, and track management.

There is also a native (Rust-side) AI driver for comparison.

## Workspace Structure

```
Cargo.toml            # Workspace root — members: racing, emulator. Excludes bot.
├── racing/           # Bevy game — physics, rendering, car spawning, AI systems
├── emulator/         # Use-case-agnostic RISC-V emulator (RV32IMAFC + RV32C/Zcf) with Bevy integration
└── bot/              # no_std RISC-V programs compiled to bare-metal ELF (separate target)
```

**Important**: `bot/` is excluded from the workspace because it cross-compiles to `riscv32imafc-unknown-none-elf`. It must be built separately before the racing crate (which embeds the ELF via `include_bytes!`).

**Important**: Always keep this file up to date when changing code.

## Build & Run

```bash
# 1. Build the bot ELF first (required before racing can compile)
cd bot && cargo build --release --bin car

# 2. Run the racing game (from workspace root)
cargo run --bin racing [-- path/to/track.toml]
# Default track: racing/assets/track1.toml
```

The bot ELF is embedded at compile time in `racing/src/main.rs` via:
```rust
const BOT_ELF: &[u8] = include_bytes!("../../bot/target/riscv32imafc-unknown-none-elf/release/car");
```

If you modify bot code, you must rebuild the bot before rebuilding racing.

## Crate Details

### `emulator/` — RISC-V Emulator

**Must remain use-case agnostic.** No car/racing-specific code belongs here.

- **`cpu.rs`** — Core emulator: `Hart` (32 GPRs, 32 FPRs, PC, LR/SC reservation), `Dram` (ELF-backed memory with stack headroom), `Mmu` (routes memory accesses to DRAM or devices), `LogDevice` (prints chars)
- **`bevy.rs`** — `EmulatorPlugin` adds `cpu_system` to `FixedUpdate`. `CpuComponent` holds a `Hart`, `Dram`, and `Vec<Box<dyn RamLike>>` of devices. Use `CpuComponent::new(elf, devices, instructions_per_update)` to create.
- **`lib.rs`** — `CpuBuilder` helper

**`RamLike` trait** (`cpu.rs`) — The memory interface for devices:
```rust
pub trait RamLike: Send + Sync {
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
| `0x400–0x4FF`   | 3           | (available)     |
| `≥ 0x1000`      | —           | DRAM            |

Devices receive **offset-relative addresses** (i.e., `addr & 0xFF`), not absolute addresses.

**`CpuComponent` device access** — Use typed downcasting:
```rust
cpu.device_as::<CarStateDevice>(1)       // &CarStateDevice
cpu.device_as_mut::<CarControlsDevice>(2) // &mut CarControlsDevice
```

### `bot/` — RISC-V Bot Programs

- Target: `riscv32imafc-unknown-none-elf` (configured in `bot/.cargo/config.toml`)
- Linker script `link.x` places `.text` at `0x1000` (start of DRAM)
- `lib.rs` defines slot constants (`SLOT1`=0x100, `SLOT2`=0x200, `SLOT3`=0x300) and a bump allocator (`#[global_allocator]`)
- `driving.rs` — `CarState` (reads from SLOT2) and `CarControls` (writes to SLOT3) via volatile pointer access
- `bin/car.rs` — The car AI: infinite loop reading state, computing steering, writing controls
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
| 0x14   | target_x    | f32  |
| 0x18   | target_y    | f32  |

**CarControls layout** (SLOT3, 0x300, written by bot):
| Offset | Field       | Type |
|--------|-------------|------|
| 0x00   | accelerator | f32  |
| 0x04   | brake       | f32  |
| 0x08   | steering    | f32  |

### `racing/` — The Game

- **`main.rs`** — Bevy app setup, car spawning, physics, camera, two AI systems
- **`devices.rs`** — `CarStateDevice` and `CarControlsDevice` implementing `RamLike` (host-side counterparts to the bot's volatile pointers)
- **`track.rs`** — `TrackSpline` resource, spline construction, track/kerb mesh generation
- **`track_format.rs`** — TOML-based track file format (`TrackFile`)
- **`bin/editor.rs`** — Track editor tool

**Key components:**
- `Car` — steering, accelerator, brake state (used by physics)
- `AIDriver` — native Rust AI (spline-following with curvature-based braking)
- `EmulatorDriver` — marks a car as RISC-V-emulator-driven
- `CpuComponent` (from emulator crate) — attached to emulator-driven cars
- `FrontWheel` — visual wheel rotation marker

**System execution order:**
1. `Update`: `handle_car_input` (keyboard → `Car`, excludes AI/emulator cars), `update_ai_driver` (native AI → `Car`)
2. `FixedUpdate` (in order):
   - `update_emulator_driver` — writes physics state into `CarStateDevice` (**before** `cpu_system`)
   - `cpu_system` — runs N RISC-V instructions per tick (emulator crate)
   - `apply_emulator_controls` — reads `CarControlsDevice` → `Car` (**after** `cpu_system`)
   - `apply_car_forces` — applies `Car` state to physics forces

**Car spawning** — `spawn_car()` takes a `DriverType` enum (`Ai`, `Emulator`, `None`). In `setup()`, cars alternate: even-indexed get `AIDriver`, odd-indexed get `EmulatorDriver` + `CpuComponent` with the embedded bot ELF. Each emulator car gets its own isolated CPU, memory, and device set.

**Physics model** — Bicycle-ish 4-wheel model: acceleration/braking along forward vector, lateral grip forces per wheel computed from slip angle. Uses `avian2d` for rigid body simulation. Fixed timestep at 200 Hz.

## Key Architectural Decisions

1. **Emulator is use-case agnostic** — Car-specific devices (`CarStateDevice`, `CarControlsDevice`) live in `racing/`, not in `emulator/`. The emulator only provides `RamLike`, `Mmu`, `LogDevice`, `CpuComponent`, and the plugin.

2. **Each emulator car is fully isolated** — Separate `Hart`, `Dram`, and device instances per car entity. No shared state between emulator instances.

3. **Device addressing** — The `Mmu` strips the high bits and passes offset-relative addresses (`addr & 0xFF`) to devices. Devices don't need to know their absolute slot address.

4. **Bot programs are embedded** — The ELF binary is included at compile time via `include_bytes!`. There's no `build.rs` automation; the bot must be built manually first.

5. **Instruction budget matters** — The `instructions_per_update` value (currently 5000) must be high enough for each bot loop iteration to make progress, but low enough to avoid burning host CPU.
6. **Strict compressed decode** — Compressed instruction decode is intentionally strict RV32C(+Zcf). Illegal encodings must trap/panic; do not add permissive fallbacks.
7. **Stack/DRAM alignment invariants** — DRAM allocation is rounded to 16-byte alignment with explicit stack headroom, and `sp` is set to a 16-byte aligned top-of-memory minus 16. Keep this when changing loader/builder code.

## Common Pitfalls

- **Forgetting to rebuild the bot** — If you change `bot/` code, you must `cd bot && cargo build --release` before rebuilding `racing/`. The `include_bytes!` path won't trigger a rebuild automatically.
- **Device index vs slot address** — Device index 0 = address 0x100, index 1 = 0x200, etc. Off-by-one errors here will silently read zeros or fail.
- **Mmu passes offsets, not absolute addresses** — If you implement a new device, your `load`/`store` will receive `addr & 0xFF`, not the full address.
- **`instructions_per_update` tuning** — Too low and the bot can't complete a loop iteration per tick. Too high and it burns CPU time.
- **Bump allocator in bot** — The bot has a 4 KiB heap that never frees. Allocating in a loop will eventually OOM. Current bot code doesn't allocate in its hot loop, but be careful adding features that do.
- **Compressed immediates are easy to misdecode** — For `C.ADDI/C.LI/C.LUI/C.ANDI`, immediate sign comes from `inst[12]` mapped to imm bit 5. Missing that sign bit causes silent control-flow/data corruption.
