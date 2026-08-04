use crate::{
    system::info::SystemInfo,
    ui::renderer::{BACKGROUND, card, label, meter, usage_color},
};

use ratatui::{
    Frame,
    layout::Alignment,
    style::Style,
    text::{Line, Span},
    widgets::Paragraph,
};

pub(crate) fn draw(frame: &mut Frame, area: ratatui::layout::Rect, info: &SystemInfo) {
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

    frame.render_widget(cpu_gauge, area);
}
