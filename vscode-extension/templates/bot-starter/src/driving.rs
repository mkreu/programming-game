use core::ptr;

use bevy_math::Vec2;

pub struct CarControls {
    accelerator: *mut f32,
    brake: *mut f32,
    steering: *mut f32,
}

impl CarControls {
    pub const fn bind(slot: usize) -> Self {
        Self {
            accelerator: (slot + 0x00) as *mut f32,
            brake: (slot + 0x04) as *mut f32,
            steering: (slot + 0x08) as *mut f32,
        }
    }
    pub fn set_accelerator(&mut self, value: f32) {
        unsafe {
            ptr::write_volatile(self.accelerator, value);
        }
    }
    pub fn set_brake(&mut self, value: f32) {
        unsafe {
            ptr::write_volatile(self.brake, value);
        }
    }
    pub fn set_steering(&mut self, value: f32) {
        unsafe {
            ptr::write_volatile(self.steering, value);
        }
    }
    pub fn accelerator(&self) -> f32 {
        unsafe { ptr::read_volatile(self.accelerator) }
    }
    pub fn brake(&self) -> f32 {
        unsafe { ptr::read_volatile(self.brake) }
    }
    pub fn steering(&self) -> f32 {
        unsafe { ptr::read_volatile(self.steering) }
    }
}

pub struct CarState {
    speed: *const f32,
    position_x: *const f32,
    position_y: *const f32,
    forward_x: *const f32,
    forward_y: *const f32,
}

impl CarState {
    pub const fn bind(slot: usize) -> Self {
        Self {
            speed: (slot + 0x00) as *const f32,
            position_x: (slot + 0x04) as *const f32,
            position_y: (slot + 0x08) as *const f32,
            forward_x: (slot + 0x0C) as *const f32,
            forward_y: (slot + 0x10) as *const f32,
        }
    }
    pub fn speed(&self) -> f32 {
        unsafe { ptr::read_volatile(self.speed) }
    }
    pub fn position(&self) -> Vec2 {
        unsafe { Vec2::new(ptr::read_volatile(self.position_x), ptr::read_volatile(self.position_y)) }
    }
    pub fn forward(&self) -> Vec2 {
        unsafe { Vec2::new(ptr::read_volatile(self.forward_x), ptr::read_volatile(self.forward_y)) }
    }
}

pub struct SplineQuery {
    t: *mut f32,
    x: *const f32,
    y: *const f32,
    t_max: *const f32,
}

impl SplineQuery {
    pub const fn bind(slot: usize) -> Self {
        Self {
            t: (slot + 0x00) as *mut f32,
            x: (slot + 0x04) as *const f32,
            y: (slot + 0x08) as *const f32,
            t_max: (slot + 0x0C) as *const f32,
        }
    }

    /// Query the spline at position t and return the resulting coordinates.
    pub fn query(&mut self, t: f32) -> Vec2 {
        unsafe {
            ptr::write_volatile(self.t, t);
            Vec2::new(ptr::read_volatile(self.x), ptr::read_volatile(self.y))
        }
    }

    /// Read the maximum t value (spline domain end).
    pub fn t_max(&self) -> f32 {
        unsafe {
            ptr::read_volatile(self.t_max)
        }
    }
}

pub struct TrackRadar {
    distances: [*const f32; 7],
}

impl TrackRadar {
    pub const fn bind(slot: usize) -> Self {
        Self {
            distances: [
                (slot + 0x00) as *const f32,
                (slot + 0x04) as *const f32,
                (slot + 0x08) as *const f32,
                (slot + 0x0C) as *const f32,
                (slot + 0x10) as *const f32,
                (slot + 0x14) as *const f32,
                (slot + 0x18) as *const f32,
            ],
        }
    }

    pub fn distances(&self) -> [f32; 7] {
        unsafe {
            [
                ptr::read_volatile(self.distances[0]),
                ptr::read_volatile(self.distances[1]),
                ptr::read_volatile(self.distances[2]),
                ptr::read_volatile(self.distances[3]),
                ptr::read_volatile(self.distances[4]),
                ptr::read_volatile(self.distances[5]),
                ptr::read_volatile(self.distances[6]),
            ]
        }
    }

    pub fn distance(&self, index: usize) -> f32 {
        if index >= self.distances.len() {
            return f32::NAN;
        }

        unsafe { ptr::read_volatile(self.distances[index]) }
    }
}

pub struct CarRadar {
    car_x: [*const f32; 4],
    car_y: [*const f32; 4],
}

impl CarRadar {
    pub const fn bind(slot: usize) -> Self {
        Self {
            car_x: [
                (slot + 0x00) as *const f32,
                (slot + 0x08) as *const f32,
                (slot + 0x10) as *const f32,
                (slot + 0x18) as *const f32,
            ],
            car_y: [
                (slot + 0x04) as *const f32,
                (slot + 0x0C) as *const f32,
                (slot + 0x14) as *const f32,
                (slot + 0x1C) as *const f32,
            ],
        }
    }

    /// Returns up to 4 absolute car positions, nearest-first.
    /// Empty slots are reported as None (NaN encoded in MMIO).
    pub fn positions(&self) -> [Option<Vec2>; 4] {
        unsafe {
            let x0 = ptr::read_volatile(self.car_x[0]);
            let y0 = ptr::read_volatile(self.car_y[0]);
            let x1 = ptr::read_volatile(self.car_x[1]);
            let y1 = ptr::read_volatile(self.car_y[1]);
            let x2 = ptr::read_volatile(self.car_x[2]);
            let y2 = ptr::read_volatile(self.car_y[2]);
            let x3 = ptr::read_volatile(self.car_x[3]);
            let y3 = ptr::read_volatile(self.car_y[3]);

            [
                if x0.is_nan() || y0.is_nan() {
                    None
                } else {
                    Some(Vec2::new(x0, y0))
                },
                if x1.is_nan() || y1.is_nan() {
                    None
                } else {
                    Some(Vec2::new(x1, y1))
                },
                if x2.is_nan() || y2.is_nan() {
                    None
                } else {
                    Some(Vec2::new(x2, y2))
                },
                if x3.is_nan() || y3.is_nan() {
                    None
                } else {
                    Some(Vec2::new(x3, y3))
                },
            ]
        }
    }

    pub fn position(&self, index: usize) -> Option<Vec2> {
        if index >= self.car_x.len() {
            return None;
        }

        unsafe {
            let x = ptr::read_volatile(self.car_x[index]);
            let y = ptr::read_volatile(self.car_y[index]);
            if x.is_nan() || y.is_nan() {
                None
            } else {
                Some(Vec2::new(x, y))
            }
        }
    }
}
