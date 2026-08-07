//! Interpretation of metric models into a small, temporal system state.
//!
//! This module deliberately sits between metric models and the Pulse.  It does
//! not collect data and it does not render anything.  The diagnostic
//! instruments keep consuming objective metric models; this layer only adds an
//! interpretation for the ambient system summary.

use std::{fmt, time::Duration};

use crate::ui::widgets::pulse_v2::NormalizedPulseInput;

use super::{
    HardwareInventory, LiveMetricModels, MetricValue, PulseBehavior,
    metrics::{CPU_THRESHOLDS, GPU_THRESHOLDS},
};

const COMPONENT_COUNT: usize = 6;
const EPSILON: f32 = 0.01;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum HealthComponent {
    Cpu,
    Memory,
    Gpu,
    Storage,
    Network,
    Temperature,
}

impl HealthComponent {
    pub const ALL: [Self; COMPONENT_COUNT] = [
        Self::Cpu,
        Self::Memory,
        Self::Gpu,
        Self::Storage,
        Self::Network,
        Self::Temperature,
    ];

    const fn index(self) -> usize {
        match self {
            Self::Cpu => 0,
            Self::Memory => 1,
            Self::Gpu => 2,
            Self::Storage => 3,
            Self::Network => 4,
            Self::Temperature => 5,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Cpu => "CPU",
            Self::Memory => "Memory",
            Self::Gpu => "GPU",
            Self::Storage => "Storage",
            Self::Network => "Network",
            Self::Temperature => "Temperature",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum HealthSeverity {
    Healthy,
    Active,
    Elevated,
    Stressed,
    Critical,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HealthSignals {
    activity: Option<f32>,
    pressure: Option<f32>,
    thermal_pressure: Option<f32>,
    trend: Option<f32>,
}

impl HealthSignals {
    pub fn new(
        activity: Option<f32>,
        pressure: Option<f32>,
        thermal_pressure: Option<f32>,
        trend: Option<f32>,
    ) -> Self {
        Self {
            activity: bounded(activity),
            pressure: bounded(pressure),
            thermal_pressure: bounded(thermal_pressure),
            trend: bounded_signed(trend),
        }
    }

    pub const fn activity(self) -> Option<f32> {
        self.activity
    }

    pub const fn pressure(self) -> Option<f32> {
        self.pressure
    }

    pub const fn thermal_pressure(self) -> Option<f32> {
        self.thermal_pressure
    }

    pub const fn trend(self) -> Option<f32> {
        self.trend
    }

    fn is_present(self) -> bool {
        self.activity.is_some()
            || self.pressure.is_some()
            || self.thermal_pressure.is_some()
            || self.trend.is_some()
    }
}

impl Default for HealthSignals {
    fn default() -> Self {
        Self::new(None, None, None, None)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct HealthSnapshot {
    components: [Option<HealthSignals>; COMPONENT_COUNT],
}

impl HealthSnapshot {
    pub fn with_component(mut self, component: HealthComponent, signals: HealthSignals) -> Self {
        self.components[component.index()] = signals.is_present().then_some(signals);
        self
    }

    pub fn set_component(&mut self, component: HealthComponent, signals: HealthSignals) {
        self.components[component.index()] = signals.is_present().then_some(signals);
    }

    pub const fn component(self, component: HealthComponent) -> Option<HealthSignals> {
        self.components[component.index()]
    }

    pub const fn components(self) -> [Option<HealthSignals>; COMPONENT_COUNT] {
        self.components
    }

    /// Adapts the already prepared metric models.  No collector or history is
    /// consulted here, and no metric value is changed.
    pub fn from_models(models: &LiveMetricModels) -> Self {
        Self::from_models_with_policy(models, HealthPolicy::standard())
    }

    pub fn from_models_with_policy(models: &LiveMetricModels, policy: HealthPolicy) -> Self {
        let cpu_model = models.cpu();
        let cpu_activity = fraction(cpu_model.visual().primary());
        let cpu_pressure = cpu_activity.map(|value| {
            let base = pressure_from(
                value,
                policy.cpu_pressure_start(),
                CPU_THRESHOLDS.critical() as f32,
            ) * 0.65;
            if cpu_model.sustained_load() {
                base.max(0.65)
            } else {
                base
            }
        });
        let cpu_thermal = cpu_model.temperature().and_then(|value| {
            thermal_pressure(
                value.value(),
                policy.thermal_warning_celsius(),
                policy.thermal_critical_celsius(),
            )
        });
        let cpu_trend = comparison_trend(cpu_model.comparison(), 0.25);

        let memory_model = models.memory();
        let memory_occupancy = fraction(memory_model.visual().primary());
        let swap_fraction = memory_model
            .swap()
            .zip(memory_model.total())
            .filter(|(_, total)| total.value() > 0.0)
            .map(|(swap, total)| (swap.value() / total.value()).clamp(0.0, 1.0) as f32);
        let memory_pressure = memory_occupancy.map(|value| {
            let occupancy_pressure = pressure_from(value, policy.memory_pressure_start(), 0.95);
            match swap_fraction {
                Some(swap) if swap > 0.0 => occupancy_pressure.max((0.25 + swap).min(1.0)),
                _ => occupancy_pressure,
            }
        });
        let memory_trend = comparison_trend(memory_model.comparison(), 0.25);

        let gpu_model = models.gpu();
        let gpu_activity = fraction(gpu_model.visual().primary());
        let vram_fraction = gpu_model
            .vram_used()
            .zip(gpu_model.vram_total())
            .filter(|(_, total)| total.value() > 0.0)
            .map(|(used, total)| (used.value() / total.value()).clamp(0.0, 1.0));
        let gpu_pressure = gpu_activity
            .map(|value| pressure_from(value, GPU_THRESHOLDS.warning() as f32, 1.0) * 0.35)
            .into_iter()
            .chain(vram_fraction.map(|value| pressure_from(value as f32, 0.80, 0.98) * 0.75))
            .max_by(f32::total_cmp);
        let gpu_thermal = gpu_model.temperature().and_then(|value| {
            thermal_pressure(
                value.value(),
                policy.thermal_warning_celsius(),
                policy.thermal_critical_celsius(),
            )
        });
        let gpu_trend = comparison_trend(gpu_model.comparison(), 0.25);

        let storage_model = models.storage();
        let storage_occupancy = fraction(storage_model.visual().primary());
        let io_activity = storage_model
            .read_rate()
            .zip(storage_model.write_rate())
            .map(|(read, write)| {
                ((read.value() + write.value()) / f64::from(policy.storage_io_reference()))
                    .clamp(0.0, 1.0) as f32
            })
            .or_else(|| {
                storage_model.read_rate().map(|value| {
                    (value.value() / f64::from(policy.storage_io_reference())).clamp(0.0, 1.0)
                        as f32
                })
            })
            .or_else(|| {
                storage_model.write_rate().map(|value| {
                    (value.value() / f64::from(policy.storage_io_reference())).clamp(0.0, 1.0)
                        as f32
                })
            });
        let storage_trend = comparison_trend(storage_model.visual_comparison(), 0.25);

        let network_model = models.network();
        let network_activity = network_model
            .download()
            .zip(network_model.upload())
            .map(|(download, upload)| {
                ((download.value() + upload.value())
                    / f64::from(policy.network_activity_reference()))
                .clamp(0.0, 1.0) as f32
            })
            .or_else(|| {
                network_model.download().map(|value| {
                    (value.value() / f64::from(policy.network_activity_reference())).clamp(0.0, 1.0)
                        as f32
                })
            })
            .or_else(|| {
                network_model.upload().map(|value| {
                    (value.value() / f64::from(policy.network_activity_reference())).clamp(0.0, 1.0)
                        as f32
                })
            });
        let network_trend = comparison_trend(
            network_model.visual_comparison(),
            policy.network_activity_reference(),
        );

        let temperature = thermal_from_inventory(
            models.inventory(),
            policy.thermal_warning_celsius(),
            policy.thermal_critical_celsius(),
        );

        Self::default()
            .with_component(
                HealthComponent::Cpu,
                HealthSignals::new(cpu_activity, cpu_pressure, cpu_thermal, cpu_trend),
            )
            .with_component(
                HealthComponent::Memory,
                HealthSignals::new(None, memory_pressure, None, memory_trend),
            )
            .with_component(
                HealthComponent::Gpu,
                HealthSignals::new(gpu_activity, gpu_pressure, gpu_thermal, gpu_trend),
            )
            .with_component(
                HealthComponent::Storage,
                HealthSignals::new(
                    io_activity,
                    storage_occupancy
                        .map(|value| pressure_from(value, policy.storage_pressure_start(), 0.95)),
                    None,
                    storage_trend,
                ),
            )
            .with_component(
                HealthComponent::Network,
                HealthSignals::new(network_activity, None, None, network_trend),
            )
            .with_component(
                HealthComponent::Temperature,
                HealthSignals::new(None, None, temperature, None),
            )
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ComponentHealth {
    component: HealthComponent,
    activity: Option<f32>,
    pressure: Option<f32>,
    thermal_pressure: Option<f32>,
    trend: Option<f32>,
    severity: HealthSeverity,
}

impl ComponentHealth {
    fn from_signals(component: HealthComponent, signals: HealthSignals) -> Self {
        let activity = signals.activity();
        let pressure = signals.pressure();
        let thermal_pressure = signals.thermal_pressure();
        let severity = classify(
            activity.unwrap_or(0.0),
            pressure.unwrap_or(0.0),
            thermal_pressure.unwrap_or(0.0),
        );
        Self {
            component,
            activity,
            pressure,
            thermal_pressure,
            trend: signals.trend(),
            severity,
        }
    }

    pub const fn component(self) -> HealthComponent {
        self.component
    }

    pub const fn activity(self) -> Option<f32> {
        self.activity
    }

    pub const fn pressure(self) -> Option<f32> {
        self.pressure
    }

    pub const fn thermal_pressure(self) -> Option<f32> {
        self.thermal_pressure
    }

    pub const fn trend(self) -> Option<f32> {
        self.trend
    }

    pub const fn severity(self) -> HealthSeverity {
        self.severity
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SystemHealth {
    activity: f32,
    pressure: f32,
    thermal_pressure: f32,
    instability: f32,
    severity: HealthSeverity,
    dominant_source: Option<HealthComponent>,
    components: [Option<ComponentHealth>; COMPONENT_COUNT],
}

impl Default for SystemHealth {
    fn default() -> Self {
        Self {
            activity: 0.0,
            pressure: 0.0,
            thermal_pressure: 0.0,
            instability: 0.0,
            severity: HealthSeverity::Healthy,
            dominant_source: None,
            components: [None; COMPONENT_COUNT],
        }
    }
}

impl SystemHealth {
    pub const fn activity(self) -> f32 {
        self.activity
    }

    pub const fn pressure(self) -> f32 {
        self.pressure
    }

    pub const fn thermal_pressure(self) -> f32 {
        self.thermal_pressure
    }

    pub const fn instability(self) -> f32 {
        self.instability
    }

    pub const fn severity(self) -> HealthSeverity {
        self.severity
    }

    pub const fn dominant_source(self) -> Option<HealthComponent> {
        self.dominant_source
    }

    pub const fn component(self, component: HealthComponent) -> Option<ComponentHealth> {
        self.components[component.index()]
    }

    pub const fn components(self) -> [Option<ComponentHealth>; COMPONENT_COUNT] {
        self.components
    }

    /// Compatibility shim for callers that still consume the old boundary.
    /// New integration code should retain `PulseBehavior` explicitly.
    pub fn pulse_input(self) -> NormalizedPulseInput {
        PulseBehavior::from_system_health(self).input()
    }
}

impl fmt::Display for SystemHealth {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "activity {:.2} pressure {:.2} thermal {:.2} instability {:.2} severity {:?} dominant {}",
            self.activity,
            self.pressure,
            self.thermal_pressure,
            self.instability,
            self.severity,
            self.dominant_source.map_or("—", HealthComponent::label),
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HealthPolicy {
    cpu_pressure_start: f32,
    memory_pressure_start: f32,
    storage_pressure_start: f32,
    thermal_warning_celsius: f32,
    thermal_critical_celsius: f32,
    network_activity_reference: f32,
    storage_io_reference: f32,
}

impl HealthPolicy {
    pub const fn standard() -> Self {
        Self {
            cpu_pressure_start: 0.75,
            memory_pressure_start: 0.70,
            storage_pressure_start: 0.80,
            thermal_warning_celsius: 75.0,
            thermal_critical_celsius: 90.0,
            network_activity_reference: 100.0 * 1024.0 * 1024.0,
            storage_io_reference: 100.0 * 1024.0 * 1024.0,
        }
    }

    pub const fn cpu_pressure_start(self) -> f32 {
        self.cpu_pressure_start
    }

    pub const fn memory_pressure_start(self) -> f32 {
        self.memory_pressure_start
    }

    pub const fn storage_pressure_start(self) -> f32 {
        self.storage_pressure_start
    }

    pub const fn thermal_warning_celsius(self) -> f32 {
        self.thermal_warning_celsius
    }

    pub const fn thermal_critical_celsius(self) -> f32 {
        self.thermal_critical_celsius
    }

    pub const fn network_activity_reference(self) -> f32 {
        self.network_activity_reference
    }

    pub const fn storage_io_reference(self) -> f32 {
        self.storage_io_reference
    }
}

impl Default for HealthPolicy {
    fn default() -> Self {
        Self::standard()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HealthEngine {
    policy: HealthPolicy,
    current: Option<SystemHealth>,
}

impl Default for HealthEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl HealthEngine {
    pub const fn new() -> Self {
        Self {
            policy: HealthPolicy::standard(),
            current: None,
        }
    }

    pub const fn with_policy(policy: HealthPolicy) -> Self {
        Self {
            policy,
            current: None,
        }
    }

    pub const fn policy(self) -> HealthPolicy {
        self.policy
    }

    pub const fn current(self) -> Option<SystemHealth> {
        self.current
    }

    pub fn reset(&mut self) {
        self.current = None;
    }

    pub fn evaluate(&self, snapshot: HealthSnapshot) -> SystemHealth {
        let components = snapshot
            .components()
            .into_iter()
            .enumerate()
            .map(|(index, signals)| {
                signals.map(|signals| {
                    ComponentHealth::from_signals(HealthComponent::ALL[index], signals)
                })
            })
            .collect_components();
        aggregate(components)
    }

    pub fn update(&mut self, snapshot: HealthSnapshot, elapsed: Duration) -> SystemHealth {
        let target = self.evaluate(snapshot);
        let Some(previous) = self.current else {
            self.current = Some(target);
            return target;
        };
        let seconds = elapsed.as_secs_f32().clamp(0.0, 1.0);
        let attack = response(seconds, 0.35);
        let release = response(seconds, 1.40);
        let components = previous
            .components()
            .into_iter()
            .zip(target.components())
            .enumerate()
            .map(|(index, (previous, target))| {
                smooth_component(
                    HealthComponent::ALL[index],
                    previous,
                    target,
                    attack,
                    release,
                )
            })
            .collect_components();
        let next = aggregate(components);
        self.current = Some(next);
        next
    }
}

trait ComponentArrayExt {
    fn collect_components(self) -> [Option<ComponentHealth>; COMPONENT_COUNT];
}

impl<I> ComponentArrayExt for I
where
    I: Iterator<Item = Option<ComponentHealth>>,
{
    fn collect_components(self) -> [Option<ComponentHealth>; COMPONENT_COUNT] {
        let mut components = [None; COMPONENT_COUNT];
        for (index, component) in self.take(COMPONENT_COUNT).enumerate() {
            components[index] = component;
        }
        components
    }
}

fn aggregate(components: [Option<ComponentHealth>; COMPONENT_COUNT]) -> SystemHealth {
    let mut activity_values = [0.0; COMPONENT_COUNT];
    let mut pressure_values = [0.0; COMPONENT_COUNT];
    let mut thermal_values = [0.0; COMPONENT_COUNT];
    let mut instability: f32 = 0.0;
    let mut dominant = None;
    let mut dominant_score = 0.0;

    for component in components.iter().flatten().copied() {
        let index = component.component().index();
        let (activity_weight, pressure_weight, thermal_weight) = weights(component.component());
        activity_values[index] = component.activity().unwrap_or(0.0) * activity_weight;
        pressure_values[index] = component.pressure().unwrap_or(0.0) * pressure_weight;
        thermal_values[index] = component.thermal_pressure().unwrap_or(0.0) * thermal_weight;
        instability = instability.max(component.trend().unwrap_or(0.0).abs());

        let score = pressure_values[index]
            .max(thermal_values[index])
            .max(component.activity().unwrap_or(0.0) * activity_weight * 0.35);
        if score > dominant_score {
            dominant_score = score;
            dominant = Some(component.component());
        }
    }

    let activity = saturating_combine(activity_values);
    let pressure = top_two_combine(pressure_values, 0.35);
    let thermal_pressure = top_two_combine(thermal_values, 0.25);
    let severity = classify(activity, pressure, thermal_pressure);
    SystemHealth {
        activity,
        pressure,
        thermal_pressure,
        instability: instability.clamp(0.0, 1.0),
        severity,
        dominant_source: (dominant_score > EPSILON).then_some(dominant).flatten(),
        components,
    }
}

fn smooth_component(
    component: HealthComponent,
    previous: Option<ComponentHealth>,
    target: Option<ComponentHealth>,
    attack: f32,
    release: f32,
) -> Option<ComponentHealth> {
    let signals = [
        smooth_optional(
            previous.and_then(ComponentHealth::activity),
            target.and_then(ComponentHealth::activity),
            attack,
            release,
        ),
        smooth_optional(
            previous.and_then(ComponentHealth::pressure),
            target.and_then(ComponentHealth::pressure),
            attack,
            release,
        ),
        smooth_optional(
            previous.and_then(ComponentHealth::thermal_pressure),
            target.and_then(ComponentHealth::thermal_pressure),
            attack,
            release,
        ),
        smooth_optional(
            previous.and_then(ComponentHealth::trend),
            target.and_then(ComponentHealth::trend),
            attack,
            release,
        ),
    ];
    let signals = HealthSignals::new(signals[0], signals[1], signals[2], signals[3]);
    signals
        .is_present()
        .then(|| ComponentHealth::from_signals(component, signals))
}

fn smooth_optional(
    previous: Option<f32>,
    target: Option<f32>,
    attack: f32,
    release: f32,
) -> Option<f32> {
    match (previous, target) {
        (Some(previous), Some(target)) => Some(approach(previous, target, attack, release)),
        (Some(previous), None) => {
            let value = approach(previous, 0.0, release, release);
            (value >= EPSILON).then_some(value)
        }
        (None, Some(target)) => Some(approach(0.0, target, attack, release)),
        (None, None) => None,
    }
}

fn approach(previous: f32, target: f32, attack: f32, release: f32) -> f32 {
    let alpha = if target.abs() > previous.abs() {
        attack
    } else {
        release
    };
    (previous + (target - previous) * alpha).clamp(-1.0, 1.0)
}

fn response(seconds: f32, time_constant: f32) -> f32 {
    if seconds <= 0.0 {
        0.0
    } else {
        1.0 - (-seconds / time_constant).exp()
    }
}

fn classify(activity: f32, pressure: f32, thermal: f32) -> HealthSeverity {
    let stress = pressure.max(thermal);
    if stress >= 0.85 {
        HealthSeverity::Critical
    } else if stress >= 0.65 {
        HealthSeverity::Stressed
    } else if stress >= 0.40 {
        HealthSeverity::Elevated
    } else if activity >= 0.55 {
        HealthSeverity::Active
    } else {
        HealthSeverity::Healthy
    }
}

fn weights(component: HealthComponent) -> (f32, f32, f32) {
    match component {
        HealthComponent::Cpu => (1.0, 0.90, 1.0),
        HealthComponent::Memory => (0.35, 1.0, 1.0),
        HealthComponent::Gpu => (0.80, 0.65, 1.0),
        HealthComponent::Storage => (0.45, 0.85, 0.90),
        HealthComponent::Network => (0.25, 0.10, 0.50),
        HealthComponent::Temperature => (0.10, 0.25, 1.10),
    }
}

fn saturating_combine(values: [f32; COMPONENT_COUNT]) -> f32 {
    values.into_iter().fold(0.0, |combined, value| {
        1.0 - (1.0 - combined) * (1.0 - value.clamp(0.0, 1.0))
    })
}

fn top_two_combine(values: [f32; COMPONENT_COUNT], secondary_weight: f32) -> f32 {
    let mut highest = 0.0;
    let mut second = 0.0;
    for value in values {
        let value = value.clamp(0.0, 1.0);
        if value >= highest {
            second = highest;
            highest = value;
        } else if value > second {
            second = value;
        }
    }
    (highest + (1.0 - highest) * second * secondary_weight).clamp(0.0, 1.0)
}

fn pressure_from(value: f32, start: f32, critical: f32) -> f32 {
    if value <= start {
        0.0
    } else {
        ((value - start) / (critical - start).max(EPSILON)).clamp(0.0, 1.0)
    }
}

fn thermal_pressure(value: f64, warning: f32, critical: f32) -> Option<f32> {
    value
        .is_finite()
        .then_some((((value as f32) - warning) / (critical - warning).max(EPSILON)).clamp(0.0, 1.0))
}

fn thermal_from_inventory(
    inventory: &HardwareInventory,
    warning: f32,
    critical: f32,
) -> Option<f32> {
    inventory
        .temperatures
        .iter()
        .filter_map(|sensor| sensor.celsius)
        .filter_map(|value| thermal_pressure(value, warning, critical))
        .max_by(f32::total_cmp)
}

fn fraction(value: Option<MetricValue>) -> Option<f32> {
    value
        .filter(|value| value.is_usable())
        .map(|value| (value.value() as f32).clamp(0.0, 1.0))
}

fn comparison_trend(comparison: super::PeriodComparison, scale: f32) -> Option<f32> {
    comparison
        .delta()
        .filter(|value| value.is_finite())
        .map(|value| ((value as f32) / scale.max(EPSILON)).clamp(-1.0, 1.0))
}

fn bounded(value: Option<f32>) -> Option<f32> {
    value
        .filter(|value| value.is_finite())
        .map(|value| value.clamp(0.0, 1.0))
}

fn bounded_signed(value: Option<f32>) -> Option<f32> {
    value
        .filter(|value| value.is_finite())
        .map(|value| value.clamp(-1.0, 1.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn component(
        component: HealthComponent,
        activity: Option<f32>,
        pressure: Option<f32>,
        thermal: Option<f32>,
        trend: Option<f32>,
    ) -> HealthSnapshot {
        HealthSnapshot::default().with_component(
            component,
            HealthSignals::new(activity, pressure, thermal, trend),
        )
    }

    #[test]
    fn idle_is_healthy_without_a_dominant_source() {
        let health = HealthEngine::new().evaluate(HealthSnapshot::default());
        assert_eq!(health.severity(), HealthSeverity::Healthy);
        assert_eq!(health.dominant_source(), None);
        assert_eq!(health.activity(), 0.0);
    }

    #[test]
    fn activity_is_not_automatically_pressure() {
        let health = HealthEngine::new().evaluate(component(
            HealthComponent::Gpu,
            Some(1.0),
            Some(0.15),
            Some(0.10),
            None,
        ));
        assert_eq!(health.severity(), HealthSeverity::Active);
        assert!(health.activity() > 0.0);
        assert!(health.pressure() < 0.40);
        assert_eq!(health.dominant_source(), Some(HealthComponent::Gpu));
    }

    #[test]
    fn sustained_cpu_pressure_is_distinguished_from_cpu_activity() {
        let health = HealthEngine::new().evaluate(component(
            HealthComponent::Cpu,
            Some(0.95),
            Some(0.78),
            Some(0.15),
            Some(0.30),
        ));
        assert_eq!(health.severity(), HealthSeverity::Stressed);
        assert_eq!(health.dominant_source(), Some(HealthComponent::Cpu));
        assert!(health.instability() > 0.0);
    }

    #[test]
    fn memory_pressure_and_thermal_pressure_have_different_sources() {
        let memory = HealthEngine::new().evaluate(component(
            HealthComponent::Memory,
            None,
            Some(0.96),
            None,
            None,
        ));
        assert_eq!(memory.severity(), HealthSeverity::Critical);
        assert_eq!(memory.dominant_source(), Some(HealthComponent::Memory));

        let thermal = HealthEngine::new().evaluate(component(
            HealthComponent::Temperature,
            None,
            None,
            Some(0.92),
            None,
        ));
        assert_eq!(thermal.severity(), HealthSeverity::Critical);
        assert_eq!(
            thermal.dominant_source(),
            Some(HealthComponent::Temperature)
        );
    }

    #[test]
    fn storage_capacity_and_network_activity_are_interpreted_separately() {
        let snapshot = component(HealthComponent::Storage, Some(0.10), Some(0.96), None, None)
            .with_component(
                HealthComponent::Network,
                HealthSignals::new(Some(0.90), None, None, Some(0.25)),
            );
        let health = HealthEngine::new().evaluate(snapshot);
        assert!(health.activity() > 0.0);
        assert!(health.pressure() > 0.80);
        assert_eq!(health.dominant_source(), Some(HealthComponent::Storage));
    }

    #[test]
    fn multiple_pressure_sources_compose_without_a_naive_sum() {
        let snapshot = component(HealthComponent::Cpu, Some(0.95), Some(0.80), None, None)
            .with_component(
                HealthComponent::Memory,
                HealthSignals::new(None, Some(0.80), None, None),
            );
        let health = HealthEngine::new().evaluate(snapshot);
        assert!(health.pressure() > 0.80);
        assert!(health.pressure() <= 1.0);
        assert_eq!(health.severity(), HealthSeverity::Critical);
    }

    #[test]
    fn temporal_update_attacks_quickly_and_releases_slowly() {
        let mut engine = HealthEngine::new();
        let calm = HealthSnapshot::default();
        let pressure = component(HealthComponent::Memory, None, Some(1.0), None, None);
        let first = engine.update(calm, Duration::from_secs(1));
        let stressed = engine.update(pressure, Duration::from_millis(100));
        assert!(stressed.pressure() > first.pressure());
        let recovering = engine.update(calm, Duration::from_millis(100));
        assert!(recovering.pressure() < stressed.pressure());
        assert!(recovering.pressure() > 0.0);
        for _ in 0..80 {
            engine.update(calm, Duration::from_millis(100));
        }
        assert_eq!(
            engine.current().unwrap().severity(),
            HealthSeverity::Healthy
        );
    }

    #[test]
    fn invalid_signals_are_bounded_and_deterministic() {
        let snapshot = component(
            HealthComponent::Cpu,
            Some(4.0),
            Some(-2.0),
            Some(f32::NAN),
            Some(3.0),
        );
        let first = HealthEngine::new().evaluate(snapshot);
        let second = HealthEngine::new().evaluate(snapshot);
        assert_eq!(first, second);
        assert!((0.0..=1.0).contains(&first.activity()));
        assert!((0.0..=1.0).contains(&first.pressure()));
        assert!((0.0..=1.0).contains(&first.thermal_pressure()));
        assert!((0.0..=1.0).contains(&first.instability()));
    }

    #[test]
    fn health_aggregate_keeps_pressure_at_the_boundary() {
        let health = HealthEngine::new().evaluate(component(
            HealthComponent::Memory,
            None,
            Some(0.90),
            None,
            None,
        ));
        assert_eq!(health.activity(), 0.0);
        assert!(health.pressure() > 0.0);
        assert!(health.severity() >= HealthSeverity::Elevated);
    }
}
