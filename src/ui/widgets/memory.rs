use crate::{
    system::info::SystemInfo,
    ui::renderer::{BACKGROUND, card, label, meter, usage_color, usage_percent},
};

use ratatui::{
    Frame,
    layout::Alignment,
    style::Style,
    text::{Line, Span},
    widgets::Paragraph,
};

pub(crate) fn draw(frame: &mut Frame, area: ratatui::layout::Rect, info: &SystemInfo) {
    let ram_percent = usage_percent(info.used_ram, info.total_ram) as u16;

    let mut ram_line = vec![Span::styled("RAM ", label())];
    ram_line.extend(meter(ram_percent, 28));
    ram_line.push(Span::styled(
        format!(" {:>3}%", ram_percent),
        usage_color(ram_percent),
    ));

    let ram = Paragraph::new(Line::from(ram_line))
        .alignment(Alignment::Center)
        .style(Style::default().bg(BACKGROUND))
        .block(card(format!(
            "RAM {:.1}/{:.1}GB  Swap {:.1}/{:.1}GB",
            info.used_ram, info.total_ram, info.used_swap, info.total_swap
        )));

    frame.render_widget(ram, area);
}
