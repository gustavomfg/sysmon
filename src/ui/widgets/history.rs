use crate::{
    app::AppState,
    system::info::SystemInfo,
    ui::renderer::{ACCENT, SURFACE, card, muted},
};

use ratatui::{
    Frame,
    style::{Modifier, Style},
    symbols,
    text::Span,
    widgets::{Axis, Chart, Dataset, GraphType},
};

pub(crate) fn draw(
    frame: &mut Frame,
    area: ratatui::layout::Rect,
    info: &SystemInfo,
    app: &AppState,
) {
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

    frame.render_widget(cpu_history, area);
}
