use crate::{
    system::info::SystemInfo,
    theme::Theme,
    ui::renderer::{card, metric_line},
};

use ratatui::{Frame, style::Style, widgets::Paragraph};

pub(crate) fn draw_info(
    frame: &mut Frame,
    area: ratatui::layout::Rect,
    info: &SystemInfo,
    theme: &Theme,
    focused: bool,
) {
    let system_info = Paragraph::new(vec![
        metric_line(
            theme,
            "OS      ",
            format!("{}  kernel {}", info.os_name, info.kernel),
        ),
        metric_line(theme, "CPU     ", info.cpu_name.clone()),
        metric_line(
            theme,
            "THREADS ",
            format!("{} cores  {} processes", info.cpu_cores, info.processes),
        ),
        metric_line(
            theme,
            "LOAD    ",
            format!(
                "{:.2}  {:.2}  {:.2}",
                info.load_average.0, info.load_average.1, info.load_average.2
            ),
        ),
    ])
    .style(Style::default().fg(theme.text).bg(theme.surface))
    .block(card(theme, "System", focused));

    frame.render_widget(system_info, area);
}
