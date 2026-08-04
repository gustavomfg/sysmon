pub struct Timeline {
    events: Vec<String>,
    limit: usize,
}

impl Timeline {
    pub fn new(limit: usize) -> Self {
        Self {
            events: Vec::with_capacity(limit),
            limit,
        }
    }

    pub fn record(&mut self, event: impl Into<String>) {
        if self.limit == 0 {
            return;
        }

        if self.events.len() == self.limit {
            self.events.remove(0);
        }

        self.events.push(event.into());
    }

    pub fn events(&self) -> &[String] {
        &self.events
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
    }

    #[test]
    fn handles_a_zero_limit() {
        let mut timeline = Timeline::new(0);
        timeline.record("ignored");

        assert!(timeline.events().is_empty());
    }
}
