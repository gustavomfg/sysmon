use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
};

use crate::theme::Theme;

use super::super::{
    BlockMetadata, BlockVisibility, FooterBlock, Highlight, IndicatorBlock, MetricIndicators,
    MetricUnit, MetricValue, MetricVisual, RenderBlock, StatusBlock, TitleBlock, TrendBlock,
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
        RenderBlock::Trend(block) => render_trend(block, area, theme, buffer),
        RenderBlock::Chart(block) => charts::render(block, area, theme, buffer),
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
        "  "
    };
    write(
        buffer,
        area,
        &format!("{prefix}{}", block.label()),
        semantic_style(block.metadata(), theme),
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
    let text = secondary.map_or(primary.clone(), |secondary| {
        format!("{primary}  {secondary}")
    });
    write(buffer, area, &text, semantic_style(block.metadata(), theme));
}

fn render_trend(block: TrendBlock, area: Rect, theme: &Theme, buffer: &mut Buffer) {
    let direction = match block.trend().direction() {
        TrendDirection::Rising => "↑",
        TrendDirection::Falling => "↓",
        TrendDirection::Stable => "→",
        TrendDirection::Unknown => "·",
    };
    let intensity = match block.intensity() {
        super::super::Intensity::VeryLow => "low",
        super::super::Intensity::Low => "quiet",
        super::super::Intensity::Normal => "normal",
        super::super::Intensity::High => "high",
        super::super::Intensity::VeryHigh => "peak",
    };
    write(
        buffer,
        area,
        &format!("trend {direction}  {intensity}"),
        semantic_style(block.metadata(), theme),
    );
}

fn render_indicators(block: IndicatorBlock, area: Rect, theme: &Theme, buffer: &mut Buffer) {
    let indicators = [
        (MetricIndicators::PROGRESS, "progress"),
        (MetricIndicators::OCCUPANCY, "occupancy"),
        (MetricIndicators::TREND, "trend"),
        (MetricIndicators::VARIATION, "variation"),
        (MetricIndicators::ACTIVITY, "activity"),
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
    write(
        buffer,
        area,
        &format!("status {}", state_label(block.state())),
        semantic_style(block.metadata(), theme),
    );
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
