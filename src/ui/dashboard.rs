use crate::system::info::SystemInfo;

use ratatui::{
    layout::{Constraint, Direction, Layout},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

pub fn draw(frame: &mut Frame, info: &SystemInfo) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(33),
            Constraint::Percentage(33),
            Constraint::Percentage(34),
        ])
        .split(frame.area());

    let cpu_text = format!(
        "{}\nUso: {:.2}%\nCores: {}",
        info.cpu_name, info.cpu_usage, info.cpu_cores
    );

    let cpu = Paragraph::new(cpu_text).block(
        Block::default()
            .title("CPU")
            .borders(Borders::ALL),
    );

    let ram_text = format!(
        "{:.2} GB / {:.2} GB",
        info.used_ram, info.total_ram
    );

    let ram = Paragraph::new(ram_text).block(
        Block::default()
            .title("MEMORY")
            .borders(Borders::ALL),
    );

    let disk_text = info
        .disks
        .iter()
        .map(|disk| {
            format!(
                "{} | {} | livre: {:.2} GB / {:.2} GB",
                disk.name, disk.mount_point, disk.available_space, disk.total_space
            )
        })
        .collect::<Vec<String>>()
        .join("\n");

    let disk = Paragraph::new(disk_text).block(
        Block::default()
            .title("DISK")
            .borders(Borders::ALL),
    );

    frame.render_widget(cpu, chunks[0]);
    frame.render_widget(ram, chunks[1]);
    frame.render_widget(disk, chunks[2]);
}