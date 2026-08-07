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
const PRIMARY_CPU_MINIMUM: u16 = 7;
const PRIMARY_CARD_MINIMUMS: [u16; PRIMARY_COUNT] = [PRIMARY_CPU_MINIMUM, 4, 4, 4, 4];
const PULSE_TITLE: &str = "PULSE V2";
const HEADER_TITLE: &str = "SYSMON V2";
const HEADER_CENTER: &str = "SYSTEM MONITOR";

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DashboardProcessSummary {
    pub(crate) name: String,
    pub(crate) cpu_percent: f32,
    pub(crate) memory_bytes: u64,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct DashboardV2SystemContext {
    pub(crate) physical_cores: Option<usize>,
    pub(crate) logical_threads: Option<usize>,
    pub(crate) total_memory_gib: Option<f64>,
    pub(crate) uptime_seconds: Option<u64>,
    pub(crate) processes: Vec<DashboardProcessSummary>,
}

/// All semantic inputs needed by the V2 dashboard compositor.
///
/// The compositor deliberately accepts prepared widget models. It never reaches
/// into collectors, snapshots, or the V1 application state.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DashboardV2Content {
    primary: [Option<WidgetRenderModel>; PRIMARY_COUNT],
    pulse_input: NormalizedPulseInput,
    state: DashboardState,
    inventory: super::HardwareInventory,
    system_context: Option<DashboardV2SystemContext>,
    live_mode: bool,
}

impl DashboardV2Content {
    pub(crate) fn new(
        primary: [Option<WidgetRenderModel>; PRIMARY_COUNT],
        pulse_input: NormalizedPulseInput,
        state: DashboardState,
    ) -> Self {
        Self {
            primary,
            pulse_input,
            state,
            inventory: super::HardwareInventory::default(),
            system_context: None,
            live_mode: false,
        }
    }

    pub(crate) fn primary(&self) -> [Option<WidgetRenderModel>; PRIMARY_COUNT] {
        self.primary
    }

    pub(crate) fn pulse_input(&self) -> NormalizedPulseInput {
        self.pulse_input
    }

    pub(crate) fn state(&self) -> DashboardState {
        self.state
    }

    pub(crate) fn with_inventory(mut self, inventory: super::HardwareInventory) -> Self {
        self.inventory = inventory;
        self
    }

    pub(crate) fn inventory(&self) -> &super::HardwareInventory {
        &self.inventory
    }

    pub(crate) fn with_system_context(mut self, context: DashboardV2SystemContext) -> Self {
        self.system_context = Some(context);
        self
    }

    pub(crate) fn system_context(&self) -> Option<&DashboardV2SystemContext> {
        self.system_context.as_ref()
    }

    pub(crate) fn with_live_mode(mut self, live_mode: bool) -> Self {
        self.live_mode = live_mode;
        self
    }

    pub(crate) const fn live_mode(&self) -> bool {
        self.live_mode
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
                content.live_mode(),
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
                content.inventory(),
            )?;
        }

        if let Some(section) = layout.section(DashboardRegion::SecondaryMetrics) {
            render_auxiliary(
                DashboardRegion::SecondaryMetrics,
                to_rect(section.bounds(), area)?,
                content.state(),
                gpu_is_available(content.primary()),
                content.inventory(),
                content.system_context(),
                content.live_mode(),
                theme,
                buffer,
            );
        }

        let diagnostics = [
            layout
                .section(DashboardRegion::Charts)
                .map(|section| to_rect(section.bounds(), area))
                .transpose()?,
            layout
                .section(DashboardRegion::Processes)
                .map(|section| to_rect(section.bounds(), area))
                .transpose()?,
            layout
                .section(DashboardRegion::Alerts)
                .map(|section| to_rect(section.bounds(), area))
                .transpose()?,
        ];
        if diagnostics.iter().any(Option::is_some) {
            let first = diagnostics.into_iter().flatten().next().unwrap_or_default();
            let last = diagnostics.into_iter().flatten().last().unwrap_or(first);
            let outer = Rect::new(
                first.x,
                first.y,
                first.width,
                last.bottom().saturating_sub(first.y),
            );
            render_diagnostics_panel(
                outer,
                diagnostics[0],
                diagnostics[1],
                diagnostics[2],
                content.state(),
                gpu_is_available(content.primary()),
                content.inventory(),
                content.system_context(),
                content.live_mode(),
                theme,
                buffer,
            );
        }

        Ok(DashboardV2RenderSummary {
            frame,
            rendered_primary,
            primary_len,
            pulse_rendered,
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn render_primary(
        self,
        area: Rect,
        preset: DashboardPreset,
        models: [Option<WidgetRenderModel>; PRIMARY_COUNT],
        theme: &Theme,
        buffer: &mut Buffer,
        rendered: &mut [Option<DashboardWidgetId>; PRIMARY_COUNT],
        inventory: &super::HardwareInventory,
    ) -> Result<usize, DashboardV2RenderError> {
        if area.width == 0 || area.height == 0 {
            return Ok(0);
        }

        // Primary metrics are one instrument panel. Individual metric
        // renderers still own their semantic anatomy, but they no longer
        // create competing outer boxes of their own.
        let framed = area.width >= 16 && area.height >= 20;
        buffer.set_style(area, Style::default().bg(theme.surface));
        if framed {
            draw_panel_frame(area, theme.border, theme.surface, buffer);
        }
        let panel = if framed { inset(area, 1, 1) } else { area };
        let visible = visible_primary(preset, panel);
        let active_count = visible
            .iter()
            .filter(|index| **index != usize::MAX && models[**index].is_some())
            .count();
        let slots = split_primary_area(panel, active_count);
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
            self.widgets
                .render_rect_with_inventory(model, preset, slot, theme, buffer, inventory)?;
            if length < rendered.len() {
                rendered[length] = Some(model.visual().widget_id());
                length += 1;
            }
        }

        for pair in slots[..active_count].windows(2) {
            let divider_y = pair[0].bottom();
            if divider_y < panel.bottom() {
                draw_rule(
                    panel.x,
                    divider_y,
                    panel.width,
                    theme.border,
                    theme.surface,
                    buffer,
                );
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
        DashboardPreset::Compact => [0, 1, 2, 3, 4],
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

    let card_count = count.min(PRIMARY_COUNT);
    let secondary_count = (card_count - 1) as u16;
    let gaps = PRIMARY_CARD_GAP.saturating_mul(secondary_count);
    let usable_height = total_height.saturating_sub(gaps);
    let minimum_height = PRIMARY_CARD_MINIMUMS
        .iter()
        .take(card_count)
        .copied()
        .sum::<u16>();
    let extra = usable_height.saturating_sub(minimum_height);
    let extra_base = extra / secondary_count.max(1);
    let extra_remainder = extra % secondary_count.max(1);
    let mut heights = [0; PRIMARY_COUNT];
    for (index, height) in heights.iter_mut().enumerate().take(card_count) {
        let additional = if index == 0 {
            0
        } else {
            extra_base.saturating_add(u16::from(((index - 1) as u16) < extra_remainder))
        };
        *height = PRIMARY_CARD_MINIMUMS[index].saturating_add(additional);
    }
    heights
}

fn primary_stack_fits(total_height: u16, count: usize) -> bool {
    if count == 0 {
        return true;
    }
    let card_count = count.min(PRIMARY_COUNT);
    let secondary_count = card_count.saturating_sub(1) as u16;
    let required = PRIMARY_CARD_MINIMUMS
        .iter()
        .take(card_count)
        .copied()
        .sum::<u16>()
        .saturating_add(PRIMARY_CARD_GAP.saturating_mul(secondary_count));
    total_height >= required
}

fn render_header(
    area: Rect,
    preset: DashboardPreset,
    state: DashboardState,
    live: bool,
    theme: &Theme,
    buffer: &mut Buffer,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    buffer.set_style(area, Style::default().bg(theme.background));
    let right = if live {
        format!("{}  ·  LIVE METRICS", preset_label(preset))
    } else {
        format!("{}  ·  {}", preset_label(preset), state_label(state))
    };
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

#[allow(clippy::too_many_arguments)]
fn render_auxiliary(
    region: DashboardRegion,
    area: Rect,
    state: DashboardState,
    gpu_available: bool,
    inventory: &super::HardwareInventory,
    system_context: Option<&DashboardV2SystemContext>,
    live_mode: bool,
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
        DashboardRegion::SecondaryMetrics => {
            render_system_context(content, system_context, live_mode, theme, buffer)
        }
        DashboardRegion::Charts => render_temperatures(
            content,
            state,
            gpu_available,
            inventory,
            live_mode,
            theme,
            buffer,
        ),
        DashboardRegion::Processes => {
            render_processes(content, state, system_context, live_mode, theme, buffer)
        }
        DashboardRegion::Alerts => {
            render_events(content, state, system_context, live_mode, theme, buffer)
        }
        _ => {}
    }
}

#[allow(clippy::too_many_arguments)]
fn render_diagnostics_panel(
    outer: Rect,
    temperatures: Option<Rect>,
    processes: Option<Rect>,
    events: Option<Rect>,
    state: DashboardState,
    gpu_available: bool,
    inventory: &super::HardwareInventory,
    system_context: Option<&DashboardV2SystemContext>,
    live_mode: bool,
    theme: &Theme,
    buffer: &mut Buffer,
) {
    if outer.width == 0 || outer.height == 0 {
        return;
    }
    buffer.set_style(outer, Style::default().bg(theme.surface));
    draw_panel_frame(outer, theme.border, theme.surface, buffer);
    let content = inset(outer, 1, 1);
    if content.width == 0 || content.height == 0 {
        return;
    }

    let regions = diagnostic_sections(
        outer,
        [
            temperatures.is_some(),
            processes.is_some(),
            events.is_some(),
        ],
        live_mode,
    );
    for (index, region) in regions.into_iter().enumerate() {
        let Some(region) = region else { continue };
        if index > 0 {
            draw_rule(
                content.x,
                region.y.saturating_sub(1),
                content.width,
                theme.border,
                theme.surface,
                buffer,
            );
        }
        match index {
            0 => render_temperatures(
                region,
                state,
                gpu_available,
                inventory,
                live_mode,
                theme,
                buffer,
            ),
            1 => render_processes(region, state, system_context, live_mode, theme, buffer),
            _ => render_events(region, state, system_context, live_mode, theme, buffer),
        }
    }
}

fn diagnostic_sections(outer: Rect, active: [bool; 3], live_mode: bool) -> [Option<Rect>; 3] {
    let content = inset(outer, 1, 1);
    if content.width == 0 || content.height == 0 {
        return [None; 3];
    }

    let active_count = active.iter().filter(|value| **value).count();
    if active_count == 0 {
        return [None; 3];
    }

    // Headers plus their first useful row are the architectural minimums.
    // Empty live events receive only their honest status line; the released
    // space goes to temperatures and processes.
    let minimums = [4_u16, 5, 3];
    let weights = if live_mode { [4_u16, 6, 0] } else { [4, 5, 3] };
    let gaps = active_count.saturating_sub(1) as u16;
    let usable = content.height.saturating_sub(gaps);
    let minimum_total = active
        .iter()
        .enumerate()
        .filter_map(|(index, is_active)| is_active.then_some(minimums[index]))
        .sum::<u16>();

    let mut heights = [0_u16; 3];
    if usable >= minimum_total {
        for (index, is_active) in active.iter().copied().enumerate() {
            if is_active {
                heights[index] = minimums[index];
            }
        }
        let extra = usable - minimum_total;
        let flexible_weight = active
            .iter()
            .enumerate()
            .filter(|(index, is_active)| **is_active && weights[*index] > 0)
            .map(|(index, _)| weights[index])
            .sum::<u16>();
        if flexible_weight > 0 {
            let mut distributed = 0_u16;
            for (index, is_active) in active.iter().copied().enumerate() {
                if !is_active || weights[index] == 0 {
                    continue;
                }
                let share = extra
                    .saturating_mul(weights[index])
                    .checked_div(flexible_weight)
                    .unwrap_or(0);
                heights[index] = heights[index].saturating_add(share);
                distributed = distributed.saturating_add(share);
            }
            let remainder = extra.saturating_sub(distributed);
            if let Some(index) = active
                .iter()
                .enumerate()
                .find_map(|(index, is_active)| (*is_active && weights[index] > 0).then_some(index))
            {
                heights[index] = heights[index].saturating_add(remainder);
            }
        }
    } else {
        // The normal layouts fit their minimums. This branch only protects
        // unusual resize boundaries while keeping every active section valid.
        let mut remaining = content.height;
        let mut remaining_sections = active_count as u16;
        for (index, is_active) in active.iter().copied().enumerate() {
            if !is_active {
                continue;
            }
            let height = remaining
                .saturating_sub(remaining_sections.saturating_sub(1))
                .max(1);
            heights[index] = height;
            remaining = remaining.saturating_sub(height);
            remaining_sections = remaining_sections.saturating_sub(1);
        }
    }

    let mut result = [None; 3];
    let mut cursor = content.y;
    let last_active = active.iter().rposition(|is_active| *is_active);
    for (index, is_active) in active.iter().copied().enumerate() {
        if !is_active {
            continue;
        }
        let height = heights[index].min(content.bottom().saturating_sub(cursor));
        if height == 0 {
            continue;
        }
        result[index] = Some(Rect::new(content.x, cursor, content.width, height));
        let gap = (last_active != Some(index)) as u16;
        cursor = cursor.saturating_add(height).saturating_add(
            gap.min(
                content
                    .bottom()
                    .saturating_sub(cursor.saturating_add(height)),
            ),
        );
    }
    result
}

fn render_system_context(
    area: Rect,
    system_context: Option<&DashboardV2SystemContext>,
    live_mode: bool,
    theme: &Theme,
    buffer: &mut Buffer,
) {
    panel_heading(area, "SYSTEM CONTEXT", theme.info, theme, buffer);
    let has_live_context = system_context.is_some_and(|context| {
        context.physical_cores.is_some()
            || context.logical_threads.is_some()
            || context.total_memory_gib.is_some()
            || context.uptime_seconds.is_some()
            || !context.processes.is_empty()
    });
    if let Some(context) = system_context.filter(|_| !live_mode || has_live_context) {
        let cores = context
            .physical_cores
            .map_or_else(|| "—".to_owned(), |value| value.to_string());
        let threads = context
            .logical_threads
            .map_or_else(|| "—".to_owned(), |value| value.to_string());
        let memory = context
            .total_memory_gib
            .map_or_else(|| "—".to_owned(), |value| format!("{value:.1} GB memory"));
        let uptime = context
            .uptime_seconds
            .map_or_else(|| "—".to_owned(), format_uptime);
        let rows = [
            (format!("{cores} physical"), format!("{threads} logical")),
            (
                format!("{} RAM", memory.trim_end_matches(" memory")),
                format!("{uptime} uptime"),
            ),
            (
                format!("{} processes", context.processes.len()),
                "Pulse online".to_owned(),
            ),
        ];
        if area.width >= 30 && area.height >= 6 {
            for (index, (left, right)) in rows.into_iter().enumerate() {
                write_context_pair(area, 2 + index as u16, &left, &right, theme, buffer);
            }
        } else {
            let compact = [
                format!("{} cores · {} threads", cores, threads),
                format!("{} · {} uptime", memory, uptime),
                format!("{} processes · Pulse online", context.processes.len()),
            ];
            for (index, row) in compact.into_iter().enumerate() {
                write_panel_row(
                    area,
                    2 + index as u16,
                    &row,
                    theme.text_muted,
                    theme,
                    buffer,
                );
            }
        }
    } else if live_mode {
        write_panel_row(area, 2, "not connected", theme.text_muted, theme, buffer);
    } else {
        let rows = [
            ("8 physical", "16 logical"),
            ("16 GB RAM", "3d 04h uptime"),
            ("Pulse online", "history ready"),
        ];
        if area.width >= 30 && area.height >= 6 {
            for (index, (left, right)) in rows.into_iter().enumerate() {
                write_context_pair(area, 2 + index as u16, left, right, theme, buffer);
            }
        } else {
            for (index, (left, right)) in rows.into_iter().enumerate() {
                let row = format!("{left} · {right}");
                write_panel_row(
                    area,
                    2 + index as u16,
                    &row,
                    theme.text_muted,
                    theme,
                    buffer,
                );
            }
        }
    }
}

fn write_context_pair(
    area: Rect,
    row: u16,
    left: &str,
    right: &str,
    theme: &Theme,
    buffer: &mut Buffer,
) {
    if row >= area.height || area.width < 4 {
        return;
    }
    let gap = 2_u16.min(area.width.saturating_sub(2));
    let column_width = area.width.saturating_sub(gap) / 2;
    let y = area.y.saturating_add(row);
    let style = Style::default().fg(theme.text_muted).bg(theme.surface);
    buffer.set_stringn(area.x, y, left, usize::from(column_width), style);
    let right_x = area.x.saturating_add(column_width).saturating_add(gap);
    buffer.set_stringn(
        right_x,
        y,
        right,
        usize::from(area.right().saturating_sub(right_x)),
        style,
    );
}

fn render_temperatures(
    area: Rect,
    state: DashboardState,
    gpu_available: bool,
    inventory: &super::HardwareInventory,
    live: bool,
    theme: &Theme,
    buffer: &mut Buffer,
) {
    panel_heading(area, "TEMPERATURES", theme.warning, theme, buffer);
    if !inventory.temperatures.is_empty() {
        for (index, sensor) in inventory.temperatures.iter().enumerate() {
            render_temperature_row(
                area,
                2 + index as u16,
                &sensor.label,
                sensor.celsius.map(|value| value.clamp(0.0, 100.0) as u16),
                theme,
                buffer,
            );
        }
        return;
    }
    if live {
        write_panel_row(area, 2, "no live sensors", theme.text_muted, theme, buffer);
        return;
    }
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
    let value_width = u16::try_from(value_text.chars().count()).unwrap_or(area.width);
    let label_width = area
        .width
        .saturating_mul(38)
        .saturating_div(100)
        .max(8)
        .min(area.width.saturating_sub(value_width.saturating_add(2)));
    let bar_start = area.x.saturating_add(label_width.saturating_add(1));
    let bar_width = area
        .width
        .saturating_sub(label_width.saturating_add(value_width).saturating_add(2));
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
    let value_x = area.right().saturating_sub(value_width);
    buffer.set_stringn(
        value_x,
        y,
        value_text,
        usize::from(area.right().saturating_sub(value_x)),
        Style::default().fg(theme.text).bg(theme.surface),
    );
}

fn render_processes(
    area: Rect,
    state: DashboardState,
    system_context: Option<&DashboardV2SystemContext>,
    live_mode: bool,
    theme: &Theme,
    buffer: &mut Buffer,
) {
    panel_heading(area, "TOP PROCESSES", theme.primary, theme, buffer);
    if area.height > 2 {
        render_process_columns_header(area, theme, buffer);
    }
    if let Some(context) = system_context {
        if context.processes.is_empty() {
            write_panel_row(area, 3, "not connected", theme.text_muted, theme, buffer);
            return;
        }
        for (index, process) in context.processes.iter().take(5).enumerate() {
            render_process_row(
                area,
                3 + index as u16,
                &process.name,
                process.cpu_percent,
                process.memory_bytes,
                theme,
                buffer,
            );
        }
        return;
    }
    if live_mode {
        write_panel_row(area, 3, "not connected", theme.text_muted, theme, buffer);
        return;
    }
    let load = match state {
        DashboardState::Calm => [12_u16, 8, 5],
        DashboardState::Active => [34, 22, 14],
        DashboardState::Busy => [68, 51, 36],
        DashboardState::Critical => [91, 78, 64],
    };
    let processes = ["sysmon", "browser", "compiler"];
    for (index, (name, cpu)) in processes.into_iter().zip(load).enumerate() {
        render_process_row(
            area,
            3 + index as u16,
            name,
            f32::from(cpu),
            u64::from((cpu / 2).max(1)) * 1024 * 1024,
            theme,
            buffer,
        );
    }
}

fn render_process_columns_header(area: Rect, theme: &Theme, buffer: &mut Buffer) {
    let memory_width = 8_u16.min(area.width);
    let cpu_width = 7_u16.min(area.width.saturating_sub(memory_width));
    let cpu_x = area.right().saturating_sub(memory_width + cpu_width);
    let memory_x = area.right().saturating_sub(memory_width);
    buffer.set_stringn(
        area.x,
        area.y.saturating_add(2),
        "PROCESS",
        usize::from(cpu_x.saturating_sub(area.x)),
        Style::default().fg(theme.text_muted).bg(theme.surface),
    );
    buffer.set_stringn(
        cpu_x,
        area.y.saturating_add(2),
        "CPU",
        usize::from(cpu_width),
        Style::default().fg(theme.text_muted).bg(theme.surface),
    );
    buffer.set_stringn(
        memory_x,
        area.y.saturating_add(2),
        "MEM",
        usize::from(memory_width),
        Style::default().fg(theme.text_muted).bg(theme.surface),
    );
}

fn render_process_row(
    area: Rect,
    row: u16,
    name: &str,
    cpu_percent: f32,
    memory_bytes: u64,
    theme: &Theme,
    buffer: &mut Buffer,
) {
    if row >= area.height {
        return;
    }
    let memory_width = 8_u16.min(area.width);
    let cpu_width = 7_u16.min(area.width.saturating_sub(memory_width));
    let cpu_x = area.right().saturating_sub(memory_width + cpu_width);
    let memory_x = area.right().saturating_sub(memory_width);
    let y = area.y.saturating_add(row);
    buffer.set_stringn(
        area.x,
        y,
        name,
        usize::from(cpu_x.saturating_sub(area.x).saturating_sub(1)),
        Style::default().fg(theme.text).bg(theme.surface),
    );
    buffer.set_stringn(
        cpu_x,
        y,
        format!("{cpu_percent:>5.1}%"),
        usize::from(cpu_width),
        Style::default().fg(theme.text_muted).bg(theme.surface),
    );
    buffer.set_stringn(
        memory_x,
        y,
        format_bytes(memory_bytes as f64),
        usize::from(memory_width),
        Style::default().fg(theme.text_muted).bg(theme.surface),
    );
}

fn render_events(
    area: Rect,
    state: DashboardState,
    system_context: Option<&DashboardV2SystemContext>,
    live_mode: bool,
    theme: &Theme,
    buffer: &mut Buffer,
) {
    panel_heading(area, "RECENT EVENTS", theme.secondary, theme, buffer);
    if live_mode || system_context.is_some() {
        write_panel_row(area, 2, "no live events", theme.text_muted, theme, buffer);
        return;
    }
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

fn format_uptime(seconds: u64) -> String {
    let days = seconds / 86_400;
    let hours = (seconds % 86_400) / 3_600;
    let minutes = (seconds % 3_600) / 60;
    if days > 0 {
        format!("{days}d {hours:02}h")
    } else if hours > 0 {
        format!("{hours}h {minutes:02}m")
    } else {
        format!("{minutes}m")
    }
}

fn format_bytes(value: f64) -> String {
    let value = value.max(0.0);
    if value >= 1024.0 * 1024.0 * 1024.0 {
        format!("{:.1} GB", value / 1024.0 / 1024.0 / 1024.0)
    } else if value >= 1024.0 * 1024.0 {
        format!("{:.1} MB", value / 1024.0 / 1024.0)
    } else if value >= 1024.0 {
        format!("{:.1} KB", value / 1024.0)
    } else {
        format!("{value:.0} B")
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
                    content.clone(),
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
        assert!((6..=7).contains(&heights[0]));
        assert!(heights[0] >= heights[1]);
        assert!(heights[1..].iter().all(|height| *height >= 5));
        assert_eq!(heights.iter().sum::<u16>() + PRIMARY_CARD_GAP * 4, 37);
        for pair in slots.windows(2) {
            assert_eq!(pair[0].bottom() + PRIMARY_CARD_GAP, pair[1].y);
        }
    }

    #[test]
    fn primary_column_degrades_lower_priority_cards_without_invalid_slots() {
        let area = Rect::new(0, 0, 48, 20);
        let visible = visible_primary(DashboardPreset::Widescreen, area);
        assert_eq!(visible, [0, 1, 2, usize::MAX, usize::MAX]);

        let slots = split_primary_area(area, 3);
        assert!(slots[0].height >= slots[1].height);
        assert!(
            slots[..3]
                .iter()
                .all(|slot| slot.width > 0 && slot.height >= 5)
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
            let expected_count = match height {
                24 => 3,
                32 => 5,
                40 => 5,
                _ => unreachable!(),
            };
            assert_eq!(count, expected_count);
            let slots = split_primary_area(
                Rect::new(
                    body.x() as u16,
                    body.y() as u16,
                    body.width() as u16,
                    body.height() as u16,
                ),
                count,
            );
            let minimums = PRIMARY_CARD_MINIMUMS;
            for (index, slot) in slots[..count].iter().enumerate() {
                assert!(slot.width > 0);
                assert!(slot.height >= minimums[index]);
            }
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

    #[test]
    fn live_diagnostics_give_empty_events_only_their_status_row() {
        let outer = Rect::new(0, 0, 44, 35);
        let sections = diagnostic_sections(outer, [true, true, true], true);
        let temperatures = sections[0].unwrap();
        let processes = sections[1].unwrap();
        let events = sections[2].unwrap();

        assert_eq!(events.height, 3);
        assert!(temperatures.height > events.height);
        assert!(processes.height > events.height);
        assert_eq!(temperatures.bottom() + 1, processes.y);
        assert_eq!(processes.bottom() + 1, events.y);
        assert!(events.bottom() <= outer.bottom());
    }

    #[test]
    fn live_context_never_falls_back_to_demo_diagnostics() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 160, 40));
        let content = DashboardV2Content::new([None; PRIMARY_COUNT], input(), DashboardState::Calm)
            .with_live_mode(true);
        DashboardV2Renderer::new()
            .render(
                DashboardPreset::Standard,
                DashboardContext::default(),
                content,
                Duration::ZERO,
                true,
                &Theme::pulse(),
                &mut buffer,
            )
            .unwrap();
        let text: String = buffer.content.iter().map(|cell| cell.symbol()).collect();
        assert!(text.contains("not connected"));
        assert!(text.contains("no live events"));
        assert!(!text.contains("sysmon"));
        assert!(!text.contains("browser"));
        assert!(!text.contains("compiler"));
    }
}
