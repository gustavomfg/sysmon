#![allow(dead_code)]

use std::time::Duration;

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
};

use crate::{
    pulse_v2::{
        ColorRasterFrame, FieldDimensions, FieldError, NormalizedPulseInput, PulseV2Engine,
        PulseV2Preset, PulseV2Rasterizer, SemanticColorField,
    },
    system::info::SystemInfo,
    theme::Theme,
};

use super::{
    DashboardBounds, DashboardContext, DashboardDimensions, DashboardEngine, DashboardError,
    DashboardFrame, DashboardPreset, DashboardRegion, DashboardState, DashboardWidgetId,
    PulseBehavior, TerminalRenderError, TerminalRenderer, WidgetRenderModel,
};

const PRIMARY_COUNT: usize = 5;
const PRIMARY_CARD_GAP: u16 = 1;
const PRIMARY_CPU_MINIMUM: u16 = 7;
const PRIMARY_CARD_MINIMUMS: [u16; PRIMARY_COUNT] = [PRIMARY_CPU_MINIMUM, 4, 4, 4, 4];
const PULSE_TITLE: &str = "SYSTEM PULSE";
const HEADER_TITLE: &str = "SYSMON";
const HEADER_CENTER: &str = "SYSTEM MONITOR";

#[derive(Debug, Clone, PartialEq)]
pub struct DashboardProcessSummary {
    pub pid: u32,
    pub name: String,
    pub cpu_percent: f32,
    pub memory_bytes: u64,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct DashboardV2SystemContext {
    pub host_name: Option<String>,
    pub os_name: Option<String>,
    pub kernel: Option<String>,
    pub physical_cores: Option<usize>,
    pub logical_threads: Option<usize>,
    pub total_memory_gib: Option<f64>,
    pub available_memory_gib: Option<f64>,
    pub uptime_seconds: Option<u64>,
    pub load_average: Option<(f64, f64, f64)>,
    pub process_count: Option<usize>,
    pub processes: Vec<DashboardProcessSummary>,
}

impl DashboardV2SystemContext {
    pub fn from_system_info(snapshot: &SystemInfo) -> Self {
        let mut processes = snapshot
            .processes_list
            .iter()
            .map(|process| DashboardProcessSummary {
                pid: process.pid,
                name: process.name.clone(),
                cpu_percent: process.cpu_usage,
                memory_bytes: if process.memory.is_finite() && process.memory >= 0.0 {
                    (process.memory * 1024.0 * 1024.0) as u64
                } else {
                    0
                },
            })
            .collect::<Vec<_>>();
        processes.sort_unstable_by(|left, right| {
            right
                .cpu_percent
                .total_cmp(&left.cpu_percent)
                .then_with(|| right.memory_bytes.cmp(&left.memory_bytes))
                .then_with(|| left.name.cmp(&right.name))
        });

        Self {
            host_name: (!snapshot.host_name.is_empty()).then(|| snapshot.host_name.clone()),
            os_name: (!snapshot.os_name.is_empty()).then(|| snapshot.os_name.clone()),
            kernel: (!snapshot.kernel.is_empty()).then(|| snapshot.kernel.clone()),
            physical_cores: snapshot.cpu_physical_cores,
            logical_threads: Some(snapshot.cpu_logical_threads.max(snapshot.cpu_cores)),
            total_memory_gib: Some(snapshot.total_ram).filter(|value| value.is_finite()),
            available_memory_gib: Some(snapshot.available_ram)
                .filter(|value| value.is_finite() && *value >= 0.0),
            uptime_seconds: Some(snapshot.uptime),
            load_average: Some(snapshot.load_average),
            process_count: Some(snapshot.processes),
            processes,
        }
    }
}

/// All semantic inputs needed by the V2 dashboard compositor.
///
/// The compositor deliberately accepts prepared widget models. It never reaches
/// into collectors, snapshots, or the V1 application state.
#[derive(Debug, Clone, PartialEq)]
pub struct DashboardV2Content {
    primary: [Option<WidgetRenderModel>; PRIMARY_COUNT],
    pulse_input: NormalizedPulseInput,
    pulse_behavior: Option<PulseBehavior>,
    state: DashboardState,
    inventory: super::HardwareInventory,
    system_context: Option<DashboardV2SystemContext>,
    live_mode: bool,
    focused_widget: Option<DashboardWidgetId>,
    expanded_widget: Option<DashboardWidgetId>,
    data_stale: bool,
    sample_age: Option<Duration>,
    recent_events: Vec<String>,
}

impl DashboardV2Content {
    pub fn new(
        primary: [Option<WidgetRenderModel>; PRIMARY_COUNT],
        pulse_input: NormalizedPulseInput,
        state: DashboardState,
    ) -> Self {
        Self {
            primary,
            pulse_input,
            pulse_behavior: None,
            state,
            inventory: super::HardwareInventory::default(),
            system_context: None,
            live_mode: false,
            focused_widget: None,
            expanded_widget: None,
            data_stale: false,
            sample_age: None,
            recent_events: Vec::new(),
        }
    }

    pub fn primary(&self) -> [Option<WidgetRenderModel>; PRIMARY_COUNT] {
        self.primary
    }

    pub fn pulse_input(&self) -> NormalizedPulseInput {
        self.pulse_input
    }

    pub fn with_pulse_behavior(mut self, behavior: PulseBehavior) -> Self {
        self.pulse_behavior = Some(behavior);
        self
    }

    pub fn pulse_behavior(&self) -> Option<PulseBehavior> {
        self.pulse_behavior
    }

    pub fn state(&self) -> DashboardState {
        self.state
    }

    pub fn with_inventory(mut self, inventory: super::HardwareInventory) -> Self {
        self.inventory = inventory;
        self
    }

    pub fn inventory(&self) -> &super::HardwareInventory {
        &self.inventory
    }

    pub fn with_system_context(mut self, context: DashboardV2SystemContext) -> Self {
        self.system_context = Some(context);
        self
    }

    pub fn system_context(&self) -> Option<&DashboardV2SystemContext> {
        self.system_context.as_ref()
    }

    pub fn with_live_mode(mut self, live_mode: bool) -> Self {
        self.live_mode = live_mode;
        self
    }

    pub const fn live_mode(&self) -> bool {
        self.live_mode
    }

    pub fn with_focused_widget(mut self, focused_widget: Option<DashboardWidgetId>) -> Self {
        self.focused_widget = focused_widget;
        self
    }

    pub const fn focused_widget(&self) -> Option<DashboardWidgetId> {
        self.focused_widget
    }

    pub fn with_expanded_widget(mut self, expanded_widget: Option<DashboardWidgetId>) -> Self {
        self.expanded_widget = expanded_widget;
        self
    }

    pub const fn expanded_widget(&self) -> Option<DashboardWidgetId> {
        self.expanded_widget
    }

    pub const fn with_data_stale(mut self, data_stale: bool) -> Self {
        self.data_stale = data_stale;
        self
    }

    pub const fn data_stale(&self) -> bool {
        self.data_stale
    }

    pub const fn with_sample_age(mut self, sample_age: Option<Duration>) -> Self {
        self.sample_age = sample_age;
        self
    }

    pub const fn sample_age(&self) -> Option<Duration> {
        self.sample_age
    }

    pub fn with_recent_events(mut self, recent_events: Vec<String>) -> Self {
        self.recent_events = recent_events;
        self
    }

    pub fn recent_events(&self) -> &[String] {
        &self.recent_events
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DashboardV2RenderError {
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
pub struct DashboardV2RenderSummary {
    frame: DashboardFrame,
    rendered_primary: [Option<DashboardWidgetId>; PRIMARY_COUNT],
    primary_len: usize,
    pulse_rendered: bool,
}

impl DashboardV2RenderSummary {
    pub const fn frame(self) -> DashboardFrame {
        self.frame
    }

    pub fn rendered_primary(self) -> impl Iterator<Item = DashboardWidgetId> {
        self.rendered_primary
            .into_iter()
            .take(self.primary_len)
            .flatten()
    }

    pub const fn pulse_rendered(self) -> bool {
        self.pulse_rendered
    }
}

/// Orchestrates the official V2 layout, widget and Pulse pipelines.
///
/// The application frontend supplies prepared visual models and the renderer
/// owns only terminal composition.
#[derive(Debug, Default)]
pub struct DashboardV2Renderer {
    dashboard: DashboardEngine,
    pulse: PulseV2Engine,
    widgets: TerminalRenderer,
    pulse_cache: Option<PulseRasterCache>,
}

#[derive(Debug, Clone, PartialEq)]
struct PulseRasterCache {
    dimensions: FieldDimensions,
    input: NormalizedPulseInput,
    tick: u128,
    reduced_motion: bool,
    theme: Theme,
    raster: ColorRasterFrame,
}

impl DashboardV2Renderer {
    pub fn new() -> Self {
        Self {
            dashboard: DashboardEngine::new(),
            pulse: PulseV2Engine::new(),
            widgets: TerminalRenderer::new(),
            pulse_cache: None,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn render(
        &mut self,
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
                &content,
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
                content
                    .pulse_behavior()
                    .map(PulseBehavior::input)
                    .unwrap_or_else(|| content.pulse_input()),
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
                content.expanded_widget(),
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
                content.inventory(),
                content.system_context(),
                content.live_mode(),
                content.recent_events(),
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
            render_diagnostics_panel(outer, diagnostics, &content, theme, buffer);
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
        &self,
        area: Rect,
        preset: DashboardPreset,
        models: [Option<WidgetRenderModel>; PRIMARY_COUNT],
        expanded_widget: Option<DashboardWidgetId>,
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
            draw_titled_panel_frame(
                area,
                "METRICS",
                theme.primary,
                theme.border,
                theme.surface,
                buffer,
            );
        }
        let panel = if framed { inset(area, 1, 1) } else { area };
        let visible = visible_primary(preset, panel, expanded_widget);
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
        &mut self,
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
        draw_titled_panel_frame(
            area,
            PULSE_TITLE,
            theme.text_muted,
            theme.border,
            theme.background,
            buffer,
        );
        let content = inset(area, 1, 1);
        if content.width == 0 || content.height == 0 {
            return Ok(false);
        }

        let raster_area = Rect::new(content.x, content.y, content.width, content.height);
        if raster_area.width == 0 || raster_area.height == 0 {
            return Ok(false);
        }

        let pulse_theme = pulse_theme(theme);
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
        let tick = if reduced_motion {
            0
        } else {
            elapsed.as_millis() / 100
        };
        let cached = self.pulse_cache.as_ref().and_then(|cache| {
            (cache.dimensions == dimensions
                && cache.input == input
                && cache.tick == tick
                && cache.reduced_motion == reduced_motion
                && cache.theme == pulse_theme)
                .then_some(&cache.raster)
        });
        if let Some(raster) = cached {
            draw_raster(raster_area, raster, theme, buffer);
            return Ok(true);
        }
        let pulse_preset = PulseV2Preset::from_dimensions(dimensions);
        let frame = self.pulse.generate_with_preset(
            dimensions,
            input,
            pulse_preset,
            elapsed,
            reduced_motion,
        )?;
        let colors = SemanticColorField::from_lighting(frame.lighting(), input, &pulse_theme)?;
        let raster = if use_braille {
            PulseV2Rasterizer::braille_colored(frame.lighting(), &colors)?
        } else {
            PulseV2Rasterizer::unicode_colored(frame.lighting(), &colors)?
        };
        self.pulse_cache = Some(PulseRasterCache {
            dimensions,
            input,
            tick,
            reduced_motion,
            theme: pulse_theme,
            raster,
        });
        let raster = &self
            .pulse_cache
            .as_ref()
            .expect("pulse cache was just populated")
            .raster;
        draw_raster(raster_area, raster, theme, buffer);
        Ok(true)
    }
}

fn pulse_theme(theme: &Theme) -> Theme {
    if *theme != Theme::pulse() {
        return *theme;
    }

    // Keep the dashboard's terminal palette restrained while preserving the
    // original Pulse identity in the central visualization: violet energy,
    // cyan flow, and warm colors only for pressure.
    let mut pulse = *theme;
    pulse.primary = Color::Rgb(0xA8, 0x55, 0xF7);
    pulse.secondary = Color::Rgb(0x22, 0xD3, 0xEE);
    pulse.accent = Color::Rgb(0x7C, 0x3A, 0xED);
    pulse.info = Color::Rgb(0x38, 0xBD, 0xF8);
    pulse.warning = Color::Rgb(0xF5, 0x9E, 0x0B);
    pulse.danger = Color::Rgb(0xEF, 0x44, 0x44);
    pulse.pulse_low = Color::Rgb(0x6D, 0x28, 0xD9);
    pulse.pulse_medium = Color::Rgb(0xA8, 0x55, 0xF7);
    pulse.pulse_high = Color::Rgb(0x22, 0xD3, 0xEE);
    pulse
}

fn visible_primary(
    preset: DashboardPreset,
    area: Rect,
    expanded_widget: Option<DashboardWidgetId>,
) -> [usize; PRIMARY_COUNT] {
    let order = match preset {
        DashboardPreset::Compact => [0, 1, 2, 3, 4],
        DashboardPreset::Standard
        | DashboardPreset::Widescreen
        | DashboardPreset::LargeWidescreen => [0, 1, 2, 3, 4],
    };
    if let Some(index) = expanded_widget.and_then(primary_index) {
        return [index, usize::MAX, usize::MAX, usize::MAX, usize::MAX];
    }
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

const fn primary_index(widget: DashboardWidgetId) -> Option<usize> {
    match widget {
        DashboardWidgetId::Cpu => Some(0),
        DashboardWidgetId::Memory => Some(1),
        DashboardWidgetId::Gpu => Some(2),
        DashboardWidgetId::Network => Some(3),
        DashboardWidgetId::Storage => Some(4),
        _ => None,
    }
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
    content: &DashboardV2Content,
    theme: &Theme,
    buffer: &mut Buffer,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    buffer.set_style(area, Style::default().bg(theme.background));
    let live = content.live_mode();
    let preview = !live && content.primary().iter().any(Option::is_some);
    let status = if content.data_stale() {
        "STALE"
    } else if live {
        "LIVE"
    } else if preview {
        "PREVIEW"
    } else {
        "WAITING"
    };
    let focus = content
        .focused_widget()
        .map(widget_label)
        .map_or_else(String::new, |label| format!("  ·  FOCUS {label}"));
    let status = match content.sample_age() {
        Some(age) if live => format!("{status} {}", format_sample_age(age)),
        _ => status.to_owned(),
    };
    let right = format!("{}  ·  {status}{focus}", preset_label(preset));
    let center = if live || preview {
        content
            .system_context()
            .and_then(|context| context.host_name.as_deref())
            .filter(|host| !host.is_empty())
            .map_or_else(
                || HEADER_CENTER.to_owned(),
                |host| format!("{HEADER_CENTER}  ·  {host}"),
            )
    } else {
        "waiting for first sample".to_owned()
    };
    buffer.set_stringn(
        area.x,
        area.y,
        HEADER_TITLE,
        usize::from(area.width),
        Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
    );
    let right_width = u16::try_from(right.chars().count())
        .unwrap_or(area.width)
        .min(area.width);
    let right_x = area.right().saturating_sub(right_width);
    let left_end = area
        .x
        .saturating_add(u16::try_from(HEADER_TITLE.chars().count()).unwrap_or(area.width));
    let center_width = u16::try_from(center.chars().count())
        .unwrap_or(area.width)
        .min(area.width);
    let center_x = area.x.saturating_add((area.width - center_width) / 2);
    let center_fits = center_x >= left_end.saturating_add(2)
        && center_x.saturating_add(center_width).saturating_add(2) <= right_x;
    if center_fits {
        buffer.set_stringn(
            center_x,
            area.y,
            center,
            usize::from(center_width),
            Style::default().fg(theme.text),
        );
    }
    if right_width > 0 {
        buffer.set_stringn(
            right_x,
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
        "motion: reduced"
    } else {
        "motion: live"
    };
    let text = format!(
        "{}  ·  {}  Tab/jk focus  e expand  / search  Ctrl+P commands  Esc back  Q quit",
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
    inventory: &super::HardwareInventory,
    system_context: Option<&DashboardV2SystemContext>,
    live_mode: bool,
    recent_events: &[String],
    theme: &Theme,
    buffer: &mut Buffer,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    buffer.set_style(area, Style::default().bg(theme.surface));
    let (title, accent) = match region {
        DashboardRegion::SecondaryMetrics => ("SYSTEM", theme.info),
        _ => return,
    };
    draw_titled_panel_frame(area, title, accent, theme.border, theme.surface, buffer);
    let content = inset(area, 1, 1);
    if content.width == 0 || content.height == 0 {
        return;
    }
    match region {
        DashboardRegion::SecondaryMetrics => {
            render_system_context(content, system_context, live_mode, theme, buffer)
        }
        DashboardRegion::Charts => {
            render_temperatures(content, inventory, live_mode, theme, buffer)
        }
        DashboardRegion::Processes => {
            render_processes(content, system_context, live_mode, theme, buffer)
        }
        DashboardRegion::Alerts => render_events(content, recent_events, live_mode, theme, buffer),
        _ => {}
    }
}

fn render_diagnostics_panel(
    outer: Rect,
    diagnostics: [Option<Rect>; 3],
    dashboard: &DashboardV2Content,
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
        diagnostics.map(|region| region.is_some()),
        dashboard.live_mode(),
        dashboard.inventory().temperatures.len(),
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
                dashboard.inventory(),
                dashboard.live_mode(),
                theme,
                buffer,
            ),
            1 => render_processes(
                region,
                dashboard.system_context(),
                dashboard.live_mode(),
                theme,
                buffer,
            ),
            _ => render_events(
                region,
                dashboard.recent_events(),
                dashboard.live_mode(),
                theme,
                buffer,
            ),
        }
    }
}

fn diagnostic_sections(
    outer: Rect,
    active: [bool; 3],
    live_mode: bool,
    temperature_count: usize,
) -> [Option<Rect>; 3] {
    let content = inset(outer, 1, 1);
    if content.width == 0 || content.height == 0 {
        return [None; 3];
    }

    let active_count = active.iter().filter(|value| **value).count();
    if active_count == 0 {
        return [None; 3];
    }

    // Temperature height follows its real row count: heading, rule, and one
    // row per sensor. Empty live telemetry still receives one honest status
    // row. The released space goes to processes while empty live events stay
    // at their compact status height.
    let temperature_rows = if live_mode {
        temperature_count.max(1)
    } else {
        3
    };
    let temperature_minimum =
        2_u16.saturating_add(u16::try_from(temperature_rows).unwrap_or(u16::MAX.saturating_sub(2)));
    let minimums = [temperature_minimum, 5, 3];
    let weights = if live_mode { [0_u16, 1, 0] } else { [4, 5, 3] };
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
    let has_live_context = system_context.is_some_and(|context| {
        context.host_name.is_some()
            || context.os_name.is_some()
            || context.kernel.is_some()
            || context.physical_cores.is_some()
            || context.logical_threads.is_some()
            || context.total_memory_gib.is_some()
            || context.available_memory_gib.is_some()
            || context.uptime_seconds.is_some()
            || context.load_average.is_some()
            || context.process_count.is_some()
            || !context.processes.is_empty()
    });
    if let Some(context) = system_context.filter(|_| !live_mode || has_live_context) {
        let host = context.host_name.as_deref().unwrap_or("—");
        let os = context.os_name.as_deref().unwrap_or("—");
        let kernel = context.kernel.as_deref().unwrap_or("—");
        let cpu = match (context.physical_cores, context.logical_threads) {
            (Some(physical), Some(logical)) => format!("{physical}C / {logical}T"),
            (Some(physical), None) => format!("{physical}C"),
            (None, Some(logical)) => format!("{logical}T"),
            _ => "—".to_owned(),
        };
        let memory = match (context.available_memory_gib, context.total_memory_gib) {
            (Some(available), Some(total)) => format!("{available:.1} / {total:.1} GiB"),
            (Some(available), None) => format!("{available:.1} GiB free"),
            (None, Some(total)) => format!("{total:.1} GiB total"),
            _ => "—".to_owned(),
        };
        let uptime = context
            .uptime_seconds
            .map_or_else(|| "—".to_owned(), format_uptime);
        let load = context.load_average.map_or_else(
            || "—".to_owned(),
            |(one, five, fifteen)| format!("{one:.2} {five:.2} {fifteen:.2}"),
        );
        let processes = context
            .process_count
            .unwrap_or(context.processes.len())
            .to_string();

        if area.width >= 34 && area.height >= 6 {
            let rows = [
                (("HOST", host), ("UP", uptime.as_str())),
                (("CPU", cpu.as_str()), ("LOAD", load.as_str())),
                (("RAM", memory.as_str()), ("PROC", processes.as_str())),
                (("OS", os), ("KERNEL", kernel)),
            ];
            for (index, ((left_label, left), (right_label, right))) in rows.into_iter().enumerate()
            {
                write_context_pair(
                    area,
                    2 + index as u16,
                    left_label,
                    left,
                    right_label,
                    right,
                    theme,
                    buffer,
                );
            }
        } else {
            let compact = [
                format!("HOST {host} · {os}"),
                format!("CPU {cpu} · LOAD {load}"),
                format!("RAM {memory} · UP {uptime}"),
                format!("PROC {processes} · KERNEL {kernel}"),
            ];
            for (index, row) in compact.into_iter().enumerate() {
                write_rich_panel_row(area, 2 + index as u16, &row, theme, buffer);
            }
        }
    } else if live_mode {
        write_panel_row(area, 2, "no system sample", theme.text_muted, theme, buffer);
    } else {
        write_panel_row(
            area,
            2,
            "waiting for first sample",
            theme.text_muted,
            theme,
            buffer,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn write_context_pair(
    area: Rect,
    row: u16,
    left_label: &str,
    left: &str,
    right_label: &str,
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
    write_context_cell(
        Rect::new(area.x, y, column_width, 1),
        left_label,
        left,
        theme,
        buffer,
    );
    let right_x = area.x.saturating_add(column_width).saturating_add(gap);
    write_context_cell(
        Rect::new(right_x, y, area.right().saturating_sub(right_x), 1),
        right_label,
        right,
        theme,
        buffer,
    );
}

fn write_context_cell(area: Rect, label: &str, value: &str, theme: &Theme, buffer: &mut Buffer) {
    if area.width < 2 || area.height == 0 {
        return;
    }
    let label_width = u16::try_from(label.chars().count())
        .unwrap_or(area.width)
        .min(area.width.saturating_sub(1));
    write_truncated(
        buffer,
        area.x,
        area.y,
        label,
        label_width,
        context_label_style(theme),
    );
    let value_x = area.x.saturating_add(label_width).saturating_add(1);
    write_truncated(
        buffer,
        value_x,
        area.y,
        value,
        area.right().saturating_sub(value_x),
        context_value_style(theme),
    );
}

fn write_rich_panel_row(area: Rect, row: u16, text: &str, theme: &Theme, buffer: &mut Buffer) {
    if row >= area.height {
        return;
    }
    write_rich_text(
        Rect::new(area.x, area.y.saturating_add(row), area.width, 1),
        text,
        context_label_style(theme),
        context_value_style(theme),
        theme,
        buffer,
    );
}

fn write_rich_text(
    area: Rect,
    text: &str,
    base_style: Style,
    value_style: Style,
    theme: &Theme,
    buffer: &mut Buffer,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    buffer.set_stringn(area.x, area.y, text, usize::from(area.width), base_style);

    let mut token_start = None;
    for (index, character) in text.char_indices() {
        if character.is_whitespace() || matches!(character, '·' | '/') {
            if let Some(start) = token_start.take() {
                overlay_context_value(area, text, start, index, value_style, theme, buffer);
            }
            if matches!(character, '·' | '/') {
                overlay_context_value(
                    area,
                    text,
                    index,
                    index + character.len_utf8(),
                    base_style,
                    theme,
                    buffer,
                );
            }
        } else if token_start.is_none() {
            token_start = Some(index);
        }
    }
    if let Some(start) = token_start {
        overlay_context_value(area, text, start, text.len(), value_style, theme, buffer);
    }
}

fn overlay_context_value(
    area: Rect,
    text: &str,
    start: usize,
    end: usize,
    style: Style,
    theme: &Theme,
    buffer: &mut Buffer,
) {
    let token = &text[start..end];
    if !is_context_value(token) && !is_status_value(token) {
        return;
    }
    let offset = u16::try_from(text[..start].chars().count()).unwrap_or(u16::MAX);
    if offset >= area.width {
        return;
    }
    let style = if token == "·" || token == "/" {
        style.fg(theme.text_muted)
    } else {
        style
    };
    buffer.set_stringn(
        area.x.saturating_add(offset),
        area.y,
        token,
        usize::from(area.width.saturating_sub(offset)),
        style,
    );
}

fn context_label_style(theme: &Theme) -> Style {
    Style::default().fg(theme.text_muted).bg(theme.surface)
}

fn context_value_style(theme: &Theme) -> Style {
    Style::default()
        .fg(theme.text)
        .bg(theme.surface)
        .add_modifier(Modifier::BOLD)
}

fn is_context_value(token: &str) -> bool {
    token.chars().any(|character| character.is_ascii_digit())
        || matches!(
            token,
            "B" | "B/s" | "GB" | "MB" | "KB" | "Hz" | "kHz" | "MHz" | "GHz" | "ms" | "s"
        )
}

fn is_status_value(token: &str) -> bool {
    matches!(token, "Pulse" | "online")
}

fn render_temperatures(
    area: Rect,
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
    write_panel_row(
        area,
        2,
        "waiting for first sample",
        theme.text_muted,
        theme,
        buffer,
    );
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
        .saturating_mul(35)
        .saturating_div(100)
        .max(8)
        .min(area.width.saturating_sub(value_width.saturating_add(2)));
    let bar_start = area.x.saturating_add(label_width.saturating_add(1));
    let bar_width = area
        .width
        .saturating_sub(label_width.saturating_add(value_width).saturating_add(2));
    let y = area.y.saturating_add(row);
    write_truncated(
        buffer,
        area.x,
        y,
        label,
        label_width,
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
        context_value_style(theme),
    );
}

fn render_processes(
    area: Rect,
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
            write_panel_row(
                area,
                3,
                "no live processes",
                theme.text_muted,
                theme,
                buffer,
            );
            return;
        }
        let visible_rows = usize::from(area.height.saturating_sub(3));
        for (index, process) in context.processes.iter().take(visible_rows).enumerate() {
            render_process_row(
                area,
                3 + index as u16,
                process.pid,
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
    write_panel_row(
        area,
        3,
        "waiting for first sample",
        theme.text_muted,
        theme,
        buffer,
    );
}

fn render_process_columns_header(area: Rect, theme: &Theme, buffer: &mut Buffer) {
    let pid_width = 7_u16.min(area.width);
    let memory_width = 10_u16.min(area.width);
    let cpu_width = 8_u16.min(area.width.saturating_sub(pid_width + memory_width));
    let cpu_x = area.right().saturating_sub(memory_width + cpu_width);
    let memory_x = area.right().saturating_sub(memory_width);
    buffer.set_stringn(
        area.x,
        area.y.saturating_add(2),
        "PID",
        usize::from(pid_width),
        Style::default().fg(theme.text_muted).bg(theme.surface),
    );
    let command_x = area.x.saturating_add(pid_width);
    buffer.set_stringn(
        command_x,
        area.y.saturating_add(2),
        "COMMAND",
        usize::from(cpu_x.saturating_sub(command_x)),
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

#[allow(clippy::too_many_arguments)]
fn render_process_row(
    area: Rect,
    row: u16,
    pid: u32,
    name: &str,
    cpu_percent: f32,
    memory_bytes: u64,
    theme: &Theme,
    buffer: &mut Buffer,
) {
    if row >= area.height {
        return;
    }
    let pid_width = 7_u16.min(area.width);
    let memory_width = 10_u16.min(area.width);
    let cpu_width = 8_u16.min(area.width.saturating_sub(pid_width + memory_width));
    let cpu_x = area.right().saturating_sub(memory_width + cpu_width);
    let memory_x = area.right().saturating_sub(memory_width);
    let command_x = area.x.saturating_add(pid_width);
    let y = area.y.saturating_add(row);
    buffer.set_stringn(
        area.x,
        y,
        format!("{pid:>5}"),
        usize::from(pid_width),
        context_value_style(theme),
    );
    write_truncated(
        buffer,
        command_x,
        y,
        name,
        cpu_x.saturating_sub(command_x).saturating_sub(1),
        Style::default().fg(theme.text).bg(theme.surface),
    );
    buffer.set_stringn(
        cpu_x,
        y,
        format!("{cpu_percent:>5.1}%"),
        usize::from(cpu_width),
        context_value_style(theme),
    );
    buffer.set_stringn(
        memory_x,
        y,
        format_bytes(memory_bytes as f64),
        usize::from(memory_width),
        context_value_style(theme),
    );
}

fn render_events(
    area: Rect,
    recent_events: &[String],
    live_mode: bool,
    theme: &Theme,
    buffer: &mut Buffer,
) {
    panel_heading(area, "RECENT EVENTS", theme.secondary, theme, buffer);
    if !recent_events.is_empty() {
        let visible_rows = usize::from(area.height.saturating_sub(2));
        for (index, event) in recent_events.iter().rev().take(visible_rows).enumerate() {
            let prefix = if index == 0 { "› " } else { "  " };
            let row = format!("{prefix}{event}");
            write_truncated(
                buffer,
                area.x,
                area.y.saturating_add(2 + index as u16),
                &row,
                area.width,
                Style::default()
                    .fg(if index == 0 {
                        theme.text
                    } else {
                        theme.text_muted
                    })
                    .bg(theme.surface),
            );
        }
        return;
    }
    if live_mode {
        write_panel_row(area, 2, "no recent events", theme.text_muted, theme, buffer);
        return;
    }
    write_panel_row(
        area,
        2,
        "waiting for first sample",
        theme.text_muted,
        theme,
        buffer,
    );
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

fn format_sample_age(age: Duration) -> String {
    if age < Duration::from_secs(1) {
        format!("{}ms", age.as_millis())
    } else if age < Duration::from_secs(60) {
        format!("{:.1}s", age.as_secs_f64())
    } else {
        let minutes = age.as_secs() / 60;
        if minutes < 60 {
            format!("{minutes}m")
        } else {
            format!("{}h {:02}m", minutes / 60, minutes % 60)
        }
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
            "┌"
        } else if x == right {
            "┐"
        } else {
            "─"
        });
        buffer[(x, bottom)].set_symbol(if x == area.x {
            "└"
        } else if x == right {
            "┘"
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

fn draw_titled_panel_frame(
    area: Rect,
    title: &str,
    accent: ratatui::style::Color,
    border: ratatui::style::Color,
    background: ratatui::style::Color,
    buffer: &mut Buffer,
) {
    draw_panel_frame(area, border, background, buffer);
    if area.width < 5 || area.height < 3 {
        return;
    }
    let label = format!(" {title} ");
    let label_width = u16::try_from(label.chars().count())
        .unwrap_or(area.width.saturating_sub(4))
        .min(area.width.saturating_sub(4));
    if label_width == 0 {
        return;
    }
    buffer.set_stringn(
        area.x.saturating_add(2),
        area.y,
        label,
        usize::from(label_width),
        Style::default()
            .fg(accent)
            .bg(background)
            .add_modifier(Modifier::BOLD),
    );
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

/// Keep measurement columns intact while making long diagnostic labels
/// legible. A plain `set_stringn` silently cuts names at the column edge;
/// this preserves the table grid and communicates that text was shortened.
fn write_truncated(buffer: &mut Buffer, x: u16, y: u16, text: &str, width: u16, style: Style) {
    if width == 0 {
        return;
    }
    let width = usize::from(width);
    let truncated = text.chars().nth(width).is_some();
    let mut chars = text.chars();
    for column in 0..width {
        let character = if truncated && column + 1 == width {
            '…'
        } else {
            chars.next().unwrap_or(' ')
        };
        buffer[(
            x.saturating_add(u16::try_from(column).unwrap_or(u16::MAX)),
            y,
        )]
            .set_char(character)
            .set_style(style);
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
            .set_char(if filled { '#' } else { '-' })
            .set_style(
                Style::default()
                    .fg(if filled { color } else { theme.surface_alt })
                    .bg(theme.surface),
            );
    }
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

const fn widget_label(widget: DashboardWidgetId) -> &'static str {
    match widget {
        DashboardWidgetId::Pulse => "PULSE",
        DashboardWidgetId::SystemSummary => "SUMMARY",
        DashboardWidgetId::Cpu => "CPU",
        DashboardWidgetId::Memory => "MEMORY",
        DashboardWidgetId::Gpu => "GPU",
        DashboardWidgetId::Network => "NETWORK",
        DashboardWidgetId::Storage => "STORAGE",
        DashboardWidgetId::History => "HISTORY",
        DashboardWidgetId::Processes => "PROCESSES",
        DashboardWidgetId::Alerts => "ALERTS",
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
    fn expanded_primary_widget_owns_the_primary_slots() {
        let visible = visible_primary(
            DashboardPreset::Standard,
            Rect::new(0, 0, 120, 30),
            Some(DashboardWidgetId::Memory),
        );
        assert_eq!(visible, [1, usize::MAX, usize::MAX, usize::MAX, usize::MAX]);
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
        assert!(text.contains("SYSTEM"));
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
        let visible = visible_primary(DashboardPreset::Widescreen, area, None);
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
                None,
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
        let sections = diagnostic_sections(outer, [true, true, true], true, 3);
        let temperatures = sections[0].unwrap();
        let processes = sections[1].unwrap();
        let events = sections[2].unwrap();

        assert_eq!(temperatures.height, 5);
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
        assert!(text.contains("no recent events"));
        assert!(!text.contains("sysmon"));
        assert!(!text.contains("browser"));
        assert!(!text.contains("compiler"));
    }

    #[test]
    fn live_context_renders_identity_freshness_and_process_identity() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 160, 40));
        let context = DashboardV2SystemContext {
            host_name: Some("workstation".to_owned()),
            os_name: Some("Linux".to_owned()),
            kernel: Some("6.12.1".to_owned()),
            physical_cores: Some(8),
            logical_threads: Some(16),
            total_memory_gib: Some(31.0),
            available_memory_gib: Some(18.5),
            uptime_seconds: Some(3_661),
            load_average: Some((0.42, 0.30, 0.20)),
            process_count: Some(128),
            processes: vec![DashboardProcessSummary {
                pid: 42,
                name: "sysmon".to_owned(),
                cpu_percent: 2.5,
                memory_bytes: 4 * 1024 * 1024,
            }],
        };
        let content =
            DashboardV2Content::new([None; PRIMARY_COUNT], input(), DashboardState::Active)
                .with_system_context(context)
                .with_live_mode(true)
                .with_sample_age(Some(Duration::from_millis(420)))
                .with_recent_events(vec!["SysMon started".to_owned()]);
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
        assert!(text.contains("workstation"));
        assert!(text.contains("8C / 16T"));
        assert!(text.contains("0.42 0.30 0.20"));
        assert!(text.contains("128"));
        assert!(text.contains("42"));
        assert!(text.contains("SysMon started"));
        assert!(text.contains("LIVE 420ms"));
    }

    #[test]
    fn diagnostic_values_are_clear_while_labels_and_headers_remain_muted() {
        let theme = Theme::pulse();
        let mut buffer = Buffer::empty(Rect::new(0, 0, 80, 12));
        render_temperature_row(
            Rect::new(0, 0, 80, 12),
            0,
            "CPU Package",
            Some(42),
            &theme,
            &mut buffer,
        );
        let value_x = 80 - "42°C".chars().count() as u16;
        assert_eq!(buffer[(value_x, 0)].fg, theme.text);
        assert!(
            buffer[(value_x, 0)]
                .modifier
                .contains(ratatui::style::Modifier::BOLD)
        );
        assert_eq!(buffer[(0, 0)].fg, theme.text_muted);

        render_process_columns_header(Rect::new(0, 1, 80, 12), &theme, &mut buffer);
        render_process_row(
            Rect::new(0, 1, 80, 12),
            1,
            42,
            "sysmon",
            12.5,
            1024 * 1024,
            &theme,
            &mut buffer,
        );
        assert_eq!(buffer[(0, 2)].fg, theme.text);
        let cpu_x = 80 - 10 - 8;
        assert_eq!(buffer[(cpu_x, 2)].fg, theme.text);
        assert!(
            buffer[(cpu_x, 2)]
                .modifier
                .contains(ratatui::style::Modifier::BOLD)
        );
        assert_eq!(buffer[(0, 3)].fg, theme.text_muted);
    }

    #[test]
    fn long_diagnostic_labels_preserve_the_measurement_columns() {
        let theme = Theme::pulse();
        let mut buffer = Buffer::empty(Rect::new(0, 0, 32, 1));
        write_truncated(
            &mut buffer,
            0,
            0,
            "a-process-with-a-long-name",
            12,
            context_label_style(&theme),
        );
        assert_eq!(buffer[(11, 0)].symbol(), "…");
        assert_eq!(buffer[(12, 0)].symbol(), " ");
    }

    #[test]
    fn titled_frames_keep_the_border_language_and_accent_the_label() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 20, 5));
        draw_titled_panel_frame(
            Rect::new(0, 0, 20, 5),
            "CPU",
            Color::Red,
            Color::Blue,
            Color::Black,
            &mut buffer,
        );

        assert_eq!(buffer[(0, 0)].symbol(), "┌");
        assert_eq!(buffer[(1, 0)].symbol(), "─");
        assert_eq!(buffer[(3, 0)].symbol(), "C");
        assert_eq!(buffer[(6, 0)].symbol(), " ");
        assert_eq!(buffer[(3, 0)].fg, Color::Red);
        assert_eq!(buffer[(1, 0)].fg, Color::Blue);
        assert_eq!(buffer[(19, 0)].symbol(), "┐");
    }
}
