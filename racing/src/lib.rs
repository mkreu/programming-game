use bevy::prelude::*;

pub mod devices;
pub mod track;
pub mod track_format;

#[derive(Component)]
pub struct Car {
    pub steer: f32,
    pub accelerator: f32,
    pub brake: f32,
}
