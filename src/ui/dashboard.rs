use crate::system::info::SystemInfo;

use ratatui::{
    layout::{Constraint, Direction, Layout},
    text::Line,
    widgets::{Block, Borders, Gauge, Paragraph},
    Frame,
};

pub fn draw(frame: &mut Frame, info: &SystemInfo) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(30),
            Constraint::Percentage(30),
            Constraint::Percentage(40),
        ])
        .split(frame.area());

    let cpu_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Length(3)])
        .split(chunks[0]);

    let ram_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Length(3)])
        .split(chunks[1]);

    let cpu_text = format!("{}\nCores: {}", info.cpu_name, info.cpu_cores);

    let cpu_info = Paragraph::new(cpu_text).block(
        Block::default()
            .title("CPU")
            .borders(Borders::ALL),
    );

    let cpu_gauge = Gauge::default()
        .block(Block::default().borders(Borders::ALL))
        .percent(info.cpu_usage as u16)
        .label(format!("{:.2}%", info.cpu_usage));

    let ram_percent = ((info.used_ram / info.total_ram) * 100.0) as u16;

    let ram_text = format!("{:.2} GB / {:.2} GB", info.used_ram, info.total_ram);

    let ram_info = Paragraph::new(ram_text).block(
        Block::default()
            .title("MEMORY")
            .borders(Borders::ALL),
    );

    let ram_gauge = Gauge::default()
        .block(Block::default().borders(Borders::ALL))
        .percent(ram_percent)
        .label(format!("{}%", ram_percent));

    let disk_lines = info
        .disks
        .iter()
        .map(|disk| {
            let used_space = disk.total_space - disk.available_space;
            let used_percent = if disk.total_space > 0.0 {
                (used_space / disk.total_space) * 100.0
            } else {
                0.0
            };

            Line::from(format!(
                "{} | {} | usado: {:.2}% | {:.2} GB / {:.2} GB",
                disk.name,
                disk.mount_point,
                used_percent,
                used_space,
                disk.total_space
            ))
        })
        .collect::<Vec<Line>>();

    let disk = Paragraph::new(disk_lines).block(
        Block::default()
            .title("DISK")
            .borders(Borders::ALL),
    );

    frame.render_widget(cpu_info, cpu_chunks[0]);
    frame.render_widget(cpu_gauge, cpu_chunks[1]);

    frame.render_widget(ram_info, ram_chunks[0]);
    frame.render_widget(ram_gauge, ram_chunks[1]);

    frame.render_widget(disk, chunks[2]);
}