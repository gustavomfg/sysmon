use std::{
    fs, io,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use crate::system::info::SystemInfo;

pub fn export_snapshot(snapshot: &SystemInfo) -> io::Result<PathBuf> {
    let path = default_snapshot_path();
    export_snapshot_to(snapshot, &path, SystemTime::now())?;
    Ok(path)
}

fn default_snapshot_path() -> PathBuf {
    let base = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/state")))
        .unwrap_or_else(|| PathBuf::from("."));

    base.join("sysmon").join("snapshot.json")
}

fn export_snapshot_to(snapshot: &SystemInfo, path: &Path, timestamp: SystemTime) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    fs::write(path, snapshot_json(snapshot, timestamp))
}

fn snapshot_json(snapshot: &SystemInfo, timestamp: SystemTime) -> String {
    let timestamp = timestamp
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs());
    let disks = snapshot
        .disks
        .iter()
        .map(|disk| {
            format!(
                "{{\"name\":{},\"mount_point\":{},\"total_gb\":{},\"available_gb\":{}}}",
                json_string(&disk.name),
                json_string(&disk.mount_point),
                json_number(disk.total_space),
                json_number(disk.available_space)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let networks = snapshot
        .networks
        .iter()
        .map(|network| {
            format!(
                "{{\"interface\":{},\"received_mb_per_sec\":{},\"transmitted_mb_per_sec\":{}}}",
                json_string(&network.interface),
                json_number(network.received_per_sec),
                json_number(network.transmitted_per_sec)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let gpus = snapshot
        .gpus
        .iter()
        .map(|gpu| {
            format!(
                "{{\"name\":{},\"temperature\":{},\"usage\":{},\"memory\":{}}}",
                json_string(&gpu.name),
                json_string(&gpu.temperature),
                json_string(&gpu.usage),
                json_string(&gpu.memory)
            )
        })
        .collect::<Vec<_>>()
        .join(",");

    format!(
        "{{\n  \"timestamp\": {timestamp},\n  \"cpu\": {{\"usage_percent\": {}}},\n  \"memory\": {{\"used_gb\": {}, \"total_gb\": {}, \"used_swap_gb\": {}, \"total_swap_gb\": {}}},\n  \"disks\": [{disks}],\n  \"network\": [{networks}],\n  \"gpu\": [{gpus}]\n}}\n",
        json_number(f64::from(snapshot.cpu_usage)),
        json_number(snapshot.used_ram),
        json_number(snapshot.total_ram),
        json_number(snapshot.used_swap),
        json_number(snapshot.total_swap),
    )
}

fn json_number(value: f64) -> String {
    if value.is_finite() {
        format!("{value:.3}")
    } else {
        "null".to_string()
    }
}

fn json_string(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len() + 2);
    escaped.push('"');
    for character in value.chars() {
        match character {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            character if character.is_control() => {
                escaped.push_str(&format!("\\u{:04x}", character as u32));
            }
            character => escaped.push(character),
        }
    }
    escaped.push('"');
    escaped
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        time::{Duration, UNIX_EPOCH},
    };

    use super::{export_snapshot_to, snapshot_json};
    use crate::system::info::{DiskInfo, ProcessInfo, SystemInfo};

    #[test]
    fn exports_the_current_metrics_as_json() {
        let snapshot = sample_snapshot();
        let json = snapshot_json(&snapshot, UNIX_EPOCH + Duration::from_secs(42));

        assert!(json.contains("\"timestamp\": 42"));
        assert!(json.contains("\"usage_percent\": 37.500"));
        assert!(json.contains("\"memory\""));
        assert!(json.contains("\"disks\""));
        assert!(json.contains("\"network\""));
        assert!(json.contains("\"gpu\": []"));
    }

    #[test]
    fn writes_snapshot_to_the_requested_path() {
        let directory =
            std::env::temp_dir().join(format!("sysmon-export-test-{}", std::process::id()));
        let path = directory.join("snapshot.json");
        export_snapshot_to(&sample_snapshot(), &path, UNIX_EPOCH).expect("snapshot should write");

        let contents = fs::read_to_string(&path).expect("snapshot should be readable");
        assert!(contents.starts_with('{'));
        assert!(contents.contains("\"timestamp\": 0"));

        fs::remove_file(path).expect("test snapshot should be removable");
        fs::remove_dir(directory).expect("test directory should be removable");
    }

    fn sample_snapshot() -> SystemInfo {
        SystemInfo {
            os_name: "Test OS".to_string(),
            kernel: "test".to_string(),
            cpu_name: "Test CPU".to_string(),
            cpu_cores: 4,
            cpu_usage: 37.5,
            cpu_frequency_hz: None,
            cpu_temperature_celsius: None,
            total_ram: 8.0,
            used_ram: 2.0,
            total_swap: 2.0,
            used_swap: 0.5,
            processes: 1,
            uptime: 10,
            load_average: (0.1, 0.2, 0.3),
            disks: vec![DiskInfo {
                name: "disk".to_string(),
                mount_point: "/".to_string(),
                total_space: 100.0,
                available_space: 50.0,
                read_bytes_per_sec: None,
                written_bytes_per_sec: None,
            }],
            processes_list: vec![ProcessInfo {
                name: "test".to_string(),
                memory: 1.0,
                cpu_usage: 2.0,
            }],
            gpus: Vec::new(),
            networks: Vec::new(),
        }
    }
}
