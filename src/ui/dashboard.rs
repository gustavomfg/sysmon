use crate::app::AppState;
use crate::system::info::SystemInfo;

use ratatui::Frame;

use super::{
    layout::DashboardLayout,
    renderer::{BACKGROUND, SURFACE, fill},
    widgets,
};

pub fn draw_app(frame: &mut Frame, app: &AppState) {
    if let Some(info) = app.snapshot() {
        draw(frame, info, app);
    } else {
        frame.render_widget(fill(BACKGROUND), frame.area());
    }
}

pub fn draw(frame: &mut Frame, info: &SystemInfo, app: &AppState) {
    frame.render_widget(fill(BACKGROUND), frame.area());

    let Some(layout) = DashboardLayout::new(frame.area()) else {
        widgets::overview::draw_minimum_size_message(frame);
        return;
    };

    for area in layout.page.iter().copied() {
        frame.render_widget(fill(BACKGROUND), area);
    }

    widgets::overview::draw(frame, layout.page[0], info);

    for area in layout.main.iter().copied() {
        frame.render_widget(fill(BACKGROUND), area);
    }

    for area in layout.top.iter().copied() {
        frame.render_widget(fill(SURFACE), area);
    }

    for area in layout.middle.iter().copied() {
        frame.render_widget(fill(SURFACE), area);
    }

    for area in layout.middle_left.iter().copied() {
        frame.render_widget(fill(SURFACE), area);
    }

    for area in layout.system.iter().copied() {
        frame.render_widget(fill(SURFACE), area);
    }

    widgets::system::draw_info(frame, layout.system[0], info);
    widgets::cpu::draw(frame, layout.system[1], info);
    widgets::gpu::draw(frame, layout.top[1], info);
    widgets::memory::draw(frame, layout.middle_left[0], info);
    widgets::processes::draw(frame, layout.middle[1], info);

    for area in layout.bottom.iter().copied() {
        frame.render_widget(fill(SURFACE), area);
    }

    widgets::storage::draw(frame, layout.middle_left[1], info);
    widgets::history::draw(frame, layout.bottom[0], info, app);
    widgets::network::draw(frame, layout.bottom[1], info);
    widgets::footer::draw(frame, layout.page[2]);
}

#[cfg(test)]
mod tests {
    use ratatui::{Terminal, backend::TestBackend};

    use super::draw;
    use crate::{
        app::AppState,
        system::info::SystemInfo,
        ui::renderer::{disk_usage, format_uptime, usage_percent},
    };

    #[test]
    fn formats_uptime_with_and_without_days() {
        assert_eq!(format_uptime(3_661), "1h 1m");
        assert_eq!(format_uptime(90_061), "1d 1h 1m");
    }

    #[test]
    fn handles_zero_and_out_of_range_usage_values() {
        assert_eq!(usage_percent(5.0, 0.0), 0.0);
        assert_eq!(usage_percent(-1.0, 10.0), 0.0);
        assert_eq!(usage_percent(12.0, 10.0), 100.0);
    }

    #[test]
    fn handles_empty_disks_without_division_by_zero() {
        assert_eq!(disk_usage(0.0, 0.0), (0.0, 0.0));
    }

    #[test]
    fn renders_dashboard_at_representative_terminal_sizes() {
        let info = sample_system_info();
        let mut app = AppState::new();
        app.update_cpu_history(info.cpu_usage);

        for (width, height) in [(80, 24), (120, 40)] {
            let backend = TestBackend::new(width, height);
            let mut terminal = Terminal::new(backend).expect("test terminal should be created");

            terminal
                .draw(|frame| draw(frame, &info, &app))
                .expect("dashboard should render");

            let rendered = terminal
                .backend()
                .buffer()
                .content
                .iter()
                .map(|cell| cell.symbol())
                .collect::<String>();
            assert!(rendered.contains("SYSMON"));
        }
    }

    #[test]
    fn explains_the_minimum_size_in_small_terminals() {
        let backend = TestBackend::new(60, 20);
        let mut terminal = Terminal::new(backend).expect("test terminal should be created");

        terminal
            .draw(|frame| draw(frame, &sample_system_info(), &AppState::new()))
            .expect("minimum-size message should render");

        let rendered = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(rendered.contains("Janela muito pequena"));
        assert!(rendered.contains("80x24"));
    }

    fn sample_system_info() -> SystemInfo {
        SystemInfo {
            os_name: "Linux".to_string(),
            kernel: "6.x".to_string(),
            cpu_name: "Test CPU".to_string(),
            cpu_cores: 8,
            cpu_usage: 42.0,
            total_ram: 16.0,
            used_ram: 8.0,
            total_swap: 4.0,
            used_swap: 1.0,
            processes: 100,
            uptime: 90_061,
            load_average: (0.5, 0.4, 0.3),
            disks: Vec::new(),
            processes_list: Vec::new(),
            gpus: Vec::new(),
            networks: Vec::new(),
        }
    }
}
