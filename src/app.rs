pub struct AppState {
    pub cpu_history: Vec<u64>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            cpu_history: Vec::new(),
        }
    }

    pub fn update_cpu_history(&mut self, cpu_usage: f32) {
        self.cpu_history.push(cpu_usage as u64);

        if self.cpu_history.len() > 60 {
            self.cpu_history.remove(0);
        }
    }
}
