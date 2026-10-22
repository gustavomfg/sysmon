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
    #[cfg(test)]
    pub(super) fn render(
        model: WidgetRenderModel,
        preset: DashboardPreset,
        bounds: DashboardBounds,
        theme: &Theme,
        buffer: &mut Buffer,
    ) -> Result<(), TerminalRenderError> {
        Self::render_with_cores(model, preset, bounds, theme, buffer, &[])
    }

    pub(super) fn render_with_cores(
        model: WidgetRenderModel,
        preset: DashboardPreset,
        bounds: DashboardBounds,
        theme: &Theme,
        buffer: &mut Buffer,
        cores: &[f64],
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
            let core_map = core_map_area(cores, body);
            let detail = core_detail_area(cores, body);
            let plot_body = Rect::new(
                body.x,
                body.y,
                body.width,
                body.height
                    .saturating_sub(detail.map_or(0, |area| area.height + 1)),
            );
            let text_width = core_map.map_or(body.width, |map| {
                map.x.saturating_sub(body.x).saturating_sub(CORE_MAP_GAP)
            });
            render_body(model, plot_body, text_width, theme, buffer);
            if let Some(map) = core_map {
                render_core_map(cores, map, theme, buffer);
            }
            if let Some(detail) = detail {
                render_core_detail(cores, detail, theme, buffer);
            }
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

fn render_body(
    model: WidgetRenderModel,
    area: Rect,
    text_width: u16,
    theme: &Theme,
    buffer: &mut Buffer,
) {
    let text_area = Rect::new(area.x, area.y, text_width, area.height);
    let value_offset = if area.height >= 7 { 1 } else { 0 };
    let value = line_region(text_area, area.y.saturating_add(value_offset));
    let hardware = line_region(text_area, value.y.saturating_add(1));
    let secondary = line_region(text_area, hardware.y.saturating_add(1));
    let history_y = secondary.y.saturating_add(1);
    let regions = CpuCardRegions {
        value,
        hardware,
        secondary,
        history: Rect::new(
            area.x,
            history_y.min(area.bottom()),
            area.width,
            area.bottom()
                .saturating_sub(history_y)
                .min(history_chart::MAX_CHART_ROWS),
        ),
    };

    render_primary_value(model, regions.value, theme, buffer);

    let hardware_text = hardware_line(model, text_width);
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

/// Text column reserved on the left of the card before the core map.
const CORE_MAP_TEXT_WIDTH: u16 = 22;
const CORE_MAP_GAP: u16 = 2;
const LEVELS: [char; 9] = [' ', '▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

fn core_map_area(cores: &[f64], body: Rect) -> Option<Rect> {
    let rows = if body.height >= 7 { 4 } else { 3 };
    let free = body
        .width
        .saturating_sub(CORE_MAP_TEXT_WIDTH + CORE_MAP_GAP);
    let count = u16::try_from(cores.len()).unwrap_or(u16::MAX);
    if count == 0 || free < count || body.height < rows + 1 {
        return None;
    }
    let stride = if free >= count.saturating_mul(2).saturating_sub(1) {
        2
    } else {
        1
    };
    let width = count
        .saturating_sub(1)
        .saturating_mul(stride)
        .saturating_add(1);
    Some(Rect::new(
        body.right().saturating_sub(width),
        body.y,
        width,
        rows,
    ))
}

/// Per-core load as a compact equalizer in the empty right side of the
/// value rows: one heat-colored column per logical CPU, so imbalance (one
/// pinned core among idle ones) is visible even when the average is low.
fn render_core_map(cores: &[f64], area: Rect, theme: &Theme, buffer: &mut Buffer) {
    let rows = area.height;
    let count = u16::try_from(cores.len()).unwrap_or(u16::MAX);
    // Gaps between bars when they fit, a solid block otherwise.
    let stride = if area.width >= count.saturating_mul(2).saturating_sub(1) {
        2
    } else {
        1
    };
    let bottom = area.bottom().saturating_sub(1);
    for (index, usage) in cores.iter().enumerate() {
        let x = area
            .x
            .saturating_add(u16::try_from(index).unwrap_or(0).saturating_mul(stride));
        let load = (usage / 100.0).clamp(0.0, 1.0);
        let mut color = super::super::vital::heat_color(load, theme);
        if stride == 1 && index % 2 == 1 {
            // Touching bars alternate tone so neighbors stay distinguishable.
            color = super::super::vital::mix(color, theme.surface, 0.30);
        }
        let eighths = (load * f64::from(rows) * 8.0).round() as u16;
        for row in 0..rows {
            let filled = eighths.saturating_sub(row * 8).min(8);
            let (symbol, foreground) = if filled == 0 && row == 0 {
                ('▁', theme.border)
            } else {
                (LEVELS[usize::from(filled)], color)
            };
            buffer[(x, bottom - row)]
                .set_char(symbol)
                .set_style(Style::default().fg(foreground).bg(theme.surface));
        }
    }
}

fn line_region(area: Rect, row: u16) -> Rect {
    if row >= area.bottom() {
        Rect::new(area.x, area.bottom(), area.width, 0)
    } else {
        Rect::new(area.x, row, area.width, 1)
    }
}

fn core_detail_area(cores: &[f64], body: Rect) -> Option<Rect> {
    if cores.is_empty() || body.height < 16 || body.width < 48 {
        return None;
    }
    let rows = (cores.len().div_ceil(4) as u16).min((body.height / 4).clamp(2, 8));
    Some(Rect::new(
        body.x,
        body.bottom() - rows - 1,
        body.width,
        rows + 1,
    ))
}

fn render_core_detail(cores: &[f64], area: Rect, theme: &Theme, buffer: &mut Buffer) {
    let slots = usize::from(area.height.saturating_sub(1)) * 4;
    let mut readings = cores.iter().copied().enumerate().collect::<Vec<_>>();
    if readings.len() > slots {
        readings.sort_by(|(left_id, left), (right_id, right)| {
            right.total_cmp(left).then_with(|| left_id.cmp(right_id))
        });
    }
    let title = if cores.len() > slots {
        format!("CORE LOAD · busiest {slots}/{}", cores.len())
    } else {
        "CORE LOAD · logical CPUs".to_owned()
    };
    write(
        buffer,
        area,
        area.x,
        area.y,
        &title,
        Style::default().fg(theme.text_muted).bg(theme.surface),
    );
    let width = area.width / 4;
    for (slot, (index, usage)) in readings.into_iter().take(slots).enumerate() {
        let x = area.x + (slot as u16 % 4) * width;
        let y = area.y + 1 + slot as u16 / 4;
        let valid = usage.is_finite() && (0.0..=100.0).contains(&usage);
        let text = if valid {
            format!("c{index:02} {usage:>3.0}%")
        } else {
            format!("c{index:02}    —")
        };
        let color = if valid {
            super::super::vital::heat_color(usage / 100.0, theme)
        } else {
            theme.text_muted
        };
        write(
            buffer,
            area,
            x,
            y,
            &text,
            Style::default().fg(theme.text).bg(theme.surface),
        );
        let start = x + text.chars().count() as u16 + 1;
        let meter = Rect::new(start, y, (x + width).saturating_sub(start + 2), 1);
        super::instruments::capacity_meter(
            meter,
            valid.then_some(usage / 100.0),
            color,
            theme,
            buffer,
        );
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
    history_chart::instrument(
        &[(sparkline(model), chart_style(model, theme))],
        Scale::Percent,
        area,
        theme,
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
    let comparison = model.visual().comparison();
    let direction = if comparison.has_previous() {
        comparison.direction()
    } else {
        model.visual().trend().direction()
    };
    let arrow = match direction {
        TrendDirection::Rising => "↑",
        TrendDirection::Falling => "↓",
        TrendDirection::Stable => "→",
        TrendDirection::Unknown => "",
    };
    if arrow.is_empty() {
        return String::new();
    }
    let delta = model.visual().comparison().delta();
    let suffix = comparison.window().map_or_else(String::new, |window| {
        if window.subsec_millis() == 0 {
            format!(" · {}s", window.as_secs())
        } else {
            format!(" · {:.1}s", window.as_secs_f64())
        }
    });
    match delta {
        Some(delta)
            if model
                .visual()
                .primary()
                .is_some_and(|value| value.unit() == MetricUnit::Percent) =>
        {
            format!("{arrow} {:+.0}pp{suffix}", delta * 100.0)
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
    Style::default().fg(color).bg(theme.surface)
}

fn header_style(model: WidgetRenderModel, theme: &Theme) -> Style {
    // Focus is carried by the panel ring; the card title echoes its color.
    let mut style = Style::default().fg(theme.text_muted).bg(theme.surface);
    if model.is_focused() {
        style = style.fg(theme.border_focused).add_modifier(Modifier::BOLD);
    }
    style
}

fn chart_style(model: WidgetRenderModel, theme: &Theme) -> Style {
    Style::default()
        .fg(super::super::vital::metric_color(model.state(), theme))
        .bg(theme.surface)
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

    #[test]
    fn core_map_draws_one_heat_column_per_core_in_the_free_space() {
        let context = WidgetContext::new(
            DashboardPreset::Widescreen,
            DashboardState::Calm,
            Duration::ZERO,
        )
        .with_availability(WidgetAvailability::all());
        let model = WidgetRenderer::render_cpu(model_at(0.2), context)
            .unwrap()
            .unwrap();
        let cores = [0.0, 100.0, 50.0, 12.5];
        let mut buffer = Buffer::empty(Rect::new(0, 0, 40, 8));
        CpuCardRenderer::render_with_cores(
            model,
            DashboardPreset::Widescreen,
            DashboardBounds::new(0, 0, 40, 8).unwrap(),
            &Theme::pulse(),
            &mut buffer,
            &cores,
        )
        .unwrap();
        // Bars are right-aligned with one-cell gaps: x = 33, 35, 37, 39.
        let column = |x: u16| -> String { (1..5).map(|y| buffer[(x, y)].symbol()).collect() };
        assert_eq!(column(33), "   ▁", "idle core keeps a floor");
        assert_eq!(column(35), "████", "saturated core fills the map");
        assert_eq!(column(37), "  ██");
        assert_ne!(buffer[(35, 4)].fg, buffer[(39, 4)].fg, "load sets heat");

        // Without per-core data the card renders exactly as before.
        assert!(text(&render_at(0.2, 40, 8)).chars().all(|c| c != '█'));
    }

    #[test]
    fn expanded_core_detail_includes_saturated_high_index_cores_and_stays_bounded() {
        let theme = Theme::pulse();
        let area = Rect::new(0, 0, 120, 30);
        let mut buffer = Buffer::empty(area);
        let mut cores = vec![5.0; 64];
        cores[63] = 100.0;
        let detail = core_detail_area(&cores, area).unwrap();
        render_core_detail(&cores, detail, &theme, &mut buffer);
        let text = text(&buffer);
        assert!(text.contains("c63 100%"));
        assert!(text.contains("busiest 28/64"));
        assert!(detail.bottom() <= area.bottom());
    }

    #[test]
    fn core_map_reserves_a_gap_and_keeps_cpu_context_legible() {
        let context = WidgetContext::new(
            DashboardPreset::Standard,
            DashboardState::Calm,
            Duration::ZERO,
        )
        .with_availability(WidgetAvailability::all());
        let model = WidgetRenderer::render_cpu(model(), context)
            .unwrap()
            .unwrap();
        for count in [8, 10] {
            let mut buffer = Buffer::empty(Rect::new(0, 0, 32, 8));
            CpuCardRenderer::render_with_cores(
                model,
                DashboardPreset::Standard,
                DashboardBounds::new(0, 0, 32, 8).unwrap(),
                &Theme::pulse(),
                &mut buffer,
                &vec![100.0; count],
            )
            .unwrap();
            let output = text(&buffer);
            assert!(output.contains("8 cores · 3.2 GHz"));
            assert!(output.contains("55.0°C  ↑ +32pp · 10s"));
            if count == 8 {
                for y in 1..5 {
                    assert_eq!(buffer[(22, y)].symbol(), " ");
                    assert_eq!(buffer[(23, y)].symbol(), " ");
                }
                assert!(output.contains('█'));
            } else {
                assert!(
                    !output.contains('█'),
                    "omit cores when the text and gap cannot fit"
                );
            }
        }
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
        buffer
            .content
            .chunks(usize::from(buffer.area.width))
            .map(|row| row.iter().map(|cell| cell.symbol()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
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
        assert_eq!(
            output.lines().filter(|line| line.trim() == "42%").count(),
            1
        );
        assert_eq!(rendered[(0, 2)].symbol(), "4");
        assert!(output.contains("8 cores"));
        assert!(output.contains("3.2 GHz"));
        assert!(output.contains("55.0°C"));
        assert!(output.contains("↑ +32pp · 10s"));
        assert!(output.contains('%'));
        assert!(!output.contains("normal"));
    }

    #[test]
    fn value_is_prominent_and_chart_occupies_the_right_region() {
        let rendered = render(DashboardPreset::Standard, 43, 10);
        let output = text(&rendered);
        assert_eq!(
            output.lines().filter(|line| line.trim() == "42%").count(),
            1
        );
        assert_eq!(rendered[(0, 2)].symbol(), "4");
        assert!(output.contains("8 cores"));
        assert!(output.contains("↑ +32pp"));
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
        assert!(text(&rendered).contains("100%"));
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
            let mut positions = Vec::new();
            for y in 5..11 {
                for x in 0..48 {
                    if rendered[(x, y)]
                        .symbol()
                        .chars()
                        .next()
                        .is_some_and(|c| ('\u{2801}'..='\u{28ff}').contains(&c))
                    {
                        positions.push((x, y));
                    }
                }
            }
            assert!(!positions.is_empty());
            assert!(
                positions.iter().all(|(_, y)| *y == positions[0].1),
                "constant telemetry is a flat trace"
            );
            let retained = model_at(value).sparkline().duration().as_secs_f64();
            let first = 5 + ((1.0 - retained / 120.0) * 85.0 / 2.0).floor() as u16;
            let actual = positions.iter().map(|(x, _)| *x).min().unwrap();
            assert!(actual.abs_diff(first) <= 1, "uncollected time stays blank");
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
