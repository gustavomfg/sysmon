use std::time::Duration;

pub const REFRESH_RATES: [Duration; 3] = [
    Duration::from_secs(1),
    Duration::from_secs(2),
    Duration::from_secs(4),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigField {
    Theme,
    ReducedMotion,
    RefreshRate,
}

impl ConfigField {
    pub const ALL: [Self; 3] = [Self::Theme, Self::ReducedMotion, Self::RefreshRate];

    pub fn label(self) -> &'static str {
        match self {
            Self::Theme => "Theme",
            Self::ReducedMotion => "Reduced Motion",
            Self::RefreshRate => "Refresh Rate",
        }
    }
}

pub struct ConfigState {
    open: bool,
    selected: usize,
}

impl ConfigState {
    pub fn new() -> Self {
        Self {
            open: false,
            selected: 0,
        }
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn selected(&self) -> ConfigField {
        ConfigField::ALL[self.selected]
    }

    pub fn open(&mut self) {
        self.open = true;
        self.selected = 0;
    }

    pub fn close(&mut self) {
        self.open = false;
        self.selected = 0;
    }

    pub fn move_selection(&mut self, direction: i32) {
        let count = ConfigField::ALL.len();
        self.selected = match direction.cmp(&0) {
            std::cmp::Ordering::Less if self.selected == 0 => count - 1,
            std::cmp::Ordering::Less => self.selected - 1,
            std::cmp::Ordering::Equal => self.selected,
            std::cmp::Ordering::Greater if self.selected + 1 == count => 0,
            std::cmp::Ordering::Greater => self.selected + 1,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::{ConfigField, ConfigState, REFRESH_RATES};

    #[test]
    fn cycles_configuration_fields_and_defines_refresh_rates() {
        let mut config = ConfigState::new();
        config.open();
        assert_eq!(config.selected(), ConfigField::Theme);
        config.move_selection(1);
        assert_eq!(config.selected(), ConfigField::ReducedMotion);
        config.move_selection(-1);
        assert_eq!(config.selected(), ConfigField::Theme);
        assert_eq!(REFRESH_RATES.len(), 3);
    }
}
