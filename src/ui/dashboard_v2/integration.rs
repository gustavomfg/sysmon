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
const PULSE_TITLE: &str = "PULSE V2";
const HEADER_TITLE: &str = "SYSMON  ·  DASHBOARD V2";

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
        let active_count = visible.iter().filter(|index| **index != usize::MAX).count();
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
            let card = inset(slot, 1, 0);
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
        buffer.set_stringn(
            area.x.saturating_add(1),
            area.y,
            PULSE_TITLE,
            usize::from(area.width.saturating_sub(2)),
            Style::default().fg(theme.text_muted),
        );

        let raster_area = Rect::new(
            area.x.saturating_add(1),
            area.y.saturating_add(1),
            area.width.saturating_sub(2),
            area.height.saturating_sub(1),
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
    let capacity = rows.saturating_mul(columns).min(PRIMARY_COUNT);
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
    let columns = if area.width >= 40 && count > 1 { 2 } else { 1 };
    let rows = count.div_ceil(columns);
    let column_width = area.width / columns as u16;
    let row_height = area.height / rows as u16;
    let mut result = [Rect::default(); PRIMARY_COUNT];
    for (index, slot) in result.iter_mut().enumerate().take(count) {
        let column = index % columns;
        let row = index / columns;
        let x = area
            .x
            .saturating_add(column_width.saturating_mul(column as u16));
        let y = area.y.saturating_add(row_height.saturating_mul(row as u16));
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
    let text = format!(
        "{HEADER_TITLE}   {}  ·  {}",
        preset_label(preset),
        state_label(state),
    );
    buffer.set_stringn(
        area.x,
        area.y,
        text,
        usize::from(area.width),
        Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
    );
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
        "{}  ·  {}  ·  P preset  S state  G GPU  R motion  Space pause  Q quit",
        preset_label(preset),
        motion
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
    theme: &Theme,
    buffer: &mut Buffer,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let title = match region {
        DashboardRegion::SecondaryMetrics => "CONTEXT",
        DashboardRegion::Charts => "TRENDS",
        DashboardRegion::Processes => "PROCESSES",
        DashboardRegion::Alerts => "EVENTS",
        _ => return,
    };
    let detail = match region {
        DashboardRegion::Alerts => match state {
            DashboardState::Calm | DashboardState::Active => "No active events",
            DashboardState::Busy => "Elevated system activity",
            DashboardState::Critical => "Critical activity detected",
        },
        DashboardRegion::Charts => "Shared history context",
        DashboardRegion::Processes => "Process context available",
        DashboardRegion::SecondaryMetrics => "Normalized system context",
        _ => "",
    };
    buffer.set_style(area, Style::default().bg(theme.surface));
    buffer.set_stringn(
        area.x.saturating_add(1),
        area.y,
        title,
        usize::from(area.width.saturating_sub(2)),
        Style::default().fg(theme.text_muted),
    );
    if area.height > 1 {
        buffer.set_stringn(
            area.x.saturating_add(1),
            area.y.saturating_add(1),
            detail,
            usize::from(area.width.saturating_sub(2)),
            Style::default().fg(theme.text_muted),
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
}
