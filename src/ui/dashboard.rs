use crate::app::AppState;
use crate::focus::FocusTarget;
use crate::theme::Theme;

use ratatui::Frame;

use super::{layout::DashboardLayout, renderer::fill, widgets};

pub fn draw_app(frame: &mut Frame, app: &AppState) {
    if let Some(info) = app.snapshot() {
        draw_with_theme(frame, info, app, app.theme());
    } else {
        frame.render_widget(fill(app.theme().background), frame.area());
    }

    if app.palette().is_open() {
        widgets::command_palette::draw(frame, frame.area(), app, app.theme());
    }
}

fn draw_with_theme(
    frame: &mut Frame,
    info: &crate::system::info::SystemInfo,
    app: &AppState,
    theme: &Theme,
) {
    frame.render_widget(fill(theme.background), frame.area());

    let Some(layout) = DashboardLayout::new(frame.area()) else {
        widgets::overview::draw_minimum_size_message(frame, theme);
        return;
    };

    for area in layout.page.iter().copied() {
        frame.render_widget(fill(theme.background), area);
    }

    widgets::overview::draw(
        frame,
        layout.page[0],
        info,
        app.cpu_usage(),
        app.memory_usage_percent(),
        theme,
        app.is_focused(FocusTarget::Overview),
    );

    for area in layout.main.iter().copied() {
        frame.render_widget(fill(theme.background), area);
    }

    for area in layout.top.iter().copied() {
        frame.render_widget(fill(theme.surface), area);
    }

    for area in layout.middle.iter().copied() {
        frame.render_widget(fill(theme.surface), area);
    }

    for area in layout.middle_left.iter().copied() {
        frame.render_widget(fill(theme.surface), area);
    }

    for area in layout.system.iter().copied() {
        frame.render_widget(fill(theme.surface), area);
    }

    widgets::system::draw_info(
        frame,
        layout.system[0],
        info,
        theme,
        app.is_focused(FocusTarget::System),
    );
    widgets::cpu::draw(
        frame,
        layout.system[1],
        app.cpu_usage(),
        theme,
        app.is_focused(FocusTarget::Cpu),
    );
    widgets::gpu::draw(
        frame,
        layout.top[1],
        info,
        theme,
        app.is_focused(FocusTarget::Gpu),
    );
    widgets::memory::draw(
        frame,
        layout.middle_left[0],
        info,
        app.memory_usage_percent(),
        theme,
        app.is_focused(FocusTarget::Memory),
    );
    widgets::processes::draw(
        frame,
        layout.middle[1],
        info,
        theme,
        app.is_focused(FocusTarget::Processes),
    );

    for area in layout.bottom.iter().copied() {
        frame.render_widget(fill(theme.surface), area);
    }

    widgets::storage::draw(
        frame,
        layout.middle_left[1],
        info,
        theme,
        app.is_focused(FocusTarget::Storage),
    );
    widgets::history::draw(
        frame,
        layout.bottom[0],
        app,
        theme,
        app.is_focused(FocusTarget::History),
    );
    widgets::network::draw(
        frame,
        layout.bottom[1],
        info,
        theme,
        app.is_focused(FocusTarget::Network),
    );
    widgets::timeline::draw(
        frame,
        layout.bottom[2],
        app.timeline(),
        theme,
        app.is_focused(FocusTarget::Timeline),
    );
    widgets::footer::draw(frame, layout.page[2], theme);
}

#[cfg(test)]
mod tests {
    use ratatui::{Terminal, backend::TestBackend};

    use super::draw_app;
    use crate::{
        action::Action,
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
        app.update(info);

        for (width, height) in [(80, 24), (120, 40)] {
            let backend = TestBackend::new(width, height);
            let mut terminal = Terminal::new(backend).expect("test terminal should be created");

            terminal
                .draw(|frame| draw_app(frame, &app))
                .expect("dashboard should render");

            let rendered = terminal
                .backend()
                .buffer()
                .content
                .iter()
                .map(|cell| cell.symbol())
                .collect::<String>();
            assert!(rendered.contains("SYSMON"));
            assert!(rendered.contains("CPU Trend"));
            assert!(rendered.contains("Timeline"));
        }
    }

    #[test]
    fn renders_the_command_palette_overlay_with_documented_commands() {
        let mut app = AppState::new();
        app.update(sample_system_info());
        app.dispatch(Action::OpenPalette);

        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).expect("test terminal should be created");
        terminal
            .draw(|frame| draw_app(frame, &app))
            .expect("command palette should render");

        let rendered = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(rendered.contains("Command Palette"));
        assert!(rendered.contains("Toggle GPU"));
        assert!(rendered.contains("Open Config"));
    }

    #[test]
    fn explains_the_minimum_size_in_small_terminals() {
        let backend = TestBackend::new(60, 20);
        let mut terminal = Terminal::new(backend).expect("test terminal should be created");

        let mut app = AppState::new();
        app.update(sample_system_info());

        terminal
            .draw(|frame| draw_app(frame, &app))
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
