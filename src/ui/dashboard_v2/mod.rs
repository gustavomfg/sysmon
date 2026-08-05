mod engine;
mod layout;
mod regions;
mod types;
mod visual;
mod widget;

#[allow(unused_imports)]
pub use engine::{DashboardEngine, DashboardFrame};
#[allow(unused_imports)]
pub use layout::{DashboardLayout, DashboardLayoutEngine, DashboardSection};
#[allow(unused_imports)]
pub use regions::{DashboardComposition, DashboardPriority, DashboardRegion};
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
