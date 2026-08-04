use crate::{
    system::info::SystemInfo,
    theme::Theme,
    ui::renderer::{label, meter, panel, usage_color},
};

use ratatui::{
    Frame,
    layout::Alignment,
    style::Style,
    text::{Line, Span},
    widgets::Paragraph,
};

pub(crate) fn draw(
    frame: &mut Frame,
    area: ratatui::layout::Rect,
    info: &SystemInfo,
    memory_percent: f64,
    theme: &Theme,
    focused: bool,
) {
    let ram_percent = memory_percent.clamp(0.0, 100.0) as u16;

    let mut ram_line = vec![Span::styled("RAM ", label(theme))];
    let meter_width = usize::from(area.width.saturating_sub(18)).clamp(8, 28);
    ram_line.extend(meter(theme, ram_percent, meter_width));
    ram_line.push(Span::styled(
        format!(" {:>3}%", ram_percent),
        usage_color(theme, ram_percent),
    ));

    let ram = Paragraph::new(Line::from(ram_line))
        .alignment(Alignment::Center)
        .style(Style::default().bg(theme.surface))
        .block(panel(
            theme,
            format!(
                "RAM {:.1}/{:.1}GB  Swap {:.1}/{:.1}GB",
                info.used_ram, info.total_ram, info.used_swap, info.total_swap
            ),
            focused,
        ));

    frame.render_widget(ram, area);
}
