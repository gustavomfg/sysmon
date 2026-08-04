use crate::{
    system::info::SystemInfo,
    ui::renderer::{OK, WARN, card, fit, section_header, value},
};

use ratatui::{
    Frame,
    style::Style,
    text::{Line, Span},
    widgets::Paragraph,
};

fn lines(info: &SystemInfo, area_width: u16) -> Vec<Line<'static>> {
    let interface_width = usize::from(area_width.saturating_sub(29)).clamp(7, 14);
    let mut lines = vec![section_header(vec![
        ("IFACE", interface_width),
        ("DOWN", 13),
        ("UP", 12),
    ])];

    lines.extend(info.networks.iter().map(|network| {
        Line::from(vec![
            Span::styled(fit(&network.interface, interface_width), value()),
            Span::styled(
                format!("{:>8.2} MB/s ", network.received_per_sec),
                Style::default().fg(OK),
            ),
            Span::styled(
                format!("{:>7.2} MB/s", network.transmitted_per_sec),
                Style::default().fg(WARN),
            ),
        ])
    }));
    lines
}

pub(crate) fn draw(frame: &mut Frame, area: ratatui::layout::Rect, info: &SystemInfo) {
    let network = Paragraph::new(lines(info, area.width))
        .style(Style::default().bg(ratatui::style::Color::Black))
        .block(card("Network"));

    frame.render_widget(network, area);
}
