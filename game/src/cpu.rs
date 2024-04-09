use std::{
    env, f32::consts::PI, fs, io::{stdout, Write}
};

use bevy::{
    prelude::*,
    sprite::{MaterialMesh2dBundle, Mesh2dHandle},
};

use emulator::{
    cpu::{instruction::Instruction, Cpu},
    dram::{Dram, DRAM_SIZE},
};

use crate::Robot;

#[derive(Component)]
pub struct CpuComponent {
    cpu: Cpu,
    instructions_per_update: u32,
}

#[derive(Component)]
pub struct Radar {
    pub sectors: [u8; 16],
}

#[derive(Component)]
pub struct Driving {
    pub speed: u16,
    pub heading: u16,
}

pub fn spawn_cpu(
    mut commands: Commands,

    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    let args: Vec<String> = env::args().collect();

    if args.len() != 2 {
        panic!("Usage: emulator <filename>");
    }
    let code = fs::read(&args[1]).unwrap();

    let (mut dram, entry) = Dram::new(code);

    dram.store(DRAM_SIZE - 4, 32, 4).unwrap();

    let cpu = Cpu::new(dram, entry);

    let rect_mesh = Mesh2dHandle(meshes.add(Rectangle::new(50.0, 25.0)));
    let triangle_mesh = Mesh2dHandle(meshes.add(Triangle2d::new(
        Vec2::Y * 25.0,
        Vec2::new(-25.0, 0.0),
        Vec2::new(25.0, 0.0),
    )));

    commands
        .spawn((
            Robot,
            CpuComponent {
                cpu,
                instructions_per_update: 100,
            },
            Driving {
                speed: 0,
                heading: 0,
            },
            Radar { sectors: [0; 16] },
            TransformBundle::default(),
            InheritedVisibility::default(),
        ))
        .with_children(|cb| {
            cb.spawn(MaterialMesh2dBundle {
                mesh: rect_mesh,
                material: materials.add(Color::YELLOW),
                transform: Transform::from_xyz(-5.0, 0.0, 0.0).with_rotation(Quat::from_rotation_z(- PI/2.)),
                ..default()
            });
            cb.spawn(MaterialMesh2dBundle {
                mesh: triangle_mesh,
                material: materials.add(Color::YELLOW),
                transform: Transform::from_xyz(7.5, 0.0, 0.0).with_rotation(Quat::from_rotation_z(- PI/2.)),
                ..default()
            });
        });
}
const BASE_ADDR: usize = 4;
const LOG_ADDR: usize = BASE_ADDR;
const LOG_SIZE: usize = 4;
const RADAR_ADDR: usize = LOG_ADDR + LOG_SIZE;
const RADAR_SIZE: usize = 16;
const DRIVING_ADDR: usize = LOG_ADDR + LOG_SIZE + RADAR_SIZE;

pub fn cpu_system(mut cpu_query: Query<(&mut CpuComponent, &Radar, &mut Driving)>) {
    for (mut cpu, radar, mut driving) in cpu_query.iter_mut() {
        cpu.cpu.dram.dram[RADAR_ADDR..RADAR_ADDR + RADAR_SIZE].clone_from_slice(&radar.sectors);

        for _ in 0..cpu.instructions_per_update {
            let cpu = &mut cpu.cpu;

            // 1. Fetch.
            let inst = cpu.fetch();

            // 2. Add 4 to the program counter.
            cpu.pc = cpu.pc + 4;

            // 3. Decode.
            // 4. Execute.
            cpu.execute(Instruction::parse(inst));

            let print = cpu.dram.load(4, 32).unwrap();
            cpu.dram.store(4, 32, 0).unwrap();

            if print != 0 {
                print!("{}", char::from_u32(print).unwrap());
                stdout().flush().unwrap()
            }
        }
        let ram = &cpu.cpu.dram.dram[0..64];
        let b = &cpu.cpu.dram.dram[DRIVING_ADDR..DRIVING_ADDR + 4];
        driving.speed = u16::from_be_bytes([b[0], b[1]]);
        driving.heading = u16::from_be_bytes([b[2], b[3]]);
    }
}
