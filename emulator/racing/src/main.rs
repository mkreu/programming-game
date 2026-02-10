use std::f32::consts::PI;

use avian2d::prelude::{forces::ForcesItem, *};
use bevy::{
    color::palettes::css::{GREEN, RED, WHITE, YELLOW},
    diagnostic::FrameTimeDiagnosticsPlugin,
    input::mouse::MouseWheel,
    prelude::*,
};
use iyes_perf_ui::{PerfUiPlugin, prelude::PerfUiDefaultEntries};

mod track;

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins,
            FrameTimeDiagnosticsPlugin::default(),
            PerfUiPlugin,
            PhysicsPlugins::default(),
            PhysicsDebugPlugin::default(),
        ))
        .insert_resource(Gravity::ZERO)
        .insert_resource(Time::<Fixed>::from_duration(std::time::Duration::from_secs_f32(1.0 / 200.0)))
        .add_systems(Startup, (setup, track::setup))
        .add_systems(Startup, set_default_zoom.after(setup))
        .add_systems(FixedUpdate, apply_car_forces)
        .add_systems(Update, (update_camera, draw_gizmos))
        .run();
}

const WHEEL_BASE: f32 = 1.18;
const WHEEL_TRACK: f32 = 0.95;

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn(PerfUiDefaultEntries::default());

    // Spawn a camera; we'll set a custom default zoom once in `set_default_zoom`.
    commands.spawn(Camera2d);

    let sprite_scale = Vec3::splat(0.008);
    let start_point = track::first_point();

    // Spawn the player car with wheels as children
    commands
        .spawn((
            Transform::from_xyz(start_point.x, start_point.y, 1.0),
            Visibility::default(),
            RigidBody::Dynamic,
            LinearDamping(0.1),
            //AngularDamping(0.8),
            Car { steer: 0.0 },
        ))
        .with_children(|parent| {
            parent.spawn((
                Collider::rectangle(1.25, 2.0),
                Transform::from_xyz(0.0, 0.66, 0.0),
            ));

            parent.spawn((
                Sprite::from_image(asset_server.load("kart.png")),
                Transform::from_xyz(0.0, 0.66, 0.1).with_scale(sprite_scale),
            ));

            // Front left wheel
            parent
                .spawn((
                    Transform::from_xyz(-WHEEL_TRACK / 2.0, WHEEL_BASE, 0.1),
                    Visibility::default(),
                    FrontWheel,
                ))
                .with_children(|parent| {
                    parent.spawn((
                        Sprite::from_image(asset_server.load("kart_wheel.png")),
                        Transform::default()
                            .with_scale(sprite_scale)
                            .with_rotation(Quat::from_rotation_z(0.0)),
                    ));
                });
            // Front right wheel
            parent
                .spawn((
                    Transform::from_xyz(WHEEL_TRACK / 2.0, WHEEL_BASE, 0.1),
                    Visibility::default(),
                    FrontWheel,
                ))
                .with_children(|parent| {
                    parent.spawn((
                        Sprite::from_image(asset_server.load("kart_wheel.png")),
                        Transform::default()
                            .with_scale(sprite_scale)
                            .with_rotation(Quat::from_rotation_z(PI)),
                    ));
                });
        });
}

fn set_default_zoom(mut camera_query: Query<&mut Projection, With<Camera2d>>) {
    let Ok(mut projection) = camera_query.single_mut() else {
        return;
    };

    if let Projection::Orthographic(ref mut ortho) = *projection {
        ortho.scale = 0.05;
    }
}

#[derive(Component)]
struct Car {
    steer: f32,
}

#[derive(Component)]
struct FrontWheel;

fn apply_car_forces(
    mut car_query: Query<(&Transform, &mut Car, &Children, Forces)>,
    mut wheel_query: Query<&mut Transform, (With<FrontWheel>, Without<Car>)>,
    mut gizmos: Gizmos,
    keyboard: Res<ButtonInput<KeyCode>>,
) {
    for (transform, mut car, children, mut forces) in &mut car_query {
        // Car physics parameters
        let acceleration = 30.0;
        let braking = 50.0;

        let position = transform.translation.xy();
        let forward = transform.up().xy().normalize();
        let left = forward.perp();

        // Forward/Backward
        if keyboard.pressed(KeyCode::KeyS) {
            forces.apply_linear_acceleration(forward * -braking);
            gizmos.arrow_2d(position, position + forward * -braking * 0.3, WHITE);
        } else if keyboard.pressed(KeyCode::KeyW) {
            forces.apply_linear_acceleration(forward * acceleration);
            gizmos.arrow_2d(position, position + forward * acceleration * 0.3, WHITE);
        }

        let max_steer = PI / 6.0; // Max steering angle (30 degrees);
        let steer_rate = 0.05 * car.steer.abs().max(0.1); // Steering speed
        if keyboard.pressed(KeyCode::KeyA) {
            car.steer = (-max_steer).max(car.steer - steer_rate);
        } else if keyboard.pressed(KeyCode::KeyD) {
            car.steer = max_steer.min(car.steer + steer_rate);
        } else {
            // Return wheels to center when no input
            car.steer = if car.steer > 0.0 {
                (car.steer - steer_rate).max(0.0)
            } else {
                (car.steer + steer_rate).min(0.0)
            };
        }

        // Front left
        apply_wheel_force(
            position,
            forward * WHEEL_BASE + left * -WHEEL_TRACK / 2.0,
            Vec2::from_angle(-car.steer).rotate(forward),
            &mut forces,
            &mut gizmos,
        );
        // Front right
        apply_wheel_force(
            position,
            forward * WHEEL_BASE + left * WHEEL_TRACK / 2.0,
            Vec2::from_angle(-car.steer).rotate(forward),
            &mut forces,
            &mut gizmos,
        );
        // Rear left
        apply_wheel_force(position, left * -WHEEL_TRACK / 2.0, forward, &mut forces, &mut gizmos);
        // Rear right
        apply_wheel_force(position, left * WHEEL_TRACK / 2.0, forward, &mut forces, &mut gizmos);

        // Update wheel rotation
        for child in children.iter() {
            if let Ok(mut wheel_transform) = wheel_query.get_mut(child) {
                wheel_transform.rotation = Quat::from_rotation_z(-car.steer);
            }
        }
    }
}

fn apply_wheel_force(
    car_position: Vec2,
    wheel_offset: Vec2,
    wheel_forward: Vec2,
    forces: &mut ForcesItem<'_, '_>,
    gizmos: &mut Gizmos,
) {
    let wheel_pos = car_position + wheel_offset;
    let wheel_left = wheel_forward.perp();
    gizmos.arrow_2d(wheel_pos, wheel_pos + wheel_forward * 1.0, YELLOW);
    gizmos.arrow_2d(wheel_pos, wheel_pos + wheel_left * 0.5, YELLOW);

    let o = forces.angular_velocity();
    let l = forces.linear_velocity();
    let wow = wheel_pos - car_position;
    let wheel_velocity = l + Vec2::new(-o * wow.y, o * wow.x);
    gizmos.arrow_2d(wheel_pos, wheel_pos + wheel_velocity * 0.1, GREEN);

    if wheel_velocity.length() > 0.1 {
        let force = -wheel_velocity.normalize().dot(wheel_left)
            * wheel_left
            * 10.0_f32.min(wheel_velocity.length() * 5.0);
        gizmos.arrow_2d(wheel_pos, wheel_pos + force, RED);
        forces.apply_linear_acceleration_at_point(force, wheel_pos);
    }
}

fn draw_gizmos(car_query: Query<(&Transform, &Car)>, mut gizmos: Gizmos) {
    for (transform, _car) in &car_query {
        gizmos.cross(transform.to_isometry(), 0.2, RED);
        gizmos.cross(
            Isometry3d::new(
                transform.translation + transform.up() * WHEEL_BASE,
                transform.rotation,
            ),
            0.2,
            RED,
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
            ortho.scale = ortho.scale.clamp(0.001, 10.0);
        }
    }

    // Follow the car
    camera_transform.translation.x = car_transform.translation.x;
    camera_transform.translation.y = car_transform.translation.y;
}
