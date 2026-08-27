use ratatui::style::Color;

use super::Theme;

pub(super) fn make() -> Theme {
    Theme {
        // A restrained terminal palette: green is the normal signal, blue is
        // informational, and amber/red are reserved for state changes.
        background: Color::Rgb(0x0B, 0x10, 0x14),
        surface: Color::Rgb(0x10, 0x17, 0x1D),
        surface_alt: Color::Rgb(0x17, 0x22, 0x2A),
        text: Color::Rgb(0xD9, 0xE2, 0xE8),
        text_muted: Color::Rgb(0x7E, 0x8D, 0x96),
        border: Color::Rgb(0x2A, 0x3A, 0x43),
        border_focused: Color::Rgb(0x7B, 0xD8, 0x8F),
        primary: Color::Rgb(0x7B, 0xD8, 0x8F),
        secondary: Color::Rgb(0x6A, 0xA6, 0xFF),
        accent: Color::Rgb(0xD7, 0xBA, 0x7D),
        success: Color::Rgb(0x7B, 0xD8, 0x8F),
        warning: Color::Rgb(0xE5, 0xB5, 0x67),
        danger: Color::Rgb(0xF0, 0x75, 0x75),
        info: Color::Rgb(0x6A, 0xA6, 0xFF),
        chart_primary: Color::Rgb(0x7B, 0xD8, 0x8F),
        chart_secondary: Color::Rgb(0x6A, 0xA6, 0xFF),
        chart_grid: Color::Rgb(0x25, 0x34, 0x3D),
        selection_background: Color::Rgb(0x1D, 0x3A, 0x2B),
        selection_text: Color::Rgb(0xF1, 0xF7, 0xF3),
        pulse_low: Color::Rgb(0x2D, 0x6A, 0x46),
        pulse_medium: Color::Rgb(0x7B, 0xD8, 0x8F),
        pulse_high: Color::Rgb(0x6A, 0xA6, 0xFF),
    }
}
