use avian2d::math::PI;
use bevy::prelude::*;
use emulator::cpu::Device;

const TRACK_RADAR_RAY_COUNT: usize = 7;
const TRACK_RADAR_CONE_HALF_ANGLE_RAD: f32 = PI * 0.25;
const TRACK_RADAR_MAX_DISTANCE: f32 = 200.0;

/// Memory-mapped device exposing distances to track borders along 7 forward rays.
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
}

#[derive(Resource)]
pub struct TrackRadarBorders {
    pub inner: Vec<Vec2>,
    pub outer: Vec<Vec2>,
}

/// Runs BEFORE cpu_system::<RacingCpuConfig>: writes border ray distances into TrackRadarDevice.
pub fn update_system(
    borders: Res<TrackRadarBorders>,
    mut emu_query: Query<(&Transform, &mut TrackRadarDevice)>,
) {
    for (transform, mut track_radar_dev) in &mut emu_query {
        let car_pos = transform.translation.xy();
        let car_forward = transform.up().xy().normalize();
        track_radar_dev.update(compute_track_radar_distances(
            car_pos,
            car_forward,
            &borders,
        ));
    }
}

fn compute_track_radar_distances(
    origin: Vec2,
    forward: Vec2,
    borders: &TrackRadarBorders,
) -> [f32; TRACK_RADAR_RAY_COUNT] {
    let mut distances = [f32::NAN; TRACK_RADAR_RAY_COUNT];

    for (ray_index, distance_slot) in distances.iter_mut().enumerate() {
        let t = if TRACK_RADAR_RAY_COUNT <= 1 {
            0.5
        } else {
            ray_index as f32 / (TRACK_RADAR_RAY_COUNT - 1) as f32
        };
        let angle = -TRACK_RADAR_CONE_HALF_ANGLE_RAD + t * (2.0 * TRACK_RADAR_CONE_HALF_ANGLE_RAD);
        let ray_direction = Vec2::from_angle(angle).rotate(forward).normalize();

        let mut best = f32::INFINITY;
        best = best.min(closest_intersection_in_polyline(
            origin,
            ray_direction,
            &borders.inner,
            TRACK_RADAR_MAX_DISTANCE,
        ));
        best = best.min(closest_intersection_in_polyline(
            origin,
            ray_direction,
            &borders.outer,
            TRACK_RADAR_MAX_DISTANCE,
        ));

        if best.is_finite() {
            *distance_slot = best;
        }
    }

    distances
}

fn closest_intersection_in_polyline(
    ray_origin: Vec2,
    ray_direction: Vec2,
    polyline: &[Vec2],
    max_distance: f32,
) -> f32 {
    if polyline.len() < 2 {
        return f32::INFINITY;
    }

    let mut best = f32::INFINITY;
    for i in 0..polyline.len() {
        let a = polyline[i];
        let b = polyline[(i + 1) % polyline.len()];
        if let Some(distance) = ray_segment_intersection_distance(ray_origin, ray_direction, a, b) {
            if distance <= max_distance {
                best = best.min(distance);
            }
        }
    }

    best
}

fn ray_segment_intersection_distance(
    ray_origin: Vec2,
    ray_direction: Vec2,
    segment_start: Vec2,
    segment_end: Vec2,
) -> Option<f32> {
    let v1 = segment_start - ray_origin;
    let v2 = segment_end - segment_start;
    let denom = ray_direction.perp_dot(v2);

    if denom.abs() < 1e-6 {
        return None;
    }

    let t = v1.perp_dot(v2) / denom;
    let u = v1.perp_dot(ray_direction) / denom;

    if t >= 0.0 && (0.0..=1.0).contains(&u) {
        Some(t)
    } else {
        None
    }
}
