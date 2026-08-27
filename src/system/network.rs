use std::time::Duration;

use sysinfo::Networks;

#[derive(Debug, Clone)]
pub struct NetworkInfo {
    pub id: String,
    pub interface: String,
    pub received_per_sec: f64,
    pub transmitted_per_sec: f64,
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
            })
            .collect::<Vec<Self>>();

        sort_deterministically(&mut interfaces);
        interfaces
    }
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
            })
            .collect();
        interfaces.sort_by(|left, right| left.id.cmp(&right.id));
        interfaces.truncate(5);

        assert_eq!(interfaces.len(), 5);
        assert_eq!(interfaces[0].id, "net0");
        assert_eq!(interfaces[4].id, "net4");
    }
}
