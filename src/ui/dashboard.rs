use crate::app::AppState;
use crate::system::info::SystemInfo;

use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    symbols,
    text::{Line, Span},
    widgets::{Axis, Block, BorderType, Borders, Chart, Dataset, GraphType, Padding, Paragraph},
};

const ACCENT: Color = Color::Cyan;
const ACCENT_ALT: Color = Color::Magenta;
const OK: Color = Color::Green;
const WARN: Color = Color::Yellow;
const DANGER: Color = Color::Red;
const BACKGROUND: Color = Color::Black;
const SURFACE: Color = Color::Black;
const TEXT: Color = Color::Gray;

fn usage_color(value: u16) -> Style {
    match value {
        0..=50 => Style::default().fg(OK),
        51..=80 => Style::default().fg(WARN),
        _ => Style::default().fg(DANGER).add_modifier(Modifier::BOLD),
    }
}

fn muted() -> Style {
    Style::default().fg(Color::DarkGray)
}

fn label() -> Style {
    Style::default().fg(Color::DarkGray)
}

fn value() -> Style {
    Style::default().fg(Color::White)
}

fn brand() -> Style {
    Style::default()
        .fg(Color::Black)
        .bg(ACCENT)
        .add_modifier(Modifier::BOLD)
}

fn chip(text: impl Into<String>, style: Style) -> Span<'static> {
    Span::styled(format!(" {} ", text.into()), style)
}

fn card(title: impl Into<String>) -> Block<'static> {
    Block::default()
        .title(Span::styled(
            format!("  {}  ", title.into()),
            Style::default().fg(ACCENT_ALT).add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::DarkGray))
        .style(Style::default().bg(SURFACE))
        .padding(Padding::horizontal(1))
}

fn fill(color: Color) -> Block<'static> {
    Block::default().style(Style::default().bg(color))
}

fn metric_line(label_text: impl Into<String>, value_text: impl Into<String>) -> Line<'static> {
    Line::from(vec![
        Span::styled(label_text.into(), label()),
        Span::styled(value_text.into(), value()),
    ])
}

fn section_header(columns: Vec<(&str, usize)>) -> Line<'static> {
    Line::from(
        columns
            .into_iter()
            .map(|(text, width)| {
                Span::styled(
                    format!("{text:<width$}"),
                    Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
                )
            })
            .collect::<Vec<Span>>(),
    )
}

fn meter(value: u16, width: usize) -> Vec<Span<'static>> {
    let filled = ((value as usize * width) / 100).min(width);
    vec![
        Span::styled("[", muted()),
        Span::styled("#".repeat(filled), usage_color(value)),
        Span::styled("-".repeat(width - filled), muted()),
        Span::styled("]", muted()),
    ]
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

fn usage_percent(used: f64, total: f64) -> f64 {
    if total > 0.0 {
        (used / total * 100.0).clamp(0.0, 100.0)
    } else {
        0.0
    }
}

fn disk_usage(total: f64, available: f64) -> (f64, f64) {
    let used = (total - available).max(0.0);
    (used, usage_percent(used, total))
}

pub fn draw(frame: &mut Frame, info: &SystemInfo, app: &AppState) {
    frame.render_widget(fill(BACKGROUND), frame.area());

    let page = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4),
            Constraint::Min(10),
            Constraint::Length(1),
        ])
        .margin(1)
        .spacing(1)
        .split(frame.area());

    for area in page.iter() {
        frame.render_widget(fill(BACKGROUND), *area);
    }

    let title = Paragraph::new(Line::from(vec![
        Span::raw(" "),
        chip("SYSMON", brand()),
        Span::raw("  "),
        chip(
            format!("CPU {:.1}%", info.cpu_usage),
            usage_color(info.cpu_usage as u16).add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        chip(
            format!("RAM {:.1}/{:.1}GB", info.used_ram, info.total_ram),
            usage_color(usage_percent(info.used_ram, info.total_ram) as u16),
        ),
        Span::raw(" "),
        chip(
            format!(
                "LOAD {:.2} {:.2} {:.2}",
                info.load_average.0, info.load_average.1, info.load_average.2
            ),
            muted(),
        ),
        Span::raw(" "),
        chip(format!("UP {}", format_uptime(info.uptime)), value()),
    ]))
    .alignment(Alignment::Center)
    .block(card("overview"));

    frame.render_widget(title, page[0]);

    let main = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(34),
            Constraint::Percentage(36),
            Constraint::Percentage(30),
        ])
        .spacing(1)
        .split(page[1]);

    for area in main.iter() {
        frame.render_widget(fill(BACKGROUND), *area);
    }

    let top = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(42), Constraint::Percentage(58)])
        .spacing(1)
        .split(main[0]);

    for area in top.iter() {
        frame.render_widget(fill(SURFACE), *area);
    }

    let middle = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(44), Constraint::Percentage(56)])
        .spacing(1)
        .split(main[1]);

    for area in middle.iter() {
        frame.render_widget(fill(SURFACE), *area);
    }

    let middle_left = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(5)])
        .spacing(1)
        .split(middle[0]);

    for area in middle_left.iter() {
        frame.render_widget(fill(SURFACE), *area);
    }

    let system_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(5), Constraint::Length(3)])
        .spacing(1)
        .split(top[0]);

    for area in system_layout.iter() {
        frame.render_widget(fill(SURFACE), *area);
    }

    let system_info = Paragraph::new(vec![
        metric_line(
            "OS      ",
            format!("{}  kernel {}", info.os_name, info.kernel),
        ),
        metric_line("CPU     ", info.cpu_name.clone()),
        metric_line(
            "THREADS ",
            format!("{} cores  {} processes", info.cpu_cores, info.processes),
        ),
        metric_line(
            "LOAD    ",
            format!(
                "{:.2}  {:.2}  {:.2}",
                info.load_average.0, info.load_average.1, info.load_average.2
            ),
        ),
    ])
    .style(Style::default().fg(TEXT).bg(SURFACE))
    .block(card("System"));

    let cpu_percent = info.cpu_usage.clamp(0.0, 100.0) as u16;

    let mut cpu_line = vec![Span::styled("CPU ", label())];
    cpu_line.extend(meter(cpu_percent, 28));
    cpu_line.push(Span::styled(
        format!(" {:>5.1}%", info.cpu_usage),
        usage_color(cpu_percent),
    ));

    let cpu_gauge = Paragraph::new(Line::from(cpu_line))
        .alignment(Alignment::Center)
        .style(Style::default().bg(BACKGROUND))
        .block(card("CPU Usage"));

    let gpu_lines = if info.gpus.is_empty() {
        vec![Line::styled("GPU não encontrada", muted())]
    } else {
        info.gpus
            .iter()
            .enumerate()
            .flat_map(|(index, gpu)| {
                [
                    Line::styled(
                        format!("GPU {}  {}", index + 1, gpu.name),
                        value().add_modifier(Modifier::BOLD),
                    ),
                    metric_line(
                        "  ",
                        format!(
                            "Temp {}  Uso {}  VRAM {}",
                            gpu.temperature, gpu.usage, gpu.memory
                        ),
                    ),
                ]
            })
            .collect()
    };

    let gpu = Paragraph::new(gpu_lines)
        .style(Style::default().bg(SURFACE))
        .block(card("GPU"));

    let ram_percent = usage_percent(info.used_ram, info.total_ram) as u16;

    let mut ram_line = vec![Span::styled("RAM ", label())];
    ram_line.extend(meter(ram_percent, 28));
    ram_line.push(Span::styled(
        format!(" {:>3}%", ram_percent),
        usage_color(ram_percent),
    ));

    let ram = Paragraph::new(Line::from(ram_line))
        .alignment(Alignment::Center)
        .style(Style::default().bg(BACKGROUND))
        .block(card(format!(
            "RAM {:.1}/{:.1}GB  Swap {:.1}/{:.1}GB",
            info.used_ram, info.total_ram, info.used_swap, info.total_swap
        )));

    let mut process_lines = vec![section_header(vec![
        ("PROCESS", 24),
        ("MEM", 9),
        ("CPU", 5),
    ])];
    process_lines.extend(
        info.processes_list
            .iter()
            .map(|p| {
                Line::from(vec![
                    Span::styled(format!("{:<24}", p.name), value()),
                    Span::styled(format!("{:>7.1}M ", p.memory), label()),
                    Span::styled(
                        format!("{:>5.1}%", p.cpu_usage),
                        usage_color(p.cpu_usage as u16),
                    ),
                ])
            })
            .collect::<Vec<Line>>(),
    );

    let processes = Paragraph::new(process_lines)
        .style(Style::default().bg(SURFACE))
        .block(card("Top Processes"));

    let mut disk_lines = vec![section_header(vec![
        ("DEVICE", 10),
        ("MOUNT", 12),
        ("USED", 12),
        ("%", 4),
    ])];
    disk_lines.extend(
        info.disks
            .iter()
            .take(5)
            .map(|disk| {
                let (used, percent) = disk_usage(disk.total_space, disk.available_space);

                Line::from(vec![
                    Span::styled(format!("{:<10}", disk.name), value()),
                    Span::styled(format!("{:<12}", disk.mount_point), label()),
                    Span::styled(format!("{:>5.1}/{:<4.1} ", used, disk.total_space), value()),
                    Span::styled(format!("{:>4.0}%", percent), usage_color(percent as u16)),
                ])
            })
            .collect::<Vec<Line>>(),
    );

    let disks = Paragraph::new(disk_lines)
        .style(Style::default().bg(SURFACE))
        .block(card("Storage"));

    let cpu_points = app
        .cpu_history
        .iter()
        .enumerate()
        .map(|(index, value)| (index as f64, *value as f64))
        .collect::<Vec<(f64, f64)>>();
    let cpu_x_max = cpu_points.len().saturating_sub(1).max(1) as f64;
    let cpu_history = Chart::new(vec![
        Dataset::default()
            .name("usage")
            .marker(symbols::Marker::Braille)
            .graph_type(GraphType::Line)
            .style(Style::default().fg(ACCENT).add_modifier(Modifier::BOLD))
            .data(&cpu_points),
    ])
    .block(card(format!("CPU Trend  {:.1}%", info.cpu_usage)))
    .style(Style::default().bg(SURFACE))
    .x_axis(
        Axis::default()
            .style(muted())
            .bounds([0.0, cpu_x_max])
            .labels(vec![Span::raw("60s"), Span::raw("now")]),
    )
    .y_axis(
        Axis::default()
            .style(muted())
            .bounds([0.0, 100.0])
            .labels(vec![Span::raw("0"), Span::raw("50"), Span::raw("100")]),
    );

    let mut network_lines = vec![section_header(vec![
        ("IFACE", 14),
        ("DOWN", 13),
        ("UP", 12),
    ])];
    network_lines.extend(
        info.networks
            .iter()
            .map(|net| {
                Line::from(vec![
                    Span::styled(format!("{:<14}", net.interface), value()),
                    Span::styled(
                        format!("{:>8.2} MB/s ", net.received_per_sec),
                        Style::default().fg(OK),
                    ),
                    Span::styled(
                        format!("{:>7.2} MB/s", net.transmitted_per_sec),
                        Style::default().fg(WARN),
                    ),
                ])
            })
            .collect::<Vec<Line>>(),
    );

    let network = Paragraph::new(network_lines)
        .style(Style::default().bg(SURFACE))
        .block(card("Network"));

    let footer = Paragraph::new("q/Esc/Ctrl+C sair  /  refresh 1s  /  NVIDIA via nvidia-smi")
        .alignment(Alignment::Center)
        .style(muted());

    frame.render_widget(system_info, system_layout[0]);
    frame.render_widget(cpu_gauge, system_layout[1]);
    frame.render_widget(gpu, top[1]);
    frame.render_widget(ram, middle_left[0]);
    frame.render_widget(processes, middle[1]);

    let bottom = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .spacing(1)
        .split(main[2]);

    for area in bottom.iter() {
        frame.render_widget(fill(SURFACE), *area);
    }

    frame.render_widget(disks, middle_left[1]);
    frame.render_widget(cpu_history, bottom[0]);
    frame.render_widget(network, bottom[1]);
    frame.render_widget(footer, page[2]);
}

#[cfg(test)]
mod tests {
    use ratatui::{Terminal, backend::TestBackend};

    use super::{disk_usage, draw, format_uptime, usage_percent};
    use crate::{app::AppState, system::info::SystemInfo};

    #[test]
    fn formats_uptime_with_and_without_days() {
        assert_eq!(format_uptime(3_661), "1h 1m");
        assert_eq!(format_uptime(90_061), "1d 1h 1m");
    }

    #[test]
    fn handles_zero_and_out_of_range_usage_values() {
        assert_eq!(usage_percent(5.0, 0.0), 0.0);
        assert_eq!(usage_percent(-1.0, 10.0), 0.0);
        assert_eq!(usage_percent(12.0, 10.0), 100.0);
    }

    #[test]
    fn handles_empty_disks_without_division_by_zero() {
        assert_eq!(disk_usage(0.0, 0.0), (0.0, 0.0));
    }

    #[test]
    fn renders_dashboard_at_representative_terminal_sizes() {
        let info = sample_system_info();
        let mut app = AppState::new();
        app.update_cpu_history(info.cpu_usage);

        for (width, height) in [(80, 24), (120, 40)] {
            let backend = TestBackend::new(width, height);
            let mut terminal = Terminal::new(backend).expect("test terminal should be created");

            terminal
                .draw(|frame| draw(frame, &info, &app))
                .expect("dashboard should render");

            let rendered = terminal
                .backend()
                .buffer()
                .content
                .iter()
                .map(|cell| cell.symbol())
                .collect::<String>();
            assert!(rendered.contains("SYSMON"));
        }
    }

    fn sample_system_info() -> SystemInfo {
        SystemInfo {
            os_name: "Linux".to_string(),
            kernel: "6.x".to_string(),
            cpu_name: "Test CPU".to_string(),
            cpu_cores: 8,
            cpu_usage: 42.0,
            total_ram: 16.0,
            used_ram: 8.0,
            total_swap: 4.0,
            used_swap: 1.0,
            processes: 100,
            uptime: 90_061,
            load_average: (0.5, 0.4, 0.3),
            disks: Vec::new(),
            processes_list: Vec::new(),
            gpus: Vec::new(),
            networks: Vec::new(),
        }
    }
}
