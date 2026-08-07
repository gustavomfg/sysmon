use ratatui::{buffer::Buffer, layout::Rect, style::Style};

use crate::theme::Theme;

use super::super::{DashboardBounds, DashboardPreset, HardwareInventory, WidgetRenderModel};
use super::{
    blocks, cpu::CpuCardRenderer, layout::TerminalLayout, layout::TerminalLayoutError, metrics,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalRenderError {
    BoundsOverflow,
    BufferTooSmall,
}

impl From<TerminalLayoutError> for TerminalRenderError {
    fn from(error: TerminalLayoutError) -> Self {
        match error {
            TerminalLayoutError::BoundsOverflow => Self::BoundsOverflow,
            TerminalLayoutError::BufferTooSmall => Self::BufferTooSmall,
        }
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct TerminalRenderer;

impl TerminalRenderer {
    pub const fn new() -> Self {
        Self
    }

    pub fn render(
        self,
        model: WidgetRenderModel,
        preset: DashboardPreset,
        bounds: DashboardBounds,
        theme: &Theme,
        buffer: &mut Buffer,
    ) -> Result<(), TerminalRenderError> {
        self.render_with_inventory(model, preset, bounds, theme, buffer, None)
    }

    pub(crate) fn render_with_inventory(
        self,
        model: WidgetRenderModel,
        preset: DashboardPreset,
        bounds: DashboardBounds,
        theme: &Theme,
        buffer: &mut Buffer,
        inventory: Option<&HardwareInventory>,
    ) -> Result<(), TerminalRenderError> {
        if model.visual().widget_id() == super::super::DashboardWidgetId::Cpu {
            return CpuCardRenderer::render(model, preset, bounds, theme, buffer);
        }
        if matches!(
            model.visual().widget_id(),
            super::super::DashboardWidgetId::Memory
                | super::super::DashboardWidgetId::Gpu
                | super::super::DashboardWidgetId::Network
                | super::super::DashboardWidgetId::Storage
        ) {
            return metrics::render_with_inventory(model, preset, bounds, theme, buffer, inventory);
        }
        let layout = TerminalLayout::new(model, preset, bounds, *buffer.area())?;
        buffer.set_style(layout.area(), Style::default().bg(theme.surface));
        for block in model.blocks() {
            if let Some(area) = layout.block_area(block.id()) {
                blocks::render_block(block, model.visual(), area, theme, buffer);
            }
        }
        Ok(())
    }

    pub fn render_rect(
        self,
        model: WidgetRenderModel,
        preset: DashboardPreset,
        area: Rect,
        theme: &Theme,
        buffer: &mut Buffer,
    ) -> Result<(), TerminalRenderError> {
        let bounds = DashboardBounds::new(
            usize::from(area.x),
            usize::from(area.y),
            usize::from(area.width),
            usize::from(area.height),
        )
        .map_err(|_| TerminalRenderError::BoundsOverflow)?;
        self.render(model, preset, bounds, theme, buffer)
    }

    pub(crate) fn render_rect_with_inventory(
        self,
        model: WidgetRenderModel,
        preset: DashboardPreset,
        area: Rect,
        theme: &Theme,
        buffer: &mut Buffer,
        inventory: &HardwareInventory,
    ) -> Result<(), TerminalRenderError> {
        let bounds = DashboardBounds::new(
            usize::from(area.x),
            usize::from(area.y),
            usize::from(area.width),
            usize::from(area.height),
        )
        .map_err(|_| TerminalRenderError::BoundsOverflow)?;
        self.render_with_inventory(model, preset, bounds, theme, buffer, Some(inventory))
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use ratatui::{
        buffer::Buffer,
        layout::Rect,
        style::{Color, Modifier},
    };

    use super::super::super::chart::{TimeSample, TimeSeries};
    use super::super::super::metrics::{
        CpuMetricInput, CpuMetricModel, GpuMetricInput, GpuMetricModel, MemoryMetricInput,
        MemoryMetricModel, MetricReading, NetworkMetricInput, NetworkMetricModel,
        StorageMetricInput, StorageMetricModel,
    };
    use super::super::super::{
        DashboardState, DashboardWidgetId, GpuDevice, HardwareId, HardwareInventory, HardwareKind,
        MetricAvailability, MetricUnit, MetricValue, MetricVisual, StorageDevice, VisualPriority,
        WidgetAvailability, WidgetContext, WidgetRenderer, WidgetState,
    };
    use super::*;

    fn history<const CAPACITY: usize>(unit: MetricUnit, values: &[f64]) -> TimeSeries<CAPACITY> {
        let mut series = TimeSeries::new(unit);
        for (index, value) in values.iter().copied().enumerate() {
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

    fn cpu_model() -> CpuMetricModel<8> {
        CpuMetricModel::from_input(
            CpuMetricInput::new(
                MetricReading::normalized(Some(0.42), MetricAvailability::Available).unwrap(),
                MetricReading::normalized(Some(0.30), MetricAvailability::Available).unwrap(),
                MetricReading::normalized(Some(0.12), MetricAvailability::Available).unwrap(),
                MetricReading::normalized(Some(0.58), MetricAvailability::Available).unwrap(),
                MetricReading::non_negative(Some(3_200_000_000.0), MetricAvailability::Available)
                    .unwrap(),
                MetricReading::new(Some(55.0), MetricAvailability::Available).unwrap(),
                8,
                history(MetricUnit::Percent, &[0.20, 0.30, 0.42]),
            )
            .unwrap(),
        )
        .unwrap()
    }

    fn memory_model() -> MemoryMetricModel<8> {
        MemoryMetricModel::from_input(
            MemoryMetricInput::new(
                MetricReading::non_negative(Some(60.0), MetricAvailability::Available).unwrap(),
                MetricReading::non_negative(Some(40.0), MetricAvailability::Available).unwrap(),
                MetricReading::non_negative(Some(100.0), MetricAvailability::Available).unwrap(),
                MetricReading::non_negative(Some(10.0), MetricAvailability::Available).unwrap(),
                MetricReading::non_negative(Some(0.0), MetricAvailability::Available).unwrap(),
                MetricReading::normalized(Some(0.60), MetricAvailability::Available).unwrap(),
                history(MetricUnit::Percent, &[0.40, 0.50, 0.60]),
            )
            .unwrap(),
        )
        .unwrap()
    }

    fn gpu_model() -> GpuMetricModel<8> {
        GpuMetricModel::from_input(
            GpuMetricInput::new(
                MetricReading::normalized(Some(0.37), MetricAvailability::Available).unwrap(),
                MetricReading::non_negative(
                    Some(0.7 * 1024.0 * 1024.0 * 1024.0),
                    MetricAvailability::Available,
                )
                .unwrap(),
                MetricReading::non_negative(
                    Some(8.0 * 1024.0 * 1024.0 * 1024.0),
                    MetricAvailability::Available,
                )
                .unwrap(),
                MetricReading::new(Some(53.0), MetricAvailability::Available).unwrap(),
                MetricReading::non_negative(Some(1.8e9), MetricAvailability::Available).unwrap(),
                MetricReading::non_negative(Some(120.0), MetricAvailability::Available).unwrap(),
                history(MetricUnit::Percent, &[0.20, 0.32, 0.37]),
            )
            .unwrap(),
        )
        .unwrap()
    }

    fn network_model() -> NetworkMetricModel<8> {
        NetworkMetricModel::from_input(
            NetworkMetricInput::new(
                MetricReading::non_negative(Some(80.0), MetricAvailability::Available).unwrap(),
                MetricReading::non_negative(Some(20.0), MetricAvailability::Available).unwrap(),
                MetricReading::non_negative(Some(800.0), MetricAvailability::Available).unwrap(),
                MetricReading::non_negative(Some(200.0), MetricAvailability::Available).unwrap(),
                MetricReading::non_negative(Some(100.0), MetricAvailability::Available).unwrap(),
                history(MetricUnit::BytesPerSecond, &[20.0, 60.0, 80.0]),
                history(MetricUnit::BytesPerSecond, &[10.0, 15.0, 20.0]),
            )
            .unwrap(),
        )
        .unwrap()
    }

    fn storage_model() -> StorageMetricModel<8> {
        StorageMetricModel::from_input(
            StorageMetricInput::new(
                MetricReading::non_negative(Some(500.0), MetricAvailability::Available).unwrap(),
                MetricReading::non_negative(Some(1_000.0), MetricAvailability::Available).unwrap(),
                MetricReading::non_negative(Some(0.0), MetricAvailability::Available).unwrap(),
                MetricReading::non_negative(Some(0.0), MetricAvailability::Available).unwrap(),
                MetricReading::non_negative(Some(0.0), MetricAvailability::Available).unwrap(),
                MetricReading::normalized(Some(0.0), MetricAvailability::Available).unwrap(),
                history(MetricUnit::Percent, &[0.40, 0.50]),
                history(MetricUnit::BytesPerSecond, &[0.0, 0.0]),
                history(MetricUnit::BytesPerSecond, &[0.0, 0.0]),
            )
            .unwrap(),
        )
        .unwrap()
    }

    fn context(preset: DashboardPreset) -> WidgetContext {
        WidgetContext::new(preset, DashboardState::Calm, Duration::from_secs(1))
            .with_availability(WidgetAvailability::all())
    }

    fn buffer_text(buffer: &Buffer) -> String {
        let mut text = String::new();
        for y in buffer.area.y..buffer.area.bottom() {
            for x in buffer.area.x..buffer.area.right() {
                text.push_str(buffer[(x, y)].symbol());
            }
            text.push('\n');
        }
        text
    }

    fn render_cpu(preset: DashboardPreset, width: u16, height: u16) -> Buffer {
        let model = WidgetRenderer::render_cpu(cpu_model(), context(preset))
            .unwrap()
            .unwrap();
        let mut buffer = Buffer::empty(Rect::new(0, 0, width, height));
        TerminalRenderer::new()
            .render(
                model,
                preset,
                super::super::super::DashboardBounds::new(
                    0,
                    0,
                    usize::from(width),
                    usize::from(height),
                )
                .unwrap(),
                &crate::theme::Theme::pulse(),
                &mut buffer,
            )
            .unwrap();
        buffer
    }

    #[test]
    fn rendering_is_deterministic_and_uses_theme_tokens() {
        let first = render_cpu(DashboardPreset::LargeWidescreen, 48, 16);
        let second = render_cpu(DashboardPreset::LargeWidescreen, 48, 16);
        assert_eq!(first, second);

        let theme = crate::theme::Theme::pulse();
        assert_eq!(first[(0, 0)].bg, theme.surface);
        let allowed = [
            Color::Reset,
            theme.background,
            theme.surface,
            theme.text,
            theme.text_muted,
            theme.border_focused,
            theme.primary,
            theme.secondary,
            theme.info,
            theme.chart_primary,
            theme.chart_secondary,
            theme.chart_grid,
        ];
        assert!(first.content.iter().all(|cell| allowed.contains(&cell.fg)));
    }

    #[test]
    fn dashboard_cpu_route_uses_product_value_and_full_width_plot() {
        let buffer = render_cpu(DashboardPreset::Standard, 43, 10);
        let output = buffer_text(&buffer);
        assert_eq!(output.matches("42%").count(), 1);
        assert_eq!(buffer[(0, 2)].symbol(), "4");

        let mut chart_cells = 0;
        for y in 5..10 {
            for x in 0..43 {
                if buffer[(x, y)]
                    .symbol()
                    .chars()
                    .next()
                    .is_some_and(|character| ('\u{2800}'..='\u{28ff}').contains(&character))
                {
                    chart_cells += 1;
                }
            }
        }
        assert!(chart_cells >= 5);
    }

    #[test]
    fn card_language_prioritizes_value_and_removes_debug_labels() {
        let standard = render_cpu(DashboardPreset::Standard, 48, 8);
        let standard_text = buffer_text(&standard);
        assert!(standard_text.contains("◈ CPU"));
        assert_eq!(standard_text.matches("42%").count(), 1);
        assert_eq!(standard[(0, 2)].symbol(), "4");
        assert!(standard_text.contains("↑ +12%"));
        assert!(standard_text.contains("8 cores"));
        assert!(standard_text.contains("55.0°C"));
        assert!(!standard_text.contains("normal"));
        assert!(!standard_text.contains("history"));
        assert!(!standard_text.contains("trend "));
        assert!(!standard_text.contains("status "));

        let wide_text = buffer_text(&render_cpu(DashboardPreset::Widescreen, 48, 12));
        assert!(!wide_text.contains("u/s/i"));

        let large_text = buffer_text(&render_cpu(DashboardPreset::LargeWidescreen, 48, 16));
        assert!(large_text.contains("8 cores"));
        assert!(large_text.contains("3.2 GHz"));
        assert_eq!(large_text.matches("55.0°C").count(), 1);

        let compact = render_cpu(DashboardPreset::Compact, 48, 8);
        let compact_text = buffer_text(&compact);
        assert!(compact_text.contains("42%"));
        assert!(!compact_text.contains("u/s/i"));
        assert!(!compact_text.contains("███"));
    }

    #[test]
    fn card_layout_places_chart_beside_value_and_keeps_context_below_main_content() {
        let model = WidgetRenderer::render_cpu(cpu_model(), context(DashboardPreset::Widescreen))
            .unwrap()
            .unwrap();
        let bounds = super::super::super::DashboardBounds::new(0, 0, 48, 12).unwrap();
        let layout = TerminalLayout::new(
            model,
            DashboardPreset::Widescreen,
            bounds,
            Rect::new(0, 0, 48, 12),
        )
        .unwrap();
        let value = layout
            .block_area(super::super::super::BlockId::Value)
            .unwrap();
        let trend = layout
            .block_area(super::super::super::BlockId::Trend)
            .unwrap();
        let chart = layout
            .block_area(super::super::super::BlockId::PrimarySparkline)
            .unwrap();
        let context = layout
            .block_area(super::super::super::BlockId::Status)
            .is_none();
        let context_area = layout
            .block_area(super::super::super::BlockId::CpuContext)
            .unwrap();

        assert_eq!(chart.y, value.y);
        assert!(chart.x > value.x + value.width);
        assert!((30..=45).contains(&(usize::from(chart.width) * 100 / 48)));
        assert!(trend.y > value.y);
        assert!(context);
        assert!(context_area.y >= chart.bottom());
    }

    #[test]
    fn visual_priority_maps_hero_above_primary_and_secondary() {
        let buffer = render_cpu(DashboardPreset::Compact, 32, 4);
        let theme = crate::theme::Theme::pulse();
        assert_eq!(buffer[(0, 1)].fg, theme.text);
        assert_eq!(buffer[(0, 2)].fg, theme.primary);

        let wide_model =
            WidgetRenderer::render_cpu(cpu_model(), context(DashboardPreset::Standard))
                .unwrap()
                .unwrap();
        let wide_layout = TerminalLayout::new(
            wide_model,
            DashboardPreset::Standard,
            super::super::super::DashboardBounds::new(0, 0, 32, 8).unwrap(),
            Rect::new(0, 0, 32, 8),
        )
        .unwrap();
        let chart_area = wide_layout
            .block_area(super::super::super::BlockId::PrimarySparkline)
            .unwrap();
        assert!(chart_area.x > 0);
        assert!(chart_area.width < 32);
        let wide = render_cpu(DashboardPreset::Standard, 32, 8);
        assert!(
            wide.content
                .iter()
                .any(|cell| cell.fg == theme.chart_primary)
        );
    }

    #[test]
    fn states_focus_and_unavailability_are_visible_without_inventing_values() {
        let critical_visual = MetricVisual::new(
            DashboardWidgetId::Cpu,
            WidgetState::Critical,
            VisualPriority::Hero,
        )
        .with_primary(
            MetricValue::new(0.95, MetricUnit::Percent, 1, VisualPriority::Hero).unwrap(),
        );
        let mut model =
            WidgetRenderer::render(critical_visual, context(DashboardPreset::Widescreen), "CPU")
                .unwrap()
                .unwrap();
        model.add_status().unwrap();
        let mut critical = Buffer::empty(Rect::new(0, 0, 32, 8));
        TerminalRenderer::new()
            .render(
                model,
                DashboardPreset::Widescreen,
                super::super::super::DashboardBounds::new(0, 0, 32, 8).unwrap(),
                &crate::theme::Theme::pulse(),
                &mut critical,
            )
            .unwrap();
        assert!(buffer_text(&critical).contains("×"));

        let focused = WidgetRenderer::render_cpu(
            cpu_model(),
            context(DashboardPreset::Compact).with_focus(Some(DashboardWidgetId::Cpu)),
        )
        .unwrap()
        .unwrap();
        let mut focused_buffer = Buffer::empty(Rect::new(0, 0, 32, 4));
        TerminalRenderer::new()
            .render(
                focused,
                DashboardPreset::Compact,
                super::super::super::DashboardBounds::new(0, 0, 32, 4).unwrap(),
                &crate::theme::Theme::pulse(),
                &mut focused_buffer,
            )
            .unwrap();
        let theme = crate::theme::Theme::pulse();
        assert_eq!(focused_buffer[(0, 1)].bg, theme.selection_background);
        assert!(focused_buffer[(0, 1)].modifier.contains(Modifier::BOLD));

        let unavailable_visual = MetricVisual::new(
            DashboardWidgetId::Cpu,
            WidgetState::Unavailable,
            VisualPriority::Hero,
        );
        let unavailable =
            WidgetRenderer::render(unavailable_visual, context(DashboardPreset::Compact), "CPU")
                .unwrap()
                .unwrap();
        let mut unavailable_buffer = Buffer::empty(Rect::new(0, 0, 32, 4));
        TerminalRenderer::new()
            .render(
                unavailable,
                DashboardPreset::Compact,
                super::super::super::DashboardBounds::new(0, 0, 32, 4).unwrap(),
                &theme,
                &mut unavailable_buffer,
            )
            .unwrap();
        assert_eq!(unavailable_buffer[(0, 1)].symbol(), "—");
        assert!(buffer_text(&unavailable_buffer).contains("·"));
    }

    #[test]
    fn charts_render_with_responsive_degradation() {
        let compact = render_cpu(DashboardPreset::Compact, 48, 8);
        let standard = render_cpu(DashboardPreset::Standard, 48, 8);
        let wide = render_cpu(DashboardPreset::Widescreen, 48, 12);
        let large = render_cpu(DashboardPreset::LargeWidescreen, 48, 16);
        let non_blank = |buffer: &Buffer| {
            buffer
                .content
                .iter()
                .filter(|cell| cell.symbol() != " ")
                .count()
        };
        assert!(non_blank(&standard) > non_blank(&compact));
        let contains_braille = |buffer: &Buffer| {
            buffer.content.iter().any(|cell| {
                cell.symbol()
                    .chars()
                    .next()
                    .is_some_and(|character| ('\u{2800}'..='\u{28ff}').contains(&character))
            })
        };
        assert!(contains_braille(&standard));
        assert!(contains_braille(&wide));
        assert!(non_blank(&large) >= non_blank(&wide));

        let memory = WidgetRenderer::render_memory(
            memory_model(),
            context(DashboardPreset::LargeWidescreen),
        )
        .unwrap()
        .unwrap();
        let network = WidgetRenderer::render_network(
            network_model(),
            context(DashboardPreset::LargeWidescreen),
        )
        .unwrap()
        .unwrap();
        let mut buffer = Buffer::empty(Rect::new(0, 0, 72, 24));
        let bounds = super::super::super::DashboardBounds::new(0, 0, 72, 24).unwrap();
        TerminalRenderer::new()
            .render(
                memory,
                DashboardPreset::LargeWidescreen,
                bounds,
                &crate::theme::Theme::pulse(),
                &mut buffer,
            )
            .unwrap();
        assert!(buffer_text(&buffer).contains("60 B"));
        let memory_text = buffer_text(&buffer);
        assert!(memory_text.contains("60%"));
        assert!(!memory_text.lines().any(|line| line.trim() == "60%"));
        assert!(memory_text.contains('█'));
        let memory_lines: Vec<&str> = memory_text.lines().collect();
        let memory_value_line = memory_lines
            .iter()
            .position(|line| line.contains("60 B / 100 B"))
            .expect("memory value row");
        assert!(memory_lines[memory_value_line].contains("60%"));
        assert!(memory_lines[memory_value_line + 1].contains('█'));
        assert!(
            !memory_text
                .chars()
                .any(|character| ('\u{2800}'..='\u{28ff}').contains(&character))
        );

        let gpu = WidgetRenderer::render_gpu(gpu_model(), context(DashboardPreset::Standard))
            .unwrap()
            .unwrap();
        let mut gpu_buffer = Buffer::empty(Rect::new(0, 0, 48, 8));
        TerminalRenderer::new()
            .render(
                gpu,
                DashboardPreset::Standard,
                super::super::super::DashboardBounds::new(0, 0, 48, 8).unwrap(),
                &crate::theme::Theme::pulse(),
                &mut gpu_buffer,
            )
            .unwrap();
        let gpu_text = buffer_text(&gpu_buffer);
        assert!(gpu_text.contains("0.7 / 8 GB"));
        assert!(gpu_text.contains("53°C"));
        assert!(!gpu_text.contains("678.0 MB"));

        let storage =
            WidgetRenderer::render_storage(storage_model(), context(DashboardPreset::Standard))
                .unwrap()
                .unwrap();
        let mut storage_buffer = Buffer::empty(Rect::new(0, 0, 48, 8));
        TerminalRenderer::new()
            .render(
                storage,
                DashboardPreset::Standard,
                super::super::super::DashboardBounds::new(0, 0, 48, 8).unwrap(),
                &crate::theme::Theme::pulse(),
                &mut storage_buffer,
            )
            .unwrap();
        let storage_text = buffer_text(&storage_buffer);
        assert!(storage_text.contains("500 B / 1000 B"));
        assert!(storage_text.contains('█'));

        let mut comparison = Buffer::empty(Rect::new(0, 0, 72, 24));
        TerminalRenderer::new()
            .render(
                network,
                DashboardPreset::LargeWidescreen,
                bounds,
                &crate::theme::Theme::pulse(),
                &mut comparison,
            )
            .unwrap();
        assert!(buffer_text(&comparison).contains('↓'));
        assert!(buffer_text(&comparison).contains('↑'));
        let vital_lines = buffer_text(&comparison)
            .lines()
            .filter(|line| {
                line.chars()
                    .any(|character| ('\u{2800}'..='\u{28ff}').contains(&character))
            })
            .count();
        assert!(vital_lines >= 2);
        assert!(!buffer_text(&comparison).contains("60s"));
    }

    #[test]
    fn storage_inventory_renderer_keeps_multiple_devices_visible() {
        let model = WidgetRenderer::render_storage(
            storage_model(),
            context(DashboardPreset::LargeWidescreen),
        )
        .unwrap()
        .unwrap();
        let storage_device =
            |key: &str, name: &str, model: &str, total: f64, used: f64| StorageDevice {
                id: HardwareId::new(HardwareKind::Storage, key).unwrap(),
                name: name.to_owned(),
                model: Some(model.to_owned()),
                mount_points: Vec::new(),
                used_bytes: Some(used),
                total_bytes: Some(total),
                read_bytes_per_sec: Some(0.0),
                write_bytes_per_sec: Some(0.0),
                latency_seconds: None,
                availability: MetricAvailability::Available,
                is_virtual: false,
            };
        let inventory = HardwareInventory::new(
            vec![
                storage_device("nvme0n1", "nvme0n1", "KINGSTON SNV2S1000G", 931.5, 215.0),
                storage_device("sda", "sda", "KINGSTON SA400S37480G", 447.1, 412.0),
            ],
            Vec::new(),
            Vec::new(),
            Vec::new(),
        );
        let mut buffer = Buffer::empty(Rect::new(0, 0, 72, 10));
        TerminalRenderer::new()
            .render_rect_with_inventory(
                model,
                DashboardPreset::LargeWidescreen,
                Rect::new(0, 0, 72, 10),
                &crate::theme::Theme::pulse(),
                &mut buffer,
                &inventory,
            )
            .unwrap();
        let text = buffer_text(&buffer);
        assert!(text.contains("sda"));
        assert!(text.contains("nvme0n1"));
        assert!(text.matches('█').count() + text.matches('░').count() >= 72 * 2);
    }

    #[test]
    fn gpu_inventory_renderer_keeps_devices_separate_and_explains_missing_telemetry() {
        let model = WidgetRenderer::render_gpu(gpu_model(), context(DashboardPreset::Standard))
            .unwrap()
            .unwrap();
        let inventory = HardwareInventory::new(
            Vec::new(),
            vec![
                GpuDevice {
                    id: HardwareId::new(HardwareKind::Gpu, "amd").unwrap(),
                    name: "AMD Radeon · AMDGPU".to_owned(),
                    utilization_percent: None,
                    vram_used_bytes: None,
                    vram_total_bytes: None,
                    temperature_celsius: None,
                    frequency_hz: None,
                    power_watts: None,
                    availability: MetricAvailability::Available,
                },
                GpuDevice {
                    id: HardwareId::new(HardwareKind::Gpu, "nvidia").unwrap(),
                    name: "NVIDIA GeForce RTX 4060".to_owned(),
                    utilization_percent: None,
                    vram_used_bytes: None,
                    vram_total_bytes: None,
                    temperature_celsius: None,
                    frequency_hz: None,
                    power_watts: None,
                    availability: MetricAvailability::Available,
                },
            ],
            Vec::new(),
            Vec::new(),
        );
        let mut buffer = Buffer::empty(Rect::new(0, 0, 56, 8));
        TerminalRenderer::new()
            .render_rect_with_inventory(
                model,
                DashboardPreset::Standard,
                Rect::new(0, 0, 56, 8),
                &crate::theme::Theme::pulse(),
                &mut buffer,
                &inventory,
            )
            .unwrap();
        let text = buffer_text(&buffer);
        assert!(text.contains("RTX 4060"));
        assert!(text.contains("driver unavailable"));
        assert!(!text.contains("AMD Radeon"));
        assert!(!text.contains("█"));
    }

    #[test]
    fn gpu_inventory_renderer_uses_a_segmented_meter_for_real_usage() {
        let model = WidgetRenderer::render_gpu(gpu_model(), context(DashboardPreset::Standard))
            .unwrap()
            .unwrap();
        let inventory = HardwareInventory::new(
            Vec::new(),
            vec![GpuDevice {
                id: HardwareId::new(HardwareKind::Gpu, "nvidia").unwrap(),
                name: "NVIDIA GeForce RTX 4060".to_owned(),
                utilization_percent: Some(26.0),
                vram_used_bytes: Some(0.7 * 1024.0 * 1024.0 * 1024.0),
                vram_total_bytes: Some(8.0 * 1024.0 * 1024.0 * 1024.0),
                temperature_celsius: Some(51.0),
                frequency_hz: None,
                power_watts: None,
                availability: MetricAvailability::Available,
            }],
            Vec::new(),
            Vec::new(),
        );
        let mut buffer = Buffer::empty(Rect::new(0, 0, 60, 6));
        TerminalRenderer::new()
            .render_rect_with_inventory(
                model,
                DashboardPreset::Standard,
                Rect::new(0, 0, 60, 6),
                &crate::theme::Theme::pulse(),
                &mut buffer,
                &inventory,
            )
            .unwrap();
        let text = buffer_text(&buffer);
        assert!(text.contains("RTX 4060"));
        assert!(text.contains("26%"));
        assert!(text.contains("█"));
        assert!(text.contains("░"));
        assert!(text.matches('█').count() + text.matches('░').count() >= 60);
        assert!(text.contains("0.7 / 8 GB"));
        assert!(text.contains("51°C"));
    }

    #[test]
    fn small_and_offset_regions_fail_safely_or_render_without_panic() {
        let model =
            WidgetRenderer::render_cpu(cpu_model(), context(DashboardPreset::LargeWidescreen))
                .unwrap()
                .unwrap();
        let mut small = Buffer::empty(Rect::new(0, 0, 8, 2));
        TerminalRenderer::new()
            .render(
                model,
                DashboardPreset::LargeWidescreen,
                super::super::super::DashboardBounds::new(0, 0, 8, 2).unwrap(),
                &crate::theme::Theme::pulse(),
                &mut small,
            )
            .unwrap();

        let mut offset = Buffer::empty(Rect::new(4, 3, 32, 8));
        TerminalRenderer::new()
            .render_rect(
                model,
                DashboardPreset::LargeWidescreen,
                Rect::new(4, 3, 32, 8),
                &crate::theme::Theme::pulse(),
                &mut offset,
            )
            .unwrap();
        assert_eq!(offset.area, Rect::new(4, 3, 32, 8));
        assert!(
            TerminalRenderer::new()
                .render(
                    model,
                    DashboardPreset::LargeWidescreen,
                    super::super::super::DashboardBounds::new(0, 0, 64, 8).unwrap(),
                    &crate::theme::Theme::pulse(),
                    &mut offset,
                )
                .is_err()
        );
    }
}
