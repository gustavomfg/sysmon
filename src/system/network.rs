use std::collections::BTreeMap;
use std::time::Duration;

use sysinfo::Networks;

#[derive(Debug, Clone)]
pub struct NetworkInfo {
    pub id: String,
    pub interface: String,
    pub received_per_sec: f64,
    pub transmitted_per_sec: f64,
    /// None marks startup, a newly observed interface, or a counter reset.
    pub sample_interval: Option<Duration>,
}

impl NetworkInfo {
    pub fn get(networks: &Networks, interval: Duration) -> Vec<Self> {
        let mut interfaces = Self::get_all(networks, interval);
        interfaces.sort_by(|left, right| {
            let left_total = left.received_per_sec + left.transmitted_per_sec;
            let right_total = right.received_per_sec + right.transmitted_per_sec;
            right_total
                .partial_cmp(&left_total)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| left.id.cmp(&right.id))
        });
        interfaces.truncate(5);
        interfaces
    }

    pub fn get_all(networks: &Networks, interval: Duration) -> Vec<Self> {
        let mut interfaces = networks
            .iter()
            .map(|(name, data)| Self {
                id: name.to_string(),
                interface: name.to_string(),
                received_per_sec: bytes_per_second(data.received(), interval),
                transmitted_per_sec: bytes_per_second(data.transmitted(), interval),
                sample_interval: (!interval.is_zero()).then_some(interval),
            })
            .collect::<Vec<Self>>();

        sort_deterministically(&mut interfaces);
        interfaces
    }

    pub(crate) fn collected(
        networks: &Networks,
        interval: Duration,
        previous: &BTreeMap<String, (u64, u64)>,
    ) -> Vec<Self> {
        let mut interfaces = Self::get_all(networks, interval);
        for interface in &mut interfaces {
            if let Some((_, data)) = networks
                .iter()
                .find(|(name, _)| *name == &interface.interface)
            {
                interface.sample_interval = valid_interval(
                    previous.get(&interface.interface).copied(),
                    (data.total_received(), data.total_transmitted()),
                    interval,
                );
            }
        }
        interfaces
    }
    pub(crate) fn legacy_view(mut interfaces: Vec<Self>) -> Vec<Self> {
        interfaces.sort_by(|a, b| {
            (b.received_per_sec + b.transmitted_per_sec)
                .total_cmp(&(a.received_per_sec + a.transmitted_per_sec))
                .then_with(|| a.id.cmp(&b.id))
        });
        interfaces.truncate(5);
        interfaces
    }
}

fn valid_interval(
    previous: Option<(u64, u64)>,
    current: (u64, u64),
    interval: Duration,
) -> Option<Duration> {
    previous
        .filter(|(rx, tx)| current.0 >= *rx && current.1 >= *tx)
        .and_then(|_| (!interval.is_zero()).then_some(interval))
}

fn sort_deterministically(interfaces: &mut [NetworkInfo]) {
    interfaces.sort_by(|a, b| a.id.cmp(&b.id));
}

fn bytes_per_second(bytes: u64, interval: Duration) -> f64 {
    let seconds = interval.as_secs_f64();

    if seconds > 0.0 {
        bytes as f64 / 1_000_000.0 / seconds
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{NetworkInfo, bytes_per_second, sort_deterministically};

    #[test]
    fn calculates_decimal_megabytes_over_the_real_interval() {
        let rate = bytes_per_second(3_000_000, Duration::from_millis(1500));

        assert_eq!(rate, 2.0);
    }

    #[test]
    fn returns_zero_when_there_is_no_previous_sample() {
        assert_eq!(bytes_per_second(1_000_000, Duration::ZERO), 0.0);
    }

    #[test]
    fn preserves_all_interfaces_in_stable_name_order() {
        let mut interfaces: Vec<NetworkInfo> = (0..7)
            .map(|index| NetworkInfo {
                id: format!("net{index}"),
                interface: format!("net{index}"),
                received_per_sec: index as f64,
                transmitted_per_sec: index as f64,
                sample_interval: Some(Duration::from_secs(1)),
            })
            .collect();

        sort_deterministically(&mut interfaces);

        assert_eq!(interfaces.len(), 7);
        assert_eq!(interfaces[0].interface, "net0");
        assert_eq!(interfaces[6].interface, "net6");
    }

    #[test]
    fn legacy_view_is_limited_but_inventory_view_is_complete() {
        let mut interfaces: Vec<NetworkInfo> = (0..7)
            .map(|index| NetworkInfo {
                id: format!("net{index}"),
                interface: format!("net{index}"),
                received_per_sec: index as f64,
                transmitted_per_sec: index as f64,
                sample_interval: Some(Duration::from_secs(1)),
            })
            .collect();
        interfaces.sort_by(|left, right| left.id.cmp(&right.id));
        interfaces.truncate(5);

        assert_eq!(interfaces.len(), 5);
        assert_eq!(interfaces[0].id, "net0");
        assert_eq!(interfaces[4].id, "net4");
    }

    #[test]
    fn startup_resets_wraps_and_zero_time_are_unavailable_instead_of_fake_rates() {
        let interval = Duration::from_millis(1500);
        assert_eq!(super::valid_interval(None, (100, 200), interval), None);
        assert_eq!(
            super::valid_interval(Some((5000, 100)), (6500, 200), interval),
            Some(interval)
        );
        assert_eq!(
            super::valid_interval(Some((6500, 200)), (10, 300), interval),
            None
        );
        assert_eq!(
            super::valid_interval(Some((u64::MAX, 200)), (2, 300), interval),
            None
        );
        assert_eq!(
            super::valid_interval(Some((0, 0)), (0, 0), Duration::ZERO),
            None
        );
    }
}
