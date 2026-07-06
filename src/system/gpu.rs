use std::process::Command;

pub struct GpuInfo {
    pub name: String,
    pub temperature: String,
    pub usage: String,
    pub memory: String,
}

impl GpuInfo {
    pub fn get() -> Option<Self> {
        let output = Command::new("nvidia-smi")
            .args([
                "--query-gpu=name,temperature.gpu,utilization.gpu,memory.used",
                "--format=csv,noheader",
            ])
            .output()
            .ok()?;

        if !output.status.success() {
            return None;
        }

        let result = String::from_utf8_lossy(&output.stdout);

        let parts: Vec<&str> = result.lines().next()?.trim().split(',').collect();

        if parts.len() < 4 {
            return None;
        }

        Some(Self {
            name: parts[0].trim().to_string(),

            temperature: format!("{}°C", parts[1].trim()),

            usage: parts[2].trim().to_string(),

            memory: parts[3].trim().to_string(),
        })
    }
}
