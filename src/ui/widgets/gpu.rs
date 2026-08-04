use crate::{
    system::info::SystemInfo,
    theme::Theme,
    ui::renderer::{card, metric_line, muted, value},
};

use ratatui::{
    Frame,
    style::{Modifier, Style},
    text::Line,
    widgets::Paragraph,
};

fn lines(info: &SystemInfo, theme: &Theme) -> Vec<Line<'static>> {
    if info.gpus.is_empty() {
        return vec![Line::styled("GPU não encontrada", muted(theme))];
    }

    info.gpus
        .iter()
        .enumerate()
        .flat_map(|(index, gpu)| {
            [
                Line::styled(
                    format!("GPU {}  {}", index + 1, gpu.name),
                    value(theme).add_modifier(Modifier::BOLD),
                ),
                metric_line(
                    theme,
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

pub(crate) fn draw(
    frame: &mut Frame,
    area: ratatui::layout::Rect,
    info: &SystemInfo,
    theme: &Theme,
) {
    let gpu = Paragraph::new(lines(info, theme))
        .style(Style::default().bg(theme.surface))
        .block(card(theme, "GPU"));

    frame.render_widget(gpu, area);
}
