use bevy::{
    ecs::query::{QueryData, QueryItem},
    prelude::*,
};

use crate::CpuBuilder;
use crate::cpu::{Device, Instruction, Mmu};

#[macro_export]
macro_rules! define_cpu_config {
    ($vis:vis $name:ident { $($body:tt)+ }) => {
        $crate::define_cpu_config!(
            @collect
            [$vis $name]
            [
                __cpu_d0 __cpu_d1 __cpu_d2 __cpu_d3 __cpu_d4 __cpu_d5 __cpu_d6 __cpu_d7
                __cpu_d8 __cpu_d9 __cpu_d10 __cpu_d11 __cpu_d12 __cpu_d13 __cpu_d14 __cpu_d15
                __cpu_d16 __cpu_d17 __cpu_d18 __cpu_d19 __cpu_d20 __cpu_d21 __cpu_d22 __cpu_d23
                __cpu_d24 __cpu_d25 __cpu_d26 __cpu_d27 __cpu_d28 __cpu_d29 __cpu_d30 __cpu_d31
            ]
            []
            []
            []
            $($body)+
        );
    };

    (@collect
        [$vis:vis $name:ident]
        [$next_binding:ident $($remaining_bindings:ident)*]
        [$($types:ty,)*]
        [$($bindings:ident,)*]
        [$($entries:tt)*]
        $slot:expr => $device:ty, $($rest:tt)+
    ) => {
        $crate::define_cpu_config!(
            @collect
            [$vis $name]
            [$($remaining_bindings)*]
            [$($types,)* $device,]
            [$($bindings,)* $next_binding,]
            [$($entries)* ($slot, &mut *$next_binding as &mut dyn $crate::cpu::Device),]
            $($rest)+
        );
    };

    (@collect
        [$vis:vis $name:ident]
        [$next_binding:ident $($remaining_bindings:ident)*]
        [$($types:ty,)*]
        [$($bindings:ident,)*]
        [$($entries:tt)*]
        $slot:expr => $device:ty $(,)?
    ) => {
        $vis struct $name;

        impl $crate::bevy::CpuConfig for $name {
            type Devices = (
                $( &'static mut $types, )*
                &'static mut $device,
            );

            fn with_slotted_devices<'w, R>(
                devices: bevy::ecs::query::QueryItem<'w, 'w, Self::Devices>,
                f: impl FnOnce(&mut [(u8, &mut dyn $crate::cpu::Device)]) -> R,
            ) -> R {
                let (
                    $( mut $bindings, )*
                    mut $next_binding,
                ) = devices;

                let mut slotted = [
                    $($entries)*
                    ($slot, &mut *$next_binding as &mut dyn $crate::cpu::Device),
                ];

                f(&mut slotted)
            }
        }
    };
}

pub trait CpuConfig: Send + Sync + 'static {
    type Devices: QueryData;

    fn with_slotted_devices<'w, R>(
        devices: QueryItem<'w, 'w, Self::Devices>,
        f: impl FnOnce(&mut [(u8, &mut dyn Device)]) -> R,
    ) -> R;
}

#[derive(Component)]
pub struct CpuComponent {
    hart: crate::cpu::Hart,
    dram: crate::cpu::Dram,
    instructions_per_update: u32,
}

impl CpuComponent {
    /// Create a new CpuComponent from an ELF binary.
    pub fn new(elf: &[u8], instructions_per_update: u32) -> Self {
        let (hart, dram) = CpuBuilder::default().build(elf);
        Self {
            hart,
            dram,
            instructions_per_update,
        }
    }
}

fn run_one_instruction(cpu: &mut CpuComponent, device_refs: &mut [&mut dyn Device]) {
    let mut mmu = Mmu::new(&mut cpu.dram, device_refs);

    // 1. Fetch.
    let inst = cpu.hart.fetch(&mmu);

    // 2. Decode.
    let (decoded, len) = Instruction::parse_with_len(inst);
    // 3. Execute.
    cpu.hart.execute(decoded, len, &mut mmu);
}

fn run_cpu(cpu: &mut CpuComponent, device_refs: &mut [&mut dyn Device]) {
    for _ in 0..cpu.instructions_per_update {
        run_one_instruction(cpu, device_refs);
    }
}

pub fn cpu_system<C: CpuConfig>(mut cpu_query: Query<(&mut CpuComponent, C::Devices)>) {
    for (mut cpu, devices) in cpu_query.iter_mut() {
        C::with_slotted_devices(devices, |slotted| {
            slotted.sort_by_key(|entry| entry.0);

            for (i, entry) in slotted.iter().enumerate() {
                let expected = (i as u8) + 1;
                assert_eq!(
                    entry.0, expected,
                    "cpu config slot mismatch at position {i}: expected slot {}, got slot {}",
                    expected, entry.0
                );
            }

            let mut device_refs: Vec<&mut dyn Device> = Vec::with_capacity(slotted.len());
            for (_, device) in slotted.iter_mut() {
                device_refs.push(&mut **device);
            }
            run_cpu(cpu.as_mut(), &mut device_refs);
        });
    }
}
