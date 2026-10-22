//! Check known telemetry through the actual app and focus/expansion controls.
#[path = "../examples/support/telemetry.rs"]
mod telemetry;

use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::{Terminal, backend::TestBackend};
use std::time::{Duration, Instant};
use sysmon::{
    SystemInfo,
    dashboard_v2::{LiveMetricsHistory, LiveSnapshot},
    ui::dashboard::DashboardApp,
};

fn fixture() -> SystemInfo {
    let mut info = telemetry::snapshot(0, "normal");
    info.cpu_usage = 37.0;
    info.used_ram = 24.0;
    info.available_ram = 8.0;
    info.used_swap = 2.0;
    info.gpus[0].usage_percent = Some(41.0);
    info.gpus[0].vram_used_bytes = Some(2.0 * 1024_f64.powi(3));
    info.gpus[0].vram_total_bytes = Some(8.0 * 1024_f64.powi(3));
    let mut second = info.gpus[0].clone();
    second.id = "gpu1".into();
    second.name = "AMD Audit GPU".into();
    second.usage_percent = Some(7.0);
    second.power_watts = Some(6.5);
    info.gpus.push(second);
    info.all_networks[0].received_per_sec = 1.0;
    info.all_networks[0].transmitted_per_sec = 0.25;
    let mut second = info.all_networks[0].clone();
    second.id = "eth1".into();
    second.interface = "eth1".into();
    second.received_per_sec = 2.0;
    second.transmitted_per_sec = 0.75;
    info.all_networks.push(second.clone());
    second.id = "lo".into();
    second.interface = "lo".into();
    second.received_per_sec = 999.0;
    info.all_networks.push(second);
    info.networks = info.all_networks.clone();
    let mut second = info.storage_devices[0].clone();
    second.id = "sdb".into();
    second.name = "sdb".into();
    second.total_space = 128.0;
    second.available_space = 32.0;
    info.storage_devices.push(second);
    info
}

#[inline(never)]
fn app_with_fixture(start: Instant) -> Box<DashboardApp> {
    let mut app = Box::new(DashboardApp::new());
    app.update(fixture(), start);
    app
}

fn key(app: &mut DashboardApp, code: KeyCode) {
    app.handle_event(Event::Key(KeyEvent::new(code, KeyModifiers::NONE)));
}

fn draw(app: &mut DashboardApp, width: u16, height: u16) -> String {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|frame| app.draw(frame)).unwrap();
    terminal
        .backend()
        .buffer()
        .content
        .chunks(usize::from(width))
        .map(|row| row.iter().map(|cell| cell.symbol()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn current_values_remain_correct_when_focused_expanded_and_updated() {
    let start = Instant::now();
    // Tab order is the real app navigation, including non-metric panels.
    for (tabs, expected) in [
        (2, vec!["workstation", "32.0", "220"]),
        (3, vec!["37%", "3.8 GHz", "54.0°C"]),
        (4, vec!["24.0 GiB", "32.0 GiB", "75%", "2.0 GiB"]),
        (5, vec!["41%", "2.0 / 8 GiB", "48°C"]),
        (6, vec!["9812", "rust-analyzer", "28.5%", "1.2 GiB"]),
        (7, vec!["nvme0n1", "sdb", "96.0 GiB", "75%"]),
        (8, vec!["CPU package", "54°C"]),
        (10, vec!["2.9 MiB/s", "976.6 KiB/s"]),
    ] {
        for (width, height) in [(80, 24), (100, 30), (120, 35), (160, 45), (200, 55)] {
            let mut app = app_with_fixture(start);
            for _ in 0..tabs {
                key(&mut app, KeyCode::Tab);
            }
            // Merely focusing must not change existing numeric readings.
            let focused = draw(&mut app, 200, 55);
            assert!(!focused.contains("NaN"));
            key(&mut app, KeyCode::Char('e'));
            let expanded = draw(&mut app, width, height);
            for token in &expected {
                assert!(
                    expanded.contains(token),
                    "tabs={tabs}, {width}x{height}, missing {token}\n{expanded}"
                );
                assert!(
                    focused.contains(token),
                    "focused tabs={tabs}, missing {token}\n{focused}"
                );
            }
            if tabs == 8 {
                assert!(expanded.contains("FOCUS TEMPERATURES"));
            }
            if tabs == 5 {
                assert!(expanded.contains("AMD Audit GPU"));
            }
            if tabs == 10 {
                for name in ["enp4s0", "eth1", "lo"] {
                    assert!(expanded.contains(name));
                }
            }
            let mut changed = fixture();
            changed.cpu_usage = 63.0;
            changed.used_ram = 16.0;
            changed.available_ram = 16.0;
            changed.gpus[0].usage_percent = Some(52.0);
            app.update(changed, start + Duration::from_secs(2));
            let updated = draw(&mut app, width, height);
            match tabs {
                3 => assert!(updated.contains("63%")),
                4 => assert!(updated.contains("50%")),
                5 => assert!(updated.contains("52%")),
                _ => {}
            }
            key(&mut app, KeyCode::Esc);
            assert!(!draw(&mut app, width, height).contains("NaN"));
        }
    }
}

#[test]
fn invalid_gpu_capacity_does_not_freeze_other_metrics_or_invent_zeroes() {
    let mut info = fixture();
    info.gpus[0].vram_used_bytes = Some(99.0 * 1024_f64.powi(3));
    info.gpus[0].temperature_celsius = Some(-5.0);
    info.cpu_core_usages[0] = f32::NAN;
    info.storage_devices[0].total_space = 0.0;
    info.storage_devices[0].available_space = 0.0;
    info.processes_list[0].cpu_usage = f32::NAN;
    let mut app = app_with_fixture(Instant::now());
    app.update(info.clone(), Instant::now() + Duration::from_secs(2));
    for _ in 0..6 {
        key(&mut app, KeyCode::Tab);
    }
    key(&mut app, KeyCode::Char('e'));
    let processes = draw(&mut app, 120, 35);
    assert!(!processes.contains("NaN"));
    assert!(processes.contains("rust-analyzer"));
    let live = LiveSnapshot::from_system_info(&info);
    assert_eq!(live.hardware.gpus[0].vram_used_bytes, None);
    assert_eq!(live.hardware.gpus[0].temperature_celsius, Some(-5.0));
    assert!(live.hardware.cpu_cores[0].is_nan());
    assert_eq!(live.download_bytes_per_sec, Some(3_000_000.0));
    assert_eq!(live.upload_bytes_per_sec, Some(1_000_000.0));
    let mut history = LiveMetricsHistory::new();
    history.push_snapshot(Instant::now(), &live).unwrap();
    let models = history.models().unwrap();
    assert_eq!(models.cpu().visual().primary().unwrap().value(), 0.37);
    assert_eq!(models.memory().visual().primary().unwrap().value(), 0.75);
}
