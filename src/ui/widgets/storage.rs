use crate::{
    system::info::SystemInfo,
    theme::Theme,
    ui::renderer::{disk_usage, fit, label, panel, section_header, usage_color, value},
};

use ratatui::{
    Frame,
    style::Style,
    text::{Line, Span},
    widgets::Paragraph,
};

fn lines(info: &SystemInfo, area_width: u16, theme: &Theme) -> Vec<Line<'static>> {
    let mount_width = usize::from(area_width.saturating_sub(32)).clamp(6, 12);
    let mut lines = vec![section_header(
        theme,
        vec![
            ("DEVICE", 10),
            ("MOUNT", mount_width),
            ("USED", 12),
            ("%", 4),
        ],
    )];

    lines.extend(info.disks.iter().take(5).map(|disk| {
        let (used, percent) = disk_usage(disk.total_space, disk.available_space);
        Line::from(vec![
            Span::styled(fit(&disk.name, 10), value(theme)),
            Span::styled(fit(&disk.mount_point, mount_width), label(theme)),
            Span::styled(
                format!("{:>5.1}/{:<4.1} ", used, disk.total_space),
                value(theme),
            ),
            Span::styled(
                format!("{:>4.0}%", percent),
                usage_color(theme, percent as u16),
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
    let disks = Paragraph::new(lines(info, area.width, theme))
        .style(Style::default().bg(theme.surface))
        .block(panel(theme, "Storage", focused));

    frame.render_widget(disks, area);
}
