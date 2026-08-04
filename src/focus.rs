#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusTarget {
    Overview,
    System,
    Cpu,
    Memory,
    Gpu,
    Processes,
    Storage,
    History,
    Timeline,
    Network,
}

impl FocusTarget {
    pub const ALL: [Self; 10] = [
        Self::Overview,
        Self::System,
        Self::Cpu,
        Self::Memory,
        Self::Gpu,
        Self::Processes,
        Self::Storage,
        Self::History,
        Self::Timeline,
        Self::Network,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Overview => "Overview",
            Self::System => "System",
            Self::Cpu => "CPU",
            Self::Memory => "Memory",
            Self::Gpu => "GPU",
            Self::Processes => "Processes",
            Self::Storage => "Storage",
            Self::History => "History",
            Self::Timeline => "Timeline",
            Self::Network => "Network",
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct FocusState {
    target: Option<FocusTarget>,
}

impl FocusState {
    pub fn target(self) -> Option<FocusTarget> {
        self.target
    }

    pub fn next(&mut self) -> FocusTarget {
        let next = match self.target {
            None => FocusTarget::ALL[0],
            Some(target) => {
                let index = FocusTarget::ALL
                    .iter()
                    .position(|candidate| *candidate == target)
                    .unwrap_or(0);
                FocusTarget::ALL[(index + 1) % FocusTarget::ALL.len()]
            }
        };
        self.target = Some(next);
        next
    }

    pub fn previous(&mut self) -> FocusTarget {
        let previous = match self.target {
            None => *FocusTarget::ALL
                .last()
                .expect("focus targets are not empty"),
            Some(target) => {
                let index = FocusTarget::ALL
                    .iter()
                    .position(|candidate| *candidate == target)
                    .unwrap_or(0);
                FocusTarget::ALL[(index + FocusTarget::ALL.len() - 1) % FocusTarget::ALL.len()]
            }
        };
        self.target = Some(previous);
        previous
    }

    pub fn clear(&mut self) {
        self.target = None;
    }

    pub fn set(&mut self, target: FocusTarget) {
        self.target = Some(target);
    }
}

#[cfg(test)]
mod tests {
    use super::{FocusState, FocusTarget};

    #[test]
    fn cycles_focus_forward_and_backward() {
        let mut focus = FocusState::default();

        assert_eq!(focus.next(), FocusTarget::Overview);
        assert_eq!(focus.next(), FocusTarget::System);
        assert_eq!(focus.previous(), FocusTarget::Overview);
        focus.clear();
        assert_eq!(focus.previous(), FocusTarget::Network);
    }
}
