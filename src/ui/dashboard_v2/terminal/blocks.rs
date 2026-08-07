use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
};

use crate::theme::Theme;

use super::super::{
    BlockMetadata, BlockVisibility, CpuContextBlock, FooterBlock, GpuContextBlock, Highlight,
    IndicatorBlock, MemoryContextBlock, MetricIndicators, MetricUnit, MetricValue, MetricVisual,
    NetworkContextBlock, RenderBlock, StatusBlock, StorageContextBlock, TitleBlock, TrendBlock,
    TrendDirection, ValueBlock, VisualPriority, WidgetState,
};
use super::charts;

pub(crate) fn render_block(
    block: RenderBlock,
    visual: MetricVisual,
    area: Rect,
    theme: &Theme,
    buffer: &mut Buffer,
) {
    match block {
        RenderBlock::Title(block) => render_title(block, area, theme, buffer),
        RenderBlock::Value(block) => render_value(block, visual, area, theme, buffer),
        RenderBlock::Trend(block) => render_trend(block, visual, area, theme, buffer),
        RenderBlock::CpuContext(block) => render_cpu_context(block, area, theme, buffer),
        RenderBlock::MemoryContext(block) => render_memory_context(block, area, theme, buffer),
        RenderBlock::GpuContext(block) => render_gpu_context(block, area, theme, buffer),
        RenderBlock::NetworkContext(block) => render_network_context(block, area, theme, buffer),
        RenderBlock::StorageContext(block) => render_storage_context(block, area, theme, buffer),
        RenderBlock::Chart(block) => charts::render(block, visual, area, theme, buffer),
        RenderBlock::Indicators(block) => render_indicators(block, area, theme, buffer),
        RenderBlock::Status(block) => render_status(block, area, theme, buffer),
        RenderBlock::Footer(block) => render_footer(block, area, theme, buffer),
    }
}

pub(crate) fn semantic_style(metadata: BlockMetadata, theme: &Theme) -> Style {
    let priority_color = match metadata.priority() {
        VisualPriority::Hero => theme.text,
        VisualPriority::Primary => theme.primary,
        VisualPriority::Secondary => theme.secondary,
        VisualPriority::Contextual | VisualPriority::Auxiliary => theme.text_muted,
    };
    let mut style = Style::default().fg(priority_color).bg(theme.surface);
    style = match metadata.state() {
        WidgetState::Idle => style.fg(theme.text_muted),
        WidgetState::Normal => style,
        WidgetState::Active => style.fg(theme.info),
        WidgetState::Warning => style.fg(theme.warning).add_modifier(Modifier::BOLD),
        WidgetState::Critical => style.fg(theme.danger).add_modifier(Modifier::BOLD),
        WidgetState::Unavailable => style.fg(theme.text_muted).add_modifier(Modifier::DIM),
        WidgetState::Loading => style.fg(theme.info).add_modifier(Modifier::DIM),
    };
    style = match metadata.highlight() {
        Highlight::None => style,
        Highlight::Subtle | Highlight::Strong => style.add_modifier(Modifier::BOLD),
        Highlight::Critical => style.fg(theme.danger).add_modifier(Modifier::BOLD),
    };
    if metadata.visibility() == BlockVisibility::Unavailable {
        style = style.fg(theme.text_muted).add_modifier(Modifier::DIM);
    }
    if metadata.is_focused() {
        style = Style::default()
            .fg(theme.selection_text)
            .bg(theme.selection_background)
            .add_modifier(Modifier::BOLD);
    }
    style
}

fn render_title(block: TitleBlock, area: Rect, theme: &Theme, buffer: &mut Buffer) {
    let prefix = if block.metadata().is_focused() {
        "› "
    } else {
        ""
    };
    let mut style = semantic_style(block.metadata(), theme).fg(theme.text_muted);
    if block.metadata().is_focused() {
        style = style.add_modifier(Modifier::BOLD);
    }
    write(
        buffer,
        area,
        &format!("{prefix}{} {}", title_icon(block.label()), block.label()),
        style,
    );
}

fn render_value(
    block: ValueBlock,
    visual: MetricVisual,
    area: Rect,
    theme: &Theme,
    buffer: &mut Buffer,
) {
    let primary = block
        .value()
        .filter(|value| value.is_usable())
        .map_or_else(|| "—".to_owned(), format_metric_value);
    let secondary = visual
        .secondary()
        .filter(|value| value.is_usable())
        .map(format_metric_value);
    let primary_style = semantic_style(block.metadata(), theme)
        .fg(theme.text)
        .add_modifier(Modifier::BOLD);
    let primary_prefix = match visual.widget_id() {
        super::super::DashboardWidgetId::Network => "↓ ",
        super::super::DashboardWidgetId::Storage => "▰ ",
        _ => "",
    };
    write(
        buffer,
        area,
        &format!("{primary_prefix}{primary}"),
        primary_style,
    );
    if let Some(secondary) = secondary {
        let secondary_prefix = match visual.widget_id() {
            super::super::DashboardWidgetId::Network => "↑ ",
            super::super::DashboardWidgetId::Storage => "• ",
            _ => "",
        };
        let secondary_style = semantic_style(block.metadata(), theme).fg(theme.text_muted);
        if area.height > 1 {
            write(
                buffer,
                Rect::new(area.x, area.y.saturating_add(1), area.width, 1),
                &format!("{secondary_prefix}{secondary}"),
                secondary_style,
            );
        } else {
            let primary_width = primary_prefix.chars().count() + primary.chars().count();
            let offset = primary_width.saturating_add(2);
            if offset < usize::from(area.width) {
                let x = area
                    .x
                    .saturating_add(u16::try_from(offset).unwrap_or(area.width));
                buffer.set_stringn(
                    x,
                    area.y,
                    format!("{secondary_prefix}{secondary}"),
                    usize::from(area.width).saturating_sub(offset),
                    secondary_style,
                );
            }
        }
    }
}

fn title_icon(label: &str) -> &'static str {
    match label {
        "CPU" => "◈",
        "Memory" => "▦",
        "GPU" => "◇",
        "Network" => "⇄",
        "Storage" => "▰",
        _ => "·",
    }
}

fn render_trend(
    block: TrendBlock,
    visual: MetricVisual,
    area: Rect,
    theme: &Theme,
    buffer: &mut Buffer,
) {
    let direction = match block.trend().direction() {
        TrendDirection::Rising => "↑",
        TrendDirection::Falling => "↓",
        TrendDirection::Stable => "→",
        TrendDirection::Unknown => "·",
    };
    let change = format_change(visual);
    let text = if change.is_empty() {
        direction.to_owned()
    } else {
        format!("{direction} {change}")
    };
    write(buffer, area, &text, trend_style(block, theme));
}

fn render_cpu_context(block: CpuContextBlock, area: Rect, theme: &Theme, buffer: &mut Buffer) {
    let mut text = format!("{} cores", block.cores());
    if let Some(frequency) = block.frequency().filter(|value| value.is_usable()) {
        text.push_str(" · ");
        text.push_str(&format_metric_value(frequency));
    }
    let style = semantic_style(block.metadata(), theme);
    let style = if block.metadata().is_focused() {
        style
    } else {
        style.fg(theme.text_muted)
    };
    write(buffer, area, &text, style);
}

fn render_memory_context(
    block: MemoryContextBlock,
    area: Rect,
    theme: &Theme,
    buffer: &mut Buffer,
) {
    let text = block
        .used()
        .zip(block.total())
        .map_or_else(String::new, |(used, total)| {
            format!(
                "{} / {}",
                format_metric_value(used),
                format_metric_value(total)
            )
        });
    write(buffer, area, &text, context_style(block.metadata(), theme));
}

fn render_gpu_context(block: GpuContextBlock, area: Rect, theme: &Theme, buffer: &mut Buffer) {
    let text =
        block
            .vram_used()
            .zip(block.vram_total())
            .map_or_else(String::new, |(used, total)| {
                format!(
                    "VRAM {} / {}",
                    format_metric_value(used),
                    format_metric_value(total)
                )
            });
    write(buffer, area, &text, context_style(block.metadata(), theme));
}

fn render_network_context(
    block: NetworkContextBlock,
    area: Rect,
    theme: &Theme,
    buffer: &mut Buffer,
) {
    let text = match (block.download(), block.upload()) {
        (Some(download), Some(upload)) => format!(
            "↓ {}  ↑ {}",
            format_metric_value(download),
            format_metric_value(upload)
        ),
        (Some(download), None) => format!("↓ {}", format_metric_value(download)),
        (None, Some(upload)) => format!("↑ {}", format_metric_value(upload)),
        _ => String::new(),
    };
    write(buffer, area, &text, context_style(block.metadata(), theme));
}

fn render_storage_context(
    block: StorageContextBlock,
    area: Rect,
    theme: &Theme,
    buffer: &mut Buffer,
) {
    let text =
        block
            .used_space()
            .zip(block.total_space())
            .map_or_else(String::new, |(used, total)| {
                format!(
                    "{} / {}",
                    format_metric_value(used),
                    format_metric_value(total)
                )
            });
    write(buffer, area, &text, context_style(block.metadata(), theme));
}

fn context_style(metadata: BlockMetadata, theme: &Theme) -> Style {
    semantic_style(metadata, theme).fg(theme.text_muted)
}

fn render_indicators(block: IndicatorBlock, area: Rect, theme: &Theme, buffer: &mut Buffer) {
    let indicators = [
        (MetricIndicators::PROGRESS, "◐"),
        (MetricIndicators::OCCUPANCY, "▰"),
        (MetricIndicators::TREND, "↗"),
        (MetricIndicators::VARIATION, "∿"),
        (MetricIndicators::ACTIVITY, "•"),
    ];
    let mut text = String::new();
    for (indicator, label) in indicators {
        if block.indicators().contains(indicator) {
            if !text.is_empty() {
                text.push(' ');
            }
            text.push_str(label);
        }
    }
    if text.is_empty() {
        text.push('·');
    }
    write(buffer, area, &text, semantic_style(block.metadata(), theme));
}

fn render_status(block: StatusBlock, area: Rect, theme: &Theme, buffer: &mut Buffer) {
    let symbol = match block.state() {
        WidgetState::Idle => "·",
        WidgetState::Normal => "●",
        WidgetState::Active => "◆",
        WidgetState::Warning => "!",
        WidgetState::Critical => "×",
        WidgetState::Unavailable => "—",
        WidgetState::Loading => "…",
    };
    write(
        buffer,
        area,
        &format!("{symbol} {}", state_label(block.state())),
        semantic_style(block.metadata(), theme),
    );
}

fn trend_style(block: TrendBlock, theme: &Theme) -> Style {
    let style = semantic_style(block.metadata(), theme);
    match block.intensity() {
        super::super::Intensity::VeryHigh | super::super::Intensity::High => {
            style.add_modifier(Modifier::BOLD)
        }
        _ => style,
    }
}

fn format_change(visual: MetricVisual) -> String {
    let Some(delta) = visual.comparison().delta() else {
        return String::new();
    };
    let Some(primary) = visual.primary() else {
        return String::new();
    };
    if !delta.is_finite() {
        return String::new();
    }
    match primary.unit() {
        MetricUnit::Percent => format!("{:+.0}%", delta * 100.0),
        MetricUnit::Bytes | MetricUnit::BytesPerSecond => {
            format!("{:+.1}", delta)
        }
        _ => format!("{:+.1}", delta),
    }
}

fn render_footer(block: FooterBlock, area: Rect, theme: &Theme, buffer: &mut Buffer) {
    write(buffer, area, "", semantic_style(block.metadata(), theme));
}

pub(crate) fn format_metric_value(value: MetricValue) -> String {
    if !value.is_usable() {
        return "—".to_owned();
    }
    let precision = usize::from(value.precision().min(6));
    let (number, suffix) = match value.unit() {
        MetricUnit::Percent => (value.value() * 100.0, "%"),
        MetricUnit::Bytes => (value.value(), "B"),
        MetricUnit::BytesPerSecond => (value.value(), "B/s"),
        MetricUnit::Count => (value.value(), ""),
        MetricUnit::DurationSeconds => (value.value(), "s"),
        MetricUnit::TemperatureCelsius => (value.value(), "°C"),
        MetricUnit::FrequencyHertz => (value.value(), "Hz"),
        MetricUnit::None => (value.value(), ""),
    };
    if suffix.is_empty() {
        format!("{number:.precision$}")
    } else {
        format!("{number:.precision$}{suffix}")
    }
}

pub(crate) fn state_label(state: WidgetState) -> &'static str {
    match state {
        WidgetState::Idle => "idle",
        WidgetState::Normal => "normal",
        WidgetState::Active => "active",
        WidgetState::Warning => "warning",
        WidgetState::Critical => "critical",
        WidgetState::Unavailable => "unavailable",
        WidgetState::Loading => "loading",
    }
}

pub(crate) fn write(buffer: &mut Buffer, area: Rect, text: &str, style: Style) {
    if area.width > 0 && area.height > 0 {
        buffer.set_stringn(area.x, area.y, text, usize::from(area.width), style);
    }
}
