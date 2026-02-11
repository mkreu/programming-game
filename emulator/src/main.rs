use emulator::CpuBuilder;
use emulator::cpu::{Dram, Hart, Instruction, LogDevice, Mmu, RamLike};
use std::env;
use std::fs;

fn main() {
    //tracing_subscriber::FmtSubscriber::builder()
    //    .with_max_level(LevelFilter::DEBUG)
    //    .init();

    let args: Vec<String> = env::args().collect();

    if args.len() != 2 {
        panic!("Usage: emulator <filename>");
    }
    let code = fs::read(&args[1]).unwrap();
    let (cpu, dram) = CpuBuilder::default().build(&code);

    run_plain(cpu, dram);
}

fn run_plain(mut cpu: Hart, mut dram: Dram) {
    let mut log = LogDevice;
    let mut devices: Vec<&mut dyn RamLike> = vec![&mut log];
    let mut mmu = Mmu::new(&mut dram, &mut devices);
    loop {
        // 1. Fetch.
        let inst = cpu.fetch(&mmu);

        // 2. Add 4 to the program counter.
        cpu.pc += 4;

        // 3. Decode.
        // 4. Execute.
        cpu.execute(Instruction::parse(inst), &mut mmu);
    }
}
