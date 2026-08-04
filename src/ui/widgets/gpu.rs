use crate::{
    system::info::SystemInfo,
    ui::renderer::{card, metric_line, muted, value},
};

use ratatui::{
    Frame,
    style::{Modifier, Style},
    text::Line,
    widgets::Paragraph,
};

fn lines(info: &SystemInfo) -> Vec<Line<'static>> {
    if info.gpus.is_empty() {
        return vec![Line::styled("GPU não encontrada", muted())];
    }

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
}

pub(crate) fn draw(frame: &mut Frame, area: ratatui::layout::Rect, info: &SystemInfo) {
    let gpu = Paragraph::new(lines(info))
        .style(Style::default().bg(ratatui::style::Color::Black))
        .block(card("GPU"));

    frame.render_widget(gpu, area);
}
