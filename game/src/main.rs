use std::f32::consts::PI;

use bevy::{
    diagnostic::FrameTimeDiagnosticsPlugin, math::vec2, prelude::*, sprite::{MaterialMesh2dBundle, Mesh2dHandle}
};
use cpu::{DrivingDevice, EmulatorPlugin, RadarDevice};
use iyes_perf_ui::{PerfUiCompleteBundle, PerfUiPlugin};
use noise::NoiseFn;

mod cpu;

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, FrameTimeDiagnosticsPlugin, PerfUiPlugin, EmulatorPlugin))
        .add_systems(Startup, setup)
        .add_systems(Update, (draw_cursor, draw_collider, draw_radar_sectors /*draw_voxel_grid*/))
        .add_systems(
            FixedUpdate,
            (radar_update, voxel_collision, robot_movement),
        )
        .run();
}

fn draw_cursor(
    camera_query: Query<(&Camera, &GlobalTransform)>,
    windows: Query<&Window>,
    mut gizmos: Gizmos,
) {
    let (camera, camera_transform) = camera_query.single();

    let Some(cursor_position) = windows.single().cursor_position() else {
        return;
    };

    // Calculate a world position based on the cursor's position.
    let Some(point) = camera.viewport_to_world_2d(camera_transform, cursor_position) else {
        return;
    };

    gizmos.circle_2d(point, 10., Color::WHITE);
}

fn draw_collider(query: Query<&Transform, With<Robot>>, mut gizmos: Gizmos) {
    for robot in query.iter() {
        gizmos.circle_2d(robot.translation.xy(), ROBOT_RADIUS, Color::CYAN);
    }
}

fn draw_voxel_grid(
    query: Query<(&Transform, &InheritedVisibility), With<Voxel>>,
    mut gizmos: Gizmos,
) {
    for voxel in query.iter() {
        if voxel.1.get() {
            gizmos.rect_2d(
                voxel.0.translation.xy(),
                0.,
                Vec2::new(VOXEL_RADIUS * 2. - 1., VOXEL_RADIUS * 2. - 1.),
                Color::WHITE,
            );
        }
    }
}

fn draw_radar_sectors(
    mut query: Query<(&RadarDevice, &Transform), (With<Robot>, Without<Voxel>)>,
    mut gizmos: Gizmos,
) {
    let (radar, transf) = query.single_mut();
    let robot_pos = transf.translation.xy();

    for i in 0..16 {
        let angle = i as f32 / 8. * PI;
        gizmos.ray_2d(robot_pos, vec2(angle.sin(), angle.cos()) * 1e5, Color::GRAY);
        let angle = angle + (PI / 16.);
        gizmos.ray_2d(robot_pos, vec2(angle.cos(), angle.sin()) * radar.sectors[i] as f32, Color::PINK)
    }
}

fn voxel_collision(
    robot_query: Query<&Transform, (With<Robot>, Without<Voxel>)>,
    mut voxel_query: Query<(&mut Visibility, &Transform), (With<Voxel>, Without<Robot>)>,
) {
    for mut voxel in voxel_query.iter_mut() {
        for robot in robot_query.iter() {
            if voxel.1.translation.xy().distance(robot.translation.xy()) < ROBOT_RADIUS + 0.1 {
                *voxel.0 = Visibility::Hidden;
            }
        }
    }
}

fn radar_update(
    mut robot_query: Query<(&mut RadarDevice, &Transform, &DrivingDevice), (With<Robot>, Without<Voxel>)>,
    voxel_query: Query<(&Voxel, &Transform, &InheritedVisibility), Without<Robot>>,
) {
    let (mut radar, transf, _driving) = robot_query.single_mut();
    let robot_pos = transf.translation.xy();

    let sectors_f = voxel_query
        .iter()
        .filter(|v| v.0.material == VoxelMaterial::ORE && v.2.get())
        .map(|v| v.1.translation.xy() - robot_pos)
        .map(|dir| {
            (
                Vec2::X.angle_between(dir),
                1. / dir.length_squared().max(1.),
            )
        })
        .fold([0.; 16], |mut acc, (angle, value)| {
            acc[((angle.to_degrees() / 22.5) +16.0) as usize % 16] += value;
            acc
        });

    let sectors = sectors_f
        .into_iter()
        .map(|f| (f * 1e4) as u8)
        .collect::<Vec<_>>()
        .try_into()
        .unwrap();

    radar.sectors = sectors;
}

fn robot_movement(
    mut robot_query: Query<(&DrivingDevice, &mut Transform), (With<Robot>, Without<Voxel>)>,
) {
    let (driving, mut robot) = robot_query.single_mut();

    let heading = (driving.heading as f32).to_radians();
    robot.rotation = Quat::from_rotation_z(heading);
    robot.translation += Vec3::new(heading.cos(), heading.sin(), 0.0) * (driving.speed as f32).min(1.);
}

fn manual_movement(
    mut robot_query: Query<&mut Transform, (With<Robot>, Without<Voxel>)>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
) {
    let mut robot = robot_query.single_mut();
    if keyboard_input.pressed(KeyCode::ArrowLeft) || keyboard_input.pressed(KeyCode::KeyA) {
        robot.translation.x -= 1.0;
        robot.rotation = Quat::from_axis_angle(Vec3::Z, 0.5 * PI);
    }
    if keyboard_input.pressed(KeyCode::ArrowRight) || keyboard_input.pressed(KeyCode::KeyD) {
        robot.translation.x += 1.0;
        robot.rotation = Quat::from_axis_angle(Vec3::Z, 1.5 * PI);
    }
    if keyboard_input.pressed(KeyCode::ArrowUp) || keyboard_input.pressed(KeyCode::KeyW) {
        robot.translation.y += 1.0;
        robot.rotation = Quat::from_axis_angle(Vec3::Z, 0.0);
    }
    if keyboard_input.pressed(KeyCode::ArrowDown) || keyboard_input.pressed(KeyCode::KeyS) {
        robot.translation.y -= 1.0;
        robot.rotation = Quat::from_axis_angle(Vec3::Z, PI);
    }
}

#[derive(Component)]
struct Robot;

#[derive(Component)]
struct Voxel {
    material: VoxelMaterial,
}

#[derive(PartialEq, Eq)]
pub enum VoxelMaterial {
    SAND,
    ORE,
}

const VOXEL_RADIUS: f32 = 2.0;
const VOXEL_COUNT: i32 = 150;
const ROBOT_RADIUS: f32 = 30.0;

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut gizmo_config: ResMut<GizmoConfigStore>,
) {
    commands.spawn(PerfUiCompleteBundle::default());

    gizmo_config
        .config_mut::<DefaultGizmoConfigGroup>()
        .0
        .line_width = 0.5;

    commands.spawn(Camera2dBundle::default());

    let voxel_mesh = Mesh2dHandle(meshes.add(Rectangle::new(
        VOXEL_RADIUS * 2.0 - 1.0,
        VOXEL_RADIUS * 2.0 - 1.0,
    )));
    let sand_material = materials.add(Color::DARK_GREEN);
    let ore_material = materials.add(Color::BLUE);
    let noise = noise::Simplex::new(0xDEADBEEF);

    for x in -VOXEL_COUNT..=VOXEL_COUNT {
        for y in -VOXEL_COUNT..=VOXEL_COUNT {
            let is_ore = noise.get([x as f64 / 100., y as f64 / 100.]) > 0.8;
            commands.spawn((
                if is_ore {
                    Voxel {
                        material: VoxelMaterial::ORE,
                    }
                } else {
                    Voxel {
                        material: VoxelMaterial::SAND,
                    }
                },
                MaterialMesh2dBundle {
                    mesh: voxel_mesh.clone(),
                    material: if is_ore {
                        ore_material.clone()
                    } else {
                        sand_material.clone()
                    },

                    transform: Transform::from_xyz(
                        x as f32 * (VOXEL_RADIUS * 2. - 1.),
                        y as f32 * (VOXEL_RADIUS * 2. - 1.),
                        -0.5,
                    ),
                    ..default()
                },
            ));
        }
    }
}
