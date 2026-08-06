use std::{
    process::Command,
    time::{Duration, Instant},
};

#[derive(Clone)]
pub struct GpuInfo {
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
            self.cached = GpuInfo::get_all();
            self.last_refresh = Some(Instant::now());
        }

        &self.cached
    }
}

impl GpuInfo {
    pub fn get_all() -> Vec<Self> {
        let Ok(output) = Command::new("nvidia-smi")
            .args([
                "--query-gpu=name,temperature.gpu,utilization.gpu,memory.used,memory.total,clocks.current.graphics,power.draw",
                "--format=csv,noheader,nounits",
            ])
            .output()
        else {
            return Vec::new();
        };

        if !output.status.success() {
            return Vec::new();
        }

        Self::parse_output(&String::from_utf8_lossy(&output.stdout))
    }

    fn parse_output(output: &str) -> Vec<Self> {
        output.lines().filter_map(Self::parse_line).collect()
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
    use super::GpuInfo;

    #[test]
    fn parses_all_valid_gpus() {
        let gpus = GpuInfo::parse_output(
            "NVIDIA GeForce RTX 4080, 52, 15, 1024\nNVIDIA Tesla T4, 40, 0, 512",
        );

        assert_eq!(gpus.len(), 2);
        assert_eq!(gpus[0].name, "NVIDIA GeForce RTX 4080");
        assert_eq!(gpus[0].temperature, "52°C");
        assert_eq!(gpus[0].usage, "15%");
        assert_eq!(gpus[0].memory, "1024 MiB");
        assert_eq!(gpus[1].name, "NVIDIA Tesla T4");
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
}
