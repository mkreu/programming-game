use std::f32::consts::PI;

use bevy::{
    color::palettes::css::WHITE, diagnostic::FrameTimeDiagnosticsPlugin, input::mouse::MouseWheel,
    math::ops::tan, prelude::*,
};
use iyes_perf_ui::{PerfUiPlugin, prelude::PerfUiDefaultEntries};

mod track;

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins,
            FrameTimeDiagnosticsPlugin::default(),
            PerfUiPlugin,
        ))
        .add_systems(Startup, (setup,track::setup))
        .add_systems(Startup, set_default_zoom.after(setup))
        .add_systems(FixedUpdate, drive_car)
        .add_systems(Update, (update_camera, draw_gizmos))
        .run();
}

const WHEEL_BASE: f32 = 3.5;

fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
) {
    commands.spawn(PerfUiDefaultEntries::default());

    // Spawn a camera; we'll set a custom default zoom once in `set_default_zoom`.
    commands.spawn(Camera2d);

    let sprite_scale = Vec3::splat(0.02);
    let start_point = track::first_point();

    // Spawn the player car with wheels as children
    commands
        .spawn((
            Transform::from_xyz(start_point.x, start_point.y, 1.0),
            Visibility::default(),
            Car {
                velocity: Vec2::ZERO,
                angle: 0.0,
                speed: 0.0,
            },
        ))
        .with_children(|parent| {
            parent.spawn((
                Sprite::from_image(asset_server.load("blue_car_without_wheels.png")),
                Transform::from_xyz(0.0, 2.05, 0.1).with_scale(sprite_scale),
            ));

            // Front left wheel
            parent.spawn((
                Transform::from_xyz(-0.75, 3.5, 0.1),
                Visibility::default(),
                Wheel,
            )).with_children(|parent| {
                parent.spawn(
                    (
                        Sprite::from_image(asset_server.load("wheel.png")),
                        Transform::default()
                            .with_scale(sprite_scale)
                            .with_rotation(Quat::from_rotation_z(0.0)),
                    ),
                );
            });
            // Front right wheel
            parent.spawn((
                Transform::from_xyz(0.75, 3.5, 0.1),
                Visibility::default(),
                Wheel,
            )).with_children(|parent| {
                parent.spawn(
                    (
                        Sprite::from_image(asset_server.load("wheel.png")),
                        Transform::default()
                            .with_scale(sprite_scale)
                            .with_rotation(Quat::from_rotation_z(PI)),
                    ),
                );
            });
        });
}


fn set_default_zoom(
    mut camera_query: Query<&mut Projection, With<Camera2d>>,
) {
    let Ok(mut projection) = camera_query.single_mut() else {
        return;
    };

    if let Projection::Orthographic(ref mut ortho) = *projection {
        ortho.scale = 0.05;
    }
}

#[derive(Component)]
struct Car {
    velocity: Vec2,
    angle: f32,
    speed: f32,
}

#[derive(Component)]
struct Wheel;

fn drive_car(
    mut car_query: Query<(&mut Transform, &mut Car, &Children)>,
    mut wheel_query: Query<&mut Transform, (With<Wheel>, Without<Car>)>,
    keyboard: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();

    for (mut transform, mut car, children) in &mut car_query {
        // Car physics parameters
        let acceleration = 50.0;
        let braking = 70.0;
        let max_speed = 100.0; // m/s
        let reverse_speed = 100.0;
        let drag: f32 = 0.99;

        // Forward/Backward
        if keyboard.pressed(KeyCode::KeyW) {
            car.speed += acceleration * dt;
            car.speed = car.speed.min(max_speed);
        }
        if keyboard.pressed(KeyCode::KeyS) {
            car.speed -= braking * dt;
            car.speed = car.speed.max(-reverse_speed);
        }

        // Apply drag
        car.speed *= drag.powf(dt * 60.0_f32);

        // Stop if very slow
        if car.speed.abs() < 0.1 {
            car.speed = 0.0;
        }

        let mut wheel_angle = 0.0;

        if car.speed.abs() > 0.3 {
            let steer_factor = 0.5/ (1.0 + 0.01 * car.speed.abs()).powf(2.0);
            let turn_radius = WHEEL_BASE / tan(steer_factor);
            let angular_velocity = car.speed / turn_radius;
            if keyboard.pressed(KeyCode::KeyA) {
                car.angle -= angular_velocity * dt;
                wheel_angle = -steer_factor; // Left turn
            }
            if keyboard.pressed(KeyCode::KeyD) {
                car.angle += angular_velocity * dt;
                wheel_angle = steer_factor; // Left turn
            }
        }

        // Update velocity based on angle and speed
        car.velocity = Vec2::new(car.angle.sin(), car.angle.cos()) * car.speed;

        // Update position
        transform.translation.x += car.velocity.x * dt;
        transform.translation.y += car.velocity.y * dt;

        // Update rotation
        transform.rotation = Quat::from_rotation_z(-car.angle);

        // Update wheel rotation
        for child in children.iter() {
            if let Ok(mut wheel_transform) = wheel_query.get_mut(child) {
                wheel_transform.rotation = Quat::from_rotation_z(-wheel_angle);
            }
        }
    }
}

fn draw_gizmos(car_query: Query<(&Transform, &Car)>, mut gizmos: Gizmos) {
    for (transform, _car) in &car_query {
        gizmos.cross(transform.to_isometry(), 1.0, WHITE);
        gizmos.cross(
            Isometry3d::new(
                transform.translation + transform.up() * WHEEL_BASE,
                transform.rotation,
            ),
            1.0,
            WHITE,
        );
    }
}

fn update_camera(
    car_query: Query<&Transform, With<Car>>,
    mut camera_query: Query<(&mut Transform, &mut Projection), (With<Camera2d>, Without<Car>)>,
    mut scroll_events: MessageReader<MouseWheel>,
) {
    let Ok(car_transform) = car_query.single() else {
        return;
    };
    let Ok((mut camera_transform, mut projection)) = camera_query.single_mut() else {
        return;
    };

    // Handle zoom with mouse wheel
    if let Projection::Orthographic(ref mut ortho) = *projection {
        for event in scroll_events.read() {
            let zoom_delta = match event.unit {
                bevy::input::mouse::MouseScrollUnit::Line => event.y * 0.1,
                bevy::input::mouse::MouseScrollUnit::Pixel => event.y * 0.001,
            };

            ortho.scale *= 1.0 - zoom_delta;
            ortho.scale = ortho.scale.clamp(0.01, 5.0);
        }
    }

    // Follow the car
    camera_transform.translation.x = car_transform.translation.x;
    camera_transform.translation.y = car_transform.translation.y;
}
