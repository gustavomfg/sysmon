use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Padding},
};

pub(crate) const ACCENT: Color = Color::Cyan;
pub(crate) const ACCENT_ALT: Color = Color::Magenta;
pub(crate) const OK: Color = Color::Green;
pub(crate) const WARN: Color = Color::Yellow;
pub(crate) const DANGER: Color = Color::Red;
pub(crate) const BACKGROUND: Color = Color::Black;
pub(crate) const SURFACE: Color = Color::Black;
pub(crate) const TEXT: Color = Color::Gray;

pub(crate) fn usage_color(value: u16) -> Style {
    match value {
        0..=50 => Style::default().fg(OK),
        51..=80 => Style::default().fg(WARN),
        _ => Style::default().fg(DANGER).add_modifier(Modifier::BOLD),
    }
}

pub(crate) fn muted() -> Style {
    Style::default().fg(Color::DarkGray)
}

pub(crate) fn label() -> Style {
    Style::default().fg(Color::DarkGray)
}

pub(crate) fn value() -> Style {
    Style::default().fg(Color::White)
}

pub(crate) fn brand() -> Style {
    Style::default()
        .fg(Color::Black)
        .bg(ACCENT)
        .add_modifier(Modifier::BOLD)
}

pub(crate) fn chip(text: impl Into<String>, style: Style) -> Span<'static> {
    Span::styled(format!(" {} ", text.into()), style)
}

pub(crate) fn card(title: impl Into<String>) -> Block<'static> {
    Block::default()
        .title(Span::styled(
            format!("  {}  ", title.into()),
            Style::default().fg(ACCENT_ALT).add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::DarkGray))
        .style(Style::default().bg(SURFACE))
        .padding(Padding::horizontal(1))
}

pub(crate) fn fill(color: Color) -> Block<'static> {
    Block::default().style(Style::default().bg(color))
}

pub(crate) fn metric_line(
    label_text: impl Into<String>,
    value_text: impl Into<String>,
) -> Line<'static> {
    Line::from(vec![
        Span::styled(label_text.into(), label()),
        Span::styled(value_text.into(), value()),
    ])
}

pub(crate) fn section_header(columns: Vec<(&str, usize)>) -> Line<'static> {
    Line::from(
        columns
            .into_iter()
            .map(|(text, width)| {
                Span::styled(
                    format!("{text:<width$}"),
                    Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
                )
            })
            .collect::<Vec<Span>>(),
    )
}

pub(crate) fn meter(value: u16, width: usize) -> Vec<Span<'static>> {
    let filled = ((value as usize * width) / 100).min(width);
    vec![
        Span::styled("[", muted()),
        Span::styled("#".repeat(filled), usage_color(value)),
        Span::styled("-".repeat(width - filled), muted()),
        Span::styled("]", muted()),
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
