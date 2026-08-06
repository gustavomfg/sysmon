use std::{
    cmp::Ordering,
    time::{Duration, Instant},
};

use crate::system::gpu::{GpuCollector, GpuInfo};
use crate::system::network::NetworkInfo;
use sysinfo::{
    Components, CpuRefreshKind, Disks, MemoryRefreshKind, Networks, ProcessRefreshKind,
    RefreshKind, System,
};

const DISK_LIST_REFRESH_INTERVAL: Duration = Duration::from_secs(30);
const GPU_REFRESH_INTERVAL: Duration = Duration::from_secs(5);

pub struct SystemInfoCollector {
    system: System,
    networks: Networks,
    disks: Disks,
    gpus: GpuCollector,
    components: Components,
    last_disk_list_refresh: Instant,
    last_disk_io_sample: Option<Instant>,
    last_network_refresh: Option<Instant>,
}

pub struct DiskInfo {
    pub name: String,
    pub mount_point: String,
    pub total_space: f64,
    pub available_space: f64,
    #[allow(dead_code)]
    pub read_bytes_per_sec: Option<f64>,
    #[allow(dead_code)]
    pub written_bytes_per_sec: Option<f64>,
}

pub struct ProcessInfo {
    pub name: String,
    pub memory: f64,
    pub cpu_usage: f32,
}

pub struct SystemInfo {
    pub os_name: String,
    pub kernel: String,
    pub cpu_name: String,
    pub cpu_cores: usize,
    pub cpu_usage: f32,
    #[allow(dead_code)]
    pub cpu_frequency_hz: Option<f64>,
    #[allow(dead_code)]
    pub cpu_temperature_celsius: Option<f64>,
    pub total_ram: f64,
    pub used_ram: f64,
    pub total_swap: f64,
    pub used_swap: f64,
    pub processes: usize,
    pub uptime: u64,
    pub load_average: (f64, f64, f64),
    pub disks: Vec<DiskInfo>,
    pub processes_list: Vec<ProcessInfo>,
    pub gpus: Vec<GpuInfo>,
    pub networks: Vec<NetworkInfo>,
}

impl SystemInfoCollector {
    pub fn new() -> Self {
        Self {
            system: System::new_with_specifics(system_refresh_kind()),
            networks: Networks::new_with_refreshed_list(),
            disks: Disks::new_with_refreshed_list(),
            gpus: GpuCollector::new(GPU_REFRESH_INTERVAL),
            components: Components::new_with_refreshed_list(),
            last_disk_list_refresh: Instant::now(),
            last_disk_io_sample: None,
            last_network_refresh: None,
        }
    }

    pub fn collect(&mut self, collected_at: Instant) -> SystemInfo {
        let network_interval = self
            .last_network_refresh
            .map_or(Duration::ZERO, |previous| {
                collected_at.saturating_duration_since(previous)
            });
        self.networks.refresh(true);
        self.last_network_refresh = Some(collected_at);

        self.system.refresh_specifics(system_refresh_kind());
        self.components.refresh(false);

        if self.last_disk_list_refresh.elapsed() >= DISK_LIST_REFRESH_INTERVAL {
            self.disks.refresh(true);
            self.last_disk_list_refresh = std::time::Instant::now();
        } else {
            for disk in self.disks.list_mut() {
                disk.refresh();
            }
        }

        let disk_interval = self
            .last_disk_io_sample
            .map(|previous| collected_at.saturating_duration_since(previous));
        let disk_infos = self
            .disks
            .iter()
            .map(|disk| {
                let usage = disk.usage();
                DiskInfo {
                    name: disk.name().to_string_lossy().to_string(),
                    mount_point: disk.mount_point().to_string_lossy().to_string(),
                    total_space: disk.total_space() as f64 / 1024.0 / 1024.0 / 1024.0,
                    available_space: disk.available_space() as f64 / 1024.0 / 1024.0 / 1024.0,
                    read_bytes_per_sec: bytes_per_second(usage.read_bytes, disk_interval),
                    written_bytes_per_sec: bytes_per_second(usage.written_bytes, disk_interval),
                }
            })
            .collect();
        self.last_disk_io_sample = Some(collected_at);

        let mut processes_list = self
            .system
            .processes()
            .values()
            .map(|process| ProcessInfo {
                name: process.name().to_string_lossy().to_string(),
                memory: process.memory() as f64 / 1024.0 / 1024.0,
                cpu_usage: process.cpu_usage(),
            })
            .collect::<Vec<ProcessInfo>>();

        processes_list.sort_by(|a, b| b.memory.partial_cmp(&a.memory).unwrap_or(Ordering::Equal));

        let load_average = System::load_average();

        let cpu_frequency_hz = average_cpu_frequency_hz(&self.system);
        let cpu_temperature_celsius = cpu_temperature_celsius(&self.components);

        SystemInfo {
            os_name: System::name().unwrap_or("Desconhecido".to_string()),
            kernel: System::kernel_version().unwrap_or("Desconhecido".to_string()),
            cpu_name: self
                .system
                .cpus()
                .first()
                .map(|cpu| cpu.brand().to_string())
                .unwrap_or("Desconhecido".to_string()),
            cpu_cores: self.system.cpus().len(),
            cpu_usage: self.system.global_cpu_usage(),
            cpu_frequency_hz,
            cpu_temperature_celsius,
            total_ram: self.system.total_memory() as f64 / 1024.0 / 1024.0 / 1024.0,
            used_ram: self.system.used_memory() as f64 / 1024.0 / 1024.0 / 1024.0,
            total_swap: self.system.total_swap() as f64 / 1024.0 / 1024.0 / 1024.0,
            used_swap: self.system.used_swap() as f64 / 1024.0 / 1024.0 / 1024.0,
            processes: self.system.processes().len(),
            uptime: System::uptime(),
            load_average: (load_average.one, load_average.five, load_average.fifteen),
            disks: disk_infos,
            processes_list,
            gpus: self.gpus.get().to_vec(),
            networks: NetworkInfo::get(&self.networks, network_interval),
        }
    }
}

fn system_refresh_kind() -> RefreshKind {
    RefreshKind::nothing()
        .with_cpu(CpuRefreshKind::nothing().with_cpu_usage().with_frequency())
        .with_memory(MemoryRefreshKind::everything())
        .with_processes(ProcessRefreshKind::nothing().with_cpu().with_memory())
}

fn average_cpu_frequency_hz(system: &System) -> Option<f64> {
    let mut total_mhz = 0.0;
    let mut count = 0usize;
    for cpu in system.cpus() {
        let frequency = cpu.frequency();
        if frequency > 0 {
            total_mhz += frequency as f64;
            count += 1;
        }
    }
    (count > 0).then_some(total_mhz / count as f64 * 1.0e6)
}

fn cpu_temperature_celsius(components: &Components) -> Option<f64> {
    components
        .iter()
        .filter(|component| {
            let label = component.label().to_ascii_lowercase();
            label.contains("cpu") || label.contains("package") || label.contains("core")
        })
        .filter_map(|component| component.temperature().map(f64::from))
        .filter(|temperature| temperature.is_finite())
        .max_by(f64::total_cmp)
}

fn bytes_per_second(bytes: u64, interval: Option<Duration>) -> Option<f64> {
    let seconds = interval?.as_secs_f64();
    (seconds > 0.0).then_some(bytes as f64 / seconds)
}
