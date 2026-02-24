use bevy::prelude::*;

pub mod devices;
pub mod track;
pub mod track_format;

#[derive(Component)]
pub struct Car {
    pub steer: f32,
    pub accelerator: f32,
    pub brake: f32,
    pub engine_rpm: f32,
    pub wheel_omega: f32,
}
