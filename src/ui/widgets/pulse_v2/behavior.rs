use std::time::Duration;

use super::NormalizedPulseInput;

/// Continuous parameters for the V2 field dynamics.
///
/// This is not a visual overlay and does not scale the final image. It only
/// moves and reshapes the potential sources that feed the scalar field. The
/// subsequent flow, transport, structure and lighting stages therefore remain
/// the source of all visible organization.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct FieldDynamics {
    activity: f32,
    intensity: f32,
    pressure: f32,
    rhythm: f32,
    elapsed_seconds: f32,
    breath: f32,
    pulse: f32,
    reorganization: f32,
    reduced_motion: bool,
}

impl FieldDynamics {
    pub(crate) fn new(
        input: NormalizedPulseInput,
        elapsed: Duration,
        reduced_motion: bool,
    ) -> Self {
        let activity = physiological_activity(input);
        let pressure = physiological_pressure(input);
        let intensity =
            (activity * 0.42 + pressure * 0.52 + input.volatility() * 0.06).clamp(0.0, 1.0);
        // This follows the V1 health-speed relationship, but is used only to
        // evolve the field's sources and transport, never as a frame-rate or
        // brightness multiplier.
        let rhythm = 1.0 + activity * 1.15 + pressure * 1.35 + input.volatility() * 0.40;
        let elapsed_seconds = if reduced_motion {
            0.0
        } else {
            elapsed.as_secs_f32().min(1_000_000.0)
        };
        let phase = elapsed_seconds * rhythm;

        let (breath, pulse, reorganization) = if reduced_motion {
            (0.5, 0.5, 0.0)
        } else {
            (
                phase.sin() * 0.5 + 0.5,
                (phase * 0.83 + 0.37).sin() * 0.5 + 0.5,
                (elapsed_seconds * (0.17 + input.volatility() * 0.12) + 0.91).cos(),
            )
        };

        Self {
            activity,
            intensity,
            pressure,
            rhythm,
            elapsed_seconds,
            breath,
            pulse,
            reorganization,
            reduced_motion,
        }
    }

    pub(crate) fn intensity(self) -> f32 {
        self.intensity
    }

    pub(crate) fn activity(self) -> f32 {
        self.activity
    }

    pub(crate) fn pressure(self) -> f32 {
        self.pressure
    }

    pub(crate) fn rhythm(self) -> f32 {
        self.rhythm
    }

    pub(crate) fn breath(self) -> f32 {
        self.breath
    }

    pub(crate) fn pulse(self) -> f32 {
        self.pulse
    }

    pub(crate) fn reorganization(self) -> f32 {
        self.reorganization
    }

    pub(crate) fn reduced_motion(self) -> bool {
        self.reduced_motion
    }

    /// Slow displacement of the dominant potential, matching the V1 core's
    /// small drift while keeping the V2 composition asymmetric.
    pub(crate) fn dominant_offset(self) -> (f32, f32) {
        if self.reduced_motion {
            return (0.0, 0.0);
        }

        let drift = self.elapsed_seconds * 0.045;
        (
            drift.sin() * 0.055 + self.reorganization * 0.008,
            (drift * 0.73 + 0.8).cos() * 0.065 + self.reorganization * 0.006,
        )
    }

    /// Slow, source-specific drift prevents the upstream and secondary
    /// potentials from becoming a rigid translated copy of the core.
    pub(crate) fn source_offset(self, source: usize) -> (f32, f32) {
        if self.reduced_motion {
            return (0.0, 0.0);
        }

        let speed = match source {
            0 => 0.018,
            1 => 0.024,
            _ => 0.015,
        };
        let phase = match source {
            0 => 0.31,
            1 => 1.73,
            _ => 2.47,
        };
        let drift = self.elapsed_seconds * (speed + self.intensity * 0.008) + phase;
        let amplitude = 0.010 + self.pressure * 0.009;
        (
            drift.sin() * amplitude,
            (drift * (0.71 + source as f32 * 0.08)).cos() * amplitude * 0.72,
        )
    }

    /// Spatial deformation of the potential landscape. It is deliberately
    /// larger than a pixel wobble but remains a bounded displacement of the
    /// existing sources, so it cannot create a new visual object on its own.
    pub(crate) fn landscape_warp(self, x: f32, y: f32) -> (f32, f32) {
        if self.reduced_motion {
            return (0.0, 0.0);
        }

        let phase = self.elapsed_seconds * (0.11 + self.intensity * 0.06);
        let broad = (y * 6.2 + phase * 0.73 + self.reorganization * 0.5).sin();
        let oblique = (x * 4.7 - phase * 0.51 + self.pulse * 0.8).cos();
        let amplitude = 0.009 + self.pressure * 0.008;
        (
            (broad * 0.62 + oblique * 0.38) * amplitude,
            (oblique * 0.58 - broad * 0.42) * amplitude * 0.72,
        )
    }

    /// Breathing changes the shape of the dominant potential, rather than
    /// multiplying the finished scalar field.
    pub(crate) fn dominant_size_scale(self) -> (f32, f32) {
        let delta = self.breath - 0.5;
        (
            (1.0 + delta * (0.10 + self.pressure * 0.05)).clamp(0.94, 1.06),
            (1.0 + delta * (0.14 + self.intensity * 0.06)).clamp(0.92, 1.08),
        )
    }

    pub(crate) fn ridge_phase(self, lane: f32) -> f32 {
        self.elapsed_seconds * (0.10 + self.intensity * 0.16) * (0.82 + self.rhythm * 0.08)
            + lane * 2.7
            + self.reorganization * 0.45
    }
}

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

    fn input() -> NormalizedPulseInput {
        NormalizedPulseInput::try_new(0.42, 0.48, Some(0.30), 0.18, 0.35, 0.08).unwrap()
    }

    #[test]
    fn dynamics_are_deterministic_and_move_potential_sources() {
        let first = FieldDynamics::new(input(), Duration::from_secs_f32(1.0), false);
        let same = FieldDynamics::new(input(), Duration::from_secs_f32(1.0), false);
        let later = FieldDynamics::new(input(), Duration::from_secs_f32(3.0), false);

        assert_eq!(first, same);
        assert_ne!(first.dominant_offset(), later.dominant_offset());
        assert_ne!(first.source_offset(1), later.source_offset(1));
        assert_ne!(first.breath(), later.breath());
        assert!(first.dominant_size_scale().0.is_finite());
    }

    #[test]
    fn reduced_motion_freezes_field_dynamics_without_scaling_output() {
        let first = FieldDynamics::new(input(), Duration::from_secs_f32(1.0), true);
        let later = FieldDynamics::new(input(), Duration::from_secs_f32(90.0), true);

        assert_eq!(first, later);
        assert_eq!(first.dominant_offset(), (0.0, 0.0));
        assert_eq!(first.source_offset(2), (0.0, 0.0));
        assert_eq!(first.landscape_warp(0.4, 0.6), (0.0, 0.0));
        assert_eq!(first.dominant_size_scale(), (1.0, 1.0));
    }

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

        let calm = FieldDynamics::new(calm, Duration::from_secs(2), false);
        let active = FieldDynamics::new(active, Duration::from_secs(2), false);
        let pressured = FieldDynamics::new(pressured, Duration::from_secs(2), false);

        assert!(active.activity() > calm.activity());
        assert!(active.activity() > pressured.activity());
        assert!(active.pressure() < pressured.pressure());
        assert!(active.rhythm() > calm.rhythm());
        assert!(pressured.intensity() > active.intensity());
    }

    #[test]
    fn physiological_signals_are_bounded_for_extreme_inputs() {
        let input = NormalizedPulseInput::try_new(1.0, 1.0, Some(1.0), 1.0, 1.0, 1.0).unwrap();
        let dynamics = FieldDynamics::new(input, Duration::from_secs(1), false);

        for value in [
            dynamics.activity(),
            dynamics.intensity(),
            dynamics.pressure(),
        ] {
            assert!(value.is_finite());
            assert!((0.0..=1.0).contains(&value));
        }
        assert!(dynamics.rhythm().is_finite());
        assert!((1.0..=3.9).contains(&dynamics.rhythm()));
    }
}
