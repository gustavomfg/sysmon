use std::time::Duration;

use crate::{
    animation::Tween,
    history::{History, Metric},
    system::info::SystemInfo,
    theme::Theme,
};

pub struct AppState {
    snapshot: Option<SystemInfo>,
    history: History,
    theme: Theme,
    cpu_tween: Tween,
    memory_tween: Tween,
}

const HISTORY_LIMIT: usize = 60;
const ANIMATION_DURATION: Duration = Duration::from_millis(250);

impl AppState {
    pub fn new() -> Self {
        Self::with_reduced_motion(false)
    }

    pub fn with_reduced_motion(reduced_motion: bool) -> Self {
        Self {
            snapshot: None,
            history: History::new(HISTORY_LIMIT),
            theme: Theme::default(),
            cpu_tween: Tween::new(0.0, ANIMATION_DURATION, reduced_motion),
            memory_tween: Tween::new(0.0, ANIMATION_DURATION, reduced_motion),
        }
    }

    pub fn update(&mut self, snapshot: SystemInfo) {
        let first_snapshot = self.snapshot.is_none();
        let cpu_usage = snapshot.cpu_usage.clamp(0.0, 100.0) as f64;
        let memory_usage = memory_usage_percent(snapshot.used_ram, snapshot.total_ram);

        self.history.record(
            Metric::Cpu,
            Some(snapshot.cpu_usage.clamp(0.0, 100.0).round() as f64),
        );
        self.record_history(&snapshot);

        if first_snapshot {
            self.cpu_tween.snap_to(cpu_usage);
            self.memory_tween.snap_to(memory_usage);
        } else {
            self.cpu_tween.set_target_clamped(cpu_usage, 0.0, 100.0);
            self.memory_tween
                .set_target_clamped(memory_usage, 0.0, 100.0);
        }

        self.snapshot = Some(snapshot);
    }

    pub fn snapshot(&self) -> Option<&SystemInfo> {
        self.snapshot.as_ref()
    }

    pub fn history(&self) -> &History {
        &self.history
    }

    pub fn theme(&self) -> &Theme {
        &self.theme
    }

    pub fn cpu_usage(&self) -> f64 {
        self.cpu_tween.current()
    }

    pub fn memory_usage_percent(&self) -> f64 {
        self.memory_tween.current()
    }

    pub fn update_animations(&mut self, delta: Duration) -> bool {
        self.cpu_tween.update(delta) || self.memory_tween.update(delta)
    }

    pub fn animations_active(&self) -> bool {
        self.cpu_tween.is_active() || self.memory_tween.is_active()
    }

    fn record_history(&mut self, snapshot: &SystemInfo) {
        self.history.record(
            Metric::Memory,
            Some(memory_usage_percent(snapshot.used_ram, snapshot.total_ram)),
        );
        self.history.record(
            Metric::NetworkDownload,
            Some(
                snapshot
                    .networks
                    .iter()
                    .map(|network| network.received_per_sec)
                    .sum(),
            ),
        );
        self.history.record(
            Metric::NetworkUpload,
            Some(
                snapshot
                    .networks
                    .iter()
                    .map(|network| network.transmitted_per_sec)
                    .sum(),
            ),
        );

        let gpu_usages = snapshot
            .gpus
            .iter()
            .filter_map(|gpu| gpu.usage.strip_suffix('%')?.parse::<f64>().ok())
            .collect::<Vec<_>>();
        let gpu_usage = if gpu_usages.is_empty() {
            None
        } else {
            Some(gpu_usages.iter().sum::<f64>() / gpu_usages.len() as f64)
        };
        self.history.record(Metric::Gpu, gpu_usage);
    }
}

fn memory_usage_percent(used: f64, total: f64) -> f64 {
    if total > 0.0 {
        (used / total * 100.0).clamp(0.0, 100.0)
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::{AppState, HISTORY_LIMIT};
    use crate::{history::Metric, system::info::SystemInfo};

    #[test]
    fn keeps_only_the_latest_sixty_cpu_samples() {
        let mut app = AppState::new();

        for sample in 0..=HISTORY_LIMIT {
            app.update(snapshot(sample as f32));
        }

        let samples = app.history().samples(Metric::Cpu);
        assert_eq!(samples.len(), HISTORY_LIMIT);
        assert_eq!(samples.first(), Some(&1.0));
        assert_eq!(samples.last(), Some(&(HISTORY_LIMIT as f64)));
    }

    #[test]
    fn clamps_and_rounds_cpu_samples() {
        let mut app = AppState::new();

        for sample in [-10.0, 42.6, 150.0] {
            app.update(snapshot(sample));
        }

        assert_eq!(app.history().samples(Metric::Cpu), &[0.0, 43.0, 100.0]);
    }

    fn snapshot(cpu_usage: f32) -> SystemInfo {
        SystemInfo {
            os_name: "Test OS".to_string(),
            kernel: "test".to_string(),
            cpu_name: "Test CPU".to_string(),
            cpu_cores: 1,
            cpu_usage,
            total_ram: 1.0,
            used_ram: 0.5,
            total_swap: 0.0,
            used_swap: 0.0,
            processes: 0,
            uptime: 0,
            load_average: (0.0, 0.0, 0.0),
            disks: Vec::new(),
            processes_list: Vec::new(),
            gpus: Vec::new(),
            networks: Vec::new(),
        }
    }
}
