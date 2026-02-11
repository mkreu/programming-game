use std::f32::consts::PI;

use avian2d::prelude::{forces::ForcesItem, *};
use bevy::{
    color::palettes::css::{GREEN, RED, WHITE, YELLOW},
    diagnostic::FrameTimeDiagnosticsPlugin,
    input::mouse::MouseWheel,
    prelude::*,
};
use iyes_perf_ui::{PerfUiPlugin, prelude::PerfUiDefaultEntries};

use racing::track;
use racing::track_format::TrackFile;

fn main() {
    let track_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "racing/assets/track1.toml".to_string());

    App::new()
        .add_plugins((
            DefaultPlugins,
            FrameTimeDiagnosticsPlugin::default(),
            PerfUiPlugin,
            PhysicsPlugins::default(),
            PhysicsDebugPlugin::default(),
        ))
        .insert_resource(Gravity::ZERO)
        .insert_resource(Time::<Fixed>::from_duration(
            std::time::Duration::from_secs_f32(1.0 / 200.0),
        ))
        .insert_resource(TrackPath(track_path))
        .add_systems(Startup, (setup, setup_track))
        .add_systems(Startup, set_default_zoom.after(setup))
        .add_systems(Update, (handle_car_input, update_ai_driver))
        .add_systems(FixedUpdate, apply_car_forces)
        .add_systems(Update, (update_camera, draw_gizmos))
        .run();
}

#[derive(Resource)]
struct TrackPath(String);

const WHEEL_BASE: f32 = 1.18;
const WHEEL_TRACK: f32 = 0.95;

fn setup_track(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    track_path: Res<TrackPath>,
) {
    let track_file = TrackFile::load(std::path::Path::new(&track_path.0))
        .expect(&format!("Failed to load track file: {}", track_path.0));

    let control_points = track_file.control_points_vec2();
    let track_width = track_file.metadata.track_width;
    let kerb_width = track_file.metadata.kerb_width;

    // Green ground plane
    commands.spawn((
        Mesh2d(meshes.add(Rectangle::new(800.0, 800.0))),
        MeshMaterial2d(materials.add(Color::srgb(0.2, 0.6, 0.2))),
        Transform::from_xyz(0.0, 0.0, -1.0),
    ));

    let spline = track::build_spline(&control_points);

    commands.insert_resource(track::TrackSpline {
        spline: spline.clone(),
    });

    // Track surface
    let track_mesh = track::create_track_mesh(&spline, track_width, 1000);
    commands.spawn((
        Mesh2d(meshes.add(track_mesh)),
        MeshMaterial2d(materials.add(Color::srgb(0.3, 0.3, 0.3))),
        Transform::from_xyz(0.0, 0.0, 0.0),
    ));

    // Kerbs
    let (inner_kerb, outer_kerb) = track::create_kerb_meshes(&spline, track_width, kerb_width, 1000);
    commands.spawn((
        Mesh2d(meshes.add(inner_kerb)),
        MeshMaterial2d(materials.add(ColorMaterial::default())),
        Transform::from_xyz(0.0, 0.0, 0.1),
    ));
    commands.spawn((
        Mesh2d(meshes.add(outer_kerb)),
        MeshMaterial2d(materials.add(ColorMaterial::default())),
        Transform::from_xyz(0.0, 0.0, 0.1),
    ));
}

fn setup(mut commands: Commands, asset_server: Res<AssetServer>, track_path: Res<TrackPath>) {
    commands.spawn(PerfUiDefaultEntries::default());

    // Spawn a camera; we'll set a custom default zoom once in `set_default_zoom`.
    commands.spawn(Camera2d);

    let track_file = TrackFile::load(std::path::Path::new(&track_path.0))
        .expect(&format!("Failed to load track file: {}", track_path.0));
    let start_point = track::first_point_from_file(&track_file);
    spawn_car(&mut commands, &asset_server, start_point + Vec2::new(0.0, 2.0), true);
    spawn_car(&mut commands, &asset_server, start_point + Vec2::new(1.0, -2.0), true);
    spawn_car(&mut commands, &asset_server, start_point + Vec2::new(2.0, 2.0), true);
    spawn_car(&mut commands, &asset_server, start_point + Vec2::new(3.0, -2.0), true);
    spawn_car(&mut commands, &asset_server, start_point + Vec2::new(4.0, 2.0), true);
    spawn_car(&mut commands, &asset_server, start_point + Vec2::new(5.0, -2.0), true);
    spawn_car(&mut commands, &asset_server, start_point + Vec2::new(6.0, 2.0), true);
    spawn_car(&mut commands, &asset_server, start_point + Vec2::new(7.0, -2.0), true);


}

fn set_default_zoom(mut camera_query: Query<&mut Projection, With<Camera2d>>) {
    let Ok(mut projection) = camera_query.single_mut() else {
        return;
    };

    if let Projection::Orthographic(ref mut ortho) = *projection {
        ortho.scale = 0.05;
    }
}

fn spawn_car(commands: &mut Commands, asset_server: &AssetServer, position: Vec2, is_ai: bool) {
    let sprite_scale = Vec3::splat(0.008);

    let mut entity = commands.spawn((
        Transform::from_xyz(position.x, position.y, 1.0).with_rotation(Quat::from_axis_angle(Vec3::Z, PI / 2.0)),
        Visibility::default(),
        RigidBody::Dynamic,
        LinearDamping(0.1),
        Friction::new(0.1),
        Restitution::new(0.2),
        Car {
            steer: 0.0,
            accelerator: 0.0,
            brake: 0.0,
        },
    ));

    if is_ai {
        entity.insert(AIDriver { target_t: 0.0 });
    }

    entity.with_children(|parent| {
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

#[derive(Component)]
struct Car {
    steer: f32,
    accelerator: f32,
    brake: f32,
}

#[derive(Component)]
struct AIDriver {
    target_t: f32,
}

#[derive(Component)]
struct FrontWheel;

fn handle_car_input(
    mut car_query: Query<&mut Car, Without<AIDriver>>,
    keyboard: Res<ButtonInput<KeyCode>>,
) {
    for mut car in &mut car_query {
        // Update accelerator and brake
        car.accelerator = if keyboard.pressed(KeyCode::KeyW) {
            1.0
        } else {
            0.0
        };
        car.brake = if keyboard.pressed(KeyCode::KeyS) {
            1.0
        } else {
            0.0
        };

        // Update steering
        let max_steer = PI / 6.0; // Max steering angle (30 degrees)
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
    }
}

fn update_ai_driver(
    mut ai_query: Query<(&Transform, &mut Car, &mut AIDriver, &LinearVelocity)>,
    track: Option<Res<track::TrackSpline>>,
    mut gizmos: Gizmos,
) {
    let Some(track) = track else {
        return;
    };

    let domain = track.spline.domain();
    let t_max = domain.end();

    for (transform, mut car, mut ai, velocity) in &mut ai_query {
        let car_pos = transform.translation.xy();
        let car_forward = transform.up().xy().normalize();
        let car_speed = velocity.length();

        // Instead of finding closest point, smoothly advance along the track
        // This prevents jumps and ensures forward progress only

        // Search a small window around current target_t to find where we actually are
        let mut best_t = ai.target_t;
        let mut best_score = f32::MAX;

        let window_samples = 50;
        let window_size = t_max * 0.1; // Search +/- 10% of track
        for i in 0..window_samples {
            let offset = (i as f32 / window_samples as f32) * window_size - window_size * 0.5;
            let test_t = (ai.target_t + offset + t_max) % t_max;
            let test_pos = track.spline.position(test_t);
            let dist = car_pos.distance(test_pos);

            // Prefer points ahead (positive offset) over points behind
            let forward_bias = if offset > 0.0 { 0.0 } else { 2.0 };
            let score = dist + forward_bias;

            if score < best_score {
                best_score = score;
                best_t = test_t;
            }
        }

        // Dynamic lookahead based on speed
        let base_lookahead = 2.0;
        let speed_factor = (car_speed * 0.5).max(1.0);
        let lookahead_distance = base_lookahead * speed_factor;

        let mut current_t = best_t;
        let mut traveled = 0.0;

        // Walk along the spline until we've traveled lookahead_distance
        while traveled < lookahead_distance {
            let step = t_max / 2000.0; // Smaller steps for smoother distance calculation
            let next_t = (current_t + step) % t_max;
            let p1 = track.spline.position(current_t);
            let p2 = track.spline.position(next_t);
            traveled += p1.distance(p2);
            current_t = next_t;
        }

        ai.target_t = current_t;

        // Calculate curvature ahead to determine braking
        let curvature_lookahead = 15.0; // Look further ahead for braking
        let mut curve_t = best_t;
        let mut curve_traveled = 0.0;

        // Sample points ahead to measure curvature
        let mut max_curvature: f32 = 0.0;

        while curve_traveled < curvature_lookahead {
            let step = t_max / 2000.0;
            let next_t = (curve_t + step) % t_max;
            let prev_t = if curve_t < step {
                t_max + curve_t - step
            } else {
                curve_t - step
            };

            // Calculate curvature using three points
            let p_prev = track.spline.position(prev_t);
            let p_curr = track.spline.position(curve_t);
            let p_next = track.spline.position(next_t);

            let v1 = (p_curr - p_prev).normalize();
            let v2 = (p_next - p_curr).normalize();

            // Angle change indicates curvature
            let angle_change = v1.angle_to(v2).abs();
            max_curvature = max_curvature.max(angle_change);

            curve_traveled += p_curr.distance(p_next);
            curve_t = next_t;
        }

        let target_pos = track.spline.position(ai.target_t);

        // Debug visualization
        gizmos.circle_2d(target_pos, 0.5, bevy::color::palettes::css::BLUE);
        gizmos.line_2d(car_pos, target_pos, bevy::color::palettes::css::AQUA);

        // Draw car forward direction
        gizmos.arrow_2d(
            car_pos,
            car_pos + car_forward * 3.0,
            bevy::color::palettes::css::LIME,
        );

        // Calculate steering to target
        let to_target = (target_pos - car_pos).normalize();
        let angle_to_target = car_forward.angle_to(to_target);

        // Smooth proportional steering with lower gain
        // Negate because physics uses -car.steer
        let max_steer = PI / 6.0;
        let desired_steer = (-angle_to_target * 0.8).clamp(-max_steer, max_steer);
        let steer_blend = 0.1; // How quickly to change steering (lower = smoother)
        car.steer = car.steer * (1.0 - steer_blend) + desired_steer * steer_blend;

        // Determine acceleration/braking based on curvature ahead
        let curvature_threshold_brake = 0.05; // Start braking at tight turns
        let curvature_threshold_caution = 0.02; // Reduce throttle at moderate turns

        if max_curvature > curvature_threshold_brake {
            // Sharp turn ahead - brake
            car.accelerator = 0.0;
            car.brake = ((max_curvature - curvature_threshold_brake) * 10.0).min(1.0);
        } else if max_curvature > curvature_threshold_caution {
            // Moderate turn - reduce throttle
            let throttle_reduction = (max_curvature - curvature_threshold_caution)
                / (curvature_threshold_brake - curvature_threshold_caution);
            car.accelerator = (1.0 - throttle_reduction * 0.7).max(0.3) * 0.1;
            car.brake = 0.0;
        } else {
            // Straight or gentle curve - full throttle
            car.accelerator = 1.0 * 0.1;
            car.brake = 0.0;
        }
    }
}

fn apply_car_forces(
    mut car_query: Query<(&Transform, &mut Car, &Children, Forces)>,
    mut wheel_query: Query<&mut Transform, (With<FrontWheel>, Without<Car>)>,
    mut gizmos: Gizmos,
) {
    for (transform, car, children, mut forces) in &mut car_query {
        // Car physics parameters
        let acceleration = 30.0;
        let braking = 50.0;

        let position = transform.translation.xy();
        let forward = transform.up().xy().normalize();
        let left = forward.perp();

        // Apply acceleration/braking based on car state
        if car.brake > 0.0 {
            forces.apply_linear_acceleration(forward * -braking * car.brake);
            gizmos.arrow_2d(
                position,
                position + forward * -braking * car.brake * 0.3,
                WHITE,
            );
        } else if car.accelerator > 0.0 {
            forces.apply_linear_acceleration(forward * acceleration * car.accelerator);
            gizmos.arrow_2d(
                position,
                position + forward * acceleration * car.accelerator * 0.3,
                WHITE,
            );
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
        apply_wheel_force(
            position,
            left * -WHEEL_TRACK / 2.0,
            forward,
            &mut forces,
            &mut gizmos,
        );
        // Rear right
        apply_wheel_force(
            position,
            left * WHEEL_TRACK / 2.0,
            forward,
            &mut forces,
            &mut gizmos,
        );

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
    let Some(car_transform) = car_query.iter().next() else {
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
