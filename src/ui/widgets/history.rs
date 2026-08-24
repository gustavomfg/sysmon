use std::time::Duration;

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
    let history_window = app.history_window();
    let sample_interval = app.refresh_interval().as_secs_f64();
    let cpu_points = app
        .history()
        .samples(Metric::Cpu)
        .iter()
        .enumerate()
        .map(|(index, value)| (index as f64 * sample_interval, *value))
        .collect::<Vec<(f64, f64)>>();
    let cpu_x_max = history_window
        .as_secs_f64()
        .max(sample_interval)
        .max(1.0);
    let history_label = format_history_window(history_window);
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
            .labels(vec![Span::raw(history_label), Span::raw("now")]),
    )
    .y_axis(
        Axis::default()
            .style(muted(theme))
            .bounds([0.0, 100.0])
            .labels(vec![Span::raw("0"), Span::raw("50"), Span::raw("100")]),
    );

    frame.render_widget(cpu_history, area);
}

fn format_history_window(window: Duration) -> String {
    format!("{}s", window.as_secs())
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::format_history_window;

    #[test]
    fn formats_history_windows_for_one_two_and_four_second_refresh_rates() {
        assert_eq!(
            format_history_window(Duration::from_secs(60)),
            "60s"
        );
        assert_eq!(
            format_history_window(Duration::from_secs(120)),
            "120s"
        );
        assert_eq!(
            format_history_window(Duration::from_secs(240)),
            "240s"
        );
    }
}
