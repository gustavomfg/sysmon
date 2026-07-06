use sysinfo::Networks;

pub struct NetworkInfo {
    pub interface: String,
    pub received_per_sec: f64,
    pub transmitted_per_sec: f64,
}

impl NetworkInfo {
    pub fn get(networks: &Networks) -> Vec<Self> {
        networks
            .iter()
            .map(|(name, data)| Self {
                interface: name.to_string(),
                received_per_sec: data.received() as f64 / 1024.0 / 1024.0,
                transmitted_per_sec: data.transmitted() as f64 / 1024.0 / 1024.0,
            })
            .collect()
    }
}
