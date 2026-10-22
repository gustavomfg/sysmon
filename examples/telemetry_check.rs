//! Bounded Linux audit against independent /proc and /sys counters.
//! Does not export snapshots, process names, hostnames or screen contents.
use std::{
    collections::BTreeMap,
    fs, thread,
    time::{Duration, Instant},
};
use sysmon::SystemInfoCollector;

fn cpu() -> (u64, u64) {
    let text = fs::read_to_string("/proc/stat").unwrap();
    let values = text
        .lines()
        .next()
        .unwrap()
        .split_whitespace()
        .skip(1)
        .take(8)
        .map(|value| value.parse::<u64>().unwrap())
        .collect::<Vec<_>>();
    (values.iter().sum(), values[3] + values[4])
}

fn memory() -> BTreeMap<String, u64> {
    fs::read_to_string("/proc/meminfo")
        .unwrap()
        .lines()
        .filter_map(|line| {
            let (name, value) = line.split_once(':')?;
            let value = value.split_whitespace().next()?.parse::<u64>().ok()?;
            Some((name.to_owned(), value * 1024))
        })
        .collect()
}

fn network() -> BTreeMap<String, (u64, u64)> {
    fs::read_to_string("/proc/net/dev")
        .unwrap()
        .lines()
        .filter_map(|line| {
            let (name, values) = line.split_once(':')?;
            let values = values.split_whitespace().collect::<Vec<_>>();
            Some((
                name.trim().to_owned(),
                (values.first()?.parse().ok()?, values.get(8)?.parse().ok()?),
            ))
        })
        .collect()
}

fn disks() -> BTreeMap<String, (u64, u64)> {
    fs::read_to_string("/proc/diskstats")
        .unwrap()
        .lines()
        .filter_map(|line| {
            let values = line.split_whitespace().collect::<Vec<_>>();
            Some((
                values.get(2)?.to_string(),
                (
                    values.get(5)?.parse::<u64>().ok()? * 512,
                    values.get(9)?.parse::<u64>().ok()? * 512,
                ),
            ))
        })
        .collect()
}

fn filesystem_capacity(mount: &str) -> Option<(f64, f64)> {
    let output = std::process::Command::new("df")
        .args(["-B1", "--output=size,avail", "--", mount])
        .env("LC_ALL", "C")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8(output.stdout).ok()?;
    let fields = text.lines().nth(1)?.split_whitespace().collect::<Vec<_>>();
    Some((fields.first()?.parse().ok()?, fields.get(1)?.parse().ok()?))
}

fn main() {
    if !cfg!(target_os = "linux") {
        eprintln!("this audit requires Linux procfs/sysfs");
        std::process::exit(2);
    }
    let mut collector = SystemInfoCollector::new();
    let mut previous_cpu = cpu();
    let mut previous_net = network();
    let mut previous_disk = disks();
    let mut previous_time = Instant::now();
    let initial = collector.collect(previous_time);
    assert!(
        initial.cpu_usage.is_nan(),
        "startup CPU must await an interval"
    );
    println!("startup: CPU and I/O unavailable until a second sample");
    for sample in 1..=3 {
        thread::sleep(Duration::from_millis(2100));
        let current_cpu = cpu();
        let current_net = network();
        let current_disk = disks();
        let now = Instant::now();
        let info = collector.collect(now);
        let elapsed = now.duration_since(previous_time).as_secs_f64();
        let ticks = current_cpu.0.saturating_sub(previous_cpu.0);
        let idle = current_cpu.1.saturating_sub(previous_cpu.1);
        let reference_cpu = 100.0 * ticks.saturating_sub(idle) as f64 / ticks.max(1) as f64;
        assert!(
            (f64::from(info.cpu_usage) - reference_cpu).abs() < 5.0,
            "CPU disagreement"
        );
        let ram = memory();
        let gib = 1024_f64.powi(3);
        let ram_error = (info.available_ram * gib - ram["MemAvailable"] as f64).abs();
        assert_eq!(info.total_ram * gib, ram["MemTotal"] as f64);
        assert!(ram_error < ram["MemTotal"] as f64 * 0.03);
        assert_eq!(info.total_swap * gib, ram["SwapTotal"] as f64);
        let swap_error = (info.used_swap * gib - (ram["SwapTotal"] - ram["SwapFree"]) as f64).abs();
        assert!(swap_error < 64.0 * 1024_f64.powi(2));
        assert!((info.used_ram + info.available_ram - info.total_ram).abs() < 1.0e-6);
        assert_eq!(info.processes, info.processes_list.len());
        assert_eq!(info.cpu_logical_threads, info.cpu_core_usages.len());
        assert!(
            info.cpu_core_usages
                .iter()
                .all(|value| value.is_finite() && (0.0..=100.0).contains(value))
        );
        let uptime: f64 = fs::read_to_string("/proc/uptime")
            .unwrap()
            .split_whitespace()
            .next()
            .unwrap()
            .parse()
            .unwrap();
        assert!((info.uptime as f64 - uptime).abs() < 2.0);
        let mut max_net_error = 0.0_f64;
        for interface in &info.all_networks {
            if let (Some(old), Some(current), Some(_)) = (
                previous_net.get(&interface.id),
                current_net.get(&interface.id),
                interface.sample_interval,
            ) {
                for (value, reference) in [
                    (
                        interface.received_per_sec * 1e6,
                        current.0.saturating_sub(old.0) as f64 / elapsed,
                    ),
                    (
                        interface.transmitted_per_sec * 1e6,
                        current.1.saturating_sub(old.1) as f64 / elapsed,
                    ),
                ] {
                    max_net_error = max_net_error.max((value - reference).abs());
                    assert!(
                        (value - reference).abs() <= reference * 0.10 + 65536.0,
                        "network rate disagreement"
                    );
                }
            }
        }
        let mut io_checked = 0;
        for disk in &info.storage_devices {
            if let (Some(old), Some(current)) =
                (previous_disk.get(&disk.id), current_disk.get(&disk.id))
            {
                for (value, reference) in [
                    (
                        disk.read_bytes_per_sec,
                        current.0.saturating_sub(old.0) as f64 / elapsed,
                    ),
                    (
                        disk.written_bytes_per_sec,
                        current.1.saturating_sub(old.1) as f64 / elapsed,
                    ),
                ] {
                    if let Some(value) = value {
                        assert!(
                            (value - reference).abs() <= reference * 0.10 + 1024_f64.powi(2),
                            "disk rate disagreement"
                        );
                        io_checked += 1;
                    }
                }
            }
            assert!(disk.available_space <= disk.total_space);
        }
        let mut capacity_checked = 0;
        for disk in &info.disks {
            if let Some((total, available)) = filesystem_capacity(&disk.mount_point) {
                assert!((disk.total_space * gib - total).abs() < 1.0);
                assert!((disk.available_space * gib - available).abs() < 64.0 * 1024_f64.powi(2));
                capacity_checked += 1;
            }
        }
        let frequency_mhz = info.cpu_frequency_hz.map(|hz| hz / 1e6);
        if let Some(mhz) = frequency_mhz {
            assert!((1.0..=20000.0).contains(&mhz));
        }
        println!("capacity_filesystems={capacity_checked} mean_frequency_mhz={frequency_mhz:?}");
        for gpu in &info.gpus {
            if let Some(usage) = gpu.usage_percent {
                assert!((0.0..=100.0).contains(&usage));
                println!(
                    "gpu: usage={usage:.1}% used_mib={:?} total_mib={:?} temp_c={:?} clock_mhz={:?} power_w={:?}",
                    gpu.vram_used_bytes.map(|v| v / 1024_f64.powi(2)),
                    gpu.vram_total_bytes.map(|v| v / 1024_f64.powi(2)),
                    gpu.temperature_celsius,
                    gpu.frequency_hz.map(|v| v / 1e6),
                    gpu.power_watts
                );
            }
        }
        let gpu_metrics = info
            .gpus
            .iter()
            .filter(|gpu| gpu.usage_percent.is_some())
            .count();
        println!(
            "sample {sample}: cpu={:.2}% proc={reference_cpu:.2}% ram_error={:.0}B swap_error={swap_error:.0}B net_error_max={max_net_error:.0}B/s disk_directions={io_checked} gpu_metrics={gpu_metrics}/{} sensors={}",
            info.cpu_usage,
            ram_error,
            info.gpus.len(),
            info.temperatures.len()
        );
        previous_cpu = current_cpu;
        previous_net = current_net;
        previous_disk = current_disk;
        previous_time = now;
    }
}
