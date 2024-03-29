use ratatui::prelude::*;
use ratatui::symbols::border;
use ratatui::widgets::block::{Position, Title};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};

use crate::cpu::Cpu;
use crate::dram::Dram;

pub struct CpuWidget<'a> {
    cpu: &'a Cpu,
}

impl<'a> CpuWidget<'a> {
    pub fn new(cpu: &'a Cpu) -> Self {
        Self { cpu }
    }
}

impl Widget for CpuWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized,
    {
        let title = Title::from(" Cpu Registers ".bold());
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

        let pc_text = Text::from(
            self.cpu
                .regs
                .iter()
                .enumerate()
                .map(|(i, val)| {
                    format!(
                        "x{i:02}: {:>2} | {:>10} | {:>10x} |",
                        reg_name(i),
                        *val as i32,
                        val
                    )
                    .into()
                })
                .collect::<Vec<Line>>(),
        );

        Paragraph::new(pc_text)
            .centered()
            .block(block)
            .render(area, buf);
    }
}
fn reg_name(reg: usize) -> &'static str {
    match reg {
        0 => "z",
        1 => "ra",
        2 => "sp",
        3 => "gp",
        4 => "tp",
        5 => "t0",
        6 => "t1",
        7 => "t2",
        8 => "s0",
        9 => "s1",
        10 => "a0",
        11 => "a1",
        12 => "a2",
        13 => "a3",
        14 => "a4",
        15 => "a5",
        16 => "a6",
        17 => "a7",
        18 => "s2",
        19 => "s3",
        20 => "s4",
        21 => "s5",
        22 => "s6",
        23 => "s7",
        24 => "s8",
        25 => "s9",
        26 => "s10",
        27 => "s11",
        28 => "t3",
        29 => "t4",
        30 => "t5",
        31 => "t6",
        _ => panic!("invalid register"),
    }
}

pub struct DramWidget<'a> {
    name: &'static str,
    dram: &'a Dram,
    start_offset: u32,
}

impl<'a> DramWidget<'a> {
    pub fn new(name: &'static str, dram: &'a Dram, start_offset: u32) -> Self {
        Self { name, dram, start_offset }
    }
}

impl Widget for DramWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized,
    {
        let title = Title::from(self.name.bold());
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

        let pc_text = Text::from(
            self.dram
                .dram
                .chunks_exact(4)
                .enumerate()
                .skip(self.start_offset as usize / 4)
                .take(area.height as usize)
                .map(|(i, chunk)| match chunk {
                    &[b0, b1, b2, b3] => format!("{:x}: {b0:02x} {b1:02x} {b2:02x} {b3:02x} {:>10}", i*4, b0 as u32 | (b1 as u32) << 8 | (b2 as u32) << 16 | (b3 as u32) << 24).into(),
                    _ => unreachable!(),
                })
                .collect::<Vec<Line>>(),
        );

        Paragraph::new(pc_text)
            .centered()
            .block(block)
            .render(area, buf);
    }
}
