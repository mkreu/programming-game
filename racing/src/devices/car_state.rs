use avian2d::prelude::*;
use bevy::prelude::*;
use emulator::cpu::Device;

/// Memory-mapped device that provides car state to the RISC-V bot.
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
}

/// Runs BEFORE cpu_system::<RacingCpuConfig>: writes host car kinematics into CarStateDevice.
pub fn system(mut emu_query: Query<(&Transform, &LinearVelocity, &mut CarStateDevice)>) {
    for (transform, velocity, mut state_dev) in &mut emu_query {
        let car_pos = transform.translation.xy();
        let car_forward = transform.up().xy().normalize();
        let car_speed = velocity.length();
        state_dev.update(car_speed, car_pos, car_forward);
    }
}
