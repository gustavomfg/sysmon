use crate::{
    theme::Theme,
    ui::renderer::{label, meter, panel, usage_color},
};

use ratatui::{
    Frame,
    layout::Alignment,
    style::Style,
    text::{Line, Span},
    widgets::Paragraph,
};

pub(crate) fn draw(
    frame: &mut Frame,
    area: ratatui::layout::Rect,
    cpu_usage: f64,
    theme: &Theme,
    focused: bool,
) {
    let cpu_percent = cpu_usage.clamp(0.0, 100.0) as u16;

    let mut cpu_line = vec![Span::styled("CPU ", label(theme))];
    let meter_width = usize::from(area.width.saturating_sub(18)).clamp(8, 28);
    cpu_line.extend(meter(theme, cpu_percent, meter_width));
    cpu_line.push(Span::styled(
        format!(" {:>5.1}%", cpu_usage),
        usage_color(theme, cpu_percent),
    ));

    let cpu_gauge = Paragraph::new(Line::from(cpu_line))
        .alignment(Alignment::Center)
        .style(Style::default().bg(theme.surface))
        .block(panel(theme, "CPU Usage", focused));

    frame.render_widget(cpu_gauge, area);
}
