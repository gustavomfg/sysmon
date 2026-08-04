use crate::{app::AppState, theme::Theme, ui::renderer::panel};

use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

const BRAILLE_DOTS: [(u16, u16, u8); 8] = [
    (0, 0, 0),
    (0, 1, 1),
    (0, 2, 2),
    (1, 0, 3),
    (1, 1, 4),
    (1, 2, 5),
    (0, 3, 6),
    (1, 3, 7),
];

pub(crate) fn draw(frame: &mut Frame, area: Rect, app: &AppState, theme: &Theme, focused: bool) {
    let pulse = app.pulse();
    let intensity = pulse.intensity();
    let title = format!("System Pulse  {:>3.0}%", intensity * 100.0);
    let lines = braille_lines(area, pulse, theme);
    let widget = Paragraph::new(lines)
        .style(Style::default().bg(theme.surface))
        .block(panel(theme, title, focused));
    frame.render_widget(widget, area);
}

fn braille_lines(
    area: Rect,
    pulse: &crate::pulse::PulseModel,
    theme: &Theme,
) -> Vec<Line<'static>> {
    let width = usize::from(area.width.saturating_sub(4)).max(1);
    let height = usize::from(area.height.saturating_sub(2)).max(1);
    let logical_width = width * 2;
    let logical_height = height * 4;
    let center_x = logical_width as f32 / 2.0;
    let center_y = logical_height as f32 / 2.0;
    let intensity = pulse.intensity();
    let radius = (logical_width.min(logical_height) as f32)
        * (0.27 + intensity * 0.08 + (pulse.breath() - 0.5) * 0.04);
    let thickness = 0.8 + intensity * 1.4;
    let phase = pulse.phase();
    let color = pulse_color(theme, intensity);

    (0..height)
        .map(|cell_y| {
            let symbols = (0..width)
                .map(|cell_x| {
                    let mut mask = 0_u8;
                    for (dot_x, dot_y, bit) in BRAILLE_DOTS {
                        let x = (cell_x * 2 + usize::from(dot_x)) as f32;
                        let y = (cell_y * 4 + usize::from(dot_y)) as f32;
                        let dx = x - center_x;
                        let dy = y - center_y;
                        let turbulence =
                            pulse.input().volatility * 1.5 * ((x * 0.17 + y * 0.11 + phase).sin());
                        let distance = (dx * dx + dy * dy).sqrt() + turbulence;
                        if (distance - radius).abs() <= thickness {
                            mask |= 1 << bit;
                        }
                    }
                    char::from_u32(0x2800 + u32::from(mask)).unwrap_or(' ')
                })
                .collect::<String>();
            Line::from(Span::styled(symbols, Style::default().fg(color)))
        })
        .collect()
}

fn pulse_color(theme: &Theme, intensity: f32) -> Color {
    match intensity {
        value if value < 0.35 => theme.pulse_low,
        value if value < 0.70 => theme.pulse_medium,
        _ => theme.pulse_high,
    }
}
