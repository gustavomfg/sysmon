use crate::{
    app::AppState,
    theme::Theme,
    ui::renderer::{card, fit},
};

use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Clear, Paragraph, Wrap},
};

pub(crate) fn draw(frame: &mut Frame, area: Rect, app: &AppState, theme: &Theme) {
    let popup = centered_rect(area);
    frame.render_widget(Clear, popup);

    let mut lines = vec![Line::from(Span::styled(
        format!("> {}", app.palette().query()),
        Style::default().fg(theme.text),
    ))];
    lines.push(Line::raw(""));

    let commands = app.palette().filtered_commands();
    if commands.is_empty() {
        lines.push(Line::styled(
            "No matching commands",
            Style::default().fg(theme.text_muted),
        ));
    } else {
        for (index, command) in commands.into_iter().enumerate() {
            let selected = index == app.palette().selected_index();
            let text = format!("{}  — {}", command.label(), command.unavailable_reason());
            let text = fit(&text, usize::from(popup.width.saturating_sub(6)));
            let style = if selected {
                Style::default()
                    .fg(theme.selection_text)
                    .bg(theme.selection_background)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.text_muted)
            };
            lines.push(Line::from(vec![Span::styled(
                if selected {
                    format!("> {text}")
                } else {
                    format!("  {text}")
                },
                style,
            )]));
        }
    }

    let palette = Paragraph::new(lines)
        .wrap(Wrap { trim: true })
        .style(Style::default().bg(theme.surface_alt))
        .block(card(theme, "Command Palette", true));
    frame.render_widget(palette, popup);
}

fn centered_rect(area: Rect) -> Rect {
    let width = area.width.saturating_sub(4).min(72);
    let height = area.height.saturating_sub(4).min(14);
    Rect {
        x: area.x + area.width.saturating_sub(width) / 2,
        y: area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    }
}
