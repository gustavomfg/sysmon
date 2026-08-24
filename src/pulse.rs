use std::time::Duration;

use crate::{animation::Tween, system::info::SystemInfo};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PulseInput {
    pub cpu: f32,
    pub memory: f32,
    pub gpu: Option<f32>,
    pub network: f32,
    pub load: f32,
    pub volatility: f32,
}

impl PulseInput {
    pub fn from_snapshots(current: &SystemInfo, previous: Option<&SystemInfo>) -> Self {
        let cpu = normalize_percent(current.cpu_usage);
        let memory = normalize_ratio(current.used_ram, current.total_ram);
        let gpu = gpu_usage(current);
        let network = network_activity(current);
        let load = normalize_load(current.load_average.0, current.cpu_cores);
        let volatility = previous.map_or(0.0, |previous| {
            let cpu_delta = (cpu - normalize_percent(previous.cpu_usage)).abs();
            let memory_delta =
                (memory - normalize_ratio(previous.used_ram, previous.total_ram)).abs();
            let network_delta = (network - network_activity(previous)).abs();
            cpu_delta.max(memory_delta).max(network_delta)
        });

        Self {
            cpu,
            memory,
            gpu,
            network,
            load,
            volatility,
        }
    }

    pub fn pressure(self) -> f32 {
        let base = self.cpu * CPU_WEIGHT
            + self.memory * MEMORY_WEIGHT
            + self.network * NETWORK_WEIGHT
            + self.load * LOAD_WEIGHT;

        match self.gpu {
            Some(gpu) => base + gpu * GPU_WEIGHT,
            None => {
                let redistributed = self.cpu * 0.04 + self.memory * 0.08 + self.load * 0.03;
                base + redistributed
            }
        }
    }

    pub fn intensity(self) -> f32 {
        (self.pressure() + self.volatility * VOLATILITY_WEIGHT).clamp(0.0, 1.0)
    }
}

const CPU_WEIGHT: f32 = 0.27;
const MEMORY_WEIGHT: f32 = 0.30;
const NETWORK_WEIGHT: f32 = 0.10;
const LOAD_WEIGHT: f32 = 0.18;
const GPU_WEIGHT: f32 = 0.15;
const VOLATILITY_WEIGHT: f32 = 0.10;

impl Default for PulseInput {
    fn default() -> Self {
        Self {
            cpu: 0.0,
            memory: 0.0,
            gpu: None,
            network: 0.0,
            load: 0.0,
            volatility: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthState {
    Calm,
    Active,
    Busy,
    Critical,
}

impl HealthState {
    pub fn label(self) -> &'static str {
        match self {
            Self::Calm => "Calm",
            Self::Active => "Active",
            Self::Busy => "Busy",
            Self::Critical => "Critical",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PulseHealth {
    state: HealthState,
    intensity: f32,
    pressure: f32,
    volatility: f32,
}

impl PulseHealth {
    fn new(intensity: f32, pressure: f32, volatility: f32) -> Self {
        let state = match intensity {
            value if value < 0.30 => HealthState::Calm,
            value if value < 0.55 => HealthState::Active,
            value if value < 0.78 => HealthState::Busy,
            _ => HealthState::Critical,
        };

        Self {
            state,
            intensity,
            pressure,
            volatility,
        }
    }

    pub fn state(self) -> HealthState {
        self.state
    }

    pub fn intensity(self) -> f32 {
        self.intensity
    }

    pub fn pressure(self) -> f32 {
        self.pressure
    }

    pub fn volatility(self) -> f32 {
        self.volatility
    }

    pub fn speed(self) -> f32 {
        1.0 + self.intensity * 1.8 + self.volatility * 0.45
    }

    pub fn amplitude(self) -> f32 {
        0.75 + self.pressure * 0.30 + self.volatility * 0.20
    }

    pub fn density(self) -> f32 {
        (0.30 + self.intensity * 0.70 + self.volatility * 0.20).clamp(0.0, 1.0)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PulseModel {
    input: PulseInput,
    intensity: Tween,
    volatility: Tween,
    elapsed: Duration,
    ambient_elapsed: Duration,
    render_elapsed: Duration,
    render_revision: u64,
    reduced_motion: bool,
    has_input: bool,
}

const PULSE_DURATION: Duration = Duration::from_millis(300);
const AMBIENT_UPDATE_INTERVAL: Duration = Duration::from_millis(80);

impl PulseModel {
    pub fn new(reduced_motion: bool) -> Self {
        Self {
            input: PulseInput::default(),
            intensity: Tween::new(0.0, PULSE_DURATION, reduced_motion),
            volatility: Tween::new(0.0, PULSE_DURATION, reduced_motion),
            elapsed: Duration::ZERO,
            ambient_elapsed: Duration::ZERO,
            render_elapsed: Duration::ZERO,
            render_revision: 0,
            reduced_motion,
            has_input: false,
        }
    }

    pub fn input(&self) -> PulseInput {
        self.input
    }

    pub fn set_input(&mut self, input: PulseInput) {
        let input_changed = !self.has_input || self.input != input;
        let target_intensity = f64::from(input.intensity());
        let target_volatility = f64::from(input.volatility.clamp(0.0, 1.0));
        self.input = input;

        if self.has_input {
            self.intensity
                .set_target_clamped(target_intensity, 0.0, 1.0);
            self.volatility
                .set_target_clamped(target_volatility, 0.0, 1.0);
        } else {
            self.intensity.snap_to(target_intensity);
            self.volatility.snap_to(target_volatility);
            self.has_input = true;
        }

        if input_changed {
            self.mark_render_revision();
        }
    }

    pub fn set_reduced_motion(&mut self, reduced_motion: bool) {
        let changed = self.reduced_motion != reduced_motion;
        self.reduced_motion = reduced_motion;
        self.intensity.set_reduced_motion(reduced_motion);
        self.volatility.set_reduced_motion(reduced_motion);
        self.ambient_elapsed = Duration::ZERO;
        if changed {
            self.mark_render_revision();
        }
    }

    pub fn update(&mut self, delta: Duration) -> bool {
        if !self.has_input {
            return false;
        }

        self.elapsed = self.elapsed.saturating_add(delta);
        let intensity_changed = self.intensity.update(delta);
        let volatility_changed = self.volatility.update(delta);
        let ambient_changed = if self.reduced_motion || delta.is_zero() {
            self.ambient_elapsed = Duration::ZERO;
            false
        } else {
            self.ambient_elapsed = self.ambient_elapsed.saturating_add(delta);
            if self.ambient_elapsed >= AMBIENT_UPDATE_INTERVAL {
                self.ambient_elapsed = Duration::ZERO;
                true
            } else {
                false
            }
        };
        let changed = intensity_changed || volatility_changed || ambient_changed;

        if changed {
            self.render_elapsed = self.elapsed;
            self.mark_render_revision();
        }
        changed
    }

    pub fn is_active(&self) -> bool {
        self.has_input && (self.intensity.is_active() || self.volatility.is_active())
    }

    pub fn intensity(&self) -> f32 {
        self.intensity.current() as f32
    }

    pub fn volatility(&self) -> f32 {
        self.volatility.current() as f32
    }

    pub fn health(&self) -> PulseHealth {
        PulseHealth::new(self.intensity(), self.input.pressure(), self.volatility())
    }

    pub fn breath(&self) -> f32 {
        if self.reduced_motion || !self.has_input {
            return 0.5;
        }

        let speed = self.health().speed();
        (self.elapsed.as_secs_f32() * speed).sin() * 0.5 + 0.5
    }

    pub fn phase(&self) -> f32 {
        if self.reduced_motion || !self.has_input {
            0.0
        } else {
            (self.elapsed.as_secs_f32() * self.health().speed()).rem_euclid(std::f32::consts::TAU)
        }
    }

    #[allow(dead_code)]
    pub fn time_seconds(&self) -> f32 {
        self.elapsed.as_secs_f32()
    }

    pub fn render_time_seconds(&self) -> f32 {
        self.render_elapsed.as_secs_f32()
    }

    pub fn render_revision(&self) -> u64 {
        self.render_revision
    }

    pub fn reduced_motion(&self) -> bool {
        self.reduced_motion
    }

    fn mark_render_revision(&mut self) {
        self.render_elapsed = self.elapsed;
        self.render_revision = self.render_revision.wrapping_add(1);
    }
}

fn normalize_percent(value: f32) -> f32 {
    if value.is_finite() {
        (value / 100.0).clamp(0.0, 1.0)
    } else {
        0.0
    }
}

fn normalize_ratio(used: f64, total: f64) -> f32 {
    if total.is_finite() && total > 0.0 && used.is_finite() {
        (used / total).clamp(0.0, 1.0) as f32
    } else {
        0.0
    }
}

fn normalize_load(load: f64, cpu_cores: usize) -> f32 {
    if load.is_finite() && cpu_cores > 0 {
        (load / cpu_cores as f64).clamp(0.0, 1.0) as f32
    } else {
        0.0
    }
}

fn network_activity(snapshot: &SystemInfo) -> f32 {
    let megabytes_per_second = snapshot
        .networks
        .iter()
        .map(|network| network.received_per_sec + network.transmitted_per_sec)
        .sum::<f64>();
    (megabytes_per_second / 100.0).clamp(0.0, 1.0) as f32
}

fn gpu_usage(snapshot: &SystemInfo) -> Option<f32> {
    let values = snapshot
        .gpus
        .iter()
        .filter_map(|gpu| gpu.usage.strip_suffix('%')?.parse::<f32>().ok())
        .filter(|usage| usage.is_finite());
    let (sum, count) = values.fold((0.0, 0_usize), |(sum, count), usage| {
        (sum + usage, count + 1)
    });

    (count > 0).then(|| normalize_percent(sum / count as f32))
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{HealthState, PulseInput, PulseModel};
    use crate::system::info::SystemInfo;

    #[test]
    fn normalizes_metrics_and_handles_missing_gpu() {
        let snapshot = snapshot(50.0, 4.0, 2.0);
        let input = PulseInput::from_snapshots(&snapshot, None);

        assert_eq!(input.cpu, 0.5);
        assert_eq!(input.memory, 0.5);
        assert_eq!(input.gpu, None);
        assert_eq!(input.load, 0.25);
        assert!(input.intensity() > 0.0);
    }

    #[test]
    fn computes_volatility_from_the_previous_snapshot() {
        let previous = snapshot(10.0, 4.0, 1.0);
        let current = snapshot(80.0, 4.0, 3.0);
        let input = PulseInput::from_snapshots(&current, Some(&previous));

        assert_eq!(input.volatility, 0.7);
    }

    #[test]
    fn gives_memory_a_meaningful_health_weight() {
        let memory = PulseInput {
            memory: 1.0,
            ..PulseInput::default()
        };
        let cpu = PulseInput {
            cpu: 1.0,
            ..PulseInput::default()
        };

        assert!(memory.pressure() > cpu.pressure());
    }

    #[test]
    fn maps_intensity_to_stable_health_states() {
        let mut pulse = PulseModel::new(false);
        pulse.set_input(PulseInput::default());
        assert_eq!(pulse.health().state(), HealthState::Calm);

        pulse.set_input(PulseInput {
            cpu: 1.0,
            memory: 1.0,
            gpu: Some(1.0),
            network: 1.0,
            load: 1.0,
            volatility: 0.0,
        });
        pulse.update(Duration::from_millis(300));
        assert_eq!(pulse.health().state(), HealthState::Critical);
    }

    #[test]
    fn pulse_animation_is_time_based_and_respects_reduced_motion() {
        let mut animated = PulseModel::new(false);
        animated.set_input(PulseInput {
            cpu: 0.8,
            memory: 0.6,
            gpu: None,
            network: 0.2,
            load: 0.5,
            volatility: 0.1,
        });
        let first = animated.breath();
        animated.update(Duration::from_millis(100));
        assert_ne!(first, animated.breath());
        assert!(!animated.is_active());

        let mut reduced = PulseModel::new(true);
        reduced.set_input(animated.input());
        assert_eq!(reduced.breath(), 0.5);
        assert_eq!(reduced.phase(), 0.0);
        assert!(!reduced.is_active());
    }

    #[test]
    fn updates_intensity_and_volatility_during_the_same_tick() {
        let mut pulse = PulseModel::new(false);
        pulse.set_input(PulseInput {
            cpu: 0.1,
            memory: 0.1,
            gpu: None,
            network: 0.0,
            load: 0.0,
            volatility: 0.0,
        });
        pulse.set_input(PulseInput {
            cpu: 0.9,
            memory: 0.8,
            gpu: None,
            network: 0.2,
            load: 0.7,
            volatility: 1.0,
        });

        pulse.update(Duration::from_millis(100));

        assert!(pulse.intensity() > 0.0);
        assert!(pulse.volatility() > 0.0);
    }

    #[test]
    fn ambient_updates_are_limited_to_the_pulse_cadence() {
        let mut pulse = PulseModel::new(false);
        pulse.set_input(PulseInput {
            cpu: 0.8,
            memory: 0.6,
            gpu: None,
            network: 0.2,
            load: 0.5,
            volatility: 0.1,
        });

        assert!(!pulse.update(Duration::from_millis(40)));
        assert!(!pulse.update(Duration::from_millis(39)));
        assert!(pulse.update(Duration::from_millis(1)));
        assert!(!pulse.update(Duration::from_millis(40)));
    }

    fn snapshot(cpu_usage: f32, total_ram: f64, used_ram: f64) -> SystemInfo {
        SystemInfo {
            os_name: "Test".to_string(),
            kernel: "Test".to_string(),
            cpu_name: "Test".to_string(),
            cpu_cores: 4,
            cpu_physical_cores: None,
            cpu_logical_threads: 4,
            cpu_usage,
            cpu_frequency_hz: None,
            cpu_temperature_celsius: None,
            total_ram,
            used_ram,
            total_swap: 0.0,
            used_swap: 0.0,
            processes: 0,
            uptime: 0,
            load_average: (1.0, 0.0, 0.0),
            disks: Vec::new(),
            storage_devices: Vec::new(),
            processes_list: Vec::new(),
            gpus: Vec::new(),
            networks: Vec::new(),
            all_networks: Vec::new(),
            temperatures: Vec::new(),
        }
    }
}
