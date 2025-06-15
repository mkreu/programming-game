use std::{
    any::TypeId,
    env,
    f32::consts::PI,
    fs,
    io::{stdout, Write},
};

use bevy::{
    prelude::*,
    sprite::{MaterialMesh2dBundle, Mesh2dHandle},
    utils::HashMap,
};

use emulator_core as emulator;

use emulator::{
    cpu::{instruction::Instruction, Cpu},
    dram::{Dram, DRAM_SIZE},
};

pub struct EmulatorPlugin;

impl Plugin for EmulatorPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_cpu);
        app.add_systems(
            FixedUpdate,
            (
                cpu_system,
                io_read_system::<RadarDevice>,
                io_write_system::<DrivingDevice>,
            ),
        );
    }
}

pub trait IODevice: Component {
    type Layout: IOMemoryLayout;
    fn write(&mut self, mem: &Self::Layout);
    fn read(&self) -> Self::Layout;
}

pub trait IOMemoryLayout {
    const LEN_BYTES: usize;
    fn from_bytes(mem: &[u8]) -> Self;
    fn into_bytes(&self, mem: &mut [u8]);
}

impl IOMemoryLayout for u16 {
    const LEN_BYTES: usize = 2;

    fn from_bytes(b: &[u8]) -> Self {
        u16::from_be_bytes([b[0], b[1]])
    }

    fn into_bytes(&self, mem: &mut [u8]) {
        mem.copy_from_slice(&self.to_be_bytes());
    }
}
impl<T1, T2> IOMemoryLayout for (T1, T2)
where
    T1: IOMemoryLayout,
    T2: IOMemoryLayout,
{
    const LEN_BYTES: usize = T1::LEN_BYTES + T2::LEN_BYTES;

    fn from_bytes(mem: &[u8]) -> Self {
        (
            T1::from_bytes(&mem[0..T1::LEN_BYTES]),
            T2::from_bytes(&mem[T1::LEN_BYTES..T1::LEN_BYTES + T2::LEN_BYTES]),
        )
    }

    fn into_bytes(&self, mem: &mut [u8]) {
        T1::into_bytes(&self.0, &mut mem[0..T1::LEN_BYTES]);
        T2::into_bytes(
            &self.1,
            &mut mem[T1::LEN_BYTES..T1::LEN_BYTES + T2::LEN_BYTES],
        );
    }
}
impl<const T: usize> IOMemoryLayout for [u8; T] {
    const LEN_BYTES: usize = T;

    fn from_bytes(mem: &[u8]) -> Self {
        let mut buf = [0; T];
        buf.copy_from_slice(mem);
        buf
    }

    fn into_bytes(&self, mem: &mut [u8]) {
        mem.copy_from_slice(self);
    }
}

use crate::Robot;

pub fn io_write_system<Device: IODevice>(mut query: Query<(&CpuComponent, &mut Device)>) {
    for (cpu, mut device) in query.iter_mut() {
        let offset = cpu.device_offsets[&TypeId::of::<Device>()];
        device.write(&Device::Layout::from_bytes(
            &cpu.cpu.dram.dram[offset..offset + Device::Layout::LEN_BYTES],
        ));
    }
}
pub fn io_read_system<Device: IODevice>(mut query: Query<(&mut CpuComponent, &Device)>) {
    for (mut cpu, device) in query.iter_mut() {
        let offset = cpu.device_offsets[&TypeId::of::<Device>()];
        device
            .read()
            .into_bytes(&mut cpu.cpu.dram.dram[offset..offset + Device::Layout::LEN_BYTES]);
    }
}

pub struct CpuComponentBuilder {
    mem_size: usize,
    instructions_per_update: u32,
    device_offsets: HashMap<TypeId, usize>,
    next_offset: usize,
}

impl CpuComponentBuilder {
    fn build(self, code: &[u8]) -> CpuComponent {
        let (mut dram, entry) = Dram::new(code);

        dram.store(self.mem_size as u32 - 4, 32, 4).unwrap();

        let cpu = Cpu::new(dram, entry);
        CpuComponent {
            cpu,
            instructions_per_update: self.instructions_per_update,
            device_offsets: self.device_offsets,
        }
    }
    fn with_device<Device: IODevice>(self) -> Self {
        let next_offset = self.next_offset + Device::Layout::LEN_BYTES;
        let mut device_offsets = self.device_offsets;
        device_offsets.insert(TypeId::of::<Device>(), self.next_offset);
        Self {
            next_offset,
            device_offsets,
            ..self
        }
    }
}

impl Default for CpuComponentBuilder {
    fn default() -> Self {
        Self {
            mem_size: DRAM_SIZE as usize,
            instructions_per_update: 100,
            device_offsets: Default::default(),
            next_offset: 8,
        }
    }
}

#[derive(Component)]
pub struct CpuComponent {
    cpu: Cpu,
    instructions_per_update: u32,
    device_offsets: HashMap<TypeId, usize>,
}

#[derive(Component)]
pub struct LogDevice {}

#[derive(Component)]
pub struct RadarDevice {
    pub sectors: [u8; 16],
}
impl IODevice for RadarDevice {
    type Layout = [u8; 16];

    fn write(&mut self, _mem: &Self::Layout) {
        // unused
    }

    fn read(&self) -> Self::Layout {
        self.sectors
    }
}

#[derive(Component)]
pub struct DrivingDevice {
    pub speed: u16,
    pub heading: u16,
}

impl IODevice for DrivingDevice {
    type Layout = (u16, u16);

    fn write(&mut self, mem: &Self::Layout) {
        self.speed = mem.0;
        self.heading = mem.1;
    }

    fn read(&self) -> Self::Layout {
        (self.speed, self.heading)
    }
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

    let cpu_comp = CpuComponentBuilder::default()
        .with_device::<RadarDevice>()
        .with_device::<DrivingDevice>()
        .build(&code);

    let rect_mesh = Mesh2dHandle(meshes.add(Rectangle::new(50.0, 25.0)));
    let triangle_mesh = Mesh2dHandle(meshes.add(Triangle2d::new(
        Vec2::Y * 25.0,
        Vec2::new(-25.0, 0.0),
        Vec2::new(25.0, 0.0),
    )));

    commands
        .spawn((
            Robot,
            cpu_comp,
            DrivingDevice {
                speed: 0,
                heading: 0,
            },
            RadarDevice { sectors: [0; 16] },
            TransformBundle::default(),
            InheritedVisibility::default(),
        ))
        .with_children(|cb| {
            cb.spawn(MaterialMesh2dBundle {
                mesh: rect_mesh,
                material: materials.add(Color::YELLOW),
                transform: Transform::from_xyz(-5.0, 0.0, 0.0)
                    .with_rotation(Quat::from_rotation_z(-PI / 2.)),
                ..default()
            });
            cb.spawn(MaterialMesh2dBundle {
                mesh: triangle_mesh,
                material: materials.add(Color::YELLOW),
                transform: Transform::from_xyz(7.5, 0.0, 0.0)
                    .with_rotation(Quat::from_rotation_z(-PI / 2.)),
                ..default()
            });
        });
}

pub fn cpu_system(mut cpu_query: Query<&mut CpuComponent>) {
    for mut cpu in cpu_query.iter_mut() {
        for _ in 0..cpu.instructions_per_update {
            let cpu = &mut cpu.cpu;

            // 1. Fetch.
            let inst = cpu.fetch();

            // 2. Add 4 to the program counter.
            cpu.pc += 4;

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
    }
}
