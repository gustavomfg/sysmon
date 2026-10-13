//! Small shared primitives for capacity and telemetry units. No collection.
use crate::theme::Theme;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
};

/// A quiet capacity rail with half-cell resolution; absent data is never zero.
pub(super) fn capacity_meter(
    area: Rect,
    ratio: Option<f64>,
    color: Color,
    theme: &Theme,
    buffer: &mut Buffer,
) {
    if area.is_empty() {
        return;
    }
    let ratio = ratio.filter(|value| value.is_finite());
    let halves =
        (ratio.unwrap_or(0.0).clamp(0.0, 1.0) * f64::from(area.width) * 2.0).round() as u16;
    let clip = *buffer.area();
    for index in 0..area.width {
        let x = area.x.saturating_add(index);
        if x < clip.x || x >= clip.right() || area.y < clip.y || area.y >= clip.bottom() {
            continue;
        }
        let coverage = halves.saturating_sub(index.saturating_mul(2)).min(2);
        let (symbol, fg) = match coverage {
            2 => ('━', color),
            1 => ('╸', color),
            _ => ('─', theme.border),
        };
        buffer[(x, area.y)]
            .set_char(symbol)
            .set_style(Style::default().fg(fg).bg(theme.surface));
    }
    if ratio.is_none()
        && area.x >= clip.x
        && area.x < clip.right()
        && area.y >= clip.y
        && area.y < clip.bottom()
    {
        buffer[(area.x, area.y)]
            .set_char('—')
            .set_fg(theme.text_muted);
    }
}

pub(crate) fn format_bytes(value: f64) -> String {
    if !value.is_finite() || value < 0.0 {
        return "—".to_owned();
    }
    for (divisor, unit) in [
        (1024_f64.powi(4), "TiB"),
        (1024_f64.powi(3), "GiB"),
        (1024_f64.powi(2), "MiB"),
        (1024.0, "KiB"),
    ] {
        if value >= divisor {
            return format!("{:.1} {unit}", value / divisor);
        }
    }
    format!("{value:.0} B")
}

pub(super) fn format_rate(value: f64) -> String {
    if !value.is_finite() || value < 0.0 {
        return "—".to_owned();
    }
    format!("{}/s", format_bytes(value))
}

/// Compact but unambiguous binary units for axis labels and extrema.
pub(super) fn short_rate(value: f64) -> String {
    if !value.is_finite() || value < 0.0 {
        return "—".to_owned();
    }
    if value >= 1024_f64.powi(5) {
        return format!("{value:.0e}B/s");
    }
    for (divisor, unit) in [
        (1024_f64.powi(4), "TiB"),
        (1024_f64.powi(3), "GiB"),
        (1024_f64.powi(2), "MiB"),
        (1024.0, "KiB"),
    ] {
        if value >= divisor {
            let number = value / divisor;
            return if number.fract().abs() < 0.01 {
                format!("{number:.0}{unit}/s")
            } else {
                format!("{number:.1}{unit}/s")
            };
        }
    }
    format!("{value:.0}B/s")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn units_are_binary_and_invalid_values_are_explicit() {
        assert_eq!(format_bytes(1024.0), "1.0 KiB");
        assert_eq!(format_rate(1_048_576.0), "1.0 MiB/s");
        assert_eq!(short_rate(4_194_304.0), "4MiB/s");
        for invalid in [f64::NAN, f64::INFINITY, -1.0] {
            assert_eq!(format_rate(invalid), "—");
        }
    }
    #[test]
    fn capacity_handles_missing_data_half_cells_and_clipping() {
        let theme = Theme::pulse();
        let full = Rect::new(0, 0, 8, 1);
        let mut buffer = Buffer::empty(full);
        capacity_meter(full, Some(0.4375), theme.primary, &theme, &mut buffer);
        let symbols: String = buffer.content.iter().map(|cell| cell.symbol()).collect();
        assert_eq!(symbols, "━━━╸────");
        let mut clipped = Buffer::empty(Rect::new(2, 0, 3, 1));
        capacity_meter(full, Some(0.4375), theme.primary, &theme, &mut clipped);
        assert_eq!(clipped.content, &buffer.content[2..5]);
        capacity_meter(full, None, theme.primary, &theme, &mut buffer);
        assert_eq!(buffer[(0, 0)].symbol(), "—");
    }
}
