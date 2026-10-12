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
}

#[derive(Debug, Clone, Copy, Default)]
pub struct FocusState {
    target: Option<FocusTarget>,
}

impl FocusState {
    pub fn target(self) -> Option<FocusTarget> {
        self.target
    }

    pub fn next_from(&mut self, targets: &[FocusTarget]) -> Option<FocusTarget> {
        if targets.is_empty() {
            self.clear();
            return None;
        }

        let next = match self.target {
            None => targets[0],
            Some(target) => match targets.iter().position(|candidate| *candidate == target) {
                Some(index) => targets[(index + 1) % targets.len()],
                None => targets[0],
            },
        };
        self.target = Some(next);
        Some(next)
    }

    pub fn previous_from(&mut self, targets: &[FocusTarget]) -> Option<FocusTarget> {
        if targets.is_empty() {
            self.clear();
            return None;
        }

        let previous = match self.target {
            None => *targets.last().expect("focus targets are not empty"),
            Some(target) => match targets.iter().position(|candidate| *candidate == target) {
                Some(index) => targets[(index + targets.len() - 1) % targets.len()],
                None => *targets.last().expect("focus targets are not empty"),
            },
        };
        self.target = Some(previous);
        Some(previous)
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

        assert_eq!(
            focus.next_from(&FocusTarget::ALL),
            Some(FocusTarget::Overview)
        );
        assert_eq!(
            focus.next_from(&FocusTarget::ALL),
            Some(FocusTarget::System)
        );
        assert_eq!(
            focus.previous_from(&FocusTarget::ALL),
            Some(FocusTarget::Overview)
        );
        focus.clear();
        assert_eq!(
            focus.previous_from(&FocusTarget::ALL),
            Some(FocusTarget::Network)
        );
    }
}
