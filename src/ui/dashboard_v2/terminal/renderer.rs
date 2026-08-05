use ratatui::{buffer::Buffer, layout::Rect, style::Style};

use crate::theme::Theme;

use super::super::{DashboardBounds, DashboardPreset, WidgetRenderModel};
use super::{blocks, layout::TerminalLayout, layout::TerminalLayoutError};

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
        CpuMetricInput, CpuMetricModel, MemoryMetricInput, MemoryMetricModel, MetricReading,
        NetworkMetricInput, NetworkMetricModel,
    };
    use super::super::super::{
        DashboardState, DashboardWidgetId, MetricAvailability, MetricUnit, MetricValue,
        MetricVisual, VisualPriority, WidgetAvailability, WidgetContext, WidgetRenderer,
        WidgetState,
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
                MetricReading::non_negative(Some(3.2), MetricAvailability::Available).unwrap(),
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
    fn visual_priority_maps_hero_above_primary_and_secondary() {
        let buffer = render_cpu(DashboardPreset::Compact, 32, 4);
        let theme = crate::theme::Theme::pulse();
        assert_eq!(buffer[(0, 1)].fg, theme.text);
        assert_eq!(buffer[(0, 2)].fg, theme.primary);

        let wide = render_cpu(DashboardPreset::Standard, 32, 8);
        assert_eq!(wide[(0, 3)].fg, theme.chart_primary);
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
        assert!(buffer_text(&critical).contains("status critical"));

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
        assert!(buffer_text(&unavailable_buffer).contains("trend ·"));
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
        assert!(non_blank(&wide) > non_blank(&standard));
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
        assert!(buffer_text(&buffer).contains('█'));
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
        assert!(buffer_text(&comparison).contains("comparison"));
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
