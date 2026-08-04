use crate::{
    system::info::SystemInfo,
    ui::renderer::{SURFACE, TEXT, card, metric_line},
};

use ratatui::{Frame, style::Style, widgets::Paragraph};

pub(crate) fn draw_info(frame: &mut Frame, area: ratatui::layout::Rect, info: &SystemInfo) {
    let system_info = Paragraph::new(vec![
        metric_line(
            "OS      ",
            format!("{}  kernel {}", info.os_name, info.kernel),
        ),
        metric_line("CPU     ", info.cpu_name.clone()),
        metric_line(
            "THREADS ",
            format!("{} cores  {} processes", info.cpu_cores, info.processes),
        ),
        metric_line(
            "LOAD    ",
            format!(
                "{:.2}  {:.2}  {:.2}",
                info.load_average.0, info.load_average.1, info.load_average.2
            ),
        ),
    ])
    .style(Style::default().fg(TEXT).bg(SURFACE))
    .block(card("System"));

    frame.render_widget(system_info, area);
}
