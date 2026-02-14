use bevy::prelude::*;
use emulator::cpu::Device;

use crate::Car;

/// Memory-mapped device for car controls written by the RISC-V bot.
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
}

/// Runs AFTER cpu_system::<RacingCpuConfig>: reads control outputs and applies them.
pub fn update_system(mut emu_query: Query<(&mut Car, &CarControlsDevice)>) {
    for (mut car, ctrl_dev) in &mut emu_query {
        car.accelerator = ctrl_dev.accelerator();
        car.brake = ctrl_dev.brake();
        car.steer = ctrl_dev.steering();
    }
}
