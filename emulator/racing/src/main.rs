use std::f32::consts::PI;

use avian2d::prelude::*;
use bevy::{
    color::palettes::css::{RED, WHITE, YELLOW}, diagnostic::FrameTimeDiagnosticsPlugin,
    input::mouse::MouseWheel, prelude::*,
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
        .add_systems(Startup, (setup, track::setup))
        .add_systems(Startup, set_default_zoom.after(setup))
        .add_systems(FixedUpdate, drive_car)
        .add_systems(Update, (update_camera, draw_gizmos))
        .run();
}

const WHEEL_BASE: f32 = 3.5;

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
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
            RigidBody::Dynamic,
            LinearDamping(0.8),
            AngularDamping(0.8),
            Car { steer: 0.0 },
        ))
        .with_children(|parent| {
            parent.spawn((
                Collider::rectangle(1.8, 5.0),
                Transform::from_xyz(0.0, 2.1, 0.0),
            ));

            parent.spawn((
                Sprite::from_image(asset_server.load("blue_car_without_wheels.png")),
                Transform::from_xyz(0.0, 2.05, 0.1).with_scale(sprite_scale),
            ));

            // Front left wheel
            parent
                .spawn((
                    Transform::from_xyz(-0.75, 3.5, 0.1),
                    Visibility::default(),
                    FrontWheel,
                ))
                .with_children(|parent| {
                    parent.spawn((
                        Sprite::from_image(asset_server.load("wheel.png")),
                        Transform::default()
                            .with_scale(sprite_scale)
                            .with_rotation(Quat::from_rotation_z(0.0)),
                    ));
                });
            // Front right wheel
            parent
                .spawn((
                    Transform::from_xyz(0.75, 3.5, 0.1),
                    Visibility::default(),
                    FrontWheel,
                ))
                .with_children(|parent| {
                    parent.spawn((
                        Sprite::from_image(asset_server.load("wheel.png")),
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

fn drive_car(
    mut car_query: Query<(&Transform, &mut Car, &Children, Forces)>,
    mut wheel_query: Query<&mut Transform, (With<FrontWheel>, Without<Car>)>,
    mut gizmos: Gizmos,
    keyboard: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();

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
            gizmos.arrow_2d(
                position,
                position + forward * -braking * 0.3,
                WHITE,
            );
        } else if keyboard.pressed(KeyCode::KeyW) {
            forces.apply_linear_acceleration(forward * acceleration);
            gizmos.arrow_2d(
                position,
                position + forward * acceleration * 0.3,
                WHITE,
            );
        }


        let max_steer = PI / 6.0; // Max steering angle (30 degrees);
        if keyboard.pressed(KeyCode::KeyA) {
            car.steer -= max_steer * dt * 5.0 / (1.0 + forces.linear_velocity().length() * 0.1);
        }
        else if keyboard.pressed(KeyCode::KeyD) {
            car.steer += max_steer * dt * 5.0 / (1.0 + forces.linear_velocity().length() * 0.1);
        } else {
            // Return wheels to center when no input
            car.steer -= car.steer.signum() * max_steer * dt * 5.0 / (1.0 + forces.linear_velocity().length() * 0.1);
        }
        car.steer = car.steer.clamp(-max_steer, max_steer);
        if car.steer.abs() < 0.001 {
            car.steer = 0.0;
        }

        let wheel_pos = position + forward * WHEEL_BASE;

        let wheel_forward = Vec2::from_angle(-car.steer).rotate(forward);
        gizmos.arrow_2d(
            wheel_pos,
            wheel_pos + wheel_forward * 2.0,
            YELLOW,
        );

        let wheel_left = wheel_forward.perp();
        gizmos.arrow_2d(
            wheel_pos,
            wheel_pos + wheel_left * 1.0,
            YELLOW,
        );

        if forces.linear_velocity().length() > 1.0 {

            let front_force = -forces.linear_velocity().dot(wheel_left).clamp(-5.0, 5.0) * wheel_left;
            let back_force = -forces.linear_velocity().dot(left).clamp(-5.0, 5.0) * left;

            gizmos.arrow_2d(
                wheel_pos,
                wheel_pos + front_force,
                RED,
            );
            gizmos.arrow_2d(
                position,
                position + back_force,
                RED,
            );

            forces.apply_linear_acceleration_at_point(
                front_force,
                wheel_pos,
            );

            forces.apply_linear_acceleration_at_point(
                back_force,
                position,
            );
        }

        // Update wheel rotation
        for child in children.iter() {
            if let Ok(mut wheel_transform) = wheel_query.get_mut(child) {
                wheel_transform.rotation = Quat::from_rotation_z(-car.steer);
            }
        }
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
            ortho.scale = ortho.scale.clamp(0.01, 5.0);
        }
    }

    // Follow the car
    camera_transform.translation.x = car_transform.translation.x;
    camera_transform.translation.y = car_transform.translation.y;
}
