use std::time::{Duration, Instant};

pub struct Timeline {
    events: Vec<String>,
    recorded_at: Vec<Instant>,
    limit: usize,
}

impl Timeline {
    pub fn new(limit: usize) -> Self {
        Self {
            events: Vec::with_capacity(limit),
            recorded_at: Vec::with_capacity(limit),
            limit,
        }
    }

    pub fn record(&mut self, event: impl Into<String>) {
        if self.limit == 0 {
            return;
        }

        if self.events.len() == self.limit {
            self.events.remove(0);
            self.recorded_at.remove(0);
        }

        self.events.push(event.into());
        self.recorded_at.push(Instant::now());
    }

    pub fn events(&self) -> &[String] {
        &self.events
    }

    /// Age of each event at `now`, aligned with `events()`.
    pub fn ages(&self, now: Instant) -> Vec<Duration> {
        self.recorded_at
            .iter()
            .map(|recorded_at| now.saturating_duration_since(*recorded_at))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::Timeline;

    #[test]
    fn keeps_recent_events_in_stable_order() {
        let mut timeline = Timeline::new(2);
        timeline.record("first");
        timeline.record("second");
        timeline.record("third");

        assert_eq!(timeline.events(), &["second", "third"]);
        assert_eq!(timeline.ages(std::time::Instant::now()).len(), 2);
    }

    #[test]
    fn handles_a_zero_limit() {
        let mut timeline = Timeline::new(0);
        timeline.record("ignored");

        assert!(timeline.events().is_empty());
    }
}
