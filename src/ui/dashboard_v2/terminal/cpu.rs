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

const COLUMN_GAP: u16 = 2;
const RIGHT_WIDTH_PERCENT: u16 = 46;
const MIN_PLOT_RANGE: f64 = 0.08;
const PLOT_MARGIN_RATIO: f64 = 0.18;

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
            let (left, right) = split_body(body);
            render_left(model, left, theme, buffer);
            render_history(model, right, theme, buffer);
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
    let title = format!("◈ CPU{marker}");
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
}

fn split_body(area: Rect) -> (Rect, Rect) {
    let right_width = (area.width.saturating_mul(RIGHT_WIDTH_PERCENT) / 100)
        .max(12)
        .min(area.width.saturating_sub(COLUMN_GAP).saturating_sub(10));
    let left_width = area
        .width
        .saturating_sub(right_width)
        .saturating_sub(COLUMN_GAP);
    (
        Rect::new(area.x, area.y, left_width, area.height),
        Rect::new(
            area.x.saturating_add(left_width).saturating_add(COLUMN_GAP),
            area.y,
            right_width,
            area.height,
        ),
    )
}

fn render_left(model: WidgetRenderModel, area: Rect, theme: &Theme, buffer: &mut Buffer) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    // Keep one full row between the header and the value on reference-sized
    // cards. The value remains a normal terminal string; its visual weight
    // comes from isolation and contrast rather than invented typography.
    let value_offset = if area.height >= 8 {
        2
    } else {
        u16::from(area.height >= 6)
    };
    let value_y = area.y.saturating_add(value_offset);
    let value_area = Rect::new(
        area.x,
        value_y,
        area.width,
        area.bottom().saturating_sub(value_y),
    );
    render_primary_value(model, value_area, theme, buffer);

    // These slots intentionally leave breathing room around the primary
    // value while using the middle of the card for its supporting context.
    let mut y = value_y.saturating_add(u16::from(area.height >= 7) + 1);
    if y < area.bottom() {
        let context = hardware_line(model);
        if !context.is_empty() {
            write(
                buffer,
                area,
                area.x,
                y,
                &context,
                context_style(model, theme),
            );
            y = y.saturating_add(1);
        }
    }
    if y < area.bottom()
        && let Some(temperature) = temperature(model)
    {
        write(
            buffer,
            area,
            area.x,
            y,
            &format_metric_value(temperature),
            context_style(model, theme),
        );
        y = y.saturating_add(1);
    }
    if y < area.bottom() {
        let trend = format_trend(model);
        if !trend.is_empty() {
            write(buffer, area, area.x, y, &trend, trend_style(model, theme));
        }
    }
}

fn render_primary_value(
    model: WidgetRenderModel,
    area: Rect,
    theme: &Theme,
    buffer: &mut Buffer,
) -> u16 {
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
    1
}

fn render_history(model: WidgetRenderModel, area: Rect, theme: &Theme, buffer: &mut Buffer) {
    let Some(sparkline) = sparkline(model) else {
        write(buffer, area, area.x, area.y, "—", chart_style(model, theme));
        return;
    };
    if area.width < 8 || area.height < 2 {
        render_compact_history(sparkline, area, model, theme, buffer);
        return;
    }

    let scale_width = if area.width >= 18 { 4 } else { 0 };
    let timestamp_height = u16::from(area.height >= 4);
    let plot_height = area.height.saturating_sub(timestamp_height);
    let plot = Rect::new(
        area.x.saturating_add(scale_width),
        area.y,
        area.width.saturating_sub(scale_width),
        plot_height,
    );
    let scale = render_plot(sparkline, plot, model, theme, buffer);
    if scale_width > 0 {
        render_scale(
            scale,
            Rect::new(area.x, area.y, scale_width, plot_height),
            theme,
            buffer,
        );
    }
    if let Some(last) = sparkline.points().last() {
        let seconds = last.sample().offset().as_secs();
        if seconds > 0 {
            write(
                buffer,
                area,
                area.right().saturating_sub(5),
                area.bottom().saturating_sub(1),
                &format!("{seconds}s"),
                scale_style(theme),
            );
        }
    }
}

fn render_compact_history(
    sparkline: SparklineModel<CHART_POINTS>,
    area: Rect,
    model: WidgetRenderModel,
    theme: &Theme,
    buffer: &mut Buffer,
) {
    let points = sparkline.points();
    let values: Vec<char> = points
        .iter()
        .filter_map(|point| point.sample().usable_value())
        .map(|value| match (value * 8.0).round() as u8 {
            0 => '▁',
            1 => '▂',
            2 => '▃',
            3 => '▄',
            4 => '▅',
            5 => '▆',
            6 => '▇',
            _ => '█',
        })
        .collect();
    let text: String = values.into_iter().take(usize::from(area.width)).collect();
    write(
        buffer,
        area,
        area.x,
        area.y,
        &text,
        chart_style(model, theme),
    );
}

fn render_plot(
    sparkline: SparklineModel<CHART_POINTS>,
    area: Rect,
    model: WidgetRenderModel,
    theme: &Theme,
    buffer: &mut Buffer,
) -> PlotScale {
    if area.width == 0 || area.height == 0 {
        return PlotScale::default();
    }
    let points = sparkline.points();
    let mut values = [None; CHART_POINTS];
    let mut count = 0;
    for point in points.iter() {
        if count == CHART_POINTS {
            break;
        }
        values[count] = point.sample().usable_value();
        count += 1;
    }
    if count == 0 {
        write(buffer, area, area.x, area.y, "—", chart_style(model, theme));
        return PlotScale::default();
    }

    // Keep the absolute percentage context visible like the reference card;
    // the organic fixture still supplies enough variation for the bars to
    // communicate local movement without becoming a solid block.
    let scale = PlotScale::default();
    let step = if area.width >= 14 { 2 } else { 1 };
    for column in (0..area.width).step_by(step) {
        let position = if area.width == 1 {
            0.0
        } else {
            f64::from(column) * (count - 1) as f64 / f64::from(area.width - 1)
        };
        let lower = position.floor() as usize;
        let upper = position.ceil() as usize;
        let value = interpolate(values[lower], values[upper], position - lower as f64);
        let Some(value) = value else {
            continue;
        };
        render_bar(value, column, area, scale, model, theme, buffer);
    }
    if area.width > 1 && (usize::from(area.width - 1) % step != 0) {
        let column = area.width - 1;
        let position = f64::from(column) * (count - 1) as f64 / f64::from(area.width - 1);
        let lower = position.floor() as usize;
        let upper = position.ceil() as usize;
        if let Some(value) = interpolate(values[lower], values[upper], position - lower as f64) {
            render_bar(value, column, area, scale, model, theme, buffer);
        }
    }

    scale
}

fn render_bar(
    value: f64,
    column: u16,
    area: Rect,
    scale: PlotScale,
    model: WidgetRenderModel,
    theme: &Theme,
    buffer: &mut Buffer,
) {
    let top = plot_row(value, area.height, scale);
    for row in top..area.height {
        put(
            buffer,
            area,
            area.x + column,
            area.y + row,
            '█',
            chart_style(model, theme),
        );
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct PlotScale {
    low: f64,
    high: f64,
}

impl Default for PlotScale {
    fn default() -> Self {
        Self {
            low: 0.0,
            high: 1.0,
        }
    }
}

fn plot_scale(values: &[Option<f64>], count: usize) -> PlotScale {
    let mut low = f64::INFINITY;
    let mut high = f64::NEG_INFINITY;
    for value in values.iter().take(count).flatten() {
        low = low.min(*value);
        high = high.max(*value);
    }
    if !low.is_finite() || !high.is_finite() {
        return PlotScale::default();
    }

    let range = (high - low).max(0.0);
    let margin = (range * PLOT_MARGIN_RATIO).max(0.015);
    let mut lower = (low - margin).max(0.0);
    let mut upper = (high + margin).min(1.0);
    if upper - lower < MIN_PLOT_RANGE {
        let center = (low + high) * 0.5;
        lower = (center - MIN_PLOT_RANGE * 0.5).max(0.0);
        upper = (lower + MIN_PLOT_RANGE).min(1.0);
        lower = (upper - MIN_PLOT_RANGE).max(0.0);
    }
    PlotScale {
        low: lower,
        high: upper.max(lower + f64::EPSILON),
    }
}

fn render_scale(scale: PlotScale, area: Rect, theme: &Theme, buffer: &mut Buffer) {
    let middle = (scale.low + scale.high) * 0.5;
    write(
        buffer,
        area,
        area.x,
        area.y,
        &format_percent(scale.high),
        scale_style(theme),
    );
    if area.height >= 4 {
        write(
            buffer,
            area,
            area.x,
            area.y + area.height / 2,
            &format_percent(middle),
            scale_style(theme),
        );
    }
    write(
        buffer,
        area,
        area.x,
        area.bottom().saturating_sub(1),
        &format_percent(scale.low),
        scale_style(theme),
    );
}

fn format_percent(value: f64) -> String {
    format!("{:.0}%", value.clamp(0.0, 1.0) * 100.0)
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

fn hardware_line(model: WidgetRenderModel) -> String {
    let context = model.block(BlockId::CpuContext);
    let Some(RenderBlock::CpuContext(context)) = context else {
        return String::new();
    };
    let mut line = format!("{} cores", context.cores());
    if let Some(frequency) = context.frequency() {
        line.push_str(" · ");
        line.push_str(&format_metric_value(frequency));
    }
    line
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

fn interpolate(lower: Option<f64>, upper: Option<f64>, amount: f64) -> Option<f64> {
    match (lower, upper) {
        (Some(lower), Some(upper)) => Some(lower + (upper - lower) * amount),
        (Some(value), None) | (None, Some(value)) => Some(value),
        (None, None) => None,
    }
}

fn plot_row(value: f64, height: u16, scale: PlotScale) -> u16 {
    let normalized = ((value - scale.low) / (scale.high - scale.low)).clamp(0.0, 1.0);
    let row = ((1.0 - normalized) * f64::from(height.saturating_sub(1))).round();
    row as u16
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

fn scale_style(theme: &Theme) -> Style {
    Style::default().fg(theme.text_muted).bg(theme.surface)
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

fn put(buffer: &mut Buffer, area: Rect, x: u16, y: u16, character: char, style: Style) {
    if x < area.right() && y < area.bottom() {
        buffer[(x, y)].set_char(character).set_style(style);
    }
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

    fn render(preset: DashboardPreset, width: u16, height: u16) -> Buffer {
        let context = WidgetContext::new(preset, DashboardState::Calm, Duration::ZERO)
            .with_availability(WidgetAvailability::all());
        let model = WidgetRenderer::render_cpu(model(), context)
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

    #[test]
    fn dedicated_card_has_large_value_hardware_and_real_history() {
        let rendered = render(DashboardPreset::Widescreen, 48, 12);
        let output = text(&rendered);
        assert!(output.contains("◈ CPU"));
        assert_eq!(output.matches("42%").count(), 1);
        assert_eq!(rendered[(0, 3)].symbol(), "4");
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
        assert_eq!(rendered[(0, 3)].symbol(), "4");
        assert!(output.contains("8 cores"));
        assert!(output.contains("↑ +32%"));

        let chart_x = 24;
        let chart_rows = (2..10)
            .filter(|y| (chart_x..43).any(|x| rendered[(x, *y)].symbol() == "█"))
            .count();
        assert!(chart_rows >= 3);
    }

    #[test]
    fn reference_sized_card_uses_a_balanced_vertical_rhythm() {
        let rendered = render(DashboardPreset::Widescreen, 43, 10);

        assert_eq!(rendered[(0, 1)].symbol(), " ");
        assert_eq!(rendered[(0, 3)].symbol(), "4");
        assert_eq!(rendered[(0, 5)].symbol(), "8");
        assert_eq!(rendered[(0, 6)].symbol(), "5");
        assert_eq!(rendered[(0, 7)].symbol(), "↑");

        let chart_rows = (1..9)
            .filter(|y| (28..43).any(|x| rendered[(x, *y)].symbol() == "█"))
            .count();
        assert!(
            chart_rows >= 5,
            "chart should use the available body height"
        );
    }

    #[test]
    fn adaptive_scale_preserves_local_shape_and_peak_candidates() {
        let values = [
            Some(0.20),
            Some(0.31),
            Some(0.24),
            Some(0.34),
            Some(0.22),
            Some(0.29),
        ];
        let scale = plot_scale(&values, values.len());

        assert!(scale.low > 0.0);
        assert!(scale.high < 1.0);
        assert!((scale.high - scale.low) >= MIN_PLOT_RANGE);
        assert_ne!(plot_row(0.20, 8, scale), plot_row(0.34, 8, scale));
        assert!(plot_row(0.31, 8, scale) < plot_row(0.24, 8, scale));
        assert!(plot_row(0.34, 8, scale) < plot_row(0.22, 8, scale));
    }

    #[test]
    fn compact_card_uses_one_line_fallback_without_panicking() {
        let rendered = render(DashboardPreset::Compact, 18, 4);
        let output = text(&rendered);
        assert!(output.contains("42%"));
        assert!(!output.contains("███"));
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
