use std::time::Duration;

use super::{
    Intensity, MetricAvailability, MetricUnit, MetricValue, PeriodComparison, Trend,
    TrendDirection, VisualPriority, WidgetState,
};

pub const MAX_REFERENCE_BANDS: usize = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChartError {
    NonFiniteValue,
    InvalidValue,
    CapacityZero,
    CapacityExceeded,
    TemporalOrder,
    InvalidWindow,
    InvalidRange,
    MismatchedUnits,
    DuplicateSegment,
    InvalidTotal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeWindow {
    Short,
    Medium,
    Long,
    Custom(usize),
}

impl TimeWindow {
    pub fn sample_limit(self, capacity: usize) -> usize {
        match self {
            Self::Short => (capacity / 4).max(1).min(capacity),
            Self::Medium => (capacity / 2).max(1).min(capacity),
            Self::Long => capacity,
            Self::Custom(limit) => limit.min(capacity),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ChartEventKind {
    Peak,
    Warning,
    Critical,
    Transition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EventMarker {
    kind: ChartEventKind,
    severity: WidgetState,
    duration: Option<Duration>,
}

impl EventMarker {
    pub const fn new(
        kind: ChartEventKind,
        severity: WidgetState,
        duration: Option<Duration>,
    ) -> Self {
        Self {
            kind,
            severity,
            duration,
        }
    }

    pub const fn kind(self) -> ChartEventKind {
        self.kind
    }

    pub const fn severity(self) -> WidgetState {
        self.severity
    }

    pub const fn duration(self) -> Option<Duration> {
        self.duration
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TimeSample {
    offset: Duration,
    value: Option<f64>,
    availability: MetricAvailability,
    event: Option<EventMarker>,
}

impl TimeSample {
    pub fn new(
        offset: Duration,
        value: Option<f64>,
        availability: MetricAvailability,
        event: Option<EventMarker>,
    ) -> Result<Self, ChartError> {
        if value.is_some_and(|value| !value.is_finite()) {
            return Err(ChartError::NonFiniteValue);
        }
        Ok(Self {
            offset,
            value,
            availability,
            event,
        })
    }

    pub const fn offset(self) -> Duration {
        self.offset
    }

    pub const fn value(self) -> Option<f64> {
        self.value
    }

    pub const fn availability(self) -> MetricAvailability {
        self.availability
    }

    pub const fn event(self) -> Option<EventMarker> {
        self.event
    }

    pub fn usable_value(self) -> Option<f64> {
        self.value.filter(|_| self.availability.is_usable())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PushOutcome {
    Added,
    EvictedOldest,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SeriesStats {
    unit: MetricUnit,
    minimum: Option<f64>,
    maximum: Option<f64>,
    current: Option<f64>,
    current_availability: MetricAvailability,
    sample_count: usize,
    available_samples: usize,
    missing_samples: usize,
}

impl SeriesStats {
    pub const fn unit(self) -> MetricUnit {
        self.unit
    }

    pub const fn minimum(self) -> Option<f64> {
        self.minimum
    }

    pub const fn maximum(self) -> Option<f64> {
        self.maximum
    }

    pub const fn current(self) -> Option<f64> {
        self.current
    }

    pub const fn current_availability(self) -> MetricAvailability {
        self.current_availability
    }

    pub const fn sample_count(self) -> usize {
        self.sample_count
    }

    pub const fn available_samples(self) -> usize {
        self.available_samples
    }

    pub const fn missing_samples(self) -> usize {
        self.missing_samples
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Volatility {
    intensity: Intensity,
    mean_absolute_delta: Option<f64>,
    relative_change: Option<f64>,
}

impl Volatility {
    pub const fn unknown() -> Self {
        Self {
            intensity: Intensity::VeryLow,
            mean_absolute_delta: None,
            relative_change: None,
        }
    }

    pub const fn intensity(self) -> Intensity {
        self.intensity
    }

    pub const fn mean_absolute_delta(self) -> Option<f64> {
        self.mean_absolute_delta
    }

    pub const fn relative_change(self) -> Option<f64> {
        self.relative_change
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MovingAverage {
    kind: MovingAverageKind,
    window: usize,
    value: Option<f64>,
}

impl MovingAverage {
    pub const fn kind(self) -> MovingAverageKind {
        self.kind
    }

    pub const fn window(self) -> usize {
        self.window
    }

    pub const fn value(self) -> Option<f64> {
        self.value
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MovingAverageKind {
    Simple,
    Exponential,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChartPoint {
    sample: TimeSample,
    aggregation: PointAggregation,
}

impl ChartPoint {
    pub const fn sample(self) -> TimeSample {
        self.sample
    }

    pub const fn aggregation(self) -> PointAggregation {
        self.aggregation
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointAggregation {
    Original,
    Minimum,
    Maximum,
    Representative,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DownsampledSeries<const POINTS: usize> {
    points: [Option<ChartPoint>; POINTS],
    length: usize,
    source_count: usize,
}

impl<const POINTS: usize> DownsampledSeries<POINTS> {
    const fn empty(source_count: usize) -> Self {
        Self {
            points: [None; POINTS],
            length: 0,
            source_count,
        }
    }

    pub const fn len(self) -> usize {
        self.length
    }

    pub const fn is_empty(self) -> bool {
        self.length == 0
    }

    pub const fn source_count(self) -> usize {
        self.source_count
    }

    pub fn iter(self) -> impl Iterator<Item = ChartPoint> {
        self.points.into_iter().take(self.length).flatten()
    }

    pub fn first(self) -> Option<ChartPoint> {
        self.iter().next()
    }

    pub fn last(self) -> Option<ChartPoint> {
        self.iter().last()
    }

    fn push(&mut self, point: ChartPoint) {
        if self.length < POINTS {
            self.points[self.length] = Some(point);
            self.length += 1;
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SparklineModel<const POINTS: usize> {
    points: DownsampledSeries<POINTS>,
    stats: SeriesStats,
    trend: Trend,
    volatility: Volatility,
}

impl<const POINTS: usize> SparklineModel<POINTS> {
    pub const fn points(self) -> DownsampledSeries<POINTS> {
        self.points
    }

    pub const fn stats(self) -> SeriesStats {
        self.stats
    }

    pub const fn trend(self) -> Trend {
        self.trend
    }

    pub const fn volatility(self) -> Volatility {
        self.volatility
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PeakEvent {
    offset: Duration,
    value_bits: u64,
    severity: WidgetState,
    kind: ChartEventKind,
    duration: Option<Duration>,
}

impl PeakEvent {
    fn new(sample: TimeSample, value: f64) -> Self {
        let marker = sample.event();
        Self {
            offset: sample.offset(),
            value_bits: value.to_bits(),
            severity: marker.map_or(WidgetState::Active, EventMarker::severity),
            kind: marker.map_or(ChartEventKind::Peak, EventMarker::kind),
            duration: marker.and_then(EventMarker::duration),
        }
    }

    pub const fn offset(self) -> Duration {
        self.offset
    }

    pub fn value(self) -> f64 {
        f64::from_bits(self.value_bits)
    }

    pub const fn severity(self) -> WidgetState {
        self.severity
    }

    pub const fn kind(self) -> ChartEventKind {
        self.kind
    }

    pub const fn duration(self) -> Option<Duration> {
        self.duration
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PeakSet<const MAX_PEAKS: usize> {
    events: [Option<PeakEvent>; MAX_PEAKS],
    length: usize,
}

impl<const MAX_PEAKS: usize> PeakSet<MAX_PEAKS> {
    const fn empty() -> Self {
        Self {
            events: [None; MAX_PEAKS],
            length: 0,
        }
    }

    pub const fn len(self) -> usize {
        self.length
    }

    pub const fn is_empty(self) -> bool {
        self.length == 0
    }

    pub fn iter(self) -> impl Iterator<Item = PeakEvent> {
        self.events.into_iter().take(self.length).flatten()
    }

    fn push(&mut self, event: PeakEvent) {
        if self.length < MAX_PEAKS {
            self.events[self.length] = Some(event);
            self.length += 1;
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReferenceKind {
    Normal,
    Attention,
    Warning,
    Critical,
    PhysicalLimit,
    Capacity,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReferenceRange {
    lower: f64,
    upper: f64,
    unit: MetricUnit,
    kind: ReferenceKind,
}

impl ReferenceRange {
    pub fn new(
        lower: f64,
        upper: f64,
        unit: MetricUnit,
        kind: ReferenceKind,
    ) -> Result<Self, ChartError> {
        if !lower.is_finite() || !upper.is_finite() {
            return Err(ChartError::NonFiniteValue);
        }
        if lower > upper {
            return Err(ChartError::InvalidRange);
        }
        Ok(Self {
            lower,
            upper,
            unit,
            kind,
        })
    }

    pub const fn lower(self) -> f64 {
        self.lower
    }

    pub const fn upper(self) -> f64 {
        self.upper
    }

    pub const fn unit(self) -> MetricUnit {
        self.unit
    }

    pub const fn kind(self) -> ReferenceKind {
        self.kind
    }

    pub fn contains(self, value: f64) -> bool {
        value >= self.lower && value <= self.upper
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReferenceBands {
    unit: MetricUnit,
    ranges: [Option<ReferenceRange>; MAX_REFERENCE_BANDS],
    length: usize,
}

impl ReferenceBands {
    pub const fn new(unit: MetricUnit) -> Self {
        Self {
            unit,
            ranges: [None; MAX_REFERENCE_BANDS],
            length: 0,
        }
    }

    pub const fn unit(self) -> MetricUnit {
        self.unit
    }

    pub const fn len(self) -> usize {
        self.length
    }

    pub const fn is_empty(self) -> bool {
        self.length == 0
    }

    pub fn push(&mut self, range: ReferenceRange) -> Result<(), ChartError> {
        if self.length >= MAX_REFERENCE_BANDS {
            return Err(ChartError::CapacityExceeded);
        }
        if range.unit() != self.unit {
            return Err(ChartError::MismatchedUnits);
        }
        if self
            .ranges
            .get(self.length.saturating_sub(1))
            .and_then(|range| *range)
            .is_some_and(|previous| previous.upper() > range.lower())
        {
            return Err(ChartError::InvalidRange);
        }
        self.ranges[self.length] = Some(range);
        self.length += 1;
        Ok(())
    }

    pub fn iter(self) -> impl Iterator<Item = ReferenceRange> {
        self.ranges.into_iter().take(self.length).flatten()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AreaModel {
    base: MetricValue,
    current: Option<MetricValue>,
    capacity: Option<MetricValue>,
    bands: ReferenceBands,
}

impl AreaModel {
    pub fn new(
        base: MetricValue,
        current: Option<MetricValue>,
        capacity: Option<MetricValue>,
        bands: ReferenceBands,
    ) -> Result<Self, ChartError> {
        if base.value() < 0.0
            || current.is_some_and(|value| value.value() < 0.0)
            || capacity.is_some_and(|value| value.value() < 0.0)
        {
            return Err(ChartError::InvalidValue);
        }
        for value in [current, capacity].into_iter().flatten() {
            if value.unit() != base.unit() {
                return Err(ChartError::MismatchedUnits);
            }
        }
        if bands.unit() != base.unit() {
            return Err(ChartError::MismatchedUnits);
        }
        Ok(Self {
            base,
            current,
            capacity,
            bands,
        })
    }

    pub const fn base(self) -> MetricValue {
        self.base
    }

    pub const fn current(self) -> Option<MetricValue> {
        self.current
    }

    pub const fn capacity(self) -> Option<MetricValue> {
        self.capacity
    }

    pub const fn bands(self) -> ReferenceBands {
        self.bands
    }

    pub fn occupancy(self) -> Option<f64> {
        let current = self.current.filter(|value| value.is_usable())?;
        let capacity = self.capacity.filter(|value| value.is_usable())?;
        if capacity.value() <= 0.0 {
            return None;
        }
        let occupancy = current.value() / capacity.value();
        occupancy.is_finite().then_some(occupancy)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SegmentId {
    User,
    System,
    Idle,
    Cache,
    Used,
    Free,
    Upload,
    Download,
    Read,
    Write,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChartSegment {
    id: SegmentId,
    value: Option<MetricValue>,
}

impl ChartSegment {
    pub const fn new(id: SegmentId, value: Option<MetricValue>) -> Self {
        Self { id, value }
    }

    pub const fn id(self) -> SegmentId {
        self.id
    }

    pub const fn value(self) -> Option<MetricValue> {
        self.value
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SegmentedBars<const MAX_SEGMENTS: usize> {
    total: Option<MetricValue>,
    unit: Option<MetricUnit>,
    segments: [Option<ChartSegment>; MAX_SEGMENTS],
    length: usize,
}

impl<const MAX_SEGMENTS: usize> SegmentedBars<MAX_SEGMENTS> {
    pub fn new(total: Option<MetricValue>) -> Result<Self, ChartError> {
        if total.is_some_and(|total| total.value() < 0.0) {
            return Err(ChartError::InvalidTotal);
        }
        Ok(Self {
            total,
            unit: total.map(MetricValue::unit),
            segments: [None; MAX_SEGMENTS],
            length: 0,
        })
    }

    pub const fn total(self) -> Option<MetricValue> {
        self.total
    }

    pub const fn len(self) -> usize {
        self.length
    }

    pub const fn is_empty(self) -> bool {
        self.length == 0
    }

    pub fn iter(self) -> impl Iterator<Item = ChartSegment> {
        self.segments.into_iter().take(self.length).flatten()
    }

    pub fn push(&mut self, segment: ChartSegment) -> Result<(), ChartError> {
        if self.length >= MAX_SEGMENTS {
            return Err(ChartError::CapacityExceeded);
        }
        if self.iter().any(|current| current.id() == segment.id()) {
            return Err(ChartError::DuplicateSegment);
        }
        if let Some(value) = segment.value() {
            if value.value() < 0.0 {
                return Err(ChartError::InvalidValue);
            }
            if let Some(unit) = self.unit
                && value.unit() != unit
            {
                return Err(ChartError::MismatchedUnits);
            }
        }

        let new_sum = self.sum_with(segment.value());
        if let Some(total) = self.total.filter(|total| total.is_usable())
            && new_sum > total.value() + 1.0e-9
        {
            return Err(ChartError::InvalidTotal);
        }

        if self.unit.is_none() {
            self.unit = segment.value().map(MetricValue::unit);
        }
        let mut insert_at = self.length;
        for (index, current) in self.segments.iter().enumerate().take(self.length) {
            if current.is_some_and(|current| segment.id() < current.id()) {
                insert_at = index;
                break;
            }
        }
        let mut index = self.length;
        while index > insert_at {
            self.segments[index] = self.segments[index - 1];
            index -= 1;
        }
        self.segments[insert_at] = Some(segment);
        self.length += 1;
        Ok(())
    }

    pub fn normalized(self, id: SegmentId) -> Option<f64> {
        let total = self.total.filter(|total| total.is_usable())?.value();
        let segment = self.iter().find(|segment| segment.id() == id)?.value()?;
        if !segment.is_usable() || total <= 0.0 {
            return None;
        }
        let normalized = segment.value() / total;
        normalized.is_finite().then_some(normalized)
    }

    fn sum_with(self, value: Option<MetricValue>) -> f64 {
        self.iter()
            .filter_map(|segment| segment.value())
            .chain(value)
            .filter(|value| value.is_usable())
            .map(MetricValue::value)
            .sum()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SeriesComparison {
    difference: Option<f64>,
    ratio: Option<f64>,
    aligned_samples: usize,
    direction: TrendDirection,
}

impl SeriesComparison {
    pub const fn difference(self) -> Option<f64> {
        self.difference
    }

    pub const fn ratio(self) -> Option<f64> {
        self.ratio
    }

    pub const fn aligned_samples(self) -> usize {
        self.aligned_samples
    }

    pub const fn direction(self) -> TrendDirection {
        self.direction
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TimeSeries<const CAPACITY: usize> {
    unit: MetricUnit,
    samples: [Option<TimeSample>; CAPACITY],
    length: usize,
}

impl<const CAPACITY: usize> TimeSeries<CAPACITY> {
    pub const fn new(unit: MetricUnit) -> Self {
        Self {
            unit,
            samples: [None; CAPACITY],
            length: 0,
        }
    }

    pub const fn capacity(self) -> usize {
        CAPACITY
    }

    pub const fn len(self) -> usize {
        self.length
    }

    pub const fn unit(self) -> MetricUnit {
        self.unit
    }

    pub fn is_empty(self) -> bool {
        self.length == 0
    }

    pub fn push(&mut self, sample: TimeSample) -> Result<PushOutcome, ChartError> {
        if CAPACITY == 0 {
            return Err(ChartError::CapacityZero);
        }
        if self
            .last()
            .is_some_and(|last| sample.offset() <= last.offset())
        {
            return Err(ChartError::TemporalOrder);
        }
        if self.length == CAPACITY {
            for index in 1..CAPACITY {
                self.samples[index - 1] = self.samples[index];
            }
            self.samples[CAPACITY - 1] = Some(sample);
            Ok(PushOutcome::EvictedOldest)
        } else {
            self.samples[self.length] = Some(sample);
            self.length += 1;
            Ok(PushOutcome::Added)
        }
    }

    pub fn get(self, index: usize) -> Option<TimeSample> {
        (index < self.length).then(|| self.samples[index]).flatten()
    }

    pub fn iter(self) -> impl Iterator<Item = TimeSample> {
        self.samples.into_iter().take(self.length).flatten()
    }

    pub fn first(self) -> Option<TimeSample> {
        self.get(0)
    }

    pub fn last(self) -> Option<TimeSample> {
        self.get(self.length.saturating_sub(1))
    }

    pub fn stats(self) -> SeriesStats {
        self.stats_from(0)
    }

    pub fn moving_average(
        self,
        kind: MovingAverageKind,
        window: usize,
    ) -> Result<MovingAverage, ChartError> {
        if window == 0 {
            return Err(ChartError::InvalidWindow);
        }
        let start = self.length.saturating_sub(window);
        let mut count = 0usize;
        let mut sum = 0.0;
        let mut exponential = None;
        let alpha = 2.0 / (window as f64 + 1.0);
        for sample in self.iter().skip(start) {
            let Some(value) = sample.usable_value() else {
                continue;
            };
            if !value.is_finite() {
                return Err(ChartError::NonFiniteValue);
            }
            count += 1;
            sum += value;
            exponential = Some(match exponential {
                Some(previous) => alpha * value + (1.0 - alpha) * previous,
                None => value,
            });
        }
        let result = match kind {
            MovingAverageKind::Simple => (count > 0).then_some(sum / count as f64),
            MovingAverageKind::Exponential => exponential,
        };
        Ok(MovingAverage {
            kind,
            window,
            value: result.filter(|value| value.is_finite()),
        })
    }

    pub fn sparkline<const POINTS: usize>(self, window: TimeWindow) -> SparklineModel<POINTS> {
        let start = self.window_start(window);
        SparklineModel {
            points: self.downsample_from::<POINTS>(start),
            stats: self.stats_from(start),
            trend: self.trend_from(start),
            volatility: self.volatility_from(start),
        }
    }

    pub fn detect_peaks<const MAX_PEAKS: usize>(
        self,
        window: TimeWindow,
        minimum: f64,
    ) -> Result<PeakSet<MAX_PEAKS>, ChartError> {
        if !minimum.is_finite() {
            return Err(ChartError::NonFiniteValue);
        }
        let start = self.window_start(window);
        let mut peaks = PeakSet::empty();
        for index in start..self.length {
            let Some(sample) = self.get(index) else {
                continue;
            };
            let Some(value) = sample.usable_value() else {
                continue;
            };
            if value < minimum {
                continue;
            }
            let Some(previous) = self.neighbor_value(index, -1, start, self.length) else {
                continue;
            };
            let Some(next) = self.neighbor_value(index, 1, start, self.length) else {
                continue;
            };
            if value >= previous && value >= next {
                peaks.push(PeakEvent::new(sample, value));
            }
        }
        Ok(peaks)
    }

    pub fn compare<const OTHER: usize>(
        self,
        other: TimeSeries<OTHER>,
    ) -> Result<SeriesComparison, ChartError> {
        if self.unit != other.unit {
            return Err(ChartError::MismatchedUnits);
        }
        let mut aligned_samples = 0;
        let mut difference = None;
        let mut ratio = None;
        for left in self.iter() {
            let Some(left_value) = left.usable_value() else {
                continue;
            };
            let Some(right) = other.iter().find(|right| right.offset() == left.offset()) else {
                continue;
            };
            let Some(right_value) = right.usable_value() else {
                continue;
            };
            let current_difference = left_value - right_value;
            if !current_difference.is_finite() {
                continue;
            }
            aligned_samples += 1;
            difference = Some(current_difference);
            ratio = (right_value != 0.0)
                .then_some(left_value / right_value)
                .filter(|value| value.is_finite());
        }
        Ok(SeriesComparison {
            difference,
            ratio,
            aligned_samples,
            direction: difference.map_or(TrendDirection::Unknown, delta_direction),
        })
    }

    pub fn volatility(self, window: TimeWindow) -> Volatility {
        self.volatility_from(self.window_start(window))
    }

    fn window_start(self, window: TimeWindow) -> usize {
        self.length.saturating_sub(window.sample_limit(CAPACITY))
    }

    fn stats_from(self, start: usize) -> SeriesStats {
        let mut minimum = None;
        let mut maximum = None;
        let mut current = None;
        let mut available_samples = 0;
        let mut missing_samples = 0;
        let mut current_availability = MetricAvailability::Unavailable;
        for sample in self.iter().skip(start) {
            current_availability = sample.availability();
            current = sample.usable_value();
            let Some(value) = sample.usable_value() else {
                missing_samples += 1;
                continue;
            };
            available_samples += 1;
            minimum = Some(minimum.map_or(value, |minimum: f64| minimum.min(value)));
            maximum = Some(maximum.map_or(value, |maximum: f64| maximum.max(value)));
            current = Some(value);
        }
        SeriesStats {
            unit: self.unit,
            minimum,
            maximum,
            current,
            current_availability,
            sample_count: self.length.saturating_sub(start),
            available_samples,
            missing_samples,
        }
    }

    fn trend_from(self, start: usize) -> Trend {
        let mut first = None;
        let mut last = None;
        for sample in self.iter().skip(start) {
            let Some(value) = sample.usable_value() else {
                continue;
            };
            let metric = MetricValue::new(value, self.unit, 0, VisualPriority::Primary);
            if let Some(metric) = metric {
                first.get_or_insert(metric);
                last = Some(metric);
            }
        }
        let direction = first
            .zip(last)
            .map(|(first, last)| PeriodComparison::from_values(last, Some(first)).direction())
            .unwrap_or(TrendDirection::Unknown);
        Trend::new(direction)
    }

    fn volatility_from(self, start: usize) -> Volatility {
        let mut previous: Option<f64> = None;
        let mut total_delta = 0.0;
        let mut changes = 0usize;
        let mut minimum = None;
        let mut maximum = None;
        for sample in self.iter().skip(start) {
            let Some(value) = sample.usable_value() else {
                continue;
            };
            minimum = Some(minimum.map_or(value, |minimum: f64| minimum.min(value)));
            maximum = Some(maximum.map_or(value, |maximum: f64| maximum.max(value)));
            if let Some(previous) = previous {
                total_delta += (value - previous).abs();
                changes += 1;
            }
            previous = Some(value);
        }
        if changes == 0 {
            return Volatility::unknown();
        }
        let mean_absolute_delta = total_delta / changes as f64;
        let range = maximum.unwrap_or(0.0) - minimum.unwrap_or(0.0);
        let relative_change = if range > 0.0 {
            Some(mean_absolute_delta / range)
        } else {
            Some(0.0)
        };
        let level = match relative_change.unwrap_or(0.0) {
            value if value < 0.05 => Intensity::VeryLow,
            value if value < 0.15 => Intensity::Low,
            value if value < 0.30 => Intensity::Normal,
            value if value < 0.60 => Intensity::High,
            _ => Intensity::VeryHigh,
        };
        Volatility {
            intensity: level,
            mean_absolute_delta: Some(mean_absolute_delta),
            relative_change,
        }
    }

    fn downsample_from<const POINTS: usize>(self, start: usize) -> DownsampledSeries<POINTS> {
        let source_count = self.length.saturating_sub(start);
        let mut result = DownsampledSeries::empty(source_count);
        if POINTS == 0 || source_count == 0 {
            return result;
        }
        if source_count <= POINTS {
            for sample in self.iter().skip(start) {
                result.push(ChartPoint {
                    sample,
                    aggregation: PointAggregation::Original,
                });
            }
            return result;
        }
        if POINTS == 1 {
            if let Some(sample) = self.get(start) {
                result.push(ChartPoint {
                    sample,
                    aggregation: PointAggregation::Original,
                });
            }
            return result;
        }

        let first = self.get(start);
        let last = self.get(self.length - 1);
        if let Some(first) = first {
            result.push(ChartPoint {
                sample: first,
                aggregation: PointAggregation::Original,
            });
        }
        let interior = source_count - 2;
        let buckets = POINTS - 2;
        for bucket in 0..buckets {
            let bucket_start = start + 1 + (interior * bucket / buckets);
            let bucket_end = start + 1 + (interior * (bucket + 1) / buckets);
            if let Some(point) = self.representative(bucket_start, bucket_end) {
                result.push(point);
            }
        }
        if let Some(last) = last {
            result.push(ChartPoint {
                sample: last,
                aggregation: PointAggregation::Original,
            });
        }
        result
    }

    fn representative(self, start: usize, end: usize) -> Option<ChartPoint> {
        let mut first_missing = None;
        let mut count = 0usize;
        let mut mean = 0.0;
        for sample in self.iter().skip(start).take(end.saturating_sub(start)) {
            if first_missing.is_none() && sample.usable_value().is_none() {
                first_missing = Some(sample);
            }
            if let Some(value) = sample.usable_value() {
                mean += value;
                count += 1;
            }
        }
        if count == 0 {
            return first_missing.map(|sample| ChartPoint {
                sample,
                aggregation: PointAggregation::Representative,
            });
        }
        mean /= count as f64;
        let mut selected = None;
        let mut selected_score = f64::NEG_INFINITY;
        let mut minimum = f64::INFINITY;
        let mut maximum = f64::NEG_INFINITY;
        for sample in self.iter().skip(start).take(end.saturating_sub(start)) {
            let Some(value) = sample.usable_value() else {
                continue;
            };
            minimum = minimum.min(value);
            maximum = maximum.max(value);
            let score = (value - mean).abs();
            if score > selected_score
                || (score == selected_score
                    && sample.event().is_some()
                    && selected.is_some_and(|current: TimeSample| current.event().is_none()))
            {
                selected = Some(sample);
                selected_score = score;
            }
        }
        let sample = selected?;
        let value = sample.usable_value()?;
        let aggregation = if value == maximum {
            PointAggregation::Maximum
        } else if value == minimum {
            PointAggregation::Minimum
        } else {
            PointAggregation::Representative
        };
        Some(ChartPoint {
            sample,
            aggregation,
        })
    }

    fn neighbor_value(
        self,
        index: usize,
        direction: isize,
        lower: usize,
        upper: usize,
    ) -> Option<f64> {
        let mut cursor = index as isize + direction;
        while cursor >= lower as isize && (cursor as usize) < upper {
            if let Some(value) = self.get(cursor as usize).and_then(TimeSample::usable_value) {
                return Some(value);
            }
            cursor += direction;
        }
        None
    }
}

fn delta_direction(delta: f64) -> TrendDirection {
    const STABILITY_EPSILON: f64 = 1.0e-9;
    if delta > STABILITY_EPSILON {
        TrendDirection::Rising
    } else if delta < -STABILITY_EPSILON {
        TrendDirection::Falling
    } else {
        TrendDirection::Stable
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn value(value: f64, unit: MetricUnit) -> MetricValue {
        MetricValue::new(value, unit, 1, VisualPriority::Primary).unwrap()
    }

    fn sample(offset: u64, value: Option<f64>) -> TimeSample {
        TimeSample::new(
            Duration::from_secs(offset),
            value,
            value.map_or(MetricAvailability::Unavailable, |_| {
                MetricAvailability::Available
            }),
            None,
        )
        .unwrap()
    }

    fn series() -> TimeSeries<8> {
        let mut series = TimeSeries::new(MetricUnit::Percent);
        for (offset, value) in [(0, 10.0), (1, 20.0), (2, 80.0), (3, 30.0), (4, 40.0)] {
            series.push(sample(offset, Some(value))).unwrap();
        }
        series
    }

    #[test]
    fn series_validates_order_capacity_and_missing_values() {
        let mut series = TimeSeries::<2>::new(MetricUnit::Percent);
        assert_eq!(series.capacity(), 2);
        series.push(sample(0, Some(10.0))).unwrap();
        assert!(matches!(
            series.push(sample(0, Some(11.0))),
            Err(ChartError::TemporalOrder)
        ));
        series.push(sample(1, None)).unwrap();
        assert_eq!(series.len(), 2);
        assert_eq!(series.stats().missing_samples(), 1);
        assert_eq!(series.stats().available_samples(), 1);
        assert_eq!(
            series.push(sample(2, Some(20.0))).unwrap(),
            PushOutcome::EvictedOldest
        );
        assert_eq!(series.first().unwrap().offset(), Duration::from_secs(1));

        let mut trailing_missing = TimeSeries::<2>::new(MetricUnit::Percent);
        trailing_missing.push(sample(0, Some(10.0))).unwrap();
        trailing_missing.push(sample(1, None)).unwrap();
        assert_eq!(trailing_missing.stats().current(), None);
        assert_eq!(
            trailing_missing.stats().current_availability(),
            MetricAvailability::Unavailable
        );

        let mut zero = TimeSeries::<0>::new(MetricUnit::None);
        assert!(matches!(
            zero.push(sample(0, Some(1.0))),
            Err(ChartError::CapacityZero)
        ));
    }

    #[test]
    fn statistics_trend_volatility_and_moving_average_are_deterministic() {
        let series = series();
        let stats = series.stats();
        assert_eq!(stats.minimum(), Some(10.0));
        assert_eq!(stats.maximum(), Some(80.0));
        assert_eq!(stats.current(), Some(40.0));
        assert_eq!(
            series.sparkline::<4>(TimeWindow::Long),
            series.sparkline::<4>(TimeWindow::Long)
        );
        assert_eq!(series.trend_from(0).direction(), TrendDirection::Rising);
        assert!(
            series
                .volatility(TimeWindow::Long)
                .mean_absolute_delta()
                .is_some()
        );
        assert_eq!(
            series
                .moving_average(MovingAverageKind::Simple, 2)
                .unwrap()
                .value(),
            Some(35.0)
        );
        assert!(
            series
                .moving_average(MovingAverageKind::Exponential, 2)
                .unwrap()
                .value()
                .is_some()
        );
        assert!(matches!(
            series.moving_average(MovingAverageKind::Simple, 0),
            Err(ChartError::InvalidWindow)
        ));
    }

    #[test]
    fn downsampling_preserves_endpoints_shape_and_peak() {
        let sparkline = series().sparkline::<4>(TimeWindow::Long);
        let points = sparkline.points();
        assert_eq!(points.len(), 4);
        assert_eq!(
            points.first().unwrap().sample().offset(),
            Duration::from_secs(0)
        );
        assert_eq!(
            points.last().unwrap().sample().offset(),
            Duration::from_secs(4)
        );
        assert!(
            points
                .iter()
                .any(|point| point.sample().value() == Some(80.0))
        );
        assert!(
            points
                .iter()
                .any(|point| point.aggregation() == PointAggregation::Maximum)
        );
        assert_eq!(points.source_count(), 5);
    }

    #[test]
    fn peak_detection_preserves_events_and_limits_output() {
        let mut series = TimeSeries::<8>::new(MetricUnit::Percent);
        for (offset, value) in [(0, 10.0), (1, 70.0), (2, 20.0), (3, 90.0), (4, 10.0)] {
            let event = (value > 80.0).then_some(EventMarker::new(
                ChartEventKind::Critical,
                WidgetState::Critical,
                Some(Duration::from_secs(1)),
            ));
            series
                .push(
                    TimeSample::new(
                        Duration::from_secs(offset),
                        Some(value),
                        MetricAvailability::Available,
                        event,
                    )
                    .unwrap(),
                )
                .unwrap();
        }
        let peaks = series.detect_peaks::<2>(TimeWindow::Long, 50.0).unwrap();
        assert_eq!(peaks.len(), 2);
        assert_eq!(peaks.iter().next().unwrap().value(), 70.0);
        assert_eq!(
            peaks.iter().last().unwrap().kind(),
            ChartEventKind::Critical
        );
        assert_eq!(
            peaks.iter().last().unwrap().severity(),
            WidgetState::Critical
        );
    }

    #[test]
    fn reference_bands_are_ordered_and_area_is_normalized() {
        let mut bands = ReferenceBands::new(MetricUnit::Percent);
        bands
            .push(
                ReferenceRange::new(0.0, 60.0, MetricUnit::Percent, ReferenceKind::Normal).unwrap(),
            )
            .unwrap();
        bands
            .push(
                ReferenceRange::new(60.0, 85.0, MetricUnit::Percent, ReferenceKind::Warning)
                    .unwrap(),
            )
            .unwrap();
        assert!(matches!(
            bands.push(
                ReferenceRange::new(80.0, 100.0, MetricUnit::Percent, ReferenceKind::Critical)
                    .unwrap()
            ),
            Err(ChartError::InvalidRange)
        ));
        let area = AreaModel::new(
            value(10.0, MetricUnit::Percent),
            Some(value(75.0, MetricUnit::Percent)),
            Some(value(100.0, MetricUnit::Percent)),
            bands,
        )
        .unwrap();
        assert_eq!(area.occupancy(), Some(0.75));
        assert_eq!(area.bands().len(), 2);
        assert!(matches!(
            AreaModel::new(
                value(1.0, MetricUnit::Percent),
                Some(value(1.0, MetricUnit::Bytes)),
                None,
                ReferenceBands::new(MetricUnit::Percent)
            ),
            Err(ChartError::MismatchedUnits)
        ));
    }

    #[test]
    fn segmented_bars_are_sorted_normalized_and_keep_unavailable_segments() {
        let mut bars = SegmentedBars::<4>::new(Some(value(100.0, MetricUnit::Percent))).unwrap();
        bars.push(ChartSegment::new(
            SegmentId::Idle,
            Some(value(40.0, MetricUnit::Percent)),
        ))
        .unwrap();
        bars.push(ChartSegment::new(
            SegmentId::User,
            Some(value(50.0, MetricUnit::Percent)),
        ))
        .unwrap();
        bars.push(ChartSegment::new(
            SegmentId::System,
            Some(
                value(0.0, MetricUnit::Percent).with_availability(MetricAvailability::Unavailable),
            ),
        ))
        .unwrap();
        assert_eq!(bars.iter().next().unwrap().id(), SegmentId::User);
        assert_eq!(bars.normalized(SegmentId::Idle), Some(0.4));
        assert_eq!(bars.normalized(SegmentId::System), None);

        let unavailable_total =
            value(100.0, MetricUnit::Percent).with_availability(MetricAvailability::Unavailable);
        let mut unavailable_bars = SegmentedBars::<2>::new(Some(unavailable_total)).unwrap();
        unavailable_bars
            .push(ChartSegment::new(
                SegmentId::Used,
                Some(value(20.0, MetricUnit::Percent)),
            ))
            .unwrap();
        assert_eq!(unavailable_bars.normalized(SegmentId::Used), None);

        assert!(matches!(
            bars.push(ChartSegment::new(
                SegmentId::Other,
                Some(value(20.0, MetricUnit::Percent))
            )),
            Err(ChartError::InvalidTotal)
        ));
    }

    #[test]
    fn series_comparison_aligns_time_and_reuses_metric_units() {
        let left = series();
        let mut right = TimeSeries::<8>::new(MetricUnit::Percent);
        for (offset, value) in [(0, 5.0), (1, 10.0), (2, 20.0), (4, 30.0)] {
            right.push(sample(offset, Some(value))).unwrap();
        }
        let comparison = left.compare(right).unwrap();
        assert_eq!(comparison.aligned_samples(), 4);
        assert_eq!(comparison.difference(), Some(10.0));
        assert_eq!(comparison.ratio(), Some(40.0 / 30.0));
        assert_eq!(comparison.direction(), TrendDirection::Rising);
        let bytes = TimeSeries::<2>::new(MetricUnit::Bytes);
        assert!(matches!(
            left.compare(bytes),
            Err(ChartError::MismatchedUnits)
        ));
    }

    #[test]
    fn windows_are_intentional_and_bounded_by_series_capacity() {
        assert_eq!(TimeWindow::Short.sample_limit(8), 2);
        assert_eq!(TimeWindow::Medium.sample_limit(8), 4);
        assert_eq!(TimeWindow::Long.sample_limit(8), 8);
        assert_eq!(TimeWindow::Custom(99).sample_limit(8), 8);
        assert_eq!(TimeWindow::Custom(0).sample_limit(8), 0);
    }
}
