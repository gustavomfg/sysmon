use std::process::Command;

pub struct GpuInfo {
    pub name: String,
    pub temperature: String,
    pub usage: String,
    pub memory: String,
}

impl GpuInfo {
    pub fn get_all() -> Vec<Self> {
        let Ok(output) = Command::new("nvidia-smi")
            .args([
                "--query-gpu=name,temperature.gpu,utilization.gpu,memory.used",
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
        // Split from the right so a comma in the GPU name does not shift metrics.
        let mut parts = line.rsplitn(4, ',').map(str::trim);
        let memory = parts.next()?.parse::<u64>().ok()?;
        let usage = parts.next()?.parse::<f32>().ok()?;
        let temperature = parts.next()?.parse::<f32>().ok()?;
        let name = parts.next()?;

        if name.is_empty() || !(0.0..=100.0).contains(&usage) {
            return None;
        }

        Some(Self {
            name: name.to_string(),
            temperature: format!("{temperature:.0}°C"),
            usage: format!("{usage:.0}%"),
            memory: format!("{memory} MiB"),
        })
    }
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
}
