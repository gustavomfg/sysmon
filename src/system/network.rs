use std::time::Duration;

use sysinfo::Networks;

pub struct NetworkInfo {
    pub interface: String,
    pub received_per_sec: f64,
    pub transmitted_per_sec: f64,
}

impl NetworkInfo {
    pub fn get(networks: &Networks, interval: Duration) -> Vec<Self> {
        let mut interfaces = networks
            .iter()
            .map(|(name, data)| Self {
                interface: name.to_string(),
                received_per_sec: bytes_per_second(data.received(), interval),
                transmitted_per_sec: bytes_per_second(data.transmitted(), interval),
            })
            .collect::<Vec<Self>>();

        sort_and_limit(&mut interfaces);
        interfaces
    }
}

fn sort_and_limit(interfaces: &mut Vec<NetworkInfo>) {
    interfaces.sort_by(|a, b| {
        let a_total = a.received_per_sec + a.transmitted_per_sec;
        let b_total = b.received_per_sec + b.transmitted_per_sec;

        b_total
            .partial_cmp(&a_total)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    interfaces.truncate(5);
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

    use super::{NetworkInfo, bytes_per_second, sort_and_limit};

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
    fn orders_interfaces_by_total_traffic_and_limits_the_result() {
        let mut interfaces = (0..7)
            .map(|index| NetworkInfo {
                interface: format!("net{index}"),
                received_per_sec: index as f64,
                transmitted_per_sec: index as f64,
            })
            .collect();

        sort_and_limit(&mut interfaces);

        assert_eq!(interfaces.len(), 5);
        assert_eq!(interfaces[0].interface, "net6");
        assert_eq!(interfaces[4].interface, "net2");
    }
}
