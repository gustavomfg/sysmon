use std::time::Duration;

use crate::{
    action::Action,
    animation::Tween,
    command_palette::{Command, CommandPaletteState, CommandStatus},
    focus::{FocusState, FocusTarget},
    history::{History, Metric},
    system::info::SystemInfo,
    theme::Theme,
    timeline::Timeline,
};

pub struct AppState {
    snapshot: Option<SystemInfo>,
    history: History,
    theme: Theme,
    cpu_tween: Tween,
    memory_tween: Tween,
    palette: CommandPaletteState,
    focus: FocusState,
    timeline: Timeline,
}

const HISTORY_LIMIT: usize = 60;
const TIMELINE_LIMIT: usize = 20;
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
            palette: CommandPaletteState::new(),
            focus: FocusState::default(),
            timeline: {
                let mut timeline = Timeline::new(TIMELINE_LIMIT);
                timeline.record("SysMon started");
                timeline
            },
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

    pub fn palette(&self) -> &CommandPaletteState {
        &self.palette
    }

    pub fn timeline(&self) -> &Timeline {
        &self.timeline
    }

    pub fn is_focused(&self, target: FocusTarget) -> bool {
        self.focus.target() == Some(target)
    }

    pub fn focus_active(&self) -> bool {
        self.focus.target().is_some()
    }

    pub fn dispatch(&mut self, action: Action) -> bool {
        match action {
            Action::Quit => return true,
            Action::OpenPalette => {
                if !self.palette.is_open() {
                    self.palette.open();
                    self.timeline.record("Opened Command Palette");
                }
            }
            Action::ClosePalette => {
                if self.palette.is_open() {
                    self.palette.close();
                    self.timeline.record("Closed Command Palette");
                }
            }
            Action::PaletteUp => self.palette.move_selection(-1),
            Action::PaletteDown => self.palette.move_selection(1),
            Action::PaletteBackspace => self.palette.backspace(),
            Action::PaletteCharacter(character) => self.palette.insert(character),
            Action::PaletteSubmit => {
                if let Some(command) = self.palette.selected_command() {
                    self.palette.close();
                    self.timeline.record("Closed Command Palette");
                    return self.execute_command(command);
                }
            }
            Action::FocusNext => {
                let target = self.focus.next();
                self.timeline.record(format!("Focus: {}", target.label()));
            }
            Action::FocusPrevious => {
                let target = self.focus.previous();
                self.timeline.record(format!("Focus: {}", target.label()));
            }
            Action::ExitFocus => {
                if self.focus_active() {
                    self.focus.clear();
                    self.timeline.record("Focus mode off");
                }
            }
        }

        false
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

    fn execute_command(&mut self, command: Command) -> bool {
        match command.status() {
            CommandStatus::Available => match command {
                Command::Quit => {
                    self.timeline.record("Executed: Quit");
                    true
                }
                Command::ShowTimeline => {
                    self.focus.set(FocusTarget::Timeline);
                    self.timeline.record("Executed: Show Timeline");
                    false
                }
                Command::ToggleFocusMode => {
                    if self.focus_active() {
                        self.focus.clear();
                    } else {
                        self.focus.set(FocusTarget::Overview);
                    }
                    self.timeline.record("Executed: Toggle Focus Mode");
                    false
                }
                _ => false,
            },
            CommandStatus::Planned => {
                if let Some(reason) = command.planned_reason() {
                    self.timeline
                        .record(format!("Unavailable: {} — {reason}", command.label()));
                }
                false
            }
            CommandStatus::Hidden => false,
        }
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
    use crate::{action::Action, focus::FocusTarget, history::Metric, system::info::SystemInfo};

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

    #[test]
    fn dispatches_available_and_planned_palette_commands_to_state() {
        let mut app = AppState::new();

        assert!(app.dispatch(Action::Quit));
        app.dispatch(Action::FocusNext);
        assert!(app.is_focused(FocusTarget::Overview));
        app.dispatch(Action::OpenPalette);
        assert!(app.palette().is_open());
        app.dispatch(Action::PaletteDown);
        assert!(!app.dispatch(Action::PaletteSubmit));
        assert!(!app.palette().is_open());
        assert!(app.is_focused(FocusTarget::Timeline));
        assert!(
            app.timeline()
                .events()
                .last()
                .unwrap()
                .contains("Executed: Show Timeline")
        );

        app.dispatch(Action::OpenPalette);
        for character in "settings".chars() {
            app.dispatch(Action::PaletteCharacter(character));
        }
        assert!(!app.dispatch(Action::PaletteSubmit));
        assert!(
            app.timeline()
                .events()
                .last()
                .unwrap()
                .contains("Unavailable: Open Settings")
        );
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
