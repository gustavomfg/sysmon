use super::super::chart::{
    ChartSegment, PeakSet, SegmentedBars, SeriesComparison, SparklineModel, TimeSeries, TimeWindow,
};
use super::super::{
    DashboardWidgetId, MetricIndicators, MetricUnit, MetricValue, PeriodComparison, Trend,
    VisualPriority, WidgetState,
};
use super::types::{
    CHART_POINTS, MAX_PEAKS, MetricModelError, MetricModelSummary, MetricReading,
    STORAGE_THRESHOLDS, classify_with_signal, contextual_metric_value, highlight_for,
    intensity_for, latest_comparison, peak_floor, rising, trend_of, usable_metric_value,
    validate_capacity_pair, validate_history, validate_non_negative_reading,
    validate_normalized_reading,
};

pub const STORAGE_ACTIVITY_WARNING: f64 = 0.85;
pub const STORAGE_LATENCY_WARNING_SECONDS: f64 = 0.10;
pub const STORAGE_LATENCY_CRITICAL_SECONDS: f64 = 0.50;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StorageMetricInput<const HISTORY: usize> {
    used_space: MetricReading,
    total_space: MetricReading,
    read_rate: MetricReading,
    write_rate: MetricReading,
    latency_seconds: MetricReading,
    activity: MetricReading,
    occupancy_history: TimeSeries<HISTORY>,
    read_history: TimeSeries<HISTORY>,
    write_history: TimeSeries<HISTORY>,
}

impl<const HISTORY: usize> StorageMetricInput<HISTORY> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        used_space: MetricReading,
        total_space: MetricReading,
        read_rate: MetricReading,
        write_rate: MetricReading,
        latency_seconds: MetricReading,
        activity: MetricReading,
        occupancy_history: TimeSeries<HISTORY>,
        read_history: TimeSeries<HISTORY>,
        write_history: TimeSeries<HISTORY>,
    ) -> Result<Self, MetricModelError> {
        for reading in [
            used_space,
            total_space,
            read_rate,
            write_rate,
            latency_seconds,
        ] {
            validate_non_negative_reading(reading)?;
        }
        validate_normalized_reading(activity)?;
        validate_capacity_pair(used_space, total_space)?;
        validate_history(occupancy_history, MetricUnit::Percent)?;
        validate_history(read_history, MetricUnit::BytesPerSecond)?;
        validate_history(write_history, MetricUnit::BytesPerSecond)?;
        Ok(Self {
            used_space,
            total_space,
            read_rate,
            write_rate,
            latency_seconds,
            activity,
            occupancy_history,
            read_history,
            write_history,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StorageMetricModel<const HISTORY: usize> {
    summary: MetricModelSummary,
    occupancy_sparkline: SparklineModel<CHART_POINTS>,
    read_sparkline: SparklineModel<CHART_POINTS>,
    write_sparkline: SparklineModel<CHART_POINTS>,
    capacity: SegmentedBars<2>,
    comparison: SeriesComparison,
    peaks: PeakSet<MAX_PEAKS>,
    read_peaks: PeakSet<MAX_PEAKS>,
    write_peaks: PeakSet<MAX_PEAKS>,
    used_space: Option<MetricValue>,
    total_space: Option<MetricValue>,
    read_rate: Option<MetricValue>,
    write_rate: Option<MetricValue>,
    latency: Option<MetricValue>,
    activity: Option<MetricValue>,
}

impl<const HISTORY: usize> StorageMetricModel<HISTORY> {
    pub fn from_input(input: StorageMetricInput<HISTORY>) -> Result<Self, MetricModelError> {
        let occupancy_sparkline = input
            .occupancy_history
            .sparkline::<CHART_POINTS>(TimeWindow::Long);
        let read_sparkline = input
            .read_history
            .sparkline::<CHART_POINTS>(TimeWindow::Long);
        let write_sparkline = input
            .write_history
            .sparkline::<CHART_POINTS>(TimeWindow::Long);
        let trend = trend_of(input.occupancy_history);
        let visual_comparison = latest_comparison(input.occupancy_history, MetricUnit::Percent);
        let occupancy = input
            .used_space
            .value()
            .zip(input.total_space.value())
            .filter(|(_, total)| *total > 0.0)
            .map(|(used, total)| used / total)
            .filter(|value| value.is_finite());
        let activity = input
            .activity
            .value()
            .filter(|_| input.activity.is_usable());
        let mut state = classify_with_signal(STORAGE_THRESHOLDS, occupancy, rising(trend), false);
        if state == WidgetState::Normal {
            state = if activity.is_some_and(|value| value >= STORAGE_ACTIVITY_WARNING) {
                WidgetState::Warning
            } else if activity.is_some_and(|value| value >= STORAGE_THRESHOLDS.active()) {
                WidgetState::Active
            } else {
                state
            };
        }
        if input
            .latency_seconds
            .value()
            .is_some_and(|latency| latency >= STORAGE_LATENCY_CRITICAL_SECONDS)
        {
            state = WidgetState::Critical;
        } else if state != WidgetState::Critical
            && input
                .latency_seconds
                .value()
                .is_some_and(|latency| latency >= STORAGE_LATENCY_WARNING_SECONDS)
        {
            state = WidgetState::Warning;
        }
        if occupancy.is_none() && activity.is_none() {
            state = WidgetState::Unavailable;
        }

        let used_space =
            contextual_metric_value(input.used_space, MetricUnit::Bytes, VisualPriority::Primary);
        let total_space = contextual_metric_value(
            input.total_space,
            MetricUnit::Bytes,
            VisualPriority::Contextual,
        );
        let read_rate = contextual_metric_value(
            input.read_rate,
            MetricUnit::BytesPerSecond,
            VisualPriority::Secondary,
        );
        let write_rate = contextual_metric_value(
            input.write_rate,
            MetricUnit::BytesPerSecond,
            VisualPriority::Secondary,
        );
        let latency = contextual_metric_value(
            input.latency_seconds,
            MetricUnit::DurationSeconds,
            VisualPriority::Contextual,
        );
        let activity = usable_metric_value(
            input.activity,
            MetricUnit::Percent,
            VisualPriority::Secondary,
        );
        let primary = occupancy.and_then(|occupancy| {
            MetricValue::new(occupancy, MetricUnit::Percent, 1, VisualPriority::Primary)
        });
        let volatility = occupancy_sparkline.volatility().intensity();
        let indicators = MetricIndicators::empty()
            .with(MetricIndicators::PROGRESS)
            .with(MetricIndicators::OCCUPANCY)
            .with(MetricIndicators::TREND)
            .with(MetricIndicators::ACTIVITY);
        let summary = MetricModelSummary::new(
            DashboardWidgetId::Storage,
            state,
            primary,
            activity,
            trend,
            visual_comparison,
            intensity_for(state, volatility),
            indicators,
            highlight_for(state),
        );
        let capacity = build_capacity(used_space, total_space)?;
        let comparison = input.read_history.compare(input.write_history)?;
        let peaks = input.occupancy_history.detect_peaks::<MAX_PEAKS>(
            TimeWindow::Long,
            peak_floor(input.occupancy_history, STORAGE_THRESHOLDS.warning()),
        )?;
        let read_peaks = input
            .read_history
            .detect_peaks::<MAX_PEAKS>(TimeWindow::Long, peak_floor(input.read_history, 1.0))?;
        let write_peaks = input
            .write_history
            .detect_peaks::<MAX_PEAKS>(TimeWindow::Long, peak_floor(input.write_history, 1.0))?;
        Ok(Self {
            summary,
            occupancy_sparkline,
            read_sparkline,
            write_sparkline,
            capacity,
            comparison,
            peaks,
            read_peaks,
            write_peaks,
            used_space,
            total_space,
            read_rate,
            write_rate,
            latency,
            activity,
        })
    }

    pub const fn frame(self) -> super::super::WidgetFrame {
        self.summary.frame()
    }

    pub const fn visual(self) -> super::super::MetricVisual {
        self.summary.visual()
    }

    pub const fn occupancy_sparkline(self) -> SparklineModel<CHART_POINTS> {
        self.occupancy_sparkline
    }

    pub const fn read_sparkline(self) -> SparklineModel<CHART_POINTS> {
        self.read_sparkline
    }

    pub const fn write_sparkline(self) -> SparklineModel<CHART_POINTS> {
        self.write_sparkline
    }

    pub const fn capacity(self) -> SegmentedBars<2> {
        self.capacity
    }

    pub const fn comparison(self) -> SeriesComparison {
        self.comparison
    }

    pub const fn peaks(self) -> PeakSet<MAX_PEAKS> {
        self.peaks
    }

    pub const fn read_peaks(self) -> PeakSet<MAX_PEAKS> {
        self.read_peaks
    }

    pub const fn write_peaks(self) -> PeakSet<MAX_PEAKS> {
        self.write_peaks
    }

    pub const fn used_space(self) -> Option<MetricValue> {
        self.used_space
    }

    pub const fn total_space(self) -> Option<MetricValue> {
        self.total_space
    }

    pub const fn read_rate(self) -> Option<MetricValue> {
        self.read_rate
    }

    pub const fn write_rate(self) -> Option<MetricValue> {
        self.write_rate
    }

    pub const fn latency(self) -> Option<MetricValue> {
        self.latency
    }

    pub const fn activity(self) -> Option<MetricValue> {
        self.activity
    }

    pub const fn state(self) -> WidgetState {
        self.summary.state()
    }

    pub const fn trend(self) -> Trend {
        self.summary.visual().trend()
    }

    pub const fn visual_comparison(self) -> PeriodComparison {
        self.summary.visual().comparison()
    }

    pub fn is_available(self) -> bool {
        self.summary.is_available()
    }
}

fn build_capacity(
    used: Option<MetricValue>,
    total: Option<MetricValue>,
) -> Result<SegmentedBars<2>, MetricModelError> {
    let mut capacity = SegmentedBars::new(total)?;
    capacity.push(ChartSegment::new(
        super::super::chart::SegmentId::Used,
        used,
    ))?;
    let free = match (
        used.filter(|value| value.is_usable()),
        total.filter(|value| value.is_usable()),
    ) {
        (Some(used), Some(total)) => Some(
            MetricValue::new(
                (total.value() - used.value()).max(0.0),
                MetricUnit::Bytes,
                1,
                VisualPriority::Secondary,
            )
            .ok_or(MetricModelError::MissingRequiredValue)?,
        ),
        _ => None,
    };
    capacity.push(ChartSegment::new(
        super::super::chart::SegmentId::Free,
        free,
    ))?;
    Ok(capacity)
}

#[cfg(test)]
mod tests {
    use super::super::super::MetricAvailability;
    use super::super::super::TrendDirection;
    use super::super::super::chart::TimeSample;
    use super::super::types::MetricReading;
    use super::*;
    use std::time::Duration;

    fn occupancy(values: &[f64]) -> TimeSeries<8> {
        let mut history = TimeSeries::new(MetricUnit::Percent);
        for (index, value) in values.iter().copied().enumerate() {
            history
                .push(
                    TimeSample::new(
                        Duration::from_secs(index as u64),
                        Some(value),
                        MetricAvailability::Available,
                        None,
                    )
                    .unwrap(),
                )
                .unwrap();
        }
        history
    }

    fn rates(values: &[f64]) -> TimeSeries<8> {
        let mut history = TimeSeries::new(MetricUnit::BytesPerSecond);
        for (index, value) in values.iter().copied().enumerate() {
            history
                .push(
                    TimeSample::new(
                        Duration::from_secs(index as u64),
                        Some(value),
                        MetricAvailability::Available,
                        None,
                    )
                    .unwrap(),
                )
                .unwrap();
        }
        history
    }

    fn input(used: f64, total: f64, activity: f64) -> StorageMetricInput<8> {
        StorageMetricInput::new(
            MetricReading::non_negative(Some(used), MetricAvailability::Available).unwrap(),
            MetricReading::non_negative(Some(total), MetricAvailability::Available).unwrap(),
            MetricReading::non_negative(Some(80.0), MetricAvailability::Available).unwrap(),
            MetricReading::non_negative(Some(20.0), MetricAvailability::Available).unwrap(),
            MetricReading::non_negative(Some(0.02), MetricAvailability::Available).unwrap(),
            MetricReading::normalized(Some(activity), MetricAvailability::Available).unwrap(),
            occupancy(&[used / total * 0.8, used / total]),
            rates(&[40.0, 80.0]),
            rates(&[10.0, 20.0]),
        )
        .unwrap()
    }

    #[test]
    fn storage_separates_capacity_from_io_activity() {
        let model = StorageMetricModel::from_input(input(50.0, 100.0, 0.20)).unwrap();
        assert_eq!(model.state(), WidgetState::Normal);
        assert_eq!(model.visual().widget_id(), DashboardWidgetId::Storage);
        assert_eq!(model.capacity().len(), 2);
        assert_eq!(model.comparison().direction(), TrendDirection::Rising);
        assert_eq!(model.visual().primary().unwrap().value(), 0.5);
    }

    #[test]
    fn storage_low_space_and_intense_activity_are_semantic_states() {
        let low_space = StorageMetricModel::from_input(input(96.0, 100.0, 0.20)).unwrap();
        assert_eq!(low_space.state(), WidgetState::Critical);
        let busy = StorageMetricModel::from_input(input(50.0, 100.0, 0.95)).unwrap();
        assert_eq!(busy.state(), WidgetState::Warning);
        assert_eq!(busy.read_rate().unwrap().unit(), MetricUnit::BytesPerSecond);
        assert_eq!(busy.latency().unwrap().unit(), MetricUnit::DurationSeconds);
    }

    #[test]
    fn storage_rejects_invalid_capacity_and_preserves_missing_metrics() {
        let invalid = StorageMetricInput::new(
            MetricReading::non_negative(Some(120.0), MetricAvailability::Available).unwrap(),
            MetricReading::non_negative(Some(100.0), MetricAvailability::Available).unwrap(),
            MetricReading::unavailable(),
            MetricReading::unavailable(),
            MetricReading::unavailable(),
            MetricReading::unavailable(),
            occupancy(&[]),
            rates(&[]),
            rates(&[]),
        );
        assert!(matches!(invalid, Err(MetricModelError::InvalidTotal)));

        let missing = StorageMetricInput::new(
            MetricReading::unavailable(),
            MetricReading::unavailable(),
            MetricReading::unavailable(),
            MetricReading::unavailable(),
            MetricReading::unavailable(),
            MetricReading::unavailable(),
            occupancy(&[]),
            rates(&[]),
            rates(&[]),
        )
        .unwrap();
        assert_eq!(
            StorageMetricModel::from_input(missing).unwrap().state(),
            WidgetState::Unavailable
        );
    }
}
