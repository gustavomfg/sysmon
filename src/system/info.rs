use std::cmp::Ordering;

use crate::system::gpu::GpuInfo;
use crate::system::network::NetworkInfo;
use sysinfo::{Disks, Networks, System};

pub struct DiskInfo {
    pub name: String,
    pub mount_point: String,
    pub total_space: f64,
    pub available_space: f64,
}

pub struct ProcessInfo {
    pub name: String,
    pub memory: f64,
}

pub struct SystemInfo {
    pub os_name: String,
    pub kernel: String,
    pub cpu_name: String,
    pub cpu_cores: usize,
    pub cpu_usage: f32,
    pub total_ram: f64,
    pub used_ram: f64,
    pub processes: usize,
    pub uptime: u64,
    pub disks: Vec<DiskInfo>,
    pub processes_list: Vec<ProcessInfo>,
    pub gpu: Option<GpuInfo>,
    pub networks: Vec<NetworkInfo>,
}

impl SystemInfo {
    pub fn get(system: &mut System, networks: &mut Networks) -> Self {
        system.refresh_all();
        networks.refresh(true);

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

        let mut processes_list = system
            .processes()
            .values()
            .map(|process| ProcessInfo {
                name: process.name().to_string_lossy().to_string(),
                memory: process.memory() as f64 / 1024.0 / 1024.0,
            })
            .collect::<Vec<ProcessInfo>>();

        processes_list.sort_by(|a, b| b.memory.partial_cmp(&a.memory).unwrap_or(Ordering::Equal));

        processes_list.truncate(5);

        Self {
            os_name: System::name().unwrap_or("Desconhecido".to_string()),
            kernel: System::kernel_version().unwrap_or("Desconhecido".to_string()),
            cpu_name: system
                .cpus()
                .first()
                .map(|cpu| cpu.brand().to_string())
                .unwrap_or("Desconhecido".to_string()),
            cpu_cores: system.cpus().len(),
            cpu_usage: system.global_cpu_usage(),
            total_ram: system.total_memory() as f64 / 1024.0 / 1024.0 / 1024.0,
            used_ram: system.used_memory() as f64 / 1024.0 / 1024.0 / 1024.0,
            processes: system.processes().len(),
            uptime: System::uptime(),
            disks: disk_infos,
            processes_list,
            gpu: GpuInfo::get(),
            networks: NetworkInfo::get(networks),
        }
    }
}
