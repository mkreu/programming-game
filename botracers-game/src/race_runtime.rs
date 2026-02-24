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

use botracers_game::Car;
use botracers_game::devices::TrackRadarBorders;
use botracers_game::devices::{
    self, CarControlsDevice, CarRadarDevice, CarStateDevice, SplineDevice, TrackRadarDevice,
};
use botracers_game::track;
use botracers_game::track_format::TrackFile;

use crate::game_api::{DriverType, SpawnResolvedCarRequest};

pub struct RaceRuntimePlugin;

impl Plugin for RaceRuntimePlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<SimState>()
            .insert_resource(Gravity::ZERO)
            .insert_resource(Time::<Fixed>::from_duration(
                std::time::Duration::from_secs_f32(1.0 / FIXED_TICK_HZ as f32),
            ))
            .insert_resource(RaceManager::default())
            .insert_resource(FollowCar::default())
            .insert_resource(KartLongitudinalParams::default())
            .insert_resource(CpuFrequencySetting::default())
            .add_systems(Startup, (setup_track, setup.after(setup_track)))
            .add_systems(Startup, set_default_zoom.after(setup))
            .add_systems(Startup, pause_physics)
            .add_systems(OnEnter(SimState::Racing), unpause_physics)
            .add_systems(OnEnter(SimState::Paused), pause_physics)
            .add_systems(OnEnter(SimState::PreRace), pause_physics)
            .add_systems(
                Update,
                (handle_spawn_resolved_event, apply_cpu_frequency_setting),
            )
            .add_systems(Update, handle_car_input)
            .configure_sets(
                FixedUpdate,
                (CpuSystems::PreCpu, CpuSystems::Cpu, CpuSystems::PostCpu).chain(),
            )
            .add_systems(
                FixedUpdate,
                (
                    devices::car_state_system.in_set(CpuSystems::PreCpu),
                    devices::car_radar_system.in_set(CpuSystems::PreCpu),
                    devices::track_radar_system.in_set(CpuSystems::PreCpu),
                    cpu_system::<RacingCpuConfig>.in_set(CpuSystems::Cpu),
                    devices::car_controls_system.in_set(CpuSystems::PostCpu),
                )
                    .run_if(in_state(SimState::Racing)),
            )
            .add_systems(
                FixedUpdate,
                apply_car_forces.run_if(in_state(SimState::Racing)),
            )
            .add_systems(Update, (update_fps_counter, update_camera, draw_gizmos));
    }
}

#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum SimState {
    #[default]
    PreRace,
    Racing,
    Paused,
}

#[derive(Resource)]
pub struct RaceManager {
    pub cars: Vec<CarEntry>,
    pub next_car_id: u32,
}

impl Default for RaceManager {
    fn default() -> Self {
        Self {
            cars: Vec::new(),
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

#[derive(Resource, Default)]
pub struct FollowCar {
    pub target: Option<Entity>,
}

pub const FIXED_TICK_HZ: u32 = 200;
const CPU_FREQUENCY_PRESETS_HZ: [u32; 10] = [
    1_000, 5_000, 10_000, 20_000, 50_000, 100_000, 200_000, 500_000, 1_000_000, 2_000_000,
];

#[derive(Resource, Clone, Copy)]
pub struct CpuFrequencySetting {
    preset_index: usize,
}

impl Default for CpuFrequencySetting {
    fn default() -> Self {
        Self {
            preset_index: CPU_FREQUENCY_PRESETS_HZ.len() - 1,
        }
    }
}

impl CpuFrequencySetting {
    pub fn hz(&self) -> u32 {
        CPU_FREQUENCY_PRESETS_HZ[self.preset_index]
    }

    pub fn instructions_per_update(&self) -> u32 {
        (self.hz() / FIXED_TICK_HZ).max(1)
    }

    pub fn step_up(&mut self) {
        if self.preset_index + 1 < CPU_FREQUENCY_PRESETS_HZ.len() {
            self.preset_index += 1;
        }
    }

    pub fn step_down(&mut self) {
        if self.preset_index > 0 {
            self.preset_index -= 1;
        }
    }

    pub fn format_hz_label(&self) -> String {
        let hz = self.hz();
        if hz >= 1_000_000 {
            format!("{:.1} MHz", hz as f32 / 1_000_000.0)
        } else {
            format!("{} kHz", hz / 1_000)
        }
    }
}

#[derive(Component)]
pub struct CarLabel {
    pub name: String,
}

#[derive(Component)]
pub struct DebugGizmos;

#[derive(Component, Default, Clone)]
pub struct LongitudinalDebugData {
    pub speed_mps: f32,
    pub engine_rpm: f32,
    pub wheel_rpm: f32,
    pub clutch_s: f32,
    pub t_eng: f32,
    pub t_drive_axle: f32,
    pub t_brake_axle: f32,
    pub f_drive: f32,
    pub f_brake: f32,
    pub f_rr: f32,
    pub f_drag: f32,
    pub f_raw: f32,
    pub f_clamped: f32,
    pub a_mps2: f32,
    pub traction_limit: f32,
    pub throttle: f32,
    pub brake: f32,
}

#[derive(Resource, Clone, Copy)]
struct KartLongitudinalParams {
    mass_kg: f32,
    wheel_radius_m: f32,
    gear_ratio: f32,
    drivetrain_efficiency: f32,
    tire_mu: f32,
    rolling_resistance: f32,
    air_density: f32,
    drag_area: f32,
    torque_peak_nm: f32,
    torque_peak_rpm: f32,
    redline_torque_fraction: f32,
    idle_rpm: f32,
    clutch_on_rpm: f32,
    clutch_lock_rpm: f32,
    redline_rpm: f32,
    engine_brake_nm: f32,
    brake_max_axle_nm: f32,
    sync_rate: f32,
    free_rev_rate: f32,
}

impl Default for KartLongitudinalParams {
    fn default() -> Self {
        Self {
            mass_kg: 165.0,
            wheel_radius_m: 0.13,
            gear_ratio: 5.0,
            drivetrain_efficiency: 0.9,
            tire_mu: 1.0,
            rolling_resistance: 0.015,
            air_density: 1.225,
            drag_area: 0.75,
            torque_peak_nm: 22.0,
            torque_peak_rpm: 2800.0,
            redline_torque_fraction: 0.6,
            idle_rpm: 1800.0,
            clutch_on_rpm: 2100.0,
            clutch_lock_rpm: 2600.0,
            redline_rpm: 6200.0,
            engine_brake_nm: 3.0,
            brake_max_axle_nm: 400.0,
            sync_rate: 40.0,
            free_rev_rate: 10.0,
        }
    }
}

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
enum CpuSystems {
    PreCpu,
    Cpu,
    PostCpu,
}

#[cfg(test)]
mod tests {
    use super::{
        CpuFrequencySetting, KartLongitudinalParams, engine_torque_full, governor_scale, smoothstep,
    };

    #[test]
    fn cpu_frequency_setting_clamps_at_boundaries() {
        let mut setting = CpuFrequencySetting::default();
        setting.step_up();
        assert_eq!(setting.hz(), 2_000_000);

        for _ in 0..20 {
            setting.step_down();
        }
        assert_eq!(setting.hz(), 1_000);

        setting.step_down();
        assert_eq!(setting.hz(), 1_000);
    }

    #[test]
    fn cpu_frequency_setting_maps_to_instruction_budget() {
        let setting = CpuFrequencySetting::default();
        assert_eq!(setting.instructions_per_update(), 10_000);

        let mut setting = CpuFrequencySetting::default();
        for _ in 0..6 {
            setting.step_down();
        }
        assert_eq!(setting.hz(), 20_000);
        assert_eq!(setting.instructions_per_update(), 100);
    }

    #[test]
    fn cpu_frequency_setting_formats_labels() {
        let setting = CpuFrequencySetting::default();
        assert_eq!(setting.format_hz_label(), "2.0 MHz");

        let mut setting = CpuFrequencySetting::default();
        for _ in 0..6 {
            setting.step_down();
        }
        assert_eq!(setting.format_hz_label(), "20 kHz");
    }

    #[test]
    fn smoothstep_clamps_and_is_monotonic() {
        assert_eq!(smoothstep(2.0, 4.0, 1.0), 0.0);
        assert_eq!(smoothstep(2.0, 4.0, 5.0), 1.0);

        let mut prev = 0.0;
        for i in 0..=20 {
            let x = 2.0 + (i as f32) * 0.1;
            let y = smoothstep(2.0, 4.0, x);
            assert!((0.0..=1.0).contains(&y));
            assert!(y >= prev - 1e-6);
            prev = y;
        }
    }

    #[test]
    fn torque_curve_peaks_near_target_and_drops_off() {
        let params = KartLongitudinalParams::default();
        let near_peak = engine_torque_full(params.torque_peak_rpm, &params);
        let low = engine_torque_full(1200.0, &params);
        let high = engine_torque_full(5200.0, &params);
        assert!(near_peak > low);
        assert!(near_peak > high);
    }

    #[test]
    fn governor_reduces_torque_above_redline() {
        let params = KartLongitudinalParams::default();
        assert_eq!(governor_scale(params.redline_rpm, &params), 1.0);
        assert!(governor_scale(params.redline_rpm + 250.0, &params) < 1.0);
        assert_eq!(governor_scale(params.redline_rpm + 1000.0, &params), 0.0);
    }

    #[test]
    fn traction_clamp_enforces_limit() {
        let params = KartLongitudinalParams::default();
        let limit = params.tire_mu * params.mass_kg * 9.81;
        let clamped = (limit * 3.0).clamp(-limit, limit);
        assert!(clamped <= limit);
        assert!(clamped >= -limit);
    }
}

const WHEEL_BASE: f32 = 1.18;
const WHEEL_TRACK: f32 = 0.95;

fn setup_track(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    let track_file =
        TrackFile::load_builtin().unwrap_or_else(|_| panic!("Failed to load track file"));

    let control_points = track_file.control_points_vec2();
    let track_width = track_file.metadata.track_width;
    let kerb_width = track_file.metadata.kerb_width;

    commands.spawn((
        Mesh2d(meshes.add(Rectangle::new(800.0, 800.0))),
        MeshMaterial2d(materials.add(Color::srgb(0.2, 0.6, 0.2))),
        Transform::from_xyz(0.0, 0.0, -1.0),
    ));

    let spline = track::build_spline(&control_points);

    commands.insert_resource(track::TrackSpline {
        spline: spline.clone(),
    });
    let (inner_border, outer_border) = track::sample_track_borders(&spline, track_width, 1000);
    commands.insert_resource(TrackRadarBorders {
        inner: inner_border,
        outer: outer_border,
    });

    let track_mesh = track::create_track_mesh(&spline, track_width, 1000);
    commands.spawn((
        Mesh2d(meshes.add(track_mesh)),
        MeshMaterial2d(materials.add(Color::srgb(0.3, 0.3, 0.3))),
        Transform::from_xyz(0.0, 0.0, 0.0),
    ));

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

fn rpm_to_rad_per_sec(rpm: f32) -> f32 {
    rpm * (2.0 * PI / 60.0)
}

fn rad_per_sec_to_rpm(rad_per_sec: f32) -> f32 {
    rad_per_sec * (60.0 / (2.0 * PI))
}

fn smoothstep(edge0: f32, edge1: f32, value: f32) -> f32 {
    if edge1 <= edge0 {
        return if value < edge0 { 0.0 } else { 1.0 };
    }
    let x = ((value - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

fn engine_torque_full(rpm: f32, params: &KartLongitudinalParams) -> f32 {
    let x = ((rpm - params.torque_peak_rpm) / (params.redline_rpm - params.torque_peak_rpm)).clamp(0.0, 1.0);
    params.torque_peak_nm * (1.0 - (1.0 - params.redline_torque_fraction) * x * x)
}

fn governor_scale(rpm: f32, params: &KartLongitudinalParams) -> f32 {
    if rpm <= params.redline_rpm {
        1.0
    } else {
        (1.0 - (rpm - params.redline_rpm) / 500.0).clamp(0.0, 1.0)
    }
}

fn setup(mut commands: Commands) {
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

fn grid_offset(index: usize) -> Vec2 {
    let row = index as f32;
    let side = if index % 2 == 0 { 1.0 } else { -1.0 };
    Vec2::new(row * 2.0, side * 2.0)
}

fn handle_spawn_resolved_event(
    mut events: MessageReader<SpawnResolvedCarRequest>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    track_spline: Res<track::TrackSpline>,
    mut manager: ResMut<RaceManager>,
    cpu_frequency: Res<CpuFrequencySetting>,
    state: Res<State<SimState>>,
) {
    for event in events.read() {
        if *state.get() != SimState::PreRace {
            continue;
        }

        spawn_car_entry(
            &mut commands,
            &asset_server,
            &track_spline,
            &mut manager,
            &cpu_frequency,
            event.driver.clone(),
            &event.elf_bytes,
        );
    }
}

fn spawn_car_entry(
    commands: &mut Commands,
    asset_server: &AssetServer,
    track_spline: &track::TrackSpline,
    manager: &mut RaceManager,
    cpu_frequency: &CpuFrequencySetting,
    driver: DriverType,
    elf_bytes: &[u8],
) {
    let car_index = manager.cars.len();
    let offset = grid_offset(car_index);

    let track_file =
        TrackFile::load_builtin().unwrap_or_else(|_| panic!("Failed to load track file"));
    let start_point = track::first_point_from_file(&track_file);

    let position = start_point + offset;
    let car_name = format!("Car {}", manager.next_car_id);
    let entity = spawn_car(
        commands,
        asset_server,
        position,
        track_spline,
        &car_name,
        elf_bytes,
        cpu_frequency.instructions_per_update(),
    );
    manager.cars.push(CarEntry {
        entity,
        name: car_name,
        driver,
        console_output: String::new(),
    });
    manager.next_car_id += 1;
}

fn spawn_car(
    commands: &mut Commands,
    asset_server: &AssetServer,
    position: Vec2,
    track_spline: &track::TrackSpline,
    name: &str,
    bot_elf: &[u8],
    instructions_per_update: u32,
) -> Entity {
    let sprite_scale = Vec3::splat(0.008);

    let mut entity = commands.spawn((
        Transform::from_xyz(position.x, position.y, 1.0)
            .with_rotation(Quat::from_axis_angle(Vec3::Z, PI / 2.0)),
        Visibility::default(),
        RigidBody::Dynamic,
        //LinearDamping(0.1),
        Friction::new(0.1),
        Restitution::new(0.2),
        Car {
            steer: 0.0,
            accelerator: 0.0,
            brake: 0.0,
            engine_rpm: 1800.0,
            wheel_omega: 0.0,
        },
        CarLabel {
            name: name.to_string(),
        },
        LongitudinalDebugData::default(),
    ));

    let cpu = CpuComponent::new(bot_elf, instructions_per_update);
    entity.insert((
        EmulatorDriver,
        cpu,
        LogDevice::default(),
        CarStateDevice::default(),
        CarControlsDevice::default(),
        SplineDevice::new(track_spline),
        TrackRadarDevice::default(),
        CarRadarDevice::default(),
    ));

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

fn apply_cpu_frequency_setting(
    cpu_frequency: Res<CpuFrequencySetting>,
    mut cpu_query: Query<&mut CpuComponent>,
) {
    if !cpu_frequency.is_changed() {
        return;
    }

    let instructions_per_update = cpu_frequency.instructions_per_update();
    for mut cpu in &mut cpu_query {
        cpu.set_instructions_per_update(instructions_per_update);
    }
}

#[derive(Component)]
struct EmulatorDriver;

#[derive(Component)]
struct FrontWheel;

emulator::define_cpu_config! {
    RacingCpuConfig {
        1 => LogDevice,
        2 => CarStateDevice,
        3 => CarControlsDevice,
        4 => SplineDevice,
        5 => TrackRadarDevice,
        6 => CarRadarDevice,
    }
}

fn handle_car_input(
    mut car_query: Query<&mut Car, Without<EmulatorDriver>>,
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

fn apply_car_forces(
    mut car_query: Query<(
        Entity,
        &Transform,
        &mut Car,
        &mut LongitudinalDebugData,
        &Children,
        Forces,
        Has<DebugGizmos>,
    )>,
    mut wheel_query: Query<&mut Transform, (With<FrontWheel>, Without<Car>)>,
    mut gizmos: Gizmos,
    params: Res<KartLongitudinalParams>,
    time: Res<Time<Fixed>>,
) {
    let dt = time.delta_secs();
    let g = 9.81_f32;

    for (_entity, transform, mut car, mut debug_data, children, mut forces, show_gizmos) in
        &mut car_query
    {
        let position = transform.translation.xy();
        let forward = transform.up().xy().normalize();
        let left = forward.perp();
        let throttle = car.accelerator.clamp(0.0, 1.0);
        let brake = car.brake.clamp(0.0, 1.0);
        let v_long = forces.linear_velocity().dot(forward);

        car.wheel_omega = v_long / params.wheel_radius_m;
        let wheel_rpm = rad_per_sec_to_rpm(car.wheel_omega.abs());

        let engine_rpm_prev = car.engine_rpm.max(params.idle_rpm);
        let engine_torque_full = engine_torque_full(engine_rpm_prev, &params);
        let mut t_eng = throttle * engine_torque_full - (1.0 - throttle) * params.engine_brake_nm;
        t_eng *= governor_scale(engine_rpm_prev, &params);

        let clutch_s = smoothstep(
            params.clutch_on_rpm,
            params.clutch_lock_rpm,
            engine_rpm_prev,
        );
        let t_drive_axle =
            params.drivetrain_efficiency * params.gear_ratio * clutch_s * t_eng.max(0.0);
        let t_brake_axle = brake * params.brake_max_axle_nm;

        let f_drive = t_drive_axle / params.wheel_radius_m;
        let f_brake = t_brake_axle / params.wheel_radius_m;
        let f_rr = params.rolling_resistance * params.mass_kg * g;
        let f_drag_mag = 0.5 * params.air_density * params.drag_area * v_long * v_long;
        let v_sign = if v_long.abs() < 0.05 {
            0.0
        } else {
            v_long.signum()
        };
        // Rolling resistance should oppose motion, not create reverse acceleration from rest.
        let rr_sign = if v_long.abs() < 0.05 {
            0.0
        } else {
            v_long.signum()
        };
        let f_raw = f_drive - f_brake - rr_sign * f_rr - v_sign * f_drag_mag;
        let traction_limit = params.tire_mu * params.mass_kg * g;
        let mut f_clamped = f_raw.clamp(-traction_limit, traction_limit);

        // Prevent low-speed sign-flip jitter while braking/coasting to a stop.
        if v_long.abs() < 0.1 && f_clamped < 0.0 {
            f_clamped = 0.0;
        }

        let a_long = f_clamped / params.mass_kg;
        forces.apply_linear_acceleration(forward * a_long);

        let omega_lock = params.gear_ratio * car.wheel_omega;
        let omega_idle = rpm_to_rad_per_sec(params.idle_rpm);
        let omega_max = rpm_to_rad_per_sec(params.redline_rpm);
        let omega_target = omega_idle + throttle * (omega_max - omega_idle);
        let mut omega_engine = rpm_to_rad_per_sec(engine_rpm_prev);
        omega_engine += params.sync_rate * clutch_s * (omega_lock - omega_engine) * dt;
        omega_engine +=
            params.free_rev_rate * (1.0 - clutch_s) * (omega_target - omega_engine) * dt;
        let omega_ceiling = rpm_to_rad_per_sec(params.redline_rpm + 500.0);
        omega_engine = omega_engine.clamp(omega_idle, omega_ceiling);
        car.engine_rpm = rad_per_sec_to_rpm(omega_engine);

        debug_data.speed_mps = v_long;
        debug_data.engine_rpm = car.engine_rpm;
        debug_data.wheel_rpm = wheel_rpm;
        debug_data.clutch_s = clutch_s;
        debug_data.t_eng = t_eng;
        debug_data.t_drive_axle = t_drive_axle;
        debug_data.t_brake_axle = t_brake_axle;
        debug_data.f_drive = f_drive;
        debug_data.f_brake = f_brake;
        debug_data.f_rr = f_rr;
        debug_data.f_drag = f_drag_mag;
        debug_data.f_raw = f_raw;
        debug_data.f_clamped = f_clamped;
        debug_data.a_mps2 = a_long;
        debug_data.traction_limit = traction_limit;
        debug_data.throttle = throttle;
        debug_data.brake = brake;

        if show_gizmos {
            gizmos.arrow_2d(position, position + forward * a_long * 0.3, WHITE);
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

    if let Some(follow_entity) = follow.target {
        if let Ok(car_tf) = car_query.get(follow_entity) {
            camera_transform.translation.x = car_tf.translation.x;
            camera_transform.translation.y = car_tf.translation.y;
            return;
        }
    }

    if mouse_buttons.pressed(MouseButton::Middle) || mouse_buttons.pressed(MouseButton::Right) {
        for event in motion_events.read() {
            camera_transform.translation.x -= event.delta.x * current_scale;
            camera_transform.translation.y += event.delta.y * current_scale;
        }
    }
}
