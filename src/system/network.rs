use sysinfo::Networks;

pub struct NetworkInfo {
    pub interface: String,
    pub received_per_sec: f64,
    pub transmitted_per_sec: f64,
}

impl NetworkInfo {
    pub fn get(networks: &Networks) -> Vec<Self> {
        let mut interfaces = networks
            .iter()
            .map(|(name, data)| Self {
                interface: name.to_string(),
                received_per_sec: data.received() as f64 / 1024.0 / 1024.0,
                transmitted_per_sec: data.transmitted() as f64 / 1024.0 / 1024.0,
            })
            .collect::<Vec<Self>>();

        interfaces.sort_by(|a, b| {
            let a_total = a.received_per_sec + a.transmitted_per_sec;
            let b_total = b.received_per_sec + b.transmitted_per_sec;

            b_total
                .partial_cmp(&a_total)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        interfaces.truncate(5);
        interfaces
    }
}
