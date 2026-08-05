mod chart;
mod engine;
mod layout;
mod metrics;
mod regions;
mod render;
mod terminal;
mod types;
mod visual;
mod widget;

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
pub use layout::{DashboardLayout, DashboardLayoutEngine, DashboardSection};
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
pub use regions::{DashboardComposition, DashboardPriority, DashboardRegion};
#[allow(unused_imports)]
pub use render::{
    BlockId, BlockMetadata, BlockRequirement, BlockVisibility, ChartBlock, ChartContent,
    FooterBlock, IndicatorBlock, MAX_RENDER_BLOCKS, RenderBlock, RenderError, StatusBlock,
    TitleBlock, TrendBlock, ValueBlock, WidgetRenderModel, WidgetRenderer,
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
