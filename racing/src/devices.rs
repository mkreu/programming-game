use std::any::Any;

use bevy::prelude::*;
use emulator::cpu::Device;

use crate::track::TrackSpline;

/// Memory-mapped device that provides car state to the RISC-V bot.
/// Mapped at SLOT2 (0x200-0x2FF). The bot reads from this device.
///
/// Layout (all f32, little-endian):
///   0x00: speed
///   0x04: position_x
///   0x08: position_y
///   0x0C: forward_x
///   0x10: forward_y
#[derive(Component)]
pub struct CarStateDevice {
    data: [u8; 20], // 5 × f32
}

impl Default for CarStateDevice {
    fn default() -> Self {
        Self { data: [0u8; 20] }
    }
}

impl CarStateDevice {
    fn write_f32(&mut self, offset: usize, value: f32) {
        let bytes = value.to_le_bytes();
        self.data[offset..offset + 4].copy_from_slice(&bytes);
    }

    /// Write the full car state from the simulation.
    pub fn update(&mut self, speed: f32, position: Vec2, forward: Vec2) {
        self.write_f32(0x00, speed);
        self.write_f32(0x04, position.x);
        self.write_f32(0x08, position.y);
        self.write_f32(0x0C, forward.x);
        self.write_f32(0x10, forward.y);
    }
}

impl Device for CarStateDevice {
    fn load(&self, addr: u32, size: u32) -> Result<u32, ()> {
        let addr = addr as usize;
        match size {
            8 => {
                if addr < self.data.len() {
                    Ok(self.data[addr] as u32)
                } else {
                    Ok(0)
                }
            }
            16 => {
                if addr + 1 < self.data.len() {
                    Ok((self.data[addr] as u32) | ((self.data[addr + 1] as u32) << 8))
                } else {
                    Ok(0)
                }
            }
            32 => {
                if addr + 3 < self.data.len() {
                    Ok((self.data[addr] as u32)
                        | ((self.data[addr + 1] as u32) << 8)
                        | ((self.data[addr + 2] as u32) << 16)
                        | ((self.data[addr + 3] as u32) << 24))
                } else {
                    Ok(0)
                }
            }
            _ => Err(()),
        }
    }

    fn store(&mut self, _addr: u32, _size: u32, _value: u32) -> Result<(), ()> {
        // CarState is read-only from the bot's perspective; silently ignore writes
        Ok(())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// Memory-mapped device for car controls written by the RISC-V bot.
/// Mapped at SLOT3 (0x300-0x3FF). The bot writes to this device.
///
/// Layout (all f32, little-endian):
///   0x00: accelerator
///   0x04: brake
///   0x08: steering
#[derive(Component)]
pub struct CarControlsDevice {
    data: [u8; 12], // 3 × f32
}

impl Default for CarControlsDevice {
    fn default() -> Self {
        Self { data: [0u8; 12] }
    }
}

impl CarControlsDevice {
    fn read_f32(&self, offset: usize) -> f32 {
        let bytes = [
            self.data[offset],
            self.data[offset + 1],
            self.data[offset + 2],
            self.data[offset + 3],
        ];
        f32::from_le_bytes(bytes)
    }

    /// Read the accelerator value set by the bot.
    pub fn accelerator(&self) -> f32 {
        self.read_f32(0x00)
    }

    /// Read the brake value set by the bot.
    pub fn brake(&self) -> f32 {
        self.read_f32(0x04)
    }

    /// Read the steering value set by the bot.
    pub fn steering(&self) -> f32 {
        self.read_f32(0x08)
    }
}

impl Device for CarControlsDevice {
    fn load(&self, addr: u32, size: u32) -> Result<u32, ()> {
        // Allow the bot to read back its own controls
        let addr = addr as usize;
        match size {
            8 => {
                if addr < self.data.len() {
                    Ok(self.data[addr] as u32)
                } else {
                    Ok(0)
                }
            }
            16 => {
                if addr + 1 < self.data.len() {
                    Ok((self.data[addr] as u32) | ((self.data[addr + 1] as u32) << 8))
                } else {
                    Ok(0)
                }
            }
            32 => {
                if addr + 3 < self.data.len() {
                    Ok((self.data[addr] as u32)
                        | ((self.data[addr + 1] as u32) << 8)
                        | ((self.data[addr + 2] as u32) << 16)
                        | ((self.data[addr + 3] as u32) << 24))
                } else {
                    Ok(0)
                }
            }
            _ => Err(()),
        }
    }

    fn store(&mut self, addr: u32, size: u32, value: u32) -> Result<(), ()> {
        let addr = addr as usize;
        match size {
            8 => {
                if addr < self.data.len() {
                    self.data[addr] = value as u8;
                    Ok(())
                } else {
                    Err(())
                }
            }
            16 => {
                if addr + 1 < self.data.len() {
                    self.data[addr] = (value & 0xFF) as u8;
                    self.data[addr + 1] = ((value >> 8) & 0xFF) as u8;
                    Ok(())
                } else {
                    Err(())
                }
            }
            32 => {
                if addr + 3 < self.data.len() {
                    self.data[addr] = (value & 0xFF) as u8;
                    self.data[addr + 1] = ((value >> 8) & 0xFF) as u8;
                    self.data[addr + 2] = ((value >> 16) & 0xFF) as u8;
                    self.data[addr + 3] = ((value >> 24) & 0xFF) as u8;
                    Ok(())
                } else {
                    Err(())
                }
            }
            _ => Err(()),
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// Memory-mapped device for spline interpolation queries.
/// Mapped at SLOT4 (0x400-0x4FF). The bot writes a `t` parameter and reads back the interpolated position.
///
/// Layout (all f32, little-endian):
///   0x00: t (write) - parameter to sample the spline at
///   0x04: x (read)  - resulting X coordinate of sampled position
///   0x08: y (read)  - resulting Y coordinate of sampled position
///   0x0C: t_max (read) - maximum value of t (spline domain end)
#[derive(Component)]
pub struct SplineDevice {
    spline: CubicCurve<Vec2>,
    t_max: f32,
    last_t: f32,
    last_position: Vec2,
}

impl SplineDevice {
    pub fn new(track_spline: &TrackSpline) -> Self {
        let domain = track_spline.spline.domain();
        let t_max = domain.end();
        Self {
            spline: track_spline.spline.clone(),
            t_max,
            last_t: 0.0,
            last_position: Vec2::ZERO,
        }
    }
}

impl Device for SplineDevice {
    fn load(&self, addr: u32, size: u32) -> Result<u32, ()> {
        if size != 32 {
            return Err(());
        }

        let addr = addr as usize;
        match addr {
            0x04 => {
                // Read X coordinate
                Ok(u32::from_le_bytes(self.last_position.x.to_le_bytes()))
            }
            0x08 => {
                // Read Y coordinate
                Ok(u32::from_le_bytes(self.last_position.y.to_le_bytes()))
            }
            0x0C => {
                // Read t_max
                Ok(u32::from_le_bytes(self.t_max.to_le_bytes()))
            }
            _ => Ok(0),
        }
    }

    fn store(&mut self, addr: u32, size: u32, value: u32) -> Result<(), ()> {
        if size != 32 {
            return Err(());
        }

        let addr = addr as usize;
        match addr {
            0x00 => {
                // Write t parameter, compute and cache the position
                let t = f32::from_le_bytes(value.to_le_bytes());
                self.last_t = t;
                self.last_position = self.spline.position(t);
                Ok(())
            }
            _ => Ok(()), // Ignore writes to other addresses
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// Memory-mapped device exposing distances to track borders along 7 forward rays.
/// Mapped at SLOT5 (0x500-0x5FF). The bot reads from this device.
///
/// Layout (all f32, little-endian):
///   0x00: ray_0_distance
///   0x04: ray_1_distance
///   0x08: ray_2_distance
///   0x0C: ray_3_distance
///   0x10: ray_4_distance
///   0x14: ray_5_distance
///   0x18: ray_6_distance
///
/// Distances are nearest-hit distances in world units. If a ray has no hit,
/// the value is NaN.
#[derive(Component)]
pub struct TrackRadarDevice {
    data: [u8; 28], // 7 × f32
}

impl Default for TrackRadarDevice {
    fn default() -> Self {
        Self { data: [0u8; 28] }
    }
}

impl TrackRadarDevice {
    fn write_f32(&mut self, offset: usize, value: f32) {
        let bytes = value.to_le_bytes();
        self.data[offset..offset + 4].copy_from_slice(&bytes);
    }

    pub fn update(&mut self, distances: [f32; 7]) {
        for (i, distance) in distances.into_iter().enumerate() {
            self.write_f32(i * 4, distance);
        }
    }
}

impl Device for TrackRadarDevice {
    fn load(&self, addr: u32, size: u32) -> Result<u32, ()> {
        let addr = addr as usize;
        match size {
            8 => {
                if addr < self.data.len() {
                    Ok(self.data[addr] as u32)
                } else {
                    Ok(0)
                }
            }
            16 => {
                if addr + 1 < self.data.len() {
                    Ok((self.data[addr] as u32) | ((self.data[addr + 1] as u32) << 8))
                } else {
                    Ok(0)
                }
            }
            32 => {
                if addr + 3 < self.data.len() {
                    Ok((self.data[addr] as u32)
                        | ((self.data[addr + 1] as u32) << 8)
                        | ((self.data[addr + 2] as u32) << 16)
                        | ((self.data[addr + 3] as u32) << 24))
                } else {
                    Ok(0)
                }
            }
            _ => Err(()),
        }
    }

    fn store(&mut self, _addr: u32, _size: u32, _value: u32) -> Result<(), ()> {
        // Track radar is read-only from the bot's perspective; silently ignore writes.
        Ok(())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// Memory-mapped device exposing absolute positions of up to the 4 nearest cars.
/// Mapped at SLOT6 (0x600-0x6FF). The bot reads from this device.
///
/// Layout (all f32, little-endian), nearest-first, excluding self:
///   0x00: car0_x
///   0x04: car0_y
///   0x08: car1_x
///   0x0C: car1_y
///   0x10: car2_x
///   0x14: car2_y
///   0x18: car3_x
///   0x1C: car3_y
///
/// Missing entries are encoded as NaN/NaN.
#[derive(Component)]
pub struct CarRadarDevice {
    data: [u8; 32], // 8 × f32
}

impl Default for CarRadarDevice {
    fn default() -> Self {
        Self { data: [0u8; 32] }
    }
}

impl CarRadarDevice {
    fn write_f32(&mut self, offset: usize, value: f32) {
        let bytes = value.to_le_bytes();
        self.data[offset..offset + 4].copy_from_slice(&bytes);
    }

    pub fn update(&mut self, nearest_positions: [Option<Vec2>; 4]) {
        for (i, maybe_pos) in nearest_positions.into_iter().enumerate() {
            let base = i * 8;
            if let Some(pos) = maybe_pos {
                self.write_f32(base, pos.x);
                self.write_f32(base + 4, pos.y);
            } else {
                self.write_f32(base, f32::NAN);
                self.write_f32(base + 4, f32::NAN);
            }
        }
    }
}

impl Device for CarRadarDevice {
    fn load(&self, addr: u32, size: u32) -> Result<u32, ()> {
        let addr = addr as usize;
        match size {
            8 => {
                if addr < self.data.len() {
                    Ok(self.data[addr] as u32)
                } else {
                    Ok(0)
                }
            }
            16 => {
                if addr + 1 < self.data.len() {
                    Ok((self.data[addr] as u32) | ((self.data[addr + 1] as u32) << 8))
                } else {
                    Ok(0)
                }
            }
            32 => {
                if addr + 3 < self.data.len() {
                    Ok((self.data[addr] as u32)
                        | ((self.data[addr + 1] as u32) << 8)
                        | ((self.data[addr + 2] as u32) << 16)
                        | ((self.data[addr + 3] as u32) << 24))
                } else {
                    Ok(0)
                }
            }
            _ => Err(()),
        }
    }

    fn store(&mut self, _addr: u32, _size: u32, _value: u32) -> Result<(), ()> {
        // Car radar is read-only from the bot's perspective; silently ignore writes.
        Ok(())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
