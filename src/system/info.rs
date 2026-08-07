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

pub struct TemperatureInfo {
    pub id: String,
    pub label: String,
    pub celsius: Option<f64>,
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
            legacy_disks.push(DiskInfo {
                id: id.clone(),
                name: raw_name,
                model: read_disk_model(&id),
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
                model: read_disk_model(&id),
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
        let storage_devices = grouped_disks.into_values().collect::<Vec<_>>();
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
        let temperatures = temperature_infos(&self.components);
        let cpu_temperature_celsius = temperatures
            .iter()
            .filter(|sensor| {
                let label = sensor.label.to_ascii_lowercase();
                label.contains("cpu") || label.contains("package") || label.contains("core")
            })
            .filter_map(|sensor| sensor.celsius)
            .max_by(f64::total_cmp);

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
            cpu_physical_cores: System::physical_core_count(),
            cpu_logical_threads: self.system.cpus().len(),
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
            disks: legacy_disks,
            storage_devices,
            processes_list,
            gpus: self.gpus.get().to_vec(),
            networks: NetworkInfo::get(&self.networks, network_interval),
            all_networks: NetworkInfo::get_all(&self.networks, network_interval),
            temperatures,
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
        human_temperature_label, physical_disk_id, should_count_storage_sample,
        temperature_priority,
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
}
