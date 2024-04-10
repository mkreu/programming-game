use color_eyre::Result;
use emulator::cpu::instruction::Instruction;
use emulator::cpu::Cpu;
use emulator::CpuBuilder;
use std::env;
use std::fs;

fn main() -> Result<()> {
    //tracing_subscriber::FmtSubscriber::builder()
    //    .with_max_level(LevelFilter::DEBUG)
    //    .init();

    let args: Vec<String> = env::args().collect();

    if args.len() != 2 {
        panic!("Usage: emulator <filename>");
    }
    let code = fs::read(&args[1])?;
    let cpu = CpuBuilder::default().build(&code);

    run_plain(cpu);

    //tui::run(cpu)
    Ok(())
}

fn run_plain(mut cpu: Cpu) {
    loop {
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
        }
    }
}
