use ratatui::layout::{Constraint, Direction, Layout, Rect};

use super::{COMPACT_DASHBOARD_WIDTH, MIN_DASHBOARD_HEIGHT, MIN_DASHBOARD_WIDTH};

pub(crate) struct DashboardLayout {
    pub(crate) page: [Rect; 3],
    pub(crate) main: [Rect; 3],
    pub(crate) top: [Rect; 2],
    pub(crate) middle: [Rect; 2],
    pub(crate) middle_left: [Rect; 2],
    pub(crate) system: [Rect; 2],
    pub(crate) bottom: [Rect; 3],
}

impl DashboardLayout {
    pub(crate) fn new(area: Rect) -> Option<Self> {
        if area.width < MIN_DASHBOARD_WIDTH || area.height < MIN_DASHBOARD_HEIGHT {
            return None;
        }

        let compact = area.width < COMPACT_DASHBOARD_WIDTH;

        let page = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(4),
                Constraint::Min(10),
                Constraint::Length(1),
            ])
            .margin(1)
            .spacing(1)
            .split(area);

        let main = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(34),
                Constraint::Percentage(36),
                Constraint::Percentage(30),
            ])
            .spacing(1)
            .split(page[1]);

        let top = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(if compact {
                [Constraint::Percentage(50), Constraint::Percentage(50)]
            } else {
                [Constraint::Percentage(42), Constraint::Percentage(58)]
            })
            .spacing(1)
            .split(main[0]);

        let middle = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(if compact {
                [Constraint::Percentage(50), Constraint::Percentage(50)]
            } else {
                [Constraint::Percentage(44), Constraint::Percentage(56)]
            })
            .spacing(1)
            .split(main[1]);

        let middle_left = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(5)])
            .spacing(1)
            .split(middle[0]);

        let system = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(5), Constraint::Length(3)])
            .spacing(1)
            .split(top[0]);

        let bottom = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(40),
                Constraint::Percentage(30),
                Constraint::Percentage(30),
            ])
            .spacing(1)
            .split(main[2]);

        Some(Self {
            page: [page[0], page[1], page[2]],
            main: [main[0], main[1], main[2]],
            top: [top[0], top[1]],
            middle: [middle[0], middle[1]],
            middle_left: [middle_left[0], middle_left[1]],
            system: [system[0], system[1]],
            bottom: [bottom[0], bottom[1], bottom[2]],
        })
    }
}
