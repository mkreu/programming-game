use crate::track::TrackSpline;
use bevy::prelude::*;
use emulator::cpu::Device;

/// Memory-mapped device for spline interpolation queries.
/// The bot writes a `t` parameter and reads back the interpolated position.
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
}
