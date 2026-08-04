use std::time::Duration;

use crate::{theme::Theme, ui::renderer::muted};

use ratatui::{Frame, layout::Alignment, widgets::Paragraph};

pub(crate) fn draw(
    frame: &mut Frame,
    area: ratatui::layout::Rect,
    theme: &Theme,
    refresh_interval: Duration,
) {
    let footer = Paragraph::new(format!(
        "q/Esc/Ctrl+C sair  /  refresh {}s  /  NVIDIA via nvidia-smi",
        refresh_interval.as_secs()
    ))
    .alignment(Alignment::Center)
    .style(muted(theme));

    frame.render_widget(footer, area);
}
