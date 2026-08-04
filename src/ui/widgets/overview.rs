use crate::{
    system::info::SystemInfo,
    theme::Theme,
    ui::{MIN_DASHBOARD_HEIGHT, MIN_DASHBOARD_WIDTH},
};

use ratatui::{
    Frame,
    layout::Alignment,
    style::Modifier,
    text::{Line, Span},
    widgets::{Paragraph, Wrap},
};

use super::super::renderer::{brand, card, chip, format_uptime, muted, usage_color, value};

pub(crate) fn draw_minimum_size_message(frame: &mut Frame, theme: &Theme) {
    let area = frame.area();
    let message = Paragraph::new(format!(
        "Janela muito pequena: {}x{}.\nO Sysmon requer ao menos {}x{}.",
        area.width, area.height, MIN_DASHBOARD_WIDTH, MIN_DASHBOARD_HEIGHT
    ))
    .alignment(Alignment::Center)
    .wrap(Wrap { trim: true })
    .block(card(theme, "Sysmon"));
    frame.render_widget(message, area);
}

pub(crate) fn draw(
    frame: &mut Frame,
    area: ratatui::layout::Rect,
    info: &SystemInfo,
    cpu_usage: f64,
    memory_percent: f64,
    theme: &Theme,
) {
    let memory_used = info.total_ram * memory_percent.clamp(0.0, 100.0) / 100.0;
    let title = Paragraph::new(Line::from(vec![
        Span::raw(" "),
        chip("SYSMON", brand(theme)),
        Span::raw("  "),
        chip(
            format!("CPU {:.1}%", cpu_usage),
            usage_color(theme, cpu_usage as u16).add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        chip(
            format!("RAM {:.1}/{:.1}GB", memory_used, info.total_ram),
            usage_color(theme, memory_percent as u16),
        ),
        Span::raw(" "),
        chip(
            format!(
                "LOAD {:.2} {:.2} {:.2}",
                info.load_average.0, info.load_average.1, info.load_average.2
            ),
            muted(theme),
        ),
        Span::raw(" "),
        chip(format!("UP {}", format_uptime(info.uptime)), value(theme)),
    ]))
    .alignment(Alignment::Center)
    .block(card(theme, "overview"));

    frame.render_widget(title, area);
}
