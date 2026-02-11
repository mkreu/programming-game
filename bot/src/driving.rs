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
    target_x: *const f32,
    target_y: *const f32,
}

impl CarState {
    pub const fn bind(slot: usize) -> Self {
        Self {
            speed: (slot + 0x00) as *const f32,
            position_x: (slot + 0x04) as *const f32,
            position_y: (slot + 0x08) as *const f32,
            forward_x: (slot + 0x0C) as *const f32,
            forward_y: (slot + 0x10) as *const f32,
            target_x: (slot + 0x14) as *const f32,
            target_y: (slot + 0x18) as *const f32,
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
    pub fn target(&self) -> Vec2 {
        unsafe { Vec2::new(ptr::read_volatile(self.target_x), ptr::read_volatile(self.target_y)) }
    }    
}
