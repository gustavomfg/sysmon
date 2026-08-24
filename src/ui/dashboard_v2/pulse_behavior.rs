//! Translate aggregate system health into continuous Pulse physiology.
//!
//! `SystemHealth` describes the machine.  `PulseBehavior` describes how the
//! existing Pulse should move in response.  Keeping this boundary explicit
//! prevents the Pulse renderer from learning about CPU, memory, GPU or any
//! other individual instrument.

use std::{fmt, time::Duration};

use crate::pulse_v2::NormalizedPulseInput;

use super::health::SystemHealth;

const ATTACK_SECONDS: f32 = 0.35;
const RELEASE_SECONDS: f32 = 1.40;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PulseBehavior {
    rate: f32,
    energy: f32,
    tension: f32,
    turbulence: f32,
    density: f32,
    recovery: f32,
}

impl PulseBehavior {
    /// Creates the perceptual behavior from aggregate health only.
    ///
    /// Activity raises rate and energy, while pressure and thermal pressure
    /// primarily raise tension and reduce recovery.  This deliberately does
    /// not assign one organ to one visual channel: the Pulse remains one
    /// organism whose behavior is driven by the aggregate state.
    pub fn from_system_health(health: SystemHealth) -> Self {
        let activity = health.activity();
        let pressure = health.pressure();
        let thermal = health.thermal_pressure();
        let instability = health.instability();

        let stress = (pressure * 0.68 + thermal * 0.32).clamp(0.0, 1.0);
        let rate = (0.18 + activity * 0.70 + stress * 0.08 + instability * 0.08).clamp(0.0, 1.0);
        let energy = (0.16 + activity * 0.64 + stress * 0.14 + instability * 0.06).clamp(0.0, 1.0);
        let tension = (0.03 + stress * 0.72 + instability * 0.10).clamp(0.0, 1.0);
        let turbulence = (0.02 + instability * 0.70 + stress * 0.12).clamp(0.0, 1.0);
        let density = (0.14 + energy * 0.42 + tension * 0.28 + activity * 0.08).clamp(0.0, 1.0);
        let recovery = (1.0 - stress * 0.76 - instability * 0.04).clamp(0.0, 1.0);

        Self {
            rate,
            energy,
            tension,
            turbulence,
            density,
            recovery,
        }
    }

    /// Adapts behavior to the existing Pulse V2 input contract.
    ///
    /// These channels are perceptual outputs, not component metrics.  The
    /// Pulse engine can therefore remain unchanged while no longer receiving
    /// raw CPU/RAM/GPU values from the dashboard compositor.
    pub fn input(self) -> NormalizedPulseInput {
        // This is a perceptual projection of the behavior, not raw CPU data.
        // Activity travels through CPU/GPU-like channels; constrained state
        // travels through memory/load-like channels for the existing engine.
        let cpu = (0.12 + self.rate * 0.65 + self.energy * 0.18).clamp(0.0, 1.0);
        let memory = self.tension;
        let gpu = Some(
            (0.10 + self.energy * 0.50 + self.rate * 0.22 + self.density * 0.18).clamp(0.0, 1.0),
        );
        let network = (0.04 + self.turbulence * 0.78 + self.rate * 0.06).clamp(0.0, 1.0);
        let load = (self.tension * 0.72 + (1.0 - self.recovery) * 0.28).clamp(0.0, 1.0);

        NormalizedPulseInput::try_new(cpu, memory, gpu, network, load, self.turbulence)
            .unwrap_or_default()
    }

    pub const fn rate(self) -> f32 {
        self.rate
    }

    pub const fn energy(self) -> f32 {
        self.energy
    }

    pub const fn tension(self) -> f32 {
        self.tension
    }

    pub const fn turbulence(self) -> f32 {
        self.turbulence
    }

    pub const fn density(self) -> f32 {
        self.density
    }

    pub const fn recovery(self) -> f32 {
        self.recovery
    }
}

impl Default for PulseBehavior {
    fn default() -> Self {
        Self::from_system_health(SystemHealth::default())
    }
}

impl fmt::Display for PulseBehavior {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "rate {:.2} energy {:.2} tension {:.2} turbulence {:.2} density {:.2} recovery {:.2}",
            self.rate, self.energy, self.tension, self.turbulence, self.density, self.recovery,
        )
    }
}

/// Stateful attack/release smoothing for Pulse physiology.
///
/// Health Engine smoothing keeps aggregate state stable.  This second, small
/// boundary keeps the perceptual behavior continuous when a health snapshot
/// changes faster than the field can naturally respond.  Pressure enters
/// quickly and relaxation releases more slowly.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct PulseBehaviorEngine {
    current: Option<PulseBehavior>,
}

impl PulseBehaviorEngine {
    pub const fn new() -> Self {
        Self { current: None }
    }

    pub const fn current(self) -> Option<PulseBehavior> {
        self.current
    }

    pub fn reset(&mut self) {
        self.current = None;
    }

    pub fn update(&mut self, health: SystemHealth, elapsed: Duration) -> PulseBehavior {
        let target = PulseBehavior::from_system_health(health);
        let Some(previous) = self.current else {
            self.current = Some(target);
            return target;
        };

        let seconds = elapsed.as_secs_f32().clamp(0.0, 1.0);
        let attack = response(seconds, ATTACK_SECONDS);
        let release = response(seconds, RELEASE_SECONDS);
        let next = PulseBehavior {
            rate: approach(previous.rate, target.rate, attack, release),
            energy: approach(previous.energy, target.energy, attack, release),
            tension: approach(previous.tension, target.tension, attack, release),
            turbulence: approach(previous.turbulence, target.turbulence, attack, release),
            density: approach(previous.density, target.density, attack, release),
            // Recovery is an inverse signal: entering pressure should be fast,
            // while returning to rest should take the longer release path.
            recovery: approach_recovery(previous.recovery, target.recovery, attack, release),
        };
        self.current = Some(next);
        next
    }
}

fn approach(previous: f32, target: f32, attack: f32, release: f32) -> f32 {
    let alpha = if target > previous { attack } else { release };
    (previous + (target - previous) * alpha).clamp(0.0, 1.0)
}

fn approach_recovery(previous: f32, target: f32, attack: f32, release: f32) -> f32 {
    let alpha = if target < previous { attack } else { release };
    (previous + (target - previous) * alpha).clamp(0.0, 1.0)
}

fn response(seconds: f32, time_constant: f32) -> f32 {
    if seconds <= 0.0 {
        0.0
    } else {
        (1.0 - (-seconds / time_constant).exp()).clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::dashboard_v2::{HealthComponent, HealthEngine, HealthSignals, HealthSnapshot};

    fn health(component: HealthComponent, signals: HealthSignals) -> SystemHealth {
        HealthEngine::new().evaluate(HealthSnapshot::default().with_component(component, signals))
    }

    #[test]
    fn idle_has_a_living_baseline_without_pressure() {
        let behavior = PulseBehavior::from_system_health(SystemHealth::default());

        assert!(behavior.rate() > 0.0);
        assert!(behavior.energy() > 0.0);
        assert!(behavior.tension() < 0.15);
        assert!(behavior.turbulence() < 0.15);
        assert!(behavior.recovery() > 0.80);
    }

    #[test]
    fn healthy_activity_raises_energy_without_becoming_pressure() {
        let idle = PulseBehavior::from_system_health(SystemHealth::default());
        let active = PulseBehavior::from_system_health(health(
            HealthComponent::Cpu,
            HealthSignals::new(Some(0.95), Some(0.10), None, None),
        ));

        assert!(active.energy() > idle.energy());
        assert!(active.rate() > idle.rate());
        assert!(active.tension() < 0.35);
    }

    #[test]
    fn pressure_and_thermal_load_raise_tension_and_reduce_recovery() {
        let pressure = PulseBehavior::from_system_health(health(
            HealthComponent::Memory,
            HealthSignals::new(None, Some(0.95), None, None),
        ));
        let thermal = PulseBehavior::from_system_health(health(
            HealthComponent::Temperature,
            HealthSignals::new(None, None, Some(0.95), None),
        ));

        assert!(pressure.tension() > PulseBehavior::default().tension());
        assert!(pressure.recovery() < PulseBehavior::default().recovery());
        assert!(thermal.tension() > PulseBehavior::default().tension());
    }

    #[test]
    fn instability_is_reflected_as_bounded_turbulence() {
        let stable = PulseBehavior::from_system_health(health(
            HealthComponent::Cpu,
            HealthSignals::new(Some(0.40), Some(0.20), None, Some(0.0)),
        ));
        let unstable = PulseBehavior::from_system_health(health(
            HealthComponent::Cpu,
            HealthSignals::new(Some(0.40), Some(0.20), None, Some(1.0)),
        ));

        assert!(unstable.turbulence() > stable.turbulence());
        assert!(unstable.turbulence() <= 1.0);
    }

    #[test]
    fn behavior_mapping_is_deterministic_and_bounded() {
        let health = health(
            HealthComponent::Cpu,
            HealthSignals::new(Some(2.0), Some(-1.0), Some(f32::NAN), Some(4.0)),
        );
        let first = PulseBehavior::from_system_health(health);
        let second = PulseBehavior::from_system_health(health);

        assert_eq!(first, second);
        for value in [
            first.rate(),
            first.energy(),
            first.tension(),
            first.turbulence(),
            first.density(),
            first.recovery(),
        ] {
            assert!(value.is_finite());
            assert!((0.0..=1.0).contains(&value));
        }
        assert!(first.input().cpu().is_finite());
    }

    #[test]
    fn behavior_engine_attacks_and_releases_continuously() {
        let mut engine = PulseBehaviorEngine::new();
        let calm = SystemHealth::default();
        let pressure = health(
            HealthComponent::Memory,
            HealthSignals::new(None, Some(1.0), None, None),
        );
        let first = engine.update(calm, Duration::from_secs(1));
        let stressed = engine.update(pressure, Duration::from_millis(100));
        let recovering = engine.update(calm, Duration::from_millis(100));

        assert!(stressed.tension() > first.tension());
        assert!(recovering.tension() < stressed.tension());
        assert!(recovering.tension() > first.tension());
        assert!(recovering.recovery() < PulseBehavior::default().recovery());
    }

    #[test]
    fn activity_does_not_make_behavior_quieter_when_pressure_is_fixed() {
        let low = PulseBehavior::from_system_health(health(
            HealthComponent::Cpu,
            HealthSignals::new(Some(0.20), Some(0.40), None, None),
        ));
        let high = PulseBehavior::from_system_health(health(
            HealthComponent::Cpu,
            HealthSignals::new(Some(0.90), Some(0.40), None, None),
        ));

        assert!(high.energy() >= low.energy());
        assert!(high.rate() >= low.rate());
    }

    #[test]
    fn pressure_is_visible_without_requiring_high_component_activity() {
        let active = health(
            HealthComponent::Cpu,
            HealthSignals::new(Some(0.95), Some(0.10), None, None),
        );
        let memory_pressure = health(
            HealthComponent::Memory,
            HealthSignals::new(None, Some(0.95), None, None),
        );
        let active = PulseBehavior::from_system_health(active);
        let memory_pressure = PulseBehavior::from_system_health(memory_pressure);

        assert!(active.energy() > memory_pressure.energy() * 0.75);
        assert!(memory_pressure.tension() > active.tension());
        assert!(memory_pressure.recovery() < active.recovery());
    }
}
