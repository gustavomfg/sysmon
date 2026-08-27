#![allow(dead_code)]

mod series;

use series::Series;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Metric {
    Cpu,
    Memory,
    NetworkDownload,
    NetworkUpload,
    Gpu,
}

pub struct History {
    cpu: Series,
    memory: Series,
    network_download: Series,
    network_upload: Series,
    gpu: Series,
}

impl History {
    pub fn new(limit: usize) -> Self {
        Self {
            cpu: Series::new(limit),
            memory: Series::new(limit),
            network_download: Series::new(limit),
            network_upload: Series::new(limit),
            gpu: Series::new(limit),
        }
    }

    pub fn record(&mut self, metric: Metric, value: Option<f64>) {
        let Some(value) = value else {
            return;
        };

        self.series_mut(metric).push(value);
    }

    pub fn samples(&self, metric: Metric) -> &[f64] {
        self.series(metric).as_slice()
    }

    fn series(&self, metric: Metric) -> &Series {
        match metric {
            Metric::Cpu => &self.cpu,
            Metric::Memory => &self.memory,
            Metric::NetworkDownload => &self.network_download,
            Metric::NetworkUpload => &self.network_upload,
            Metric::Gpu => &self.gpu,
        }
    }

    fn series_mut(&mut self, metric: Metric) -> &mut Series {
        match metric {
            Metric::Cpu => &mut self.cpu,
            Metric::Memory => &mut self.memory,
            Metric::NetworkDownload => &mut self.network_download,
            Metric::NetworkUpload => &mut self.network_upload,
            Metric::Gpu => &mut self.gpu,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{History, Metric};

    #[test]
    fn keeps_a_stable_order_and_respects_the_limit() {
        let mut history = History::new(3);
        for value in [1.0, 2.0, 3.0, 4.0] {
            history.record(Metric::Cpu, Some(value));
        }

        assert_eq!(history.samples(Metric::Cpu), &[2.0, 3.0, 4.0]);
    }

    #[test]
    fn updates_multiple_metrics_independently() {
        let mut history = History::new(3);
        history.record(Metric::Cpu, Some(40.0));
        history.record(Metric::Memory, Some(60.0));
        history.record(Metric::NetworkDownload, Some(2.5));
        history.record(Metric::NetworkUpload, Some(1.5));

        assert_eq!(history.samples(Metric::Cpu), &[40.0]);
        assert_eq!(history.samples(Metric::Memory), &[60.0]);
        assert_eq!(history.samples(Metric::NetworkDownload), &[2.5]);
        assert_eq!(history.samples(Metric::NetworkUpload), &[1.5]);
    }

    #[test]
    fn ignores_missing_gpu_samples() {
        let mut history = History::new(3);
        history.record(Metric::Gpu, None);

        assert!(history.samples(Metric::Gpu).is_empty());
    }

    #[test]
    fn zero_limit_never_grows() {
        let mut history = History::new(0);
        history.record(Metric::Cpu, Some(42.0));

        assert!(history.samples(Metric::Cpu).is_empty());
    }
}
