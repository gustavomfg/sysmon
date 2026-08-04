use std::time::Duration;

use crate::{
    action::Action,
    animation::Tween,
    command_palette::{Command, CommandPaletteState, CommandStatus},
    config::{ConfigField, ConfigState, REFRESH_RATES},
    export,
    focus::{FocusState, FocusTarget},
    history::{History, Metric},
    process_search::ProcessSearchState,
    system::info::SystemInfo,
    theme::{Theme, ThemeKind},
    timeline::Timeline,
    widget::{WidgetId, WidgetVisibility},
};

pub struct AppState {
    snapshot: Option<SystemInfo>,
    history: History,
    theme: Theme,
    theme_kind: ThemeKind,
    cpu_tween: Tween,
    memory_tween: Tween,
    reduced_motion: bool,
    refresh_interval: Duration,
    widgets: WidgetVisibility,
    palette: CommandPaletteState,
    process_search: ProcessSearchState,
    selected_process: Option<String>,
    config: ConfigState,
    focus: FocusState,
    timeline: Timeline,
}

const HISTORY_LIMIT: usize = 60;
const TIMELINE_LIMIT: usize = 20;
const ANIMATION_DURATION: Duration = Duration::from_millis(250);
const DEFAULT_REFRESH_INTERVAL: Duration = REFRESH_RATES[0];

impl AppState {
    pub fn new() -> Self {
        Self::with_reduced_motion(false)
    }

    pub fn with_reduced_motion(reduced_motion: bool) -> Self {
        Self {
            snapshot: None,
            history: History::new(HISTORY_LIMIT),
            theme: ThemeKind::Pulse.theme(),
            theme_kind: ThemeKind::Pulse,
            cpu_tween: Tween::new(0.0, ANIMATION_DURATION, reduced_motion),
            memory_tween: Tween::new(0.0, ANIMATION_DURATION, reduced_motion),
            reduced_motion,
            refresh_interval: DEFAULT_REFRESH_INTERVAL,
            widgets: WidgetVisibility::default(),
            palette: CommandPaletteState::new(),
            process_search: ProcessSearchState::new(),
            selected_process: None,
            config: ConfigState::new(),
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

    pub fn theme_kind(&self) -> ThemeKind {
        self.theme_kind
    }

    pub fn reduced_motion(&self) -> bool {
        self.reduced_motion
    }

    pub fn refresh_interval(&self) -> Duration {
        self.refresh_interval
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

    pub fn process_search(&self) -> &ProcessSearchState {
        &self.process_search
    }

    pub fn config(&self) -> &ConfigState {
        &self.config
    }

    pub fn selected_process(&self) -> Option<&str> {
        self.selected_process.as_deref()
    }

    pub fn widget_visible(&self, widget: WidgetId) -> bool {
        self.widgets.is_visible(widget)
    }

    pub fn process_search_results(&self) -> Vec<&crate::system::info::ProcessInfo> {
        let query = self.process_search.query().to_ascii_lowercase();
        self.snapshot
            .as_ref()
            .map(|snapshot| {
                snapshot
                    .processes_list
                    .iter()
                    .filter(|process| process.name.to_ascii_lowercase().contains(&query))
                    .collect()
            })
            .unwrap_or_default()
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
            Action::ProcessSearchUp => {
                let count = self.process_search_results().len();
                self.process_search.move_selection(-1, count);
            }
            Action::ProcessSearchDown => {
                let count = self.process_search_results().len();
                self.process_search.move_selection(1, count);
            }
            Action::ProcessSearchBackspace => self.process_search.backspace(),
            Action::ProcessSearchCharacter(character) => self.process_search.insert(character),
            Action::ProcessSearchSubmit => {
                let selected = self
                    .process_search_results()
                    .get(self.process_search.selected_index())
                    .map(|process| process.name.clone());
                if let Some(name) = selected {
                    self.selected_process = Some(name.clone());
                    self.process_search.close();
                    self.timeline.record("Closed Process Search");
                    self.timeline.record(format!("Selected process: {name}"));
                }
            }
            Action::CloseProcessSearch => {
                if self.process_search.is_open() {
                    self.process_search.close();
                    self.timeline.record("Closed Process Search");
                }
            }
            Action::ConfigUp => self.config.move_selection(-1),
            Action::ConfigDown => self.config.move_selection(1),
            Action::ConfigPrevious => self.adjust_config_value(-1),
            Action::ConfigNext => self.adjust_config_value(1),
            Action::ConfigToggle => self.toggle_config_value(),
            Action::CloseConfig => {
                if self.config.is_open() {
                    self.config.close();
                    self.timeline.record("Closed Settings");
                }
            }
            Action::FocusNext => {
                if let Some(target) = self.focus.next_from(&self.visible_focus_targets()) {
                    self.timeline.record(format!("Focus: {}", target.label()));
                }
            }
            Action::FocusPrevious => {
                if let Some(target) = self.focus.previous_from(&self.visible_focus_targets()) {
                    self.timeline.record(format!("Focus: {}", target.label()));
                }
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
                Command::ChangeTheme => {
                    let theme = self.change_theme(1);
                    self.timeline
                        .record(format!("Executed: Change Theme ({})", theme.label()));
                    false
                }
                Command::ToggleGpuWidget => {
                    self.toggle_widget(WidgetId::Gpu);
                    false
                }
                Command::ToggleMemoryWidget => {
                    self.toggle_widget(WidgetId::Memory);
                    false
                }
                Command::ToggleNetworkWidget => {
                    self.toggle_widget(WidgetId::Network);
                    false
                }
                Command::ExportSnapshot => {
                    match self.snapshot.as_ref().map(export::export_snapshot) {
                        Some(Ok(path)) => self
                            .timeline
                            .record(format!("Snapshot exported: {}", path.display())),
                        Some(Err(error)) => self
                            .timeline
                            .record(format!("Snapshot export failed: {error}")),
                        None => self
                            .timeline
                            .record("Snapshot export failed: no snapshot available"),
                    }
                    false
                }
                Command::SearchProcess => {
                    self.process_search.open();
                    self.timeline.record("Opened Process Search");
                    false
                }
                Command::OpenSettings => {
                    self.config.open();
                    self.timeline.record("Opened Settings");
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
                Command::RestartWidget => false,
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

    fn change_theme(&mut self, direction: i32) -> ThemeKind {
        self.theme_kind = self.theme_kind.offset(direction);
        self.theme = self.theme_kind.theme();
        self.theme_kind
    }

    fn toggle_widget(&mut self, widget: WidgetId) {
        if !self.widgets.toggle(widget) {
            self.timeline.record(format!(
                "Cannot hide {} widget: at least one widget must remain visible",
                widget.label()
            ));
            return;
        }

        if !self.widgets.is_focus_visible(widget.focus_target()) {
            self.focus.clear();
        }

        let state = if self.widgets.is_visible(widget) {
            "shown"
        } else {
            "hidden"
        };
        self.timeline
            .record(format!("{} widget {state}", widget.label()));
    }

    fn visible_focus_targets(&self) -> Vec<FocusTarget> {
        FocusTarget::ALL
            .into_iter()
            .filter(|target| self.widgets.is_focus_visible(*target))
            .collect()
    }

    fn set_reduced_motion(&mut self, reduced_motion: bool) {
        self.reduced_motion = reduced_motion;
        self.cpu_tween.set_reduced_motion(reduced_motion);
        self.memory_tween.set_reduced_motion(reduced_motion);
        self.timeline.record(format!(
            "Reduced motion {}",
            if reduced_motion {
                "enabled"
            } else {
                "disabled"
            }
        ));
    }

    fn adjust_refresh_rate(&mut self, direction: i32) {
        let index = REFRESH_RATES
            .iter()
            .position(|rate| *rate == self.refresh_interval)
            .unwrap_or(0) as i32;
        let next = (index + direction).clamp(0, REFRESH_RATES.len() as i32 - 1) as usize;
        self.refresh_interval = REFRESH_RATES[next];
        self.timeline.record(format!(
            "Refresh rate set to {}s",
            self.refresh_interval.as_secs()
        ));
    }

    fn adjust_config_value(&mut self, direction: i32) {
        match self.config.selected() {
            ConfigField::Theme => {
                let theme = self.change_theme(direction);
                self.timeline
                    .record(format!("Theme changed to {}", theme.label()));
            }
            ConfigField::RefreshRate => self.adjust_refresh_rate(direction),
            ConfigField::ReducedMotion => {}
        }
    }

    fn toggle_config_value(&mut self) {
        match self.config.selected() {
            ConfigField::Theme => {
                let theme = self.change_theme(1);
                self.timeline
                    .record(format!("Theme changed to {}", theme.label()));
            }
            ConfigField::ReducedMotion => self.set_reduced_motion(!self.reduced_motion),
            ConfigField::RefreshRate => self.adjust_refresh_rate(1),
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
    use crate::{
        action::Action,
        focus::FocusTarget,
        history::Metric,
        system::info::{ProcessInfo, SystemInfo},
        theme::ThemeKind,
        widget::WidgetId,
    };

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
        for character in "restart".chars() {
            app.dispatch(Action::PaletteCharacter(character));
        }
        assert!(!app.dispatch(Action::PaletteSubmit));
        assert!(
            app.timeline()
                .events()
                .last()
                .unwrap()
                .contains("Unavailable: Restart Widget")
        );
    }

    #[test]
    fn changes_theme_immediately_through_the_palette() {
        let mut app = AppState::new();
        assert_eq!(app.theme_kind(), ThemeKind::Pulse);

        run_command(&mut app, "theme");

        assert_eq!(app.theme_kind(), ThemeKind::CatppuccinMocha);
        assert!(
            app.timeline()
                .events()
                .iter()
                .any(|event| event.contains("Executed: Change Theme (Catppuccin Mocha)"))
        );
    }

    #[test]
    fn toggles_widgets_and_keeps_focus_valid() {
        let mut app = AppState::new();
        for _ in 0..5 {
            app.dispatch(Action::FocusNext);
        }
        assert!(app.is_focused(FocusTarget::Gpu));

        run_command(&mut app, "gpu");
        assert!(!app.widget_visible(WidgetId::Gpu));
        assert!(!app.focus_active());

        run_command(&mut app, "memory");
        assert!(!app.widget_visible(WidgetId::Memory));
        run_command(&mut app, "network");
        assert!(app.widget_visible(WidgetId::Network));
        assert!(
            app.timeline()
                .events()
                .iter()
                .any(|event| event.contains("Cannot hide Network widget"))
        );
    }

    #[test]
    fn searches_and_selects_a_process() {
        let mut app = AppState::new();
        let mut snapshot = snapshot(42.0);
        snapshot.processes_list = vec![
            ProcessInfo {
                name: "shell".to_string(),
                memory: 10.0,
                cpu_usage: 1.0,
            },
            ProcessInfo {
                name: "editor".to_string(),
                memory: 20.0,
                cpu_usage: 2.0,
            },
        ];
        app.update(snapshot);

        run_command(&mut app, "search");
        for character in "SHELL".chars() {
            app.dispatch(Action::ProcessSearchCharacter(character));
        }
        app.dispatch(Action::ProcessSearchSubmit);

        assert_eq!(app.selected_process(), Some("shell"));
        assert!(!app.process_search().is_open());
        assert!(
            app.timeline()
                .events()
                .iter()
                .any(|event| event.contains("Selected process: shell"))
        );
    }

    #[test]
    fn edits_session_configuration() {
        let mut app = AppState::new();
        run_command(&mut app, "settings");
        app.dispatch(Action::ConfigDown);
        app.dispatch(Action::ConfigToggle);
        assert!(app.reduced_motion());

        app.dispatch(Action::ConfigDown);
        app.dispatch(Action::ConfigNext);
        assert_eq!(app.refresh_interval(), std::time::Duration::from_secs(2));
        assert!(
            app.timeline()
                .events()
                .iter()
                .any(|event| event.contains("Reduced motion enabled"))
        );
    }

    fn run_command(app: &mut AppState, query: &str) {
        app.dispatch(Action::OpenPalette);
        for character in query.chars() {
            app.dispatch(Action::PaletteCharacter(character));
        }
        assert!(!app.dispatch(Action::PaletteSubmit));
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
