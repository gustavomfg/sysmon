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

#[cfg(test)]
mod tests {
    use super::{AppState, CPU_HISTORY_LIMIT};

    #[test]
    fn keeps_only_the_latest_sixty_cpu_samples() {
        let mut app = AppState::new();

        for sample in 0..=CPU_HISTORY_LIMIT {
            app.update_cpu_history(sample as f32);
        }

        assert_eq!(app.cpu_history.len(), CPU_HISTORY_LIMIT);
        assert_eq!(app.cpu_history.first(), Some(&1));
        assert_eq!(app.cpu_history.last(), Some(&(CPU_HISTORY_LIMIT as u64)));
    }

    #[test]
    fn clamps_and_rounds_cpu_samples() {
        let mut app = AppState::new();

        for sample in [-10.0, 42.6, 150.0] {
            app.update_cpu_history(sample);
        }

        assert_eq!(app.cpu_history, vec![0, 43, 100]);
    }
}
