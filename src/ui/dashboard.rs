use crate::app::AppState;
use crate::system::info::SystemInfo;

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    text::Line,
    widgets::{Block, Borders, Gauge, Paragraph, Sparkline},
};

fn usage_color(value: u16) -> Style {
    match value {
        0..=50 => Style::default().fg(Color::Green),
        51..=80 => Style::default().fg(Color::Yellow),
        _ => Style::default().fg(Color::Red),
    }
}

fn format_uptime(seconds: u64) -> String {
    let days = seconds / 86_400;
    let hours = (seconds % 86_400) / 3_600;
    let minutes = (seconds % 3_600) / 60;

    if days > 0 {
        format!("{days}d {hours}h {minutes}m")
    } else {
        format!("{hours}h {minutes}m")
    }
}

pub fn draw(frame: &mut Frame, info: &SystemInfo, app: &AppState) {
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
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(main[0]);

    let middle = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(main[1]);

    let system_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(5), Constraint::Length(3)])
        .split(top[0]);

    let system_info = Paragraph::new(format!(
        "{} | Kernel {}\nCPU: {}\nCores: {} | Processos: {}\nUptime: {}",
        info.os_name,
        info.kernel,
        info.cpu_name,
        info.cpu_cores,
        info.processes,
        format_uptime(info.uptime)
    ))
    .block(Block::default().title("SYSTEM").borders(Borders::ALL));

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

    let gpu = Paragraph::new(gpu_text).block(Block::default().title("GPU").borders(Borders::ALL));

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

    let disks =
        Paragraph::new(disk_lines).block(Block::default().title("DISKS").borders(Borders::ALL));

    let cpu_history = Sparkline::default()
        .block(Block::default().title("CPU HISTORY").borders(Borders::ALL))
        .data(&app.cpu_history);

    let network_lines = info
        .networks
        .iter()
        .map(|net| {
            Line::from(format!(
                "{} ↓ {:.2}MB/s ↑ {:.2}MB/s",
                net.interface, net.received_per_sec, net.transmitted_per_sec
            ))
        })
        .collect::<Vec<Line>>();

    let network = Paragraph::new(network_lines)
        .block(Block::default().title("NETWORK").borders(Borders::ALL));

    frame.render_widget(system_info, system_layout[0]);
    frame.render_widget(cpu_gauge, system_layout[1]);
    frame.render_widget(gpu, top[1]);
    frame.render_widget(ram, middle[0]);
    frame.render_widget(processes, middle[1]);

    let bottom = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(33),
            Constraint::Percentage(33),
            Constraint::Percentage(34),
        ])
        .split(main[2]);

    frame.render_widget(disks, bottom[0]);
    frame.render_widget(cpu_history, bottom[1]);
    frame.render_widget(network, bottom[2]);
}
