mod cpu;
mod gpu;
mod memory;
mod network;
mod storage;
mod types;

pub use cpu::{CPU_TEMPERATURE_CRITICAL, CPU_TEMPERATURE_WARNING, CpuMetricInput, CpuMetricModel};
pub use gpu::{GPU_TEMPERATURE_CRITICAL, GPU_TEMPERATURE_WARNING, GpuMetricInput, GpuMetricModel};
pub use memory::{MemoryMetricInput, MemoryMetricModel};
pub use network::{NetworkMetricInput, NetworkMetricModel};
pub use storage::{
    STORAGE_ACTIVITY_WARNING, STORAGE_LATENCY_CRITICAL_SECONDS, STORAGE_LATENCY_WARNING_SECONDS,
    StorageMetricInput, StorageMetricModel,
};
pub use types::{
    CHART_POINTS, CPU_THRESHOLDS, GPU_THRESHOLDS, MAX_PEAKS, MEMORY_THRESHOLDS, MetricModelError,
    MetricModelSummary, MetricReading, MetricThresholds, NETWORK_THRESHOLDS, STORAGE_THRESHOLDS,
};
