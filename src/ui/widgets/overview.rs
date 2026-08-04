use crate::{
    system::info::SystemInfo,
    ui::{MIN_DASHBOARD_HEIGHT, MIN_DASHBOARD_WIDTH},
};

use ratatui::{
    Frame,
    layout::Alignment,
    style::Modifier,
    text::{Line, Span},
    widgets::{Paragraph, Wrap},
};

use super::super::renderer::{
    brand, card, chip, format_uptime, muted, usage_color, usage_percent, value,
};

pub(crate) fn draw_minimum_size_message(frame: &mut Frame) {
    let area = frame.area();
    let message = Paragraph::new(format!(
        "Janela muito pequena: {}x{}.\nO Sysmon requer ao menos {}x{}.",
        area.width, area.height, MIN_DASHBOARD_WIDTH, MIN_DASHBOARD_HEIGHT
    ))
    .alignment(Alignment::Center)
    .wrap(Wrap { trim: true })
    .block(card("Sysmon"));
    frame.render_widget(message, area);
}

pub(crate) fn draw(frame: &mut Frame, area: ratatui::layout::Rect, info: &SystemInfo) {
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

    frame.render_widget(title, area);
}
