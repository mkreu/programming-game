use std::f32::consts::PI;

use avian2d::prelude::{forces::ForcesItem, *};
use bevy::{
    color::palettes::css::{GREEN, RED, WHITE, YELLOW},
    diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin},
    input::mouse::{MouseMotion, MouseWheel},
    prelude::*,
};
use emulator::bevy::{CpuComponent, cpu_system};
use emulator::cpu::LogDevice;

use racing::devices::{CarControlsDevice, CarStateDevice, SplineDevice};
use racing::track;
use racing::track_format::TrackFile;

mod ui;

/// Pre-built RISC-V ELF binary for the bot car AI.
const BOT_ELF: &[u8] = include_bytes!("../../bot/target/riscv32imafc-unknown-none-elf/release/car");

// Re-export types used by the UI module.
pub(crate) use main_game::*;

/// All game-specific types live here so `ui` can import them via `crate::main_game::*`.
mod main_game {
    use super::*;

    // ── Simulation state ────────────────────────────────────────────────

    #[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
    pub enum SimState {
        #[default]
        PreRace,
        Racing,
        Paused,
    }

    // ── Events ──────────────────────────────────────────────────────────

    #[derive(Message)]
    pub struct SpawnCarRequest {
        pub driver: DriverType,
    }

    // ── Resources ───────────────────────────────────────────────────────

    #[derive(Resource)]
    pub struct RaceManager {
        pub cars: Vec<CarEntry>,
        pub selected_driver: DriverType,
        pub next_car_id: u32,
    }

    impl Default for RaceManager {
        fn default() -> Self {
            Self {
                cars: Vec::new(),
                selected_driver: DriverType::NativeAI,
                next_car_id: 1,
            }
        }
    }

    pub struct CarEntry {
        pub entity: Entity,
        pub name: String,
        pub driver: DriverType,
        pub console_output: String,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum DriverType {
        NativeAI,
        Emulator,
    }

    #[derive(Resource, Default)]
    pub struct FollowCar {
        pub target: Option<Entity>,
    }

    // ── Components ──────────────────────────────────────────────────────

    #[derive(Component)]
    pub struct CarLabel {
        pub name: String,
    }

    /// Marker: when present on a car entity, debug gizmos are drawn for it.
    #[derive(Component)]
    pub struct DebugGizmos;
}

fn main() {
    let track_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "racing/assets/track1.toml".to_string());

    App::new()
        .add_plugins((
            DefaultPlugins,
            FrameTimeDiagnosticsPlugin::default(),
            PhysicsPlugins::default(),
            ui::RaceUiPlugin,
        ))
        .init_state::<SimState>()
        .add_message::<SpawnCarRequest>()
        .insert_resource(Gravity::ZERO)
        .insert_resource(Time::<Fixed>::from_duration(
            std::time::Duration::from_secs_f32(1.0 / 200.0),
        ))
        .insert_resource(TrackPath(track_path))
        .insert_resource(RaceManager::default())
        .insert_resource(FollowCar::default())
        .add_systems(Startup, (setup_track, setup.after(setup_track)))
        .add_systems(Startup, set_default_zoom.after(setup))
        // Pause/unpause avian2d physics based on SimState
        .add_systems(Startup, pause_physics)
        .add_systems(OnEnter(SimState::Racing), unpause_physics)
        .add_systems(OnEnter(SimState::Paused), pause_physics)
        .add_systems(OnEnter(SimState::PreRace), pause_physics)
        // Spawning: always active so cars can be added in PreRace
        .add_systems(Update, handle_spawn_car_event)
        // Keyboard driving: always active (only affects non-AI, non-emulator cars)
        .add_systems(Update, handle_car_input)
        // AI + emulator: only run while Racing
        .add_systems(
            FixedUpdate,
            update_ai_driver.run_if(in_state(SimState::Racing)),
        )
        .add_systems(
            FixedUpdate,
            (
                update_emulator_driver.before(cpu_system),
                cpu_system,
                apply_emulator_controls.after(cpu_system),
            )
                .run_if(in_state(SimState::Racing)),
        )
        // Physics forces: only while Racing
        .add_systems(
            FixedUpdate,
            apply_car_forces.run_if(in_state(SimState::Racing)),
        )
        .add_systems(Update, (update_fps_counter, update_camera, draw_gizmos))
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
        .unwrap_or_else(|_| panic!("Failed to load track file: {}", track_path.0));

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
    let (inner_kerb, outer_kerb) =
        track::create_kerb_meshes(&spline, track_width, kerb_width, 1000);
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

fn setup(mut commands: Commands) {
    // FPS counter
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(8.0),
            left: Val::Px(8.0),
            padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)),
        Text::new("FPS: --"),
        TextFont {
            font_size: 18.0,
            ..default()
        },
        TextColor(WHITE.into()),
        FpsCounterText,
    ));

    // Camera — starts free (not following any car)
    commands.spawn(Camera2d);
}

#[derive(Component)]
struct FpsCounterText;

fn update_fps_counter(
    diagnostics: Res<DiagnosticsStore>,
    mut query: Query<&mut Text, With<FpsCounterText>>,
) {
    let Ok(mut text) = query.single_mut() else {
        return;
    };

    if let Some(fps) = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FPS)
        .and_then(|value| value.smoothed())
    {
        text.0 = format!("FPS: {fps:>3.0}");
    }
}

fn set_default_zoom(mut camera_query: Query<&mut Projection, With<Camera2d>>) {
    let Ok(mut projection) = camera_query.single_mut() else {
        return;
    };

    if let Projection::Orthographic(ref mut ortho) = *projection {
        ortho.scale = 0.05;
    }
}

fn pause_physics(mut physics_time: ResMut<Time<Physics>>) {
    physics_time.pause();
}

fn unpause_physics(mut physics_time: ResMut<Time<Physics>>) {
    physics_time.unpause();
}

// ── Starting grid positions ─────────────────────────────────────────────────

/// Return the staggered grid offset for the Nth car (0-indexed).
fn grid_offset(index: usize) -> Vec2 {
    let row = index as f32;
    let side = if index % 2 == 0 { 1.0 } else { -1.0 };
    Vec2::new(row * 2.0, side * 2.0)
}

// ── Car spawning via event ──────────────────────────────────────────────────

fn handle_spawn_car_event(
    mut events: MessageReader<SpawnCarRequest>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    track_path: Res<TrackPath>,
    track_spline: Res<track::TrackSpline>,
    mut manager: ResMut<RaceManager>,
) {
    for event in events.read() {
        let car_index = manager.cars.len();
        let offset = grid_offset(car_index);

        let track_file = TrackFile::load(std::path::Path::new(&track_path.0))
            .unwrap_or_else(|_| panic!("Failed to load track file: {}", track_path.0));
        let start_point = track::first_point_from_file(&track_file);

        let position = start_point + offset;
        let car_name = format!("Car {}", manager.next_car_id);
        let entity = spawn_car(
            &mut commands,
            &asset_server,
            position,
            event.driver,
            &track_spline,
            &car_name,
        );
        manager.cars.push(CarEntry {
            entity,
            name: car_name,
            driver: event.driver,
            console_output: String::new(),
        });
        manager.next_car_id += 1;
    }
}

fn spawn_car(
    commands: &mut Commands,
    asset_server: &AssetServer,
    position: Vec2,
    driver: DriverType,
    track_spline: &track::TrackSpline,
    name: &str,
) -> Entity {
    let sprite_scale = Vec3::splat(0.008);

    let mut entity = commands.spawn((
        Transform::from_xyz(position.x, position.y, 1.0)
            .with_rotation(Quat::from_axis_angle(Vec3::Z, PI / 2.0)),
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
        CarLabel {
            name: name.to_string(),
        },
    ));

    match driver {
        DriverType::NativeAI => {
            entity.insert(AIDriver { target_t: 0.0 });
        }
        DriverType::Emulator => {
            let devices: Vec<Box<dyn emulator::cpu::RamLike>> = vec![
                Box::new(LogDevice::new()),
                Box::new(CarStateDevice::default()),
                Box::new(CarControlsDevice::default()),
                Box::new(SplineDevice::new(track_spline)),
            ];
            let cpu = CpuComponent::new(BOT_ELF, devices, 10000);
            entity.insert((EmulatorDriver, cpu));
        }
    }

    let entity_id = entity.id();

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

    entity_id
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
struct EmulatorDriver;

#[derive(Component)]
struct FrontWheel;

fn handle_car_input(
    mut car_query: Query<&mut Car, (Without<AIDriver>, Without<EmulatorDriver>)>,
    keyboard: Res<ButtonInput<KeyCode>>,
) {
    for mut car in &mut car_query {
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

        let max_steer = PI / 6.0;
        let steer_rate = 0.05 * car.steer.abs().max(0.1);
        if keyboard.pressed(KeyCode::KeyA) {
            car.steer = (-max_steer).max(car.steer - steer_rate);
        } else if keyboard.pressed(KeyCode::KeyD) {
            car.steer = max_steer.min(car.steer + steer_rate);
        } else {
            car.steer = if car.steer > 0.0 {
                (car.steer - steer_rate).max(0.0)
            } else {
                (car.steer + steer_rate).min(0.0)
            };
        }
    }
}

fn update_ai_driver(
    mut ai_query: Query<(
        Entity,
        &Transform,
        &mut Car,
        &mut AIDriver,
        &LinearVelocity,
        Has<DebugGizmos>,
    )>,
    track: Option<Res<track::TrackSpline>>,
    mut gizmos: Gizmos,
) {
    let Some(track) = track else {
        return;
    };

    let domain = track.spline.domain();
    let t_max = domain.end();

    for (_entity, transform, mut car, mut ai, velocity, show_gizmos) in &mut ai_query {
        let car_pos = transform.translation.xy();
        let car_forward = transform.up().xy().normalize();
        let car_speed = velocity.length();

        let mut best_t = ai.target_t;
        let mut best_score = f32::MAX;

        let window_samples = 50;
        let window_size = t_max * 0.1;
        for i in 0..window_samples {
            let offset = (i as f32 / window_samples as f32) * window_size - window_size * 0.5;
            let test_t = (ai.target_t + offset + t_max) % t_max;
            let test_pos = track.spline.position(test_t);
            let dist = car_pos.distance(test_pos);

            let forward_bias = if offset > 0.0 { 0.0 } else { 2.0 };
            let score = dist + forward_bias;

            if score < best_score {
                best_score = score;
                best_t = test_t;
            }
        }

        let base_lookahead = 2.0;
        let speed_factor = (car_speed * 0.5).max(1.0);
        let lookahead_distance = base_lookahead * speed_factor;

        let mut current_t = best_t;
        let mut traveled = 0.0;

        while traveled < lookahead_distance {
            let step = t_max / 2000.0;
            let next_t = (current_t + step) % t_max;
            let p1 = track.spline.position(current_t);
            let p2 = track.spline.position(next_t);
            traveled += p1.distance(p2);
            current_t = next_t;
        }

        ai.target_t = current_t;

        let curvature_lookahead = 15.0;
        let mut curve_t = best_t;
        let mut curve_traveled = 0.0;
        let mut max_curvature: f32 = 0.0;

        while curve_traveled < curvature_lookahead {
            let step = t_max / 2000.0;
            let next_t = (curve_t + step) % t_max;
            let prev_t = if curve_t < step {
                t_max + curve_t - step
            } else {
                curve_t - step
            };

            let p_prev = track.spline.position(prev_t);
            let p_curr = track.spline.position(curve_t);
            let p_next = track.spline.position(next_t);

            let v1 = (p_curr - p_prev).normalize();
            let v2 = (p_next - p_curr).normalize();

            let angle_change = v1.angle_to(v2).abs();
            max_curvature = max_curvature.max(angle_change);

            curve_traveled += p_curr.distance(p_next);
            curve_t = next_t;
        }

        let target_pos = track.spline.position(ai.target_t);

        if show_gizmos {
            gizmos.circle_2d(target_pos, 0.5, bevy::color::palettes::css::BLUE);
            gizmos.line_2d(car_pos, target_pos, bevy::color::palettes::css::AQUA);
            gizmos.arrow_2d(
                car_pos,
                car_pos + car_forward * 3.0,
                bevy::color::palettes::css::LIME,
            );
        }

        let to_target = (target_pos - car_pos).normalize();
        let angle_to_target = car_forward.angle_to(to_target);

        let max_steer = PI / 6.0;
        let desired_steer = (-angle_to_target * 0.8).clamp(-max_steer, max_steer);
        let steer_blend = 0.1;
        car.steer = car.steer * (1.0 - steer_blend) + desired_steer * steer_blend;

        let curvature_threshold_brake = 0.05;
        let curvature_threshold_caution = 0.02;

        if max_curvature > curvature_threshold_brake {
            car.accelerator = 0.0;
            car.brake = ((max_curvature - curvature_threshold_brake) * 10.0).min(1.0);
        } else if max_curvature > curvature_threshold_caution {
            let throttle_reduction = (max_curvature - curvature_threshold_caution)
                / (curvature_threshold_brake - curvature_threshold_caution);
            car.accelerator = (1.0 - throttle_reduction * 0.7).max(0.3) * 0.1;
            car.brake = 0.0;
        } else {
            car.accelerator = 1.0 * 0.1;
            car.brake = 0.0;
        }
    }
}

/// Runs BEFORE cpu_system: writes car state into the emulator's CarStateDevice.
fn update_emulator_driver(
    mut emu_query: Query<(&Transform, &LinearVelocity, &mut CpuComponent), With<EmulatorDriver>>,
) {
    for (transform, velocity, mut cpu) in &mut emu_query {
        let car_pos = transform.translation.xy();
        let car_forward = transform.up().xy().normalize();
        let car_speed = velocity.length();

        if let Some(state_dev) = cpu.device_as_mut::<CarStateDevice>(1) {
            state_dev.update(car_speed, car_pos, car_forward);
        }
    }
}

/// Runs AFTER cpu_system: reads the bot's control outputs and applies them.
fn apply_emulator_controls(mut emu_query: Query<(&mut Car, &CpuComponent), With<EmulatorDriver>>) {
    for (mut car, cpu) in &mut emu_query {
        if let Some(ctrl_dev) = cpu.device_as::<CarControlsDevice>(2) {
            car.accelerator = ctrl_dev.accelerator();
            car.brake = ctrl_dev.brake();
            car.steer = ctrl_dev.steering();
        }
    }
}

fn apply_car_forces(
    mut car_query: Query<(
        Entity,
        &Transform,
        &mut Car,
        &Children,
        Forces,
        Has<DebugGizmos>,
    )>,
    mut wheel_query: Query<&mut Transform, (With<FrontWheel>, Without<Car>)>,
    mut gizmos: Gizmos,
) {
    for (_entity, transform, car, children, mut forces, show_gizmos) in &mut car_query {
        let acceleration = 30.0;
        let braking = 50.0;

        let position = transform.translation.xy();
        let forward = transform.up().xy().normalize();
        let left = forward.perp();

        if car.brake > 0.0 {
            forces.apply_linear_acceleration(forward * -braking * car.brake);
            if show_gizmos {
                gizmos.arrow_2d(
                    position,
                    position + forward * -braking * car.brake * 0.3,
                    WHITE,
                );
            }
        } else if car.accelerator > 0.0 {
            forces.apply_linear_acceleration(forward * acceleration * car.accelerator);
            if show_gizmos {
                gizmos.arrow_2d(
                    position,
                    position + forward * acceleration * car.accelerator * 0.3,
                    WHITE,
                );
            }
        }

        apply_wheel_force(
            position,
            forward * WHEEL_BASE + left * -WHEEL_TRACK / 2.0,
            Vec2::from_angle(-car.steer).rotate(forward),
            &mut forces,
            &mut gizmos,
            show_gizmos,
        );
        apply_wheel_force(
            position,
            forward * WHEEL_BASE + left * WHEEL_TRACK / 2.0,
            Vec2::from_angle(-car.steer).rotate(forward),
            &mut forces,
            &mut gizmos,
            show_gizmos,
        );
        apply_wheel_force(
            position,
            left * -WHEEL_TRACK / 2.0,
            forward,
            &mut forces,
            &mut gizmos,
            show_gizmos,
        );
        apply_wheel_force(
            position,
            left * WHEEL_TRACK / 2.0,
            forward,
            &mut forces,
            &mut gizmos,
            show_gizmos,
        );

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
    show_gizmos: bool,
) {
    let wheel_pos = car_position + wheel_offset;
    let wheel_left = wheel_forward.perp();

    if show_gizmos {
        gizmos.arrow_2d(wheel_pos, wheel_pos + wheel_forward * 1.0, YELLOW);
        gizmos.arrow_2d(wheel_pos, wheel_pos + wheel_left * 0.5, YELLOW);
    }

    let o = forces.angular_velocity();
    let l = forces.linear_velocity();
    let wow = wheel_pos - car_position;
    let wheel_velocity = l + Vec2::new(-o * wow.y, o * wow.x);

    if show_gizmos {
        gizmos.arrow_2d(wheel_pos, wheel_pos + wheel_velocity * 0.1, GREEN);
    }

    if wheel_velocity.length() > 0.1 {
        let force = -wheel_velocity.normalize().dot(wheel_left)
            * wheel_left
            * 10.0_f32.min(wheel_velocity.length() * 5.0);
        if show_gizmos {
            gizmos.arrow_2d(wheel_pos, wheel_pos + force, RED);
        }
        forces.apply_linear_acceleration_at_point(force, wheel_pos);
    }
}

fn draw_gizmos(car_query: Query<(&Transform, &Car), With<DebugGizmos>>, mut gizmos: Gizmos) {
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
    mut motion_events: MessageReader<MouseMotion>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    follow: Res<FollowCar>,
) {
    let Ok((mut camera_transform, mut projection)) = camera_query.single_mut() else {
        return;
    };

    let mut current_scale = 0.05_f32;
    if let Projection::Orthographic(ref mut ortho) = *projection {
        for event in scroll_events.read() {
            let zoom_delta = match event.unit {
                bevy::input::mouse::MouseScrollUnit::Line => event.y * 0.1,
                bevy::input::mouse::MouseScrollUnit::Pixel => event.y * 0.001,
            };

            ortho.scale *= 1.0 - zoom_delta;
            ortho.scale = ortho.scale.clamp(0.001, 10.0);
        }
        current_scale = ortho.scale;
    }

    // If following a car, snap camera to it
    if let Some(follow_entity) = follow.target {
        if let Ok(car_tf) = car_query.get(follow_entity) {
            camera_transform.translation.x = car_tf.translation.x;
            camera_transform.translation.y = car_tf.translation.y;
            return; // Skip manual panning when following
        }
    }

    // Free camera: middle-mouse or right-mouse drag to pan
    if mouse_buttons.pressed(MouseButton::Middle) || mouse_buttons.pressed(MouseButton::Right) {
        for event in motion_events.read() {
            camera_transform.translation.x -= event.delta.x * current_scale;
            camera_transform.translation.y += event.delta.y * current_scale;
        }
    }
}
