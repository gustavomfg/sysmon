use std::{thread, time::Duration};
use sysinfo::System;

fn main() {
    let mut system = System::new_all();

    loop {
        system.refresh_all();

        let total_ram = system.total_memory() as f64 / 1024.0 / 1024.0;
        let used_ram = system.used_memory() as f64 / 1024.0 / 1024.0;

        println!("============================");
        println!("      SYSTEM MONITOR");
        println!("============================");

        println!("Sistema: {}", System::name().unwrap_or("Desconhecido".into()));
        println!("Kernel: {}", System::kernel_version().unwrap_or("Desconhecido".into()));

        println!("CPU cores: {}", system.cpus().len());
        println!("CPU uso: {:.2}%", system.global_cpu_usage());

        println!("RAM: {:.2} GB / {:.2} GB", used_ram, total_ram);

        println!("Processos: {}", system.processes().len());

        thread::sleep(Duration::from_secs(2));

        print!("\x1B[2J\x1B[1;1H");
    }
}