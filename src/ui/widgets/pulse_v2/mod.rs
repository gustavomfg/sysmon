//! Experimental Pulse V2 infrastructure.
//!
//! This module deliberately stops before simulation and rasterization.  Pulse V1
//! remains the only renderer used by the dashboard until a later phase proves
//! the V2 visual pipeline.

// V2 is intentionally registered before integration. Its public contracts are
// exercised by this module's pure tests and will be consumed by later phases.
#![allow(dead_code, unused_imports)]

mod engine;
mod field;
mod noise;
mod types;

pub use engine::{PulseV2Engine, PulseV2Frame};
pub use field::{FieldError, FieldGradient, ScalarField};
pub use noise::NoiseField;
pub use types::{
    CoordinateError, DimensionsError, FieldDimensions, InputError, NormalizedPoint,
    NormalizedPulseInput, PresetParameters, PulseV2Preset,
};

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::pulse::PulseInput;

    use super::{
        FieldDimensions, FieldGradient, NoiseField, NormalizedPoint, NormalizedPulseInput,
        PulseV2Engine, PulseV2Frame, PulseV2Preset, ScalarField,
    };

    #[test]
    fn selects_all_responsive_presets_from_the_received_area() {
        assert_eq!(
            PulseV2Preset::from_dimensions(FieldDimensions::new(30, 8).unwrap()),
            PulseV2Preset::Compact
        );
        assert_eq!(
            PulseV2Preset::from_dimensions(FieldDimensions::new(50, 12).unwrap()),
            PulseV2Preset::Standard
        );
        assert_eq!(
            PulseV2Preset::from_dimensions(FieldDimensions::new(80, 20).unwrap()),
            PulseV2Preset::Widescreen
        );
        assert_eq!(
            PulseV2Preset::from_dimensions(FieldDimensions::new(140, 40).unwrap()),
            PulseV2Preset::LargeWidescreen
        );
        assert!(
            PulseV2Preset::Compact.parameters().sample_limit()
                < PulseV2Preset::Standard.parameters().sample_limit()
        );
        assert!(
            PulseV2Preset::Standard.parameters().sample_limit()
                < PulseV2Preset::Widescreen.parameters().sample_limit()
        );
        assert_eq!(PulseV2Preset::Compact.parameters().complexity(), 1);
        assert_eq!(PulseV2Preset::LargeWidescreen.parameters().complexity(), 4);
    }

    #[test]
    fn zero_dimensions_create_an_explicit_empty_field() {
        let dimensions = FieldDimensions::new(0, 0).unwrap();
        let field = ScalarField::try_new(dimensions).unwrap();

        assert!(dimensions.is_empty());
        assert_eq!(dimensions.sample_count(), 0);
        assert_eq!(field.as_slice(), &[]);
        assert_eq!(field.read(0, 0), None);
    }

    #[test]
    fn representative_and_minimum_dimensions_are_contiguous_and_bounded() {
        for (width, height) in [(1, 1), (2, 1), (1, 2), (160, 80), (180, 50)] {
            let dimensions = FieldDimensions::new(width, height).unwrap();
            let field = ScalarField::try_new(dimensions).unwrap();

            assert_eq!(field.as_slice().len(), width * height);
            assert!(field.as_slice().iter().all(|value| value.is_finite()));
            assert!(
                field
                    .as_slice()
                    .iter()
                    .all(|value| (0.0..=1.0).contains(value))
            );
        }
    }

    #[test]
    fn excessive_dimensions_are_rejected_without_allocating() {
        assert!(FieldDimensions::new(usize::MAX, 2).is_err());
        assert!(FieldDimensions::new(4097, 1).is_err());
        assert!(FieldDimensions::new(1024, 1024).is_ok());
    }

    #[test]
    fn field_supports_safe_read_write_and_fill_operations() {
        let dimensions = FieldDimensions::new(4, 3).unwrap();
        let mut field = ScalarField::try_new(dimensions).unwrap();

        field.write(2, 1, 0.75).unwrap();
        assert_eq!(field.read(2, 1), Some(0.75));
        assert_eq!(field.read(4, 1), None);
        assert!(field.write(4, 1, 0.5).is_err());
        assert!(field.write(0, 0, f32::NAN).is_err());
        assert!(field.write(0, 0, 1.1).is_err());

        field.fill(0.25).unwrap();
        assert!(field.as_slice().iter().all(|value| *value == 0.25));
        assert!(field.fill(f32::INFINITY).is_err());
    }

    #[test]
    fn normalized_contracts_reject_invalid_values_and_sanitize_v1_input() {
        let point = NormalizedPoint::try_new(0.5, 0.5).unwrap();
        assert_eq!((point.x(), point.y()), (0.5, 0.5));
        assert!(NormalizedPoint::try_new(-0.1, 0.5).is_err());
        assert!(NormalizedPoint::try_new(0.5, f32::INFINITY).is_err());

        assert!(NormalizedPulseInput::try_new(0.0, 1.0, None, 0.5, 0.25, 0.0).is_ok());
        assert!(NormalizedPulseInput::try_new(1.1, 0.0, None, 0.0, 0.0, 0.0).is_err());

        let input = NormalizedPulseInput::from(PulseInput {
            cpu: f32::NAN,
            memory: 1.5,
            gpu: Some(-0.5),
            network: 0.5,
            load: f32::INFINITY,
            volatility: 0.2,
        });
        assert_eq!(input.cpu(), 0.0);
        assert_eq!(input.memory(), 1.0);
        assert_eq!(input.gpu(), Some(0.0));
        assert_eq!(input.load(), 0.0);
    }

    #[test]
    fn empty_engine_frames_are_valid_and_deterministic() {
        let input = NormalizedPulseInput::from(PulseInput::default());
        let dimensions = FieldDimensions::new(80, 20).unwrap();
        let engine = PulseV2Engine::new();

        let first = engine.empty_frame(dimensions, input).unwrap();
        let same = engine.empty_frame(dimensions, input).unwrap();

        assert_eq!(first, same);
        assert_eq!(first.dimensions(), dimensions);
        assert_eq!(first.preset(), PulseV2Preset::Widescreen);
        assert_eq!(first.input(), input);
        assert_eq!(first.field().as_slice().len(), 80 * 20);
        assert!(first.field().as_slice().iter().all(|value| *value == 0.0));
        assert!(first.is_empty());
    }

    #[test]
    fn engine_accepts_zero_dimensions_without_terminal_specific_behavior() {
        let engine = PulseV2Engine::new();
        let frame = engine
            .empty_frame(
                FieldDimensions::new(0, 0).unwrap(),
                NormalizedPulseInput::default(),
            )
            .unwrap();

        assert_eq!(frame.preset(), PulseV2Preset::Compact);
        assert!(frame.field().is_empty());
        assert!(frame.is_empty());
    }

    #[test]
    fn generated_field_is_populated_finite_and_bounded() {
        let frame = generated(sample_input(), 0.0, false);
        let values = frame.field().as_slice();

        assert!(!frame.is_empty());
        assert!(values.iter().any(|value| *value > 0.0));
        assert!(values.iter().all(|value| value.is_finite()));
        assert!(values.iter().all(|value| (0.0..=1.0).contains(value)));
    }

    #[test]
    fn field_has_negative_space_midtone_energy_and_rare_peaks() {
        let frame = generated(sample_input(), 0.0, false);
        let values = frame.field().as_slice();
        let silent = values.iter().filter(|value| **value < 0.025).count();
        let midtones = values
            .iter()
            .filter(|value| (0.08..0.52).contains(&**value))
            .count();
        let peaks = values.iter().filter(|value| **value > 0.84).count();
        assert!(silent > values.len() / 5);
        assert!(midtones > values.len() / 20);
        assert!(peaks > 0);
        assert!(peaks < values.len() / 12);
    }

    #[test]
    fn field_contains_broad_medium_and_micro_spatial_variation() {
        let frame = generated(sample_input(), 0.0, false);
        let field = frame.field();
        let width = field.width();
        let height = field.height();
        let adjacent_variation = field
            .as_slice()
            .chunks(width)
            .flat_map(|row| row.windows(2).map(|pair| (pair[1] - pair[0]).abs()))
            .sum::<f32>();
        let broad_variation = (0..height)
            .flat_map(|y| (0..width.saturating_sub(8)).map(move |x| (x, y)))
            .map(|(x, y)| (field.read(x + 8, y).unwrap() - field.read(x, y).unwrap()).abs())
            .sum::<f32>();

        assert!(adjacent_variation > 0.0);
        assert!(broad_variation > 0.0);
        assert!(adjacent_variation > broad_variation * 0.12);
    }

    #[test]
    fn same_time_and_input_are_deterministic_while_time_remains_continuous() {
        let first = generated(sample_input(), 2.0, false);
        let same = generated(sample_input(), 2.0, false);
        let near = generated(sample_input(), 2.01, false);
        let distant = generated(sample_input(), 18.0, false);

        assert_eq!(first, same);
        assert_ne!(first.field(), distant.field());
        assert!(mean_difference(first.field(), near.field()) < 0.04);
    }

    #[test]
    fn reduced_motion_freezes_the_landscape_without_removing_it() {
        let first = generated(sample_input(), 0.0, true);
        let later = generated(sample_input(), 30.0, true);

        assert_eq!(first.field(), later.field());
        assert!(!first.is_empty());
        assert!(first.reduced_motion());
    }

    #[test]
    fn dominant_region_is_asymmetric_and_displaced_from_center() {
        let frame = generated(sample_input(), 0.0, false);
        let field = frame.field();
        let (maximum_index, _) = field
            .as_slice()
            .iter()
            .enumerate()
            .max_by(|left, right| left.1.total_cmp(right.1))
            .unwrap();
        let x = maximum_index % field.width();
        let y = maximum_index / field.width();
        let normalized_x = x as f32 / field.width().saturating_sub(1).max(1) as f32;
        let normalized_y = y as f32 / field.height().saturating_sub(1).max(1) as f32;
        let mirrored_difference = field
            .as_slice()
            .chunks(field.width())
            .enumerate()
            .map(|(row, values)| {
                values
                    .iter()
                    .enumerate()
                    .map(|(column, value)| {
                        let mirror = values[values.len() - column - 1];
                        (value - mirror).abs() * (row as f32 + 1.0)
                    })
                    .sum::<f32>()
            })
            .sum::<f32>();

        assert!((0.40..0.78).contains(&normalized_x));
        assert!((0.20..0.72).contains(&normalized_y));
        assert!((normalized_x - 0.5).abs() > 0.015 || (normalized_y - 0.5).abs() > 0.015);
        assert!(mirrored_difference > 0.1);
    }

    #[test]
    fn metrics_modulate_activity_compression_direction_and_deformation() {
        let calm = NormalizedPulseInput::try_new(0.12, 0.16, None, 0.02, 0.10, 0.01).unwrap();
        let cpu_busy = NormalizedPulseInput::try_new(0.86, 0.16, None, 0.02, 0.10, 0.01).unwrap();
        let memory_busy =
            NormalizedPulseInput::try_new(0.12, 0.90, None, 0.02, 0.10, 0.01).unwrap();
        let network_busy =
            NormalizedPulseInput::try_new(0.12, 0.16, None, 0.90, 0.10, 0.01).unwrap();
        let volatile = NormalizedPulseInput::try_new(0.12, 0.16, None, 0.02, 0.10, 0.90).unwrap();

        let calm_frame = generated(calm, 2.0, false);
        let cpu_frame = generated(cpu_busy, 2.0, false);
        let memory_frame = generated(memory_busy, 2.0, false);
        let network_frame = generated(network_busy, 2.0, false);
        let volatile_frame = generated(volatile, 2.0, false);

        assert!(mean_energy(cpu_frame.field()) > mean_energy(calm_frame.field()));
        assert!(center_energy(memory_frame.field()) > center_energy(calm_frame.field()));
        assert!(centroid_x(network_frame.field()) > centroid_x(calm_frame.field()));
        assert_ne!(volatile_frame.field(), calm_frame.field());
    }

    #[test]
    fn gradients_are_finite_bounded_and_valid_at_edges() {
        let frame = generated(sample_input(), 1.0, false);
        let field = frame.field();
        let coordinates = [
            (0, 0),
            (field.width().saturating_sub(1), 0),
            (0, field.height().saturating_sub(1)),
            (
                field.width().saturating_sub(1),
                field.height().saturating_sub(1),
            ),
            (field.width() / 2, field.height() / 2),
        ];

        for (x, y) in coordinates {
            let gradient = field.gradient(x, y).unwrap();
            assert_gradient_is_valid(gradient);
        }
        assert!(field.gradient(field.width(), 0).is_none());
        assert!(field.gradient(0, field.height()).is_none());
    }

    #[test]
    fn generated_frames_preserve_presets_and_minimum_dimensions() {
        for (dimensions, expected) in [
            ((1, 1), PulseV2Preset::Compact),
            ((30, 8), PulseV2Preset::Compact),
            ((50, 12), PulseV2Preset::Standard),
            ((80, 20), PulseV2Preset::Widescreen),
            ((140, 40), PulseV2Preset::LargeWidescreen),
        ] {
            let dimensions = FieldDimensions::new(dimensions.0, dimensions.1).unwrap();
            let frame = PulseV2Engine::new()
                .generate(
                    dimensions,
                    sample_input(),
                    Duration::from_millis(400),
                    false,
                )
                .unwrap();

            assert_eq!(frame.preset(), expected);
            assert_eq!(frame.field().as_slice().len(), dimensions.sample_count());
            assert_eq!(frame.noise().as_slice().len(), dimensions.sample_count());
        }
    }

    #[test]
    fn noise_field_is_populated_finite_bounded_and_separate_from_energy() {
        let frame = generated(sample_input(), 2.0, false);
        let noise = frame.noise();

        assert!(!noise.is_empty());
        assert_eq!(noise.dimensions(), frame.dimensions());
        assert!(noise.read(0, 0).is_some());
        assert!(noise.read(noise.width(), 0).is_none());
        assert!(
            noise
                .as_slice()
                .iter()
                .all(|value| value.is_finite() && (0.0..=1.0).contains(value))
        );
        assert_ne!(noise.as_slice(), frame.field().as_slice());
    }

    #[test]
    fn noise_is_deterministic_and_changes_continuously_over_time() {
        let first = generated(sample_input(), 2.0, false);
        let same = generated(sample_input(), 2.0, false);
        let near = generated(sample_input(), 2.01, false);
        let distant = generated(sample_input(), 18.0, false);

        assert_eq!(first.noise(), same.noise());
        let nearby_change = mean_noise_difference(first.noise(), near.noise());
        let distant_change = mean_noise_difference(first.noise(), distant.noise());
        assert!(nearby_change < 0.02);
        assert!(distant_change > nearby_change);
        assert!(distant_change > 0.02);
    }

    #[test]
    fn reduced_motion_freezes_noise_without_removing_the_texture() {
        let first = generated(sample_input(), 0.0, true);
        let later = generated(sample_input(), 30.0, true);

        assert_eq!(first.noise(), later.noise());
        assert!(first.noise().as_slice().iter().any(|value| *value > 0.0));
    }

    #[test]
    fn noise_has_useful_variance_without_grid_like_constant_blocks() {
        let frame = generated(sample_input(), 3.0, false);
        let noise = frame.noise();
        let values = noise.as_slice();
        let mean = values.iter().sum::<f32>() / values.len() as f32;
        let variance = values
            .iter()
            .map(|value| (*value - mean).powi(2))
            .sum::<f32>()
            / values.len() as f32;
        let adjacent = mean_horizontal_noise_difference(noise, 1);
        let medium_scale = mean_horizontal_noise_difference(noise, 4);
        let broad_scale = mean_horizontal_noise_difference(noise, 12);

        assert!((0.25..=0.75).contains(&mean));
        assert!(variance > 0.001);
        assert!(adjacent > 0.0);
        assert!(medium_scale > adjacent);
        assert!(broad_scale > adjacent);
        assert!(
            values
                .windows(2)
                .any(|pair| (pair[1] - pair[0]).abs() > 0.001)
        );
    }

    #[test]
    fn noise_presets_add_complexity_without_changing_the_contract() {
        let dimensions = FieldDimensions::new(140, 40).unwrap();
        let parameters = [
            PulseV2Preset::Compact.parameters(),
            PulseV2Preset::Standard.parameters(),
            PulseV2Preset::Widescreen.parameters(),
            PulseV2Preset::LargeWidescreen.parameters(),
        ];

        assert_eq!(parameters[0].noise_octaves(), 3);
        assert_eq!(parameters[1].noise_octaves(), 3);
        assert_eq!(parameters[2].noise_octaves(), 4);
        assert_eq!(parameters[3].noise_octaves(), 5);
        assert!(
            parameters
                .windows(2)
                .all(|pair| pair[1].noise_base_frequency() > pair[0].noise_base_frequency())
        );
        assert!(
            parameters
                .windows(2)
                .all(|pair| pair[1].noise_micro_amplitude() > pair[0].noise_micro_amplitude())
        );

        let frame = PulseV2Engine::new()
            .generate(dimensions, sample_input(), Duration::from_secs(2), false)
            .unwrap();
        assert_eq!(frame.preset(), PulseV2Preset::LargeWidescreen);
        assert!(!frame.noise().is_empty());
        assert!(
            frame
                .noise()
                .as_slice()
                .iter()
                .all(|value| value.is_finite())
        );
    }

    #[test]
    fn noise_supports_empty_minimum_and_representative_dimensions() {
        let engine = PulseV2Engine::new();
        for dimensions in [
            FieldDimensions::new(0, 0).unwrap(),
            FieldDimensions::new(1, 1).unwrap(),
            FieldDimensions::new(2, 1).unwrap(),
            FieldDimensions::new(160, 80).unwrap(),
        ] {
            let frame = engine
                .generate(dimensions, sample_input(), Duration::from_secs(1), false)
                .unwrap();
            assert_eq!(frame.noise().as_slice().len(), dimensions.sample_count());
            assert!(
                frame
                    .noise()
                    .as_slice()
                    .iter()
                    .all(|value| value.is_finite() && (0.0..=1.0).contains(value))
            );
        }
    }

    fn sample_input() -> NormalizedPulseInput {
        NormalizedPulseInput::try_new(0.42, 0.48, Some(0.30), 0.18, 0.35, 0.08).unwrap()
    }

    fn generated(input: NormalizedPulseInput, seconds: f32, reduced_motion: bool) -> PulseV2Frame {
        PulseV2Engine::new()
            .generate(
                FieldDimensions::new(96, 48).unwrap(),
                input,
                Duration::from_secs_f32(seconds),
                reduced_motion,
            )
            .unwrap()
    }

    fn mean_energy(field: &ScalarField) -> f32 {
        field.as_slice().iter().sum::<f32>() / field.as_slice().len() as f32
    }

    fn mean_difference(left: &ScalarField, right: &ScalarField) -> f32 {
        left.as_slice()
            .iter()
            .zip(right.as_slice())
            .map(|(left, right)| (left - right).abs())
            .sum::<f32>()
            / left.as_slice().len() as f32
    }

    fn mean_noise_difference(left: &NoiseField, right: &NoiseField) -> f32 {
        left.as_slice()
            .iter()
            .zip(right.as_slice())
            .map(|(left, right)| (left - right).abs())
            .sum::<f32>()
            / left.as_slice().len().max(1) as f32
    }

    fn mean_horizontal_noise_difference(noise: &NoiseField, distance: usize) -> f32 {
        let width = noise.width();
        if width <= distance {
            return 0.0;
        }

        let mut total = 0.0;
        let mut count = 0;
        for y in 0..noise.height() {
            for x in 0..(width - distance) {
                total += (noise.read(x + distance, y).unwrap() - noise.read(x, y).unwrap()).abs();
                count += 1;
            }
        }
        total / count.max(1) as f32
    }

    fn center_energy(field: &ScalarField) -> f32 {
        let center_x = field.width() as f32 * 0.585;
        let center_y = field.height() as f32 * 0.455;
        let radius_x = field.width() as f32 * 0.16;
        let radius_y = field.height() as f32 * 0.14;
        let mut total = 0.0;
        let mut count = 0;

        for y in 0..field.height() {
            for x in 0..field.width() {
                if (x as f32 - center_x).abs() <= radius_x
                    && (y as f32 - center_y).abs() <= radius_y
                {
                    total += field.read(x, y).unwrap();
                    count += 1;
                }
            }
        }

        total / count as f32
    }

    fn centroid_x(field: &ScalarField) -> f32 {
        let mut weighted = 0.0;
        let mut total = 0.0;
        for (index, value) in field.as_slice().iter().copied().enumerate() {
            weighted += (index % field.width()) as f32 * value;
            total += value;
        }
        weighted / total.max(f32::EPSILON) / field.width().saturating_sub(1).max(1) as f32
    }

    fn assert_gradient_is_valid(gradient: FieldGradient) {
        assert!(gradient.x().is_finite());
        assert!(gradient.y().is_finite());
        assert!((-1.0..=1.0).contains(&gradient.x()));
        assert!((-1.0..=1.0).contains(&gradient.y()));
    }
}
