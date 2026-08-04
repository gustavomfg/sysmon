use crate::{
    app::AppState,
    config::ConfigField,
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
    let popup = centered_overlay(area, 64, 12);
    frame.render_widget(Clear, popup);

    let mut lines = vec![Line::styled(
        "Temporary settings for this session",
        Style::default().fg(theme.text_muted),
    )];
    lines.push(Line::raw(""));

    for field in ConfigField::ALL {
        let selected = field == app.config().selected();
        let value = match field {
            ConfigField::Theme => app.theme_kind().label().to_string(),
            ConfigField::ReducedMotion => {
                if app.reduced_motion() {
                    "On".to_string()
                } else {
                    "Off".to_string()
                }
            }
            ConfigField::RefreshRate => format!("{}s", app.refresh_interval().as_secs()),
        };
        let text = fit(
            &format!("{}: {value}", field.label()),
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

    lines.push(Line::raw(""));
    lines.push(Line::styled(
        "↑/↓ select  ←/→ change  Enter toggle  Esc close",
        Style::default().fg(theme.text_muted),
    ));

    let config = Paragraph::new(lines)
        .wrap(Wrap { trim: true })
        .style(Style::default().bg(theme.surface_alt))
        .block(card(theme, "Settings", true));
    frame.render_widget(config, popup);
}
