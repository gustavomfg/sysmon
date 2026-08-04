use crate::{
    system::info::SystemInfo,
    theme::Theme,
    ui::renderer::{card, fit, section_header, usage_color, value},
};

use ratatui::{
    Frame,
    style::Style,
    text::{Line, Span},
    widgets::Paragraph,
};

fn lines(info: &SystemInfo, area_width: u16, theme: &Theme) -> Vec<Line<'static>> {
    let process_width = usize::from(area_width.saturating_sub(20)).clamp(8, 24);
    let mut lines = vec![section_header(
        theme,
        vec![("PROCESS", process_width), ("MEM", 9), ("CPU", 5)],
    )];

    lines.extend(info.processes_list.iter().map(|process| {
        Line::from(vec![
            Span::styled(fit(&process.name, process_width), value(theme)),
            Span::styled(
                format!("{:>7.1}M ", process.memory),
                Style::default().fg(theme.text_muted),
            ),
            Span::styled(
                format!("{:>5.1}%", process.cpu_usage),
                usage_color(theme, process.cpu_usage as u16),
            ),
        ])
    }));
    lines
}

pub(crate) fn draw(
    frame: &mut Frame,
    area: ratatui::layout::Rect,
    info: &SystemInfo,
    theme: &Theme,
    focused: bool,
) {
    let processes = Paragraph::new(lines(info, area.width, theme))
        .style(Style::default().bg(theme.surface))
        .block(card(theme, "Top Processes", focused));

    frame.render_widget(processes, area);
}
