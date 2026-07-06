use crate::system::info::SystemInfo;

use ratatui::{
    layout::{Constraint, Direction, Layout},
    text::Line,
    widgets::{Block, Borders, Gauge, Paragraph},
    Frame,
};


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


    // CPU

    let cpu_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
        ])
        .split(top[0]);


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
        .block(
            Block::default()
                .borders(Borders::ALL)
        )
        .percent(
            info.cpu_usage as u16
        )
        .label(
            format!("{:.2}%", info.cpu_usage)
        );


    // GPU

    let gpu_text = match &info.gpu {

        Some(gpu) => format!(
            "{}\nTemp: {}\nUso: {}\nVRAM: {}",
            gpu.name,
            gpu.temperature,
            gpu.usage,
            gpu.memory
        ),

        None =>
            "GPU não encontrada".to_string(),
    };


    let gpu = Paragraph::new(gpu_text)
        .block(
            Block::default()
                .title("GPU")
                .borders(Borders::ALL)
        );


    // RAM

    let ram_percent =
        ((info.used_ram / info.total_ram) * 100.0)
            as u16;


    let ram = Gauge::default()
        .block(
            Block::default()
                .title(
                    format!(
                        "RAM {:.1}/{:.1}GB",
                        info.used_ram,
                        info.total_ram
                    )
                )
                .borders(Borders::ALL)
        )
        .percent(ram_percent)
        .label(format!("{}%", ram_percent));


    // PROCESSOS

    let process_lines =
        info.processes_list
            .iter()
            .map(|p| {

                Line::from(
                    format!(
                        "{} {:.1}MB",
                        p.name,
                        p.memory
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


    // DISKS

    let disk_lines =
        info.disks
            .iter()
            .map(|disk| {

                let used =
                    disk.total_space
                    -
                    disk.available_space;


                Line::from(
                    format!(
                        "{} {} {:.1}/{:.1}GB",
                        disk.name,
                        disk.mount_point,
                        used,
                        disk.total_space
                    )
                )
            })
            .collect::<Vec<Line>>();


    let disks =
        Paragraph::new(disk_lines)
            .block(
                Block::default()
                    .title("DISKS")
                    .borders(Borders::ALL)
            );


    frame.render_widget(cpu_info, cpu_layout[0]);
    frame.render_widget(cpu_gauge, cpu_layout[1]);

    frame.render_widget(gpu, top[1]);

    frame.render_widget(ram, middle[0]);

    frame.render_widget(processes, middle[1]);

    frame.render_widget(disks, main[2]);
}