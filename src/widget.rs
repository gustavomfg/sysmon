use crate::focus::FocusTarget;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WidgetId {
    Gpu,
    Memory,
    Network,
}

impl WidgetId {
    pub fn label(self) -> &'static str {
        match self {
            Self::Gpu => "GPU",
            Self::Memory => "Memory",
            Self::Network => "Network",
        }
    }

    pub fn focus_target(self) -> FocusTarget {
        match self {
            Self::Gpu => FocusTarget::Gpu,
            Self::Memory => FocusTarget::Memory,
            Self::Network => FocusTarget::Network,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct WidgetVisibility {
    gpu: bool,
    memory: bool,
    network: bool,
}

impl Default for WidgetVisibility {
    fn default() -> Self {
        Self {
            gpu: true,
            memory: true,
            network: true,
        }
    }
}

impl WidgetVisibility {
    pub fn is_visible(self, widget: WidgetId) -> bool {
        match widget {
            WidgetId::Gpu => self.gpu,
            WidgetId::Memory => self.memory,
            WidgetId::Network => self.network,
        }
    }

    pub fn is_focus_visible(self, target: FocusTarget) -> bool {
        match target {
            FocusTarget::Gpu => self.gpu,
            FocusTarget::Memory => self.memory,
            FocusTarget::Network => self.network,
            _ => true,
        }
    }

    pub fn toggle(&mut self, widget: WidgetId) -> bool {
        if self.visible_count() == 1 && self.is_visible(widget) {
            return false;
        }

        match widget {
            WidgetId::Gpu => self.gpu = !self.gpu,
            WidgetId::Memory => self.memory = !self.memory,
            WidgetId::Network => self.network = !self.network,
        }
        true
    }

    fn visible_count(self) -> usize {
        [self.gpu, self.memory, self.network]
            .into_iter()
            .filter(|visible| *visible)
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::{WidgetId, WidgetVisibility};

    #[test]
    fn never_hides_the_last_managed_widget() {
        let mut visibility = WidgetVisibility::default();

        assert!(visibility.toggle(WidgetId::Gpu));
        assert!(visibility.toggle(WidgetId::Memory));
        assert!(!visibility.toggle(WidgetId::Network));
        assert!(visibility.is_visible(WidgetId::Network));
    }
}
