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

    Constraint::Percentage(20),
    Constraint::Percentage(20),
    Constraint::Percentage(20),
    Constraint::Percentage(20),
    Constraint::Percentage(20),

    ])
        .split(frame.area());


    let cpu_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
        ])
        .split(chunks[0]);


    let cpu_info = Paragraph::new(
        format!(
            "{}\nCores: {}",
            info.cpu_name,
            info.cpu_cores
        )
    )
    .block(
        Block::default()
            .title("CPU")
            .borders(Borders::ALL)
    );


    let cpu_gauge = Gauge::default()
        .block(Block::default().borders(Borders::ALL))
        .percent(info.cpu_usage as u16)
        .label(format!("{:.2}%", info.cpu_usage));


    let ram_percent =
        ((info.used_ram / info.total_ram) * 100.0) as u16;


    let ram = Gauge::default()
        .block(
            Block::default()
                .title(
                    format!(
                        "MEMORY {:.2}/{:.2} GB",
                        info.used_ram,
                        info.total_ram
                    )
                )
                .borders(Borders::ALL)
        )
        .percent(ram_percent);


    let disk_lines = info.disks
        .iter()
        .map(|disk| {

            let used =
                disk.total_space - disk.available_space;

            Line::from(
                format!(
                    "{} {} {:.2}/{:.2} GB",
                    disk.name,
                    disk.mount_point,
                    used,
                    disk.total_space
                )
            )
        })
        .collect::<Vec<Line>>();


    let disks = Paragraph::new(disk_lines)
        .block(
            Block::default()
                .title("DISKS")
                .borders(Borders::ALL)
        );

    let gpu_text = match &info.gpu {
        Some(gpu) => format!(
            "{}\nTemp: {}\nUso: {}\nVRAM usada: {}",
                gpu.name,
                gpu.temperature,
                gpu.usage,
                gpu.memory
            ),
            None => "GPU NVIDIA não encontrada".to_string(),
};

    let gpu = Paragraph::new(gpu_text)
        .block(
            Block::default()
                .title("GPU")
                .borders(Borders::ALL)
    );

    let process_lines = info
        .processes_list
        .iter()
        .map(|process| {
            Line::from(
                format!(
                    "{} - {:.2} MB",
                    process.name,
                    process.memory
                )
            )
        })
        .collect::<Vec<Line>>();


    let processes =
        Paragraph::new(process_lines)
        .block(
            Block::default()
                .title("TOP MEMORY")
                .borders(Borders::ALL)
        );

    frame.render_widget(cpu_info, cpu_chunks[0]);
    frame.render_widget(cpu_gauge, cpu_chunks[1]);

    frame.render_widget(ram, chunks[1]);
    frame.render_widget(disks, chunks[2]);
    frame.render_widget(gpu, chunks[3]);
    frame.render_widget(processes, chunks[4]);
}