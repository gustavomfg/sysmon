use std::{
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet},
    time::{Duration, Instant},
};

use crate::system::gpu::{GpuCollector, GpuInfo};
use crate::system::network::NetworkInfo;
use sysinfo::{
    Components, CpuRefreshKind, Disks, MemoryRefreshKind, Networks, ProcessRefreshKind,
    RefreshKind, System,
};

const PROCESS_REFRESH_INTERVAL: Duration = Duration::from_secs(2);
const DISK_IO_REFRESH_INTERVAL: Duration = Duration::from_secs(2);
const DISK_INVENTORY_REFRESH_INTERVAL: Duration = Duration::from_secs(30);
const DISK_METADATA_REFRESH_INTERVAL: Duration = Duration::from_secs(5 * 60);
const GPU_REFRESH_INTERVAL: Duration = Duration::from_secs(5);
const TOP_PROCESS_COUNT: usize = 5;

struct DiskModelCacheEntry {
    model: Option<String>,
    refreshed_at: Instant,
}

pub struct SystemInfoCollector {
    system: System,
    networks: Networks,
    disks: Disks,
    gpus: GpuCollector,
    components: Components,
    last_disk_inventory_refresh: Option<Instant>,
    last_disk_io_sample: Option<Instant>,
    last_network_refresh: Option<Instant>,
    last_process_refresh: Option<Instant>,
    cached_disks: Vec<DiskInfo>,
    cached_storage_devices: Vec<DiskInfo>,
    processes_list: Vec<ProcessInfo>,
    disk_models: BTreeMap<String, DiskModelCacheEntry>,
}

#[derive(Debug, Clone)]
pub struct DiskInfo {
    #[allow(dead_code)]
    pub id: String,
    pub name: String,
    #[allow(dead_code)]
    pub model: Option<String>,
    pub mount_point: String,
    #[allow(dead_code)]
    pub mount_points: Vec<String>,
    pub total_space: f64,
    pub available_space: f64,
    #[allow(dead_code)]
    pub is_virtual: bool,
    #[allow(dead_code)]
    pub read_bytes_per_sec: Option<f64>,
    #[allow(dead_code)]
    pub written_bytes_per_sec: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct TemperatureInfo {
    pub id: String,
    pub label: String,
    pub celsius: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub memory: f64,
    pub cpu_usage: f32,
}

#[derive(Debug, Clone)]
pub struct SystemInfo {
    pub host_name: String,
    pub os_name: String,
    pub kernel: String,
    pub cpu_name: String,
    /// Legacy logical CPU count retained for existing consumers.
    pub cpu_cores: usize,
    /// Physical cores when the platform reports them.
    #[allow(dead_code)]
    pub cpu_physical_cores: Option<usize>,
    /// Logical execution threads reported by the collector.
    #[allow(dead_code)]
    pub cpu_logical_threads: usize,
    pub cpu_usage: f32,
    #[allow(dead_code)]
    pub cpu_frequency_hz: Option<f64>,
    #[allow(dead_code)]
    pub cpu_temperature_celsius: Option<f64>,
    pub total_ram: f64,
    pub used_ram: f64,
    pub available_ram: f64,
    pub total_swap: f64,
    pub used_swap: f64,
    pub processes: usize,
    pub uptime: u64,
    pub load_average: (f64, f64, f64),
    /// Legacy disk entries, kept partition-oriented for the existing dashboard.
    pub disks: Vec<DiskInfo>,
    /// Stable physical/logical storage inventory for Dashboard V2.
    #[allow(dead_code)]
    pub storage_devices: Vec<DiskInfo>,
    pub processes_list: Vec<ProcessInfo>,
    pub gpus: Vec<GpuInfo>,
    /// Legacy traffic-oriented list, retained for the existing dashboard.
    pub networks: Vec<NetworkInfo>,
    /// Complete deterministically ordered interface inventory for Dashboard V2.
    #[allow(dead_code)]
    pub all_networks: Vec<NetworkInfo>,
    #[allow(dead_code)]
    pub temperatures: Vec<TemperatureInfo>,
}

impl SystemInfoCollector {
    pub fn new() -> Self {
        Self {
            system: System::new_with_specifics(system_refresh_kind()),
            networks: Networks::new_with_refreshed_list(),
            disks: Disks::new_with_refreshed_list(),
            gpus: GpuCollector::new(GPU_REFRESH_INTERVAL),
            components: Components::new_with_refreshed_list(),
            last_disk_inventory_refresh: None,
            last_disk_io_sample: None,
            last_network_refresh: None,
            last_process_refresh: None,
            cached_disks: Vec::new(),
            cached_storage_devices: Vec::new(),
            processes_list: Vec::new(),
            disk_models: BTreeMap::new(),
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

        let process_refresh_due = refresh_due(
            self.last_process_refresh,
            collected_at,
            PROCESS_REFRESH_INTERVAL,
        );
        let mut refresh_kind = system_refresh_kind();
        if process_refresh_due {
            refresh_kind = refresh_kind.with_processes(process_refresh_kind());
        }
        self.system.refresh_specifics(refresh_kind);
        if process_refresh_due {
            self.refresh_processes(collected_at);
        }
        self.components.refresh(false);

        self.refresh_disks_if_due(collected_at);

        let temperatures = temperature_infos(&self.components);
        let load_average = System::load_average();
        let cpu_frequency_hz = average_cpu_frequency_hz(&self.system);
        let cpu_temperature_celsius = temperatures
            .iter()
            .filter(|sensor| {
                let label = sensor.label.to_ascii_lowercase();
                label.contains("cpu") || label.contains("package") || label.contains("core")
            })
            .filter_map(|sensor| sensor.celsius)
            .max_by(f64::total_cmp);

        SystemInfo {
            host_name: System::host_name().unwrap_or_else(|| "localhost".to_string()),
            os_name: System::name().unwrap_or("Desconhecido".to_string()),
            kernel: System::kernel_version().unwrap_or("Desconhecido".to_string()),
            cpu_name: self
                .system
                .cpus()
                .first()
                .map(|cpu| cpu.brand().to_string())
                .unwrap_or("Desconhecido".to_string()),
            cpu_cores: self.system.cpus().len(),
            cpu_physical_cores: System::physical_core_count(),
            cpu_logical_threads: self.system.cpus().len(),
            cpu_usage: self.system.global_cpu_usage(),
            cpu_frequency_hz,
            cpu_temperature_celsius,
            total_ram: self.system.total_memory() as f64 / 1024.0 / 1024.0 / 1024.0,
            used_ram: self.system.used_memory() as f64 / 1024.0 / 1024.0 / 1024.0,
            available_ram: self.system.available_memory() as f64 / 1024.0 / 1024.0 / 1024.0,
            total_swap: self.system.total_swap() as f64 / 1024.0 / 1024.0 / 1024.0,
            used_swap: self.system.used_swap() as f64 / 1024.0 / 1024.0 / 1024.0,
            processes: self.processes_list.len(),
            uptime: System::uptime(),
            load_average: (load_average.one, load_average.five, load_average.fifteen),
            disks: self.cached_disks.clone(),
            storage_devices: self.cached_storage_devices.clone(),
            processes_list: self.processes_list.clone(),
            gpus: self.gpus.get().to_vec(),
            networks: NetworkInfo::get(&self.networks, network_interval),
            all_networks: NetworkInfo::get_all(&self.networks, network_interval),
            temperatures,
        }
    }

    fn refresh_disks_if_due(&mut self, collected_at: Instant) {
        let inventory_due = refresh_due(
            self.last_disk_inventory_refresh,
            collected_at,
            DISK_INVENTORY_REFRESH_INTERVAL,
        );
        let io_due = refresh_due(
            self.last_disk_io_sample,
            collected_at,
            DISK_IO_REFRESH_INTERVAL,
        );
        if !inventory_due && !io_due {
            return;
        }

        if inventory_due {
            self.disks.refresh(true);
            self.last_disk_inventory_refresh = Some(collected_at);
        } else {
            for disk in self.disks.list_mut() {
                disk.refresh();
            }
        }

        let disk_interval = self
            .last_disk_io_sample
            .map(|previous| collected_at.saturating_duration_since(previous));
        let mut legacy_disks = Vec::new();
        let mut grouped_disks = BTreeMap::<String, DiskInfo>::new();
        let raw_disk_names = self
            .disks
            .iter()
            .map(|disk| disk.name().to_string_lossy().to_string())
            .collect::<BTreeSet<_>>();
        let partitioned_devices = raw_disk_names
            .iter()
            .filter_map(|name| {
                let id = physical_disk_id(name);
                (id != *name).then_some(id)
            })
            .collect::<BTreeSet<_>>();
        let current_disk_ids = raw_disk_names
            .iter()
            .map(|name| physical_disk_id(name))
            .collect::<BTreeSet<_>>();
        self.disk_models
            .retain(|id, _| current_disk_ids.contains(id));
        for id in &current_disk_ids {
            let _ = self.cached_disk_model(id, collected_at);
        }
        let mut seen_partition_names = BTreeSet::new();
        for disk in self.disks.iter() {
            let raw_name = disk.name().to_string_lossy().to_string();
            let id = physical_disk_id(&raw_name);
            let count_capacity = should_count_storage_sample(
                &mut seen_partition_names,
                &raw_name,
                &id,
                partitioned_devices.contains(&id),
            );
            let mount_point = disk.mount_point().to_string_lossy().to_string();
            let total_space = disk.total_space() as f64 / 1024.0 / 1024.0 / 1024.0;
            let available_space = disk.available_space() as f64 / 1024.0 / 1024.0 / 1024.0;
            let usage = disk.usage();
            let model = self
                .disk_models
                .get(&id)
                .and_then(|entry| entry.model.clone());
            legacy_disks.push(DiskInfo {
                id: id.clone(),
                name: raw_name,
                model: model.clone(),
                mount_point: mount_point.clone(),
                mount_points: (!mount_point.is_empty())
                    .then_some(vec![mount_point.clone()])
                    .unwrap_or_default(),
                total_space,
                available_space,
                is_virtual: id.starts_with("zram") || mount_point == "[SWAP]",
                read_bytes_per_sec: bytes_per_second(usage.read_bytes, disk_interval),
                written_bytes_per_sec: bytes_per_second(usage.written_bytes, disk_interval),
            });
            let entry = grouped_disks.entry(id.clone()).or_insert_with(|| DiskInfo {
                id: id.clone(),
                name: id.clone(),
                model,
                mount_point: mount_point.clone(),
                mount_points: Vec::new(),
                total_space: 0.0,
                available_space: 0.0,
                is_virtual: id.starts_with("zram") || mount_point == "[SWAP]",
                read_bytes_per_sec: Some(0.0),
                written_bytes_per_sec: Some(0.0),
            });
            if !mount_point.is_empty() && !entry.mount_points.contains(&mount_point) {
                entry.mount_points.push(mount_point);
            }
            if count_capacity {
                entry.total_space += total_space;
                entry.available_space += available_space;
                entry.read_bytes_per_sec = add_optional(
                    entry.read_bytes_per_sec,
                    bytes_per_second(usage.read_bytes, disk_interval),
                );
                entry.written_bytes_per_sec = add_optional(
                    entry.written_bytes_per_sec,
                    bytes_per_second(usage.written_bytes, disk_interval),
                );
            }
        }
        self.cached_disks = legacy_disks;
        self.cached_storage_devices = grouped_disks.into_values().collect::<Vec<_>>();
        self.last_disk_io_sample = Some(collected_at);
    }

    fn refresh_processes(&mut self, collected_at: Instant) {
        let mut processes = self
            .system
            .processes()
            .values()
            .map(|process| ProcessInfo {
                pid: process.pid().as_u32(),
                name: process.name().to_string_lossy().to_string(),
                memory: process.memory() as f64 / 1024.0 / 1024.0,
                cpu_usage: process.cpu_usage(),
            })
            .collect::<Vec<ProcessInfo>>();
        sort_processes_for_dashboard(&mut processes);
        self.processes_list = processes;
        self.last_process_refresh = Some(collected_at);
    }

    fn cached_disk_model(&mut self, id: &str, collected_at: Instant) -> Option<String> {
        let should_refresh = self.disk_models.get(id).is_none_or(|entry| {
            collected_at.saturating_duration_since(entry.refreshed_at)
                >= DISK_METADATA_REFRESH_INTERVAL
        });
        if should_refresh {
            self.disk_models.insert(
                id.to_owned(),
                DiskModelCacheEntry {
                    model: read_disk_model(id),
                    refreshed_at: collected_at,
                },
            );
        }
        self.disk_models
            .get(id)
            .and_then(|entry| entry.model.clone())
    }
}

fn system_refresh_kind() -> RefreshKind {
    RefreshKind::nothing()
        .with_cpu(CpuRefreshKind::nothing().with_cpu_usage().with_frequency())
        .with_memory(MemoryRefreshKind::everything())
}

fn process_refresh_kind() -> ProcessRefreshKind {
    ProcessRefreshKind::nothing().with_cpu().with_memory()
}

fn refresh_due(last_refresh: Option<Instant>, now: Instant, interval: Duration) -> bool {
    last_refresh.is_none_or(|previous| now.saturating_duration_since(previous) >= interval)
}

fn sort_processes_for_dashboard(processes: &mut [ProcessInfo]) {
    let top_count = TOP_PROCESS_COUNT.min(processes.len());
    if top_count == 0 {
        return;
    }

    // Keep every process available to search while only fully ordering the rows shown by the UI.
    processes.select_nth_unstable_by(top_count - 1, compare_processes);
    processes[..top_count].sort_unstable_by(compare_processes);
}

fn compare_processes(left: &ProcessInfo, right: &ProcessInfo) -> Ordering {
    right
        .memory
        .partial_cmp(&left.memory)
        .unwrap_or(Ordering::Equal)
        .then_with(|| left.name.cmp(&right.name))
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

fn temperature_infos(components: &Components) -> Vec<TemperatureInfo> {
    let mut sensors = components
        .iter()
        .enumerate()
        .filter_map(|(index, component)| {
            let raw_label = component.label().trim();
            if raw_label.is_empty() {
                return None;
            }
            let label = human_temperature_label(raw_label);
            let base_id = stable_sensor_id(raw_label);
            let id = format!("{base_id}-{index}");
            Some(TemperatureInfo {
                id,
                label,
                celsius: component
                    .temperature()
                    .map(f64::from)
                    .filter(|value| value.is_finite()),
            })
        })
        .collect::<Vec<_>>();
    sensors.sort_by(|left, right| {
        temperature_priority(&left.label)
            .cmp(&temperature_priority(&right.label))
            .then_with(|| left.id.cmp(&right.id))
    });
    sensors
}

fn human_temperature_label(label: &str) -> String {
    let normalized = label.to_ascii_lowercase();
    if normalized.contains("k10temp")
        || normalized.contains("tctl")
        || normalized.contains("tdie")
        || normalized.contains("cpu")
        || normalized.contains("package")
    {
        return "CPU Package".to_owned();
    }
    if normalized.contains("amdgpu") || normalized.contains("amd gpu") {
        return "AMD GPU".to_owned();
    }
    if normalized.contains("nvidia") || normalized.contains("nouveau") {
        return "NVIDIA GPU".to_owned();
    }
    if normalized.contains("nvme") || normalized.contains("composite") {
        return "NVMe".to_owned();
    }
    label.to_owned()
}

fn temperature_priority(label: &str) -> u8 {
    match label {
        "CPU Package" => 0,
        "AMD GPU" | "NVIDIA GPU" => 1,
        "NVMe" => 2,
        _ => 3,
    }
}

fn stable_sensor_id(label: &str) -> String {
    label
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
}

fn physical_disk_id(name: &str) -> String {
    let name = name.strip_prefix("/dev/").unwrap_or(name);
    if let Some(index) = name.rfind('p')
        && name[index + 1..]
            .chars()
            .all(|character| character.is_ascii_digit())
        && index > 0
        && (name[..index].contains("nvme") || name[..index].contains("mmcblk"))
    {
        return name[..index].to_owned();
    }
    if name.contains("nvme") || name.contains("mmcblk") {
        return name.to_owned();
    }
    let suffix_start = name
        .char_indices()
        .rev()
        .find(|(_, character)| !character.is_ascii_digit())
        .map_or(0, |(index, _)| index + 1);
    if suffix_start < name.len() {
        name[..suffix_start].to_owned()
    } else {
        name.to_owned()
    }
}

fn read_disk_model(id: &str) -> Option<String> {
    #[cfg(target_os = "linux")]
    {
        let path = format!("/sys/class/block/{id}/device/model");
        std::fs::read_to_string(path)
            .ok()
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty())
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = id;
        None
    }
}

fn add_optional(left: Option<f64>, right: Option<f64>) -> Option<f64> {
    match (left, right) {
        (Some(left), Some(right)) if left.is_finite() && right.is_finite() => Some(left + right),
        (Some(left), _) if left.is_finite() => Some(left),
        (_, Some(right)) if right.is_finite() => Some(right),
        _ => None,
    }
}

fn should_count_storage_sample(
    seen_partition_names: &mut BTreeSet<String>,
    raw_name: &str,
    physical_id: &str,
    has_partition_records: bool,
) -> bool {
    if has_partition_records && raw_name == physical_id {
        return false;
    }
    seen_partition_names.insert(raw_name.to_owned())
}

fn bytes_per_second(bytes: u64, interval: Option<Duration>) -> Option<f64> {
    let seconds = interval?.as_secs_f64();
    (seconds > 0.0).then_some(bytes as f64 / seconds)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::{
        ProcessInfo, human_temperature_label, physical_disk_id, should_count_storage_sample,
        sort_processes_for_dashboard, temperature_priority,
    };

    #[test]
    fn physical_disk_ids_group_common_linux_partition_names() {
        assert_eq!(physical_disk_id("/dev/sda1"), "sda");
        assert_eq!(physical_disk_id("/dev/sda2"), "sda");
        assert_eq!(physical_disk_id("/dev/nvme0n1p1"), "nvme0n1");
        assert_eq!(physical_disk_id("/dev/nvme0n1"), "nvme0n1");
        assert_eq!(physical_disk_id("/dev/mmcblk0p2"), "mmcblk0");
    }

    #[test]
    fn physical_disk_id_keeps_unknown_names_without_inventing_relationships() {
        assert_eq!(physical_disk_id("/dev/mapper/root"), "mapper/root");
        assert_eq!(physical_disk_id("custom-device"), "custom-device");
    }

    #[test]
    fn storage_capacity_counts_each_partition_once_and_prefers_partitions() {
        let mut seen = BTreeSet::new();
        assert!(!should_count_storage_sample(&mut seen, "sda", "sda", true));
        assert!(should_count_storage_sample(&mut seen, "sda1", "sda", true));
        assert!(!should_count_storage_sample(&mut seen, "sda1", "sda", true));
    }

    #[test]
    fn temperature_labels_are_human_and_ordered_by_component_family() {
        assert_eq!(human_temperature_label("Tctl"), "CPU Package");
        assert_eq!(human_temperature_label("amdgpu edge"), "AMD GPU");
        assert_eq!(human_temperature_label("NVIDIA GPU temp"), "NVIDIA GPU");
        assert_eq!(human_temperature_label("Composite"), "NVMe");
        assert!(temperature_priority("CPU Package") < temperature_priority("AMD GPU"));
        assert!(temperature_priority("AMD GPU") < temperature_priority("NVMe"));
    }

    #[test]
    fn keeps_all_processes_but_orders_only_the_dashboard_top_k() {
        let mut processes = vec![
            ProcessInfo {
                pid: 1,
                name: "small".to_owned(),
                memory: 1.0,
                cpu_usage: 0.0,
            },
            ProcessInfo {
                pid: 2,
                name: "largest".to_owned(),
                memory: 6.0,
                cpu_usage: 0.0,
            },
            ProcessInfo {
                pid: 3,
                name: "second".to_owned(),
                memory: 5.0,
                cpu_usage: 0.0,
            },
            ProcessInfo {
                pid: 4,
                name: "third".to_owned(),
                memory: 4.0,
                cpu_usage: 0.0,
            },
            ProcessInfo {
                pid: 5,
                name: "fourth".to_owned(),
                memory: 3.0,
                cpu_usage: 0.0,
            },
            ProcessInfo {
                pid: 6,
                name: "fifth".to_owned(),
                memory: 2.0,
                cpu_usage: 0.0,
            },
        ];

        sort_processes_for_dashboard(&mut processes);

        assert_eq!(processes.len(), 6);
        let top_names = processes[..5]
            .iter()
            .map(|process| process.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(top_names, ["largest", "second", "third", "fourth", "fifth"]);
        assert!(processes.iter().any(|process| process.name == "small"));
    }
}
