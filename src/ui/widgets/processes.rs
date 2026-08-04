use crate::{
    system::info::SystemInfo,
    theme::Theme,
    ui::renderer::{fit, panel, section_header, usage_color, value},
};

use ratatui::{
    Frame,
    style::Style,
    text::{Line, Span},
    widgets::Paragraph,
};

fn lines(
    info: &SystemInfo,
    area_width: u16,
    theme: &Theme,
    selected_process: Option<&str>,
) -> Vec<Line<'static>> {
    let process_width = usize::from(area_width.saturating_sub(20)).clamp(8, 24);
    let mut lines = vec![section_header(
        theme,
        vec![("PROCESS", process_width), ("MEM", 9), ("CPU", 5)],
    )];

    lines.extend(info.processes_list.iter().take(5).map(|process| {
        let selected = selected_process == Some(process.name.as_str());
        let process_style = if selected {
            Style::default()
                .fg(theme.selection_text)
                .bg(theme.selection_background)
        } else {
            value(theme)
        };
        Line::from(vec![
            Span::styled(fit(&process.name, process_width), process_style),
            Span::styled(
                format!("{:>7.1}M ", process.memory),
                if selected {
                    process_style
                } else {
                    Style::default().fg(theme.text_muted)
                },
            ),
            Span::styled(
                format!("{:>5.1}%", process.cpu_usage),
                if selected {
                    process_style
                } else {
                    usage_color(theme, process.cpu_usage as u16)
                },
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
    selected_process: Option<&str>,
) {
    let processes = Paragraph::new(lines(info, area.width, theme, selected_process))
        .style(Style::default().bg(theme.surface))
        .block(panel(theme, "Top Processes", focused));

    frame.render_widget(processes, area);
}
