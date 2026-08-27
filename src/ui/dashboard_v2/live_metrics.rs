use std::time::{Duration, Instant};

use crate::system::info::SystemInfo;

use super::chart::{ChartError, TimeSample, TimeSeries};
use super::metrics::{
    CpuMetricModel, GpuMetricInput, GpuMetricModel, MemoryMetricInput, MemoryMetricModel,
    MetricModelError, MetricReading, NetworkMetricInput, NetworkMetricModel, StorageMetricInput,
    StorageMetricModel,
};
use super::visual::MetricUnit;
use super::{CpuLiveHistory, CpuLiveSample};
use super::{
    GpuDevice, HardwareId, HardwareInventory, HardwareKind, MetricAvailability, NetworkInterface,
    StorageDevice, TemperatureSensor,
};

/// Two minutes of one-second samples keeps the metric models' medium window
/// close to the requested sixty-second visual history.
pub const LIVE_METRICS_HISTORY_CAPACITY: usize = 120;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LiveMetricsError {
    Chart(ChartError),
    Model(MetricModelError),
}

impl From<ChartError> for LiveMetricsError {
    fn from(error: ChartError) -> Self {
        Self::Chart(error)
    }
}

impl From<MetricModelError> for LiveMetricsError {
    fn from(error: MetricModelError) -> Self {
        Self::Model(error)
    }
}

/// Collector-independent input produced by the runtime snapshot adapter.
#[derive(Debug, Clone, PartialEq)]
pub struct LiveSnapshot {
    pub cpu: CpuLiveSample,
    pub memory_used_bytes: Option<f64>,
    pub memory_available_bytes: Option<f64>,
    pub memory_total_bytes: Option<f64>,
    pub memory_swap_bytes: Option<f64>,
    pub memory_total_swap_bytes: Option<f64>,
    pub gpu: Option<LiveGpuSnapshot>,
    pub download_bytes_per_sec: Option<f64>,
    pub upload_bytes_per_sec: Option<f64>,
    pub storage: Option<LiveStorageSnapshot>,
    pub hardware: HardwareInventory,
}

impl LiveSnapshot {
    pub fn from_system_info(snapshot: &SystemInfo) -> Self {
        let gpus = snapshot
            .gpus
            .iter()
            .filter_map(|gpu| {
                Some(GpuDevice {
                    id: HardwareId::new(HardwareKind::Gpu, gpu.id.clone())?,
                    name: gpu.name.clone(),
                    utilization_percent: gpu
                        .usage_percent
                        .or_else(|| parse_percent(&gpu.usage))
                        .and_then(valid_percent),
                    vram_used_bytes: finite_non_negative(gpu.vram_used_bytes),
                    vram_total_bytes: finite_non_negative(gpu.vram_total_bytes),
                    temperature_celsius: finite_non_negative(gpu.temperature_celsius),
                    frequency_hz: finite_non_negative(gpu.frequency_hz),
                    power_watts: finite_non_negative(gpu.power_watts),
                    availability: MetricAvailability::Available,
                })
            })
            .collect::<Vec<_>>();
        let storages = snapshot
            .storage_devices
            .iter()
            .filter(|disk| !disk.is_virtual)
            .filter_map(|disk| {
                Some(StorageDevice {
                    id: HardwareId::new(HardwareKind::Storage, disk.id.clone())?,
                    name: disk.name.clone(),
                    model: disk.model.clone(),
                    mount_points: disk.mount_points.clone(),
                    used_bytes: finite_non_negative(Some(
                        (disk.total_space - disk.available_space).max(0.0) * GIBIBYTE,
                    )),
                    total_bytes: finite_non_negative(Some(disk.total_space.max(0.0) * GIBIBYTE)),
                    read_bytes_per_sec: finite_non_negative(disk.read_bytes_per_sec),
                    write_bytes_per_sec: finite_non_negative(disk.written_bytes_per_sec),
                    latency_seconds: None,
                    availability: MetricAvailability::Available,
                    is_virtual: false,
                })
            })
            .collect::<Vec<_>>();
        let networks = snapshot
            .all_networks
            .iter()
            .filter_map(|network| {
                Some(NetworkInterface {
                    id: HardwareId::new(HardwareKind::Network, network.id.clone())?,
                    name: network.interface.clone(),
                    download_bytes_per_sec: finite_non_negative(Some(
                        network.received_per_sec * 1_000_000.0,
                    )),
                    upload_bytes_per_sec: finite_non_negative(Some(
                        network.transmitted_per_sec * 1_000_000.0,
                    )),
                    availability: MetricAvailability::Available,
                })
            })
            .collect::<Vec<_>>();
        let temperatures = snapshot
            .temperatures
            .iter()
            .filter_map(|sensor| {
                Some(TemperatureSensor {
                    id: HardwareId::new(HardwareKind::Temperature, sensor.id.clone())?,
                    label: sensor.label.clone(),
                    celsius: finite_non_negative(sensor.celsius),
                    availability: MetricAvailability::Available,
                })
            })
            .collect::<Vec<_>>();
        let hardware = HardwareInventory::new(storages, gpus, networks, temperatures);
        let gpu = hardware.gpus.first().map(|gpu| LiveGpuSnapshot {
            utilization_percent: gpu.utilization_percent,
            vram_used_bytes: finite_non_negative(gpu.vram_used_bytes),
            vram_total_bytes: finite_non_negative(gpu.vram_total_bytes),
            temperature_celsius: finite_non_negative(gpu.temperature_celsius),
            frequency_hz: finite_non_negative(gpu.frequency_hz),
            power_watts: finite_non_negative(gpu.power_watts),
        });
        let storage = hardware.storages.first().map(|disk| LiveStorageSnapshot {
            used_bytes: disk.used_bytes,
            total_bytes: disk.total_bytes,
            read_bytes_per_sec: disk.read_bytes_per_sec,
            write_bytes_per_sec: disk.write_bytes_per_sec,
        });
        let (download, upload) = snapshot
            .all_networks
            .iter()
            .filter(|network| !is_loopback_interface(&network.interface))
            .fold((0.0, 0.0), |(down, up), network| {
                (
                    down + network.received_per_sec * 1_000_000.0,
                    up + network.transmitted_per_sec * 1_000_000.0,
                )
            });

        Self {
            cpu: CpuLiveSample {
                utilization_percent: valid_percent(f64::from(snapshot.cpu_usage)),
                frequency_hz: finite_non_negative(snapshot.cpu_frequency_hz),
                temperature_celsius: finite_non_negative(snapshot.cpu_temperature_celsius),
                cores: snapshot.cpu_physical_cores.unwrap_or(snapshot.cpu_cores),
            },
            memory_used_bytes: finite_non_negative(Some(snapshot.used_ram * GIBIBYTE)),
            memory_available_bytes: finite_non_negative(Some(snapshot.available_ram * GIBIBYTE)),
            memory_total_bytes: finite_non_negative(Some(snapshot.total_ram * GIBIBYTE)),
            memory_swap_bytes: finite_non_negative(Some(snapshot.used_swap * GIBIBYTE)),
            memory_total_swap_bytes: finite_non_negative(Some(snapshot.total_swap * GIBIBYTE)),
            gpu,
            download_bytes_per_sec: Some(download),
            upload_bytes_per_sec: Some(upload),
            storage,
            hardware,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LiveGpuSnapshot {
    pub utilization_percent: Option<f64>,
    pub vram_used_bytes: Option<f64>,
    pub vram_total_bytes: Option<f64>,
    pub temperature_celsius: Option<f64>,
    pub frequency_hz: Option<f64>,
    pub power_watts: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LiveStorageSnapshot {
    pub used_bytes: Option<f64>,
    pub total_bytes: Option<f64>,
    pub read_bytes_per_sec: Option<f64>,
    pub write_bytes_per_sec: Option<f64>,
}

#[derive(Debug, Clone, Copy)]
struct MemorySample {
    used_bytes: Option<f64>,
    available_bytes: Option<f64>,
    total_bytes: Option<f64>,
    swap_bytes: Option<f64>,
    swap_total_bytes: Option<f64>,
}

#[derive(Debug, Clone, Copy)]
struct GpuSample {
    utilization_percent: Option<f64>,
    vram_used_bytes: Option<f64>,
    vram_total_bytes: Option<f64>,
    temperature_celsius: Option<f64>,
    frequency_hz: Option<f64>,
    power_watts: Option<f64>,
}

#[derive(Debug, Clone, Copy)]
struct NetworkSample {
    download_bytes_per_sec: Option<f64>,
    upload_bytes_per_sec: Option<f64>,
}

#[derive(Debug, Clone, Copy)]
struct StorageSample {
    used_bytes: Option<f64>,
    total_bytes: Option<f64>,
    read_bytes_per_sec: Option<f64>,
    write_bytes_per_sec: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct LiveMetricModels {
    cpu: CpuMetricModel<LIVE_METRICS_HISTORY_CAPACITY>,
    memory: MemoryMetricModel<LIVE_METRICS_HISTORY_CAPACITY>,
    gpu: GpuMetricModel<LIVE_METRICS_HISTORY_CAPACITY>,
    network: NetworkMetricModel<LIVE_METRICS_HISTORY_CAPACITY>,
    storage: StorageMetricModel<LIVE_METRICS_HISTORY_CAPACITY>,
    hardware: HardwareInventory,
}

impl LiveMetricModels {
    pub fn cpu(&self) -> CpuMetricModel<LIVE_METRICS_HISTORY_CAPACITY> {
        self.cpu
    }
    pub fn memory(&self) -> MemoryMetricModel<LIVE_METRICS_HISTORY_CAPACITY> {
        self.memory
    }
    pub fn gpu(&self) -> GpuMetricModel<LIVE_METRICS_HISTORY_CAPACITY> {
        self.gpu
    }
    pub fn network(&self) -> NetworkMetricModel<LIVE_METRICS_HISTORY_CAPACITY> {
        self.network
    }
    pub fn storage(&self) -> StorageMetricModel<LIVE_METRICS_HISTORY_CAPACITY> {
        self.storage
    }

    pub fn inventory(&self) -> &HardwareInventory {
        &self.hardware
    }
}

#[derive(Debug)]
pub struct LiveMetricsHistory {
    origin: Option<Instant>,
    cpu: CpuLiveHistory,
    memory_history: TimeSeries<LIVE_METRICS_HISTORY_CAPACITY>,
    gpu_history: TimeSeries<LIVE_METRICS_HISTORY_CAPACITY>,
    download_history: TimeSeries<LIVE_METRICS_HISTORY_CAPACITY>,
    upload_history: TimeSeries<LIVE_METRICS_HISTORY_CAPACITY>,
    occupancy_history: TimeSeries<LIVE_METRICS_HISTORY_CAPACITY>,
    read_history: TimeSeries<LIVE_METRICS_HISTORY_CAPACITY>,
    write_history: TimeSeries<LIVE_METRICS_HISTORY_CAPACITY>,
    memory: MemorySample,
    gpu: GpuSample,
    network: NetworkSample,
    storage: StorageSample,
    hardware: HardwareInventory,
}

impl Default for LiveMetricsHistory {
    fn default() -> Self {
        Self::new()
    }
}

impl LiveMetricsHistory {
    pub fn new() -> Self {
        Self {
            origin: None,
            cpu: CpuLiveHistory::new(),
            memory_history: TimeSeries::new(MetricUnit::Percent),
            gpu_history: TimeSeries::new(MetricUnit::Percent),
            download_history: TimeSeries::new(MetricUnit::BytesPerSecond),
            upload_history: TimeSeries::new(MetricUnit::BytesPerSecond),
            occupancy_history: TimeSeries::new(MetricUnit::Percent),
            read_history: TimeSeries::new(MetricUnit::BytesPerSecond),
            write_history: TimeSeries::new(MetricUnit::BytesPerSecond),
            memory: MemorySample {
                used_bytes: None,
                available_bytes: None,
                total_bytes: None,
                swap_bytes: None,
                swap_total_bytes: None,
            },
            gpu: GpuSample {
                utilization_percent: None,
                vram_used_bytes: None,
                vram_total_bytes: None,
                temperature_celsius: None,
                frequency_hz: None,
                power_watts: None,
            },
            network: NetworkSample {
                download_bytes_per_sec: None,
                upload_bytes_per_sec: None,
            },
            storage: StorageSample {
                used_bytes: None,
                total_bytes: None,
                read_bytes_per_sec: None,
                write_bytes_per_sec: None,
            },
            hardware: HardwareInventory::default(),
        }
    }

    pub fn push_snapshot(
        &mut self,
        collected_at: Instant,
        snapshot: &LiveSnapshot,
    ) -> Result<(), LiveMetricsError> {
        let origin = *self.origin.get_or_insert(collected_at);
        let offset = collected_at.saturating_duration_since(origin);

        self.cpu.push(collected_at, snapshot.cpu)?;
        self.hardware = snapshot.hardware.clone();
        self.memory = MemorySample {
            used_bytes: finite_non_negative(snapshot.memory_used_bytes),
            available_bytes: finite_non_negative(snapshot.memory_available_bytes),
            total_bytes: finite_non_negative(snapshot.memory_total_bytes),
            swap_bytes: finite_non_negative(snapshot.memory_swap_bytes),
            swap_total_bytes: finite_non_negative(snapshot.memory_total_swap_bytes),
        };
        push_normalized(
            &mut self.memory_history,
            offset,
            memory_occupancy(self.memory),
        )?;

        self.gpu = snapshot
            .gpu
            .map_or_else(GpuSample::unavailable, |gpu| GpuSample {
                utilization_percent: finite_percent(gpu.utilization_percent),
                vram_used_bytes: finite_non_negative(gpu.vram_used_bytes),
                vram_total_bytes: finite_non_negative(gpu.vram_total_bytes),
                temperature_celsius: finite_non_negative(gpu.temperature_celsius),
                frequency_hz: finite_non_negative(gpu.frequency_hz),
                power_watts: finite_non_negative(gpu.power_watts),
            });
        push_normalized(
            &mut self.gpu_history,
            offset,
            self.gpu.utilization_percent.map(|value| value / 100.0),
        )?;

        self.network = NetworkSample {
            download_bytes_per_sec: finite_non_negative(snapshot.download_bytes_per_sec),
            upload_bytes_per_sec: finite_non_negative(snapshot.upload_bytes_per_sec),
        };
        push_value(
            &mut self.download_history,
            offset,
            self.network.download_bytes_per_sec,
        )?;
        push_value(
            &mut self.upload_history,
            offset,
            self.network.upload_bytes_per_sec,
        )?;

        self.storage = snapshot
            .storage
            .map_or_else(StorageSample::unavailable, |storage| StorageSample {
                used_bytes: finite_non_negative(storage.used_bytes),
                total_bytes: finite_non_negative(storage.total_bytes),
                read_bytes_per_sec: finite_non_negative(storage.read_bytes_per_sec),
                write_bytes_per_sec: finite_non_negative(storage.write_bytes_per_sec),
            });
        push_normalized(
            &mut self.occupancy_history,
            offset,
            storage_occupancy(self.storage),
        )?;
        push_value(
            &mut self.read_history,
            offset,
            self.storage.read_bytes_per_sec,
        )?;
        push_value(
            &mut self.write_history,
            offset,
            self.storage.write_bytes_per_sec,
        )?;
        Ok(())
    }

    pub fn models(&self) -> Result<LiveMetricModels, LiveMetricsError> {
        let memory_input = MemoryMetricInput::new(
            non_negative(self.memory.used_bytes),
            non_negative(self.memory.available_bytes),
            non_negative(self.memory.total_bytes),
            MetricReading::unavailable(),
            non_negative(self.memory.swap_bytes),
            // The current snapshot contract has no kernel pressure metric;
            // occupancy remains the history signal without being mislabeled
            // as pressure in the card.
            MetricReading::unavailable(),
            self.memory_history,
        )?
        .with_swap_total(non_negative(self.memory.swap_total_bytes))?;
        let memory = MemoryMetricModel::from_input(memory_input)?;
        let gpu = GpuMetricModel::from_input(GpuMetricInput::new(
            normalized(self.gpu.utilization_percent.map(|value| value / 100.0)),
            non_negative(self.gpu.vram_used_bytes),
            non_negative(self.gpu.vram_total_bytes),
            non_negative(self.gpu.temperature_celsius),
            non_negative(self.gpu.frequency_hz),
            non_negative(self.gpu.power_watts),
            self.gpu_history,
        )?)?;
        let network = NetworkMetricModel::from_input(NetworkMetricInput::new(
            non_negative(self.network.download_bytes_per_sec),
            non_negative(self.network.upload_bytes_per_sec),
            MetricReading::unavailable(),
            MetricReading::unavailable(),
            MetricReading::unavailable(),
            self.download_history,
            self.upload_history,
        )?)?;
        let storage = StorageMetricModel::from_input(StorageMetricInput::new(
            non_negative(self.storage.used_bytes),
            non_negative(self.storage.total_bytes),
            non_negative(self.storage.read_bytes_per_sec),
            non_negative(self.storage.write_bytes_per_sec),
            MetricReading::unavailable(),
            MetricReading::unavailable(),
            self.occupancy_history,
            self.read_history,
            self.write_history,
        )?)?;
        Ok(LiveMetricModels {
            cpu: self.cpu.model()?,
            memory,
            gpu,
            network,
            storage,
            hardware: self.hardware.clone(),
        })
    }
}

impl GpuSample {
    const fn unavailable() -> Self {
        Self {
            utilization_percent: None,
            vram_used_bytes: None,
            vram_total_bytes: None,
            temperature_celsius: None,
            frequency_hz: None,
            power_watts: None,
        }
    }
}

impl StorageSample {
    const fn unavailable() -> Self {
        Self {
            used_bytes: None,
            total_bytes: None,
            read_bytes_per_sec: None,
            write_bytes_per_sec: None,
        }
    }
}

fn push_normalized(
    series: &mut TimeSeries<LIVE_METRICS_HISTORY_CAPACITY>,
    offset: Duration,
    value: Option<f64>,
) -> Result<(), ChartError> {
    push_value(series, offset, value.map(|value| value.clamp(0.0, 1.0)))
}

fn push_value(
    series: &mut TimeSeries<LIVE_METRICS_HISTORY_CAPACITY>,
    offset: Duration,
    value: Option<f64>,
) -> Result<(), ChartError> {
    let availability = value.map_or(MetricAvailability::Unavailable, |_| {
        MetricAvailability::Available
    });
    series.push(TimeSample::new(offset, value, availability, None)?)?;
    Ok(())
}

fn memory_occupancy(sample: MemorySample) -> Option<f64> {
    sample
        .used_bytes
        .zip(sample.total_bytes)
        .map(|(used, total)| used / total)
        .filter(|value| value.is_finite() && (0.0..=1.0).contains(value))
}

fn storage_occupancy(sample: StorageSample) -> Option<f64> {
    sample
        .used_bytes
        .zip(sample.total_bytes)
        .map(|(used, total)| used / total)
        .filter(|value| value.is_finite() && (0.0..=1.0).contains(value))
}

fn normalized(value: Option<f64>) -> MetricReading {
    let Some(value) = value.filter(|value| value.is_finite() && (0.0..=1.0).contains(value)) else {
        return MetricReading::unavailable();
    };
    MetricReading::normalized(Some(value), MetricAvailability::Available)
        .unwrap_or_else(|_| MetricReading::unavailable())
}

fn non_negative(value: Option<f64>) -> MetricReading {
    let Some(value) = value.filter(|value| value.is_finite() && *value >= 0.0) else {
        return MetricReading::unavailable();
    };
    MetricReading::non_negative(Some(value), MetricAvailability::Available)
        .unwrap_or_else(|_| MetricReading::unavailable())
}

fn finite_percent(value: Option<f64>) -> Option<f64> {
    value.filter(|value| value.is_finite() && (0.0..=100.0).contains(value))
}

fn finite_non_negative(value: Option<f64>) -> Option<f64> {
    value.filter(|value| value.is_finite() && *value >= 0.0)
}

const GIBIBYTE: f64 = 1024.0 * 1024.0 * 1024.0;

fn parse_percent(value: &str) -> Option<f64> {
    value.trim_end_matches('%').trim().parse::<f64>().ok()
}

fn valid_percent(value: f64) -> Option<f64> {
    value
        .is_finite()
        .then_some(value)
        .filter(|value| (0.0..=100.0).contains(value))
}

fn is_loopback_interface(name: &str) -> bool {
    name == "lo" || name.starts_with("lo:")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot() -> LiveSnapshot {
        LiveSnapshot {
            cpu: CpuLiveSample {
                utilization_percent: Some(42.0),
                frequency_hz: Some(3.2e9),
                temperature_celsius: Some(52.0),
                cores: 8,
            },
            memory_used_bytes: Some(6.8 * 1024.0 * 1024.0 * 1024.0),
            memory_available_bytes: Some(9.2 * 1024.0 * 1024.0 * 1024.0),
            memory_total_bytes: Some(16.0 * 1024.0 * 1024.0 * 1024.0),
            memory_swap_bytes: Some(0.0),
            memory_total_swap_bytes: Some(8.0 * 1024.0 * 1024.0 * 1024.0),
            gpu: None,
            download_bytes_per_sec: Some(0.0),
            upload_bytes_per_sec: Some(0.0),
            storage: Some(LiveStorageSnapshot {
                used_bytes: Some(680.0 * 1024.0 * 1024.0 * 1024.0),
                total_bytes: Some(1000.0 * 1024.0 * 1024.0 * 1024.0),
                read_bytes_per_sec: Some(8.0e6),
                write_bytes_per_sec: Some(2.0e6),
            }),
            hardware: HardwareInventory::default(),
        }
    }

    #[test]
    fn adapter_keeps_memory_storage_and_gpu_availability_explicit() {
        let mut history = LiveMetricsHistory::new();
        history.push_snapshot(Instant::now(), &snapshot()).unwrap();
        let models = history.models().unwrap();

        assert!(
            (models.memory().used().unwrap().value() - 6.8 * 1024.0 * 1024.0 * 1024.0).abs() < 1.0
        );
        assert!(models.memory().visual().primary().is_some());
        assert!(models.storage().read_rate().is_some());
        assert!(models.storage().write_rate().is_some());
        assert_eq!(models.gpu().state(), super::super::WidgetState::Unavailable);
    }

    #[test]
    fn adapter_keeps_real_directions_and_available_gpu_separate() {
        let mut history = LiveMetricsHistory::new();
        let mut current = snapshot();
        current.gpu = Some(LiveGpuSnapshot {
            utilization_percent: Some(37.0),
            vram_used_bytes: Some(2.0 * 1024.0 * 1024.0 * 1024.0),
            vram_total_bytes: Some(8.0 * 1024.0 * 1024.0 * 1024.0),
            temperature_celsius: Some(55.0),
            frequency_hz: Some(1.8e9),
            power_watts: Some(120.0),
        });
        current.download_bytes_per_sec = Some(48.0e6);
        current.upload_bytes_per_sec = Some(12.0e6);
        let start = Instant::now();
        history.push_snapshot(start, &current).unwrap();
        history
            .push_snapshot(start + Duration::from_secs(1), &current)
            .unwrap();
        let models = history.models().unwrap();

        assert_eq!(models.gpu().state(), super::super::WidgetState::Normal);
        assert_eq!(models.network().download().unwrap().value(), 48.0e6);
        assert_eq!(models.network().upload().unwrap().value(), 12.0e6);
        assert_eq!(
            models.network().download_sparkline().stats().current(),
            Some(48.0e6)
        );
        assert_eq!(
            models.network().upload_sparkline().stats().current(),
            Some(12.0e6)
        );
    }

    #[test]
    fn histories_are_fixed_and_latest_samples_are_rendered_at_the_right_edge() {
        let mut history = LiveMetricsHistory::new();
        let start = Instant::now();
        for index in 0..=LIVE_METRICS_HISTORY_CAPACITY {
            let mut sample = snapshot();
            sample.memory_used_bytes = Some((index + 1) as f64);
            sample.memory_available_bytes = Some(10_000.0 - (index + 1) as f64);
            sample.memory_total_bytes = Some(10_000.0);
            history
                .push_snapshot(start + Duration::from_secs(index as u64), &sample)
                .unwrap();
        }
        let models = history.models().unwrap();
        assert_eq!(models.memory().used().unwrap().value(), 121.0);
        assert_eq!(
            models.cpu().visual().widget_id(),
            super::super::DashboardWidgetId::Cpu
        );
    }
}
