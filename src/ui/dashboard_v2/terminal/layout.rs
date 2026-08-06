use ratatui::layout::Rect;

use super::super::{
    BlockId, BlockRequirement, DashboardBounds, DashboardPreset, DashboardWidgetId,
    MAX_RENDER_BLOCKS, WidgetRenderModel,
};

const COLUMN_GAP: u16 = 2;
const MIN_SIDE_BY_SIDE_WIDTH: u16 = 24;
const MIN_CHART_WIDTH: u16 = 8;
const CHART_WIDTH_PERCENT: u16 = 42;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalLayoutError {
    BoundsOverflow,
    BufferTooSmall,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Placement {
    id: BlockId,
    area: Rect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalLayout {
    area: Rect,
    placements: [Option<Placement>; MAX_RENDER_BLOCKS],
    length: usize,
}

impl TerminalLayout {
    pub fn new(
        model: WidgetRenderModel,
        preset: DashboardPreset,
        bounds: DashboardBounds,
        buffer_area: Rect,
    ) -> Result<Self, TerminalLayoutError> {
        let area = to_rect(bounds)?;
        if !contains(buffer_area, area) {
            return Err(TerminalLayoutError::BufferTooSmall);
        }

        let mut ids = [None; MAX_RENDER_BLOCKS];
        let mut selected = [false; MAX_RENDER_BLOCKS];
        let mut length = 0;
        for required in [true, false] {
            for block in model.blocks() {
                if length >= MAX_RENDER_BLOCKS
                    || !allowed(preset, block.id())
                    || (block.metadata().requirement() == BlockRequirement::Required) != required
                {
                    continue;
                }
                ids[length] = Some(block.id());
                selected[length] = true;
                length += 1;
            }
        }

        let has_primary_chart = selected_primary_chart(&ids, &selected, length);
        let side_by_side =
            has_primary_chart && area.width >= MIN_SIDE_BY_SIDE_WIDTH && area.height >= 4;
        if !side_by_side {
            for index in 0..length {
                if selected[index] && ids[index] == Some(BlockId::PrimarySparkline) {
                    selected[index] = false;
                }
            }
        }

        let mut placements = [None; MAX_RENDER_BLOCKS];
        let mut placement_length = 0;
        let title_id = ids
            .iter()
            .take(length)
            .flatten()
            .find(|id| **id == BlockId::Title)
            .copied();
        if area.height > 0
            && let Some(id) = title_id
        {
            place(
                &mut placements,
                &mut placement_length,
                id,
                Rect::new(area.x, area.y, area.width, 1),
            );
        }

        let footer_id = ids
            .iter()
            .take(length)
            .flatten()
            .find(|id| matches!(id, BlockId::Status | BlockId::Footer))
            .copied();
        let footer_height = u16::from(area.height > 1 && footer_id.is_some());
        let title_spacer =
            u16::from(model.visual().widget_id() == DashboardWidgetId::Cpu && area.height >= 8);
        let body_top = area
            .y
            .saturating_add(u16::from(area.height > 0))
            .saturating_add(title_spacer);
        let body_height = area
            .height
            .saturating_sub(u16::from(area.height > 0))
            .saturating_sub(title_spacer)
            .saturating_sub(footer_height);
        let body = Rect::new(area.x, body_top, area.width, body_height);

        if side_by_side && body.height > 0 {
            // CPU cards keep a short lower context row visible beside the
            // compact chart. Other widgets retain the broader chart area.
            let main_height = if model.visual().widget_id() == DashboardWidgetId::Cpu {
                if body.height < 3 {
                    body.height
                } else {
                    body.height.saturating_sub(2).clamp(3, 7)
                }
            } else {
                body.height.min(6)
            };
            let main = Rect::new(body.x, body.y, body.width, main_height);
            let (value_area, chart_area) = split_columns(main);
            let value_height = value_area
                .height
                .min(if value_area.height >= 2 { 2 } else { 1 });
            if let Some(id) = find_id(&ids, &selected, length, BlockId::Value) {
                place(
                    &mut placements,
                    &mut placement_length,
                    id,
                    Rect::new(value_area.x, value_area.y, value_area.width, value_height),
                );
            }
            if let Some(id) = find_id(&ids, &selected, length, BlockId::PrimarySparkline) {
                place(&mut placements, &mut placement_length, id, chart_area);
            }
            let left_y = value_area.y.saturating_add(value_height);
            if let Some(id) = find_id(&ids, &selected, length, BlockId::Trend)
                && left_y < value_area.bottom()
            {
                place(
                    &mut placements,
                    &mut placement_length,
                    id,
                    Rect::new(value_area.x, left_y, value_area.width, 1),
                );
            }
            place_details(
                &ids,
                &selected,
                length,
                &mut placements,
                &mut placement_length,
                Rect::new(
                    body.x,
                    body.y.saturating_add(main_height),
                    body.width,
                    body.height.saturating_sub(main_height),
                ),
            );
        } else {
            place_vertical(
                &ids,
                &selected,
                length,
                &mut placements,
                &mut placement_length,
                body,
            );
        }

        if let Some(id) = footer_id
            && footer_height > 0
        {
            place(
                &mut placements,
                &mut placement_length,
                id,
                Rect::new(area.x, area.bottom().saturating_sub(1), area.width, 1),
            );
        }

        Ok(Self {
            area,
            placements,
            length: placement_length,
        })
    }

    pub const fn area(self) -> Rect {
        self.area
    }

    pub fn block_area(self, id: BlockId) -> Option<Rect> {
        self.placements
            .into_iter()
            .take(self.length)
            .flatten()
            .find(|placement| placement.id == id)
            .map(|placement| placement.area)
    }
}

fn find_id(
    ids: &[Option<BlockId>; MAX_RENDER_BLOCKS],
    selected: &[bool; MAX_RENDER_BLOCKS],
    length: usize,
    id: BlockId,
) -> Option<BlockId> {
    (0..length).find(|index| selected[*index] && ids[*index] == Some(id))?;
    Some(id)
}

fn selected_primary_chart(
    ids: &[Option<BlockId>; MAX_RENDER_BLOCKS],
    selected: &[bool; MAX_RENDER_BLOCKS],
    length: usize,
) -> bool {
    (0..length).any(|index| selected[index] && ids[index] == Some(BlockId::PrimarySparkline))
}

fn split_columns(area: Rect) -> (Rect, Rect) {
    let chart_width = (area.width.saturating_mul(CHART_WIDTH_PERCENT) / 100)
        .max(MIN_CHART_WIDTH)
        .min(area.width.saturating_sub(COLUMN_GAP).saturating_sub(8));
    let value_width = area
        .width
        .saturating_sub(chart_width)
        .saturating_sub(COLUMN_GAP);
    (
        Rect::new(area.x, area.y, value_width, area.height),
        Rect::new(
            area.x
                .saturating_add(value_width)
                .saturating_add(COLUMN_GAP),
            area.y,
            chart_width,
            area.height,
        ),
    )
}

fn place(
    placements: &mut [Option<Placement>; MAX_RENDER_BLOCKS],
    length: &mut usize,
    id: BlockId,
    area: Rect,
) {
    if area.width == 0 || area.height == 0 || *length >= MAX_RENDER_BLOCKS {
        return;
    }
    placements[*length] = Some(Placement { id, area });
    *length += 1;
}

fn place_details(
    ids: &[Option<BlockId>; MAX_RENDER_BLOCKS],
    selected: &[bool; MAX_RENDER_BLOCKS],
    length: usize,
    placements: &mut [Option<Placement>; MAX_RENDER_BLOCKS],
    placement_length: &mut usize,
    area: Rect,
) {
    let mut y = area.y;
    for index in 0..length {
        let Some(id) = ids[index] else {
            continue;
        };
        if !selected[index]
            || matches!(
                id,
                BlockId::Title
                    | BlockId::Value
                    | BlockId::Trend
                    | BlockId::PrimarySparkline
                    | BlockId::Status
                    | BlockId::Footer
            )
        {
            continue;
        }
        if y >= area.bottom() {
            break;
        }
        let height = detail_height(id).min(area.bottom().saturating_sub(y));
        place(
            placements,
            placement_length,
            id,
            Rect::new(area.x, y, area.width, height),
        );
        y = y.saturating_add(height);
    }
}

fn place_vertical(
    ids: &[Option<BlockId>; MAX_RENDER_BLOCKS],
    selected: &[bool; MAX_RENDER_BLOCKS],
    length: usize,
    placements: &mut [Option<Placement>; MAX_RENDER_BLOCKS],
    placement_length: &mut usize,
    area: Rect,
) {
    let mut y = area.y;
    for index in 0..length {
        let Some(id) = ids[index] else {
            continue;
        };
        if !selected[index]
            || id == BlockId::Title
            || matches!(id, BlockId::Status | BlockId::Footer)
        {
            continue;
        }
        if y >= area.bottom() {
            break;
        }
        let height = match id {
            BlockId::Value => 1,
            BlockId::Trend => 1,
            _ => detail_height(id),
        }
        .min(area.bottom().saturating_sub(y));
        place(
            placements,
            placement_length,
            id,
            Rect::new(area.x, y, area.width, height),
        );
        y = y.saturating_add(height);
    }
}

fn detail_height(id: BlockId) -> u16 {
    match id {
        BlockId::SecondarySparkline | BlockId::TertiarySparkline => 2,
        BlockId::Area => 1,
        BlockId::Segments | BlockId::Comparison | BlockId::Peaks | BlockId::Indicators => 1,
        _ => 1,
    }
}

fn to_rect(bounds: DashboardBounds) -> Result<Rect, TerminalLayoutError> {
    let x = u16::try_from(bounds.x()).map_err(|_| TerminalLayoutError::BoundsOverflow)?;
    let y = u16::try_from(bounds.y()).map_err(|_| TerminalLayoutError::BoundsOverflow)?;
    let width = u16::try_from(bounds.width()).map_err(|_| TerminalLayoutError::BoundsOverflow)?;
    let height = u16::try_from(bounds.height()).map_err(|_| TerminalLayoutError::BoundsOverflow)?;
    Ok(Rect::new(x, y, width, height))
}

fn contains(container: Rect, area: Rect) -> bool {
    let container_right = u32::from(container.x) + u32::from(container.width);
    let container_bottom = u32::from(container.y) + u32::from(container.height);
    let area_right = u32::from(area.x) + u32::from(area.width);
    let area_bottom = u32::from(area.y) + u32::from(area.height);
    u32::from(area.x) >= u32::from(container.x)
        && u32::from(area.y) >= u32::from(container.y)
        && area_right <= container_right
        && area_bottom <= container_bottom
}

fn allowed(preset: DashboardPreset, id: BlockId) -> bool {
    match preset {
        DashboardPreset::Compact => matches!(
            id,
            BlockId::Title
                | BlockId::Value
                | BlockId::Trend
                | BlockId::CpuContext
                | BlockId::Status
        ),
        DashboardPreset::Standard => matches!(
            id,
            BlockId::Title
                | BlockId::Value
                | BlockId::Trend
                | BlockId::CpuContext
                | BlockId::PrimarySparkline
                | BlockId::Status
        ),
        DashboardPreset::Widescreen => matches!(
            id,
            BlockId::Title
                | BlockId::Value
                | BlockId::Trend
                | BlockId::CpuContext
                | BlockId::PrimarySparkline
                | BlockId::SecondarySparkline
                | BlockId::Area
                | BlockId::Segments
                | BlockId::Comparison
                | BlockId::Indicators
                | BlockId::Status
        ),
        DashboardPreset::LargeWidescreen => true,
    }
}
