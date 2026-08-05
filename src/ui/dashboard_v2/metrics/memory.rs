use super::super::chart::{
    AreaModel, ChartSegment, PeakSet, ReferenceBands, ReferenceKind, ReferenceRange, SegmentedBars,
    SparklineModel, TimeSeries, TimeWindow,
};
use super::super::{
    DashboardWidgetId, MetricIndicators, MetricUnit, MetricValue, PeriodComparison, Trend,
    VisualPriority, WidgetState,
};
use super::types::{
    CHART_POINTS, MAX_PEAKS, MEMORY_THRESHOLDS, MetricModelError, MetricModelSummary,
    MetricReading, classify_with_signal, contextual_metric_value, highlight_for, intensity_for,
    latest_comparison, peak_floor, rising, sustained_above, trend_of, usable_metric_value,
    validate_capacity_pair, validate_history, validate_non_negative_reading,
    validate_normalized_reading,
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MemoryMetricInput<const HISTORY: usize> {
    used: MetricReading,
    available: MetricReading,
    total: MetricReading,
    cache: MetricReading,
    swap: MetricReading,
    pressure: MetricReading,
    history: TimeSeries<HISTORY>,
}

impl<const HISTORY: usize> MemoryMetricInput<HISTORY> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        used: MetricReading,
        available: MetricReading,
        total: MetricReading,
        cache: MetricReading,
        swap: MetricReading,
        pressure: MetricReading,
        history: TimeSeries<HISTORY>,
    ) -> Result<Self, MetricModelError> {
        for reading in [used, available, total, cache, swap] {
            validate_non_negative_reading(reading)?;
        }
        validate_normalized_reading(pressure)?;
        validate_capacity_pair(used, total)?;
        if used.is_usable()
            && available.is_usable()
            && total.is_usable()
            && used
                .value()
                .zip(available.value())
                .zip(total.value())
                .is_some_and(|((used, available), total)| used + available > total + 1.0e-9)
        {
            return Err(MetricModelError::InvalidTotal);
        }
        if cache.is_usable()
            && used.is_usable()
            && cache
                .value()
                .zip(used.value())
                .is_some_and(|(cache, used)| cache > used)
        {
            return Err(MetricModelError::InvalidTotal);
        }
        validate_history(history, MetricUnit::Percent)?;
        Ok(Self {
            used,
            available,
            total,
            cache,
            swap,
            pressure,
            history,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MemoryMetricModel<const HISTORY: usize> {
    summary: MetricModelSummary,
    sparkline: SparklineModel<CHART_POINTS>,
    area: Option<AreaModel>,
    composition: SegmentedBars<3>,
    reference_bands: ReferenceBands,
    peaks: PeakSet<MAX_PEAKS>,
    used: Option<MetricValue>,
    available: Option<MetricValue>,
    total: Option<MetricValue>,
    cache: Option<MetricValue>,
    swap: Option<MetricValue>,
    pressure: Option<MetricValue>,
}

impl<const HISTORY: usize> MemoryMetricModel<HISTORY> {
    pub fn from_input(input: MemoryMetricInput<HISTORY>) -> Result<Self, MetricModelError> {
        let sparkline = input.history.sparkline::<CHART_POINTS>(TimeWindow::Medium);
        let trend = trend_of(input.history);
        let comparison = latest_comparison(input.history, MetricUnit::Percent);
        let occupancy = input
            .used
            .value()
            .zip(input.total.value())
            .filter(|(_, total)| *total > 0.0)
            .map(|(used, total)| used / total)
            .filter(|value| value.is_finite());
        let pressure = input
            .pressure
            .value()
            .filter(|_| input.pressure.is_usable());
        let signal = pressure.or(occupancy);
        let sustained_pressure = sustained_above(input.history, MEMORY_THRESHOLDS.warning(), 3);
        let mut state =
            classify_with_signal(MEMORY_THRESHOLDS, signal, rising(trend), sustained_pressure);
        if input.swap.is_usable() && input.swap.value().is_some_and(|swap| swap > 0.0) {
            state = match state {
                WidgetState::Critical => WidgetState::Critical,
                WidgetState::Warning => WidgetState::Warning,
                _ => WidgetState::Active,
            };
        }
        if signal.is_none() {
            state = WidgetState::Unavailable;
        }

        let used = contextual_metric_value(input.used, MetricUnit::Bytes, VisualPriority::Primary);
        let available = contextual_metric_value(
            input.available,
            MetricUnit::Bytes,
            VisualPriority::Contextual,
        );
        let total =
            contextual_metric_value(input.total, MetricUnit::Bytes, VisualPriority::Contextual);
        let cache =
            contextual_metric_value(input.cache, MetricUnit::Bytes, VisualPriority::Contextual);
        let swap =
            contextual_metric_value(input.swap, MetricUnit::Bytes, VisualPriority::Contextual);
        let pressure_value = usable_metric_value(
            input.pressure,
            MetricUnit::Percent,
            VisualPriority::Secondary,
        );
        let primary = occupancy.and_then(|occupancy| {
            MetricValue::new(occupancy, MetricUnit::Percent, 1, VisualPriority::Primary)
        });
        let volatility = sparkline.volatility().intensity();
        let indicators = MetricIndicators::empty()
            .with(MetricIndicators::PROGRESS)
            .with(MetricIndicators::OCCUPANCY)
            .with(MetricIndicators::TREND)
            .with(MetricIndicators::VARIATION);
        let summary = MetricModelSummary::new(
            DashboardWidgetId::Memory,
            state,
            primary,
            pressure_value,
            trend,
            comparison,
            intensity_for(state, volatility),
            indicators,
            highlight_for(state),
        );

        let reference_bands = occupancy_reference_bands()?;
        let area = build_area(used, total)?;
        let composition = build_composition(used, available, total, cache)?;
        let peaks = input.history.detect_peaks::<MAX_PEAKS>(
            TimeWindow::Long,
            peak_floor(input.history, MEMORY_THRESHOLDS.warning()),
        )?;
        Ok(Self {
            summary,
            sparkline,
            area,
            composition,
            reference_bands,
            peaks,
            used,
            available,
            total,
            cache,
            swap,
            pressure: pressure_value,
        })
    }

    pub const fn frame(self) -> super::super::WidgetFrame {
        self.summary.frame()
    }

    pub const fn visual(self) -> super::super::MetricVisual {
        self.summary.visual()
    }

    pub const fn sparkline(self) -> SparklineModel<CHART_POINTS> {
        self.sparkline
    }

    pub const fn area(self) -> Option<AreaModel> {
        self.area
    }

    pub const fn composition(self) -> SegmentedBars<3> {
        self.composition
    }

    pub const fn reference_bands(self) -> ReferenceBands {
        self.reference_bands
    }

    pub const fn peaks(self) -> PeakSet<MAX_PEAKS> {
        self.peaks
    }

    pub const fn used(self) -> Option<MetricValue> {
        self.used
    }

    pub const fn available(self) -> Option<MetricValue> {
        self.available
    }

    pub const fn total(self) -> Option<MetricValue> {
        self.total
    }

    pub const fn cache(self) -> Option<MetricValue> {
        self.cache
    }

    pub const fn swap(self) -> Option<MetricValue> {
        self.swap
    }

    pub const fn pressure(self) -> Option<MetricValue> {
        self.pressure
    }

    pub const fn state(self) -> WidgetState {
        self.summary.state()
    }

    pub const fn trend(self) -> Trend {
        self.summary.visual().trend()
    }

    pub const fn comparison(self) -> PeriodComparison {
        self.summary.visual().comparison()
    }

    pub fn is_available(self) -> bool {
        self.summary.is_available()
    }
}

fn occupancy_reference_bands() -> Result<ReferenceBands, MetricModelError> {
    let mut bands = ReferenceBands::new(MetricUnit::Percent);
    for (lower, upper, kind) in [
        (0.0, 0.60, ReferenceKind::Normal),
        (0.60, 0.80, ReferenceKind::Attention),
        (0.80, 0.95, ReferenceKind::Warning),
        (0.95, 1.0, ReferenceKind::Critical),
    ] {
        bands.push(ReferenceRange::new(
            lower,
            upper,
            MetricUnit::Percent,
            kind,
        )?)?;
    }
    Ok(bands)
}

fn build_area(
    used: Option<MetricValue>,
    total: Option<MetricValue>,
) -> Result<Option<AreaModel>, MetricModelError> {
    let (Some(used), Some(total)) = (used, total) else {
        return Ok(None);
    };
    if !used.is_usable() || !total.is_usable() {
        return Ok(None);
    }
    let mut bands = ReferenceBands::new(MetricUnit::Bytes);
    for (lower, upper, kind) in [
        (0.0, total.value() * 0.60, ReferenceKind::Normal),
        (
            total.value() * 0.60,
            total.value() * 0.80,
            ReferenceKind::Attention,
        ),
        (
            total.value() * 0.80,
            total.value() * 0.95,
            ReferenceKind::Warning,
        ),
        (total.value() * 0.95, total.value(), ReferenceKind::Critical),
    ] {
        bands.push(ReferenceRange::new(lower, upper, MetricUnit::Bytes, kind)?)?;
    }
    let base = MetricValue::new(0.0, MetricUnit::Bytes, 0, VisualPriority::Contextual)
        .ok_or(MetricModelError::MissingRequiredValue)?;
    Ok(Some(AreaModel::new(base, Some(used), Some(total), bands)?))
}

fn build_composition(
    used: Option<MetricValue>,
    available: Option<MetricValue>,
    total: Option<MetricValue>,
    cache: Option<MetricValue>,
) -> Result<SegmentedBars<3>, MetricModelError> {
    let mut composition = SegmentedBars::new(total)?;
    let cached = cache.filter(|value| value.is_usable());
    let working = match (used.filter(|value| value.is_usable()), cached) {
        (Some(used), Some(cache)) => Some(
            MetricValue::new(
                (used.value() - cache.value()).max(0.0),
                MetricUnit::Bytes,
                1,
                VisualPriority::Secondary,
            )
            .ok_or(MetricModelError::MissingRequiredValue)?,
        ),
        (Some(used), None) => Some(used),
        _ => None,
    };
    composition.push(ChartSegment::new(
        super::super::chart::SegmentId::Used,
        working,
    ))?;
    composition.push(ChartSegment::new(
        super::super::chart::SegmentId::Cache,
        cache,
    ))?;
    composition.push(ChartSegment::new(
        super::super::chart::SegmentId::Free,
        available,
    ))?;
    Ok(composition)
}

#[cfg(test)]
mod tests {
    use super::super::super::MetricAvailability;
    use super::super::super::TrendDirection;
    use super::super::types::MetricReading;
    use super::*;

    fn history(values: &[f64]) -> TimeSeries<8> {
        let mut history = TimeSeries::new(MetricUnit::Percent);
        for (index, value) in values.iter().copied().enumerate() {
            history
                .push(
                    super::super::super::chart::TimeSample::new(
                        std::time::Duration::from_secs(index as u64),
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

    fn input(
        used: f64,
        total: f64,
        pressure: f64,
        swap: f64,
        history_values: &[f64],
    ) -> MemoryMetricInput<8> {
        MemoryMetricInput::new(
            MetricReading::non_negative(Some(used), MetricAvailability::Available).unwrap(),
            MetricReading::non_negative(Some(total - used), MetricAvailability::Available).unwrap(),
            MetricReading::non_negative(Some(total), MetricAvailability::Available).unwrap(),
            MetricReading::non_negative(Some(used * 0.2), MetricAvailability::Available).unwrap(),
            MetricReading::non_negative(Some(swap), MetricAvailability::Available).unwrap(),
            MetricReading::normalized(Some(pressure), MetricAvailability::Available).unwrap(),
            history(history_values),
        )
        .unwrap()
    }

    #[test]
    fn memory_distinguishes_healthy_cache_from_pressure_and_swap() {
        let healthy =
            MemoryMetricModel::from_input(input(60.0, 100.0, 0.30, 0.0, &[0.5, 0.6, 0.6])).unwrap();
        assert_eq!(healthy.state(), WidgetState::Normal);
        assert_eq!(healthy.composition().len(), 3);
        assert!(healthy.cache().is_some());

        let pressured =
            MemoryMetricModel::from_input(input(70.0, 100.0, 0.85, 0.0, &[0.65, 0.75, 0.85]))
                .unwrap();
        assert_eq!(pressured.state(), WidgetState::Warning);
        assert_eq!(
            pressured.visual().trend().direction(),
            TrendDirection::Rising
        );

        let critical =
            MemoryMetricModel::from_input(input(96.0, 100.0, 0.96, 4.0, &[0.8, 0.9, 0.96]))
                .unwrap();
        assert_eq!(critical.state(), WidgetState::Critical);
        assert!(critical.swap().is_some());
    }

    #[test]
    fn memory_produces_area_bands_and_visual_context() {
        let model =
            MemoryMetricModel::from_input(input(50.0, 100.0, 0.45, 0.0, &[0.4, 0.5])).unwrap();
        assert_eq!(model.visual().widget_id(), DashboardWidgetId::Memory);
        assert_eq!(
            model.visual().primary().unwrap().unit(),
            MetricUnit::Percent
        );
        assert_eq!(model.area().unwrap().occupancy(), Some(0.5));
        assert_eq!(model.reference_bands().len(), 4);
        assert_eq!(model.sparkline().stats().available_samples(), 2);
    }

    #[test]
    fn memory_rejects_inconsistent_capacity() {
        let result = MemoryMetricInput::new(
            MetricReading::non_negative(Some(90.0), MetricAvailability::Available).unwrap(),
            MetricReading::non_negative(Some(20.0), MetricAvailability::Available).unwrap(),
            MetricReading::non_negative(Some(100.0), MetricAvailability::Available).unwrap(),
            MetricReading::non_negative(Some(5.0), MetricAvailability::Available).unwrap(),
            MetricReading::unavailable(),
            MetricReading::unavailable(),
            history(&[]),
        );
        assert!(matches!(result, Err(MetricModelError::InvalidTotal)));
    }
}
