//! Shared, explicitly synthetic telemetry for capture and integration checks.
use std::time::Duration;
use sysmon::{
    SystemInfo,
    system::{
        gpu::GpuInfo,
        info::{DiskInfo, ProcessInfo, TemperatureInfo},
        network::NetworkInfo,
    },
};
pub fn snapshot(i: usize, status: &str) -> SystemInfo {
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
