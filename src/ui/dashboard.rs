use crate::system::info::SystemInfo;

use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    text::Line,
    widgets::{Block, Borders, Gauge, Paragraph},
    Frame,
};

fn usage_color(value: u16) -> Style {
    match value {
        0..=50 => Style::default().fg(Color::Green),
        51..=80 => Style::default().fg(Color::Yellow),
        _ => Style::default().fg(Color::Red),
    }
}

pub fn draw(frame: &mut Frame, info: &SystemInfo) {
    let main = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(35),
            Constraint::Percentage(35),
            Constraint::Percentage(30),
        ])
        .split(frame.area());

    let top = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(50),
            Constraint::Percentage(50),
        ])
        .split(main[0]);

    let middle = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(50),
            Constraint::Percentage(50),
        ])
        .split(main[1]);

    let cpu_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
        ])
        .split(top[0]);

    let cpu_info = Paragraph::new(format!(
        "{}\nCores: {}",
        info.cpu_name, info.cpu_cores
    ))
    .block(Block::default().title("CPU").borders(Borders::ALL));

    let cpu_percent = info.cpu_usage.clamp(0.0, 100.0) as u16;

    let cpu_gauge = Gauge::default()
        .block(Block::default().borders(Borders::ALL))
        .gauge_style(usage_color(cpu_percent))
        .percent(cpu_percent)
        .label(format!("{:.2}%", info.cpu_usage));

    let gpu_text = match &info.gpu {
        Some(gpu) => format!(
            "{}\nTemp: {}\nUso: {}\nVRAM: {}",
            gpu.name, gpu.temperature, gpu.usage, gpu.memory
        ),
        None => "GPU não encontrada".to_string(),
    };

    let gpu = Paragraph::new(gpu_text)
        .block(Block::default().title("GPU").borders(Borders::ALL));

    let ram_percent = if info.total_ram > 0.0 {
        ((info.used_ram / info.total_ram) * 100.0).clamp(0.0, 100.0) as u16
    } else {
        0
    };

    let ram = Gauge::default()
        .block(
            Block::default()
                .title(format!("RAM {:.1}/{:.1}GB", info.used_ram, info.total_ram))
                .borders(Borders::ALL),
        )
        .gauge_style(usage_color(ram_percent))
        .percent(ram_percent)
        .label(format!("{}%", ram_percent));

    let process_lines = info
        .processes_list
        .iter()
        .map(|p| Line::from(format!("{} {:.1}MB", p.name, p.memory)))
        .collect::<Vec<Line>>();

    let processes = Paragraph::new(process_lines)
        .block(Block::default().title("TOP MEMORY").borders(Borders::ALL));

    let disk_lines = info
        .disks
        .iter()
        .map(|disk| {
            let used = disk.total_space - disk.available_space;

            Line::from(format!(
                "{} {} {:.1}/{:.1}GB",
                disk.name, disk.mount_point, used, disk.total_space
            ))
        })
        .collect::<Vec<Line>>();

    let disks = Paragraph::new(disk_lines)
        .block(Block::default().title("DISKS").borders(Borders::ALL));

    frame.render_widget(cpu_info, cpu_layout[0]);
    frame.render_widget(cpu_gauge, cpu_layout[1]);
    frame.render_widget(gpu, top[1]);
    frame.render_widget(ram, middle[0]);
    frame.render_widget(processes, middle[1]);
    frame.render_widget(disks, main[2]);
}