use sysinfo::System;

pub struct SystemInfo {
    pub os_name: String,
    pub kernel: String,
    pub cpu_cores: usize,
    pub cpu_usage: f32,
    pub total_ram: f64,
    pub used_ram: f64,
    pub processes: usize,
}

impl SystemInfo {
    pub fn get(system: &mut System) -> Self {
        system.refresh_all();

        Self {
            os_name: System::name()
                .unwrap_or("Desconhecido".to_string()),

            kernel: System::kernel_version()
                .unwrap_or("Desconhecido".to_string()),

            cpu_cores: system.cpus().len(),

            cpu_usage: system.global_cpu_usage(),

            total_ram: system.total_memory() as f64 / 1024.0 / 1024.0,

            used_ram: system.used_memory() as f64 / 1024.0 / 1024.0,

            processes: system.processes().len(),
        }
    }
}