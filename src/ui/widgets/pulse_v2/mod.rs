//! Pulse V2's terminal-independent energy, organic-motion and presentation
//! pipeline. Pulse V1 remains separately available to the existing dashboard.

// V2 is intentionally registered before integration. Its public contracts are
// exercised by this module's pure tests and will be consumed by later phases.
#![allow(dead_code, unused_imports)]

mod behavior;
mod color;
mod engine;
mod field;
mod flow;
mod lighting;
mod motion;
mod noise;
mod refinement;
mod renderer;
mod structure;
mod transport;
mod types;

pub use color::{SemanticColorField, SemanticColorSample};
pub use engine::{PulseV2Engine, PulseV2Frame};
pub use field::{FieldError, FieldGradient, ScalarField};
pub use flow::{FlowField, FlowSample};
pub use lighting::{LightingField, LightingSample};
pub use noise::NoiseField;
pub use refinement::{RefinedEnergyField, RefinedEnergySample};
pub use renderer::{
    ColorRasterCell, ColorRasterFrame, PulseV2Rasterizer, RasterDimensions, RasterFrame, RasterMode,
};
pub use structure::StructuredField;
pub use transport::TransportedField;
pub use types::{
    CoordinateError, DimensionsError, FieldDimensions, InputError, NormalizedPoint,
    NormalizedPulseInput, PresetParameters, PulseV2InputTransition, PulseV2Preset,
};

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::pulse::PulseInput;

    use super::{
        FieldDimensions, FieldGradient, FlowField, FlowSample, LightingField, NoiseField,
        NormalizedPoint, NormalizedPulseInput, PulseV2Engine, PulseV2Frame, PulseV2Preset,
        PulseV2Rasterizer, RasterMode, ScalarField, StructuredField, TransportedField,
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
        assert_eq!(first.flow().as_slice().len(), 80 * 20);
        assert!(first.flow().as_slice().iter().all(|sample| {
            sample.direction_magnitude() == 0.0
                && sample.intensity() == 0.0
                && sample.convergence() == 0.0
        }));
        assert_eq!(first.transported().as_slice().len(), 80 * 20);
        assert!(
            first
                .transported()
                .as_slice()
                .iter()
                .all(|value| *value == 0.0)
        );
        assert_eq!(first.structured().as_slice().len(), 80 * 20);
        assert!(
            first
                .structured()
                .as_slice()
                .iter()
                .all(|value| *value == 0.0)
        );
        assert_eq!(first.refined().as_slice().len(), 80 * 20);
        assert!(first.refined().as_slice().iter().all(|sample| {
            sample.energy() == 0.0
                && sample.thickness() == 0.0
                && sample.branching() == 0.0
                && sample.island() == 0.0
                && sample.dissipation() == 0.0
        }));
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
        assert!(frame.flow().is_empty());
        assert!(frame.transported().is_empty());
        assert!(frame.structured().is_empty());
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
    fn input_transitions_ease_without_rebuilding_the_v2_composition() {
        let engine = PulseV2Engine::new();
        let dimensions = FieldDimensions::new(96, 48).unwrap();
        let calm = NormalizedPulseInput::try_new(0.12, 0.16, None, 0.02, 0.10, 0.01).unwrap();
        let critical =
            NormalizedPulseInput::try_new(0.58, 0.62, Some(0.55), 0.52, 0.60, 0.35).unwrap();

        let start = engine
            .generate_with_preset_transition(
                dimensions,
                super::PulseV2InputTransition::new(calm, critical, 0.0),
                PulseV2Preset::Widescreen,
                Duration::from_secs(4),
                false,
            )
            .unwrap();
        let middle = engine
            .generate_with_preset_transition(
                dimensions,
                super::PulseV2InputTransition::new(calm, critical, 0.5),
                PulseV2Preset::Widescreen,
                Duration::from_secs(4),
                false,
            )
            .unwrap();
        let end = engine
            .generate_with_preset_transition(
                dimensions,
                super::PulseV2InputTransition::new(calm, critical, 1.0),
                PulseV2Preset::Widescreen,
                Duration::from_secs(4),
                false,
            )
            .unwrap();

        assert_eq!(start.input(), calm);
        assert_eq!(end.input(), critical);
        assert!(middle.input().cpu() > calm.cpu());
        assert!(middle.input().cpu() < critical.cpu());
        assert!(mean_energy(start.field()) < mean_energy(end.field()));
        assert!(mean_difference(start.field(), middle.field()) > 0.0);
        assert!(mean_difference(middle.field(), end.field()) > 0.0);
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
    fn flow_samples_are_normalized_finite_and_bounded() {
        let frame = generated(sample_input(), 1.0, false);
        let flow = frame.flow();
        let nonzero_directions = flow
            .as_slice()
            .iter()
            .filter(|sample| sample.direction_magnitude() > f32::EPSILON)
            .count();

        assert_eq!(flow.dimensions(), frame.dimensions());
        assert!(nonzero_directions > flow.as_slice().len() / 10);
        for sample in flow.as_slice() {
            let magnitude = sample.direction_magnitude();
            assert!(magnitude.is_finite());
            assert!(magnitude <= 1.0001);
            if magnitude > f32::EPSILON {
                assert!((magnitude - 1.0).abs() < 0.0001);
            }
            assert!(sample.tangent_x().is_finite());
            assert!(sample.tangent_y().is_finite());
            assert!((0.0..=1.0).contains(&sample.intensity()));
            assert!((-1.0..=1.0).contains(&sample.convergence()));
        }
    }

    #[test]
    fn flow_is_derived_from_the_scalar_gradient_without_texture_dependencies() {
        let dimensions = FieldDimensions::new(5, 3).unwrap();
        let mut scalar = ScalarField::try_new(dimensions).unwrap();
        for y in 0..dimensions.height() {
            for x in 0..dimensions.width() {
                scalar
                    .write(x, y, x as f32 / (dimensions.width() - 1) as f32)
                    .unwrap();
            }
        }

        let flow = FlowField::from_scalar(&scalar).unwrap();
        for sample in flow.as_slice() {
            assert!(sample.direction_x() > 0.99);
            assert!(sample.direction_y().abs() < 0.001);
            assert!(sample.tangent_x().abs() < 0.001);
            assert!(sample.tangent_y() > 0.99);
            assert!(sample.convergence().abs() < 0.001);
        }
    }

    #[test]
    fn convergence_and_divergence_are_detected_from_scalar_shape() {
        let dimensions = FieldDimensions::new(7, 7).unwrap();
        let mut peak = ScalarField::try_new(dimensions).unwrap();
        let mut valley = ScalarField::try_new(dimensions).unwrap();
        for y in 0..dimensions.height() {
            for x in 0..dimensions.width() {
                let dx = x as f32 - 3.0;
                let dy = y as f32 - 3.0;
                let distance = (dx * dx + dy * dy) / 18.0;
                peak.write(x, y, (1.0 - distance).max(0.0)).unwrap();
                valley.write(x, y, distance.min(1.0)).unwrap();
            }
        }

        let peak_flow = FlowField::from_scalar(&peak).unwrap();
        let valley_flow = FlowField::from_scalar(&valley).unwrap();
        let peak_convergence = peak_flow
            .as_slice()
            .iter()
            .map(|sample| sample.convergence())
            .fold(f32::NEG_INFINITY, f32::max);
        let valley_divergence = valley_flow
            .as_slice()
            .iter()
            .map(|sample| sample.convergence())
            .fold(f32::INFINITY, f32::min);

        assert!(peak_convergence > 0.01);
        assert!(valley_divergence < -0.01);
    }

    #[test]
    fn flow_is_spatially_and_temporally_continuous() {
        let first = generated(sample_input(), 2.0, false);
        let same = generated(sample_input(), 2.0, false);
        let near = generated(sample_input(), 2.01, false);
        let distant = generated(sample_input(), 18.0, false);

        assert_eq!(first.flow(), same.flow());
        let spatial_change = mean_neighbor_flow_difference(first.flow());
        let nearby_change = mean_flow_difference(first.flow(), near.flow());
        let distant_change = mean_flow_difference(first.flow(), distant.flow());
        assert!(spatial_change > 0.0);
        assert!(spatial_change < 1.5);
        assert!(nearby_change < 0.08);
        assert!(distant_change > nearby_change);
    }

    #[test]
    fn reduced_motion_freezes_flow_and_preserves_its_organization() {
        let first = generated(sample_input(), 0.0, true);
        let later = generated(sample_input(), 30.0, true);

        assert_eq!(first.flow(), later.flow());
        assert!(
            first
                .flow()
                .as_slice()
                .iter()
                .any(|sample| sample.intensity() > 0.0)
        );
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
            assert_eq!(frame.flow().as_slice().len(), dimensions.sample_count());
            assert_eq!(
                frame.transported().as_slice().len(),
                dimensions.sample_count()
            );
            assert_eq!(
                frame.structured().as_slice().len(),
                dimensions.sample_count()
            );
            assert_eq!(frame.refined().as_slice().len(), dimensions.sample_count());
        }
    }

    #[test]
    fn refinement_adds_local_structure_without_mutating_approved_source_fields() {
        let frame = generated(sample_input(), 2.0, false);
        let refined = frame.refined();

        assert_eq!(refined.dimensions(), frame.dimensions());
        assert!(
            refined
                .as_slice()
                .iter()
                .any(|sample| sample.energy() > 0.0)
        );
        assert!(
            refined
                .as_slice()
                .iter()
                .any(|sample| sample.thickness() > 0.0)
        );
        assert!(
            refined
                .as_slice()
                .iter()
                .any(|sample| sample.branching() > 0.0)
        );
        assert!(
            refined
                .as_slice()
                .iter()
                .any(|sample| sample.island() > 0.0)
        );
        assert_eq!(frame.field().dimensions(), frame.dimensions());
        assert_eq!(frame.noise().dimensions(), frame.dimensions());
        assert_eq!(frame.flow().dimensions(), frame.dimensions());
        assert_eq!(frame.transported().dimensions(), frame.dimensions());
        assert_eq!(frame.structured().dimensions(), frame.dimensions());
        assert_eq!(frame.lighting().dimensions(), frame.dimensions());
    }

    #[test]
    fn refinement_is_deterministic_temporally_continuous_and_reduced_motion_static() {
        let first = generated(sample_input(), 2.0, false);
        let same = generated(sample_input(), 2.0, false);
        let near = generated(sample_input(), 2.01, false);
        let distant = generated(sample_input(), 18.0, false);
        let reduced_first = generated(sample_input(), 0.0, true);
        let reduced_later = generated(sample_input(), 30.0, true);

        assert_eq!(first.refined(), same.refined());
        assert!(mean_refined_difference(first.refined(), near.refined()) < 0.08);
        assert!(
            mean_refined_difference(first.refined(), distant.refined())
                > mean_refined_difference(first.refined(), near.refined())
        );
        assert_eq!(reduced_first.refined(), reduced_later.refined());
    }

    #[test]
    fn structured_field_is_bounded_non_uniform_and_preserves_negative_space() {
        let frame = generated(sample_input(), 2.0, false);
        let values = frame.structured().as_slice();
        let mean = values.iter().sum::<f32>() / values.len() as f32;
        let variance = values
            .iter()
            .map(|value| (*value - mean).powi(2))
            .sum::<f32>()
            / values.len() as f32;
        let silence = values.iter().filter(|value| **value < 0.025).count();
        let midtones = values
            .iter()
            .filter(|value| (0.08..0.62).contains(&**value))
            .count();
        let peaks = values.iter().filter(|value| **value > 0.78).count();

        assert!(!frame.structured().is_empty());
        assert!(
            values
                .iter()
                .all(|value| value.is_finite() && (0.0..=1.0).contains(value))
        );
        assert!(variance > 0.001);
        assert!(silence > values.len() / 10);
        assert!(midtones > values.len() / 20);
        assert!(peaks > 0);
        assert!(peaks < values.len() / 8);
    }

    #[test]
    fn structured_field_is_deterministic_temporally_continuous_and_static_in_reduced_motion() {
        let first = generated(sample_input(), 2.0, false);
        let same = generated(sample_input(), 2.0, false);
        let near = generated(sample_input(), 2.01, false);
        let distant = generated(sample_input(), 18.0, false);
        let reduced_first = generated(sample_input(), 0.0, true);
        let reduced_later = generated(sample_input(), 30.0, true);

        assert_eq!(first.structured(), same.structured());
        let nearby_change = mean_structured_difference(first.structured(), near.structured());
        let distant_change = mean_structured_difference(first.structured(), distant.structured());
        assert!(nearby_change < 0.08);
        assert!(distant_change > nearby_change);
        assert_eq!(reduced_first.structured(), reduced_later.structured());
    }

    #[test]
    fn lighting_uses_active_energy_recovers_midtone_depth_and_preserves_silence() {
        let frame = generated(sample_input(), 2.0, false);
        let structured = frame.structured();
        let values = frame.lighting().as_slice();
        let active_count = structured
            .as_slice()
            .iter()
            .filter(|value| **value > 0.02)
            .count();
        let silent_count = structured
            .as_slice()
            .iter()
            .filter(|value| **value <= 0.02)
            .count();
        let visible_active = values
            .iter()
            .zip(structured.as_slice())
            .filter(|(sample, energy)| **energy > 0.02 && sample.intensity() >= 0.10)
            .count();
        let midtones = values
            .iter()
            .filter(|sample| (0.20..0.82).contains(&sample.intensity()))
            .count();
        let peaks = values.iter().filter(|sample| sample.is_peak()).count();
        let depths = values
            .iter()
            .map(|sample| sample.depth())
            .collect::<Vec<_>>();

        assert_eq!(values.len(), structured.as_slice().len());
        assert!(active_count > 0);
        assert!(silent_count > values.len() / 10);
        assert!(visible_active >= active_count / 2);
        assert!(midtones > active_count / 8);
        assert!(peaks > 0);
        assert!(peaks < values.len() / 32);
        assert!(peaks <= active_count / 8 + 1);
        assert!(values.iter().all(|sample| {
            sample.intensity().is_finite()
                && sample.depth().is_finite()
                && sample.relative_energy().is_finite()
                && sample.local_highlight().is_finite()
                && (0.0..=1.0).contains(&sample.intensity())
                && (0.0..=1.0).contains(&sample.depth())
                && (0.0..=1.0).contains(&sample.relative_energy())
                && (0.0..=1.0).contains(&sample.local_highlight())
        }));
        assert!(
            values
                .iter()
                .zip(structured.as_slice())
                .filter(|(_, energy)| **energy <= 0.02)
                .all(|(sample, _)| {
                    sample.intensity() == 0.0
                        && sample.depth() == 0.0
                        && sample.relative_energy() == 0.0
                        && !sample.is_peak()
                })
        );
        assert!(depths.iter().any(|depth| *depth < 0.35));
        assert!(depths.iter().any(|depth| (0.35..=0.68).contains(depth)));
        assert!(depths.iter().any(|depth| *depth > 0.68));
    }

    #[test]
    fn lighting_is_deterministic_temporally_continuous_and_reduced_motion_static() {
        let first = generated(sample_input(), 2.0, false);
        let same = generated(sample_input(), 2.0, false);
        let near = generated(sample_input(), 2.01, false);
        let distant = generated(sample_input(), 18.0, false);
        let reduced_first = generated(sample_input(), 0.0, true);
        let reduced_later = generated(sample_input(), 30.0, true);

        assert_eq!(first.lighting(), same.lighting());
        assert!(mean_lighting_difference(first.lighting(), near.lighting()) < 0.08);
        assert!(
            mean_lighting_difference(first.lighting(), distant.lighting())
                > mean_lighting_difference(first.lighting(), near.lighting())
        );
        assert_eq!(reduced_first.lighting(), reduced_later.lighting());
    }

    #[test]
    fn lighting_hierarchy_responds_to_health_without_global_saturation() {
        let calm = NormalizedPulseInput::try_new(0.05, 0.10, Some(0.05), 0.04, 0.06, 0.02).unwrap();
        let busy = NormalizedPulseInput::try_new(0.86, 0.82, Some(0.78), 0.72, 0.88, 0.62).unwrap();
        let critical = NormalizedPulseInput::try_new(1.0, 1.0, Some(1.0), 0.95, 1.0, 1.0).unwrap();
        let base = generated(sample_input(), 2.0, false);
        let calm_lighting =
            LightingField::from_fields(base.structured(), base.flow(), calm, base.preset(), false)
                .unwrap();
        let busy_lighting =
            LightingField::from_fields(base.structured(), base.flow(), busy, base.preset(), false)
                .unwrap();
        let critical_lighting = LightingField::from_fields(
            base.structured(),
            base.flow(),
            critical,
            base.preset(),
            false,
        )
        .unwrap();
        let calm_mean = mean_lighting(&calm_lighting);
        let busy_mean = mean_lighting(&busy_lighting);

        assert!(calm_mean > 0.0);
        assert!(busy_mean > calm_mean);
        assert!(
            busy_lighting
                .as_slice()
                .iter()
                .any(|sample| sample.intensity() > 0.75)
        );
        assert!(
            critical_lighting
                .as_slice()
                .iter()
                .all(|sample| sample.intensity().is_finite() && sample.intensity() <= 1.0)
        );
        assert!(
            critical_lighting
                .as_slice()
                .iter()
                .any(|sample| sample.intensity() < 0.95)
        );
    }

    #[test]
    fn lighting_keeps_source_fields_unchanged_and_local_contrast_localized() {
        let dimensions = FieldDimensions::new(17, 9).unwrap();
        let mut scalar = ScalarField::try_new(dimensions).unwrap();
        let mut noise = NoiseField::try_new(dimensions).unwrap();
        scalar.fill(0.46).unwrap();
        noise.fill(0.18).unwrap();
        noise.write(8, 4, 1.0).unwrap();
        let flow = FlowField::from_scalar(&scalar).unwrap();
        let transported =
            TransportedField::from_fields(&scalar, &noise, &flow, PulseV2Preset::Standard, false)
                .unwrap();
        let structured =
            StructuredField::from_fields(&scalar, &transported, &flow, PulseV2Preset::Standard)
                .unwrap();
        let scalar_before = scalar.clone();
        let noise_before = noise.clone();
        let flow_before = flow.clone();
        let transported_before = transported.clone();
        let structured_before = structured.clone();
        let lighting = LightingField::from_fields(
            &structured,
            &flow,
            sample_input(),
            PulseV2Preset::Standard,
            false,
        )
        .unwrap();

        assert_eq!(scalar, scalar_before);
        assert_eq!(noise, noise_before);
        assert_eq!(flow, flow_before);
        assert_eq!(transported, transported_before);
        assert_eq!(structured, structured_before);
        assert!(
            lighting.read(8, 4).unwrap().intensity() > lighting.read(0, 0).unwrap().intensity()
        );
        assert!(lighting.read(8, 4).unwrap().local_highlight() > 0.0);
    }

    #[test]
    fn lighting_preserves_responsive_dimensions_and_empty_contract() {
        let engine = PulseV2Engine::new();
        for (width, height) in [(0, 0), (1, 1), (2, 1), (50, 12), (140, 40)] {
            let dimensions = FieldDimensions::new(width, height).unwrap();
            let frame = engine
                .generate(
                    dimensions,
                    sample_input(),
                    Duration::from_millis(400),
                    false,
                )
                .unwrap();
            assert_eq!(frame.lighting().as_slice().len(), dimensions.sample_count());
            assert_eq!(frame.lighting().dimensions(), dimensions);
            assert!(
                frame
                    .lighting()
                    .as_slice()
                    .iter()
                    .all(|sample| sample.intensity().is_finite())
            );
        }

        let empty = engine
            .empty_frame(
                FieldDimensions::new(0, 0).unwrap(),
                NormalizedPulseInput::default(),
            )
            .unwrap();
        assert!(empty.lighting().is_empty());
        assert!(empty.is_empty());
    }

    #[test]
    fn braille_rasterization_is_deterministic_and_preserves_negative_space() {
        let frame = generated(sample_input(), 2.0, false);
        let first = PulseV2Rasterizer::braille(frame.lighting()).unwrap();
        let same = PulseV2Rasterizer::render(frame.lighting(), RasterMode::Braille).unwrap();

        assert_eq!(first, same);
        assert_eq!(first.mode(), RasterMode::Braille);
        assert_eq!((first.width(), first.height()), (48, 12));
        assert_eq!(first.as_slice().len(), 48 * 12);
        assert!(first.as_slice().contains(&' '));
        assert!(first.as_slice().iter().any(|character| *character != ' '));
        assert!(first.to_text().contains('\n'));
    }

    #[test]
    fn braille_peaks_receive_more_occupation_than_quiet_regions() {
        let frame = generated(sample_input(), 2.0, false);
        let raster = PulseV2Rasterizer::braille(frame.lighting()).unwrap();
        let max_index = frame
            .lighting()
            .as_slice()
            .iter()
            .enumerate()
            .max_by(|(_, left), (_, right)| left.intensity().total_cmp(&right.intensity()))
            .map(|(index, _)| index)
            .unwrap();
        let max_x = max_index % frame.lighting().width();
        let max_y = max_index / frame.lighting().width();
        let peak_character = raster.read(max_x / 2, max_y / 4).unwrap();
        let peak_dots = braille_dot_count(peak_character);
        let minimum_active_dots = raster
            .as_slice()
            .iter()
            .map(|character| braille_dot_count(*character))
            .filter(|count| *count > 0)
            .min()
            .unwrap();
        let maximum_dots = raster
            .as_slice()
            .iter()
            .map(|character| braille_dot_count(*character))
            .max()
            .unwrap();

        assert!(peak_dots > 0);
        assert!(maximum_dots > minimum_active_dots);
    }

    #[test]
    fn unicode_fallback_is_simple_bounded_and_deterministic() {
        let frame = generated(sample_input(), 2.0, false);
        let first = PulseV2Rasterizer::unicode(frame.lighting()).unwrap();
        let same = PulseV2Rasterizer::render(frame.lighting(), RasterMode::Unicode).unwrap();
        let allowed = [' ', '·', '•', '▪', '■'];

        assert_eq!(first, same);
        assert_eq!(first.mode(), RasterMode::Unicode);
        assert_eq!((first.width(), first.height()), (96, 48));
        assert!(first.as_slice().contains(&' '));
        assert!(first.as_slice().iter().any(|character| *character != ' '));
        assert!(
            first
                .as_slice()
                .iter()
                .all(|character| allowed.contains(character))
        );
    }

    #[test]
    fn continuous_lighting_produces_connected_terminal_regions() {
        let frame = generated(sample_input(), 2.0, false);
        let raster = PulseV2Rasterizer::braille(frame.lighting()).unwrap();
        let mut adjacent = 0;
        for y in 0..raster.height() {
            for x in 0..raster.width() {
                if raster.read(x, y).is_some_and(|character| character != ' ') {
                    if raster
                        .read(x + 1, y)
                        .is_some_and(|character| character != ' ')
                    {
                        adjacent += 1;
                    }
                    if raster
                        .read(x, y + 1)
                        .is_some_and(|character| character != ' ')
                    {
                        adjacent += 1;
                    }
                }
            }
        }

        assert!(adjacent > raster.as_slice().len() / 12);
    }

    #[test]
    fn rasterization_handles_presets_minimum_dimensions_and_reduced_motion() {
        let engine = PulseV2Engine::new();
        for (width, height) in [
            (0, 0),
            (1, 1),
            (2, 1),
            (30, 8),
            (50, 12),
            (80, 20),
            (140, 40),
        ] {
            let dimensions = FieldDimensions::new(width, height).unwrap();
            let frame = engine
                .generate(
                    dimensions,
                    sample_input(),
                    Duration::from_millis(400),
                    false,
                )
                .unwrap();
            let braille = PulseV2Rasterizer::braille(frame.lighting()).unwrap();
            let unicode = PulseV2Rasterizer::unicode(frame.lighting()).unwrap();

            assert_eq!(unicode.width(), width);
            assert_eq!(unicode.height(), height);
            assert!(braille.as_slice().iter().all(|character| {
                *character == ' ' || (0x2800..=0x28ff).contains(&(*character as u32))
            }));
        }

        let first = engine
            .generate(
                FieldDimensions::new(80, 20).unwrap(),
                sample_input(),
                Duration::ZERO,
                true,
            )
            .unwrap();
        let later = engine
            .generate(
                FieldDimensions::new(80, 20).unwrap(),
                sample_input(),
                Duration::from_secs(30),
                true,
            )
            .unwrap();
        assert_eq!(
            PulseV2Rasterizer::braille(first.lighting()).unwrap(),
            PulseV2Rasterizer::braille(later.lighting()).unwrap()
        );
        assert_eq!(
            PulseV2Rasterizer::unicode(first.lighting()).unwrap(),
            PulseV2Rasterizer::unicode(later.lighting()).unwrap()
        );
    }

    #[test]
    fn local_contrast_reveals_a_crista_without_creating_a_halo() {
        let dimensions = FieldDimensions::new(9, 5).unwrap();
        let mut scalar = ScalarField::try_new(dimensions).unwrap();
        let mut noise = NoiseField::try_new(dimensions).unwrap();
        scalar.fill(0.8).unwrap();
        noise.fill(0.25).unwrap();
        noise.write(4, 2, 1.0).unwrap();
        let flow = FlowField::from_scalar(&scalar).unwrap();
        let transported =
            TransportedField::from_fields(&scalar, &noise, &flow, PulseV2Preset::Standard, false)
                .unwrap();
        let structured =
            StructuredField::from_fields(&scalar, &transported, &flow, PulseV2Preset::Standard)
                .unwrap();

        let center = structured.read(4, 2).unwrap();
        let neighbor = structured.read(3, 2).unwrap();
        let corner = structured.read(0, 0).unwrap();
        assert!(center > neighbor);
        assert!(neighbor >= corner * 0.75);
    }

    #[test]
    fn convergence_reinforces_organized_energy_over_a_non_convergent_flow() {
        let dimensions = FieldDimensions::new(9, 9).unwrap();
        let mut peak = ScalarField::try_new(dimensions).unwrap();
        let mut noise = NoiseField::try_new(dimensions).unwrap();
        let mut linear = ScalarField::try_new(dimensions).unwrap();
        noise.fill(0.55).unwrap();
        for y in 0..dimensions.height() {
            for x in 0..dimensions.width() {
                let dx = x as f32 - 4.0;
                let dy = y as f32 - 4.0;
                peak.write(x, y, (1.0 - (dx * dx + dy * dy) / 24.0).max(0.0))
                    .unwrap();
                linear
                    .write(x, y, x as f32 / (dimensions.width() - 1) as f32)
                    .unwrap();
            }
        }
        let convergent_flow = FlowField::from_scalar(&peak).unwrap();
        let non_convergent_flow = FlowField::from_scalar(&linear).unwrap();
        let transported = TransportedField::from_fields(
            &peak,
            &noise,
            &convergent_flow,
            PulseV2Preset::Standard,
            false,
        )
        .unwrap();
        let organized = StructuredField::from_fields(
            &peak,
            &transported,
            &convergent_flow,
            PulseV2Preset::Standard,
        )
        .unwrap();
        let without_convergence = StructuredField::from_fields(
            &peak,
            &transported,
            &non_convergent_flow,
            PulseV2Preset::Standard,
        )
        .unwrap();

        let organized_peak = organized
            .as_slice()
            .iter()
            .copied()
            .max_by(f32::total_cmp)
            .unwrap();
        let non_convergent_peak = without_convergence
            .as_slice()
            .iter()
            .copied()
            .max_by(f32::total_cmp)
            .unwrap();
        assert!(organized_peak > non_convergent_peak);
    }

    #[test]
    fn dominant_structure_is_displaced_irregular_and_connected_before_and_after() {
        let frame = generated(sample_input(), 2.0, false);
        let field = frame.structured();
        let (maximum_index, _) = field
            .as_slice()
            .iter()
            .enumerate()
            .max_by(|left, right| left.1.total_cmp(right.1))
            .unwrap();
        let maximum_x = maximum_index % field.width();
        let maximum_y = maximum_index / field.width();
        let normalized_x = maximum_x as f32 / field.width().saturating_sub(1).max(1) as f32;
        let normalized_y = maximum_y as f32 / field.height().saturating_sub(1).max(1) as f32;
        let mirror_difference = field
            .as_slice()
            .chunks(field.width())
            .map(|row| {
                row.iter()
                    .enumerate()
                    .map(|(x, value)| (value - row[row.len() - x - 1]).abs())
                    .sum::<f32>()
            })
            .sum::<f32>();
        let before = mean_region(field, 0, field.width() / 3);
        let after = mean_region(field, field.width() * 2 / 3, field.width());
        let connected_pairs = count_adjacent_active(field, 0.08);

        assert!((0.30..0.82).contains(&normalized_x));
        assert!((0.15..0.82).contains(&normalized_y));
        assert!((normalized_x - 0.5).abs() > 0.01 || (normalized_y - 0.5).abs() > 0.01);
        assert!(mirror_difference > 0.05);
        assert!(before > 0.02);
        assert!(after > 0.02);
        assert!(connected_pairs > field.as_slice().len() / 8);
    }

    #[test]
    fn structured_presets_keep_a_bounded_structural_progression() {
        let parameters = [
            PulseV2Preset::Compact.parameters(),
            PulseV2Preset::Standard.parameters(),
            PulseV2Preset::Widescreen.parameters(),
            PulseV2Preset::LargeWidescreen.parameters(),
        ];
        assert!(
            parameters
                .windows(2)
                .all(|pair| pair[1].structure_radius() >= pair[0].structure_radius())
        );
        assert!(parameters.windows(2).all(|pair| {
            pair[1].structure_contrast_strength() > pair[0].structure_contrast_strength()
                && pair[1].structure_convergence_strength()
                    > pair[0].structure_convergence_strength()
        }));
    }

    #[test]
    fn transported_field_is_bounded_and_keeps_source_fields_unchanged() {
        let frame = generated(sample_input(), 2.0, false);
        let scalar_before = frame.field().clone();
        let noise_before = frame.noise().clone();
        let flow_before = frame.flow().clone();
        let transported = frame.transported();

        assert!(!transported.is_empty());
        assert_eq!(transported.dimensions(), frame.dimensions());
        assert!(
            transported
                .as_slice()
                .iter()
                .all(|value| value.is_finite() && (0.0..=1.0).contains(value))
        );
        assert_eq!(frame.field(), &scalar_before);
        assert_eq!(frame.noise(), &noise_before);
        assert_eq!(frame.flow(), &flow_before);
    }

    #[test]
    fn transport_is_deterministic_and_temporally_continuous() {
        let first = generated(sample_input(), 2.0, false);
        let same = generated(sample_input(), 2.0, false);
        let near = generated(sample_input(), 2.01, false);
        let distant = generated(sample_input(), 18.0, false);

        assert_eq!(first.transported(), same.transported());
        let nearby_change = mean_transport_difference(first.transported(), near.transported());
        let distant_change = mean_transport_difference(first.transported(), distant.transported());
        assert!(nearby_change < 0.08);
        assert!(distant_change > nearby_change);
    }

    #[test]
    fn transport_smooths_texture_along_the_local_flow_direction() {
        let dimensions = FieldDimensions::new(21, 5).unwrap();
        let mut scalar = ScalarField::try_new(dimensions).unwrap();
        let mut noise = NoiseField::try_new(dimensions).unwrap();
        for y in 0..dimensions.height() {
            for x in 0..dimensions.width() {
                scalar
                    .write(x, y, x as f32 / (dimensions.width() - 1) as f32)
                    .unwrap();
                noise
                    .write(x, y, if x % 2 == 0 { 0.0 } else { 1.0 })
                    .unwrap();
            }
        }
        let flow = FlowField::from_scalar(&scalar).unwrap();
        let transported =
            TransportedField::from_fields(&scalar, &noise, &flow, PulseV2Preset::Standard, false)
                .unwrap();

        assert!(
            mean_horizontal_value_difference(&transported)
                < mean_horizontal_noise_source_difference(&noise)
        );
        assert!(
            transported
                .read(10, 2)
                .is_some_and(|value| (0.0..=1.0).contains(&value))
        );
        assert!(
            flow.as_slice()
                .iter()
                .all(|sample| sample.direction_x() > 0.99)
        );
    }

    #[test]
    fn flat_flow_preserves_noise_without_inventing_artifacts() {
        let dimensions = FieldDimensions::new(8, 4).unwrap();
        let mut scalar = ScalarField::try_new(dimensions).unwrap();
        let mut noise = NoiseField::try_new(dimensions).unwrap();
        scalar.fill(0.5).unwrap();
        for y in 0..dimensions.height() {
            for x in 0..dimensions.width() {
                noise.write(x, y, ((x + y) % 3) as f32 / 2.0).unwrap();
            }
        }
        let flow = FlowField::from_scalar(&scalar).unwrap();
        let transported =
            TransportedField::from_fields(&scalar, &noise, &flow, PulseV2Preset::Standard, false)
                .unwrap();

        assert!(
            flow.as_slice()
                .iter()
                .all(|sample| sample.direction_magnitude() == 0.0)
        );
        assert_eq!(transported.as_slice(), noise.as_slice());
    }

    #[test]
    fn reduced_motion_freezes_transport_and_presets_bound_its_distance() {
        let first = generated(sample_input(), 0.0, true);
        let later = generated(sample_input(), 30.0, true);
        assert_eq!(first.transported(), later.transported());

        let parameters = [
            PulseV2Preset::Compact.parameters(),
            PulseV2Preset::Standard.parameters(),
            PulseV2Preset::Widescreen.parameters(),
            PulseV2Preset::LargeWidescreen.parameters(),
        ];
        assert!(
            parameters
                .windows(2)
                .all(|pair| pair[1].transport_steps() > pair[0].transport_steps())
        );
        assert!(
            parameters
                .windows(2)
                .all(|pair| pair[1].transport_radius() > pair[0].transport_radius())
        );
        assert!(
            parameters
                .iter()
                .all(|parameters| (0.0..=1.0).contains(&parameters.transport_strength()))
        );
    }

    #[test]
    fn transport_rejects_mismatched_dimensions_without_panicking() {
        let scalar_dimensions = FieldDimensions::new(4, 4).unwrap();
        let noise_dimensions = FieldDimensions::new(5, 4).unwrap();
        let scalar = ScalarField::try_new(scalar_dimensions).unwrap();
        let noise = NoiseField::try_new(noise_dimensions).unwrap();
        let flow = FlowField::from_scalar(&scalar).unwrap();

        assert_eq!(
            TransportedField::from_fields(&scalar, &noise, &flow, PulseV2Preset::Compact, false,),
            Err(super::FieldError::DimensionsMismatch)
        );
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
            FieldDimensions::new(1, 2).unwrap(),
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

    fn mean_transport_difference(left: &TransportedField, right: &TransportedField) -> f32 {
        left.as_slice()
            .iter()
            .zip(right.as_slice())
            .map(|(left, right)| (left - right).abs())
            .sum::<f32>()
            / left.as_slice().len().max(1) as f32
    }

    fn mean_structured_difference(left: &StructuredField, right: &StructuredField) -> f32 {
        left.as_slice()
            .iter()
            .zip(right.as_slice())
            .map(|(left, right)| (left - right).abs())
            .sum::<f32>()
            / left.as_slice().len().max(1) as f32
    }

    fn mean_refined_difference(
        left: &super::RefinedEnergyField,
        right: &super::RefinedEnergyField,
    ) -> f32 {
        left.as_slice()
            .iter()
            .zip(right.as_slice())
            .map(|(left, right)| {
                (left.energy() - right.energy()).abs()
                    + (left.thickness() - right.thickness()).abs()
                    + (left.branching() - right.branching()).abs()
                    + (left.island() - right.island()).abs()
                    + (left.dissipation() - right.dissipation()).abs()
            })
            .sum::<f32>()
            / left.as_slice().len().max(1) as f32
    }

    fn mean_lighting(field: &LightingField) -> f32 {
        field
            .as_slice()
            .iter()
            .map(|sample| sample.intensity())
            .sum::<f32>()
            / field.as_slice().len().max(1) as f32
    }

    fn mean_lighting_difference(left: &LightingField, right: &LightingField) -> f32 {
        left.as_slice()
            .iter()
            .zip(right.as_slice())
            .map(|(left, right)| {
                (left.intensity() - right.intensity()).abs()
                    + (left.depth() - right.depth()).abs()
                    + (left.relative_energy() - right.relative_energy()).abs()
                    + (left.local_highlight() - right.local_highlight()).abs()
            })
            .sum::<f32>()
            / left.as_slice().len().max(1) as f32
    }

    fn braille_dot_count(character: char) -> u32 {
        let code = character as u32;
        if (0x2800..=0x28ff).contains(&code) {
            (code - 0x2800).count_ones()
        } else {
            0
        }
    }

    fn mean_region(field: &StructuredField, start_x: usize, end_x: usize) -> f32 {
        let mut total = 0.0;
        let mut count = 0;
        for y in 0..field.height() {
            for x in start_x..end_x.min(field.width()) {
                total += field.read(x, y).unwrap_or(0.0);
                count += 1;
            }
        }
        total / count.max(1) as f32
    }

    fn count_adjacent_active(field: &StructuredField, threshold: f32) -> usize {
        let mut count = 0;
        for y in 0..field.height() {
            for x in 0..field.width() {
                let current = field.read(x, y).unwrap_or(0.0);
                if current <= threshold {
                    continue;
                }
                if field.read(x + 1, y).is_some_and(|value| value > threshold) {
                    count += 1;
                }
                if field.read(x, y + 1).is_some_and(|value| value > threshold) {
                    count += 1;
                }
            }
        }
        count
    }

    fn mean_horizontal_noise_source_difference(noise: &NoiseField) -> f32 {
        let mut total = 0.0;
        let mut count = 0;
        for y in 0..noise.height() {
            for x in 0..noise.width().saturating_sub(1) {
                total += (noise.read(x + 1, y).unwrap() - noise.read(x, y).unwrap()).abs();
                count += 1;
            }
        }
        total / count.max(1) as f32
    }

    fn mean_horizontal_value_difference(field: &TransportedField) -> f32 {
        let mut total = 0.0;
        let mut count = 0;
        for y in 0..field.height() {
            for x in 0..field.width().saturating_sub(1) {
                total += (field.read(x + 1, y).unwrap() - field.read(x, y).unwrap()).abs();
                count += 1;
            }
        }
        total / count.max(1) as f32
    }

    fn mean_flow_difference(left: &FlowField, right: &FlowField) -> f32 {
        left.as_slice()
            .iter()
            .zip(right.as_slice())
            .map(|(left, right)| {
                (left.direction_x() - right.direction_x()).abs()
                    + (left.direction_y() - right.direction_y()).abs()
                    + (left.intensity() - right.intensity()).abs()
                    + (left.convergence() - right.convergence()).abs()
            })
            .sum::<f32>()
            / left.as_slice().len().max(1) as f32
    }

    fn mean_neighbor_flow_difference(flow: &FlowField) -> f32 {
        let width = flow.width();
        let height = flow.height();
        let mut total = 0.0;
        let mut count = 0;

        for y in 0..height {
            for x in 0..width {
                let current = flow.read(x, y).unwrap();
                if let Some(right) = flow.read(x + 1, y) {
                    total += direction_difference(current, right);
                    count += 1;
                }
                if let Some(down) = flow.read(x, y + 1) {
                    total += direction_difference(current, down);
                    count += 1;
                }
            }
        }

        total / count.max(1) as f32
    }

    fn direction_difference(left: super::FlowSample, right: super::FlowSample) -> f32 {
        (left.direction_x() - right.direction_x()).abs()
            + (left.direction_y() - right.direction_y()).abs()
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
