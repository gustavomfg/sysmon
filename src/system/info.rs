use sysinfo::{Disks, System};

pub struct DiskInfo {
    pub name: String,
    pub mount_point: String,
    pub total_space: f64,
    pub available_space: f64,
}

pub struct SystemInfo {
    pub os_name: String,
    pub kernel: String,
    pub cpu_cores: usize,
    pub cpu_usage: f32,
    pub total_ram: f64,
    pub used_ram: f64,
    pub processes: usize,
    pub disks: Vec<DiskInfo>,
    pub cpu_name: String,
    pub uptime: u64

}

impl SystemInfo {
    pub fn get(system: &mut System) -> Self {
        system.refresh_all();

        let disks = Disks::new_with_refreshed_list();

        let disk_infos = disks
            .iter()
            .map(|disk| DiskInfo {
                name: disk.name().to_string_lossy().to_string(),
                mount_point: disk.mount_point().to_string_lossy().to_string(),
                total_space: disk.total_space() as f64 / 1024.0 / 1024.0 / 1024.0,
                available_space: disk.available_space() as f64 / 1024.0 / 1024.0 / 1024.0,
            })
            .collect();

        Self {
            os_name: System::name().unwrap_or("Desconhecido".to_string()),
            kernel: System::kernel_version().unwrap_or("Desconhecido".to_string()),
            cpu_cores: system.cpus().len(),
            cpu_usage: system.global_cpu_usage(),
            total_ram: system.total_memory() as f64 / 1024.0 / 1024.0,
            used_ram: system.used_memory() as f64 / 1024.0 / 1024.0,
            processes: system.processes().len(),
            disks: disk_infos,
            cpu_name: system
            .cpus()
            .first()
            .map(|cpu| cpu.brand().to_string())
            .unwrap_or("Desconhecido".to_string()),
            uptime: System::uptime()
            
        }
    }
}