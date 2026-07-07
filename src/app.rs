pub struct AppState {
    pub cpu_history: Vec<u64>,
}

const CPU_HISTORY_LIMIT: usize = 60;

impl AppState {
    pub fn new() -> Self {
        Self {
            cpu_history: Vec::with_capacity(CPU_HISTORY_LIMIT),
        }
    }

    pub fn update_cpu_history(&mut self, cpu_usage: f32) {
        if self.cpu_history.len() == CPU_HISTORY_LIMIT {
            self.cpu_history.remove(0);
        }

        self.cpu_history
            .push(cpu_usage.clamp(0.0, 100.0).round() as u64);
    }
}
