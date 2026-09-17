use super::super::chart::{PeakSet, SeriesComparison, SparklineModel, TimeSeries, TimeWindow};
use super::super::{
    DashboardWidgetId, MetricIndicators, MetricUnit, MetricValue, PeriodComparison, Trend,
    VisualPriority, WidgetState,
};
use super::types::{
    CHART_POINTS, MAX_PEAKS, MetricModelError, MetricModelSummary, MetricReading,
    NETWORK_THRESHOLDS, classify_with_signal, contextual_metric_value, highlight_for,
    intensity_for, latest_comparison, peak_floor, rising, trend_of, usable_metric_value,
    validate_history, validate_non_negative_reading,
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NetworkMetricInput<const HISTORY: usize> {
    download_rate: MetricReading,
    upload_rate: MetricReading,
    download_total: MetricReading,
    upload_total: MetricReading,
    capacity: MetricReading,
    download_history: TimeSeries<HISTORY>,
    upload_history: TimeSeries<HISTORY>,
}

impl<const HISTORY: usize> NetworkMetricInput<HISTORY> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        download_rate: MetricReading,
        upload_rate: MetricReading,
        download_total: MetricReading,
        upload_total: MetricReading,
        capacity: MetricReading,
        download_history: TimeSeries<HISTORY>,
        upload_history: TimeSeries<HISTORY>,
    ) -> Result<Self, MetricModelError> {
        for reading in [
            download_rate,
            upload_rate,
            download_total,
            upload_total,
            capacity,
        ] {
            validate_non_negative_reading(reading)?;
        }
        validate_history(download_history, MetricUnit::BytesPerSecond)?;
        validate_history(upload_history, MetricUnit::BytesPerSecond)?;
        Ok(Self {
            download_rate,
            upload_rate,
            download_total,
            upload_total,
            capacity,
            download_history,
            upload_history,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NetworkMetricModel<const HISTORY: usize> {
    summary: MetricModelSummary,
    download_sparkline: SparklineModel<CHART_POINTS>,
    upload_sparkline: SparklineModel<CHART_POINTS>,
    comparison: SeriesComparison,
    download_peaks: PeakSet<MAX_PEAKS>,
    upload_peaks: PeakSet<MAX_PEAKS>,
    download: Option<MetricValue>,
    upload: Option<MetricValue>,
    download_total: Option<MetricValue>,
    upload_total: Option<MetricValue>,
}

impl<const HISTORY: usize> NetworkMetricModel<HISTORY> {
    pub fn from_input(input: NetworkMetricInput<HISTORY>) -> Result<Self, MetricModelError> {
        let download_sparkline =
            input
                .download_history
                .sparkline::<CHART_POINTS>(TimeWindow::Custom(
                    super::super::chart::VISIBLE_HISTORY_SAMPLES,
                ));
        let upload_sparkline = input
            .upload_history
            .sparkline::<CHART_POINTS>(TimeWindow::Custom(
                super::super::chart::VISIBLE_HISTORY_SAMPLES,
            ));
        let download = usable_metric_value(
            input.download_rate,
            MetricUnit::BytesPerSecond,
            VisualPriority::Primary,
        );
        let upload = usable_metric_value(
            input.upload_rate,
            MetricUnit::BytesPerSecond,
            VisualPriority::Secondary,
        );
        let download_value = download.map_or(0.0, MetricValue::value);
        let upload_value = upload.map_or(0.0, MetricValue::value);
        let current_activity =
            (download.is_some() || upload.is_some()).then_some(download_value.max(upload_value));
        let normalized_activity = input
            .capacity
            .value()
            .filter(|_| input.capacity.is_usable())
            .filter(|capacity| *capacity > 0.0)
            .and_then(|capacity| Some((current_activity? / capacity).min(1.0)));
        let trend = if download_value >= upload_value {
            trend_of(input.download_history)
        } else {
            trend_of(input.upload_history)
        };
        let signal = normalized_activity.or_else(|| {
            current_activity.and_then(|value| (value > 0.0).then_some(NETWORK_THRESHOLDS.active()))
        });
        let mut state = classify_with_signal(NETWORK_THRESHOLDS, signal, rising(trend), false);
        if current_activity == Some(0.0) {
            state = WidgetState::Normal;
        }
        if current_activity.is_none() {
            state = WidgetState::Unavailable;
        }
        let primary = match (download, upload) {
            (Some(download), Some(upload)) if upload.value() > download.value() => Some(upload),
            (Some(download), _) => Some(download),
            (_, Some(upload)) => Some(upload),
            _ => None,
        };
        let secondary = match (download, upload) {
            (Some(download), Some(upload)) if upload.value() > download.value() => Some(download),
            (Some(_), Some(upload)) => Some(upload),
            _ => None,
        };
        let comparison = input.download_history.compare(input.upload_history)?;
        let visual_comparison = if download_value >= upload_value {
            latest_comparison(input.download_history, MetricUnit::BytesPerSecond)
        } else {
            latest_comparison(input.upload_history, MetricUnit::BytesPerSecond)
        };
        let (download_volatility, upload_volatility) = (
            download_sparkline.volatility().intensity(),
            upload_sparkline.volatility().intensity(),
        );
        let volatility = download_volatility.max(upload_volatility);
        let indicators = MetricIndicators::empty()
            .with(MetricIndicators::TREND)
            .with(MetricIndicators::VARIATION)
            .with(MetricIndicators::ACTIVITY);
        let summary = MetricModelSummary::new(
            DashboardWidgetId::Network,
            state,
            primary,
            secondary,
            trend,
            visual_comparison,
            intensity_for(state, volatility),
            indicators,
            highlight_for(state),
        );
        let download_peaks = input
            .download_history
            .detect_peaks::<MAX_PEAKS>(TimeWindow::Long, peak_floor(input.download_history, 1.0))?;
        let upload_peaks = input
            .upload_history
            .detect_peaks::<MAX_PEAKS>(TimeWindow::Long, peak_floor(input.upload_history, 1.0))?;
        Ok(Self {
            summary,
            download_sparkline,
            upload_sparkline,
            comparison,
            download_peaks,
            upload_peaks,
            download,
            upload,
            download_total: contextual_metric_value(
                input.download_total,
                MetricUnit::Bytes,
                VisualPriority::Contextual,
            ),
            upload_total: contextual_metric_value(
                input.upload_total,
                MetricUnit::Bytes,
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

    pub const fn download_sparkline(self) -> SparklineModel<CHART_POINTS> {
        self.download_sparkline
    }

    pub const fn upload_sparkline(self) -> SparklineModel<CHART_POINTS> {
        self.upload_sparkline
    }

    pub const fn comparison(self) -> SeriesComparison {
        self.comparison
    }

    pub const fn download_peaks(self) -> PeakSet<MAX_PEAKS> {
        self.download_peaks
    }

    pub const fn upload_peaks(self) -> PeakSet<MAX_PEAKS> {
        self.upload_peaks
    }

    pub const fn download(self) -> Option<MetricValue> {
        self.download
    }

    pub const fn upload(self) -> Option<MetricValue> {
        self.upload
    }

    pub const fn download_total(self) -> Option<MetricValue> {
        self.download_total
    }

    pub const fn upload_total(self) -> Option<MetricValue> {
        self.upload_total
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

#[cfg(test)]
mod tests {
    use super::super::super::MetricAvailability;
    use super::super::super::TrendDirection;
    use super::super::super::chart::TimeSample;
    use super::super::types::MetricReading;
    use super::*;
    use std::time::Duration;

    fn history(values: &[f64]) -> TimeSeries<8> {
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

    fn input(download: f64, upload: f64) -> NetworkMetricInput<8> {
        NetworkMetricInput::new(
            MetricReading::non_negative(Some(download), MetricAvailability::Available).unwrap(),
            MetricReading::non_negative(Some(upload), MetricAvailability::Available).unwrap(),
            MetricReading::non_negative(Some(1000.0), MetricAvailability::Available).unwrap(),
            MetricReading::non_negative(Some(500.0), MetricAvailability::Available).unwrap(),
            MetricReading::non_negative(Some(100.0), MetricAvailability::Available).unwrap(),
            history(&[download * 0.5, download]),
            history(&[upload * 0.5, upload]),
        )
        .unwrap()
    }

    #[test]
    fn idle_network_is_healthy_and_directions_remain_separate() {
        let model = NetworkMetricModel::from_input(input(0.0, 0.0)).unwrap();
        assert_eq!(model.state(), WidgetState::Normal);
        assert_eq!(model.visual().widget_id(), DashboardWidgetId::Network);
        assert!(model.download().is_some());
        assert!(model.upload().is_some());
        assert_eq!(model.comparison().aligned_samples(), 2);
    }

    #[test]
    fn network_exposes_dominant_direction_and_comparison() {
        let download = NetworkMetricModel::from_input(input(80.0, 20.0)).unwrap();
        assert_eq!(download.visual().primary().unwrap().value(), 80.0);
        assert_eq!(download.comparison().direction(), TrendDirection::Rising);
        assert_eq!(download.download_sparkline().stats().current(), Some(80.0));

        let upload = NetworkMetricModel::from_input(input(20.0, 80.0)).unwrap();
        assert_eq!(upload.visual().primary().unwrap().value(), 80.0);
        assert_eq!(upload.visual().secondary().unwrap().value(), 20.0);
    }

    #[test]
    fn network_missing_data_is_not_treated_as_idle() {
        let input = NetworkMetricInput::new(
            MetricReading::unavailable(),
            MetricReading::unavailable(),
            MetricReading::unavailable(),
            MetricReading::unavailable(),
            MetricReading::unavailable(),
            history(&[]),
            history(&[]),
        )
        .unwrap();
        let model = NetworkMetricModel::from_input(input).unwrap();
        assert_eq!(model.state(), WidgetState::Unavailable);
        assert!(!model.is_available());
    }
}
