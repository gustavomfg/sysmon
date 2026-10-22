use super::NormalizedPulseInput;

pub(super) fn physiological_activity(input: NormalizedPulseInput) -> f32 {
    (input.cpu() * 0.48
        + input.gpu().unwrap_or(0.0) * 0.28
        + input.network() * 0.10
        + input.volatility() * 0.14)
        .clamp(0.0, 1.0)
}

pub(super) fn physiological_pressure(input: NormalizedPulseInput) -> f32 {
    (input.memory() * 0.45
        + input.load() * 0.35
        + input.cpu() * 0.08
        + input.gpu().unwrap_or(0.0) * 0.05
        + input.network() * 0.02
        + input.volatility() * 0.05)
        .clamp(0.0, 1.0)
}

pub(crate) fn interpolate_input(
    from: NormalizedPulseInput,
    target: NormalizedPulseInput,
    progress: f32,
) -> NormalizedPulseInput {
    from.interpolate_toward(target, progress)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn input_transition_is_bounded_and_smooth() {
        let from = NormalizedPulseInput::try_new(0.1, 0.2, None, 0.0, 0.1, 0.0).unwrap();
        let target = NormalizedPulseInput::try_new(0.9, 0.8, Some(0.7), 0.8, 0.9, 0.6).unwrap();

        let start = interpolate_input(from, target, 0.0);
        let middle = interpolate_input(from, target, 0.5);
        let end = interpolate_input(from, target, 1.0);

        assert_eq!(start, from);
        assert_eq!(end, target);
        assert!(middle.cpu() > from.cpu() && middle.cpu() < target.cpu());
        assert!(middle.gpu().is_some_and(|value| value > 0.0 && value < 0.7));
    }

    #[test]
    fn activity_and_pressure_have_distinct_physiological_roles() {
        let calm = NormalizedPulseInput::try_new(0.12, 0.08, Some(0.10), 0.04, 0.06, 0.02).unwrap();
        let active =
            NormalizedPulseInput::try_new(0.92, 0.10, Some(0.82), 0.18, 0.08, 0.12).unwrap();
        let pressured =
            NormalizedPulseInput::try_new(0.20, 0.92, Some(0.18), 0.04, 0.86, 0.10).unwrap();

        assert!(physiological_activity(active) > physiological_activity(calm));
        assert!(physiological_activity(active) > physiological_activity(pressured));
        assert!(physiological_pressure(active) < physiological_pressure(pressured));
    }

    #[test]
    fn physiological_signals_are_bounded_for_extreme_inputs() {
        let input = NormalizedPulseInput::try_new(1.0, 1.0, Some(1.0), 1.0, 1.0, 1.0).unwrap();
        for value in [physiological_activity(input), physiological_pressure(input)] {
            assert!(value.is_finite());
            assert!((0.0..=1.0).contains(&value));
        }
    }
}
