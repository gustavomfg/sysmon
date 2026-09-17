use super::MetricAvailability;

/// Stable identity for a hardware item. The kind is part of the identity so
/// names shared by different buses cannot collide in the dashboard.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct HardwareId {
    kind: HardwareKind,
    key: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum HardwareKind {
    Storage,
    Gpu,
    Network,
    Temperature,
}

impl HardwareId {
    pub fn new(kind: HardwareKind, key: impl Into<String>) -> Option<Self> {
        let key = key.into();
        (!key.trim().is_empty()).then_some(Self { kind, key })
    }

    pub const fn kind(&self) -> HardwareKind {
        self.kind
    }

    pub fn key(&self) -> &str {
        &self.key
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct StorageDevice {
    pub id: HardwareId,
    pub name: String,
    pub model: Option<String>,
    pub mount_points: Vec<String>,
    pub used_bytes: Option<f64>,
    pub total_bytes: Option<f64>,
    pub read_bytes_per_sec: Option<f64>,
    pub write_bytes_per_sec: Option<f64>,
    pub latency_seconds: Option<f64>,
    pub availability: MetricAvailability,
    pub is_virtual: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GpuDevice {
    pub id: HardwareId,
    pub name: String,
    pub utilization_percent: Option<f64>,
    pub vram_used_bytes: Option<f64>,
    pub vram_total_bytes: Option<f64>,
    pub temperature_celsius: Option<f64>,
    pub frequency_hz: Option<f64>,
    pub power_watts: Option<f64>,
    pub availability: MetricAvailability,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NetworkInterface {
    pub id: HardwareId,
    pub name: String,
    pub download_bytes_per_sec: Option<f64>,
    pub upload_bytes_per_sec: Option<f64>,
    pub availability: MetricAvailability,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TemperatureSensor {
    pub id: HardwareId,
    pub label: String,
    pub celsius: Option<f64>,
    pub availability: MetricAvailability,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct HardwareInventory {
    pub storages: Vec<StorageDevice>,
    pub gpus: Vec<GpuDevice>,
    pub networks: Vec<NetworkInterface>,
    pub temperatures: Vec<TemperatureSensor>,
}

impl HardwareInventory {
    pub fn new(
        mut storages: Vec<StorageDevice>,
        mut gpus: Vec<GpuDevice>,
        mut networks: Vec<NetworkInterface>,
        mut temperatures: Vec<TemperatureSensor>,
    ) -> Self {
        normalize_storage(&mut storages);
        normalize_by_id(&mut gpus, |item| &item.id);
        normalize_by_id(&mut networks, |item| &item.id);
        normalize_by_id(&mut temperatures, |item| &item.id);
        Self {
            storages,
            gpus,
            networks,
            temperatures,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.storages.is_empty()
            && self.gpus.is_empty()
            && self.networks.is_empty()
            && self.temperatures.is_empty()
    }
}

fn normalize_storage(items: &mut Vec<StorageDevice>) {
    for item in items.iter_mut() {
        item.mount_points.sort();
        item.mount_points.dedup();
    }
    items.sort_by(|left, right| left.id.cmp(&right.id));
    items.dedup_by(|right, left| {
        if right.id != left.id {
            return false;
        }
        left.mount_points.append(&mut right.mount_points);
        left.mount_points.sort();
        left.mount_points.dedup();
        true
    });
}

fn normalize_by_id<T, F>(items: &mut Vec<T>, id: F)
where
    F: Fn(&T) -> &HardwareId,
{
    items.sort_by(|left, right| id(left).cmp(id(right)));
    items.dedup_by(|right, left| id(right) == id(left));
}

pub(super) fn select_primary_gpu(gpus: &[GpuDevice]) -> usize {
    let mut selected = 0;
    let mut selected_score = (0_u8, 0_u8, 0_u8, 0_u8);
    for (index, gpu) in gpus.iter().enumerate() {
        let complete = u8::from(
            gpu.utilization_percent.is_some()
                && gpu.vram_used_bytes.is_some()
                && gpu.vram_total_bytes.is_some()
                && gpu.temperature_celsius.is_some(),
        );
        let usage = u8::from(gpu.utilization_percent.is_some());
        let discrete = u8::from(is_discrete_gpu(&gpu.name));
        let telemetry = u8::from(
            gpu.vram_used_bytes.is_some()
                || gpu.vram_total_bytes.is_some()
                || gpu.temperature_celsius.is_some(),
        );
        let score = (complete, usage, discrete, telemetry);
        if score > selected_score {
            selected = index;
            selected_score = score;
        }
    }
    selected
}

fn is_discrete_gpu(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    name.contains("nvidia")
        || name.contains("geforce")
        || name.contains("rtx")
        || name.contains("gtx")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(kind: HardwareKind, key: &str) -> HardwareId {
        HardwareId::new(kind, key).unwrap()
    }

    #[test]
    fn inventory_is_sorted_and_deduplicates_mount_points() {
        let first = StorageDevice {
            id: id(HardwareKind::Storage, "nvme0n1"),
            name: "nvme0n1".to_owned(),
            model: Some("Fast SSD".to_owned()),
            mount_points: vec!["/games".to_owned(), "/".to_owned()],
            used_bytes: Some(10.0),
            total_bytes: Some(100.0),
            read_bytes_per_sec: Some(1.0),
            write_bytes_per_sec: Some(2.0),
            latency_seconds: None,
            availability: MetricAvailability::Available,
            is_virtual: false,
        };
        let duplicate = StorageDevice {
            mount_points: vec!["/".to_owned(), "/home".to_owned()],
            ..first.clone()
        };
        let second = StorageDevice {
            id: id(HardwareKind::Storage, "sda"),
            name: "sda".to_owned(),
            model: None,
            mount_points: Vec::new(),
            used_bytes: None,
            total_bytes: None,
            read_bytes_per_sec: None,
            write_bytes_per_sec: None,
            latency_seconds: None,
            availability: MetricAvailability::Unavailable,
            is_virtual: false,
        };

        let inventory = HardwareInventory::new(
            vec![first, duplicate, second],
            Vec::new(),
            Vec::new(),
            Vec::new(),
        );
        assert_eq!(inventory.storages.len(), 2);
        assert_eq!(inventory.storages[0].name, "nvme0n1");
        assert_eq!(inventory.storages[0].mount_points, ["/", "/games", "/home"]);
    }

    #[test]
    fn all_inventory_kinds_are_ordered_by_stable_id() {
        let inventory = HardwareInventory::new(
            Vec::new(),
            vec![GpuDevice {
                id: id(HardwareKind::Gpu, "gpu-1"),
                name: "GPU".to_owned(),
                utilization_percent: None,
                vram_used_bytes: None,
                vram_total_bytes: None,
                temperature_celsius: None,
                frequency_hz: None,
                power_watts: None,
                availability: MetricAvailability::Unavailable,
            }],
            vec![NetworkInterface {
                id: id(HardwareKind::Network, "eth0"),
                name: "eth0".to_owned(),
                download_bytes_per_sec: Some(0.0),
                upload_bytes_per_sec: Some(0.0),
                availability: MetricAvailability::Available,
            }],
            vec![TemperatureSensor {
                id: id(HardwareKind::Temperature, "cpu"),
                label: "CPU".to_owned(),
                celsius: Some(50.0),
                availability: MetricAvailability::Available,
            }],
        );
        assert_eq!(inventory.gpus[0].id.key(), "gpu-1");
        assert_eq!(inventory.networks[0].id.key(), "eth0");
        assert!(inventory.temperatures[0].availability.is_usable());
    }

    #[test]
    fn refresh_can_add_and_remove_devices_without_reordering_existing_ids() {
        let storage = |key: &str| StorageDevice {
            id: id(HardwareKind::Storage, key),
            name: key.to_owned(),
            model: None,
            mount_points: Vec::new(),
            used_bytes: Some(1.0),
            total_bytes: Some(2.0),
            read_bytes_per_sec: None,
            write_bytes_per_sec: None,
            latency_seconds: None,
            availability: MetricAvailability::Available,
            is_virtual: false,
        };
        let initial =
            HardwareInventory::new(vec![storage("sda")], Vec::new(), Vec::new(), Vec::new());
        let added = HardwareInventory::new(
            vec![storage("sda"), storage("nvme0n1")],
            Vec::new(),
            Vec::new(),
            Vec::new(),
        );
        let removed =
            HardwareInventory::new(vec![storage("nvme0n1")], Vec::new(), Vec::new(), Vec::new());

        assert_eq!(initial.storages[0].id, added.storages[1].id);
        assert_eq!(removed.storages[0].id.key(), "nvme0n1");
    }
}
