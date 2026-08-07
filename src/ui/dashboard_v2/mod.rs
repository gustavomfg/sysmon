mod chart;
mod engine;
mod health;
mod integration;
mod inventory;
mod layout;
mod live_cpu;
mod live_metrics;
mod metrics;
mod pulse_behavior;
mod regions;
mod render;
mod terminal;
mod types;
mod visual;
mod widget;
pub mod widgets;

#[allow(unused_imports)]
pub use chart::{
    AreaModel, ChartError, ChartEventKind, ChartPoint, ChartSegment, DownsampledSeries,
    EventMarker, MAX_REFERENCE_BANDS, MovingAverage, MovingAverageKind, PeakEvent, PeakSet,
    PointAggregation, PushOutcome, ReferenceBands, ReferenceKind, ReferenceRange, SegmentId,
    SegmentedBars, SeriesComparison, SeriesStats, SparklineModel, TimeSample, TimeSeries,
    TimeWindow, Volatility,
};
#[allow(unused_imports)]
pub use engine::{DashboardEngine, DashboardFrame};
#[allow(unused_imports)]
pub use health::{
    ComponentHealth, HealthComponent, HealthEngine, HealthPolicy, HealthSeverity, HealthSignals,
    HealthSnapshot, SystemHealth,
};
#[allow(unused_imports)]
pub(crate) use integration::{
    DashboardProcessSummary, DashboardV2Content, DashboardV2RenderError, DashboardV2RenderSummary,
    DashboardV2Renderer, DashboardV2SystemContext,
};
#[allow(unused_imports)]
pub use inventory::{
    GpuDevice, HardwareId, HardwareInventory, HardwareKind, NetworkInterface, StorageDevice,
    TemperatureSensor,
};
#[allow(unused_imports)]
pub use layout::{DashboardLayout, DashboardLayoutEngine, DashboardSection};
#[allow(unused_imports)]
pub use live_cpu::{CPU_LIVE_HISTORY_CAPACITY, CpuLiveHistory, CpuLiveSample};
#[allow(unused_imports)]
pub use live_metrics::{
    LIVE_METRICS_HISTORY_CAPACITY, LiveGpuSnapshot, LiveMetricModels, LiveMetricsError,
    LiveMetricsHistory, LiveSnapshot, LiveStorageSnapshot,
};
#[allow(unused_imports)]
pub use metrics::{
    CPU_TEMPERATURE_CRITICAL, CPU_TEMPERATURE_WARNING, CPU_THRESHOLDS, CpuMetricInput,
    CpuMetricModel, GPU_TEMPERATURE_CRITICAL, GPU_TEMPERATURE_WARNING, GPU_THRESHOLDS,
    GpuMetricInput, GpuMetricModel, MEMORY_THRESHOLDS, MemoryMetricInput, MemoryMetricModel,
    MetricModelError, MetricModelSummary, MetricReading, MetricThresholds, NETWORK_THRESHOLDS,
    NetworkMetricInput, NetworkMetricModel, STORAGE_ACTIVITY_WARNING,
    STORAGE_LATENCY_CRITICAL_SECONDS, STORAGE_LATENCY_WARNING_SECONDS, STORAGE_THRESHOLDS,
    StorageMetricInput, StorageMetricModel,
};
#[allow(unused_imports)]
pub use pulse_behavior::{PulseBehavior, PulseBehaviorEngine};
#[allow(unused_imports)]
pub use regions::{DashboardComposition, DashboardPriority, DashboardRegion};
#[allow(unused_imports)]
pub use render::{
    BlockId, BlockMetadata, BlockRequirement, BlockVisibility, ChartBlock, ChartContent,
    CpuContextBlock, FooterBlock, GpuContextBlock, IndicatorBlock, MAX_RENDER_BLOCKS,
    MemoryContextBlock, NetworkContextBlock, RenderBlock, RenderError, StatusBlock,
    StorageContextBlock, TitleBlock, TrendBlock, ValueBlock, WidgetRenderModel, WidgetRenderer,
};
#[allow(unused_imports)]
pub use terminal::{TerminalRenderError, TerminalRenderer};
#[allow(unused_imports)]
pub use types::{
    DashboardBounds, DashboardContext, DashboardDimensions, DashboardError, DashboardMinimum,
    DashboardPreset, MAX_DASHBOARD_CELLS, MAX_DASHBOARD_DIMENSION, MAX_DASHBOARD_REGIONS,
};
#[allow(unused_imports)]
pub use visual::{
    Highlight, Intensity, MetricAvailability, MetricIndicators, MetricUnit, MetricValue,
    MetricVisual, PeriodComparison, Trend, TrendDirection, VisualPriority,
};
#[allow(unused_imports)]
pub use widget::{
    DashboardState, DashboardWidgetId, MAX_DASHBOARD_WIDGETS, WidgetAvailability, WidgetContext,
    WidgetDescriptor, WidgetFrame, WidgetModel, WidgetPlacement, WidgetPresetSupport,
    WidgetRegistry, WidgetRequirements, WidgetSelection, WidgetState,
};
#[allow(unused_imports)]
pub use widgets::PrimaryWidget;
