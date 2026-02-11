use emulator::CpuBuilder;
use emulator::cpu::{Hart, Instruction, Mmu};
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
    let mmu = Mmu { dram };

    run_plain(cpu, mmu);
}

fn run_plain(mut cpu: Hart, mut mmu: Mmu) {
    loop {
        // 1. Fetch.
        let inst = cpu.fetch(&mut mmu);

        // 2. Add 4 to the program counter.
        cpu.pc += 4;

        // 3. Decode.
        // 4. Execute.
        cpu.execute(Instruction::parse(inst), &mut mmu);
    }
}
