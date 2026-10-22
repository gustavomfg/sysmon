use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
};

use crate::theme::Theme;

use super::super::chart::SparklineModel;
use super::super::inventory::select_primary_gpu;
use super::super::metrics::CHART_POINTS;
use super::super::{
    BlockId, ChartContent, DashboardBounds, DashboardPreset, HardwareInventory, RenderBlock,
    WidgetRenderModel, WidgetState,
};
use super::blocks::write;
use super::history_chart::{self, Scale};
use super::instruments::{capacity_meter, format_bytes, format_rate, short_rate};
use super::renderer::TerminalRenderError;

// A medium-weight pair: more legible than a thin rule, without returning to
// the visually heavy full blocks used by the original meters.
#[cfg(test)]
const METER_FILL_GLYPH: char = '━';
#[cfg(test)]
const METER_TRACK_GLYPH: char = '─';

pub(super) fn render_with_inventory(
    model: WidgetRenderModel,
    preset: DashboardPreset,
    bounds: DashboardBounds,
    theme: &Theme,
    buffer: &mut Buffer,
    inventory: Option<&HardwareInventory>,
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
            if let Some(inventory) = inventory.filter(|inventory| !inventory.gpus.is_empty()) {
                render_gpu_inventory(model, inventory, area, theme, buffer);
            } else {
                render_gpu(model, preset, area, theme, buffer);
            }
            Ok(())
        }
        super::super::DashboardWidgetId::Network => {
            render_network(model, area, theme, buffer, inventory);
            Ok(())
        }
        super::super::DashboardWidgetId::Storage => {
            if let Some(inventory) = inventory.filter(|inventory| !inventory.storages.is_empty()) {
                render_storage_inventory(model, inventory, area, theme, buffer);
            } else {
                render_storage(model, preset, area, theme, buffer);
            }
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
    render_header("MEMORY", model, area, theme, buffer);
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
    let secondary = memory_context_line(&context, body.width);
    if body.height == 1 {
        write(buffer, line(body, 0), &value, value_style(model, theme));
        return;
    }

    write(buffer, line(body, 0), &value, value_style(model, theme));
    let occupancy_width = u16::try_from(occupancy.chars().count()).unwrap_or(body.width);
    if occupancy_width < body.width {
        buffer.set_stringn(
            body.right().saturating_sub(occupancy_width),
            body.y,
            occupancy,
            usize::from(occupancy_width),
            value_style(model, theme),
        );
    }
    let inline_context = body.height <= 3
        && !secondary.is_empty()
        && secondary.chars().count() <= usize::from(area.width.saturating_sub(8));
    if body.height > 1 && !secondary.is_empty() {
        let context_area = if inline_context {
            Rect::new(area.x + 8, area.y, area.width - 8, 1)
        } else {
            line(body, 1)
        };
        write_rich_context(
            buffer,
            context_area,
            &secondary,
            context_style(model, theme),
            theme,
            value_style(model, theme),
        );
    }
    let mut offset = if secondary.is_empty() || inline_context {
        1
    } else {
        2
    };
    if body.height >= offset + 2 {
        let ratio = used
            .zip(total)
            .filter(|(_, total)| total.value() > 0.0)
            .map(|(used, total)| used.value() / total.value());
        capacity_meter(
            line(body, offset),
            ratio,
            super::super::vital::metric_color(model.state(), theme),
            theme,
            buffer,
        );
        offset += 1;
    }
    render_percent_history(model, history_chart::region(body, offset), theme, buffer);
}

fn memory_context_line(context: &super::super::MemoryContextBlock, width: u16) -> String {
    let available = context
        .available()
        .map(|value| format!("{} available", format_bytes(value.value())));
    let pressure = context
        .pressure()
        .map(|value| format!("{} pressure", format_percent(value.value())));
    let swap = context
        .swap()
        .filter(|value| value.value() > 0.0)
        .map(|value| format!("swap {}", format_bytes(value.value())));
    let cache = context
        .cache()
        .map(|value| format!("cache {}", format_bytes(value.value())));

    let join = |left: Option<&str>, right: Option<&str>| match (left, right) {
        (Some(left), Some(right)) => Some(format!("{left}  ·  {right}")),
        (Some(left), None) => Some(left.to_owned()),
        (None, Some(right)) => Some(right.to_owned()),
        (None, None) => None,
    };
    let full = match (available.as_deref(), pressure.as_deref(), cache.as_deref()) {
        (Some(available), Some(pressure), Some(cache)) => {
            Some(format!("{available}  ·  {pressure}  ·  {cache}"))
        }
        _ => None,
    };
    // Active swap is a diagnostic signal, not just another context token.
    // Prefer it over a long availability label so the instrument answers the
    // more urgent question first while remaining compact.
    let candidates = if swap.is_some() {
        [
            join(available.as_deref(), swap.as_deref()),
            join(swap.as_deref(), pressure.as_deref()),
            join(swap.as_deref(), cache.as_deref()),
            full,
            join(available.as_deref(), pressure.as_deref()),
            available.clone(),
            pressure.clone(),
            swap.clone(),
        ]
    } else {
        [
            full,
            join(available.as_deref(), pressure.as_deref()),
            join(available.as_deref(), cache.as_deref()),
            available.clone(),
            pressure.clone(),
            cache.clone(),
            None,
            None,
        ]
    };
    candidates
        .into_iter()
        .flatten()
        .find(|candidate| candidate.chars().count() <= usize::from(width))
        .unwrap_or_default()
}

fn render_gpu(
    model: WidgetRenderModel,
    _preset: DashboardPreset,
    area: Rect,
    theme: &Theme,
    buffer: &mut Buffer,
) {
    render_header("GPU", model, area, theme, buffer);
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
    let value = model
        .visual()
        .primary()
        .map_or_else(|| "—".to_owned(), |value| format_percent(value.value()));
    write(buffer, line(body, 0), &value, value_style(model, theme));
    if model.visual().primary().is_none() {
        if body.height > 1 {
            write(
                buffer,
                line(body, 1),
                "telemetry unavailable",
                context_style(model, theme),
            );
        }
        return;
    }
    let mut details = Vec::new();
    if let Some((used, total)) = context.vram_used().zip(context.vram_total()) {
        details.push(format_compact_capacity(used.value(), total.value()));
    }
    if let Some(value) = context.temperature() {
        details.push(format_temperature(value.value()));
    }
    let mut clock = Vec::new();
    if let Some(value) = context.frequency() {
        clock.push(format_frequency(value.value()));
    }
    if let Some(value) = context.power() {
        clock.push(format!("{:.0} W", value.value()));
    }
    let clock = clock.join(" · ");
    let inline_clock = body.height <= 4
        && !clock.is_empty()
        && value.chars().count() + 2 + clock.chars().count() <= usize::from(body.width);
    if inline_clock {
        let start = value.chars().count() as u16 + 2;
        write_rich_context(
            buffer,
            Rect::new(body.x + start, body.y, body.width - start, 1),
            &clock,
            context_style(model, theme),
            theme,
            value_style(model, theme),
        );
    } else if !clock.is_empty() {
        details.push(clock);
    }
    let mut offset = write_context_rows(
        body,
        &details.join(" · "),
        context_style(model, theme),
        theme,
        value_style(model, theme),
        buffer,
    );
    if body.height >= offset + 3
        && let Some((used, total)) = context
            .vram_used()
            .zip(context.vram_total())
            .filter(|(_, total)| total.value() > 0.0)
    {
        capacity_meter(
            line(body, offset),
            Some(used.value() / total.value()),
            super::super::vital::heat_color(used.value() / total.value(), theme),
            theme,
            buffer,
        );
        offset += 1;
    }
    render_percent_history(model, history_chart::region(body, offset), theme, buffer);
}

fn render_network(
    model: WidgetRenderModel,
    area: Rect,
    theme: &Theme,
    buffer: &mut Buffer,
    inventory: Option<&HardwareInventory>,
) {
    let title = inventory
        .and_then(network_inventory_label)
        .map_or_else(|| "NETWORK".to_owned(), |label| format!("NETWORK  {label}"));
    render_header(&title, model, area, theme, buffer);
    let body = body(area);
    if body.height == 0 {
        return;
    }
    let Some(RenderBlock::NetworkContext(context)) = context_block(model, BlockId::NetworkContext)
    else {
        render_unavailable(body, model, theme, buffer);
        return;
    };
    let download = context
        .download()
        .map_or_else(|| "—".to_owned(), |value| format_rate(value.value()));
    let upload = context
        .upload()
        .map_or_else(|| "—".to_owned(), |value| format_rate(value.value()));
    let download_style = value_style_with(model, theme, theme.info);
    let upload_style = value_style_with(model, theme, theme.primary);
    let rate_style = value_style(model, theme);
    write_flow_value(body, 0, "↓", &download, download_style, rate_style, buffer);
    if body.height > 1 {
        write_flow_value(body, 1, "↑", &upload, upload_style, rate_style, buffer);
    }
    let mut graph_offset = 2;
    if body.height >= 7 {
        let totals = context
            .download_total()
            .zip(context.upload_total())
            .map_or_else(String::new, |(download, upload)| {
                format!(
                    "Σ {}  ·  {}",
                    format_bytes(download.value()),
                    format_bytes(upload.value())
                )
            });
        if !totals.is_empty() {
            write_rich_context(
                buffer,
                line(body, graph_offset),
                &totals,
                context_style(model, theme),
                theme,
                value_style(model, theme),
            );
            graph_offset += 1;
        }
    }
    let graph = Rect::new(
        body.x,
        body.y.saturating_add(graph_offset),
        body.width,
        body.height.saturating_sub(graph_offset),
    );
    let series = [
        (sparkline(model, BlockId::PrimarySparkline), download_style),
        (sparkline(model, BlockId::SecondarySparkline), upload_style),
    ];
    for (row, (history, _)) in series.iter().enumerate() {
        if let Some(peak) = history.as_ref().and_then(history_chart::recent_peak) {
            let text = format!("pk {}", short_rate(peak));
            let width = text.chars().count() as u16;
            let value_width = if row == 0 {
                download.chars().count()
            } else {
                upload.chars().count()
            } as u16;
            if body.width >= width + value_width + 5 && row < usize::from(body.height) {
                write(
                    buffer,
                    Rect::new(body.right() - width, body.y + row as u16, width, 1),
                    &text,
                    context_style(model, theme),
                );
            }
        }
    }
    history_chart::flow_instrument(&series, graph, ["↓ RECEIVE", "↑ SEND"], theme, buffer);
}

fn write_flow_value(
    body: Rect,
    offset: u16,
    arrow: &str,
    value: &str,
    arrow_style: Style,
    value_style: Style,
    buffer: &mut Buffer,
) {
    if offset >= body.height || body.width == 0 {
        return;
    }
    let row = line(body, offset);
    write(buffer, row, arrow, arrow_style);
    let value_x = row.x.saturating_add(2);
    if value_x < row.right() {
        write(
            buffer,
            Rect::new(value_x, row.y, row.right().saturating_sub(value_x), 1),
            value,
            value_style,
        );
    }
}

fn network_inventory_label(inventory: &HardwareInventory) -> Option<String> {
    let mut count = 0usize;
    let mut first = None;
    for network in inventory
        .networks
        .iter()
        .filter(|network| !is_loopback(&network.name))
    {
        count += 1;
        first = first.or(Some(network.name.as_str()));
    }
    match (count, first) {
        (0, _) => Some("no network interface".to_owned()),
        (1, Some(name)) => Some(name.to_owned()),
        (count, _) => Some(format!("Σ {count} interfaces")),
    }
}

fn render_storage(
    model: WidgetRenderModel,
    _preset: DashboardPreset,
    area: Rect,
    theme: &Theme,
    buffer: &mut Buffer,
) {
    render_header("STORAGE", model, area, theme, buffer);
    let body = body(area);
    if body.height == 0 {
        return;
    }
    let Some(RenderBlock::StorageContext(context)) = context_block(model, BlockId::StorageContext)
    else {
        render_unavailable(body, model, theme, buffer);
        return;
    };
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
    write(buffer, line(body, 0), &value, value_style(model, theme));
    if body.height > 1 {
        let fraction = context
            .used_space()
            .zip(context.total_space())
            .map(|(used, total)| used.value() / total.value().max(1.0))
            .or_else(|| model.visual().primary().map(|value| value.value()));
        render_segmented_meter(
            line(body, 1),
            fraction.unwrap_or(0.0),
            super::super::vital::metric_color(model.state(), theme),
            theme,
            buffer,
        );
    }
    if body.height > 2 {
        let text = storage_context_activity(&context);
        write_rich_context(
            buffer,
            line(body, 2),
            &text,
            context_style(model, theme),
            theme,
            value_style(model, theme),
        );
    }
    if body.height > 3 {
        let text = context.latency().map_or_else(String::new, |value| {
            format!("latency {:.0} ms", value.value() * 1000.0)
        });
        write_rich_context(
            buffer,
            line(body, 3),
            &text,
            context_style(model, theme),
            theme,
            value_style(model, theme),
        );
    }
    let offset = if context.latency().is_some() { 4 } else { 3 };
    render_storage_history(model, history_chart::region(body, offset), theme, buffer);
}

fn render_storage_history(
    model: WidgetRenderModel,
    area: Rect,
    theme: &Theme,
    buffer: &mut Buffer,
) {
    let series = [
        (
            sparkline(model, BlockId::SecondarySparkline),
            Style::default().fg(theme.info).bg(theme.surface),
        ),
        (
            sparkline(model, BlockId::TertiarySparkline),
            Style::default().fg(theme.primary).bg(theme.surface),
        ),
    ];
    history_chart::flow_instrument(&series, area, ["R READ", "W WRITE"], theme, buffer);
}

fn storage_context_activity(context: &super::super::StorageContextBlock) -> String {
    match (context.read_rate(), context.write_rate()) {
        (Some(read), Some(write)) => format!(
            "R {}  ·  W {}",
            format_rate(read.value()),
            format_rate(write.value())
        ),
        (Some(read), None) => format!("R {}", format_rate(read.value())),
        (None, Some(write)) => format!("W {}", format_rate(write.value())),
        _ => String::new(),
    }
}

fn render_gpu_inventory(
    model: WidgetRenderModel,
    inventory: &HardwareInventory,
    area: Rect,
    theme: &Theme,
    buffer: &mut Buffer,
) {
    clear_surface(area, theme, buffer);
    let primary_index = select_primary_gpu(&inventory.gpus);
    let primary = &inventory.gpus[primary_index];
    let primary_name = gpu_display_name(&primary.name);
    let title = if inventory.gpus.len() > 1 {
        format!(
            "GPU  {primary_name}  ({}/{})",
            primary_index + 1,
            inventory.gpus.len()
        )
    } else {
        format!("GPU  {primary_name}")
    };
    render_header(&title, model, area, theme, buffer);
    let body = body(area);
    if body.height == 0 {
        return;
    }
    let name = gpu_display_name(&primary.name);
    if body.height == 1 {
        let status = if gpu_has_usage(primary) {
            format!("{name}  {}", gpu_usage_text(primary))
        } else {
            format!("{name}  ·  {}", gpu_unavailable_label(primary))
        };
        write(
            buffer,
            line(body, 0),
            &status,
            context_style_for_theme(theme),
        );
    } else if gpu_has_usage(primary) {
        write(
            buffer,
            line(body, 0),
            &gpu_usage_text(primary),
            value_style_for_inventory(primary.availability, theme),
        );
        let frequency = primary.frequency_hz.map(format_frequency);
        let inline_clock = body.height <= 4
            && frequency.as_ref().is_some_and(|frequency| {
                gpu_usage_text(primary).chars().count() + 2 + frequency.chars().count()
                    <= usize::from(body.width)
            });
        if inline_clock {
            let start = gpu_usage_text(primary).chars().count() as u16 + 2;
            write_rich_context(
                buffer,
                Rect::new(body.x + start, body.y, body.width - start, 1),
                frequency.as_deref().unwrap_or_default(),
                context_style_for_theme(theme),
                theme,
                context_value_style_for_theme(theme),
            );
        }
        let mut offset = write_context_rows(
            body,
            &gpu_context_with_frequency(primary, !inline_clock),
            context_style_for_theme(theme),
            theme,
            context_value_style_for_theme(theme),
            buffer,
        );
        if body.height >= offset + 3
            && let Some((used, total)) = primary
                .vram_used_bytes
                .zip(primary.vram_total_bytes)
                .filter(|(_, total)| *total > 0.0)
        {
            capacity_meter(
                line(body, offset),
                Some(used / total),
                super::super::vital::heat_color(used / total, theme),
                theme,
                buffer,
            );
            offset += 1;
        }
        render_percent_history(model, history_chart::region(body, offset), theme, buffer);
    } else {
        write(
            buffer,
            line(body, 0),
            gpu_unavailable_label(primary),
            context_style_for_theme(theme),
        );
        if body.height > 1 {
            let context = gpu_context(primary);
            if !context.is_empty() {
                write_rich_context(
                    buffer,
                    line(body, 1),
                    &context,
                    context_style_for_theme(theme),
                    theme,
                    context_value_style_for_theme(theme),
                );
            }
        }
    }
}

// Preserve complete context tokens before assigning remaining rows to history.
fn write_context_rows(
    body: Rect,
    context: &str,
    style: Style,
    theme: &Theme,
    value_style: Style,
    buffer: &mut Buffer,
) -> u16 {
    let mut row = 1;
    let mut text = String::new();
    for token in context.split(" · ").filter(|token| !token.is_empty()) {
        if !text.is_empty()
            && text.chars().count() + 3 + token.chars().count() > usize::from(body.width)
        {
            if row < body.height {
                write_rich_context(buffer, line(body, row), &text, style, theme, value_style);
            }
            row += 1;
            text.clear();
        }
        if !text.is_empty() {
            text.push_str(" · ");
        }
        text.push_str(token);
    }
    if !text.is_empty() && row < body.height {
        write_rich_context(buffer, line(body, row), &text, style, theme, value_style);
        row += 1;
    }
    row
}

fn gpu_has_usage(gpu: &super::super::GpuDevice) -> bool {
    gpu.utilization_percent.is_some()
}

fn gpu_unavailable_label(gpu: &super::super::GpuDevice) -> &'static str {
    if gpu.name.to_ascii_lowercase().contains("nvidia") {
        "driver unavailable"
    } else {
        "telemetry unavailable"
    }
}

fn gpu_display_name(name: &str) -> String {
    let lower = name.to_ascii_lowercase();
    if lower.contains("nvidia") && lower.contains("4060") {
        "RTX 4060".to_owned()
    } else if lower.contains("amd") && lower.contains("radeon") {
        "AMD Radeon".to_owned()
    } else {
        name.split(" · ").next().unwrap_or(name).to_owned()
    }
}

fn gpu_usage_text(gpu: &super::super::GpuDevice) -> String {
    gpu.utilization_percent
        .map_or_else(|| "—".to_owned(), |value| format!("{value:.0}%"))
}

fn gpu_context(gpu: &super::super::GpuDevice) -> String {
    gpu_context_with_frequency(gpu, true)
}

fn gpu_context_with_frequency(gpu: &super::super::GpuDevice, include_frequency: bool) -> String {
    let mut values = Vec::new();
    if let Some((used, total)) = gpu.vram_used_bytes.zip(gpu.vram_total_bytes) {
        values.push(format_compact_capacity(used, total));
    }
    if let Some(temperature) = gpu.temperature_celsius {
        values.push(format!("{temperature:.0}°C"));
    }
    if include_frequency && let Some(frequency) = gpu.frequency_hz {
        values.push(format_frequency(frequency));
    }
    values.join(" · ")
}

fn render_segmented_meter(
    area: Rect,
    value: f64,
    fill: ratatui::style::Color,
    theme: &Theme,
    buffer: &mut Buffer,
) {
    capacity_meter(area, Some(value), fill, theme, buffer);
}

fn value_style_for_inventory(
    availability: super::super::MetricAvailability,
    theme: &Theme,
) -> Style {
    Style::default()
        .fg(
            if availability == super::super::MetricAvailability::Available {
                theme.text
            } else {
                theme.text_muted
            },
        )
        .bg(theme.surface)
        .add_modifier(Modifier::BOLD)
}

fn is_loopback(name: &str) -> bool {
    name == "lo" || name.starts_with("lo:")
}

fn render_storage_inventory(
    model: WidgetRenderModel,
    inventory: &HardwareInventory,
    area: Rect,
    theme: &Theme,
    buffer: &mut Buffer,
) {
    if inventory.storages.is_empty() {
        return;
    }
    clear_surface(area, theme, buffer);
    let content = body(area);
    buffer.set_style(content, Style::default().bg(theme.surface));
    let title = format!("STORAGE  {} devices", inventory.storages.len());
    render_header(&title, model, area, theme, buffer);
    if content.height == 0 {
        return;
    }
    // Storage is a capacity instrument. Every physical device gets a value
    // row and its own gauge; I/O is a subordinate row only when the panel has
    // enough room. This keeps the inventory readable without turning it into a
    // raw hardware list.
    let device_count = u16::try_from(inventory.storages.len()).unwrap_or(u16::MAX);
    let rows_per_device = if content.height >= device_count.saturating_mul(3) {
        3
    } else if content.height >= device_count.saturating_mul(2) {
        2
    } else {
        1
    };
    let visible = usize::from(content.height / rows_per_device.max(1));
    let name_width = inventory
        .storages
        .iter()
        .map(|storage| storage.name.chars().count())
        .max()
        .unwrap_or(0);
    let capacity_width = inventory
        .storages
        .iter()
        .map(storage_capacity)
        .map(|capacity| capacity.chars().count())
        .max()
        .unwrap_or(0);
    for (index, device) in inventory.storages.iter().take(visible).enumerate() {
        let row = index as u16 * rows_per_device;
        let value = storage_value(device, name_width, capacity_width, content.width);
        write_storage_value(
            buffer,
            line(content, row),
            &value,
            &device.name,
            context_style_for_theme(theme),
            theme,
            context_value_style_for_theme(theme),
        );
        if rows_per_device >= 2 {
            let fraction = device
                .used_bytes
                .zip(device.total_bytes)
                .map_or(0.0, |(used, total)| used / total.max(1.0));
            render_segmented_meter(
                line(content, row.saturating_add(1)),
                fraction,
                super::super::vital::metric_color(
                    super::super::metrics::STORAGE_THRESHOLDS.classify(Some(fraction)),
                    theme,
                ),
                theme,
                buffer,
            );
        }
        if rows_per_device == 3 {
            let activity = storage_activity(device);
            if !activity.is_empty() {
                write_rich_context(
                    buffer,
                    line(content, row.saturating_add(2)),
                    &activity,
                    context_style_for_theme(theme),
                    theme,
                    context_value_style_for_theme(theme),
                );
            }
        }
    }
    if visible < inventory.storages.len() && content.height > 0 {
        write(
            buffer,
            line(content, content.height.saturating_sub(1)),
            &format!("+{} devices", inventory.storages.len() - visible),
            context_style_for_theme(theme),
        );
    } else {
        let used_rows = (inventory.storages.len() as u16).saturating_mul(rows_per_device);
        let mut offset = used_rows;
        if content.height.saturating_sub(offset) >= 4 && inventory.storages.len() > 1 {
            write(
                buffer,
                line(content, offset),
                &format!("I/O {} · R / W", inventory.storages[0].name),
                context_style_for_theme(theme),
            );
            offset += 1;
        }
        render_storage_history(model, history_chart::region(content, offset), theme, buffer);
    }
}

fn storage_activity(device: &super::super::StorageDevice) -> String {
    match (device.read_bytes_per_sec, device.write_bytes_per_sec) {
        (Some(read), Some(write)) => {
            format!("R {}  ·  W {}", format_rate(read), format_rate(write))
        }
        (Some(read), None) => format!("R {}", format_rate(read)),
        (None, Some(write)) => format!("W {}", format_rate(write)),
        (None, None) => String::new(),
    }
}

fn storage_value(
    device: &super::super::StorageDevice,
    name_width: usize,
    capacity_width: usize,
    available_width: u16,
) -> String {
    let capacity = storage_capacity(device);
    let percent = device.used_bytes.zip(device.total_bytes).map_or_else(
        || "—".to_owned(),
        |(used, total)| format_percent(used / total.max(1.0)),
    );
    let required = name_width
        .saturating_add(capacity_width)
        .saturating_add(percent.chars().count())
        .saturating_add(4);
    if required <= usize::from(available_width) {
        format!(
            "{:<name_width$}  {:>capacity_width$}  {:>4}",
            device.name, capacity, percent
        )
    } else {
        format!("{}  {}  {}", device.name, capacity, percent)
    }
}

fn storage_capacity(device: &super::super::StorageDevice) -> String {
    device.used_bytes.zip(device.total_bytes).map_or_else(
        || "—".to_owned(),
        |(used, total)| format!("{} / {}", format_bytes(used), format_bytes(total)),
    )
}

fn context_style_for_theme(theme: &Theme) -> Style {
    Style::default().fg(theme.text_muted).bg(theme.surface)
}

fn context_value_style_for_theme(theme: &Theme) -> Style {
    Style::default()
        .fg(theme.text)
        .bg(theme.surface)
        .add_modifier(Modifier::BOLD)
}

fn write_rich_context(
    buffer: &mut Buffer,
    area: Rect,
    text: &str,
    base_style: Style,
    theme: &Theme,
    value_style: Style,
) {
    write(buffer, area, text, base_style);
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
                    base_style.fg(theme.text_muted),
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

fn write_storage_value(
    buffer: &mut Buffer,
    area: Rect,
    text: &str,
    name: &str,
    base_style: Style,
    theme: &Theme,
    value_style: Style,
) {
    write(buffer, area, text, base_style);
    let name_width = name.chars().count();
    let value_start = name.len().min(text.len());
    write_rich_context(
        buffer,
        Rect::new(
            area.x
                .saturating_add(u16::try_from(name_width).unwrap_or(0)),
            area.y,
            area.width
                .saturating_sub(u16::try_from(name_width).unwrap_or(0)),
            area.height,
        ),
        &text[value_start..],
        base_style,
        theme,
        value_style,
    );
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
        Rect::new(
            area.x.saturating_add(offset),
            area.y,
            area.width.saturating_sub(offset),
            area.height,
        ),
        token,
        style,
    );
}

fn is_context_value(token: &str) -> bool {
    token.chars().any(|character| character.is_ascii_digit())
        || matches!(
            token,
            "B" | "B/s" | "GB" | "MB" | "KB" | "Hz" | "kHz" | "MHz" | "GHz" | "ms" | "s"
        )
}

fn clear_surface(area: Rect, theme: &Theme, buffer: &mut Buffer) {
    buffer.set_style(area, Style::default().bg(theme.surface));
    for y in area.y..area.bottom() {
        for x in area.x..area.right() {
            buffer[(x, y)].set_symbol(" ");
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

fn line(area: Rect, offset: u16) -> Rect {
    Rect::new(area.x, area.y.saturating_add(offset), area.width, 1)
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

fn render_percent_history(
    model: WidgetRenderModel,
    area: Rect,
    theme: &Theme,
    buffer: &mut Buffer,
) {
    history_chart::instrument(
        &[(
            sparkline(model, BlockId::PrimarySparkline),
            Style::default()
                .fg(super::super::vital::metric_color(model.state(), theme))
                .bg(theme.surface),
        )],
        if model.visual().widget_id() == super::super::DashboardWidgetId::Memory {
            Scale::Memory
        } else {
            Scale::Percent
        },
        area,
        theme,
        buffer,
    );
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
    // Focus is carried by the panel ring; the card title echoes its color.
    if model.is_focused() {
        return Style::default()
            .fg(theme.border_focused)
            .bg(theme.surface)
            .add_modifier(Modifier::BOLD);
    }
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

fn format_compact_capacity(used: f64, total: f64) -> String {
    const GIBIBYTE: f64 = 1024.0 * 1024.0 * 1024.0;
    const MEBIBYTE: f64 = 1024.0 * 1024.0;
    if total >= GIBIBYTE {
        let capacity = total / GIBIBYTE;
        let capacity = if (capacity - capacity.round()).abs() < 0.05 {
            format!("{capacity:.0}")
        } else {
            format!("{capacity:.1}")
        };
        format!("{:.1} / {capacity} GiB", used / GIBIBYTE)
    } else if total >= MEBIBYTE {
        format!("{:.0} / {:.0} MiB", used / MEBIBYTE, total / MEBIBYTE)
    } else {
        format!("{} / {}", format_bytes(used), format_bytes(total))
    }
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
    fn compact_gpu_capacity_avoids_unnecessary_precision() {
        let gibibyte = 1024.0 * 1024.0 * 1024.0;
        assert_eq!(
            format_compact_capacity(0.7 * gibibyte, 8.0 * gibibyte),
            "0.7 / 8 GiB"
        );
        assert_eq!(format_compact_capacity(2e9, 8e9), "1.9 / 7.5 GiB");
    }

    #[test]
    fn segmented_meter_is_proportional_and_width_adaptive() {
        let theme = crate::theme::Theme::pulse();
        let mut narrow = Buffer::empty(Rect::new(0, 0, 10, 1));
        render_segmented_meter(
            Rect::new(0, 0, 10, 1),
            0.3,
            theme.chart_primary,
            &theme,
            &mut narrow,
        );
        assert_eq!(
            narrow
                .content
                .iter()
                .filter(|cell| cell.symbol() == METER_FILL_GLYPH.to_string())
                .count(),
            3
        );
        assert_eq!(
            narrow
                .content
                .iter()
                .filter(|cell| cell.symbol() == METER_TRACK_GLYPH.to_string())
                .count(),
            7
        );

        let mut wide = Buffer::empty(Rect::new(0, 0, 20, 1));
        render_segmented_meter(
            Rect::new(0, 0, 20, 1),
            0.3,
            theme.chart_primary,
            &theme,
            &mut wide,
        );
        assert_eq!(
            wide.content
                .iter()
                .filter(|cell| cell.symbol() == METER_FILL_GLYPH.to_string())
                .count(),
            6
        );
    }

    #[test]
    fn occupancy_meter_uses_segmented_track_and_proportional_fill() {
        let theme = crate::theme::Theme::pulse();
        let mut buffer = Buffer::empty(Rect::new(0, 0, 10, 1));
        render_segmented_meter(
            Rect::new(0, 0, 10, 1),
            0.4,
            theme.chart_primary,
            &theme,
            &mut buffer,
        );
        assert_eq!(
            buffer
                .content
                .iter()
                .filter(|cell| cell.symbol() == METER_FILL_GLYPH.to_string())
                .count(),
            4
        );
        assert_eq!(
            buffer
                .content
                .iter()
                .filter(|cell| cell.symbol() == METER_TRACK_GLYPH.to_string())
                .count(),
            6
        );
    }

    #[test]
    fn primary_gpu_selection_prefers_telemetry_and_has_deterministic_fallback() {
        let gpu = |id: &str, name: &str, usage: Option<f64>, complete: bool| {
            super::super::super::GpuDevice {
                id: super::super::super::HardwareId::new(
                    super::super::super::HardwareKind::Gpu,
                    id,
                )
                .unwrap(),
                name: name.to_owned(),
                utilization_percent: usage,
                vram_used_bytes: complete.then_some(1.0),
                vram_total_bytes: complete.then_some(8.0),
                temperature_celsius: complete.then_some(50.0),
                frequency_hz: None,
                power_watts: None,
                availability: super::super::super::MetricAvailability::Available,
            }
        };
        let complete_amd = gpu("amd", "AMD Radeon", Some(40.0), true);
        let nvidia = gpu("nvidia", "NVIDIA GeForce RTX 4060", Some(20.0), false);
        assert_eq!(select_primary_gpu(&[complete_amd, nvidia]), 0);

        let amd_without_telemetry = gpu("amd", "AMD Radeon", None, false);
        let nvidia_without_telemetry = gpu("nvidia", "NVIDIA GeForce RTX 4060", None, false);
        assert_eq!(
            select_primary_gpu(&[amd_without_telemetry, nvidia_without_telemetry]),
            1
        );
    }
}
