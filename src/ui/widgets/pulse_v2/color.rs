use ratatui::style::Color;

use crate::theme::Theme;

use super::{FieldDimensions, FieldError, LightingField, NormalizedPulseInput};

/// A semantic color assigned to one perceptual lighting sample.
///
/// The color is presentation data only. It never feeds back into the energy,
/// structure or lighting buffers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SemanticColorSample {
    color: Color,
    spatial_position: u8,
    critical_mix: u8,
    hotspot: u8,
}

impl SemanticColorSample {
    pub fn color(self) -> Color {
        self.color
    }

    pub fn spatial_position(self) -> f32 {
        f32::from(self.spatial_position) / 255.0
    }

    pub fn critical_mix(self) -> f32 {
        f32::from(self.critical_mix) / 255.0
    }

    pub fn hotspot(self) -> f32 {
        f32::from(self.hotspot) / 255.0
    }
}

/// Semantic colors for the already-rasterizable Pulse V2 lighting field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticColorField {
    dimensions: FieldDimensions,
    values: Vec<SemanticColorSample>,
}

impl SemanticColorField {
    pub fn from_lighting(
        lighting: &LightingField,
        input: NormalizedPulseInput,
        theme: &Theme,
    ) -> Result<Self, FieldError> {
        let dimensions = lighting.dimensions();
        let mut colors = Self {
            dimensions,
            values: vec![
                SemanticColorSample {
                    color: theme.info,
                    spatial_position: 0,
                    critical_mix: 0,
                    hotspot: 0,
                };
                dimensions.sample_count()
            ],
        };
        let stress = system_stress(input);
        let criticality = smoothstep((stress - 0.46) / 0.30);
        let state_purple_shift = smoothstep((stress - 0.18) / 0.55) * 0.10;
        let width = dimensions.width();
        let height = dimensions.height();
        let x_denominator = width.saturating_sub(1).max(1) as f32;
        let y_denominator = height.saturating_sub(1).max(1) as f32;

        for y in 0..height {
            let y_position = if height <= 1 {
                0.5
            } else {
                y as f32 / y_denominator
            };
            for x in 0..width {
                let sample = lighting.as_slice()[y * width + x];
                if sample.intensity() <= f32::EPSILON {
                    continue;
                }
                let x_position = if width <= 1 {
                    0.5
                } else {
                    x as f32 / x_denominator
                };
                let vertical_position = 1.0 - y_position;
                let organized_energy = (sample.relative_energy() * 0.52
                    + sample.depth() * 0.28
                    + sample.local_highlight() * 0.20)
                    .clamp(0.0, 1.0);
                let spatial_position = (vertical_position * 0.60
                    + organized_energy * 0.25
                    + sample.local_highlight() * 0.08
                    + (x_position - 0.5) * (0.06 + sample.depth() * 0.04)
                    + (organized_energy - 0.5) * 0.035
                    + state_purple_shift * (0.45 + organized_energy * 0.55))
                    .clamp(0.0, 1.0);

                let blue_to_cyan = smoothstep((spatial_position - 0.02) / 0.34);
                let cyan_to_purple = smoothstep((spatial_position - 0.30) / 0.42);
                let purple_to_magenta = smoothstep((spatial_position - 0.62) / 0.35);
                let mut color = blend_color(theme.info, theme.secondary, blue_to_cyan);
                color = blend_color(color, theme.primary, cyan_to_purple);
                color = blend_color(color, theme.accent, purple_to_magenta);

                let energetic_region = smoothstep((organized_energy - 0.46) / 0.42);
                let critical_mix = criticality
                    * energetic_region
                    * (0.38
                        + sample.local_highlight() * 0.34
                        + if sample.is_peak() { 0.18 } else { 0.0 });
                color = blend_color(color, theme.danger, critical_mix * 0.72);

                let hotspot = if sample.is_peak() {
                    smoothstep((sample.intensity() - 0.54) / 0.46)
                } else {
                    0.0
                };
                color = blend_color(color, theme.text, hotspot * 0.68);

                colors.values[y * width + x] = SemanticColorSample {
                    color,
                    spatial_position: quantize(spatial_position),
                    critical_mix: quantize(critical_mix),
                    hotspot: quantize(hotspot),
                };
            }
        }

        Ok(colors)
    }

    pub fn dimensions(&self) -> FieldDimensions {
        self.dimensions
    }

    pub fn width(&self) -> usize {
        self.dimensions.width()
    }

    pub fn height(&self) -> usize {
        self.dimensions.height()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub fn as_slice(&self) -> &[SemanticColorSample] {
        &self.values
    }

    pub fn read(&self, x: usize, y: usize) -> Option<SemanticColorSample> {
        if x >= self.width() || y >= self.height() {
            return None;
        }
        Some(self.values[y * self.width() + x])
    }
}

fn system_stress(input: NormalizedPulseInput) -> f32 {
    (input.cpu() * 0.30
        + input.memory() * 0.22
        + input.gpu().unwrap_or(0.0) * 0.16
        + input.network() * 0.10
        + input.load() * 0.22)
        .clamp(0.0, 1.0)
}

fn smoothstep(value: f32) -> f32 {
    let value = value.clamp(0.0, 1.0);
    value * value * (3.0 - 2.0 * value)
}

fn quantize(value: f32) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn blend_color(from: Color, to: Color, amount: f32) -> Color {
    let amount = amount.clamp(0.0, 1.0);
    match (from, to) {
        (Color::Rgb(fr, fg, fb), Color::Rgb(tr, tg, tb)) => Color::Rgb(
            interpolate_channel(fr, tr, amount),
            interpolate_channel(fg, tg, amount),
            interpolate_channel(fb, tb, amount),
        ),
        (_, target) if amount >= 0.5 => target,
        (source, _) => source,
    }
}

fn interpolate_channel(from: u8, to: u8, amount: f32) -> u8 {
    (from as f32 + (to as f32 - from as f32) * amount)
        .round()
        .clamp(0.0, 255.0) as u8
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::super::{FieldDimensions, NormalizedPulseInput, PulseV2Engine, PulseV2Preset};
    use super::{SemanticColorField, system_stress};
    use crate::pulse::PulseInput;
    use crate::theme::Theme;

    fn input(cpu: f32, memory: f32, gpu: Option<f32>, load: f32) -> NormalizedPulseInput {
        NormalizedPulseInput::try_new(cpu, memory, gpu, 0.20, load, 0.08).unwrap()
    }

    fn generated(input: NormalizedPulseInput, reduced_motion: bool) -> super::super::PulseV2Frame {
        PulseV2Engine::new()
            .generate_with_preset(
                FieldDimensions::new(80, 20).unwrap(),
                input,
                PulseV2Preset::Widescreen,
                Duration::from_secs(3),
                reduced_motion,
            )
            .unwrap()
    }

    #[test]
    fn mapping_is_deterministic_and_does_not_mutate_lighting() {
        let frame = generated(input(0.22, 0.20, Some(0.18), 0.18), false);
        let before = frame.lighting().clone();
        let theme = Theme::pulse();
        let first =
            SemanticColorField::from_lighting(frame.lighting(), frame.input(), &theme).unwrap();
        let second =
            SemanticColorField::from_lighting(frame.lighting(), frame.input(), &theme).unwrap();

        assert_eq!(first, second);
        assert_eq!(&before, frame.lighting());
        assert_eq!(first.dimensions(), frame.dimensions());
        assert!(first.as_slice().iter().all(|sample| {
            sample.spatial_position().is_finite()
                && sample.critical_mix().is_finite()
                && sample.hotspot().is_finite()
        }));
    }

    #[test]
    fn spatial_mapping_moves_from_blue_lower_plane_to_purple_upper_plane() {
        let frame = generated(input(0.22, 0.20, Some(0.18), 0.18), false);
        let colors =
            SemanticColorField::from_lighting(frame.lighting(), frame.input(), &Theme::pulse())
                .unwrap();
        let mut top = None;
        let mut bottom = None;
        for y in 0..frame.lighting().height() {
            for x in 0..frame.lighting().width() {
                if frame.lighting().read(x, y).unwrap().intensity() <= f32::EPSILON {
                    continue;
                }
                let sample = colors.read(x, y).unwrap();
                if top.is_none() {
                    top = Some(sample);
                }
                bottom = Some(sample);
            }
        }
        let top = top.unwrap();
        let bottom = bottom.unwrap();

        assert!(bottom.spatial_position() < top.spatial_position());
        assert_ne!(bottom.color(), top.color());
    }

    #[test]
    fn calm_has_no_critical_color_and_peaks_are_the_only_hotspots() {
        let frame = generated(input(0.12, 0.16, None, 0.10), false);
        let colors =
            SemanticColorField::from_lighting(frame.lighting(), frame.input(), &Theme::pulse())
                .unwrap();

        assert!(system_stress(frame.input()) < 0.46);
        assert!(
            colors
                .as_slice()
                .iter()
                .all(|sample| sample.critical_mix() == 0.0)
        );
        let peak_count = frame
            .lighting()
            .as_slice()
            .iter()
            .filter(|sample| sample.is_peak())
            .count();
        let hotspot_count = colors
            .as_slice()
            .iter()
            .filter(|sample| sample.hotspot() > 0.0)
            .count();
        assert!(hotspot_count <= peak_count);
        if peak_count > 0 {
            assert!(hotspot_count > 0);
        }
    }

    #[test]
    fn critical_color_is_localized_to_energetic_regions() {
        let frame = generated(input(0.58, 0.62, Some(0.55), 0.60), false);
        let colors =
            SemanticColorField::from_lighting(frame.lighting(), frame.input(), &Theme::pulse())
                .unwrap();
        let colored = colors
            .as_slice()
            .iter()
            .filter(|sample| sample.critical_mix() > 0.01)
            .count();
        let active = frame
            .lighting()
            .as_slice()
            .iter()
            .filter(|sample| sample.intensity() > 0.02)
            .count();

        assert!(colored > 0);
        assert!(colored < active.max(1));
    }

    #[test]
    fn busier_states_shift_the_existing_gradient_toward_purple() {
        let calm = generated(input(0.12, 0.16, None, 0.10), false);
        let busy = generated(input(0.40, 0.45, Some(0.30), 0.45), false);
        let theme = Theme::pulse();
        let calm_colors =
            SemanticColorField::from_lighting(calm.lighting(), calm.input(), &theme).unwrap();
        let busy_colors =
            SemanticColorField::from_lighting(busy.lighting(), busy.input(), &theme).unwrap();
        let calm_mean = calm_colors
            .as_slice()
            .iter()
            .map(|sample| sample.spatial_position())
            .sum::<f32>()
            / calm_colors.as_slice().len() as f32;
        let busy_mean = busy_colors
            .as_slice()
            .iter()
            .map(|sample| sample.spatial_position())
            .sum::<f32>()
            / busy_colors.as_slice().len() as f32;

        assert!(busy_mean > calm_mean);
        assert!(
            busy_colors
                .as_slice()
                .iter()
                .all(|sample| sample.critical_mix() == 0.0)
        );
    }

    #[test]
    fn reduced_motion_preserves_the_same_semantic_colors() {
        let first = generated(input(0.24, 0.28, Some(0.20), 0.24), true);
        let later = PulseV2Engine::new()
            .generate_with_preset(
                first.dimensions(),
                first.input(),
                first.preset(),
                Duration::from_secs(30),
                true,
            )
            .unwrap();
        let theme = Theme::pulse();
        let first_colors =
            SemanticColorField::from_lighting(first.lighting(), first.input(), &theme).unwrap();
        let later_colors =
            SemanticColorField::from_lighting(later.lighting(), later.input(), &theme).unwrap();

        assert_eq!(first_colors, later_colors);
    }

    #[test]
    fn theme_tokens_are_the_only_color_sources() {
        let theme = Theme::pulse();
        let input = PulseInput {
            cpu: 0.0,
            memory: 0.0,
            gpu: None,
            network: 0.0,
            load: 0.0,
            volatility: 0.0,
        };
        let normalized = NormalizedPulseInput::from(input);
        let frame = generated(normalized, true);
        let colors =
            SemanticColorField::from_lighting(frame.lighting(), normalized, &theme).unwrap();

        assert!(!colors.is_empty());
        assert_eq!(theme.info, Theme::pulse().info);
        assert_eq!(theme.secondary, Theme::pulse().secondary);
        assert_eq!(theme.primary, Theme::pulse().primary);
        assert_eq!(theme.accent, Theme::pulse().accent);
        assert_eq!(theme.danger, Theme::pulse().danger);
        assert_eq!(theme.text, Theme::pulse().text);
    }
}
