mod system;

use std::{
    thread,
    time::Duration,
};

use sysinfo::System;

use system::info::SystemInfo;


fn main() {
    let mut sys = System::new_all();

    loop {
        let info = SystemInfo::get(&mut sys);

        println!("============================");
        println!("       SYSTEM MONITOR");
        println!("============================");

        println!("Sistema: {}", info.os_name);
        println!("Kernel: {}", info.kernel);
        println!("CPU: {}", info.cpu_name);

        println!(
            "CPU cores: {}",
            info.cpu_cores
        );

        println!(
            "CPU uso: {:.2}%",
            info.cpu_usage
        );

        println!(
            "RAM: {:.2} GB / {:.2} GB",
            info.used_ram,
            info.total_ram
        );

        println!("Uptime: {}h {}min", info.uptime / 3600, (info.uptime % 3600) / 60);

        println!(
            "Processos: {}",
            info.processes
        );

        println!("Discos:");

for disk in info.disks {
    println!(
        "- {} | {} | livre: {:.2} GB / {:.2} GB",
        disk.name,
        disk.mount_point,
        disk.available_space,
        disk.total_space
    );
}

        thread::sleep(
            Duration::from_secs(2)
        );

        print!("\x1B[2J\x1B[1;1H");
    }
}