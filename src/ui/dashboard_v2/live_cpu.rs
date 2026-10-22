use std::time::Instant;

use super::chart::{ChartError, PushOutcome, TimeSample, TimeSeries};
use super::metrics::{CpuMetricInput, CpuMetricModel, MetricModelError, MetricReading};
use super::visual::{MetricAvailability, MetricUnit};

/// Retain 120 collector snapshots for diagnostic calculations. The chart
/// displays the latest width-selected samples without changing the cadence.
pub const CPU_LIVE_HISTORY_CAPACITY: usize = 120;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CpuLiveSample {
    pub utilization_percent: Option<f64>,
    pub frequency_hz: Option<f64>,
    pub temperature_celsius: Option<f64>,
    pub cores: usize,
}

impl CpuLiveSample {
    pub const fn unavailable() -> Self {
        Self {
            utilization_percent: None,
            frequency_hz: None,
            temperature_celsius: None,
            cores: 1,
        }
    }
}

#[derive(Debug)]
pub struct CpuLiveHistory {
    origin: Option<Instant>,
    history: TimeSeries<CPU_LIVE_HISTORY_CAPACITY>,
    latest: CpuLiveSample,
}

impl Default for CpuLiveHistory {
    fn default() -> Self {
        Self::new()
    }
}

impl CpuLiveHistory {
    pub const fn new() -> Self {
        Self {
            origin: None,
            history: TimeSeries::new(MetricUnit::Percent),
            latest: CpuLiveSample::unavailable(),
        }
    }

    pub fn push(
        &mut self,
        collected_at: Instant,
        sample: CpuLiveSample,
    ) -> Result<PushOutcome, ChartError> {
        let origin = *self.origin.get_or_insert(collected_at);
        let offset = collected_at.saturating_duration_since(origin);
        let value = normalized_percent(sample.utilization_percent);
        let availability = if value.is_some() {
            MetricAvailability::Available
        } else {
            MetricAvailability::Unavailable
        };
        let point = TimeSample::new(offset, value, availability, None)?;
        let outcome = self.history.push(point)?;
        self.latest = sample;
        Ok(outcome)
    }

    pub const fn history(&self) -> TimeSeries<CPU_LIVE_HISTORY_CAPACITY> {
        self.history
    }

    pub const fn latest(&self) -> CpuLiveSample {
        self.latest
    }

    pub fn model(&self) -> Result<CpuMetricModel<CPU_LIVE_HISTORY_CAPACITY>, MetricModelError> {
        let sample = self.latest;
        let input = CpuMetricInput::new(
            normalized_reading(sample.utilization_percent),
            MetricReading::unavailable(),
            MetricReading::unavailable(),
            MetricReading::unavailable(),
            non_negative_reading(sample.frequency_hz),
            MetricReading::new(
                sample
                    .temperature_celsius
                    .filter(|value| value.is_finite() && *value >= -273.15),
                MetricAvailability::Available,
            )?,
            sample.cores.max(1),
            self.history,
        )?;
        CpuMetricModel::from_input(input)
    }
}

fn normalized_percent(value: Option<f64>) -> Option<f64> {
    value
        .filter(|value| value.is_finite() && (0.0..=100.0).contains(value))
        .map(|value| value / 100.0)
}

fn normalized_reading(value: Option<f64>) -> MetricReading {
    let Some(value) = normalized_percent(value) else {
        return MetricReading::unavailable();
    };
    MetricReading::normalized(Some(value), MetricAvailability::Available)
        .unwrap_or_else(|_| MetricReading::unavailable())
}

fn non_negative_reading(value: Option<f64>) -> MetricReading {
    let Some(value) = value.filter(|value| value.is_finite() && *value >= 0.0) else {
        return MetricReading::unavailable();
    };
    MetricReading::non_negative(Some(value), MetricAvailability::Available)
        .unwrap_or_else(|_| MetricReading::unavailable())
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    fn sample(value: f64) -> CpuLiveSample {
        CpuLiveSample {
            utilization_percent: Some(value),
            frequency_hz: Some(3.2e9),
            temperature_celsius: Some(52.0),
            cores: 8,
        }
    }

    #[test]
    fn bounded_history_keeps_order_and_latest_sample_at_the_right_edge() {
        let start = Instant::now();
        let mut history = CpuLiveHistory::new();
        for index in 0..=CPU_LIVE_HISTORY_CAPACITY {
            history
                .push(
                    start + Duration::from_secs(index as u64),
                    sample(40.0 + (index % 50) as f64),
                )
                .unwrap();
        }

        let series = history.history();
        assert_eq!(series.len(), CPU_LIVE_HISTORY_CAPACITY);
        assert_eq!(series.first().unwrap().offset(), Duration::from_secs(1));
        assert_eq!(series.last().unwrap().offset(), Duration::from_secs(120));
        assert_eq!(series.last().unwrap().usable_value(), Some(60.0 / 100.0));
        assert_eq!(history.latest().utilization_percent, Some(60.0));
    }

    #[test]
    fn invalid_snapshot_values_become_explicitly_unavailable() {
        let start = Instant::now();
        let mut history = CpuLiveHistory::new();
        history
            .push(
                start,
                CpuLiveSample {
                    utilization_percent: Some(f64::NAN),
                    frequency_hz: Some(-1.0),
                    temperature_celsius: Some(f64::INFINITY),
                    cores: 8,
                },
            )
            .unwrap();
        let model = history.model().unwrap();
        assert_eq!(model.state(), super::super::WidgetState::Unavailable);
        assert!(model.visual().primary().is_none());
        assert!(history.history().last().unwrap().usable_value().is_none());
    }

    #[test]
    fn model_preserves_real_optional_context_when_available() {
        let start = Instant::now();
        let mut history = CpuLiveHistory::new();
        history.push(start, sample(42.0)).unwrap();
        let model = history.model().unwrap();
        assert_eq!(model.cores(), 8);
        assert_eq!(model.frequency().unwrap().value(), 3.2e9);
        assert_eq!(model.temperature().unwrap().value(), 52.0);
    }
}
