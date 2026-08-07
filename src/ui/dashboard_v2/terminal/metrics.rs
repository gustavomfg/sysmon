use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
};

use crate::theme::Theme;

use super::super::chart::SparklineModel;
use super::super::metrics::CHART_POINTS;
use super::super::{
    BlockId, ChartContent, DashboardBounds, DashboardPreset, HardwareInventory, RenderBlock,
    WidgetRenderModel, WidgetState,
};
use super::blocks::write;
use super::renderer::TerminalRenderError;

const MIN_GRAPH_WIDTH: u16 = 10;
const MAX_BRAILLE_CELLS: usize = 4096;
// A medium-weight pair: more legible than a thin rule, without returning to
// the visually heavy full blocks used by the original meters.
const METER_FILL_GLYPH: char = '▰';
const METER_TRACK_GLYPH: char = '▱';

pub(super) fn render(
    model: WidgetRenderModel,
    preset: DashboardPreset,
    bounds: DashboardBounds,
    theme: &Theme,
    buffer: &mut Buffer,
) -> Result<(), TerminalRenderError> {
    render_with_inventory(model, preset, bounds, theme, buffer, None)
}

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
    render_header("▦ MEMORY", model, area, theme, buffer);
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
    let fraction = used
        .zip(total)
        .map(|(used, total)| used.value() / total.value().max(1.0))
        .or_else(|| model.visual().primary().map(|value| value.value()));

    let secondary = memory_context_line(&context, body.width);
    if body.height == 1 {
        write(buffer, line(body, 0), &value, value_style(model, theme));
        return;
    }

    // Memory is an occupancy instrument, not a temporal signal. The value,
    // occupancy and gauge share one strict grid: one reading row, one
    // instrument row and one context row. No history is read here.
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
    if body.height > 1 {
        render_segmented_meter(
            line(body, 1),
            fraction.unwrap_or(0.0),
            theme.chart_primary,
            theme,
            buffer,
        );
    }
    if body.height > 2 && !secondary.is_empty() {
        write_rich_context(
            buffer,
            line(body, 2),
            &secondary,
            context_style(model, theme),
            theme,
            value_style(model, theme),
        );
    }
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
    let candidates = [
        full,
        join(available.as_deref(), pressure.as_deref()),
        join(available.as_deref(), swap.as_deref()),
        join(pressure.as_deref(), swap.as_deref()),
        available.clone(),
        pressure.clone(),
        swap.clone(),
        cache.clone(),
    ];
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
    let value = model
        .visual()
        .primary()
        .map_or_else(|| "—".to_owned(), |value| format_percent(value.value()));
    write(buffer, line(body, 0), &value, value_style(model, theme));
    if body.height > 1 {
        let vram = context
            .vram_used()
            .zip(context.vram_total())
            .map(|(used, total)| format_compact_capacity(used.value(), total.value()));
        let temperature = context
            .temperature()
            .map(|value| format_temperature(value.value()));
        let text = match (vram, temperature) {
            (Some(vram), Some(temperature)) => format!("{vram}  ·  {temperature}"),
            (Some(vram), None) => vram,
            (None, Some(temperature)) => temperature,
            (None, None) => String::new(),
        };
        let usage = model.visual().primary().map(|value| value.value());
        if let Some(usage) = usage {
            render_segmented_meter(line(body, 1), usage, theme.chart_primary, theme, buffer);
            if body.height > 2 {
                write_rich_context(
                    buffer,
                    line(body, 2),
                    &text,
                    context_style(model, theme),
                    theme,
                    value_style(model, theme),
                );
            }
        } else {
            write(
                buffer,
                line(body, 1),
                "telemetry unavailable",
                context_style(model, theme),
            );
        }
    }
    if body.height > 3 && model.visual().primary().is_some() {
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
        write_rich_context(
            buffer,
            line(body, 3),
            &text,
            context_style(model, theme),
            theme,
            value_style(model, theme),
        );
    }
}

fn render_network(
    model: WidgetRenderModel,
    area: Rect,
    theme: &Theme,
    buffer: &mut Buffer,
    inventory: Option<&HardwareInventory>,
) {
    let title = inventory.and_then(network_inventory_label).map_or_else(
        || "⇄ NETWORK".to_owned(),
        |label| format!("⇄ NETWORK  {label}"),
    );
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
    if graph.width >= MIN_GRAPH_WIDTH
        && let Some((download_area, upload_area)) = dual_graph_areas(graph)
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
        }
    }
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
    let physical = inventory
        .networks
        .iter()
        .filter(|network| !is_loopback(&network.name))
        .collect::<Vec<_>>();
    match physical.as_slice() {
        [] => Some("no physical interface".to_owned()),
        [network] => Some(network.name.clone()),
        _ => Some(format!("{} interfaces", physical.len())),
    }
}

fn render_storage(
    model: WidgetRenderModel,
    _preset: DashboardPreset,
    area: Rect,
    theme: &Theme,
    buffer: &mut Buffer,
) {
    render_header("▰ STORAGE", model, area, theme, buffer);
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
            line(body, 2),
            fraction.unwrap_or(0.0),
            theme.chart_primary,
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
        format!("◇ GPU  {primary_name}  ({}/{})", 1, inventory.gpus.len())
    } else {
        format!("◇ GPU  {primary_name}")
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
        let usage = primary.utilization_percent.unwrap_or(0.0).clamp(0.0, 100.0) / 100.0;
        write(
            buffer,
            line(body, 0),
            &gpu_usage_text(primary),
            value_style_for_inventory(primary.availability, theme),
        );
        render_segmented_meter(line(body, 1), usage, theme.chart_primary, theme, buffer);
        if body.height > 2 {
            write_rich_context(
                buffer,
                line(body, 2),
                &gpu_context(primary),
                context_style_for_theme(theme),
                theme,
                context_value_style_for_theme(theme),
            );
        }
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

fn select_primary_gpu(gpus: &[super::super::GpuDevice]) -> usize {
    let mut selected = 0;
    let mut selected_score = (0_u8, 0_u8, 0_u8, 0_u8);
    for (index, gpu) in gpus.iter().enumerate() {
        let complete = u8::from(
            gpu.utilization_percent.is_some()
                && gpu.vram_used_bytes.is_some()
                && gpu.vram_total_bytes.is_some()
                && gpu.temperature_celsius.is_some(),
        );
        let usage = u8::from(gpu.utilization_percent.is_some());
        let discrete = u8::from(is_discrete_gpu(&gpu.name));
        let telemetry = u8::from(
            gpu.vram_used_bytes.is_some()
                || gpu.vram_total_bytes.is_some()
                || gpu.temperature_celsius.is_some(),
        );
        let score = (complete, usage, discrete, telemetry);
        if score > selected_score {
            selected = index;
            selected_score = score;
        }
    }
    selected
}

fn is_discrete_gpu(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    name.contains("nvidia")
        || name.contains("geforce")
        || name.contains("rtx")
        || name.contains("gtx")
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
    let mut values = Vec::new();
    if let Some((used, total)) = gpu.vram_used_bytes.zip(gpu.vram_total_bytes) {
        values.push(format_compact_capacity(used, total));
    }
    if let Some(temperature) = gpu.temperature_celsius {
        values.push(format!("{temperature:.0}°C"));
    }
    if let Some(frequency) = gpu.frequency_hz {
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
    if area.width == 0 || area.height == 0 {
        return;
    }
    let filled = (f64::from(area.width) * value.clamp(0.0, 1.0)).round() as u16;
    for index in 0..area.width {
        let (character, style) = if index < filled {
            (
                METER_FILL_GLYPH,
                Style::default().fg(fill).bg(theme.surface),
            )
        } else {
            (
                METER_TRACK_GLYPH,
                Style::default()
                    .fg(theme.surface_alt)
                    .bg(theme.surface)
                    .add_modifier(Modifier::DIM),
            )
        };
        buffer[(area.x + index, area.y)]
            .set_char(character)
            .set_style(style);
    }
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
    let title = format!("▰ STORAGE  {} devices", inventory.storages.len());
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
                theme.chart_primary,
                theme,
                buffer,
            );
        } else if rows_per_device == 3 {
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

fn dual_graph_areas(graph: Rect) -> Option<(Rect, Rect)> {
    if graph.height < 2 {
        return None;
    }
    if graph.height == 2 {
        return Some((
            Rect::new(graph.x, graph.y, graph.width, 1),
            Rect::new(graph.x, graph.y.saturating_add(1), graph.width, 1),
        ));
    }
    let first_height = (graph.height / 2).max(1);
    let second_height = graph.height.saturating_sub(first_height);
    (second_height > 0).then(|| {
        (
            Rect::new(graph.x, graph.y, graph.width, first_height),
            Rect::new(
                graph.x,
                graph.y.saturating_add(first_height),
                graph.width,
                second_height,
            ),
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
    let (lower, upper) = vital_scale(low, high);
    let visual_span = (upper - lower).max(0.08);
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
        if (raw - value).abs() > visual_span * 0.18 {
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
    if count == 1
        && let Some(value) = smooth[0]
    {
        let normalized = ((value - lower) / (upper - lower)).clamp(0.0, 1.0);
        let y = pixel_height.saturating_sub(1).saturating_sub(
            (normalized * (pixel_height.saturating_sub(1) as f64)).round() as usize,
        );
        draw_segment(&mut cells, area, (0, y), (pixel_width.saturating_sub(1), y));
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

fn vital_scale(low: f64, high: f64) -> (f64, f64) {
    let low = if low.is_finite() { low.max(0.0) } else { 0.0 };
    let high = if high.is_finite() { high.max(low) } else { low };
    let normalized = high <= 1.0;
    let minimum_range = if normalized {
        0.08
    } else {
        (high.abs() * 0.08).max(1.0)
    };
    let span = (high - low).max(minimum_range);
    let margin = (span * 0.18).max(if normalized { 0.015 } else { 0.08 });
    let lower = (low - margin).max(0.0);
    let upper = if normalized {
        (high + margin).min(1.0).max(lower + 0.08)
    } else {
        high + margin
    };
    (lower, upper.max(lower + minimum_range))
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
            let (first, second) = dual_graph_areas(graph).unwrap();
            assert!(first.x >= graph.x && first.right() <= graph.right());
            assert!(second.x >= graph.x && second.right() <= graph.right());
            assert!(first.y >= graph.y && second.bottom() <= graph.bottom());
            assert_eq!(first.bottom(), second.y);
        }
    }

    #[test]
    fn dual_graphs_keep_both_directions_in_a_two_row_area() {
        let graph = Rect::new(4, 6, 18, 2);
        let (download, upload) = dual_graph_areas(graph).unwrap();
        assert_eq!(download.height, 1);
        assert_eq!(upload.height, 1);
        assert_eq!(download.bottom(), upload.y);
        assert_eq!(upload.bottom(), graph.bottom());
    }

    #[test]
    fn compact_gpu_capacity_avoids_unnecessary_precision() {
        let gibibyte = 1024.0 * 1024.0 * 1024.0;
        assert_eq!(
            format_compact_capacity(0.7 * gibibyte, 8.0 * gibibyte),
            "0.7 / 8 GB"
        );
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
    fn vital_scale_reveals_absolute_rate_variation_without_flattening_it() {
        let (lower, upper) = vital_scale(20.0, 80.0);
        assert!(lower < 20.0);
        assert!(upper > 80.0);
        assert!(upper - lower < 100.0);

        let (normalized_lower, normalized_upper) = vital_scale(0.20, 0.80);
        assert!(normalized_lower >= 0.0);
        assert!(normalized_upper <= 1.0);
        assert!(normalized_upper - normalized_lower >= 0.08);
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
