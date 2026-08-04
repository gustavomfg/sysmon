use std::time::Duration;

use super::easing::ease_out;

#[derive(Debug, Clone, Copy)]
pub struct Tween {
    current: f64,
    start: f64,
    target: f64,
    duration: Duration,
    elapsed: Duration,
    reduced_motion: bool,
}

impl Tween {
    pub fn new(initial: f64, duration: Duration, reduced_motion: bool) -> Self {
        let initial = finite_or_zero(initial);

        Self {
            current: initial,
            start: initial,
            target: initial,
            duration,
            elapsed: Duration::ZERO,
            reduced_motion,
        }
    }

    pub fn current(&self) -> f64 {
        self.current
    }

    pub fn is_active(&self) -> bool {
        !self.reduced_motion && !self.duration.is_zero() && self.current != self.target
    }

    pub fn set_target(&mut self, target: f64) {
        if !target.is_finite() {
            return;
        }

        self.start = self.current;
        self.target = target;
        self.elapsed = Duration::ZERO;

        if self.reduced_motion || self.duration.is_zero() {
            self.current = target;
            self.start = target;
        }
    }

    pub fn set_target_clamped(&mut self, target: f64, minimum: f64, maximum: f64) {
        let (minimum, maximum) = if minimum <= maximum {
            (minimum, maximum)
        } else {
            (maximum, minimum)
        };

        let target = if target.is_finite() {
            target.clamp(minimum, maximum)
        } else {
            self.current.clamp(minimum, maximum)
        };

        self.set_target(target);
    }

    pub fn snap_to(&mut self, value: f64) {
        if !value.is_finite() {
            return;
        }

        self.current = value;
        self.start = value;
        self.target = value;
        self.elapsed = Duration::ZERO;
    }

    pub fn update(&mut self, delta: Duration) -> bool {
        if !self.is_active() {
            return false;
        }

        let previous = self.current;
        self.elapsed = self.elapsed.saturating_add(delta).min(self.duration);

        if self.elapsed >= self.duration {
            self.current = self.target;
        } else {
            let progress = self.elapsed.as_secs_f64() / self.duration.as_secs_f64();
            self.current = self.start + (self.target - self.start) * ease_out(progress);
        }

        previous != self.current
    }
}

fn finite_or_zero(value: f64) -> f64 {
    if value.is_finite() { value } else { 0.0 }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::Tween;

    #[test]
    fn interpolates_at_the_start_middle_and_end() {
        let mut tween = Tween::new(0.0, Duration::from_millis(100), false);
        tween.set_target(100.0);

        assert_eq!(tween.current(), 0.0);
        assert!(tween.update(Duration::from_millis(50)));
        assert_eq!(tween.current(), 75.0);
        assert!(tween.update(Duration::from_millis(50)));
        assert_eq!(tween.current(), 100.0);
        assert!(!tween.is_active());
    }

    #[test]
    fn reduced_motion_finishes_immediately() {
        let mut tween = Tween::new(0.0, Duration::from_millis(100), true);
        tween.set_target(100.0);

        assert_eq!(tween.current(), 100.0);
        assert!(!tween.is_active());
    }

    #[test]
    fn zero_duration_finishes_immediately() {
        let mut tween = Tween::new(0.0, Duration::ZERO, false);
        tween.set_target(100.0);

        assert_eq!(tween.current(), 100.0);
        assert!(!tween.is_active());
    }

    #[test]
    fn clamped_targets_and_invalid_values_finish_predictably() {
        let mut tween = Tween::new(50.0, Duration::ZERO, false);
        tween.set_target_clamped(150.0, 0.0, 100.0);
        assert_eq!(tween.current(), 100.0);

        tween.set_target_clamped(f64::NAN, 0.0, 100.0);
        assert_eq!(tween.current(), 100.0);
    }
}
