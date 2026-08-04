use crate::{
    app::AppState,
    history::Metric,
    theme::Theme,
    ui::renderer::{muted, panel},
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
    app: &AppState,
    theme: &Theme,
    focused: bool,
) {
    let cpu_points = app
        .history()
        .samples(Metric::Cpu)
        .iter()
        .enumerate()
        .map(|(index, value)| (index as f64, *value))
        .collect::<Vec<(f64, f64)>>();
    let cpu_x_max = cpu_points.len().saturating_sub(1).max(1) as f64;
    let cpu_history = Chart::new(vec![
        Dataset::default()
            .name("usage")
            .marker(symbols::Marker::Braille)
            .graph_type(GraphType::Line)
            .style(
                Style::default()
                    .fg(theme.chart_primary)
                    .add_modifier(Modifier::BOLD),
            )
            .data(&cpu_points),
    ])
    .block(panel(
        theme,
        format!("CPU Trend  {:.1}%", app.cpu_usage()),
        focused,
    ))
    .style(Style::default().bg(theme.surface))
    .x_axis(
        Axis::default()
            .style(muted(theme))
            .bounds([0.0, cpu_x_max])
            .labels(vec![Span::raw("60s"), Span::raw("now")]),
    )
    .y_axis(
        Axis::default()
            .style(muted(theme))
            .bounds([0.0, 100.0])
            .labels(vec![Span::raw("0"), Span::raw("50"), Span::raw("100")]),
    );

    frame.render_widget(cpu_history, area);
}
