use bevy::prelude::*;
use emulator::cpu::Device;

use crate::Car;

/// Memory-mapped device exposing absolute positions of up to the 4 nearest cars.
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
}

/// Runs BEFORE cpu_system::<RacingCpuConfig>: writes nearest-car positions into CarRadarDevice.
pub fn update_system(
    all_cars: Query<(Entity, &Transform), With<Car>>,
    mut emu_query: Query<(Entity, &Transform, &mut CarRadarDevice)>,
) {
    let car_positions: Vec<(Entity, Vec2)> = all_cars
        .iter()
        .map(|(entity, transform)| (entity, transform.translation.xy()))
        .collect();

    for (entity, transform, mut car_radar_dev) in &mut emu_query {
        let car_pos = transform.translation.xy();
        car_radar_dev.update(compute_nearest_car_positions(
            entity,
            car_pos,
            &car_positions,
        ));
    }
}

fn compute_nearest_car_positions(
    self_entity: Entity,
    self_position: Vec2,
    all_car_positions: &[(Entity, Vec2)],
) -> [Option<Vec2>; 4] {
    let mut nearest: Vec<(f32, Vec2)> = all_car_positions
        .iter()
        .filter_map(|(entity, position)| {
            if *entity == self_entity {
                None
            } else {
                Some((self_position.distance_squared(*position), *position))
            }
        })
        .collect();

    nearest.sort_by(|a, b| a.0.total_cmp(&b.0));

    let mut result = [None, None, None, None];
    for (slot, (_, position)) in nearest.into_iter().take(4).enumerate() {
        result[slot] = Some(position);
    }

    result
}
