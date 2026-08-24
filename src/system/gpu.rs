use std::{
    collections::BTreeMap,
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

const NVIDIA_SMI_TIMEOUT: Duration = Duration::from_millis(500);
const NVIDIA_SMI_POLL_INTERVAL: Duration = Duration::from_millis(5);

#[derive(Clone)]
pub struct GpuInfo {
    pub id: String,
    pub name: String,
    pub temperature: String,
    pub usage: String,
    pub memory: String,
    #[allow(dead_code)]
    pub usage_percent: Option<f64>,
    #[allow(dead_code)]
    pub vram_used_bytes: Option<f64>,
    #[allow(dead_code)]
    pub vram_total_bytes: Option<f64>,
    #[allow(dead_code)]
    pub temperature_celsius: Option<f64>,
    #[allow(dead_code)]
    pub frequency_hz: Option<f64>,
    #[allow(dead_code)]
    pub power_watts: Option<f64>,
}

pub struct GpuCollector {
    cached: Vec<GpuInfo>,
    last_refresh: Option<Instant>,
    refresh_interval: Duration,
}

impl GpuCollector {
    pub fn new(refresh_interval: Duration) -> Self {
        Self {
            cached: Vec::new(),
            last_refresh: None,
            refresh_interval,
        }
    }

    pub fn get(&mut self) -> &[GpuInfo] {
        let should_refresh = self
            .last_refresh
            .is_none_or(|last_refresh| last_refresh.elapsed() >= self.refresh_interval);

        if should_refresh {
            if let Some(gpus) = GpuInfo::get_all() {
                self.cached = gpus;
            }
            self.last_refresh = Some(Instant::now());
        }

        &self.cached
    }
}

impl GpuInfo {
    pub fn get_all() -> Option<Vec<Self>> {
        let nvidia_gpus = query_nvidia_smi();
        let drm_gpus = drm_gpu_infos();

        if nvidia_gpus.is_none() && drm_gpus.is_empty() {
            return None;
        }

        let nvidia_gpus = nvidia_gpus.unwrap_or_default();
        let has_nvidia_metrics = nvidia_gpus
            .iter()
            .any(|gpu| gpu.name.to_ascii_lowercase().contains("nvidia"));
        let mut gpus = nvidia_gpus;
        gpus.extend(
            drm_gpus
                .into_iter()
                .filter(|gpu| !has_nvidia_metrics || !gpu.name.starts_with("NVIDIA")),
        );
        assign_stable_gpu_ids(&mut gpus);
        Some(gpus)
    }

    fn parse_output(output: &str) -> Vec<Self> {
        let mut gpus = output
            .lines()
            .filter_map(Self::parse_line)
            .collect::<Vec<_>>();
        assign_stable_gpu_ids(&mut gpus);
        gpus
    }

    fn parse_line(line: &str) -> Option<Self> {
        // Split from the right so commas in GPU names do not shift metrics.
        let fields = line.rsplitn(7, ',').map(str::trim).collect::<Vec<_>>();
        if fields.len() >= 7 {
            let power_watts = parse_optional(fields[0]);
            let frequency_hz = parse_optional(fields[1]).map(|value| value * 1.0e6);
            let vram_total_bytes = parse_optional(fields[2]).map(mebibytes_to_bytes);
            let vram_used_bytes = parse_optional(fields[3]).map(mebibytes_to_bytes);
            let usage_percent = fields[4].parse::<f64>().ok()?;
            let temperature_celsius = parse_optional(fields[5]);
            let name = fields[6];

            if name.is_empty() || !(0.0..=100.0).contains(&usage_percent) {
                return None;
            }

            return Some(Self {
                id: String::new(),
                name: name.to_string(),
                temperature: temperature_celsius
                    .map_or_else(|| "—".to_owned(), |value| format!("{value:.0}°C")),
                usage: format!("{usage_percent:.0}%"),
                memory: vram_used_bytes.map_or_else(
                    || "—".to_owned(),
                    |value| format!("{:.0} MiB", value / 1024.0 / 1024.0),
                ),
                usage_percent: Some(usage_percent),
                vram_used_bytes,
                vram_total_bytes,
                temperature_celsius,
                frequency_hz,
                power_watts,
            });
        }

        let mut parts = line.rsplitn(4, ',').map(str::trim);
        let memory = parts.next()?.parse::<u64>().ok()?;
        let usage = parts.next()?.parse::<f64>().ok()?;
        let temperature = parts.next()?.parse::<f64>().ok()?;
        let name = parts.next()?;

        if name.is_empty() || !(0.0..=100.0).contains(&usage) {
            return None;
        }

        Some(Self {
            id: String::new(),
            name: name.to_string(),
            temperature: format!("{temperature:.0}°C"),
            usage: format!("{usage:.0}%"),
            memory: format!("{memory} MiB"),
            usage_percent: Some(usage),
            vram_used_bytes: Some(memory as f64 * 1024.0 * 1024.0),
            vram_total_bytes: None,
            temperature_celsius: Some(temperature),
            frequency_hz: None,
            power_watts: None,
        })
    }
}

fn query_nvidia_smi() -> Option<Vec<GpuInfo>> {
    let mut child = Command::new("nvidia-smi")
        .args([
            "--query-gpu=name,temperature.gpu,utilization.gpu,memory.used,memory.total,clocks.current.graphics,power.draw",
            "--format=csv,noheader,nounits",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;

    let deadline = Instant::now() + NVIDIA_SMI_TIMEOUT;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if !status.success() {
                    return None;
                }

                let output = child.wait_with_output().ok()?;
                return Some(GpuInfo::parse_output(&String::from_utf8_lossy(
                    &output.stdout,
                )));
            }
            Ok(None) if Instant::now() < deadline => {
                thread::sleep(NVIDIA_SMI_POLL_INTERVAL);
            }
            Ok(None) | Err(_) => {
                stop_child(&mut child);
                return None;
            }
        }
    }
}

fn stop_child(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

fn drm_gpu_infos() -> Vec<GpuInfo> {
    #[cfg(target_os = "linux")]
    {
        let mut entries = std::fs::read_dir("/sys/class/drm")
            .ok()
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
            .filter(|entry| {
                let name = entry.file_name();
                let name = name.to_string_lossy();
                name.starts_with("card") && !name.contains('-')
            })
            .collect::<Vec<_>>();
        entries.sort_by_key(|entry| entry.file_name());

        entries
            .into_iter()
            .filter_map(|entry| {
                let uevent = std::fs::read_to_string(entry.path().join("device/uevent")).ok()?;
                parse_drm_uevent(&uevent)
            })
            .collect()
    }
    #[cfg(not(target_os = "linux"))]
    {
        Vec::new()
    }
}

fn parse_drm_uevent(uevent: &str) -> Option<GpuInfo> {
    let driver = uevent
        .lines()
        .find_map(|line| line.strip_prefix("DRIVER="))?
        .trim();
    if driver.is_empty() {
        return None;
    }
    let pci_id = uevent
        .lines()
        .find_map(|line| line.strip_prefix("PCI_ID="))
        .map(str::trim)
        .unwrap_or_default()
        .to_ascii_lowercase();
    let name = match (driver.to_ascii_lowercase().as_str(), pci_id.as_str()) {
        ("amdgpu", "1002:1636") => "AMD Radeon · AMDGPU",
        ("nvidia", "10de:2882") => "NVIDIA GeForce RTX 4060",
        ("amdgpu", _) => "AMD GPU · AMDGPU",
        ("nvidia", _) => "NVIDIA GPU · NVIDIA",
        ("i915", _) => "Intel GPU · i915",
        other => {
            return Some(fallback_gpu_info(&format!(
                "{} GPU",
                other.0.to_ascii_uppercase()
            )));
        }
    };
    Some(fallback_gpu_info(name))
}

fn fallback_gpu_info(name: &str) -> GpuInfo {
    GpuInfo {
        id: String::new(),
        name: name.to_owned(),
        temperature: "—".to_owned(),
        usage: "—".to_owned(),
        memory: "—".to_owned(),
        usage_percent: None,
        vram_used_bytes: None,
        vram_total_bytes: None,
        temperature_celsius: None,
        frequency_hz: None,
        power_watts: None,
    }
}

fn assign_stable_gpu_ids(gpus: &mut [GpuInfo]) {
    let mut occurrences = BTreeMap::<String, usize>::new();
    for gpu in gpus {
        let base = stable_gpu_id(&gpu.name);
        let occurrence = occurrences.entry(base.clone()).or_insert(0);
        gpu.id = if *occurrence == 0 {
            base
        } else {
            format!("{base}-{}", *occurrence + 1)
        };
        *occurrence += 1;
    }
}

fn stable_gpu_id(name: &str) -> String {
    let id = name
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>();
    if id.is_empty() { "gpu".to_owned() } else { id }
}

fn parse_optional(value: &str) -> Option<f64> {
    value
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite() && *value >= 0.0)
}

fn mebibytes_to_bytes(value: f64) -> f64 {
    value * 1024.0 * 1024.0
}

#[cfg(test)]
mod tests {
    use super::{GpuInfo, parse_drm_uevent};

    #[test]
    fn parses_all_valid_gpus() {
        let gpus = GpuInfo::parse_output(
            "NVIDIA GeForce RTX 4080, 52, 15, 1024\nNVIDIA Tesla T4, 40, 0, 512",
        );

        assert_eq!(gpus.len(), 2);
        assert_eq!(gpus[0].name, "NVIDIA GeForce RTX 4080");
        assert_eq!(gpus[0].id, "nvidia-geforce-rtx-4080");
        assert_eq!(gpus[0].temperature, "52°C");
        assert_eq!(gpus[0].usage, "15%");
        assert_eq!(gpus[0].memory, "1024 MiB");
        assert_eq!(gpus[1].name, "NVIDIA Tesla T4");
        assert_eq!(gpus[1].id, "nvidia-tesla-t4");
    }

    #[test]
    fn skips_invalid_lines_without_discarding_valid_gpus() {
        let gpus = GpuInfo::parse_output(
            "invalid line\nNVIDIA Tesla T4, N/A, 10, 512\nNVIDIA A100, 45, 25, 2048",
        );

        assert_eq!(gpus.len(), 1);
        assert_eq!(gpus[0].name, "NVIDIA A100");
    }

    #[test]
    fn preserves_commas_in_gpu_name() {
        let gpus = GpuInfo::parse_output("NVIDIA, Experimental GPU, 45, 25, 2048");

        assert_eq!(gpus.len(), 1);
        assert_eq!(gpus[0].name, "NVIDIA, Experimental GPU");
    }

    #[test]
    fn rejects_each_invalid_metric_and_empty_names() {
        for line in [
            ", 45, 25, 2048",
            "GPU, invalid, 25, 2048",
            "GPU, 45, invalid, 2048",
            "GPU, 45, 101, 2048",
            "GPU, 45, 25, invalid",
        ] {
            assert!(GpuInfo::parse_output(line).is_empty(), "accepted {line}");
        }
    }

    #[test]
    fn duplicate_gpu_names_receive_stable_ordinal_ids() {
        let gpus = GpuInfo::parse_output("GPU, 45, 25, 2048\nGPU, 46, 26, 2048");

        assert_eq!(
            gpus.iter().map(|gpu| gpu.id.as_str()).collect::<Vec<_>>(),
            ["gpu", "gpu-2"]
        );
    }

    #[test]
    fn drm_fallback_keeps_identity_when_metrics_are_unavailable() {
        let gpu = parse_drm_uevent("DRIVER=amdgpu\nPCI_ID=1002:1636\n").unwrap();

        assert_eq!(gpu.name, "AMD Radeon · AMDGPU");
        assert!(gpu.usage_percent.is_none());
        assert!(gpu.temperature_celsius.is_none());
    }

    #[test]
    fn drm_fallback_uses_known_pci_identity_without_inventing_telemetry() {
        let gpu = parse_drm_uevent("DRIVER=nvidia\nPCI_ID=10DE:2882\n").unwrap();

        assert_eq!(gpu.name, "NVIDIA GeForce RTX 4060");
        assert!(gpu.usage_percent.is_none());
        assert!(gpu.vram_total_bytes.is_none());
    }
}
