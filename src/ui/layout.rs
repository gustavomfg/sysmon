use ratatui::layout::{Constraint, Direction, Layout, Rect};

use crate::widget::{WidgetId, WidgetVisibility};

use super::{MIN_DASHBOARD_HEIGHT, MIN_DASHBOARD_WIDTH};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LayoutMode {
    Compact,
    Standard,
    Widescreen,
}

pub(crate) struct DashboardLayout {
    pub(crate) header: Rect,
    pub(crate) pulse: Rect,
    pub(crate) system: Rect,
    pub(crate) cpu: Rect,
    pub(crate) memory: Option<Rect>,
    pub(crate) gpu: Option<Rect>,
    pub(crate) processes: Rect,
    pub(crate) storage: Rect,
    pub(crate) history: Rect,
    pub(crate) network: Option<Rect>,
    pub(crate) timeline: Rect,
    pub(crate) footer: Rect,
}

impl DashboardLayout {
    pub(crate) fn new(area: Rect, visibility: &WidgetVisibility) -> Option<Self> {
        if area.width < MIN_DASHBOARD_WIDTH || area.height < MIN_DASHBOARD_HEIGHT {
            return None;
        }

        let mode = layout_mode(area);
        let page = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(10),
                Constraint::Length(1),
            ])
            .margin(1)
            .spacing(1)
            .split(area);
        let body = Layout::default()
            .direction(Direction::Vertical)
            .constraints(body_constraints(mode))
            .spacing(1)
            .split(page[1]);

        let top = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(pulse_width(mode)),
                Constraint::Min(1),
            ])
            .spacing(1)
            .split(body[0]);
        let top_right = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(1)])
            .spacing(1)
            .split(top[1]);
        let metric_areas = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(metric_constraints(visibility))
            .spacing(1)
            .split(top_right[1]);

        let mut metric_index = 0;
        let cpu = metric_areas[metric_index];
        metric_index += 1;
        let memory = if visibility.is_visible(WidgetId::Memory) {
            let area = metric_areas[metric_index];
            metric_index += 1;
            Some(area)
        } else {
            None
        };
        let gpu = if visibility.is_visible(WidgetId::Gpu) {
            Some(metric_areas[metric_index])
        } else {
            None
        };

        let middle = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(middle_constraints(mode))
            .spacing(1)
            .split(body[1]);
        let bottom = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(history_width(mode)),
                Constraint::Min(1),
            ])
            .spacing(1)
            .split(body[2]);
        let bottom_right = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(if visibility.is_visible(WidgetId::Network) {
                vec![Constraint::Percentage(50), Constraint::Percentage(50)]
            } else {
                vec![Constraint::Percentage(100)]
            })
            .spacing(1)
            .split(bottom[1]);

        Some(Self {
            header: page[0],
            pulse: top[0],
            system: top_right[0],
            cpu,
            memory,
            gpu,
            processes: middle[0],
            storage: middle[1],
            history: bottom[0],
            network: visibility
                .is_visible(WidgetId::Network)
                .then_some(bottom_right[0]),
            timeline: if visibility.is_visible(WidgetId::Network) {
                bottom_right[1]
            } else {
                bottom_right[0]
            },
            footer: page[2],
        })
    }
}

fn layout_mode(area: Rect) -> LayoutMode {
    if area.width >= 140 && area.height >= 32 {
        LayoutMode::Widescreen
    } else if area.width >= 100 && area.height >= 28 {
        LayoutMode::Standard
    } else {
        LayoutMode::Compact
    }
}

fn body_constraints(mode: LayoutMode) -> [Constraint; 3] {
    match mode {
        LayoutMode::Compact => [
            Constraint::Percentage(50),
            Constraint::Percentage(28),
            Constraint::Percentage(22),
        ],
        LayoutMode::Standard => [
            Constraint::Percentage(42),
            Constraint::Percentage(32),
            Constraint::Percentage(26),
        ],
        LayoutMode::Widescreen => [
            Constraint::Percentage(48),
            Constraint::Percentage(28),
            Constraint::Percentage(24),
        ],
    }
}

fn pulse_width(mode: LayoutMode) -> u16 {
    match mode {
        LayoutMode::Compact => 44,
        LayoutMode::Standard => 40,
        LayoutMode::Widescreen => 34,
    }
}

fn middle_constraints(mode: LayoutMode) -> [Constraint; 2] {
    match mode {
        LayoutMode::Compact => [Constraint::Percentage(58), Constraint::Percentage(42)],
        LayoutMode::Standard => [Constraint::Percentage(62), Constraint::Percentage(38)],
        LayoutMode::Widescreen => [Constraint::Percentage(68), Constraint::Percentage(32)],
    }
}

fn history_width(mode: LayoutMode) -> u16 {
    match mode {
        LayoutMode::Compact => 36,
        LayoutMode::Standard => 38,
        LayoutMode::Widescreen => 32,
    }
}

fn metric_constraints(visibility: &WidgetVisibility) -> Vec<Constraint> {
    let count = 1
        + usize::from(visibility.is_visible(WidgetId::Memory))
        + usize::from(visibility.is_visible(WidgetId::Gpu));
    (0..count)
        .map(|_| Constraint::Ratio(1, count as u32))
        .collect()
}

#[cfg(test)]
mod tests {
    use ratatui::layout::Rect;

    use super::DashboardLayout;
    use crate::widget::{WidgetId, WidgetVisibility};

    #[test]
    fn adapts_regions_and_removes_hidden_widget_areas() {
        let visibility = WidgetVisibility::default();
        let compact = DashboardLayout::new(Rect::new(80, 0, 80, 24), &visibility)
            .expect("compact layout should fit");
        let widescreen = DashboardLayout::new(Rect::new(0, 0, 160, 40), &visibility)
            .expect("widescreen layout should fit");

        assert!(compact.pulse.width > 0);
        assert!(widescreen.pulse.width > compact.pulse.width);

        let mut hidden = visibility;
        hidden.toggle(WidgetId::Network);
        let layout = DashboardLayout::new(Rect::new(0, 0, 120, 32), &hidden)
            .expect("layout should fit without network");
        assert!(layout.network.is_none());
        assert!(layout.timeline.width > 0);
    }

    #[test]
    fn rejects_terminals_below_the_existing_minimum() {
        assert!(
            DashboardLayout::new(Rect::new(0, 0, 79, 24), &WidgetVisibility::default()).is_none()
        );
    }
}
