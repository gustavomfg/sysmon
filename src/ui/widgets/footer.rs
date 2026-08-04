use crate::{theme::Theme, ui::renderer::muted};

use ratatui::{Frame, layout::Alignment, widgets::Paragraph};

pub(crate) fn draw(frame: &mut Frame, area: ratatui::layout::Rect, theme: &Theme) {
    let footer = Paragraph::new("q/Esc/Ctrl+C sair  /  refresh 1s  /  NVIDIA via nvidia-smi")
        .alignment(Alignment::Center)
        .style(muted(theme));

    frame.render_widget(footer, area);
}
