mod pulse;

use ratatui::style::Color;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Theme {
    pub(crate) background: Color,
    pub(crate) surface: Color,
    pub(crate) surface_alt: Color,
    pub(crate) text: Color,
    pub(crate) text_muted: Color,
    pub(crate) border: Color,
    pub(crate) border_focused: Color,
    pub(crate) primary: Color,
    pub(crate) secondary: Color,
    pub(crate) accent: Color,
    pub(crate) success: Color,
    pub(crate) warning: Color,
    pub(crate) danger: Color,
    pub(crate) info: Color,
    pub(crate) chart_primary: Color,
    pub(crate) chart_secondary: Color,
    pub(crate) chart_grid: Color,
    pub(crate) selection_background: Color,
    pub(crate) selection_text: Color,
    pub(crate) pulse_low: Color,
    pub(crate) pulse_medium: Color,
    pub(crate) pulse_high: Color,
}

impl Theme {
    pub fn pulse() -> Self {
        pulse::make()
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::pulse()
    }
}

#[cfg(test)]
mod tests {
    use ratatui::style::Color;

    use super::Theme;

    #[test]
    fn pulse_contains_the_documented_semantic_colors() {
        let theme = Theme::pulse();

        assert_eq!(theme.background, Color::Rgb(0x08, 0x0A, 0x0F));
        assert_eq!(theme.primary, Color::Rgb(0xA8, 0x55, 0xF7));
        assert_eq!(theme.secondary, Color::Rgb(0x22, 0xD3, 0xEE));
        assert_eq!(theme.danger, Color::Rgb(0xEF, 0x44, 0x44));
    }
}
