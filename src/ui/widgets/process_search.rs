use crate::{
    app::AppState,
    theme::Theme,
    ui::renderer::{card, centered_overlay, fit},
};

use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Clear, Paragraph, Wrap},
};

pub(crate) fn draw(frame: &mut Frame, area: Rect, app: &AppState, theme: &Theme) {
    let popup = centered_overlay(area, 80, 20);
    frame.render_widget(Clear, popup);

    let mut lines = vec![Line::from(Span::styled(
        format!("Search: {}", app.process_search().query()),
        Style::default().fg(theme.text),
    ))];
    lines.push(Line::raw(""));

    let results = app.process_search_results();
    if results.is_empty() {
        lines.push(Line::styled(
            "No matching processes",
            Style::default().fg(theme.text_muted),
        ));
    } else {
        for (index, process) in results.into_iter().enumerate() {
            let selected = index == app.process_search().selected_index();
            let text = fit(
                &format!(
                    "{}  {:>7.1}M  {:>5.1}%",
                    process.name, process.memory, process.cpu_usage
                ),
                usize::from(popup.width.saturating_sub(6)),
            );
            let style = if selected {
                Style::default()
                    .fg(theme.selection_text)
                    .bg(theme.selection_background)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.text)
            };
            lines.push(Line::from(Span::styled(
                if selected {
                    format!("> {text}")
                } else {
                    format!("  {text}")
                },
                style,
            )));
        }
    }
    lines.push(Line::styled(
        "↑/↓ navigate  Enter select  Esc close",
        Style::default().fg(theme.text_muted),
    ));

    let search = Paragraph::new(lines)
        .wrap(Wrap { trim: true })
        .style(Style::default().bg(theme.surface_alt))
        .block(card(theme, "Search Process", true));
    frame.render_widget(search, popup);
}
