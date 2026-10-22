use ratatui::style::Color;

use super::Theme;

pub(super) fn make() -> Theme {
    Theme {
        // Neutral instruments: green is normal telemetry; yellow/red encode
        // attention and critical states. Focus and information stay achromatic.
        background: Color::Rgb(0x08, 0x08, 0x08),
        surface: Color::Rgb(0x10, 0x10, 0x10),
        surface_alt: Color::Rgb(0x20, 0x20, 0x20),
        text: Color::Rgb(0xFF, 0xFF, 0xFF),
        text_muted: Color::Rgb(0xFF, 0xFF, 0xFF),
        border: Color::Rgb(0x42, 0x42, 0x42),
        border_focused: Color::Rgb(0xFF, 0xFF, 0xFF),
        primary: Color::Rgb(0x7B, 0xD8, 0x8F),
        secondary: Color::Rgb(0xC8, 0xC8, 0xC8),
        accent: Color::Rgb(0xD8, 0xD8, 0xD8),
        success: Color::Rgb(0x7B, 0xD8, 0x8F),
        warning: Color::Rgb(0xE8, 0xC6, 0x5A),
        danger: Color::Rgb(0xF0, 0x75, 0x75),
        info: Color::Rgb(0xC8, 0xC8, 0xC8),
        chart_primary: Color::Rgb(0x7B, 0xD8, 0x8F),
        chart_secondary: Color::Rgb(0xC8, 0xC8, 0xC8),
        chart_grid: Color::Rgb(0x30, 0x30, 0x30),
        selection_background: Color::Rgb(0x30, 0x30, 0x30),
        selection_text: Color::Rgb(0xFF, 0xFF, 0xFF),
        pulse_low: Color::Rgb(0x31, 0x5D, 0x3B),
        pulse_medium: Color::Rgb(0x7B, 0xD8, 0x8F),
        pulse_high: Color::Rgb(0xE3, 0xF2, 0xE6),
    }
}
