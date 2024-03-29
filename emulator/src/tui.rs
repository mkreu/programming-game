use color_eyre::eyre::{self, bail, Context};
use color_eyre::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    prelude::*,
    symbols::border,
    widgets::{
        block::{Position, Title},
        Block, Borders, Paragraph,
    },
};
use std::{
    io::{self, stdout, Stdout},
    panic,
};

use crate::cpu::instruction::Instruction;
use crate::cpu::Cpu;
use crate::dram::DRAM_SIZE;

use self::widgets::{CpuWidget, DramWidget};

mod widgets;

pub(crate) fn run(cpu: Cpu) -> Result<()> {
    let mut terminal = init()?;
    App::new(cpu).run(&mut terminal)?;
    restore()?;
    Ok(())
}

#[derive(Debug)]
pub struct App {
    cpu: Cpu,
    exit: bool,
}

impl App {
    pub fn new(cpu: Cpu) -> Self {
        Self { cpu, exit: false }
    }

    /// runs the application's main loop until the user quits
    pub fn run(&mut self, terminal: &mut Tui) -> Result<()> {
        while !self.exit {
            terminal.draw(|frame| self.render_frame(frame))?;
            self.handle_events().wrap_err("handle events failed")?;
        }
        Ok(())
    }

    fn render_frame(&self, frame: &mut Frame) {
        frame.render_widget(self, frame.size())
    }

    fn handle_events(&mut self) -> Result<()> {
        match event::read()? {
            // it's important to check that the event is a key press event as
            // crossterm also emits key release and repeat events on Windows.
            Event::Key(key_event) if key_event.kind == KeyEventKind::Press => self
                .handle_key_event(key_event)
                .wrap_err_with(|| format!("handling key event failed:\n{key_event:#?}")),
            _ => Ok(()),
        }
    }

    fn handle_key_event(&mut self, key_event: KeyEvent) -> Result<()> {
        match key_event.code {
            KeyCode::Char('q') => self.exit(),
            KeyCode::Enter => {
                let cpu = &mut self.cpu;
                // 1. Fetch.
                let inst = cpu.fetch();

                // 2. Add 4 to the program counter.
                cpu.pc = cpu.pc + 4;

                // 3. Decode.
                // 4. Execute.
                cpu.execute(inst);
            }
            _ => {}
        }
        Ok(())
    }

    // -- snip --

    fn exit(&mut self) {
        self.exit = true;
    }
}

impl Widget for &App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(33), Constraint::Percentage(33), Constraint::Percentage(33)])
            .split(area);

        let title = Title::from("Risc-V Cpu Simulator".bold());
        let instructions = Title::from(Line::from(vec![
            " Step ".into(),
            "<Enter>".blue().bold(),
            " Quit ".into(),
            "<Q> ".blue().bold(),
        ]));
        let block = Block::default()
            .title(title.alignment(Alignment::Center))
            .title(
                instructions
                    .alignment(Alignment::Center)
                    .position(Position::Bottom),
            )
            .borders(Borders::ALL)
            .border_set(border::THICK);

        let pc_text = Text::from(vec![
            Line::from(vec!["PC: ".into(), format!("0x{:x}",self.cpu.pc).yellow()]),
            Line::from(format!("Inst: {:?}", Instruction::parse(self.cpu.fetch()))),
        ]);

        Paragraph::new(pc_text)
            .centered()
            .block(block)
            .render(layout[0], buf);

        CpuWidget::new(&self.cpu).render(layout[1], buf);
        DramWidget::new(" Stack ", &self.cpu.dram, self.cpu.regs[2]).render(layout[2], buf);

    }
}


/// A type alias for the terminal type used in this application
pub type Tui = Terminal<CrosstermBackend<Stdout>>;

/// Initialize the terminal
pub fn init() -> Result<Tui> {
    install_hooks()?;
    execute!(stdout(), EnterAlternateScreen)?;
    enable_raw_mode()?;
    Ok(Terminal::new(CrosstermBackend::new(stdout()))?)
}

/// Restore the terminal to its original state
pub fn restore() -> io::Result<()> {
    execute!(stdout(), LeaveAlternateScreen)?;
    disable_raw_mode()?;
    Ok(())
}

/// This replaces the standard color_eyre panic and error hooks with hooks that
/// restore the terminal before printing the panic or error.
pub fn install_hooks() -> color_eyre::Result<()> {
    // add any extra configuration you need to the hook builder
    let hook_builder = color_eyre::config::HookBuilder::default();
    let (panic_hook, eyre_hook) = hook_builder.into_hooks();

    // convert from a color_eyre PanicHook to a standard panic hook
    let panic_hook = panic_hook.into_panic_hook();
    panic::set_hook(Box::new(move |panic_info| {
        restore().unwrap();
        panic_hook(panic_info);
    }));

    // convert from a color_eyre EyreHook to a eyre ErrorHook
    let eyre_hook = eyre_hook.into_eyre_hook();
    eyre::set_hook(Box::new(move |error| {
        restore().unwrap();
        eyre_hook(error)
    }))?;

    Ok(())
}
