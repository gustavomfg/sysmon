use crate::{
    theme::Theme,
    timeline::Timeline,
    ui::renderer::{fit, panel},
};

use ratatui::{
    Frame,
    style::Style,
    text::{Line, Span},
    widgets::Paragraph,
};

pub(crate) fn draw(
    frame: &mut Frame,
    area: ratatui::layout::Rect,
    timeline: &Timeline,
    theme: &Theme,
    focused: bool,
) {
    let visible = usize::from(area.height.saturating_sub(2)).max(1);
    let events = timeline.events();
    let start = events.len().saturating_sub(visible);
    let width = usize::from(area.width.saturating_sub(4));
    let lines = events[start..]
        .iter()
        .map(|event| {
            Line::from(vec![
                Span::styled("• ", Style::default().fg(theme.secondary)),
                Span::styled(
                    fit(event, width.saturating_sub(2)),
                    Style::default().fg(theme.text),
                ),
            ])
        })
        .collect::<Vec<_>>();

    let timeline_widget = Paragraph::new(lines)
        .style(Style::default().bg(theme.surface))
        .block(panel(theme, "Timeline", focused));
    frame.render_widget(timeline_widget, area);
}
