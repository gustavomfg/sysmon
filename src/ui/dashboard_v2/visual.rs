use super::{DashboardWidgetId, WidgetFrame, WidgetState};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetricUnit {
    None,
    Percent,
    Bytes,
    BytesPerSecond,
    Count,
    DurationSeconds,
    TemperatureCelsius,
    FrequencyHertz,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetricAvailability {
    Available,
    Unavailable,
    Loading,
    Stale,
}

impl MetricAvailability {
    pub const fn is_usable(self) -> bool {
        matches!(self, Self::Available | Self::Stale)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum VisualPriority {
    Hero,
    Primary,
    Secondary,
    Contextual,
    Auxiliary,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MetricValue {
    value: f64,
    unit: MetricUnit,
    precision: u8,
    relevance: VisualPriority,
    availability: MetricAvailability,
}

impl MetricValue {
    pub fn new(
        value: f64,
        unit: MetricUnit,
        precision: u8,
        relevance: VisualPriority,
    ) -> Option<Self> {
        value.is_finite().then_some(Self {
            value,
            unit,
            precision,
            relevance,
            availability: MetricAvailability::Available,
        })
    }

    pub const fn value(self) -> f64 {
        self.value
    }

    pub const fn unit(self) -> MetricUnit {
        self.unit
    }

    pub const fn precision(self) -> u8 {
        self.precision
    }

    pub const fn relevance(self) -> VisualPriority {
        self.relevance
    }

    pub const fn availability(self) -> MetricAvailability {
        self.availability
    }

    pub const fn with_availability(self, availability: MetricAvailability) -> Self {
        Self {
            availability,
            ..self
        }
    }

    pub const fn is_usable(self) -> bool {
        self.availability.is_usable()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrendDirection {
    Rising,
    Falling,
    Stable,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Trend {
    direction: TrendDirection,
}

impl Trend {
    pub const fn unknown() -> Self {
        Self {
            direction: TrendDirection::Unknown,
        }
    }

    pub const fn new(direction: TrendDirection) -> Self {
        Self { direction }
    }

    pub const fn direction(self) -> TrendDirection {
        self.direction
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PeriodComparison {
    direction: TrendDirection,
    delta: Option<f64>,
    window: Option<std::time::Duration>,
}

impl PeriodComparison {
    pub const fn unknown() -> Self {
        Self {
            direction: TrendDirection::Unknown,
            delta: None,
            window: None,
        }
    }

    pub fn from_values(current: MetricValue, previous: Option<MetricValue>) -> Self {
        let Some(previous) = previous else {
            return Self::unknown();
        };
        if current.unit() != previous.unit() || !current.is_usable() || !previous.is_usable() {
            return Self::unknown();
        }

        let delta = current.value() - previous.value();
        if !delta.is_finite() {
            return Self::unknown();
        }
        Self {
            direction: direction_for_delta(delta),
            delta: Some(delta),
            window: None,
        }
    }

    pub const fn direction(self) -> TrendDirection {
        self.direction
    }

    pub const fn delta(self) -> Option<f64> {
        self.delta
    }

    pub const fn has_previous(self) -> bool {
        self.delta.is_some()
    }

    pub const fn with_window(mut self, window: std::time::Duration) -> Self {
        self.window = Some(window);
        self
    }
    pub const fn window(self) -> Option<std::time::Duration> {
        self.window
    }
}

fn direction_for_delta(delta: f64) -> TrendDirection {
    const STABILITY_EPSILON: f64 = 1.0e-9;
    if delta > STABILITY_EPSILON {
        TrendDirection::Rising
    } else if delta < -STABILITY_EPSILON {
        TrendDirection::Falling
    } else {
        TrendDirection::Stable
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Intensity {
    VeryLow,
    Low,
    Normal,
    High,
    VeryHigh,
}

impl Intensity {
    pub const fn from_level(level: u8) -> Self {
        match level {
            0 => Self::VeryLow,
            1 => Self::Low,
            2 => Self::Normal,
            3 => Self::High,
            _ => Self::VeryHigh,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MetricIndicators(u8);

impl MetricIndicators {
    pub const PROGRESS: Self = Self(1 << 0);
    pub const OCCUPANCY: Self = Self(1 << 1);
    pub const TREND: Self = Self(1 << 2);
    pub const VARIATION: Self = Self(1 << 3);
    pub const ACTIVITY: Self = Self(1 << 4);

    pub const fn empty() -> Self {
        Self(0)
    }

    pub const fn with(self, indicator: Self) -> Self {
        Self(self.0 | indicator.0)
    }

    pub const fn contains(self, indicator: Self) -> bool {
        self.0 & indicator.0 == indicator.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Highlight {
    None,
    Subtle,
    Strong,
    Critical,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MetricVisual {
    widget_id: DashboardWidgetId,
    primary: Option<MetricValue>,
    secondary: Option<MetricValue>,
    trend: Trend,
    comparison: PeriodComparison,
    intensity: Intensity,
    priority: VisualPriority,
    state: WidgetState,
    indicators: MetricIndicators,
    highlight: Highlight,
}

impl MetricVisual {
    pub const fn new(
        widget_id: DashboardWidgetId,
        state: WidgetState,
        priority: VisualPriority,
    ) -> Self {
        Self {
            widget_id,
            primary: None,
            secondary: None,
            trend: Trend::unknown(),
            comparison: PeriodComparison::unknown(),
            intensity: Intensity::Normal,
            priority,
            state,
            indicators: MetricIndicators::empty(),
            highlight: Highlight::None,
        }
    }

    pub const fn from_widget(frame: WidgetFrame, priority: VisualPriority) -> Self {
        Self::new(frame.id(), frame.state(), priority)
    }

    pub const fn widget_id(self) -> DashboardWidgetId {
        self.widget_id
    }

    pub const fn primary(self) -> Option<MetricValue> {
        self.primary
    }

    pub const fn secondary(self) -> Option<MetricValue> {
        self.secondary
    }

    pub const fn trend(self) -> Trend {
        self.trend
    }

    pub const fn comparison(self) -> PeriodComparison {
        self.comparison
    }

    pub const fn intensity(self) -> Intensity {
        self.intensity
    }

    pub const fn priority(self) -> VisualPriority {
        self.priority
    }

    pub const fn state(self) -> WidgetState {
        self.state
    }

    pub const fn indicators(self) -> MetricIndicators {
        self.indicators
    }

    pub const fn highlight(self) -> Highlight {
        self.highlight
    }

    pub const fn with_primary(self, primary: MetricValue) -> Self {
        Self {
            primary: Some(primary),
            ..self
        }
    }

    pub const fn with_secondary(self, secondary: MetricValue) -> Self {
        Self {
            secondary: Some(secondary),
            ..self
        }
    }

    pub const fn with_trend(self, trend: Trend) -> Self {
        Self { trend, ..self }
    }

    pub const fn with_comparison(self, comparison: PeriodComparison) -> Self {
        Self { comparison, ..self }
    }

    pub const fn with_intensity(self, intensity: Intensity) -> Self {
        Self { intensity, ..self }
    }

    pub const fn with_indicators(self, indicators: MetricIndicators) -> Self {
        Self { indicators, ..self }
    }

    pub const fn with_highlight(self, highlight: Highlight) -> Self {
        Self { highlight, ..self }
    }

    pub fn is_available(self) -> bool {
        self.primary.is_some_and(MetricValue::is_usable)
            || self.secondary.is_some_and(MetricValue::is_usable)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn value(value: f64, unit: MetricUnit) -> MetricValue {
        MetricValue::new(value, unit, 1, VisualPriority::Primary).unwrap()
    }

    #[test]
    fn metric_values_keep_context_and_reject_non_finite_input() {
        let reading = value(72.5, MetricUnit::Percent).with_availability(MetricAvailability::Stale);
        assert_eq!(reading.value(), 72.5);
        assert_eq!(reading.unit(), MetricUnit::Percent);
        assert_eq!(reading.precision(), 1);
        assert_eq!(reading.relevance(), VisualPriority::Primary);
        assert!(reading.is_usable());
        assert!(
            MetricValue::new(f64::NAN, MetricUnit::None, 0, VisualPriority::Auxiliary).is_none()
        );
        assert!(
            MetricValue::new(
                f64::INFINITY,
                MetricUnit::None,
                0,
                VisualPriority::Auxiliary
            )
            .is_none()
        );
    }

    #[test]
    fn trends_and_period_comparisons_are_semantic_and_deterministic() {
        let current = value(80.0, MetricUnit::Percent);
        let previous = value(50.0, MetricUnit::Percent);
        let comparison = PeriodComparison::from_values(current, Some(previous));
        assert_eq!(comparison.direction(), TrendDirection::Rising);
        assert_eq!(comparison.delta(), Some(30.0));
        assert!(comparison.has_previous());
        assert_eq!(
            PeriodComparison::from_values(current, None).direction(),
            TrendDirection::Unknown
        );
        assert_eq!(
            PeriodComparison::from_values(current, Some(value(1.0, MetricUnit::Bytes))).direction(),
            TrendDirection::Unknown
        );
        assert_eq!(
            PeriodComparison::from_values(current, Some(value(80.0, MetricUnit::Percent)))
                .direction(),
            TrendDirection::Stable
        );
        assert_eq!(
            Trend::new(TrendDirection::Falling).direction(),
            TrendDirection::Falling
        );
    }

    #[test]
    fn intensity_and_priority_have_ordered_semantic_levels() {
        assert!(Intensity::VeryLow < Intensity::Low);
        assert!(Intensity::Low < Intensity::Normal);
        assert!(Intensity::Normal < Intensity::High);
        assert!(Intensity::High < Intensity::VeryHigh);
        assert_eq!(Intensity::from_level(0), Intensity::VeryLow);
        assert_eq!(Intensity::from_level(99), Intensity::VeryHigh);
        assert!(VisualPriority::Hero < VisualPriority::Primary);
        assert!(VisualPriority::Primary < VisualPriority::Secondary);
        assert!(VisualPriority::Secondary < VisualPriority::Contextual);
        assert!(VisualPriority::Contextual < VisualPriority::Auxiliary);
    }

    #[test]
    fn indicators_are_reusable_without_deciding_their_rendering() {
        let indicators = MetricIndicators::empty()
            .with(MetricIndicators::PROGRESS)
            .with(MetricIndicators::TREND)
            .with(MetricIndicators::ACTIVITY);
        assert!(indicators.contains(MetricIndicators::PROGRESS));
        assert!(indicators.contains(MetricIndicators::TREND));
        assert!(indicators.contains(MetricIndicators::ACTIVITY));
        assert!(!indicators.contains(MetricIndicators::OCCUPANCY));
    }

    #[test]
    fn metric_visual_composes_primary_secondary_and_semantic_state() {
        let visual = MetricVisual::new(
            DashboardWidgetId::Cpu,
            WidgetState::Active,
            VisualPriority::Primary,
        )
        .with_primary(value(64.0, MetricUnit::Percent))
        .with_secondary(value(4.0, MetricUnit::Count))
        .with_trend(Trend::new(TrendDirection::Rising))
        .with_intensity(Intensity::High)
        .with_indicators(
            MetricIndicators::PROGRESS
                .with(MetricIndicators::TREND)
                .with(MetricIndicators::ACTIVITY),
        )
        .with_highlight(Highlight::Strong);

        assert_eq!(visual.widget_id(), DashboardWidgetId::Cpu);
        assert_eq!(visual.primary().unwrap().unit(), MetricUnit::Percent);
        assert_eq!(visual.secondary().unwrap().unit(), MetricUnit::Count);
        assert_eq!(visual.trend().direction(), TrendDirection::Rising);
        assert_eq!(visual.intensity(), Intensity::High);
        assert_eq!(visual.priority(), VisualPriority::Primary);
        assert_eq!(visual.state(), WidgetState::Active);
        assert_eq!(visual.highlight(), Highlight::Strong);
        assert!(visual.is_available());
    }

    #[test]
    fn visual_model_is_compatible_with_widget_frame() {
        let frame = WidgetFrame::new(DashboardWidgetId::Pulse, WidgetState::Normal);
        let first = MetricVisual::from_widget(frame, VisualPriority::Hero);
        let second = MetricVisual::from_widget(frame, VisualPriority::Hero);
        assert_eq!(first, second);
        assert_eq!(first.widget_id(), DashboardWidgetId::Pulse);
        assert_eq!(first.state(), WidgetState::Normal);
        assert_eq!(first.priority(), VisualPriority::Hero);
        assert!(!first.is_available());
    }

    #[test]
    fn visual_availability_is_independent_from_terminal_appearance() {
        let unavailable =
            value(0.0, MetricUnit::Percent).with_availability(MetricAvailability::Unavailable);
        let visual = MetricVisual::new(
            DashboardWidgetId::Memory,
            WidgetState::Unavailable,
            VisualPriority::Primary,
        )
        .with_primary(unavailable);
        assert!(!visual.primary().unwrap().is_usable());
        assert!(!visual.is_available());
        assert_eq!(visual.highlight(), Highlight::None);
    }
}
