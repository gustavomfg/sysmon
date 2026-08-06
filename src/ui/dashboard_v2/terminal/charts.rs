use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
};

use crate::theme::Theme;

use super::super::metrics::MAX_PEAKS;
use super::super::{
    ChartBlock, ChartContent, ChartEventKind, DashboardWidgetId, MetricVisual, PeakSet,
    PointAggregation, SegmentId, SegmentedBars, TrendDirection,
};
use super::blocks::{semantic_style, write};

pub(crate) fn render(
    block: ChartBlock,
    visual: MetricVisual,
    area: Rect,
    theme: &Theme,
    buffer: &mut Buffer,
) {
    match block.content() {
        ChartContent::Sparkline(sparkline) => {
            render_sparkline(sparkline, block, visual, area, theme, buffer);
        }
        ChartContent::Area(area_model) => {
            render_area(area_model, block, visual, area, theme, buffer);
        }
        ChartContent::Segments2(segments) => {
            render_segments(segments, block, visual, area, theme, buffer);
        }
        ChartContent::Segments3(segments) => {
            render_segments(segments, block, visual, area, theme, buffer);
        }
        ChartContent::Comparison(comparison) => {
            render_comparison(comparison, visual, block, area, theme, buffer);
        }
        ChartContent::Peaks(peaks) => {
            render_peaks(peaks, block, area, theme, buffer);
        }
    }
}

fn render_sparkline<const POINTS: usize>(
    sparkline: super::super::SparklineModel<POINTS>,
    block: ChartBlock,
    visual: MetricVisual,
    area: Rect,
    theme: &Theme,
    buffer: &mut Buffer,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let points = sparkline.points();
    let mut samples = [None; POINTS];
    let mut length = 0;
    for point in points.iter() {
        if length < POINTS {
            samples[length] = Some(point);
            length += 1;
        }
    }
    if length == 0 {
        write(
            buffer,
            area,
            "·",
            chart_style(block, theme, theme.chart_grid),
        );
        return;
    }
    let minimum = sparkline.stats().minimum().unwrap_or(0.0);
    let maximum = sparkline.stats().maximum().unwrap_or(minimum);
    let range = (maximum - minimum).max(f64::EPSILON);
    for column in 0..area.width {
        let sample_index = if area.width <= 1 || length <= 1 {
            0
        } else {
            usize::from(column) * (length - 1) / usize::from(area.width - 1)
        };
        let Some(point) = samples[sample_index] else {
            continue;
        };
        let Some(value) = point.sample().usable_value() else {
            continue;
        };
        let normalized = ((value - minimum) / range).clamp(0.0, 1.0);
        let filled = normalized * f64::from(area.height);
        for row in 0..area.height {
            let from_bottom = f64::from(area.height - 1 - row);
            let character = if filled > from_bottom + 1.0 {
                '█'
            } else if filled > from_bottom {
                '▂'
            } else {
                ' '
            };
            let mut style = chart_style(block, theme, theme.chart_primary);
            if point.aggregation() == PointAggregation::Maximum {
                style = style.add_modifier(Modifier::BOLD);
            }
            buffer[(area.x + column, area.y + row)]
                .set_char(character)
                .set_style(style);
        }
    }
    let marker = chart_marker(visual.widget_id(), block.metadata().id());
    if !marker.is_empty() {
        write(
            buffer,
            area,
            marker,
            semantic_style(block.metadata(), theme).fg(theme.text_muted),
        );
    }
}

fn render_area(
    area_model: super::super::AreaModel,
    block: ChartBlock,
    visual: MetricVisual,
    area: Rect,
    theme: &Theme,
    buffer: &mut Buffer,
) {
    let Some(occupancy) = area_model.occupancy() else {
        write(
            buffer,
            area,
            "—",
            chart_style(block, theme, theme.chart_grid),
        );
        return;
    };
    render_bar(
        occupancy,
        area,
        chart_style(block, theme, theme.chart_secondary),
        buffer,
    );
    let marker = chart_marker(visual.widget_id(), block.metadata().id());
    if !marker.is_empty() {
        write(
            buffer,
            area,
            marker,
            semantic_style(block.metadata(), theme).fg(theme.text_muted),
        );
    }
}

fn render_segments<const MAX_SEGMENTS: usize>(
    segments: SegmentedBars<MAX_SEGMENTS>,
    block: ChartBlock,
    visual: MetricVisual,
    area: Rect,
    theme: &Theme,
    buffer: &mut Buffer,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let label = segment_label(visual.widget_id(), block.metadata().id());
    let label_width = if label.is_empty() {
        0
    } else {
        label.len().saturating_add(1).min(usize::from(area.width))
    };
    if label_width > 0 {
        write(
            buffer,
            Rect::new(
                area.x,
                area.y,
                u16::try_from(label_width).unwrap_or(area.width),
                1,
            ),
            label,
            semantic_style(block.metadata(), theme).fg(theme.text_muted),
        );
    }
    let bar_start = area
        .x
        .saturating_add(u16::try_from(label_width).unwrap_or(area.width));
    let bar_width = area
        .width
        .saturating_sub(u16::try_from(label_width).unwrap_or(area.width));
    if bar_width == 0 {
        return;
    }
    let mut normalized = [(SegmentId::Other, 0.0); MAX_SEGMENTS];
    let mut length = 0;
    for segment in segments.iter() {
        if length < MAX_SEGMENTS {
            normalized[length] = (
                segment.id(),
                segments.normalized(segment.id()).unwrap_or(0.0),
            );
            length += 1;
        }
    }
    if length == 0 || segments.total().is_none_or(|total| !total.is_usable()) {
        write(
            buffer,
            area,
            "—",
            chart_style(block, theme, theme.chart_grid),
        );
        return;
    }
    for column in 0..bar_width {
        let position = (f64::from(column) + 0.5) / f64::from(bar_width);
        let mut cursor = 0.0;
        let mut color = theme.chart_grid;
        let mut found = false;
        for (id, value) in normalized.iter().take(length) {
            cursor += *value;
            if position <= cursor {
                color = segment_color(*id, theme);
                found = true;
                break;
            }
        }
        let style = chart_style(block, theme, color);
        let character = if found { '█' } else { '·' };
        buffer[(bar_start + column, area.y)]
            .set_char(character)
            .set_style(style);
    }
}

fn render_bar(value: f64, area: Rect, style: Style, buffer: &mut Buffer) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let value = value.clamp(0.0, 1.0);
    for row in 0..area.height {
        for column in 0..area.width {
            let position = (f64::from(column) + 0.5) / f64::from(area.width);
            let character = if position <= value { '█' } else { '░' };
            buffer[(area.x + column, area.y + row)]
                .set_char(character)
                .set_style(style);
        }
    }
}

fn segment_color(id: SegmentId, theme: &Theme) -> ratatui::style::Color {
    match id {
        SegmentId::User | SegmentId::Used | SegmentId::Download => theme.chart_primary,
        SegmentId::System | SegmentId::Write | SegmentId::Upload => theme.chart_secondary,
        SegmentId::Idle | SegmentId::Free => theme.text_muted,
        SegmentId::Cache => theme.info,
        SegmentId::Read => theme.accent,
        SegmentId::Other => theme.primary,
    }
}

fn chart_style(block: ChartBlock, theme: &Theme, color: ratatui::style::Color) -> Style {
    let mut style = semantic_style(block.metadata(), theme);
    if !block.metadata().is_focused() {
        style = style.fg(color);
    }
    style
}

fn render_comparison(
    comparison: super::super::SeriesComparison,
    visual: MetricVisual,
    block: ChartBlock,
    area: Rect,
    theme: &Theme,
    buffer: &mut Buffer,
) {
    let direction = match comparison.direction() {
        TrendDirection::Rising => "↑",
        TrendDirection::Falling => "↓",
        TrendDirection::Stable => "→",
        TrendDirection::Unknown => "·",
    };
    let difference = comparison
        .difference()
        .map_or_else(|| "—".to_owned(), |value| format!("{value:+.1}"));
    let ratio = comparison
        .ratio()
        .map_or_else(|| "—".to_owned(), |value| format!("{value:.1}×"));
    let label = match visual.widget_id() {
        DashboardWidgetId::Network => "d/u",
        DashboardWidgetId::Storage => "r/w",
        _ => "",
    };
    write(
        buffer,
        area,
        &format!("{label} {direction} {difference}  {ratio}"),
        chart_style(block, theme, theme.chart_secondary),
    );
}

fn render_peaks(
    peaks: PeakSet<MAX_PEAKS>,
    block: ChartBlock,
    area: Rect,
    theme: &Theme,
    buffer: &mut Buffer,
) {
    let mut text = String::new();
    for peak in peaks.iter() {
        let symbol = match peak.kind() {
            ChartEventKind::Critical => "◆",
            ChartEventKind::Warning => "◇",
            ChartEventKind::Transition => "·",
            ChartEventKind::Peak => "•",
        };
        text.push_str(symbol);
    }
    if text.is_empty() {
        text.push('·');
    }
    write(
        buffer,
        area,
        &text,
        chart_style(block, theme, theme.chart_primary),
    );
}

fn segment_label(widget: DashboardWidgetId, block: super::super::BlockId) -> &'static str {
    match (widget, block) {
        (DashboardWidgetId::Cpu, super::super::BlockId::Segments) => "u/s/i",
        (DashboardWidgetId::Memory, super::super::BlockId::Segments) => "u/c/f",
        (DashboardWidgetId::Gpu, super::super::BlockId::Segments) => "vram",
        (DashboardWidgetId::Storage, super::super::BlockId::Segments) => "cap",
        _ => "",
    }
}

fn chart_marker(widget: DashboardWidgetId, block: super::super::BlockId) -> &'static str {
    match (widget, block) {
        (DashboardWidgetId::Network, super::super::BlockId::PrimarySparkline) => "↓",
        (DashboardWidgetId::Network, super::super::BlockId::SecondarySparkline) => "↑",
        (DashboardWidgetId::Storage, super::super::BlockId::PrimarySparkline) => "▰",
        (DashboardWidgetId::Storage, super::super::BlockId::SecondarySparkline) => "r",
        (DashboardWidgetId::Storage, super::super::BlockId::TertiarySparkline) => "w",
        (DashboardWidgetId::Memory, super::super::BlockId::Area) => "p",
        _ => "",
    }
}
