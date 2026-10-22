mod catppuccin_mocha;
mod pulse;
mod tokyo_night;

use ratatui::style::Color;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeKind {
    Pulse,
    CatppuccinMocha,
    TokyoNight,
}

impl ThemeKind {
    pub const ALL: [Self; 3] = [Self::Pulse, Self::CatppuccinMocha, Self::TokyoNight];

    pub fn label(self) -> &'static str {
        match self {
            Self::Pulse => "Pulse",
            Self::CatppuccinMocha => "Catppuccin Mocha",
            Self::TokyoNight => "Tokyo Night",
        }
    }

    pub fn theme(self) -> Theme {
        match self {
            Self::Pulse => pulse::make(),
            Self::CatppuccinMocha => catppuccin_mocha::make(),
            Self::TokyoNight => tokyo_night::make(),
        }
    }

    pub fn offset(self, direction: i32) -> Self {
        let index = Self::ALL
            .iter()
            .position(|candidate| *candidate == self)
            .unwrap_or(0) as i32;
        let next = (index + direction).rem_euclid(Self::ALL.len() as i32) as usize;
        Self::ALL[next]
    }
}

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
    /// Presentation-only palette for the central particle field. Geometry,
    /// telemetry and animation stay independent of these colors.
    pub(crate) fn pulse_scene(self) -> Self {
        if self != Self::pulse() {
            return self;
        }
        Self {
            secondary: self.pulse_high,
            accent: self.pulse_low,
            info: self.text_muted,
            ..self
        }
    }

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

    use super::{Theme, ThemeKind};

    #[test]
    fn pulse_contains_the_documented_semantic_colors() {
        let theme = Theme::pulse();

        assert_eq!(theme.background, Color::Rgb(0x08, 0x08, 0x08));
        assert_eq!(theme.primary, Color::Rgb(0x7B, 0xD8, 0x8F));
        assert_eq!(theme.secondary, Color::Rgb(0xC8, 0xC8, 0xC8));
        assert_eq!(theme.border_focused, Color::Rgb(0xEE, 0xEE, 0xEE));
        assert_eq!(theme.selection_background, Color::Rgb(0x30, 0x30, 0x30));
        assert_eq!(theme.danger, Color::Rgb(0xF0, 0x75, 0x75));
    }

    #[test]
    fn neutral_chrome_and_semantic_text_have_readable_contrast() {
        let theme = Theme::pulse();
        for color in [
            theme.background,
            theme.surface,
            theme.surface_alt,
            theme.text,
            theme.text_muted,
            theme.border,
            theme.border_focused,
            theme.secondary,
            theme.accent,
            theme.info,
            theme.selection_background,
        ] {
            let Color::Rgb(r, g, b) = color else {
                panic!("RGB palette required")
            };
            assert_eq!(r, g);
            assert_eq!(g, b);
        }
        fn luminance(color: Color) -> f64 {
            let Color::Rgb(r, g, b) = color else {
                panic!("RGB palette required")
            };
            let linear = |v: u8| {
                let v = f64::from(v) / 255.0;
                if v <= 0.04045 {
                    v / 12.92
                } else {
                    ((v + 0.055) / 1.055).powf(2.4)
                }
            };
            0.2126 * linear(r) + 0.7152 * linear(g) + 0.0722 * linear(b)
        }
        for background in [
            theme.background,
            theme.surface,
            theme.surface_alt,
            theme.selection_background,
        ] {
            for foreground in [
                theme.text,
                theme.text_muted,
                theme.success,
                theme.warning,
                theme.danger,
            ] {
                let contrast = (luminance(foreground) + 0.05) / (luminance(background) + 0.05);
                assert!(
                    contrast >= 4.5,
                    "{foreground:?} on {background:?}: {contrast}"
                );
            }
        }
    }

    #[test]
    fn cycles_through_the_documented_theme_presets() {
        assert_eq!(ThemeKind::Pulse.offset(1), ThemeKind::CatppuccinMocha);
        assert_eq!(ThemeKind::CatppuccinMocha.offset(1), ThemeKind::TokyoNight);
        assert_eq!(ThemeKind::Pulse.offset(-1), ThemeKind::TokyoNight);
        assert_ne!(ThemeKind::Pulse.theme(), ThemeKind::TokyoNight.theme());
    }
}
