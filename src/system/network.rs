use sysinfo::Networks;


pub struct NetworkInfo {
    pub interface: String,
    pub received: f64,
    pub transmitted: f64,
}


impl NetworkInfo {

    pub fn get() -> Vec<Self> {

        let networks =
            Networks::new_with_refreshed_list();


        networks
            .iter()
            .map(|(name, data)| {

                Self {

                    interface:
                        name.to_string(),

                    received:
                        data.total_received()
                            as f64
                            /1024.0
                            /1024.0,

                    transmitted:
                        data.total_transmitted()
                            as f64
                            /1024.0
                            /1024.0,
                }

            })
            .collect()
    }
}