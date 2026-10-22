//! Offline visual fixture. All telemetry in this example is synthetic.
//! Usage: cargo run --release --example graphics_capture -- OUTPUT_DIR [--bench] [--animate] [--status=normal|warning|critical]
//! It drives the real app, focus/expansion controls and ratatui TestBackend.
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    Terminal,
    backend::TestBackend,
    style::{Color, Modifier},
};
use std::{
    fmt::Write,
    fs,
    time::{Duration, Instant},
};
use sysmon::{
    SystemInfo,
    system::{
        gpu::GpuInfo,
        info::{DiskInfo, ProcessInfo, TemperatureInfo},
        network::NetworkInfo,
    },
    ui::dashboard::DashboardApp,
};
fn snapshot(i: usize, status: &str) -> SystemInfo {
    let t = i as f64;
    let cpu = 22.0 + 15.0 * (t * 0.31).sin() + if i.is_multiple_of(19) { 39.0 } else { 0.0 };
    let net = NetworkInfo {
        id: "enp4s0".into(),
        interface: "enp4s0".into(),
        received_per_sec: (1.3 + (t * 0.25).sin()).max(0.0),
        transmitted_per_sec: (0.6 + (t * 0.19).sin() * 0.5).max(0.0),
        sample_interval: Some(Duration::from_secs(2)),
    };
    let disk = DiskInfo {
        id: "nvme0n1".into(),
        name: "nvme0n1".into(),
        model: Some("Samsung 980 PRO".into()),
        mount_point: "/".into(),
        mount_points: vec!["/".into()],
        total_space: 1000.0,
        available_space: 580.0,
        is_virtual: false,
        read_bytes_per_sec: Some(10_000_000.0),
        written_bytes_per_sec: Some(1_000_000.0),
    };
    let mut info = SystemInfo {
        host_name: "workstation".into(),
        os_name: "Linux".into(),
        kernel: "6.18".into(),
        cpu_name: "AMD Ryzen 7 5800X".into(),
        cpu_cores: 16,
        cpu_physical_cores: Some(8),
        cpu_logical_threads: 16,
        cpu_usage: cpu as f32,
        cpu_frequency_hz: Some(3_800_000_000.0),
        cpu_temperature_celsius: Some(54.0),
        cpu_core_usages: (0..16)
            .map(|j| ((cpu + (j as f64 * 0.67).sin() * 22.0).clamp(0.0, 100.0)) as f32)
            .collect(),
        total_ram: 32.0,
        used_ram: 14.0 + (t * 0.1).sin() * 0.25,
        available_ram: 18.0,
        total_swap: 8.0,
        used_swap: 0.0,
        processes: 220,
        uptime: 15327,
        load_average: (1.25, 1.4, 0.96),
        disks: vec![disk.clone()],
        storage_devices: vec![disk],
        processes_list: vec![
            ProcessInfo {
                pid: 9812,
                name: "rust-analyzer".into(),
                memory: 1200.0,
                cpu_usage: 28.5,
            },
            ProcessInfo {
                pid: 1264,
                name: "firefox".into(),
                memory: 820.0,
                cpu_usage: 11.2,
            },
        ],
        gpus: vec![GpuInfo {
            id: "gpu0".into(),
            name: "RTX 4060".into(),
            temperature: "48 C".into(),
            usage: "32%".into(),
            memory: "2 / 8 GB".into(),
            usage_percent: Some(32.0 + (t * 0.25).sin() * 13.0),
            vram_used_bytes: Some(2e9),
            vram_total_bytes: Some(8e9),
            temperature_celsius: Some(48.0),
            frequency_hz: Some(1.8e9),
            power_watts: Some(35.0),
        }],
        networks: vec![net.clone()],
        all_networks: vec![net],
        temperatures: vec![TemperatureInfo {
            id: "cpu".into(),
            label: "CPU package".into(),
            celsius: Some(54.0),
            critical_celsius: Some(95.0),
        }],
    };
    // Explicit synthetic pressure fixtures for palette and state review.
    if status != "normal" {
        let load = if status == "warning" { 0.88 } else { 0.98 };
        info.cpu_usage = (load * 100.0) as f32;
        info.cpu_core_usages.fill(info.cpu_usage);
        info.used_ram = info.total_ram * load;
        info.available_ram = info.total_ram - info.used_ram;
        info.load_average = (16.0 * load, 16.0 * load, 16.0 * load);
        let temperature = if status == "warning" { 86.0 } else { 98.0 };
        info.cpu_temperature_celsius = Some(temperature);
        info.temperatures[0].celsius = Some(temperature);
        for gpu in &mut info.gpus {
            gpu.usage_percent = Some(load * 100.0);
            gpu.vram_used_bytes = gpu.vram_total_bytes.map(|total| total * load);
        }
        for disk in info.disks.iter_mut().chain(&mut info.storage_devices) {
            disk.available_space = disk.total_space * (1.0 - load);
        }
    }
    info
}
fn key(app: &mut DashboardApp, code: KeyCode, modifiers: KeyModifiers) {
    app.handle_event(Event::Key(KeyEvent::new(code, modifiers)));
}
fn color(c: Color) -> String {
    match c {
        Color::Rgb(r, g, b) => format!("{r},{g},{b}"),
        _ => "217,226,232".into(),
    }
}
fn main() {
    let folder = std::env::args()
        .nth(1)
        .expect("provide a capture directory");
    fs::create_dir_all(&folder).unwrap();
    let benchmark = std::env::args().any(|arg| arg == "--bench");
    let animate = std::env::args().any(|arg| arg == "--animate");
    let status = std::env::args()
        .find_map(|arg| arg.strip_prefix("--status=").map(str::to_owned))
        .unwrap_or_else(|| "normal".to_owned());
    assert!(
        matches!(status.as_str(), "normal" | "warning" | "critical"),
        "unknown fixture status"
    );
    let cases = [
        (80, 24, 0, 0),
        (100, 30, 0, 0),
        (120, 35, 0, 0),
        (160, 45, 0, 0),
        (200, 55, 0, 0),
        (120, 35, 1, 0),
        (120, 35, 2, 0),
        (160, 45, 0, 3),
        (160, 45, 0, 4),
        (160, 45, 0, 5),
        (160, 45, 0, 10),
        (160, 45, 0, 7),
    ];
    println!("case,width,height,frames,ns_per_frame");
    for (w, h, theme, tabs) in cases {
        let mut app = DashboardApp::new();
        let start = Instant::now() - Duration::from_secs(240);
        for i in 0..120 {
            app.update(
                snapshot(i, &status),
                start + Duration::from_secs(2 * i as u64),
            );
            app.update_animations(Duration::from_secs(2));
        }
        for _ in 0..theme {
            key(&mut app, KeyCode::Char('p'), KeyModifiers::CONTROL);
            for ch in "theme".chars() {
                key(&mut app, KeyCode::Char(ch), KeyModifiers::NONE);
            }
            key(&mut app, KeyCode::Enter, KeyModifiers::NONE);
        }
        if tabs > 0 {
            for _ in 0..tabs {
                key(&mut app, KeyCode::Tab, KeyModifiers::NONE);
            }
            key(&mut app, KeyCode::Char('e'), KeyModifiers::NONE);
        }
        let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
        terminal.draw(|frame| app.draw(frame)).unwrap();
        let mut data = format!("{w} {h}\n");
        for cell in &terminal.backend().buffer().content {
            writeln!(
                data,
                "{}|{}|{}|{}",
                cell.symbol()
                    .chars()
                    .map(|ch| format!("{:x}", ch as u32))
                    .collect::<Vec<_>>()
                    .join(","),
                color(cell.fg),
                color(cell.bg),
                u8::from(cell.modifier.contains(Modifier::BOLD))
            )
            .unwrap();
        }
        let name = format!("{w}x{h}-theme{theme}-focus{tabs}");
        fs::write(format!("{folder}/{name}.cells"), data).unwrap();
        let mut plain = String::new();
        for row in terminal.backend().buffer().content.chunks(w as usize) {
            for cell in row {
                plain.push_str(cell.symbol());
            }
            plain.push('\n');
        }
        fs::write(format!("{folder}/{name}.txt"), plain).unwrap();
        if benchmark {
            for _ in 0..10 {
                if animate {
                    app.update_animations(Duration::from_millis(50));
                }
                terminal.draw(|frame| app.draw(frame)).unwrap();
            }
            let begin = Instant::now();
            let frames = 100;
            for _ in 0..frames {
                if animate {
                    app.update_animations(Duration::from_millis(50));
                }
                terminal.draw(|frame| app.draw(frame)).unwrap();
            }
            println!(
                "{name},{w},{h},{frames},{}",
                begin.elapsed().as_nanos() / frames
            );
        }
    }
}
