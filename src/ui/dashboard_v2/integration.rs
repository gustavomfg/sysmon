#![allow(dead_code)]

use std::time::Duration;

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
};

use crate::{
    theme::Theme,
    ui::widgets::pulse_v2::{
        ColorRasterFrame, FieldDimensions, FieldError, NormalizedPulseInput, PulseV2Engine,
        PulseV2Preset, PulseV2Rasterizer, SemanticColorField,
    },
};

use super::{
    DashboardBounds, DashboardContext, DashboardDimensions, DashboardEngine, DashboardError,
    DashboardFrame, DashboardPreset, DashboardRegion, DashboardState, DashboardWidgetId,
    TerminalRenderError, TerminalRenderer, WidgetRenderModel,
};

const PRIMARY_COUNT: usize = 5;
const PRIMARY_CARD_GAP: u16 = 1;
const PRIMARY_CPU_MINIMUM: u16 = 9;
const PRIMARY_SECONDARY_MINIMUM: u16 = 4;
const PULSE_TITLE: &str = "PULSE V2";
const HEADER_TITLE: &str = "SYSMON V2";
const HEADER_CENTER: &str = "SYSTEM MONITOR";

/// All semantic inputs needed by the V2 dashboard compositor.
///
/// The compositor deliberately accepts prepared widget models. It never reaches
/// into collectors, snapshots, or the V1 application state.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct DashboardV2Content {
    primary: [Option<WidgetRenderModel>; PRIMARY_COUNT],
    pulse_input: NormalizedPulseInput,
    state: DashboardState,
}

impl DashboardV2Content {
    pub(crate) const fn new(
        primary: [Option<WidgetRenderModel>; PRIMARY_COUNT],
        pulse_input: NormalizedPulseInput,
        state: DashboardState,
    ) -> Self {
        Self {
            primary,
            pulse_input,
            state,
        }
    }

    pub(crate) const fn primary(self) -> [Option<WidgetRenderModel>; PRIMARY_COUNT] {
        self.primary
    }

    pub(crate) const fn pulse_input(self) -> NormalizedPulseInput {
        self.pulse_input
    }

    pub(crate) const fn state(self) -> DashboardState {
        self.state
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DashboardV2RenderError {
    Dashboard(DashboardError),
    Terminal(TerminalRenderError),
    Pulse(FieldError),
    BoundsOverflow,
}

impl From<DashboardError> for DashboardV2RenderError {
    fn from(error: DashboardError) -> Self {
        Self::Dashboard(error)
    }
}

impl From<TerminalRenderError> for DashboardV2RenderError {
    fn from(error: TerminalRenderError) -> Self {
        Self::Terminal(error)
    }
}

impl From<FieldError> for DashboardV2RenderError {
    fn from(error: FieldError) -> Self {
        Self::Pulse(error)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DashboardV2RenderSummary {
    frame: DashboardFrame,
    rendered_primary: [Option<DashboardWidgetId>; PRIMARY_COUNT],
    primary_len: usize,
    pulse_rendered: bool,
}

impl DashboardV2RenderSummary {
    pub(crate) const fn frame(self) -> DashboardFrame {
        self.frame
    }

    pub(crate) fn rendered_primary(self) -> impl Iterator<Item = DashboardWidgetId> {
        self.rendered_primary
            .into_iter()
            .take(self.primary_len)
            .flatten()
    }

    pub(crate) const fn pulse_rendered(self) -> bool {
        self.pulse_rendered
    }
}

/// Orchestrates the already existing V2 layout, widget and Pulse pipelines.
///
/// This is an adapter for a future application frontend, not a replacement for
/// the current dashboard renderer.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct DashboardV2Renderer {
    dashboard: DashboardEngine,
    pulse: PulseV2Engine,
    widgets: TerminalRenderer,
}

impl DashboardV2Renderer {
    pub(crate) fn new() -> Self {
        Self {
            dashboard: DashboardEngine::new(),
            pulse: PulseV2Engine::new(),
            widgets: TerminalRenderer::new(),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn render(
        self,
        preset: DashboardPreset,
        context: DashboardContext,
        content: DashboardV2Content,
        elapsed: Duration,
        reduced_motion: bool,
        theme: &Theme,
        buffer: &mut Buffer,
    ) -> Result<DashboardV2RenderSummary, DashboardV2RenderError> {
        let area = *buffer.area();
        let dimensions =
            DashboardDimensions::new(usize::from(area.width), usize::from(area.height))?;
        let frame = match self
            .dashboard
            .compose_with_preset(dimensions, preset, context)
        {
            Ok(frame) => frame,
            Err(_) => self.dashboard.compose_with_preset(
                dimensions,
                DashboardPreset::from_dimensions(dimensions),
                context,
            )?,
        };
        let preset = frame.preset();

        buffer.set_style(area, Style::default().bg(theme.background));
        let layout = frame.layout();

        if let Some(section) = layout.section(DashboardRegion::Header) {
            render_header(
                to_rect(section.bounds(), area)?,
                preset,
                content.state(),
                theme,
                buffer,
            );
        }
        if let Some(section) = layout.section(DashboardRegion::Footer) {
            render_footer(
                to_rect(section.bounds(), area)?,
                preset,
                reduced_motion,
                theme,
                buffer,
            );
        }

        let pulse_rendered = if let Some(section) = layout.section(DashboardRegion::Pulse) {
            self.render_pulse(
                to_rect(section.bounds(), area)?,
                content.pulse_input(),
                elapsed,
                reduced_motion,
                theme,
                buffer,
            )?
        } else {
            false
        };

        let mut rendered_primary = [None; PRIMARY_COUNT];
        let mut primary_len = 0;
        if let Some(section) = layout.section(DashboardRegion::PrimaryMetrics) {
            primary_len = self.render_primary(
                to_rect(section.bounds(), area)?,
                preset,
                content.primary(),
                theme,
                buffer,
                &mut rendered_primary,
            )?;
        }

        for region in [
            DashboardRegion::SecondaryMetrics,
            DashboardRegion::Charts,
            DashboardRegion::Processes,
            DashboardRegion::Alerts,
        ] {
            if let Some(section) = layout.section(region) {
                render_auxiliary(
                    region,
                    to_rect(section.bounds(), area)?,
                    content.state(),
                    gpu_is_available(content.primary()),
                    theme,
                    buffer,
                );
            }
        }

        Ok(DashboardV2RenderSummary {
            frame,
            rendered_primary,
            primary_len,
            pulse_rendered,
        })
    }

    fn render_primary(
        self,
        area: Rect,
        preset: DashboardPreset,
        models: [Option<WidgetRenderModel>; PRIMARY_COUNT],
        theme: &Theme,
        buffer: &mut Buffer,
        rendered: &mut [Option<DashboardWidgetId>; PRIMARY_COUNT],
    ) -> Result<usize, DashboardV2RenderError> {
        let visible = visible_primary(preset, area);
        let active_count = visible
            .iter()
            .filter(|index| **index != usize::MAX && models[**index].is_some())
            .count();
        let slots = split_primary_area(area, active_count);
        let mut length = 0;
        let mut slot_index = 0;

        for model_index in visible {
            if model_index == usize::MAX {
                continue;
            }
            let Some(model) = models[model_index] else {
                continue;
            };
            let slot = slots[slot_index];
            slot_index += 1;
            if slot.width < 2 || slot.height < 1 {
                continue;
            }
            if slot.width >= 12 && slot.height >= 4 {
                buffer.set_style(slot, Style::default().bg(theme.surface));
                draw_panel_frame(slot, theme.border, theme.surface, buffer);
            }
            let card = inset(slot, 1, u16::from(slot.height >= 4));
            if card.width == 0 || card.height == 0 {
                continue;
            }
            self.widgets
                .render_rect(model, preset, card, theme, buffer)?;
            if length < rendered.len() {
                rendered[length] = Some(model.visual().widget_id());
                length += 1;
            }
        }

        Ok(length)
    }

    fn render_pulse(
        self,
        area: Rect,
        input: NormalizedPulseInput,
        elapsed: Duration,
        reduced_motion: bool,
        theme: &Theme,
        buffer: &mut Buffer,
    ) -> Result<bool, DashboardV2RenderError> {
        if area.width < 2 || area.height < 2 {
            return Ok(false);
        }
        buffer.set_style(area, Style::default().bg(theme.background));
        draw_panel_frame(area, theme.border, theme.background, buffer);
        let content = inset(area, 1, 1);
        if content.width == 0 || content.height == 0 {
            return Ok(false);
        }
        buffer.set_stringn(
            content.x,
            content.y,
            PULSE_TITLE,
            usize::from(content.width),
            Style::default().fg(theme.text_muted),
        );

        let raster_area = Rect::new(
            content.x,
            content.y.saturating_add(1),
            content.width,
            content.height.saturating_sub(1),
        );
        if raster_area.width == 0 || raster_area.height == 0 {
            return Ok(false);
        }

        let use_braille = raster_area.width >= 8 && raster_area.height >= 4;
        let field_width = if use_braille {
            usize::from(raster_area.width).checked_mul(2)
        } else {
            Some(usize::from(raster_area.width))
        };
        let field_height = if use_braille {
            usize::from(raster_area.height).checked_mul(4)
        } else {
            Some(usize::from(raster_area.height))
        };
        let (Some(field_width), Some(field_height)) = (field_width, field_height) else {
            return Err(DashboardV2RenderError::BoundsOverflow);
        };
        let dimensions = FieldDimensions::new(field_width, field_height)
            .map_err(|error| DashboardV2RenderError::Pulse(FieldError::Dimensions(error)))?;
        let pulse_preset = PulseV2Preset::from_dimensions(dimensions);
        let frame = self.pulse.generate_with_preset(
            dimensions,
            input,
            pulse_preset,
            elapsed,
            reduced_motion,
        )?;
        let colors = SemanticColorField::from_lighting(frame.lighting(), input, theme)?;
        let raster = if use_braille {
            PulseV2Rasterizer::braille_colored(frame.lighting(), &colors)?
        } else {
            PulseV2Rasterizer::unicode_colored(frame.lighting(), &colors)?
        };
        draw_raster(raster_area, &raster, theme, buffer);
        Ok(true)
    }
}

fn visible_primary(preset: DashboardPreset, area: Rect) -> [usize; PRIMARY_COUNT] {
    let order = match preset {
        // CPU, memory and network are the most useful compact live signals.
        DashboardPreset::Compact => [0, 1, 3, 2, 4],
        DashboardPreset::Standard
        | DashboardPreset::Widescreen
        | DashboardPreset::LargeWidescreen => [0, 1, 2, 3, 4],
    };
    let columns = if area.width >= 40 { 2 } else { 1 };
    let rows = usize::from(area.height) / 3;
    let mut capacity = rows.saturating_mul(columns).min(PRIMARY_COUNT);
    while capacity > 0
        && !primary_stack_fits(area.height, capacity)
        && (capacity > 2 || columns == 1)
    {
        capacity -= 1;
    }
    let mut visible = [usize::MAX; PRIMARY_COUNT];
    for (slot, index) in visible.iter_mut().zip(order).take(capacity) {
        *slot = index;
    }
    visible
}

fn split_primary_area(area: Rect, count: usize) -> [Rect; PRIMARY_COUNT] {
    let count = count.min(PRIMARY_COUNT);
    if count == 0 || area.width == 0 || area.height == 0 {
        return [Rect::default(); PRIMARY_COUNT];
    }
    let tall_stack = count > 2 && usize::from(area.height) >= count.saturating_mul(4);
    let columns = if !tall_stack && area.width >= 40 && count > 1 {
        2
    } else {
        1
    };
    let rows = count.div_ceil(columns);
    let mut result = [Rect::default(); PRIMARY_COUNT];

    // Keep CPU visually prioritized without consuming the column: reserve a
    // compact card-sized region and return the released height to the other
    // metrics, which remain visible below it with their own footprint.
    if columns == 1 {
        let heights = primary_stack_heights(area.height, count);
        let mut y = area.y;
        for (index, slot) in result.iter_mut().enumerate().take(count) {
            let height = heights[index];
            *slot = Rect::new(area.x, y, area.width, height);
            y = y.saturating_add(height).saturating_add(PRIMARY_CARD_GAP);
        }
        return result;
    }

    let horizontal_gap = PRIMARY_CARD_GAP.min(area.width.saturating_sub(1));
    let vertical_gap = PRIMARY_CARD_GAP.min(area.height.saturating_sub(1));
    let column_width = area
        .width
        .saturating_sub(horizontal_gap.saturating_mul(columns.saturating_sub(1) as u16))
        / columns as u16;
    let row_height = area
        .height
        .saturating_sub(vertical_gap.saturating_mul(rows.saturating_sub(1) as u16))
        / rows as u16;
    for (index, slot) in result.iter_mut().enumerate().take(count) {
        let column = index % columns;
        let row = index / columns;
        let x = area
            .x
            .saturating_add(column_width.saturating_mul(column as u16))
            .saturating_add(horizontal_gap.saturating_mul(column as u16));
        let y = area
            .y
            .saturating_add(row_height.saturating_mul(row as u16))
            .saturating_add(vertical_gap.saturating_mul(row as u16));
        let width = if column + 1 == columns {
            area.right().saturating_sub(x)
        } else {
            column_width
        };
        let height = if row + 1 == rows {
            area.bottom().saturating_sub(y)
        } else {
            row_height
        };
        *slot = Rect::new(x, y, width, height);
    }
    result
}

fn primary_stack_heights(total_height: u16, count: usize) -> [u16; PRIMARY_COUNT] {
    if count == 0 {
        return [0; PRIMARY_COUNT];
    }
    if count == 1 {
        return [total_height, 0, 0, 0, 0];
    }

    let secondary_count = (count - 1) as u16;
    let gaps = PRIMARY_CARD_GAP.saturating_mul(secondary_count);
    let usable_height = total_height.saturating_sub(gaps);
    let reserved_for_secondary = PRIMARY_SECONDARY_MINIMUM.saturating_mul(secondary_count);
    let maximum_cpu = usable_height.saturating_sub(reserved_for_secondary);
    let cpu_height = PRIMARY_CPU_MINIMUM.min(maximum_cpu).max(1);
    let remaining = usable_height.saturating_sub(cpu_height);
    let secondary_base = remaining / secondary_count.max(1);
    let secondary_remainder = remaining % secondary_count.max(1);
    let mut heights = [0; PRIMARY_COUNT];
    heights[0] = cpu_height;
    for (index, height) in heights.iter_mut().enumerate().skip(1).take(count - 1) {
        *height =
            secondary_base.saturating_add(u16::from(((index - 1) as u16) < secondary_remainder));
    }
    heights
}

fn primary_stack_fits(total_height: u16, count: usize) -> bool {
    if count == 0 {
        return true;
    }
    let secondary_count = count.saturating_sub(1) as u16;
    let required = PRIMARY_CPU_MINIMUM
        .saturating_add(PRIMARY_SECONDARY_MINIMUM.saturating_mul(secondary_count))
        .saturating_add(PRIMARY_CARD_GAP.saturating_mul(secondary_count));
    total_height >= required
}

fn render_header(
    area: Rect,
    preset: DashboardPreset,
    state: DashboardState,
    theme: &Theme,
    buffer: &mut Buffer,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    buffer.set_style(area, Style::default().bg(theme.background));
    let right = format!("{}  ·  {}", preset_label(preset), state_label(state));
    buffer.set_stringn(
        area.x,
        area.y,
        HEADER_TITLE,
        usize::from(area.width),
        Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
    );
    let center_width = u16::try_from(HEADER_CENTER.chars().count()).unwrap_or(area.width);
    if center_width <= area.width {
        buffer.set_stringn(
            area.x.saturating_add((area.width - center_width) / 2),
            area.y,
            HEADER_CENTER,
            usize::from(center_width),
            Style::default().fg(theme.text),
        );
    }
    let right_width = u16::try_from(right.chars().count()).unwrap_or(area.width);
    if right_width <= area.width {
        buffer.set_stringn(
            area.right().saturating_sub(right_width),
            area.y,
            right,
            usize::from(right_width),
            Style::default().fg(theme.text_muted),
        );
    }
    if area.height > 1 {
        draw_rule(
            area.x,
            area.y.saturating_add(1),
            area.width,
            theme.border,
            theme.background,
            buffer,
        );
    }
}

fn render_footer(
    area: Rect,
    preset: DashboardPreset,
    reduced_motion: bool,
    theme: &Theme,
    buffer: &mut Buffer,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let motion = if reduced_motion {
        "Reduced Motion"
    } else {
        "Live"
    };
    let text = format!(
        "{}  ·  {}                                      P preset  S state  G GPU  R motion  Q quit",
        preset_label(preset),
        motion,
    );
    buffer.set_stringn(
        area.x,
        area.y,
        text,
        usize::from(area.width),
        Style::default().fg(theme.text_muted),
    );
}

fn render_auxiliary(
    region: DashboardRegion,
    area: Rect,
    state: DashboardState,
    gpu_available: bool,
    theme: &Theme,
    buffer: &mut Buffer,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    buffer.set_style(area, Style::default().bg(theme.surface));
    draw_panel_frame(area, theme.border, theme.surface, buffer);
    let content = inset(area, 1, 1);
    if content.width == 0 || content.height == 0 {
        return;
    }
    match region {
        DashboardRegion::SecondaryMetrics => render_system_context(content, theme, buffer),
        DashboardRegion::Charts => {
            render_temperatures(content, state, gpu_available, theme, buffer)
        }
        DashboardRegion::Processes => render_processes(content, state, theme, buffer),
        DashboardRegion::Alerts => render_events(content, state, theme, buffer),
        _ => {}
    }
}

fn render_system_context(area: Rect, theme: &Theme, buffer: &mut Buffer) {
    panel_heading(area, "SYSTEM CONTEXT", theme.info, theme, buffer);
    let rows = [
        "8 cores  ·  16 threads",
        "16 GB memory  ·  3d 04h",
        "Pulse online  ·  history ready",
    ];
    for (index, row) in rows.into_iter().enumerate() {
        write_panel_row(area, 2 + index as u16, row, theme.text_muted, theme, buffer);
    }
}

fn render_temperatures(
    area: Rect,
    state: DashboardState,
    gpu_available: bool,
    theme: &Theme,
    buffer: &mut Buffer,
) {
    panel_heading(area, "TEMPERATURES", theme.warning, theme, buffer);
    let values = match state {
        DashboardState::Calm => [52_u16, 54, 42],
        DashboardState::Active => [68, 70, 48],
        DashboardState::Busy => [84, 85, 61],
        DashboardState::Critical => [96, 96, 76],
    };
    let labels = ["CPU", "GPU", "NVMe"];
    for (index, (label, value)) in labels.into_iter().zip(values).enumerate() {
        if index == 1 && !gpu_available {
            render_temperature_row(area, 2 + index as u16, label, None, theme, buffer);
        } else {
            render_temperature_row(area, 2 + index as u16, label, Some(value), theme, buffer);
        }
    }
}

fn render_temperature_row(
    area: Rect,
    row: u16,
    label: &str,
    value: Option<u16>,
    theme: &Theme,
    buffer: &mut Buffer,
) {
    if row >= area.height {
        return;
    }
    let value_text = value.map_or_else(|| "—".to_owned(), |value| format!("{value}°C"));
    let label_width = 6_u16.min(area.width.saturating_sub(1));
    let bar_start = area.x.saturating_add(label_width);
    let bar_width = area.width.saturating_sub(label_width).saturating_sub(7);
    let y = area.y.saturating_add(row);
    buffer.set_stringn(
        area.x,
        y,
        label,
        usize::from(label_width),
        Style::default().fg(theme.text_muted).bg(theme.surface),
    );
    if let Some(value) = value {
        draw_horizontal_bar(
            bar_start,
            y,
            bar_width,
            f64::from(value) / 100.0,
            if value >= 90 {
                theme.danger
            } else if value >= 75 {
                theme.warning
            } else {
                theme.info
            },
            theme,
            buffer,
        );
    }
    let value_x = area.right().saturating_sub(7);
    buffer.set_stringn(
        value_x,
        y,
        value_text,
        usize::from(area.right().saturating_sub(value_x)),
        Style::default().fg(theme.text).bg(theme.surface),
    );
}

fn render_processes(area: Rect, state: DashboardState, theme: &Theme, buffer: &mut Buffer) {
    panel_heading(area, "TOP PROCESSES", theme.primary, theme, buffer);
    let load = match state {
        DashboardState::Calm => [12_u16, 8, 5],
        DashboardState::Active => [34, 22, 14],
        DashboardState::Busy => [68, 51, 36],
        DashboardState::Critical => [91, 78, 64],
    };
    let processes = ["sysmon", "browser", "compiler"];
    for (index, (name, cpu)) in processes.into_iter().zip(load).enumerate() {
        let y = area.y.saturating_add(2 + index as u16);
        if y >= area.bottom() {
            break;
        }
        let value = format!("{cpu:>3}%  {:>3}%", (cpu / 2).max(1));
        let available = usize::from(area.width).saturating_sub(value.chars().count() + 1);
        buffer.set_stringn(
            area.x,
            y,
            name,
            available,
            Style::default().fg(theme.text).bg(theme.surface),
        );
        buffer.set_stringn(
            area.right()
                .saturating_sub(u16::try_from(value.chars().count()).unwrap_or(area.width)),
            y,
            value,
            usize::from(area.width),
            Style::default().fg(theme.text_muted).bg(theme.surface),
        );
    }
}

fn render_events(area: Rect, state: DashboardState, theme: &Theme, buffer: &mut Buffer) {
    panel_heading(area, "RECENT EVENTS", theme.secondary, theme, buffer);
    let events = match state {
        DashboardState::Calm => ["Pulse stabilized", "No active pressure"],
        DashboardState::Active => ["Workload increased", "Network flow active"],
        DashboardState::Busy => ["Memory pressure rising", "Thermal headroom reduced"],
        DashboardState::Critical => ["Critical load detected", "Thermal limit approaching"],
    };
    for (index, event) in events.into_iter().enumerate() {
        write_panel_row(
            area,
            2 + index as u16,
            event,
            theme.text_muted,
            theme,
            buffer,
        );
    }
}

fn panel_heading(
    area: Rect,
    title: &str,
    accent: ratatui::style::Color,
    theme: &Theme,
    buffer: &mut Buffer,
) {
    if area.height == 0 || area.width == 0 {
        return;
    }
    buffer.set_stringn(
        area.x,
        area.y,
        title,
        usize::from(area.width),
        Style::default()
            .fg(accent)
            .bg(theme.surface)
            .add_modifier(Modifier::BOLD),
    );
    if area.height > 1 {
        draw_rule(
            area.x,
            area.y.saturating_add(1),
            area.width,
            theme.border,
            theme.surface,
            buffer,
        );
    }
}

fn draw_panel_frame(
    area: Rect,
    border: ratatui::style::Color,
    background: ratatui::style::Color,
    buffer: &mut Buffer,
) {
    if area.width < 3 || area.height < 3 {
        return;
    }
    let style = Style::default().fg(border).bg(background);
    let right = area.right().saturating_sub(1);
    let bottom = area.bottom().saturating_sub(1);
    for x in area.x..=right {
        buffer[(x, area.y)].set_symbol(if x == area.x {
            "╭"
        } else if x == right {
            "╮"
        } else {
            "─"
        });
        buffer[(x, bottom)].set_symbol(if x == area.x {
            "╰"
        } else if x == right {
            "╯"
        } else {
            "─"
        });
        buffer[(x, area.y)].set_style(style);
        buffer[(x, bottom)].set_style(style);
    }
    for y in area.y.saturating_add(1)..bottom {
        buffer[(area.x, y)].set_symbol("│");
        buffer[(right, y)].set_symbol("│");
        buffer[(area.x, y)].set_style(style);
        buffer[(right, y)].set_style(style);
    }
}

fn draw_rule(
    x: u16,
    y: u16,
    width: u16,
    foreground: ratatui::style::Color,
    background: ratatui::style::Color,
    buffer: &mut Buffer,
) {
    let style = Style::default().fg(foreground).bg(background);
    for column in 0..width {
        buffer[(x.saturating_add(column), y)]
            .set_symbol("─")
            .set_style(style);
    }
}

fn write_panel_row(
    area: Rect,
    row: u16,
    text: &str,
    color: ratatui::style::Color,
    theme: &Theme,
    buffer: &mut Buffer,
) {
    if row < area.height {
        buffer.set_stringn(
            area.x,
            area.y.saturating_add(row),
            text,
            usize::from(area.width),
            Style::default().fg(color).bg(theme.surface),
        );
    }
}

fn draw_horizontal_bar(
    x: u16,
    y: u16,
    width: u16,
    value: f64,
    color: ratatui::style::Color,
    theme: &Theme,
    buffer: &mut Buffer,
) {
    for column in 0..width {
        let position = (f64::from(column) + 0.5) / f64::from(width.max(1));
        let filled = position <= value.clamp(0.0, 1.0);
        buffer[(x.saturating_add(column), y)]
            .set_char(if filled { '━' } else { '·' })
            .set_style(
                Style::default()
                    .fg(if filled { color } else { theme.border })
                    .bg(theme.surface),
            );
    }
}

fn gpu_is_available(models: [Option<WidgetRenderModel>; PRIMARY_COUNT]) -> bool {
    models[2]
        .map(|model| model.state() != super::WidgetState::Unavailable)
        .unwrap_or(false)
}

fn draw_raster(area: Rect, raster: &ColorRasterFrame, theme: &Theme, buffer: &mut Buffer) {
    let width = raster.width().min(usize::from(area.width));
    let height = raster.height().min(usize::from(area.height));
    let raster_width = raster.width();
    for y in 0..height {
        for x in 0..width {
            let cell = raster.as_slice()[y * raster_width + x];
            let mut encoded = [0_u8; 4];
            let symbol = cell.character().encode_utf8(&mut encoded);
            let terminal_cell = &mut buffer[(
                area.x.saturating_add(u16::try_from(x).unwrap_or(0)),
                area.y.saturating_add(u16::try_from(y).unwrap_or(0)),
            )];
            terminal_cell.set_symbol(symbol);
            terminal_cell.set_fg(cell.foreground());
            terminal_cell.set_bg(theme.background);
        }
    }
}

fn to_rect(bounds: DashboardBounds, outer: Rect) -> Result<Rect, DashboardV2RenderError> {
    let x = outer
        .x
        .checked_add(u16::try_from(bounds.x()).map_err(|_| DashboardV2RenderError::BoundsOverflow)?)
        .ok_or(DashboardV2RenderError::BoundsOverflow)?;
    let y = outer
        .y
        .checked_add(u16::try_from(bounds.y()).map_err(|_| DashboardV2RenderError::BoundsOverflow)?)
        .ok_or(DashboardV2RenderError::BoundsOverflow)?;
    let width =
        u16::try_from(bounds.width()).map_err(|_| DashboardV2RenderError::BoundsOverflow)?;
    let height =
        u16::try_from(bounds.height()).map_err(|_| DashboardV2RenderError::BoundsOverflow)?;
    let right = x
        .checked_add(width)
        .ok_or(DashboardV2RenderError::BoundsOverflow)?;
    let bottom = y
        .checked_add(height)
        .ok_or(DashboardV2RenderError::BoundsOverflow)?;
    if right > outer.right() || bottom > outer.bottom() {
        return Err(DashboardV2RenderError::BoundsOverflow);
    }
    Ok(Rect::new(x, y, width, height))
}

fn inset(area: Rect, horizontal: u16, vertical: u16) -> Rect {
    let horizontal = horizontal.min(area.width / 2);
    let vertical = vertical.min(area.height / 2);
    Rect::new(
        area.x.saturating_add(horizontal),
        area.y.saturating_add(vertical),
        area.width.saturating_sub(horizontal.saturating_mul(2)),
        area.height.saturating_sub(vertical.saturating_mul(2)),
    )
}

const fn preset_label(preset: DashboardPreset) -> &'static str {
    match preset {
        DashboardPreset::Compact => "Compact",
        DashboardPreset::Standard => "Standard",
        DashboardPreset::Widescreen => "Widescreen",
        DashboardPreset::LargeWidescreen => "Large Widescreen",
    }
}

const fn state_label(state: DashboardState) -> &'static str {
    match state {
        DashboardState::Calm => "Calm",
        DashboardState::Active => "Active",
        DashboardState::Busy => "Busy",
        DashboardState::Critical => "Critical",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{buffer::Buffer, layout::Rect};

    fn input() -> NormalizedPulseInput {
        NormalizedPulseInput::try_new(0.35, 0.42, Some(0.28), 0.22, 0.34, 0.12).unwrap()
    }

    #[test]
    fn empty_content_still_renders_the_pulse_and_valid_layout() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 160, 40));
        let content = DashboardV2Content::new([None; PRIMARY_COUNT], input(), DashboardState::Calm);
        let result = DashboardV2Renderer::new()
            .render(
                DashboardPreset::Widescreen,
                DashboardContext::default(),
                content,
                Duration::from_secs(2),
                false,
                &Theme::pulse(),
                &mut buffer,
            )
            .unwrap();

        assert!(result.pulse_rendered());
        assert!(result.frame().layout().validate().is_ok());
        assert!(
            buffer
                .content
                .iter()
                .any(|cell| cell.symbol() == "⠂" || cell.symbol() == "·")
        );
    }

    #[test]
    fn integration_is_deterministic_and_preserves_pulse_priority() {
        let content =
            DashboardV2Content::new([None; PRIMARY_COUNT], input(), DashboardState::Active);
        let render = |buffer: &mut Buffer| {
            DashboardV2Renderer::new()
                .render(
                    DashboardPreset::LargeWidescreen,
                    DashboardContext::default(),
                    content,
                    Duration::from_secs(5),
                    true,
                    &Theme::pulse(),
                    buffer,
                )
                .unwrap()
        };
        let mut first = Buffer::empty(Rect::new(0, 0, 240, 60));
        let mut second = Buffer::empty(Rect::new(0, 0, 240, 60));
        let first_summary = render(&mut first);
        let second_summary = render(&mut second);
        assert_eq!(first_summary, second_summary);
        assert_eq!(first, second);
        assert_eq!(
            first_summary
                .frame()
                .layout()
                .composition()
                .regions()
                .next(),
            Some(DashboardRegion::Pulse)
        );
    }

    #[test]
    fn compact_hides_lower_priority_primary_slots_without_overflow() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 80, 24));
        let content = DashboardV2Content::new([None; PRIMARY_COUNT], input(), DashboardState::Calm);
        let summary = DashboardV2Renderer::new()
            .render(
                DashboardPreset::Compact,
                DashboardContext::default(),
                content,
                Duration::ZERO,
                true,
                &Theme::pulse(),
                &mut buffer,
            )
            .unwrap();
        assert!(summary.pulse_rendered());
        assert!(summary.frame().layout().validate().is_ok());
    }

    #[test]
    fn an_explicit_large_preset_degrades_safely_in_a_small_terminal() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 80, 24));
        let content = DashboardV2Content::new([None; PRIMARY_COUNT], input(), DashboardState::Calm);
        let summary = DashboardV2Renderer::new()
            .render(
                DashboardPreset::Widescreen,
                DashboardContext::default(),
                content,
                Duration::ZERO,
                true,
                &Theme::pulse(),
                &mut buffer,
            )
            .unwrap();
        assert!(summary.pulse_rendered());
        assert!(summary.frame().layout().validate().is_ok());
    }

    #[test]
    fn raster_modes_follow_available_pulse_area() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 24, 8));
        let content = DashboardV2Content::new([None; PRIMARY_COUNT], input(), DashboardState::Calm);
        assert!(
            DashboardV2Renderer::new()
                .render(
                    DashboardPreset::Compact,
                    DashboardContext::default(),
                    content,
                    Duration::ZERO,
                    true,
                    &Theme::pulse(),
                    &mut buffer,
                )
                .is_ok()
        );
    }

    #[test]
    fn wide_composition_contains_real_context_panels_instead_of_placeholders() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 240, 60));
        let content = DashboardV2Content::new([None; PRIMARY_COUNT], input(), DashboardState::Busy);
        DashboardV2Renderer::new()
            .render(
                DashboardPreset::LargeWidescreen,
                DashboardContext::default(),
                content,
                Duration::from_secs(4),
                false,
                &Theme::pulse(),
                &mut buffer,
            )
            .unwrap();
        let text: String = buffer.content.iter().map(|cell| cell.symbol()).collect();
        assert!(text.contains("SYSTEM CONTEXT"));
        assert!(text.contains("TEMPERATURES"));
        assert!(text.contains("TOP PROCESSES"));
        assert!(text.contains("RECENT EVENTS"));
        assert!(!text.contains("Shared history context"));
        assert!(!text.contains("Process context available"));
    }

    #[test]
    fn primary_column_gives_cpu_a_reference_sized_region() {
        let slots = split_primary_area(Rect::new(0, 0, 48, 37), PRIMARY_COUNT);
        let heights: Vec<u16> = slots.iter().map(|slot| slot.height).collect();
        assert!((9..=10).contains(&heights[0]));
        assert!(heights[0] > heights[1]);
        assert!(heights[1..].iter().all(|height| *height >= 4));
        assert_eq!(heights.iter().sum::<u16>() + PRIMARY_CARD_GAP * 4, 37);
        for pair in slots.windows(2) {
            assert_eq!(pair[0].bottom() + PRIMARY_CARD_GAP, pair[1].y);
        }
    }

    #[test]
    fn primary_column_degrades_lower_priority_cards_without_invalid_slots() {
        let area = Rect::new(0, 0, 48, 21);
        let visible = visible_primary(DashboardPreset::Widescreen, area);
        assert_eq!(visible, [0, 1, 2, usize::MAX, usize::MAX]);

        let slots = split_primary_area(area, 3);
        assert!(slots[0].height > slots[1].height);
        assert!(
            slots[..3]
                .iter()
                .all(|slot| slot.width > 0 && slot.height >= PRIMARY_SECONDARY_MINIMUM)
        );
        assert_eq!(slots[0].bottom() + PRIMARY_CARD_GAP, slots[1].y);
        assert_eq!(slots[1].bottom() + PRIMARY_CARD_GAP, slots[2].y);
        assert_eq!(slots[2].bottom(), area.bottom());
    }

    #[test]
    fn primary_slots_leave_a_gap_and_never_reach_the_footer() {
        for (width, height) in [(80, 24), (120, 32), (160, 40)] {
            let dimensions = DashboardDimensions::new(width, height).unwrap();
            let layout = DashboardEngine::new()
                .compose_with_preset(
                    dimensions,
                    DashboardPreset::Widescreen,
                    DashboardContext::default(),
                )
                .unwrap()
                .layout();
            let body = layout
                .section(DashboardRegion::PrimaryMetrics)
                .unwrap()
                .bounds();
            let footer = layout.section(DashboardRegion::Footer).unwrap().bounds();
            let visible = visible_primary(
                DashboardPreset::Widescreen,
                Rect::new(
                    body.x() as u16,
                    body.y() as u16,
                    body.width() as u16,
                    body.height() as u16,
                ),
            );
            let count = visible.iter().filter(|index| **index != usize::MAX).count();
            let slots = split_primary_area(
                Rect::new(
                    body.x() as u16,
                    body.y() as u16,
                    body.width() as u16,
                    body.height() as u16,
                ),
                count,
            );
            for pair in slots[..count].windows(2) {
                assert!(pair[0].bottom() < pair[1].y);
            }
            assert!(
                slots[..count]
                    .iter()
                    .all(|slot| slot.bottom() <= footer.y() as u16)
            );
        }
    }
}
