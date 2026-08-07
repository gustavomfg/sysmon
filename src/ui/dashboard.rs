use crate::app::AppState;
use crate::focus::FocusTarget;
use crate::theme::Theme;
use crate::widget::WidgetId;

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
    if app.process_search().is_open() {
        widgets::process_search::draw(frame, frame.area(), app, app.theme());
    }
    if app.config().is_open() {
        widgets::config::draw(frame, frame.area(), app, app.theme());
    }
}

fn draw_with_theme(
    frame: &mut Frame,
    info: &crate::system::info::SystemInfo,
    app: &AppState,
    theme: &Theme,
) {
    frame.render_widget(fill(theme.background), frame.area());

    let Some(layout) = DashboardLayout::new(frame.area(), app.widget_visibility()) else {
        widgets::overview::draw_minimum_size_message(frame, theme);
        return;
    };

    widgets::overview::draw(
        frame,
        layout.header,
        info,
        app.cpu_usage(),
        app.memory_usage_percent(),
        theme,
        app.is_focused(FocusTarget::Overview),
    );

    widgets::pulse::draw(frame, layout.pulse, app, theme, false);

    widgets::system::draw_info(
        frame,
        layout.system,
        info,
        theme,
        app.is_focused(FocusTarget::System),
    );
    widgets::cpu::draw(
        frame,
        layout.cpu,
        app.cpu_usage(),
        theme,
        app.is_focused(FocusTarget::Cpu),
    );
    if app.widget_visible(WidgetId::Gpu)
        && let Some(area) = layout.gpu
    {
        widgets::gpu::draw(frame, area, info, theme, app.is_focused(FocusTarget::Gpu));
    }
    if app.widget_visible(WidgetId::Memory)
        && let Some(area) = layout.memory
    {
        widgets::memory::draw(
            frame,
            area,
            info,
            app.memory_usage_percent(),
            theme,
            app.is_focused(FocusTarget::Memory),
        );
    }
    widgets::processes::draw(
        frame,
        layout.processes,
        info,
        theme,
        app.is_focused(FocusTarget::Processes),
        app.selected_process(),
    );

    widgets::storage::draw(
        frame,
        layout.storage,
        info,
        theme,
        app.is_focused(FocusTarget::Storage),
    );
    widgets::history::draw(
        frame,
        layout.history,
        app,
        theme,
        app.is_focused(FocusTarget::History),
    );
    if app.widget_visible(WidgetId::Network)
        && let Some(area) = layout.network
    {
        widgets::network::draw(
            frame,
            area,
            info,
            theme,
            app.is_focused(FocusTarget::Network),
        );
    }
    widgets::timeline::draw(
        frame,
        layout.timeline,
        app.timeline(),
        theme,
        app.is_focused(FocusTarget::Timeline),
    );
    widgets::footer::draw(frame, layout.footer, theme, app.refresh_interval());
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

        for (width, height) in [
            (80, 24),
            (90, 30),
            (100, 30),
            (120, 32),
            (120, 40),
            (140, 40),
            (160, 40),
            (180, 50),
        ] {
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
            assert!(rendered.contains("System Pulse"));
            assert!(rendered.contains("CPU Trend"));
            assert!(rendered.contains("Timeline"));
        }
    }

    #[test]
    fn renders_the_command_palette_overlay_with_documented_commands() {
        let mut app = AppState::new();
        app.update(sample_system_info());
        app.dispatch(Action::OpenPalette);

        let backend = TestBackend::new(160, 40);
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
        assert!(rendered.contains("Quit"));
        assert!(rendered.contains("Toggle GPU Widget"));
        assert!(rendered.contains("Open Settings"));
        assert!(rendered.contains("Toggle Memory Widget"));
    }

    #[test]
    fn renders_process_and_config_overlays_on_widescreen() {
        for (query, expected) in [("search", "Search Process"), ("settings", "Settings")] {
            let mut app = AppState::new();
            app.update(sample_system_info());
            open_command(&mut app, query);

            let backend = TestBackend::new(160, 40);
            let mut terminal = Terminal::new(backend).expect("test terminal should be created");
            terminal
                .draw(|frame| draw_app(frame, &app))
                .expect("overlay should render on widescreen");

            let rendered = terminal
                .backend()
                .buffer()
                .content
                .iter()
                .map(|cell| cell.symbol())
                .collect::<String>();
            assert!(rendered.contains(expected));
        }
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
            cpu_physical_cores: None,
            cpu_logical_threads: 8,
            cpu_usage: 42.0,
            cpu_frequency_hz: None,
            cpu_temperature_celsius: None,
            total_ram: 16.0,
            used_ram: 8.0,
            total_swap: 4.0,
            used_swap: 1.0,
            processes: 100,
            uptime: 90_061,
            load_average: (0.5, 0.4, 0.3),
            disks: Vec::new(),
            storage_devices: Vec::new(),
            processes_list: Vec::new(),
            gpus: Vec::new(),
            networks: Vec::new(),
            all_networks: Vec::new(),
            temperatures: Vec::new(),
        }
    }

    fn open_command(app: &mut AppState, query: &str) {
        app.dispatch(Action::OpenPalette);
        for character in query.chars() {
            app.dispatch(Action::PaletteCharacter(character));
        }
        app.dispatch(Action::PaletteSubmit);
    }
}
