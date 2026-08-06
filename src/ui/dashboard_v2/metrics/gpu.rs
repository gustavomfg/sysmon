use super::super::chart::{
    ChartSegment, PeakSet, SegmentedBars, SparklineModel, TimeSeries, TimeWindow,
};
use super::super::{
    DashboardWidgetId, MetricIndicators, MetricUnit, MetricValue, PeriodComparison, Trend,
    VisualPriority, WidgetState,
};
use super::types::{
    CHART_POINTS, GPU_THRESHOLDS, MAX_PEAKS, MetricModelError, MetricModelSummary, MetricReading,
    classify_with_signal, contextual_metric_value, highlight_for, intensity_for, latest_comparison,
    peak_floor, rising, trend_of, usable_metric_value, validate_capacity_pair, validate_history,
    validate_non_negative_reading, validate_normalized_reading,
};

pub const GPU_TEMPERATURE_WARNING: f64 = 80.0;
pub const GPU_TEMPERATURE_CRITICAL: f64 = 90.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GpuMetricInput<const HISTORY: usize> {
    utilization: MetricReading,
    vram_used: MetricReading,
    vram_total: MetricReading,
    temperature: MetricReading,
    frequency: MetricReading,
    power: MetricReading,
    history: TimeSeries<HISTORY>,
}

impl<const HISTORY: usize> GpuMetricInput<HISTORY> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        utilization: MetricReading,
        vram_used: MetricReading,
        vram_total: MetricReading,
        temperature: MetricReading,
        frequency: MetricReading,
        power: MetricReading,
        history: TimeSeries<HISTORY>,
    ) -> Result<Self, MetricModelError> {
        validate_normalized_reading(utilization)?;
        for reading in [vram_used, vram_total, frequency, power] {
            validate_non_negative_reading(reading)?;
        }
        validate_capacity_pair(vram_used, vram_total)?;
        validate_history(history, MetricUnit::Percent)?;
        Ok(Self {
            utilization,
            vram_used,
            vram_total,
            temperature,
            frequency,
            power,
            history,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GpuMetricModel<const HISTORY: usize> {
    summary: MetricModelSummary,
    sparkline: SparklineModel<CHART_POINTS>,
    vram: SegmentedBars<2>,
    peaks: PeakSet<MAX_PEAKS>,
    vram_used: Option<MetricValue>,
    vram_total: Option<MetricValue>,
    temperature: Option<MetricValue>,
    frequency: Option<MetricValue>,
    power: Option<MetricValue>,
}

impl<const HISTORY: usize> GpuMetricModel<HISTORY> {
    pub fn from_input(input: GpuMetricInput<HISTORY>) -> Result<Self, MetricModelError> {
        let sparkline = input.history.sparkline::<CHART_POINTS>(TimeWindow::Medium);
        let trend = trend_of(input.history);
        let comparison = latest_comparison(input.history, MetricUnit::Percent);
        let utilization = input
            .utilization
            .value()
            .filter(|_| input.utilization.is_usable());
        let vram_fraction = input
            .vram_used
            .value()
            .zip(input.vram_total.value())
            .filter(|(_, total)| *total > 0.0)
            .map(|(used, total)| used / total)
            .filter(|value| value.is_finite());
        let signal = match (utilization, vram_fraction) {
            (Some(utilization), Some(vram_fraction)) => Some(utilization.max(vram_fraction)),
            (Some(utilization), None) => Some(utilization),
            (None, Some(vram_fraction)) => Some(vram_fraction),
            (None, None) => None,
        };
        let mut state = classify_with_signal(GPU_THRESHOLDS, signal, rising(trend), false);
        if input
            .temperature
            .value()
            .is_some_and(|temperature| temperature >= GPU_TEMPERATURE_CRITICAL)
        {
            state = WidgetState::Critical;
        } else if state != WidgetState::Critical
            && input
                .temperature
                .value()
                .is_some_and(|temperature| temperature >= GPU_TEMPERATURE_WARNING)
        {
            state = WidgetState::Warning;
        }
        if signal.is_none() {
            state = WidgetState::Unavailable;
        }

        let primary = usable_metric_value(
            input.utilization,
            MetricUnit::Percent,
            VisualPriority::Primary,
        );
        let temperature = contextual_metric_value(
            input.temperature,
            MetricUnit::TemperatureCelsius,
            VisualPriority::Contextual,
        );
        let secondary = temperature.or(vram_fraction.and_then(|fraction| {
            MetricValue::new(fraction, MetricUnit::Percent, 1, VisualPriority::Secondary)
        }));
        let volatility = sparkline.volatility().intensity();
        let indicators = MetricIndicators::empty()
            .with(MetricIndicators::PROGRESS)
            .with(MetricIndicators::TREND)
            .with(MetricIndicators::VARIATION)
            .with(MetricIndicators::ACTIVITY);
        let summary = MetricModelSummary::new(
            DashboardWidgetId::Gpu,
            state,
            primary,
            secondary,
            trend,
            comparison,
            intensity_for(state, volatility),
            indicators,
            highlight_for(state),
        );

        let vram_used = contextual_metric_value(
            input.vram_used,
            MetricUnit::Bytes,
            VisualPriority::Secondary,
        );
        let vram_total = contextual_metric_value(
            input.vram_total,
            MetricUnit::Bytes,
            VisualPriority::Contextual,
        );
        let vram = build_vram(vram_used, vram_total)?;
        let peaks = input.history.detect_peaks::<MAX_PEAKS>(
            TimeWindow::Long,
            peak_floor(input.history, GPU_THRESHOLDS.warning()),
        )?;
        Ok(Self {
            summary,
            sparkline,
            vram,
            peaks,
            vram_used,
            vram_total,
            temperature,
            frequency: contextual_metric_value(
                input.frequency,
                MetricUnit::FrequencyHertz,
                VisualPriority::Contextual,
            ),
            power: contextual_metric_value(
                input.power,
                MetricUnit::None,
                VisualPriority::Contextual,
            ),
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

    pub const fn vram(self) -> SegmentedBars<2> {
        self.vram
    }

    pub const fn peaks(self) -> PeakSet<MAX_PEAKS> {
        self.peaks
    }

    pub const fn vram_used(self) -> Option<MetricValue> {
        self.vram_used
    }

    pub const fn vram_total(self) -> Option<MetricValue> {
        self.vram_total
    }

    pub const fn temperature(self) -> Option<MetricValue> {
        self.temperature
    }

    pub const fn frequency(self) -> Option<MetricValue> {
        self.frequency
    }

    pub const fn power(self) -> Option<MetricValue> {
        self.power
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

fn build_vram(
    used: Option<MetricValue>,
    total: Option<MetricValue>,
) -> Result<SegmentedBars<2>, MetricModelError> {
    let mut vram = SegmentedBars::new(total)?;
    vram.push(ChartSegment::new(
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
    vram.push(ChartSegment::new(
        super::super::chart::SegmentId::Free,
        free,
    ))?;
    Ok(vram)
}

#[cfg(test)]
mod tests {
    use super::super::super::chart::TimeSample;
    use super::super::types::MetricReading;
    use super::*;
    use std::time::Duration;

    fn history(values: &[f64]) -> TimeSeries<8> {
        let mut history = TimeSeries::new(MetricUnit::Percent);
        for (index, value) in values.iter().copied().enumerate() {
            history
                .push(
                    TimeSample::new(
                        Duration::from_secs(index as u64),
                        Some(value),
                        super::super::super::MetricAvailability::Available,
                        None,
                    )
                    .unwrap(),
                )
                .unwrap();
        }
        history
    }

    fn input(utilization: f64, vram_used: f64, temperature: f64) -> GpuMetricInput<8> {
        GpuMetricInput::new(
            MetricReading::normalized(
                Some(utilization),
                super::super::super::MetricAvailability::Available,
            )
            .unwrap(),
            MetricReading::non_negative(
                Some(vram_used),
                super::super::super::MetricAvailability::Available,
            )
            .unwrap(),
            MetricReading::non_negative(
                Some(100.0),
                super::super::super::MetricAvailability::Available,
            )
            .unwrap(),
            MetricReading::new(
                Some(temperature),
                super::super::super::MetricAvailability::Available,
            )
            .unwrap(),
            MetricReading::non_negative(
                Some(1500.0),
                super::super::super::MetricAvailability::Available,
            )
            .unwrap(),
            MetricReading::non_negative(
                Some(100.0),
                super::super::super::MetricAvailability::Available,
            )
            .unwrap(),
            history(&[0.2, utilization]),
        )
        .unwrap()
    }

    #[test]
    fn gpu_available_model_exposes_vram_and_context() {
        let model = GpuMetricModel::from_input(input(0.70, 40.0, 65.0)).unwrap();
        assert_eq!(model.state(), WidgetState::Active);
        assert_eq!(model.visual().widget_id(), DashboardWidgetId::Gpu);
        assert_eq!(model.vram().len(), 2);
        assert_eq!(
            model.temperature().unwrap().unit(),
            MetricUnit::TemperatureCelsius
        );
        assert_eq!(
            model.frequency().unwrap().unit(),
            MetricUnit::FrequencyHertz
        );
        assert!(model.visual().primary().is_some());
    }

    #[test]
    fn gpu_temperature_and_vram_can_raise_semantic_state() {
        assert_eq!(
            GpuMetricModel::from_input(input(0.50, 96.0, 65.0))
                .unwrap()
                .state(),
            WidgetState::Critical
        );
        assert_eq!(
            GpuMetricModel::from_input(input(0.50, 40.0, 85.0))
                .unwrap()
                .state(),
            WidgetState::Warning
        );
    }

    #[test]
    fn gpu_absence_is_unavailable_without_global_failure() {
        let input = GpuMetricInput::new(
            MetricReading::unavailable(),
            MetricReading::unavailable(),
            MetricReading::unavailable(),
            MetricReading::unavailable(),
            MetricReading::unavailable(),
            MetricReading::unavailable(),
            history(&[]),
        )
        .unwrap();
        let model = GpuMetricModel::from_input(input).unwrap();
        assert_eq!(model.state(), WidgetState::Unavailable);
        assert!(model.visual().primary().is_none());
        assert!(!model.is_available());
    }
}
