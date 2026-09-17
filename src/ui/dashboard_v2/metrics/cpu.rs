use std::time::Duration;

use super::super::chart::{PeakSet, SegmentedBars, SparklineModel, TimeSeries, TimeWindow};
use super::super::{
    DashboardWidgetId, MetricIndicators, MetricUnit, MetricValue, PeriodComparison, Trend,
    VisualPriority, WidgetState,
};
use super::types::{
    CHART_POINTS, CPU_THRESHOLDS, MAX_PEAKS, MetricModelError, MetricModelSummary, MetricReading,
    classify_with_signal, contextual_metric_value, highlight_for, intensity_for, peak_floor,
    rising, sustained_above, trend_of, usable_metric_value, validate_history,
    validate_non_negative_reading, validate_normalized_reading,
};

pub const CPU_TEMPERATURE_WARNING: f64 = 80.0;
pub const CPU_TEMPERATURE_CRITICAL: f64 = 90.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CpuMetricInput<const HISTORY: usize> {
    utilization: MetricReading,
    user: MetricReading,
    system: MetricReading,
    idle: MetricReading,
    frequency: MetricReading,
    temperature: MetricReading,
    cores: usize,
    history: TimeSeries<HISTORY>,
}

impl<const HISTORY: usize> CpuMetricInput<HISTORY> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        utilization: MetricReading,
        user: MetricReading,
        system: MetricReading,
        idle: MetricReading,
        frequency: MetricReading,
        temperature: MetricReading,
        cores: usize,
        history: TimeSeries<HISTORY>,
    ) -> Result<Self, MetricModelError> {
        if cores == 0 {
            return Err(MetricModelError::InvalidComposition);
        }
        for reading in [utilization, user, system, idle] {
            validate_normalized_reading(reading)?;
        }
        validate_non_negative_reading(frequency)?;
        validate_history(history, MetricUnit::Percent)?;
        if [user, system, idle]
            .into_iter()
            .all(MetricReading::is_usable)
            && [user.value(), system.value(), idle.value()]
                .into_iter()
                .flatten()
                .sum::<f64>()
                > 1.0 + 1.0e-9
        {
            return Err(MetricModelError::InvalidComposition);
        }
        Ok(Self {
            utilization,
            user,
            system,
            idle,
            frequency,
            temperature,
            cores,
            history,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CpuMetricModel<const HISTORY: usize> {
    summary: MetricModelSummary,
    sparkline: SparklineModel<CHART_POINTS>,
    composition: SegmentedBars<3>,
    peaks: PeakSet<MAX_PEAKS>,
    frequency: Option<MetricValue>,
    temperature: Option<MetricValue>,
    sustained_load: bool,
    cores: usize,
}

impl<const HISTORY: usize> CpuMetricModel<HISTORY> {
    pub fn from_input(input: CpuMetricInput<HISTORY>) -> Result<Self, MetricModelError> {
        let sparkline = input.history.sparkline::<CHART_POINTS>(TimeWindow::Custom(
            super::super::chart::VISIBLE_HISTORY_SAMPLES,
        ));
        let trend = trend_of(input.history);
        let comparison = comparison_over_recent_window(input.history, Duration::from_secs(5));
        let sustained_load = sustained_above(input.history, CPU_THRESHOLDS.warning(), 3);
        let mut state = classify_with_signal(
            CPU_THRESHOLDS,
            input
                .utilization
                .value()
                .filter(|_| input.utilization.is_usable()),
            rising(trend),
            sustained_load,
        );
        if input
            .temperature
            .value()
            .is_some_and(|temperature| temperature >= CPU_TEMPERATURE_CRITICAL)
        {
            state = WidgetState::Critical;
        } else if state != WidgetState::Critical
            && input
                .temperature
                .value()
                .is_some_and(|temperature| temperature >= CPU_TEMPERATURE_WARNING)
        {
            state = WidgetState::Warning;
        }
        if !input.utilization.is_usable() {
            state = WidgetState::Unavailable;
        }

        let volatility = sparkline.volatility().intensity();
        let primary = usable_metric_value(
            input.utilization,
            MetricUnit::Percent,
            VisualPriority::Primary,
        );
        let frequency = contextual_metric_value(
            input.frequency,
            MetricUnit::FrequencyHertz,
            VisualPriority::Contextual,
        );
        let temperature = contextual_metric_value(
            input.temperature,
            MetricUnit::TemperatureCelsius,
            VisualPriority::Contextual,
        );
        let secondary = temperature.or(frequency);
        let indicators = MetricIndicators::empty()
            .with(MetricIndicators::TREND)
            .with(MetricIndicators::VARIATION)
            .with(MetricIndicators::ACTIVITY);
        let summary = MetricModelSummary::new(
            DashboardWidgetId::Cpu,
            state,
            primary,
            secondary,
            trend,
            comparison,
            intensity_for(state, volatility),
            indicators,
            highlight_for(state),
        );

        let total = MetricValue::new(1.0, MetricUnit::Percent, 1, VisualPriority::Secondary)
            .ok_or(MetricModelError::MissingRequiredValue)?;
        let mut composition = SegmentedBars::<3>::new(Some(total))?;
        for (id, reading) in [
            (super::super::chart::SegmentId::User, input.user),
            (super::super::chart::SegmentId::System, input.system),
            (super::super::chart::SegmentId::Idle, input.idle),
        ] {
            composition.push(super::super::chart::ChartSegment::new(
                id,
                contextual_metric_value(reading, MetricUnit::Percent, VisualPriority::Secondary),
            ))?;
        }
        let peaks = input.history.detect_peaks::<MAX_PEAKS>(
            TimeWindow::Long,
            peak_floor(input.history, CPU_THRESHOLDS.warning()),
        )?;
        Ok(Self {
            summary,
            sparkline,
            composition,
            peaks,
            frequency,
            temperature,
            sustained_load,
            cores: input.cores,
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

    pub const fn composition(self) -> SegmentedBars<3> {
        self.composition
    }

    pub const fn peaks(self) -> PeakSet<MAX_PEAKS> {
        self.peaks
    }

    pub const fn frequency(self) -> Option<MetricValue> {
        self.frequency
    }

    pub const fn temperature(self) -> Option<MetricValue> {
        self.temperature
    }

    pub const fn sustained_load(self) -> bool {
        self.sustained_load
    }

    pub const fn cores(self) -> usize {
        self.cores
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

fn comparison_over_recent_window<const CAPACITY: usize>(
    history: TimeSeries<CAPACITY>,
    minimum_lag: Duration,
) -> PeriodComparison {
    let Some(current) = history.last().and_then(|sample| sample.usable_value()) else {
        return PeriodComparison::unknown();
    };
    let Some(current_sample) = history.last() else {
        return PeriodComparison::unknown();
    };
    let target = current_sample.offset().saturating_sub(minimum_lag);
    let mut previous = None;
    let mut fallback = None;
    for sample in history.iter() {
        if sample.offset() < current_sample.offset() {
            fallback = sample.usable_value();
        }
        if sample.offset() < current_sample.offset() && sample.offset() <= target {
            previous = sample.usable_value();
        }
    }
    let previous = if current_sample.offset() >= minimum_lag {
        previous.or(fallback)
    } else {
        fallback
    };
    let Some(current) = MetricValue::new(current, MetricUnit::Percent, 1, VisualPriority::Primary)
    else {
        return PeriodComparison::unknown();
    };
    let previous = previous
        .and_then(|value| MetricValue::new(value, MetricUnit::Percent, 1, VisualPriority::Primary));
    PeriodComparison::from_values(current, previous)
}

#[cfg(test)]
mod tests {
    use super::super::super::MetricAvailability;
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

    fn input(utilization: f64, history_values: &[f64]) -> CpuMetricInput<8> {
        CpuMetricInput::new(
            MetricReading::normalized(Some(utilization), MetricAvailability::Available).unwrap(),
            MetricReading::normalized(Some(0.45), MetricAvailability::Available).unwrap(),
            MetricReading::normalized(Some(0.15), MetricAvailability::Available).unwrap(),
            MetricReading::normalized(Some(0.40), MetricAvailability::Available).unwrap(),
            MetricReading::non_negative(Some(3_200_000_000.0), MetricAvailability::Available)
                .unwrap(),
            MetricReading::new(Some(55.0), MetricAvailability::Available).unwrap(),
            8,
            history(history_values),
        )
        .unwrap()
    }

    #[test]
    fn cpu_states_consider_current_value_and_sustained_history() {
        assert_eq!(
            CpuMetricModel::from_input(input(0.20, &[0.1, 0.2, 0.2]))
                .unwrap()
                .state(),
            WidgetState::Normal
        );
        assert_eq!(
            CpuMetricModel::from_input(input(0.60, &[0.3, 0.4, 0.5]))
                .unwrap()
                .state(),
            WidgetState::Active
        );
        assert_eq!(
            CpuMetricModel::from_input(input(0.70, &[0.82, 0.84, 0.86]))
                .unwrap()
                .state(),
            WidgetState::Warning
        );
        assert_eq!(
            CpuMetricModel::from_input(input(0.97, &[0.7, 0.8, 0.9]))
                .unwrap()
                .state(),
            WidgetState::Critical
        );
        assert!(
            CpuMetricModel::from_input(input(0.70, &[0.82, 0.84, 0.86]))
                .unwrap()
                .sustained_load()
        );
    }

    #[test]
    fn cpu_exposes_visual_chart_composition_and_context() {
        let model = CpuMetricModel::from_input(input(0.65, &[0.2, 0.4, 0.65])).unwrap();
        assert_eq!(model.visual().widget_id(), DashboardWidgetId::Cpu);
        assert_eq!(model.composition().len(), 3);
        assert_eq!(model.sparkline().stats().available_samples(), 3);
        assert_eq!(
            model.frequency().unwrap().unit(),
            MetricUnit::FrequencyHertz
        );
        assert_eq!(
            model.temperature().unwrap().unit(),
            MetricUnit::TemperatureCelsius
        );
        assert!(model.visual().primary().is_some());
    }

    #[test]
    fn cpu_comparison_uses_a_stable_recent_window() {
        let model =
            CpuMetricModel::from_input(input(0.80, &[0.20, 0.30, 0.40, 0.50, 0.60, 0.80])).unwrap();
        let delta = model.comparison().delta().unwrap();
        assert!((delta - 0.60).abs() < 1.0e-9);
    }

    #[test]
    fn cpu_rejects_invalid_compositions_and_preserves_unavailable_state() {
        let invalid = CpuMetricInput::new(
            MetricReading::normalized(Some(0.5), MetricAvailability::Available).unwrap(),
            MetricReading::normalized(Some(0.8), MetricAvailability::Available).unwrap(),
            MetricReading::normalized(Some(0.3), MetricAvailability::Available).unwrap(),
            MetricReading::normalized(Some(0.1), MetricAvailability::Available).unwrap(),
            MetricReading::unavailable(),
            MetricReading::unavailable(),
            4,
            history(&[0.2, 0.3]),
        );
        assert!(matches!(invalid, Err(MetricModelError::InvalidComposition)));

        let unavailable = CpuMetricInput::new(
            MetricReading::unavailable(),
            MetricReading::unavailable(),
            MetricReading::unavailable(),
            MetricReading::unavailable(),
            MetricReading::unavailable(),
            MetricReading::unavailable(),
            4,
            history(&[]),
        )
        .unwrap();
        let model = CpuMetricModel::from_input(unavailable).unwrap();
        assert_eq!(model.state(), WidgetState::Unavailable);
        assert!(model.visual().primary().is_none());
    }
}
