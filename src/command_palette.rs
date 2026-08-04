#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    Quit,
    ShowTimeline,
    ToggleFocusMode,
    ChangeTheme,
    ToggleGpuWidget,
    RestartWidget,
    ExportSnapshot,
    SearchProcess,
    OpenSettings,
    ToggleMemoryWidget,
    ToggleNetworkWidget,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandStatus {
    Available,
    Planned,
    Hidden,
}

impl Command {
    pub const ALL: [Self; 11] = [
        Self::Quit,
        Self::ShowTimeline,
        Self::ToggleFocusMode,
        Self::ChangeTheme,
        Self::ToggleGpuWidget,
        Self::RestartWidget,
        Self::ExportSnapshot,
        Self::SearchProcess,
        Self::OpenSettings,
        Self::ToggleMemoryWidget,
        Self::ToggleNetworkWidget,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Quit => "Quit",
            Self::ShowTimeline => "Show Timeline",
            Self::ToggleFocusMode => "Toggle Focus Mode",
            Self::ChangeTheme => "Change Theme",
            Self::ToggleGpuWidget => "Toggle GPU Widget",
            Self::RestartWidget => "Restart Widget",
            Self::ExportSnapshot => "Export Snapshot",
            Self::SearchProcess => "Search Process",
            Self::OpenSettings => "Open Settings",
            Self::ToggleMemoryWidget => "Toggle Memory Widget",
            Self::ToggleNetworkWidget => "Toggle Network Widget",
        }
    }

    pub fn status(self) -> CommandStatus {
        match self {
            Self::Quit | Self::ShowTimeline | Self::ToggleFocusMode => CommandStatus::Available,
            Self::ChangeTheme
            | Self::ToggleGpuWidget
            | Self::RestartWidget
            | Self::ExportSnapshot
            | Self::SearchProcess
            | Self::OpenSettings => CommandStatus::Planned,
            Self::ToggleMemoryWidget | Self::ToggleNetworkWidget => CommandStatus::Hidden,
        }
    }

    pub fn planned_reason(self) -> Option<&'static str> {
        let reason = match self {
            Self::ChangeTheme => "theme selection is planned for a future phase",
            Self::ToggleGpuWidget => "widget visibility is planned for a future phase",
            Self::RestartWidget => "widget restart is planned for a future phase",
            Self::ExportSnapshot => "snapshot export is planned for a future phase",
            Self::SearchProcess => "process search is planned for a future phase",
            Self::OpenSettings => "settings are planned for a future phase",
            _ => return None,
        };
        Some(reason)
    }

    fn matches(self, query: &str) -> bool {
        self.label().to_ascii_lowercase().contains(query)
            || self
                .keywords()
                .iter()
                .any(|keyword| keyword.contains(query))
    }

    fn keywords(self) -> &'static [&'static str] {
        match self {
            Self::ShowTimeline => &["timeline", "events"],
            Self::ToggleFocusMode => &["focus", "focused"],
            Self::ToggleGpuWidget => &["gpu", "graphics"],
            Self::OpenSettings => &["settings", "config", "configuration"],
            Self::ChangeTheme => &["theme", "colors", "colour"],
            Self::RestartWidget => &["restart", "reload"],
            Self::ExportSnapshot => &["export", "snapshot"],
            Self::SearchProcess => &["search", "process", "processes"],
            Self::Quit => &["exit", "close"],
            Self::ToggleMemoryWidget => &["memory", "ram"],
            Self::ToggleNetworkWidget => &["network", "net"],
        }
    }
}

pub struct CommandPaletteState {
    open: bool,
    query: String,
    selected: usize,
}

impl CommandPaletteState {
    pub fn new() -> Self {
        Self {
            open: false,
            query: String::new(),
            selected: 0,
        }
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn query(&self) -> &str {
        &self.query
    }

    pub fn open(&mut self) {
        self.open = true;
        self.query.clear();
        self.selected = 0;
    }

    pub fn close(&mut self) {
        self.open = false;
        self.query.clear();
        self.selected = 0;
    }

    pub fn insert(&mut self, character: char) {
        if !character.is_control() && self.query.chars().count() < 64 {
            self.query.push(character);
            self.selected = 0;
        }
    }

    pub fn backspace(&mut self) {
        self.query.pop();
        self.selected = 0;
    }

    pub fn move_selection(&mut self, direction: i32) {
        let count = self.filtered_commands().len();
        if count == 0 {
            self.selected = 0;
            return;
        }

        self.selected = match direction.cmp(&0) {
            std::cmp::Ordering::Less if self.selected == 0 => count - 1,
            std::cmp::Ordering::Less => self.selected - 1,
            std::cmp::Ordering::Equal => self.selected,
            std::cmp::Ordering::Greater if self.selected + 1 == count => 0,
            std::cmp::Ordering::Greater => self.selected + 1,
        };
    }

    pub fn filtered_commands(&self) -> Vec<Command> {
        let query = self.query.to_ascii_lowercase();
        Command::ALL
            .into_iter()
            .filter(|command| command.status() != CommandStatus::Hidden)
            .filter(|command| command.matches(query.as_str()))
            .collect()
    }

    pub fn selected_command(&self) -> Option<Command> {
        self.filtered_commands().get(self.selected).copied()
    }

    pub fn selected_index(&self) -> usize {
        self.selected
    }
}

#[cfg(test)]
mod tests {
    use super::{Command, CommandPaletteState, CommandStatus};

    #[test]
    fn classifies_current_commands_and_hides_future_only_commands() {
        assert_eq!(Command::Quit.status(), CommandStatus::Available);
        assert_eq!(Command::ShowTimeline.status(), CommandStatus::Available);
        assert_eq!(Command::ToggleFocusMode.status(), CommandStatus::Available);
        assert_eq!(Command::ChangeTheme.status(), CommandStatus::Planned);
        assert_eq!(Command::ToggleMemoryWidget.status(), CommandStatus::Hidden);

        let palette = CommandPaletteState::new();
        let visible = palette.filtered_commands();
        assert_eq!(visible.len(), 9);
        assert!(!visible.contains(&Command::ToggleMemoryWidget));
        assert!(!visible.contains(&Command::ToggleNetworkWidget));
    }

    #[test]
    fn filters_commands_and_wraps_selection() {
        let mut palette = CommandPaletteState::new();
        palette.open();
        for character in "gpu".chars() {
            palette.insert(character);
        }

        assert_eq!(palette.filtered_commands(), vec![Command::ToggleGpuWidget]);
        assert_eq!(palette.selected_command(), Some(Command::ToggleGpuWidget));

        let mut settings = CommandPaletteState::new();
        settings.open();
        for character in "config".chars() {
            settings.insert(character);
        }
        assert_eq!(settings.selected_command(), Some(Command::OpenSettings));

        let mut all_commands = CommandPaletteState::new();
        all_commands.open();
        all_commands.move_selection(-1);
        assert_eq!(all_commands.selected_command(), Some(Command::OpenSettings));
    }

    #[test]
    fn limits_query_length_and_resets_selection_when_query_changes() {
        let mut palette = CommandPaletteState::new();
        palette.open();

        for _ in 0..80 {
            palette.insert('x');
        }

        assert_eq!(palette.query().chars().count(), 64);
        assert!(palette.filtered_commands().is_empty());
        assert_eq!(palette.selected_command(), None);
    }
}
