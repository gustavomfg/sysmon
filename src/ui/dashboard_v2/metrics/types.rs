use super::super::chart::{ChartError, TimeSeries};
use super::super::{
    Highlight, Intensity, MetricAvailability, MetricIndicators, MetricUnit, MetricValue,
    MetricVisual, PeriodComparison, Trend, TrendDirection, VisualPriority, WidgetFrame,
    WidgetState,
};

pub const CHART_POINTS: usize = 12;
pub const MAX_PEAKS: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetricModelError {
    NonFiniteValue,
    NegativeValue,
    OutOfRange,
    InvalidTotal,
    InvalidComposition,
    MissingRequiredValue,
    HistoryUnitMismatch,
    Chart(ChartError),
}

impl From<ChartError> for MetricModelError {
    fn from(error: ChartError) -> Self {
        Self::Chart(error)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MetricReading {
    value: Option<f64>,
    availability: MetricAvailability,
}

impl MetricReading {
    pub fn new(
        value: Option<f64>,
        availability: MetricAvailability,
    ) -> Result<Self, MetricModelError> {
        if value.is_some_and(|value| !value.is_finite()) {
            return Err(MetricModelError::NonFiniteValue);
        }
        Ok(Self {
            value,
            availability,
        })
    }

    pub fn normalized(
        value: Option<f64>,
        availability: MetricAvailability,
    ) -> Result<Self, MetricModelError> {
        let reading = Self::new(value, availability)?;
        if reading
            .value
            .is_some_and(|value| !(0.0..=1.0).contains(&value))
        {
            return Err(MetricModelError::OutOfRange);
        }
        Ok(reading)
    }

    pub fn non_negative(
        value: Option<f64>,
        availability: MetricAvailability,
    ) -> Result<Self, MetricModelError> {
        let reading = Self::new(value, availability)?;
        if reading.value.is_some_and(|value| value < 0.0) {
            return Err(MetricModelError::NegativeValue);
        }
        Ok(reading)
    }

    pub fn available(value: f64) -> Result<Self, MetricModelError> {
        Self::new(Some(value), MetricAvailability::Available)
    }

    pub const fn unavailable() -> Self {
        Self {
            value: None,
            availability: MetricAvailability::Unavailable,
        }
    }

    pub const fn value(self) -> Option<f64> {
        self.value
    }

    pub const fn availability(self) -> MetricAvailability {
        self.availability
    }

    pub const fn is_usable(self) -> bool {
        self.value.is_some() && self.availability.is_usable()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MetricThresholds {
    active: f64,
    warning: f64,
    critical: f64,
}

impl MetricThresholds {
    pub const fn new(active: f64, warning: f64, critical: f64) -> Self {
        Self {
            active,
            warning,
            critical,
        }
    }

    pub const fn active(self) -> f64 {
        self.active
    }

    pub const fn warning(self) -> f64 {
        self.warning
    }

    pub const fn critical(self) -> f64 {
        self.critical
    }

    pub const fn is_valid(self) -> bool {
        self.active >= 0.0
            && self.active <= self.warning
            && self.warning <= self.critical
            && self.critical <= 1.0
    }

    pub fn classify(self, value: Option<f64>) -> WidgetState {
        match value {
            Some(value) if value >= self.critical => WidgetState::Critical,
            Some(value) if value >= self.warning => WidgetState::Warning,
            Some(value) if value >= self.active => WidgetState::Active,
            Some(_) => WidgetState::Normal,
            None => WidgetState::Unavailable,
        }
    }
}

pub const CPU_THRESHOLDS: MetricThresholds = MetricThresholds::new(0.55, 0.80, 0.95);
pub const MEMORY_THRESHOLDS: MetricThresholds = MetricThresholds::new(0.60, 0.80, 0.95);
pub const GPU_THRESHOLDS: MetricThresholds = MetricThresholds::new(0.55, 0.80, 0.95);
pub const NETWORK_THRESHOLDS: MetricThresholds = MetricThresholds::new(0.25, 0.60, 0.90);
pub const STORAGE_THRESHOLDS: MetricThresholds = MetricThresholds::new(0.70, 0.85, 0.95);

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MetricModelSummary {
    frame: WidgetFrame,
    visual: MetricVisual,
}

impl MetricModelSummary {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: super::super::DashboardWidgetId,
        state: WidgetState,
        primary: Option<MetricValue>,
        secondary: Option<MetricValue>,
        trend: Trend,
        comparison: PeriodComparison,
        intensity: Intensity,
        indicators: MetricIndicators,
        highlight: Highlight,
    ) -> Self {
        let mut visual = MetricVisual::new(id, state, VisualPriority::Primary)
            .with_trend(trend)
            .with_comparison(comparison)
            .with_intensity(intensity)
            .with_indicators(indicators)
            .with_highlight(highlight);
        if let Some(primary) = primary {
            visual = visual.with_primary(primary);
        }
        if let Some(secondary) = secondary {
            visual = visual.with_secondary(secondary);
        }
        Self {
            frame: WidgetFrame::new(id, state),
            visual,
        }
    }

    pub const fn frame(self) -> WidgetFrame {
        self.frame
    }

    pub const fn visual(self) -> MetricVisual {
        self.visual
    }

    pub const fn state(self) -> WidgetState {
        self.frame.state()
    }

    pub const fn widget_id(self) -> super::super::DashboardWidgetId {
        self.frame.id()
    }

    pub fn is_available(self) -> bool {
        self.visual.is_available()
    }
}

pub fn contextual_metric_value(
    reading: MetricReading,
    unit: MetricUnit,
    priority: VisualPriority,
) -> Option<MetricValue> {
    reading
        .value()
        .and_then(|value| MetricValue::new(value, unit, 1, priority))
        .map(|value| value.with_availability(reading.availability()))
}

pub fn usable_metric_value(
    reading: MetricReading,
    unit: MetricUnit,
    priority: VisualPriority,
) -> Option<MetricValue> {
    reading
        .is_usable()
        .then(|| contextual_metric_value(reading, unit, priority))
        .flatten()
}

pub fn validate_history<const CAPACITY: usize>(
    history: TimeSeries<CAPACITY>,
    unit: MetricUnit,
) -> Result<(), MetricModelError> {
    (history.unit() == unit)
        .then_some(())
        .ok_or(MetricModelError::HistoryUnitMismatch)
}

pub fn validate_normalized_reading(reading: MetricReading) -> Result<(), MetricModelError> {
    if reading
        .value()
        .is_some_and(|value| !(0.0..=1.0).contains(&value))
    {
        return Err(MetricModelError::OutOfRange);
    }
    Ok(())
}

pub fn validate_non_negative_reading(reading: MetricReading) -> Result<(), MetricModelError> {
    if reading.value().is_some_and(|value| value < 0.0) {
        return Err(MetricModelError::NegativeValue);
    }
    Ok(())
}

pub fn latest_comparison<const CAPACITY: usize>(
    history: TimeSeries<CAPACITY>,
    unit: MetricUnit,
) -> PeriodComparison {
    let mut previous = None;
    let mut current = None;
    for sample in history.iter() {
        let Some(value) = sample.usable_value() else {
            continue;
        };
        let Some(metric) = MetricValue::new(value, unit, 1, VisualPriority::Primary) else {
            continue;
        };
        previous = current;
        current = Some(metric);
    }
    current
        .map(|current| PeriodComparison::from_values(current, previous))
        .unwrap_or_else(PeriodComparison::unknown)
}

pub fn intensity_for(state: WidgetState, volatility: Intensity) -> Intensity {
    match state {
        WidgetState::Critical => Intensity::VeryHigh,
        WidgetState::Warning => Intensity::High,
        WidgetState::Active => match volatility {
            Intensity::VeryHigh | Intensity::High => Intensity::High,
            _ => Intensity::Normal,
        },
        WidgetState::Normal => match volatility {
            Intensity::VeryHigh => Intensity::High,
            Intensity::High => Intensity::Normal,
            _ => Intensity::Low,
        },
        WidgetState::Idle => Intensity::VeryLow,
        WidgetState::Unavailable | WidgetState::Loading => Intensity::VeryLow,
    }
}

pub fn highlight_for(state: WidgetState) -> Highlight {
    match state {
        WidgetState::Critical => Highlight::Critical,
        WidgetState::Warning => Highlight::Strong,
        WidgetState::Active => Highlight::Subtle,
        _ => Highlight::None,
    }
}

pub fn classify_with_signal(
    thresholds: MetricThresholds,
    value: Option<f64>,
    rising_signal: bool,
    sustained_warning: bool,
) -> WidgetState {
    let state = thresholds.classify(value);
    match state {
        WidgetState::Active if sustained_warning => WidgetState::Warning,
        WidgetState::Normal if sustained_warning => WidgetState::Warning,
        WidgetState::Normal
            if rising_signal && value.is_some_and(|value| value >= thresholds.active() * 0.80) =>
        {
            WidgetState::Active
        }
        _ => state,
    }
}

pub fn sustained_above<const CAPACITY: usize>(
    history: TimeSeries<CAPACITY>,
    threshold: f64,
    minimum_samples: usize,
) -> bool {
    let mut index = history.len();
    let mut found = 0;
    while index > 0 && found < minimum_samples {
        index -= 1;
        let Some(value) = history.get(index).and_then(|sample| sample.usable_value()) else {
            continue;
        };
        if value < threshold {
            return false;
        }
        found += 1;
    }
    found >= minimum_samples
}

pub fn peak_floor<const CAPACITY: usize>(history: TimeSeries<CAPACITY>, fallback: f64) -> f64 {
    history
        .stats()
        .maximum()
        .map_or(fallback, |maximum| maximum * 0.75)
}

pub fn validate_non_negative_pair(
    first: MetricReading,
    second: MetricReading,
) -> Result<(), MetricModelError> {
    if first.value().is_some_and(|value| value < 0.0)
        || second.value().is_some_and(|value| value < 0.0)
    {
        return Err(MetricModelError::NegativeValue);
    }
    Ok(())
}

pub fn validate_capacity_pair(
    used: MetricReading,
    total: MetricReading,
) -> Result<(), MetricModelError> {
    if total.is_usable() && total.value().is_some_and(|value| value <= 0.0) {
        return Err(MetricModelError::InvalidTotal);
    }
    if used.is_usable()
        && total.is_usable()
        && used
            .value()
            .zip(total.value())
            .is_some_and(|(used, total)| used > total)
    {
        return Err(MetricModelError::InvalidTotal);
    }
    Ok(())
}

pub fn trend_of<const CAPACITY: usize>(history: TimeSeries<CAPACITY>) -> Trend {
    history
        .sparkline::<CHART_POINTS>(super::super::chart::TimeWindow::Medium)
        .trend()
}

pub fn volatility_of<const CAPACITY: usize>(history: TimeSeries<CAPACITY>) -> Intensity {
    history
        .volatility(super::super::chart::TimeWindow::Medium)
        .intensity()
}

pub fn rising(trend: Trend) -> bool {
    trend.direction() == TrendDirection::Rising
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thresholds_are_centralized_and_ordered() {
        assert!(CPU_THRESHOLDS.is_valid());
        assert!(MEMORY_THRESHOLDS.is_valid());
        assert!(GPU_THRESHOLDS.is_valid());
        assert!(NETWORK_THRESHOLDS.is_valid());
        assert!(STORAGE_THRESHOLDS.is_valid());
        assert_eq!(CPU_THRESHOLDS.classify(Some(0.99)), WidgetState::Critical);
    }

    #[test]
    fn readings_preserve_availability_and_reject_invalid_values() {
        assert!(matches!(
            MetricReading::new(Some(f64::NAN), MetricAvailability::Available),
            Err(MetricModelError::NonFiniteValue)
        ));
        assert!(matches!(
            MetricReading::normalized(Some(1.1), MetricAvailability::Available),
            Err(MetricModelError::OutOfRange)
        ));
        let missing = MetricReading::unavailable();
        assert_eq!(missing.value(), None);
        assert!(!missing.is_usable());
    }
}
