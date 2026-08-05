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
mod types;

pub use engine::{PulseV2Engine, PulseV2Frame};
pub use field::{FieldError, ScalarField};
pub use types::{
    CoordinateError, DimensionsError, FieldDimensions, InputError, NormalizedPoint,
    NormalizedPulseInput, PresetParameters, PulseV2Preset,
};

#[cfg(test)]
mod tests {
    use crate::pulse::PulseInput;

    use super::{
        FieldDimensions, NormalizedPoint, NormalizedPulseInput, PulseV2Engine, PulseV2Preset,
        ScalarField,
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
}
