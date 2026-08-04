use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Padding},
};

use crate::theme::Theme;

pub(crate) fn usage_color(theme: &Theme, value: u16) -> Style {
    match value {
        0..=50 => Style::default().fg(theme.success),
        51..=80 => Style::default().fg(theme.warning),
        _ => Style::default()
            .fg(theme.danger)
            .add_modifier(Modifier::BOLD),
    }
}

pub(crate) fn muted(theme: &Theme) -> Style {
    Style::default().fg(theme.text_muted)
}

pub(crate) fn label(theme: &Theme) -> Style {
    Style::default().fg(theme.text_muted)
}

pub(crate) fn value(theme: &Theme) -> Style {
    Style::default().fg(theme.text)
}

pub(crate) fn brand(theme: &Theme) -> Style {
    Style::default()
        .fg(theme.background)
        .bg(theme.secondary)
        .add_modifier(Modifier::BOLD)
}

pub(crate) fn chip(text: impl Into<String>, style: Style) -> Span<'static> {
    Span::styled(format!(" {} ", text.into()), style)
}

pub(crate) fn card(theme: &Theme, title: impl Into<String>, focused: bool) -> Block<'static> {
    Block::default()
        .title(Span::styled(
            format!("  {}  ", title.into()),
            Style::default()
                .fg(theme.primary)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(if focused {
            theme.border_focused
        } else {
            theme.border
        }))
        .style(Style::default().bg(theme.surface))
        .padding(Padding::horizontal(1))
}

pub(crate) fn panel(theme: &Theme, title: impl Into<String>, focused: bool) -> Block<'static> {
    Block::default()
        .title(Span::styled(
            format!("  {}  ", title.into()),
            Style::default()
                .fg(theme.primary)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::BOTTOM)
        .border_style(Style::default().fg(if focused {
            theme.border_focused
        } else {
            theme.border
        }))
        .style(Style::default().bg(theme.surface))
        .padding(Padding::horizontal(1))
}

pub(crate) fn centered_overlay(
    area: ratatui::layout::Rect,
    max_width: u16,
    max_height: u16,
) -> ratatui::layout::Rect {
    let width = area.width.saturating_sub(4).min(max_width);
    let height = area.height.saturating_sub(4).min(max_height);
    ratatui::layout::Rect {
        x: area.x + area.width.saturating_sub(width) / 2,
        y: area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    }
}

pub(crate) fn fill(color: Color) -> Block<'static> {
    Block::default().style(Style::default().bg(color))
}

pub(crate) fn metric_line(
    theme: &Theme,
    label_text: impl Into<String>,
    value_text: impl Into<String>,
) -> Line<'static> {
    Line::from(vec![
        Span::styled(label_text.into(), label(theme)),
        Span::styled(value_text.into(), value(theme)),
    ])
}

pub(crate) fn section_header(theme: &Theme, columns: Vec<(&str, usize)>) -> Line<'static> {
    Line::from(
        columns
            .into_iter()
            .map(|(text, width)| {
                Span::styled(
                    format!("{text:<width$}"),
                    Style::default()
                        .fg(theme.secondary)
                        .add_modifier(Modifier::BOLD),
                )
            })
            .collect::<Vec<Span>>(),
    )
}

pub(crate) fn meter(theme: &Theme, value: u16, width: usize) -> Vec<Span<'static>> {
    let filled = ((value as usize * width) / 100).min(width);
    vec![
        Span::styled("[", muted(theme)),
        Span::styled("#".repeat(filled), usage_color(theme, value)),
        Span::styled("-".repeat(width - filled), muted(theme)),
        Span::styled("]", muted(theme)),
    ]
}

pub(crate) fn format_uptime(seconds: u64) -> String {
    let days = seconds / 86_400;
    let hours = (seconds % 86_400) / 3_600;
    let minutes = (seconds % 3_600) / 60;

    if days > 0 {
        format!("{days}d {hours}h {minutes}m")
    } else {
        format!("{hours}h {minutes}m")
    }
}

pub(crate) fn usage_percent(used: f64, total: f64) -> f64 {
    if total > 0.0 {
        (used / total * 100.0).clamp(0.0, 100.0)
    } else {
        0.0
    }
}

pub(crate) fn disk_usage(total: f64, available: f64) -> (f64, f64) {
    let used = (total - available).max(0.0);
    (used, usage_percent(used, total))
}

pub(crate) fn fit(text: &str, width: usize) -> String {
    let value = text.chars().take(width).collect::<String>();
    format!("{value:<width$}")
}
