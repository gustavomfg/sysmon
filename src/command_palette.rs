#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    ToggleGpu,
    ChangeTheme,
    RestartWidget,
    ExportSnapshot,
    SearchProcess,
    OpenConfig,
}

impl Command {
    pub const ALL: [Self; 6] = [
        Self::ToggleGpu,
        Self::ChangeTheme,
        Self::RestartWidget,
        Self::ExportSnapshot,
        Self::SearchProcess,
        Self::OpenConfig,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::ToggleGpu => "Toggle GPU",
            Self::ChangeTheme => "Change Theme",
            Self::RestartWidget => "Restart Widget",
            Self::ExportSnapshot => "Export Snapshot",
            Self::SearchProcess => "Search Process",
            Self::OpenConfig => "Open Config",
        }
    }

    pub fn unavailable_reason(self) -> &'static str {
        match self {
            Self::ToggleGpu => "widget visibility is outside Phase 3",
            Self::ChangeTheme => "theme selection is not implemented",
            Self::RestartWidget => "widget restart lifecycle is not defined",
            Self::ExportSnapshot => "snapshot export is not implemented",
            Self::SearchProcess => "process search is not implemented",
            Self::OpenConfig => "configuration support is not implemented",
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
            .filter(|command| command.label().to_ascii_lowercase().contains(&query))
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
    use super::{Command, CommandPaletteState};

    #[test]
    fn exposes_only_the_documented_commands() {
        assert_eq!(Command::ALL.len(), 6);
        assert_eq!(Command::ALL[0].label(), "Toggle GPU");
        assert_eq!(Command::ALL[5].label(), "Open Config");
    }

    #[test]
    fn filters_commands_and_wraps_selection() {
        let mut palette = CommandPaletteState::new();
        palette.open();
        for character in "toggle".chars() {
            palette.insert(character);
        }

        assert_eq!(palette.filtered_commands(), vec![Command::ToggleGpu]);
        assert_eq!(palette.selected_command(), Some(Command::ToggleGpu));

        let mut all_commands = CommandPaletteState::new();
        all_commands.open();
        all_commands.move_selection(-1);
        assert_eq!(all_commands.selected_command(), Some(Command::OpenConfig));
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
