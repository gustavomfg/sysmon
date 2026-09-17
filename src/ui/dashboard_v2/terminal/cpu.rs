use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
};

use crate::theme::Theme;

use super::super::chart::SparklineModel;
use super::super::metrics::CHART_POINTS;
use super::super::{
    BlockId, ChartContent, DashboardBounds, DashboardPreset, MetricUnit, MetricValue, RenderBlock,
    TrendDirection, WidgetRenderModel, WidgetState,
};
use super::renderer::TerminalRenderError;

use super::history_chart::{self, Scale};

pub(super) struct CpuCardRenderer;

impl CpuCardRenderer {
    pub(super) fn render(
        model: WidgetRenderModel,
        preset: DashboardPreset,
        bounds: DashboardBounds,
        theme: &Theme,
        buffer: &mut Buffer,
    ) -> Result<(), TerminalRenderError> {
        let area = to_rect(bounds, *buffer.area())?;
        buffer.set_style(area, Style::default().bg(theme.surface));
        if area.width == 0 || area.height == 0 {
            return Ok(());
        }

        render_header(model, area, theme, buffer);
        if area.height == 1 {
            return Ok(());
        }

        let body = Rect::new(
            area.x,
            area.y.saturating_add(1),
            area.width,
            area.height.saturating_sub(1),
        );
        if preset == DashboardPreset::Compact || body.width < 24 || body.height < 4 {
            render_compact_body(model, body, theme, buffer);
        } else {
            render_body(model, body, theme, buffer);
        }
        Ok(())
    }
}

fn render_header(model: WidgetRenderModel, area: Rect, theme: &Theme, buffer: &mut Buffer) {
    let style = header_style(model, theme);
    let marker = match model.state() {
        WidgetState::Active => " ·",
        WidgetState::Warning => " !",
        WidgetState::Critical => " ×",
        WidgetState::Unavailable => " ·",
        _ => "",
    };
    let title = format!("CPU{marker}");
    write(buffer, area, area.x, area.y, &title, style);
}

fn render_compact_body(model: WidgetRenderModel, area: Rect, theme: &Theme, buffer: &mut Buffer) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let value = model
        .visual()
        .primary()
        .map_or_else(|| "—".to_owned(), format_metric_value);
    write(
        buffer,
        area,
        area.x,
        area.y,
        &value,
        value_style(model, theme, true),
    );
    if area.height > 1 {
        let trend = format_trend(model);
        if !trend.is_empty() {
            write(
                buffer,
                area,
                area.x,
                area.y + 1,
                &trend,
                trend_style(model, theme),
            );
        }
    }
    if area.height > 2
        && let Some(temperature) = temperature(model)
    {
        write(
            buffer,
            area,
            area.x,
            area.y + 2,
            &format_metric_value(temperature),
            context_style(model, theme),
        );
    }
    if area.height > 3 {
        render_history(model, history_chart::region(area, 3), theme, buffer);
    }
}

#[derive(Debug, Clone, Copy)]
struct CpuCardRegions {
    value: Rect,
    hardware: Rect,
    secondary: Rect,
    history: Rect,
}

fn render_body(model: WidgetRenderModel, area: Rect, theme: &Theme, buffer: &mut Buffer) {
    let value_offset = if area.height >= 7 { 1 } else { 0 };
    let value = line_region(area, area.y.saturating_add(value_offset));
    let hardware = line_region(area, value.y.saturating_add(1));
    let secondary = line_region(area, hardware.y.saturating_add(1));
    let history_y = secondary.y.saturating_add(1);
    let regions = CpuCardRegions {
        value,
        hardware,
        secondary,
        history: Rect::new(
            area.x,
            history_y.min(area.bottom()),
            area.width,
            area.bottom().saturating_sub(history_y).min(3),
        ),
    };

    render_primary_value(model, regions.value, theme, buffer);

    let hardware_text = hardware_line(model, area.width);
    if !hardware_text.is_empty() {
        write_rich_context(
            buffer,
            regions.hardware,
            &hardware_text,
            context_style(model, theme),
            theme,
            context_value_style(model, theme),
        );
    }

    render_secondary_context(model, regions.secondary, theme, buffer);

    render_history(model, regions.history, theme, buffer);
}

fn line_region(area: Rect, row: u16) -> Rect {
    if row >= area.bottom() {
        Rect::new(area.x, area.bottom(), area.width, 0)
    } else {
        Rect::new(area.x, row, area.width, 1)
    }
}

fn render_primary_value(model: WidgetRenderModel, area: Rect, theme: &Theme, buffer: &mut Buffer) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let value = model
        .visual()
        .primary()
        .map_or_else(|| "—".to_owned(), format_metric_value);
    write(
        buffer,
        area,
        area.x,
        area.y,
        &value,
        value_style(model, theme, true),
    );
}

fn render_secondary_context(
    model: WidgetRenderModel,
    area: Rect,
    theme: &Theme,
    buffer: &mut Buffer,
) {
    let x = if let Some(temperature) = temperature(model) {
        let value = format_metric_value(temperature);
        write(
            buffer,
            area,
            area.x,
            area.y,
            &value,
            context_value_style(model, theme),
        );
        area.x
            .saturating_add(value.chars().count() as u16)
            .saturating_add(2)
    } else {
        area.x
    };

    let trend = format_trend(model);
    if !trend.is_empty() {
        let trend_area = Rect::new(x, area.y, area.right().saturating_sub(x), area.height);
        write_rich_context(
            buffer,
            trend_area,
            &trend,
            trend_style(model, theme),
            theme,
            context_value_style(model, theme),
        );
    }
}

fn render_history(model: WidgetRenderModel, area: Rect, theme: &Theme, buffer: &mut Buffer) {
    history_chart::render(
        &[(sparkline(model), chart_style(model, theme))],
        Scale::Percent,
        area,
        buffer,
    );
}

fn sparkline(model: WidgetRenderModel) -> Option<SparklineModel<CHART_POINTS>> {
    model.blocks().find_map(|block| match block {
        RenderBlock::Chart(block) if block.metadata().id() == BlockId::PrimarySparkline => {
            match block.content() {
                ChartContent::Sparkline(sparkline) => Some(sparkline),
                _ => None,
            }
        }
        _ => None,
    })
}

fn temperature(model: WidgetRenderModel) -> Option<MetricValue> {
    model
        .visual()
        .secondary()
        .filter(|value| value.unit() == MetricUnit::TemperatureCelsius)
}

fn hardware_line(model: WidgetRenderModel, available_width: u16) -> String {
    let context = model.block(BlockId::CpuContext);
    let Some(RenderBlock::CpuContext(context)) = context else {
        return String::new();
    };
    let frequency = context.frequency().map(format_metric_value);
    let full = frequency.as_ref().map_or_else(
        || format!("{} cores", context.cores()),
        |frequency| format!("{} cores · {frequency}", context.cores()),
    );
    if full.chars().count() <= usize::from(available_width) {
        return full;
    }

    let compact = frequency.as_ref().map_or_else(
        || format!("{}c", context.cores()),
        |frequency| format!("{}c · {}", context.cores(), frequency.replace(' ', "")),
    );
    if compact.chars().count() <= usize::from(available_width) {
        compact
    } else {
        format!("{}c", context.cores())
    }
}

fn format_metric_value(value: MetricValue) -> String {
    match value.unit() {
        MetricUnit::Percent => format!("{:.0}%", value.value() * 100.0),
        MetricUnit::TemperatureCelsius => format!("{:.1}°C", value.value()),
        MetricUnit::FrequencyHertz => format_frequency(value.value()),
        _ => format!("{:.1}", value.value()),
    }
}

fn format_frequency(hertz: f64) -> String {
    let absolute = hertz.abs();
    if absolute >= 1.0e9 {
        format!("{:.1} GHz", hertz / 1.0e9)
    } else if absolute >= 1.0e6 {
        format!("{:.1} MHz", hertz / 1.0e6)
    } else if absolute >= 1.0e3 {
        format!("{:.1} kHz", hertz / 1.0e3)
    } else {
        format!("{:.1} Hz", hertz)
    }
}

fn format_trend(model: WidgetRenderModel) -> String {
    let arrow = match model.visual().trend().direction() {
        TrendDirection::Rising => "↑",
        TrendDirection::Falling => "↓",
        TrendDirection::Stable => "→",
        TrendDirection::Unknown => "",
    };
    if arrow.is_empty() {
        return String::new();
    }
    let delta = model.visual().comparison().delta();
    let window = sparkline(model).and_then(|sparkline| {
        let mut previous = None;
        let mut last = None;
        for point in sparkline.points().iter() {
            previous = last;
            last = Some(point);
        }
        let last = last?.sample().offset();
        let previous = previous?.sample().offset();
        let seconds = last.saturating_sub(previous).as_secs();
        (seconds > 0).then_some(seconds)
    });
    let suffix = window.map_or_else(String::new, |seconds| format!(" · {seconds}s"));
    match delta {
        Some(delta)
            if model
                .visual()
                .primary()
                .is_some_and(|value| value.unit() == MetricUnit::Percent) =>
        {
            format!("{arrow} {:+.0}%{suffix}", delta * 100.0)
        }
        Some(delta) => format!("{arrow} {delta:+.1}{suffix}"),
        None => arrow.to_owned(),
    }
}

fn value_style(model: WidgetRenderModel, theme: &Theme, bold: bool) -> Style {
    let color = match model.state() {
        WidgetState::Warning => theme.warning,
        WidgetState::Critical => theme.danger,
        WidgetState::Active => theme.info,
        WidgetState::Unavailable => theme.text_muted,
        _ => theme.text,
    };
    let mut style = Style::default().fg(color).bg(theme.surface);
    if bold {
        style = style.add_modifier(Modifier::BOLD);
    }
    if model.is_focused() {
        style = style
            .fg(theme.selection_text)
            .bg(theme.selection_background)
            .add_modifier(Modifier::BOLD);
    }
    style
}

fn context_style(model: WidgetRenderModel, theme: &Theme) -> Style {
    let mut style = Style::default().fg(theme.text_muted).bg(theme.surface);
    if matches!(model.state(), WidgetState::Warning | WidgetState::Critical) {
        style = style.fg(if model.state() == WidgetState::Critical {
            theme.danger
        } else {
            theme.warning
        });
    }
    if model.is_focused() {
        style = style
            .fg(theme.selection_text)
            .bg(theme.selection_background);
    }
    style
}

fn context_value_style(model: WidgetRenderModel, theme: &Theme) -> Style {
    value_style(model, theme, true)
}

fn trend_style(model: WidgetRenderModel, theme: &Theme) -> Style {
    let color = match model.state() {
        WidgetState::Warning => theme.warning,
        WidgetState::Critical => theme.danger,
        WidgetState::Active => theme.info,
        WidgetState::Unavailable => theme.text_muted,
        _ => theme.primary,
    };
    let mut style = Style::default().fg(color).bg(theme.surface);
    if model.is_focused() {
        style = style
            .fg(theme.selection_text)
            .bg(theme.selection_background)
            .add_modifier(Modifier::BOLD);
    }
    style
}

fn header_style(model: WidgetRenderModel, theme: &Theme) -> Style {
    let mut style = Style::default().fg(theme.primary).bg(theme.surface);
    if model.is_focused() {
        style = style
            .fg(theme.selection_text)
            .bg(theme.selection_background)
            .add_modifier(Modifier::BOLD);
    }
    style
}

fn chart_style(model: WidgetRenderModel, theme: &Theme) -> Style {
    let mut style = Style::default().fg(theme.chart_primary).bg(theme.surface);
    if model.is_focused() {
        style = style
            .fg(theme.selection_text)
            .bg(theme.selection_background);
    }
    style
}

fn write(buffer: &mut Buffer, area: Rect, x: u16, y: u16, text: &str, style: Style) {
    if y >= area.bottom() || x >= area.right() {
        return;
    }
    buffer.set_stringn(
        x,
        y,
        text,
        usize::from(area.right().saturating_sub(x)),
        style,
    );
}

fn write_rich_context(
    buffer: &mut Buffer,
    area: Rect,
    text: &str,
    base_style: Style,
    theme: &Theme,
    value_style: Style,
) {
    write(buffer, area, area.x, area.y, text, base_style);
    let mut token_start = None;
    for (index, character) in text.char_indices() {
        if character.is_whitespace() || matches!(character, '·' | '/') {
            if let Some(start) = token_start.take() {
                overlay_context_token(buffer, area, text, start, index, value_style);
            }
            if matches!(character, '·' | '/') {
                overlay_context_token(
                    buffer,
                    area,
                    text,
                    index,
                    index + character.len_utf8(),
                    context_separator_style(base_style, theme),
                );
            }
        } else if token_start.is_none() {
            token_start = Some(index);
        }
    }
    if let Some(start) = token_start {
        overlay_context_token(buffer, area, text, start, text.len(), value_style);
    }
}

fn overlay_context_token(
    buffer: &mut Buffer,
    area: Rect,
    text: &str,
    start: usize,
    end: usize,
    style: Style,
) {
    let token = &text[start..end];
    if !is_context_value(token) && token != "·" && token != "/" {
        return;
    }
    let offset = u16::try_from(text[..start].chars().count()).unwrap_or(u16::MAX);
    write(
        buffer,
        area,
        area.x.saturating_add(offset),
        area.y,
        token,
        style,
    );
}

fn context_separator_style(base_style: Style, theme: &Theme) -> Style {
    base_style.fg(theme.text_muted)
}

fn is_context_value(token: &str) -> bool {
    token.chars().any(|character| character.is_ascii_digit())
        || matches!(
            token,
            "B" | "B/s" | "GB" | "MB" | "KB" | "Hz" | "kHz" | "MHz" | "GHz" | "ms" | "s"
        )
}

fn to_rect(bounds: DashboardBounds, buffer_area: Rect) -> Result<Rect, TerminalRenderError> {
    let x = u16::try_from(bounds.x()).map_err(|_| TerminalRenderError::BoundsOverflow)?;
    let y = u16::try_from(bounds.y()).map_err(|_| TerminalRenderError::BoundsOverflow)?;
    let width = u16::try_from(bounds.width()).map_err(|_| TerminalRenderError::BoundsOverflow)?;
    let height = u16::try_from(bounds.height()).map_err(|_| TerminalRenderError::BoundsOverflow)?;
    let right = x
        .checked_add(width)
        .ok_or(TerminalRenderError::BoundsOverflow)?;
    let bottom = y
        .checked_add(height)
        .ok_or(TerminalRenderError::BoundsOverflow)?;
    if x < buffer_area.x
        || y < buffer_area.y
        || right > buffer_area.right()
        || bottom > buffer_area.bottom()
    {
        return Err(TerminalRenderError::BufferTooSmall);
    }
    Ok(Rect::new(x, y, width, height))
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use ratatui::{buffer::Buffer, layout::Rect};

    use super::super::super::chart::{TimeSample, TimeSeries};
    use super::super::super::metrics::{CpuMetricInput, CpuMetricModel, MetricReading};
    use super::super::super::{
        DashboardState, DashboardWidgetId, MetricAvailability, MetricUnit, WidgetAvailability,
        WidgetContext, WidgetRenderer,
    };
    use super::*;

    fn model() -> CpuMetricModel<8> {
        let mut history = TimeSeries::new(MetricUnit::Percent);
        for (index, value) in [0.18, 0.30, 0.42, 0.65, 0.40, 0.72].into_iter().enumerate() {
            history
                .push(
                    TimeSample::new(
                        Duration::from_secs(index as u64 * 10),
                        Some(value),
                        MetricAvailability::Available,
                        None,
                    )
                    .unwrap(),
                )
                .unwrap();
        }
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
                history,
            )
            .unwrap(),
        )
        .unwrap()
    }

    fn model_at(utilization: f64) -> CpuMetricModel<8> {
        let utilization = utilization.clamp(0.0, 1.0);
        let mut history = TimeSeries::new(MetricUnit::Percent);
        for (index, value) in [
            utilization,
            utilization,
            utilization,
            utilization,
            utilization,
            utilization,
        ]
        .into_iter()
        .enumerate()
        {
            history
                .push(
                    TimeSample::new(
                        Duration::from_secs(index as u64 * 10),
                        Some(value),
                        MetricAvailability::Available,
                        None,
                    )
                    .unwrap(),
                )
                .unwrap();
        }
        CpuMetricModel::from_input(
            CpuMetricInput::new(
                MetricReading::normalized(Some(utilization), MetricAvailability::Available)
                    .unwrap(),
                MetricReading::normalized(Some(utilization * 0.70), MetricAvailability::Available)
                    .unwrap(),
                MetricReading::normalized(Some(utilization * 0.30), MetricAvailability::Available)
                    .unwrap(),
                MetricReading::normalized(Some(1.0 - utilization), MetricAvailability::Available)
                    .unwrap(),
                MetricReading::non_negative(Some(3_200_000_000.0), MetricAvailability::Available)
                    .unwrap(),
                MetricReading::new(Some(55.0), MetricAvailability::Available).unwrap(),
                8,
                history,
            )
            .unwrap(),
        )
        .unwrap()
    }

    fn render(preset: DashboardPreset, width: u16, height: u16) -> Buffer {
        render_model(model(), preset, width, height)
    }

    fn render_at(utilization: f64, width: u16, height: u16) -> Buffer {
        render_model(
            model_at(utilization),
            DashboardPreset::Widescreen,
            width,
            height,
        )
    }

    fn render_model(
        metric_model: CpuMetricModel<8>,
        preset: DashboardPreset,
        width: u16,
        height: u16,
    ) -> Buffer {
        let context = WidgetContext::new(preset, DashboardState::Calm, Duration::ZERO)
            .with_availability(WidgetAvailability::all());
        let model = WidgetRenderer::render_cpu(metric_model, context)
            .unwrap()
            .unwrap();
        let mut buffer = Buffer::empty(Rect::new(0, 0, width, height));
        CpuCardRenderer::render(
            model,
            preset,
            DashboardBounds::new(0, 0, usize::from(width), usize::from(height)).unwrap(),
            &Theme::pulse(),
            &mut buffer,
        )
        .unwrap();
        buffer
    }

    fn text(buffer: &Buffer) -> String {
        buffer.content.iter().map(|cell| cell.symbol()).collect()
    }

    fn braille_cells(buffer: &Buffer, x_start: u16, x_end: u16, y_start: u16, y_end: u16) -> usize {
        (y_start..y_end)
            .flat_map(|y| (x_start..x_end).map(move |x| buffer[(x, y)].symbol()))
            .filter(|symbol| {
                symbol
                    .chars()
                    .next()
                    .is_some_and(|character| ('\u{2800}'..='\u{28ff}').contains(&character))
            })
            .count()
    }

    #[test]
    fn dedicated_card_has_large_value_hardware_and_real_history() {
        let rendered = render(DashboardPreset::Widescreen, 48, 12);
        let output = text(&rendered);
        assert!(output.contains("CPU"));
        assert_eq!(output.matches("42%").count(), 1);
        assert_eq!(rendered[(0, 2)].symbol(), "4");
        assert!(output.contains("8 cores"));
        assert!(output.contains("3.2 GHz"));
        assert!(output.contains("55.0°C"));
        assert!(output.contains("↑ +32% · 10s"));
        assert!(output.contains('%'));
        assert!(!output.contains("normal"));
    }

    #[test]
    fn value_is_prominent_and_chart_occupies_the_right_region() {
        let rendered = render(DashboardPreset::Standard, 43, 10);
        let output = text(&rendered);
        assert_eq!(output.matches("42%").count(), 1);
        assert_eq!(rendered[(0, 2)].symbol(), "4");
        assert!(output.contains("8 cores"));
        assert!(output.contains("↑ +32%"));
        assert!(!output.contains('█'));

        let chart_cells = braille_cells(&rendered, 0, 43, 5, 9);
        assert!(chart_cells >= 5);
    }

    #[test]
    fn reference_sized_card_uses_a_balanced_vertical_rhythm() {
        let rendered = render(DashboardPreset::Widescreen, 43, 10);

        assert_eq!(rendered[(0, 1)].symbol(), " ");
        assert_eq!(rendered[(0, 2)].symbol(), "4");
        assert_eq!(rendered[(0, 3)].symbol(), "8");
        assert_eq!(rendered[(0, 4)].symbol(), "5");

        let chart_rows = (5..9)
            .filter(|y| {
                (0..43).any(|x| {
                    rendered[(x, *y)]
                        .symbol()
                        .chars()
                        .next()
                        .is_some_and(|character| ('\u{2800}'..='\u{28ff}').contains(&character))
                })
            })
            .count();
        assert!(
            chart_rows >= 2,
            "chart should remain visible across the available body height"
        );
        assert!(!text(&rendered).contains("50s"));
        assert!(!text(&rendered).contains("100%"));
    }

    #[test]
    fn compact_dashboard_slot_keeps_a_two_row_vital_line() {
        let rendered = render(DashboardPreset::Standard, 48, 7);
        let chart_rows = (4..7)
            .filter(|y| {
                (0..48).any(|x| {
                    rendered[(x, *y)]
                        .symbol()
                        .chars()
                        .next()
                        .is_some_and(|character| ('\u{2800}'..='\u{28ff}').contains(&character))
                })
            })
            .count();
        assert!(chart_rows >= 2);
    }

    #[test]
    fn constant_histories_remain_flat_and_numeric_cpu_values_stay_absolute() {
        for value in [0.10, 0.30, 0.50, 0.80, 0.95] {
            let rendered = render_at(value, 48, 12);
            assert!(text(&rendered).contains(&format!("{:.0}%", value * 100.0)));
            assert!(text(&rendered).contains("55.0°C"));
            let row = (5..8)
                .find(|row| {
                    rendered[(0, *row)]
                        .symbol()
                        .chars()
                        .next()
                        .is_some_and(|c| ('\u{2801}'..='\u{28ff}').contains(&c))
                })
                .expect("constant level should render a Braille line");
            for column in 1..48 {
                assert_eq!(
                    rendered[(column, row)].symbol(),
                    rendered[(0, row)].symbol()
                );
            }
            assert_eq!(braille_cells(&rendered, 0, 48, 5, 8), 48);
        }
    }

    #[test]
    fn invalid_cpu_samples_are_rejected_before_rasterization() {
        assert!(MetricReading::normalized(Some(f64::NAN), MetricAvailability::Available).is_err());
        assert!(
            MetricReading::normalized(Some(f64::INFINITY), MetricAvailability::Available).is_err()
        );
    }

    #[test]
    fn compact_card_uses_one_line_fallback_without_panicking() {
        let rendered = render(DashboardPreset::Compact, 18, 4);
        let output = text(&rendered);
        assert!(output.contains("42%"));
        assert!(!output.contains("███"));
    }

    #[test]
    fn narrow_card_uses_compact_hardware_context_without_clipping() {
        let output = text(&render(DashboardPreset::Standard, 29, 9));
        assert!(output.contains("8 cores · 3.2 GHz"));
    }

    #[test]
    fn unavailable_card_does_not_invent_cpu_values() {
        let visual = super::super::super::MetricVisual::new(
            DashboardWidgetId::Cpu,
            WidgetState::Unavailable,
            super::super::super::VisualPriority::Hero,
        );
        let context = WidgetContext::new(
            DashboardPreset::Widescreen,
            DashboardState::Calm,
            Duration::ZERO,
        )
        .with_availability(WidgetAvailability::all());
        let model = WidgetRenderer::render(visual, context, "CPU")
            .unwrap()
            .unwrap();
        let mut buffer = Buffer::empty(Rect::new(0, 0, 48, 12));
        CpuCardRenderer::render(
            model,
            DashboardPreset::Widescreen,
            DashboardBounds::new(0, 0, 48, 12).unwrap(),
            &Theme::pulse(),
            &mut buffer,
        )
        .unwrap();
        let output = text(&buffer);
        assert!(output.contains('—'));
        assert!(!output.contains("0.0%"));
        assert!(!output.contains("normal"));
    }
}
