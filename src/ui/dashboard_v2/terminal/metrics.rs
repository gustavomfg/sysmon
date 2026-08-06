use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
};

use crate::theme::Theme;

use super::super::chart::SparklineModel;
use super::super::metrics::CHART_POINTS;
use super::super::{
    BlockId, ChartContent, DashboardBounds, DashboardPreset, RenderBlock, WidgetRenderModel,
    WidgetState,
};
use super::blocks::write;
use super::renderer::TerminalRenderError;

const GAP: u16 = 2;
const GRAPH_RATIO: u16 = 36;
const MIN_GRAPH_WIDTH: u16 = 10;
const MAX_BRAILLE_CELLS: usize = 4096;

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

    match model.visual().widget_id() {
        super::super::DashboardWidgetId::Memory => {
            render_memory(model, preset, area, theme, buffer);
            Ok(())
        }
        super::super::DashboardWidgetId::Gpu => {
            render_gpu(model, preset, area, theme, buffer);
            Ok(())
        }
        super::super::DashboardWidgetId::Network => {
            render_network(model, preset, area, theme, buffer);
            Ok(())
        }
        super::super::DashboardWidgetId::Storage => {
            render_storage(model, preset, area, theme, buffer);
            Ok(())
        }
        _ => Ok(()),
    }
}

fn render_memory(
    model: WidgetRenderModel,
    _preset: DashboardPreset,
    area: Rect,
    theme: &Theme,
    buffer: &mut Buffer,
) {
    render_header("▦ Memory", model, area, theme, buffer);
    let body = body(area);
    if body.height == 0 {
        return;
    }
    let Some(context) = context_block(model, BlockId::MemoryContext) else {
        render_unavailable(body, model, theme, buffer);
        return;
    };
    let RenderBlock::MemoryContext(context) = context else {
        return;
    };
    let left = body;
    let used = context.used();
    let total = context.total();
    let value = match (used, total) {
        (Some(used), Some(total)) => format!(
            "{} / {}",
            format_bytes(used.value()),
            format_bytes(total.value())
        ),
        (Some(used), None) => format_bytes(used.value()),
        _ => "—".to_owned(),
    };
    let occupancy = model
        .visual()
        .primary()
        .map_or_else(|| "—".to_owned(), |value| format_percent(value.value()));
    let fraction = used
        .zip(total)
        .map(|(used, total)| used.value() / total.value().max(1.0))
        .or_else(|| model.visual().primary().map(|value| value.value()));

    if left.height <= 2 {
        let compact_value = if occupancy == "—" {
            value.clone()
        } else {
            format!("{value}  ·  {occupancy}")
        };
        write(
            buffer,
            line(left, 0),
            &compact_value,
            value_style(model, theme),
        );
        if left.height > 1 {
            render_meter(
                line(left, 1),
                fraction.unwrap_or(0.0),
                theme.chart_primary,
                theme,
                buffer,
            );
        }
        return;
    }

    write(buffer, line(left, 0), &value, value_style(model, theme));
    if left.height > 1 {
        write(
            buffer,
            line(left, 1),
            &format!("{occupancy} used"),
            context_style(model, theme),
        );
    }

    let mut details = String::new();
    if let Some(pressure) = context
        .pressure()
        .map(|value| format_percent(value.value()))
    {
        details.push_str(&pressure);
    }
    if let Some(cache) = context.cache().map(|value| format_bytes(value.value())) {
        if !details.is_empty() {
            details.push_str("  ·  ");
        }
        details.push_str("cache ");
        details.push_str(&cache);
    }
    if let Some(swap) = context.swap().filter(|value| value.value() > 0.0) {
        if !details.is_empty() {
            details.push_str("  ·  ");
        }
        details.push_str("swap ");
        details.push_str(&format_bytes(swap.value()));
    }
    if details.is_empty()
        && let Some(available) = context.available()
    {
        details = format!("{} available", format_bytes(available.value()));
    }
    if left.height > 3 {
        write(buffer, line(left, 2), &details, context_style(model, theme));
        render_meter(
            line(left, 3),
            fraction.unwrap_or(0.0),
            theme.chart_primary,
            theme,
            buffer,
        );
    } else {
        render_meter(
            line(left, 2),
            fraction.unwrap_or(0.0),
            theme.chart_primary,
            theme,
            buffer,
        );
    }
}

fn render_gpu(
    model: WidgetRenderModel,
    preset: DashboardPreset,
    area: Rect,
    theme: &Theme,
    buffer: &mut Buffer,
) {
    render_header("◇ GPU", model, area, theme, buffer);
    let body = body(area);
    if body.height == 0 {
        return;
    }
    if model.state() == WidgetState::Unavailable {
        render_unavailable(body, model, theme, buffer);
        return;
    }
    let Some(RenderBlock::GpuContext(context)) = context_block(model, BlockId::GpuContext) else {
        render_unavailable(body, model, theme, buffer);
        return;
    };
    let (left, graph) = split(body, preset);
    let value = model
        .visual()
        .primary()
        .map_or_else(|| "—".to_owned(), |value| format_percent(value.value()));
    write(buffer, left, &value, value_style(model, theme));
    if left.height > 1 {
        let vram = context
            .vram_used()
            .zip(context.vram_total())
            .map(|(used, total)| format_compact_capacity(used.value(), total.value()));
        let temperature = context
            .temperature()
            .map(|value| format_temperature(value.value()));
        let text = match (vram, temperature) {
            (Some(vram), Some(temperature)) if left.width >= 24 => {
                format!("VRAM {vram}  ·  {temperature}")
            }
            (Some(vram), Some(temperature)) => format!("{vram}  ·  {temperature}"),
            (Some(vram), None) => format!("VRAM {vram}"),
            (None, Some(temperature)) => temperature,
            (None, None) => String::new(),
        };
        write(buffer, line(left, 1), &text, context_style(model, theme));
    }
    if left.height > 2 && preset == DashboardPreset::LargeWidescreen {
        let frequency = context
            .frequency()
            .map(|value| format_frequency(value.value()));
        let power = context
            .power()
            .map(|value| format!("{:.0} W", value.value()));
        let text = match (frequency, power) {
            (Some(frequency), Some(power)) => format!("{frequency}  ·  {power}"),
            (Some(frequency), None) => frequency,
            (None, Some(power)) => power,
            (None, None) => String::new(),
        };
        write(buffer, line(left, 2), &text, context_style(model, theme));
    }
    if preset != DashboardPreset::Compact
        && graph.width >= MIN_GRAPH_WIDTH
        && graph.height >= 2
        && let Some(sparkline) = sparkline(model, BlockId::PrimarySparkline)
    {
        let trace = Rect::new(
            graph.x,
            graph.y,
            graph.width,
            graph.height.saturating_sub(1),
        );
        let timestamp = Rect::new(graph.x, graph.bottom().saturating_sub(1), graph.width, 1);
        if trace.height > 0 {
            render_vital_line(sparkline, trace, 0.34, theme.chart_primary, theme, buffer);
        }
        render_window(sparkline, timestamp, theme, buffer);
    }
}

fn render_network(
    model: WidgetRenderModel,
    preset: DashboardPreset,
    area: Rect,
    theme: &Theme,
    buffer: &mut Buffer,
) {
    render_header("⇄ Network", model, area, theme, buffer);
    let body = body(area);
    if body.height == 0 {
        return;
    }
    let Some(RenderBlock::NetworkContext(context)) = context_block(model, BlockId::NetworkContext)
    else {
        render_unavailable(body, model, theme, buffer);
        return;
    };
    let (left, graph) = split(body, preset);
    let download = context
        .download()
        .map_or_else(|| "—".to_owned(), |value| format_rate(value.value()));
    let upload = context
        .upload()
        .map_or_else(|| "—".to_owned(), |value| format_rate(value.value()));
    write(
        buffer,
        left,
        &format!("↓ {download}"),
        value_style_with(model, theme, theme.chart_primary),
    );
    if left.height > 1 {
        write(
            buffer,
            line(left, 1),
            &format!("↑ {upload}"),
            value_style_with(model, theme, theme.chart_secondary),
        );
    }
    if left.height > 2 {
        let totals = context
            .download_total()
            .zip(context.upload_total())
            .map_or_else(String::new, |(download, upload)| {
                format!(
                    "{}  ·  {} total",
                    format_bytes(download.value()),
                    format_bytes(upload.value())
                )
            });
        write(buffer, line(left, 2), &totals, context_style(model, theme));
    }
    if preset != DashboardPreset::Compact
        && graph.width >= MIN_GRAPH_WIDTH
        && let Some((download_area, upload_area, timestamp_area)) = dual_graph_areas(graph)
    {
        if let Some(sparkline) = sparkline(model, BlockId::PrimarySparkline) {
            render_vital_line(
                sparkline,
                download_area,
                0.34,
                theme.chart_primary,
                theme,
                buffer,
            );
        }
        if let Some(sparkline) = sparkline(model, BlockId::SecondarySparkline) {
            render_vital_line(
                sparkline,
                upload_area,
                0.34,
                theme.chart_secondary,
                theme,
                buffer,
            );
            render_window(sparkline, timestamp_area, theme, buffer);
        }
    }
}

fn render_storage(
    model: WidgetRenderModel,
    preset: DashboardPreset,
    area: Rect,
    theme: &Theme,
    buffer: &mut Buffer,
) {
    render_header("▰ Storage", model, area, theme, buffer);
    let body = body(area);
    if body.height == 0 {
        return;
    }
    let Some(RenderBlock::StorageContext(context)) = context_block(model, BlockId::StorageContext)
    else {
        render_unavailable(body, model, theme, buffer);
        return;
    };
    let (left, graph) = split(body, preset);
    let value = context.used_space().zip(context.total_space()).map_or_else(
        || "—".to_owned(),
        |(used, total)| {
            format!(
                "{} / {}",
                format_bytes(used.value()),
                format_bytes(total.value())
            )
        },
    );
    write(buffer, left, &value, value_style(model, theme));
    if left.height > 1 {
        let text = match (context.read_rate(), context.write_rate()) {
            (Some(read), Some(write)) => format!(
                "R {}  ·  W {}",
                format_rate(read.value()),
                format_rate(write.value())
            ),
            (Some(read), None) => format!("R {}", format_rate(read.value())),
            (None, Some(write)) => format!("W {}", format_rate(write.value())),
            _ => String::new(),
        };
        write(buffer, line(left, 1), &text, context_style(model, theme));
    }
    if left.height > 2 {
        let fraction = context
            .used_space()
            .zip(context.total_space())
            .map(|(used, total)| used.value() / total.value().max(1.0))
            .or_else(|| model.visual().primary().map(|value| value.value()));
        render_meter(
            line(left, 2),
            fraction.unwrap_or(0.0),
            theme.chart_primary,
            theme,
            buffer,
        );
    }
    if left.height > 3 {
        let text = context.latency().map_or_else(String::new, |value| {
            format!("latency {:.0} ms", value.value() * 1000.0)
        });
        write(buffer, line(left, 3), &text, context_style(model, theme));
    }
    if preset != DashboardPreset::Compact
        && graph.width >= MIN_GRAPH_WIDTH
        && let Some((read_area, write_area, timestamp_area)) = dual_graph_areas(graph)
    {
        if let Some(sparkline) = sparkline(model, BlockId::SecondarySparkline) {
            render_vital_line(
                sparkline,
                read_area,
                0.40,
                theme.chart_primary,
                theme,
                buffer,
            );
        }
        if let Some(sparkline) = sparkline(model, BlockId::TertiarySparkline) {
            render_vital_line(
                sparkline,
                write_area,
                0.40,
                theme.chart_secondary,
                theme,
                buffer,
            );
            render_window(sparkline, timestamp_area, theme, buffer);
        }
    }
}

fn render_header(
    title: &str,
    model: WidgetRenderModel,
    area: Rect,
    theme: &Theme,
    buffer: &mut Buffer,
) {
    let marker = match model.state() {
        WidgetState::Active => "",
        WidgetState::Warning => " !",
        WidgetState::Critical => " ×",
        WidgetState::Unavailable => "",
        _ => "",
    };
    write(
        buffer,
        Rect::new(area.x, area.y, area.width, 1),
        &format!("{title}{marker}"),
        header_style(model, theme),
    );
}

fn render_unavailable(area: Rect, model: WidgetRenderModel, theme: &Theme, buffer: &mut Buffer) {
    write(buffer, area, "—", unavailable_style(model, theme));
}

fn body(area: Rect) -> Rect {
    Rect::new(
        area.x,
        area.y.saturating_add(1),
        area.width,
        area.height.saturating_sub(1),
    )
}

fn split(area: Rect, preset: DashboardPreset) -> (Rect, Rect) {
    if preset == DashboardPreset::Compact || area.width < 24 {
        return (area, Rect::new(area.right(), area.y, 0, area.height));
    }
    let graph_width = (area.width.saturating_mul(GRAPH_RATIO) / 100)
        .max(MIN_GRAPH_WIDTH)
        .min(area.width.saturating_sub(GAP + 8));
    let left_width = area.width.saturating_sub(graph_width).saturating_sub(GAP);
    (
        Rect::new(area.x, area.y, left_width, area.height),
        Rect::new(
            area.x.saturating_add(left_width + GAP),
            area.y,
            graph_width,
            area.height,
        ),
    )
}

fn line(area: Rect, offset: u16) -> Rect {
    Rect::new(area.x, area.y.saturating_add(offset), area.width, 1)
}

fn render_meter(
    area: Rect,
    value: f64,
    color: ratatui::style::Color,
    theme: &Theme,
    buffer: &mut Buffer,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let value = value.clamp(0.0, 1.0);
    for column in 0..area.width {
        let position = (f64::from(column) + 0.5) / f64::from(area.width);
        let (character, foreground) = if position <= value {
            ('━', color)
        } else {
            ('·', theme.border)
        };
        buffer[(area.x + column, area.y)]
            .set_char(character)
            .set_style(Style::default().fg(foreground).bg(theme.surface));
    }
}

fn context_block(model: WidgetRenderModel, id: BlockId) -> Option<RenderBlock> {
    model.block(id)
}

fn sparkline(model: WidgetRenderModel, id: BlockId) -> Option<SparklineModel<CHART_POINTS>> {
    model.blocks().find_map(|block| match block {
        RenderBlock::Chart(chart) if chart.metadata().id() == id => match chart.content() {
            ChartContent::Sparkline(sparkline) => Some(sparkline),
            _ => None,
        },
        _ => None,
    })
}

fn render_window(
    sparkline: SparklineModel<CHART_POINTS>,
    area: Rect,
    theme: &Theme,
    buffer: &mut Buffer,
) {
    if let Some(last) = sparkline.points().last() {
        let seconds = last.sample().offset().as_secs();
        if seconds > 0 && area.width >= 5 && area.height > 0 {
            write(
                buffer,
                Rect::new(
                    area.right().saturating_sub(5),
                    area.bottom().saturating_sub(1),
                    5,
                    1,
                ),
                &format!("{seconds}s"),
                Style::default().fg(theme.text_muted).bg(theme.surface),
            );
        }
    }
}

fn dual_graph_areas(graph: Rect) -> Option<(Rect, Rect, Rect)> {
    if graph.height < 3 {
        return None;
    }
    let data_height = graph.height.saturating_sub(1);
    let first_height = (data_height / 2).max(1);
    let second_height = data_height.saturating_sub(first_height);
    (second_height > 0).then(|| {
        (
            Rect::new(graph.x, graph.y, graph.width, first_height),
            Rect::new(
                graph.x,
                graph.y.saturating_add(first_height),
                graph.width,
                second_height,
            ),
            Rect::new(graph.x, graph.bottom().saturating_sub(1), graph.width, 1),
        )
    })
}

fn render_vital_line(
    sparkline: SparklineModel<CHART_POINTS>,
    area: Rect,
    alpha: f64,
    color: ratatui::style::Color,
    theme: &Theme,
    buffer: &mut Buffer,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let mut values = [None; CHART_POINTS];
    let mut count = 0;
    for point in sparkline.points().iter() {
        if count == CHART_POINTS {
            break;
        }
        values[count] = point.sample().usable_value();
        count += 1;
    }
    if count == 0 {
        return;
    }
    let low = sparkline.stats().minimum().unwrap_or(0.0);
    let high = sparkline.stats().maximum().unwrap_or(1.0);
    let span = (high - low).max(0.08);
    let margin = (span * 0.18).max(0.015);
    let lower = (low - margin).max(0.0);
    let upper = (high + margin).min(1.0).max(lower + 0.08);
    let pixel_width = usize::from(area.width).saturating_mul(2);
    let pixel_height = usize::from(area.height).saturating_mul(4);
    if pixel_width == 0
        || pixel_height == 0
        || usize::from(area.width) * usize::from(area.height) > MAX_BRAILLE_CELLS
    {
        render_fallback_line(values, count, area, lower, upper, color, theme, buffer);
        return;
    }
    let mut smooth = [None; CHART_POINTS];
    let mut previous = None;
    for index in 0..count {
        let Some(raw) = values[index] else { continue };
        let mut value = previous.map_or(raw, |last: f64| last + alpha * (raw - last));
        if (raw - value).abs() > span * 0.18 {
            value += (raw - value) * 0.45;
        }
        if index + 1 == count {
            value += (raw - value) * 0.70;
        }
        smooth[index] = Some(value.clamp(lower, upper));
        previous = smooth[index];
    }
    let mut cells = [0u8; MAX_BRAILLE_CELLS];
    let mut previous_point = None;
    for (index, value) in smooth.iter().take(count).enumerate() {
        let Some(value) = *value else { continue };
        let x = if count <= 1 {
            0
        } else {
            index * pixel_width.saturating_sub(1) / (count - 1)
        };
        let normalized = ((value - lower) / (upper - lower)).clamp(0.0, 1.0);
        let y = pixel_height.saturating_sub(1).saturating_sub(
            (normalized * (pixel_height.saturating_sub(1) as f64)).round() as usize,
        );
        let point = (x, y);
        if let Some(previous_point) = previous_point {
            draw_segment(&mut cells, area, previous_point, point);
        } else {
            set_dot(&mut cells, area, x, y);
        }
        previous_point = Some(point);
    }
    let style = Style::default().fg(color).bg(theme.surface);
    for row in 0..area.height {
        for column in 0..area.width {
            let mask = cells[usize::from(row) * usize::from(area.width) + usize::from(column)];
            if mask != 0 {
                let character = char::from_u32(0x2800 + u32::from(mask)).unwrap_or('·');
                buffer.set_string(area.x + column, area.y + row, character.to_string(), style);
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn render_fallback_line(
    values: [Option<f64>; CHART_POINTS],
    count: usize,
    area: Rect,
    lower: f64,
    upper: f64,
    color: ratatui::style::Color,
    theme: &Theme,
    buffer: &mut Buffer,
) {
    let style = Style::default().fg(color).bg(theme.surface);
    let mut previous = None;
    for column in 0..area.width {
        let index = if area.width <= 1 {
            0
        } else {
            usize::from(column) * count.saturating_sub(1) / usize::from(area.width - 1)
        };
        let Some(value) = values.get(index).copied().flatten() else {
            continue;
        };
        let level = ((value - lower) / (upper - lower)).clamp(0.0, 1.0);
        let current = (level * 3.0).round() as u8;
        let glyph = match (previous, current) {
            (Some(a), b) if b > a => '╲',
            (Some(a), b) if b < a => '╱',
            (Some(_), _) => '─',
            _ => '·',
        };
        buffer.set_string(
            area.x + column,
            area.y + area.height / 2,
            glyph.to_string(),
            style,
        );
        previous = Some(current);
    }
}

fn draw_segment(cells: &mut [u8], area: Rect, start: (usize, usize), end: (usize, usize)) {
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
        set_dot(cells, area, x as usize, y as usize);
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

fn set_dot(cells: &mut [u8], area: Rect, x: usize, y: usize) {
    let pixel_width = usize::from(area.width).saturating_mul(2);
    let pixel_height = usize::from(area.height).saturating_mul(4);
    if x >= pixel_width || y >= pixel_height {
        return;
    }
    let bit = match (x % 2, y % 4) {
        (0, 0) => 1,
        (0, 1) => 2,
        (0, 2) => 4,
        (0, 3) => 64,
        (1, 0) => 8,
        (1, 1) => 16,
        (1, 2) => 32,
        (1, 3) => 128,
        _ => return,
    };
    let index = (y / 4) * usize::from(area.width) + (x / 2);
    if let Some(cell) = cells.get_mut(index) {
        *cell |= bit;
    }
}

fn value_style(model: WidgetRenderModel, theme: &Theme) -> Style {
    value_style_with(model, theme, theme.text)
}

fn value_style_with(
    model: WidgetRenderModel,
    theme: &Theme,
    color: ratatui::style::Color,
) -> Style {
    let mut style = Style::default()
        .fg(color)
        .bg(theme.surface)
        .add_modifier(Modifier::BOLD);
    if model.is_focused() {
        style = style
            .fg(theme.selection_text)
            .bg(theme.selection_background);
    }
    if model.state() == WidgetState::Warning {
        style = style.fg(theme.warning);
    }
    if model.state() == WidgetState::Critical {
        style = style.fg(theme.danger);
    }
    style
}

fn header_style(model: WidgetRenderModel, theme: &Theme) -> Style {
    let color = match model.state() {
        WidgetState::Warning => theme.warning,
        WidgetState::Critical => theme.danger,
        WidgetState::Active => theme.info,
        _ => theme.text_muted,
    };
    Style::default().fg(color).bg(theme.surface)
}

fn context_style(model: WidgetRenderModel, theme: &Theme) -> Style {
    let color = if model.state() == WidgetState::Critical {
        theme.danger
    } else {
        theme.text_muted
    };
    Style::default().fg(color).bg(theme.surface)
}

fn state_style(model: WidgetRenderModel, theme: &Theme) -> Style {
    let color = match model.state() {
        WidgetState::Critical => theme.danger,
        WidgetState::Warning => theme.warning,
        _ => theme.text_muted,
    };
    Style::default().fg(color).bg(theme.surface)
}

fn unavailable_style(_model: WidgetRenderModel, theme: &Theme) -> Style {
    Style::default()
        .fg(theme.text_muted)
        .bg(theme.surface)
        .add_modifier(Modifier::DIM)
}

fn format_percent(value: f64) -> String {
    format!("{:.0}%", value * 100.0)
}

fn format_temperature(value: f64) -> String {
    format!("{value:.0}°C")
}

fn format_frequency(value: f64) -> String {
    if value >= 1.0e9 {
        format!("{:.1} GHz", value / 1.0e9)
    } else if value >= 1.0e6 {
        format!("{:.0} MHz", value / 1.0e6)
    } else {
        format!("{:.0} Hz", value)
    }
}

fn format_bytes(value: f64) -> String {
    let absolute = value.abs();
    if absolute >= 1024.0 * 1024.0 * 1024.0 {
        format!("{:.1} GB", value / 1024.0 / 1024.0 / 1024.0)
    } else if absolute >= 1024.0 * 1024.0 {
        format!("{:.1} MB", value / 1024.0 / 1024.0)
    } else if absolute >= 1024.0 {
        format!("{:.1} KB", value / 1024.0)
    } else {
        format!("{value:.0} B")
    }
}

fn format_compact_capacity(used: f64, total: f64) -> String {
    const GIBIBYTE: f64 = 1024.0 * 1024.0 * 1024.0;
    const MEBIBYTE: f64 = 1024.0 * 1024.0;
    if total >= GIBIBYTE {
        format!("{:.1} / {:.0} GB", used / GIBIBYTE, total / GIBIBYTE)
    } else if total >= MEBIBYTE {
        format!("{:.0} / {:.0} MB", used / MEBIBYTE, total / MEBIBYTE)
    } else {
        format!("{} / {}", format_bytes(used), format_bytes(total))
    }
}

fn format_rate(value: f64) -> String {
    format!("{}/s", format_bytes(value))
}

fn to_rect(bounds: DashboardBounds, buffer: Rect) -> Result<Rect, TerminalRenderError> {
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
    if x < buffer.x || y < buffer.y || right > buffer.right() || bottom > buffer.bottom() {
        return Err(TerminalRenderError::BufferTooSmall);
    }
    Ok(Rect::new(x, y, width, height))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dual_graphs_reserve_the_window_inside_their_bounds() {
        for height in 3..=10 {
            let graph = Rect::new(4, 6, 18, height);
            let (first, second, timestamp) = dual_graph_areas(graph).unwrap();
            assert!(first.x >= graph.x && first.right() <= graph.right());
            assert!(second.x >= graph.x && second.right() <= graph.right());
            assert!(first.y >= graph.y && second.bottom() <= timestamp.y);
            assert_eq!(timestamp.bottom(), graph.bottom());
            assert_eq!(first.bottom(), second.y);
        }
    }

    #[test]
    fn compact_gpu_capacity_avoids_unnecessary_precision() {
        let gibibyte = 1024.0 * 1024.0 * 1024.0;
        assert_eq!(
            format_compact_capacity(0.7 * gibibyte, 8.0 * gibibyte),
            "0.7 / 8 GB"
        );
    }
}
