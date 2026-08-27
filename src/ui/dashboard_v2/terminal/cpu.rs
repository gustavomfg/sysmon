use std::time::Duration;

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

const RENDER_EMA_ALPHA: f64 = 0.32;
const PEAK_PRESERVE_THRESHOLD: f64 = 0.06;
const PEAK_PRESERVE_WEIGHT: f64 = 0.58;
const CURRENT_POINT_WEIGHT: f64 = 0.65;
const LOCAL_BASELINE_ALPHA: f64 = 0.45;
const LOCAL_DETAIL_GAIN: f64 = 2.0;
const LOCAL_DETAIL_LIMIT: f64 = 0.05;
const CPU_VISUAL_WINDOW: Duration = Duration::from_secs(30);
const MAX_BRAILLE_CELLS: usize = 4096;

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
}

#[derive(Debug, Clone, Copy)]
struct CpuCardRegions {
    value: Rect,
    hardware: Rect,
    secondary: Rect,
    history: Rect,
}

fn render_body(model: WidgetRenderModel, area: Rect, theme: &Theme, buffer: &mut Buffer) {
    let value_offset = if area.height >= 6 { 1 } else { 0 };
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
            area.bottom().saturating_sub(history_y),
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
    let Some(sparkline) = sparkline(model) else {
        write(buffer, area, area.x, area.y, "—", chart_style(model, theme));
        return;
    };
    if area.width < 8 || area.height < 2 {
        render_compact_history(sparkline, area, model, theme, buffer);
        return;
    }

    // The history region is already the lower half of the CPU instrument.
    // Give its complete height to the Braille raster instead of centering a
    // shorter plot inside it. This makes absolute utilization readable from
    // the curve itself without changing the metric or its scale.
    let plot = Rect::new(area.x, area.y, area.width, area.height);
    render_line_plot(sparkline, plot, model, theme, buffer);
}

fn render_compact_history(
    sparkline: SparklineModel<CHART_POINTS>,
    area: Rect,
    model: WidgetRenderModel,
    theme: &Theme,
    buffer: &mut Buffer,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let (raw_values, count) = chart_values(sparkline);
    let values = visual_values(raw_values, count);
    let scale = PlotScale::default();
    let mut previous = None;
    for column in 0..area.width {
        let position = if area.width == 1 || count <= 1 {
            0.0
        } else {
            f64::from(column) * (count - 1) as f64 / f64::from(area.width - 1)
        };
        let lower = position.floor() as usize;
        let upper = position.ceil() as usize;
        let value = interpolate(values[lower], values[upper], position - lower as f64);
        let current = value.map(|value| plot_row(value, 2, scale));
        let glyph = match (previous, current) {
            (Some(previous), Some(current)) if previous < current => '╲',
            (Some(previous), Some(current)) if previous > current => '╱',
            (Some(_), Some(_)) => '─',
            _ => '·',
        };
        put(
            buffer,
            area,
            area.x + column,
            area.y,
            glyph,
            chart_style(model, theme),
        );
        previous = current;
    }
}

fn render_line_plot(
    sparkline: SparklineModel<CHART_POINTS>,
    area: Rect,
    model: WidgetRenderModel,
    theme: &Theme,
    buffer: &mut Buffer,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let (raw_values, count) = chart_values(sparkline);
    if count == 0 {
        write(buffer, area, area.x, area.y, "—", chart_style(model, theme));
        return;
    }

    let values = visual_values(raw_values, count);
    let scale = PlotScale::default();
    let pixel_width = usize::from(area.width).saturating_mul(2);
    let pixel_height = usize::from(area.height).saturating_mul(4);
    if pixel_width == 0 || pixel_height == 0 {
        return;
    }

    let cell_count = usize::from(area.width) * usize::from(area.height);
    if cell_count > MAX_BRAILLE_CELLS {
        render_compact_history(sparkline, area, model, theme, buffer);
        return;
    }

    // One fixed mask per Braille cell keeps the line light without allocating
    // while rendering. A single adjacent subpixel stroke gives the signal
    // enough presence to read at a glance while leaving its center on the
    // absolute 0..100% scale. The raw history remains in the model; only this
    // local presentation series is smoothed.
    let mut cells = [0u8; MAX_BRAILLE_CELLS];
    let mut previous = None;
    for (index, value) in values.iter().take(count).enumerate() {
        let Some(value) = *value else {
            previous = None;
            continue;
        };
        let x = if count <= 1 {
            0
        } else {
            index.saturating_mul(pixel_width - 1) / (count - 1)
        };
        let y = plot_row(value, pixel_height, scale);
        let current = (x, y);
        let support = braille_support_point(current, pixel_height);
        if let Some(previous) = previous {
            draw_braille_segment(&mut cells, area, previous, current);
            draw_braille_segment(
                &mut cells,
                area,
                braille_support_point(previous, pixel_height),
                support,
            );
        } else {
            set_braille_dot(&mut cells, area, current.0, current.1);
            set_braille_dot(&mut cells, area, support.0, support.1);
        }
        previous = Some(current);
    }

    for row in 0..area.height {
        for column in 0..area.width {
            let mask = cells[usize::from(row) * usize::from(area.width) + usize::from(column)];
            if mask != 0 {
                let character = char::from_u32(0x2800 + u32::from(mask)).unwrap_or('·');
                put(
                    buffer,
                    area,
                    area.x + column,
                    area.y + row,
                    character,
                    chart_style(model, theme),
                );
            }
        }
    }
}

fn chart_values(sparkline: SparklineModel<CHART_POINTS>) -> ([Option<f64>; CHART_POINTS], usize) {
    let mut values = [None; CHART_POINTS];
    let mut count = 0;
    let latest_offset = sparkline
        .points()
        .last()
        .map(|point| point.sample().offset());
    let visual_start = latest_offset.map(|offset| offset.saturating_sub(CPU_VISUAL_WINDOW));
    for point in sparkline.points().iter() {
        if visual_start.is_some_and(|start| point.sample().offset() < start) {
            continue;
        }
        if count == CHART_POINTS {
            break;
        }
        values[count] = point.sample().usable_value();
        count += 1;
    }
    (values, count)
}

fn visual_values(
    raw_values: [Option<f64>; CHART_POINTS],
    count: usize,
) -> [Option<f64>; CHART_POINTS] {
    amplify_local_detail(smooth_values(raw_values, count), raw_values, count)
}

fn amplify_local_detail(
    values: [Option<f64>; CHART_POINTS],
    raw_values: [Option<f64>; CHART_POINTS],
    count: usize,
) -> [Option<f64>; CHART_POINTS] {
    let mut visual = [None; CHART_POINTS];
    let mut baseline = None;

    for index in 0..count.min(CHART_POINTS) {
        let Some(value) = values[index] else {
            baseline = None;
            continue;
        };

        let sample = raw_values[index].unwrap_or(value);
        let local_baseline = baseline.unwrap_or(sample);
        let bounded_detail = ((sample - local_baseline) * LOCAL_DETAIL_GAIN)
            .clamp(-LOCAL_DETAIL_LIMIT, LOCAL_DETAIL_LIMIT);
        visual[index] = Some((value + bounded_detail).clamp(0.0, 1.0));
        baseline = Some(match baseline {
            Some(previous) => previous + LOCAL_BASELINE_ALPHA * (sample - previous),
            None => sample,
        });
    }

    visual
}

fn smooth_values(
    raw_values: [Option<f64>; CHART_POINTS],
    count: usize,
) -> [Option<f64>; CHART_POINTS] {
    let mut smoothed = [None; CHART_POINTS];
    let mut previous = None;
    let last = count.saturating_sub(1);

    for index in 0..count.min(CHART_POINTS) {
        let Some(raw) = raw_values[index] else {
            previous = None;
            continue;
        };

        let mut value = match previous {
            Some(previous) => previous + RENDER_EMA_ALPHA * (raw - previous),
            None => raw,
        };
        let deviation = raw - value;
        if deviation.abs() >= PEAK_PRESERVE_THRESHOLD {
            value += deviation * PEAK_PRESERVE_WEIGHT;
        }
        if index == last {
            value += (raw - value) * CURRENT_POINT_WEIGHT;
        }

        value = value.clamp(0.0, 1.0);
        smoothed[index] = Some(value);
        previous = Some(value);
    }
    smoothed
}

fn draw_braille_segment(cells: &mut [u8], area: Rect, start: (usize, usize), end: (usize, usize)) {
    let mut x = start.0 as isize;
    let mut y = start.1 as isize;
    let target_x = end.0 as isize;
    let target_y = end.1 as isize;
    let delta_x = (target_x - x).abs();
    let step_x = if x < target_x { 1 } else { -1 };
    let delta_y = -(target_y - y).abs();
    let step_y = if y < target_y { 1 } else { -1 };
    let mut error = delta_x + delta_y;

    loop {
        set_braille_dot(cells, area, x as usize, y as usize);
        if x == target_x && y == target_y {
            break;
        }
        let doubled = error.saturating_mul(2);
        if doubled >= delta_y {
            error += delta_y;
            x += step_x;
        }
        if doubled <= delta_x {
            error += delta_x;
            y += step_y;
        }
    }
}

fn braille_support_point(point: (usize, usize), pixel_height: usize) -> (usize, usize) {
    let last_row = pixel_height.saturating_sub(1);
    if point.1 < last_row {
        (point.0, point.1 + 1)
    } else {
        (point.0, point.1.saturating_sub(1))
    }
}

fn set_braille_dot(cells: &mut [u8], area: Rect, x: usize, y: usize) {
    let pixel_width = usize::from(area.width).saturating_mul(2);
    let pixel_height = usize::from(area.height).saturating_mul(4);
    if x >= pixel_width || y >= pixel_height {
        return;
    }
    let column = x / 2;
    let row = y / 4;
    let bit = match (x % 2, y % 4) {
        (0, 0) => 0b0000_0001,
        (0, 1) => 0b0000_0010,
        (0, 2) => 0b0000_0100,
        (0, 3) => 0b0100_0000,
        (1, 0) => 0b0000_1000,
        (1, 1) => 0b0001_0000,
        (1, 2) => 0b0010_0000,
        (1, 3) => 0b1000_0000,
        _ => return,
    };
    let index = row * usize::from(area.width) + column;
    if let Some(cell) = cells.get_mut(index) {
        *cell |= bit;
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

fn interpolate(lower: Option<f64>, upper: Option<f64>, amount: f64) -> Option<f64> {
    match (lower, upper) {
        (Some(lower), Some(upper)) => Some(lower + (upper - lower) * amount),
        (Some(value), None) | (None, Some(value)) => Some(value),
        (None, None) => None,
    }
}

fn plot_row(value: f64, height: usize, scale: PlotScale) -> usize {
    let normalized = ((value - scale.low) / (scale.high - scale.low)).clamp(0.0, 1.0);
    ((1.0 - normalized) * height.saturating_sub(1) as f64).round() as usize
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

    use super::super::super::chart::{TimeSample, TimeSeries, TimeWindow};
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

    fn raw_values(values: &[f64]) -> ([Option<f64>; CHART_POINTS], usize) {
        let mut raw = [None; CHART_POINTS];
        let count = values.len().min(CHART_POINTS);
        for (slot, value) in raw.iter_mut().zip(values.iter().copied()).take(count) {
            *slot = Some(value);
        }
        (raw, count)
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
    fn absolute_scale_preserves_semantic_vertical_order_and_bounds() {
        let scale = PlotScale::default();
        assert_eq!(scale.low, 0.0);
        assert_eq!(scale.high, 1.0);

        let low = plot_row(0.10, 16, scale);
        let middle = plot_row(0.50, 16, scale);
        let high = plot_row(0.90, 16, scale);
        assert!(low > middle && middle > high);
        assert_eq!(plot_row(-1.0, 16, scale), 15);
        assert_eq!(plot_row(2.0, 16, scale), 0);
    }

    #[test]
    fn render_smoothing_is_deterministic_and_reduces_small_oscillations() {
        let (raw, count) = raw_values(&[0.30, 0.32, 0.31, 0.33, 0.32, 0.34]);
        let first = smooth_values(raw, count);
        let second = smooth_values(raw, count);

        assert_eq!(first, second);
        let raw_delta = (raw[5].unwrap() - raw[4].unwrap()).abs();
        let smoothed_delta = (first[5].unwrap() - first[4].unwrap()).abs();
        assert!(smoothed_delta < raw_delta);
    }

    #[test]
    fn render_smoothing_preserves_peaks_and_keeps_the_current_point_close() {
        let (raw, count) = raw_values(&[0.25, 0.27, 0.80, 0.29]);
        let smoothed = smooth_values(raw, count);

        assert!(smoothed[2].unwrap() > 0.50);
        assert!((smoothed[3].unwrap() - raw[3].unwrap()).abs() < 0.20);
    }

    #[test]
    fn local_detail_reveals_small_real_changes_without_leaving_absolute_region() {
        let (raw, count) = raw_values(&[0.08, 0.10, 0.09, 0.12, 0.08, 0.11]);
        let smoothed = smooth_values(raw, count);
        let visual = visual_values(raw, count);

        assert!(visual[3].unwrap() > visual[2].unwrap());
        assert!(visual[4].unwrap() < visual[3].unwrap());
        for (rendered, baseline) in visual[..count].iter().zip(smoothed[..count].iter()) {
            let rendered = rendered.unwrap();
            let baseline = baseline.unwrap();
            assert!((rendered - baseline).abs() <= LOCAL_DETAIL_LIMIT + 1.0e-9);
            assert!((0.0..=1.0).contains(&rendered));
        }
    }

    #[test]
    fn local_detail_keeps_constant_low_mid_and_high_load_stable() {
        for value in [0.10, 0.50, 0.90] {
            let (raw, count) = raw_values(&[value, value, value, value, value]);
            let visual = visual_values(raw, count);
            assert!(
                visual[..count]
                    .iter()
                    .flatten()
                    .all(|rendered| (*rendered - value).abs() < f64::EPSILON)
            );
        }
    }

    #[test]
    fn constant_rendered_levels_use_the_absolute_cpu_scale() {
        let positions = [0.10, 0.30, 0.50, 0.80, 0.95].map(|value| {
            let rendered = render_at(value, 48, 12);
            (5..12)
                .find(|row| {
                    (0..48).any(|column| {
                        rendered[(column, *row)]
                            .symbol()
                            .chars()
                            .next()
                            .is_some_and(|character| ('\u{2800}'..='\u{28ff}').contains(&character))
                    })
                })
                .expect("constant level should render a Braille line")
        });
        assert!(positions[0] > positions[2]);
        assert!(positions[2] > positions[4]);
        assert!(positions[0] > positions[1]);
        assert!(positions[3] > positions[4]);
    }

    #[test]
    fn visual_window_uses_recent_points_without_mutating_the_source_model() {
        let mut history = TimeSeries::<64>::new(MetricUnit::Percent);
        for index in 0..12 {
            history
                .push(
                    TimeSample::new(
                        Duration::from_secs(index * 5),
                        Some(0.20 + index as f64 * 0.01),
                        MetricAvailability::Available,
                        None,
                    )
                    .unwrap(),
                )
                .unwrap();
        }
        let sparkline = history.sparkline::<CHART_POINTS>(TimeWindow::Long);
        let source_length = sparkline.points().len();
        let (values, count) = chart_values(sparkline);

        assert!(count > 0);
        assert!(count < source_length);
        assert_eq!(values[count - 1], Some(0.31));
        assert_eq!(sparkline.points().len(), source_length);
    }

    #[test]
    fn isolated_peaks_and_abrupt_drops_remain_visible_after_smoothing() {
        let (raw, count) = raw_values(&[0.10, 0.12, 0.92, 0.14]);
        let smoothed = smooth_values(raw, count);

        assert!(smoothed[2].unwrap() > smoothed[1].unwrap());
        assert!(smoothed[3].unwrap() < smoothed[2].unwrap());
        assert!(
            smoothed[..count]
                .iter()
                .flatten()
                .all(|value| value.is_finite())
        );
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
