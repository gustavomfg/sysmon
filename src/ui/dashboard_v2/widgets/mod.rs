mod cpu;
mod gpu;
mod memory;
mod network;
mod storage;

use super::DashboardWidgetId;

pub use cpu::compose as compose_cpu;
pub use gpu::compose as compose_gpu;
pub use memory::compose as compose_memory;
pub use network::compose as compose_network;
pub use storage::compose as compose_storage;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrimaryWidget {
    Cpu,
    Memory,
    Gpu,
    Network,
    Storage,
}

impl PrimaryWidget {
    pub const ALL: [Self; 5] = [
        Self::Cpu,
        Self::Memory,
        Self::Gpu,
        Self::Network,
        Self::Storage,
    ];

    pub const fn id(self) -> DashboardWidgetId {
        match self {
            Self::Cpu => DashboardWidgetId::Cpu,
            Self::Memory => DashboardWidgetId::Memory,
            Self::Gpu => DashboardWidgetId::Gpu,
            Self::Network => DashboardWidgetId::Network,
            Self::Storage => DashboardWidgetId::Storage,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Cpu => "CPU",
            Self::Memory => "Memory",
            Self::Gpu => "GPU",
            Self::Network => "Network",
            Self::Storage => "Storage",
        }
    }

    pub const fn next(self) -> Self {
        match self {
            Self::Cpu => Self::Memory,
            Self::Memory => Self::Gpu,
            Self::Gpu => Self::Network,
            Self::Network => Self::Storage,
            Self::Storage => Self::Cpu,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primary_widget_ids_are_stable_and_complete() {
        assert_eq!(PrimaryWidget::ALL.len(), 5);
        assert_eq!(
            PrimaryWidget::ALL.map(PrimaryWidget::id),
            [
                DashboardWidgetId::Cpu,
                DashboardWidgetId::Memory,
                DashboardWidgetId::Gpu,
                DashboardWidgetId::Network,
                DashboardWidgetId::Storage,
            ]
        );
    }

    #[test]
    fn primary_widget_selection_cycles_deterministically() {
        let mut current = PrimaryWidget::Cpu;
        for expected in [
            PrimaryWidget::Memory,
            PrimaryWidget::Gpu,
            PrimaryWidget::Network,
            PrimaryWidget::Storage,
            PrimaryWidget::Cpu,
        ] {
            current = current.next();
            assert_eq!(current, expected);
        }
    }

    #[cfg(test)]
    mod compositions {
        use std::time::Duration;

        use super::super::super::{BlockId, DashboardPreset};
        use super::super::super::{
            CpuMetricInput, CpuMetricModel, DashboardState, DashboardWidgetId, GpuMetricInput,
            GpuMetricModel, MemoryMetricInput, MemoryMetricModel, MetricAvailability,
            MetricReading, MetricUnit, NetworkMetricInput, NetworkMetricModel, StorageMetricInput,
            StorageMetricModel, TimeSample, TimeSeries, WidgetAvailability, WidgetContext,
            WidgetRenderModel,
        };
        use super::super::{
            compose_cpu, compose_gpu, compose_memory, compose_network, compose_storage,
        };

        const HISTORY: usize = 8;

        fn history(unit: MetricUnit, values: [f64; HISTORY]) -> TimeSeries<HISTORY> {
            let mut series = TimeSeries::new(unit);
            for (index, value) in values.into_iter().enumerate() {
                series
                    .push(
                        TimeSample::new(
                            Duration::from_secs(index as u64),
                            Some(value),
                            MetricAvailability::Available,
                            None,
                        )
                        .unwrap(),
                    )
                    .unwrap();
            }
            series
        }

        fn context() -> WidgetContext {
            WidgetContext::new(
                DashboardPreset::LargeWidescreen,
                DashboardState::Calm,
                Duration::from_secs(3),
            )
            .with_availability(WidgetAvailability::all())
        }

        fn normalized(value: f64) -> MetricReading {
            MetricReading::normalized(Some(value), MetricAvailability::Available).unwrap()
        }

        fn non_negative(value: f64) -> MetricReading {
            MetricReading::non_negative(Some(value), MetricAvailability::Available).unwrap()
        }

        fn cpu() -> CpuMetricModel<HISTORY> {
            CpuMetricModel::from_input(
                CpuMetricInput::new(
                    normalized(0.42),
                    normalized(0.30),
                    normalized(0.12),
                    normalized(0.58),
                    non_negative(3.2),
                    non_negative(55.0),
                    8,
                    history(
                        MetricUnit::Percent,
                        [0.20, 0.24, 0.28, 0.32, 0.35, 0.38, 0.40, 0.42],
                    ),
                )
                .unwrap(),
            )
            .unwrap()
        }

        fn memory() -> MemoryMetricModel<HISTORY> {
            MemoryMetricModel::from_input(
                MemoryMetricInput::new(
                    non_negative(60.0),
                    non_negative(40.0),
                    non_negative(100.0),
                    non_negative(10.0),
                    non_negative(0.0),
                    normalized(0.60),
                    history(
                        MetricUnit::Percent,
                        [0.40, 0.43, 0.46, 0.49, 0.52, 0.55, 0.58, 0.60],
                    ),
                )
                .unwrap(),
            )
            .unwrap()
        }

        fn gpu() -> GpuMetricModel<HISTORY> {
            GpuMetricModel::from_input(
                GpuMetricInput::new(
                    normalized(0.52),
                    non_negative(42.0),
                    non_negative(100.0),
                    non_negative(64.0),
                    non_negative(1_800.0),
                    non_negative(120.0),
                    history(
                        MetricUnit::Percent,
                        [0.20, 0.24, 0.28, 0.32, 0.37, 0.42, 0.47, 0.52],
                    ),
                )
                .unwrap(),
            )
            .unwrap()
        }

        fn network() -> NetworkMetricModel<HISTORY> {
            NetworkMetricModel::from_input(
                NetworkMetricInput::new(
                    non_negative(80.0),
                    non_negative(20.0),
                    non_negative(800.0),
                    non_negative(200.0),
                    non_negative(100.0),
                    history(
                        MetricUnit::BytesPerSecond,
                        [20.0, 30.0, 40.0, 45.0, 55.0, 65.0, 72.0, 80.0],
                    ),
                    history(
                        MetricUnit::BytesPerSecond,
                        [10.0, 11.0, 12.0, 13.0, 14.0, 16.0, 18.0, 20.0],
                    ),
                )
                .unwrap(),
            )
            .unwrap()
        }

        fn storage() -> StorageMetricModel<HISTORY> {
            StorageMetricModel::from_input(
                StorageMetricInput::new(
                    non_negative(620.0),
                    non_negative(1_000.0),
                    non_negative(120.0),
                    non_negative(60.0),
                    non_negative(0.04),
                    normalized(0.48),
                    history(
                        MetricUnit::Percent,
                        [0.48, 0.49, 0.50, 0.51, 0.53, 0.56, 0.59, 0.62],
                    ),
                    history(
                        MetricUnit::BytesPerSecond,
                        [20.0, 30.0, 40.0, 55.0, 70.0, 85.0, 100.0, 120.0],
                    ),
                    history(
                        MetricUnit::BytesPerSecond,
                        [10.0, 15.0, 20.0, 25.0, 32.0, 40.0, 50.0, 60.0],
                    ),
                )
                .unwrap(),
            )
            .unwrap()
        }

        fn assert_base(
            model: Option<WidgetRenderModel>,
            id: DashboardWidgetId,
        ) -> WidgetRenderModel {
            let model = model.unwrap();
            assert_eq!(model.visual().widget_id(), id);
            assert!(model.block(BlockId::Title).is_some());
            assert!(model.block(BlockId::Value).is_some());
            assert!(model.block(BlockId::Trend).is_some());
            model
        }

        #[test]
        fn each_primary_widget_has_a_shared_base_and_specific_visual_context() {
            let context = context();
            let cpu = assert_base(compose_cpu(cpu(), context).unwrap(), DashboardWidgetId::Cpu);
            let memory = assert_base(
                compose_memory(memory(), context).unwrap(),
                DashboardWidgetId::Memory,
            );
            let gpu = assert_base(compose_gpu(gpu(), context).unwrap(), DashboardWidgetId::Gpu);
            let network = assert_base(
                compose_network(network(), context).unwrap(),
                DashboardWidgetId::Network,
            );
            let storage = assert_base(
                compose_storage(storage(), context).unwrap(),
                DashboardWidgetId::Storage,
            );

            assert!(cpu.block(BlockId::Segments).is_some());
            assert!(memory.block(BlockId::Area).is_some());
            assert!(memory.block(BlockId::Segments).is_some());
            assert!(gpu.block(BlockId::Segments).is_some());
            assert!(network.block(BlockId::SecondarySparkline).is_some());
            assert!(network.block(BlockId::Comparison).is_some());
            assert!(storage.block(BlockId::TertiarySparkline).is_some());
            assert!(storage.block(BlockId::Comparison).is_some());
        }

        #[test]
        fn primary_compositions_are_deterministic_and_degrade_to_essentials() {
            let context = context();
            assert_eq!(compose_cpu(cpu(), context), compose_cpu(cpu(), context));

            let compact = compose_memory(
                memory(),
                WidgetContext::new(
                    DashboardPreset::Compact,
                    DashboardState::Calm,
                    Duration::from_secs(3),
                )
                .with_availability(WidgetAvailability::all()),
            )
            .unwrap()
            .unwrap();
            assert_eq!(compact.block_count(), 6);
            assert!(compact.block(BlockId::Value).is_some());
            assert!(compact.block(BlockId::MemoryContext).is_some());
            assert!(compact.block(BlockId::Status).is_some());
            assert!(compact.block(BlockId::PrimarySparkline).is_some());
        }
    }
}
