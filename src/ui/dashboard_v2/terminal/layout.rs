use ratatui::layout::Rect;

use super::super::{
    BlockId, ChartContent, DashboardBounds, DashboardPreset, MAX_RENDER_BLOCKS, RenderBlock,
    WidgetRenderModel,
};

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
        let mut heights = [1u16; MAX_RENDER_BLOCKS];
        let mut length = 0;
        for block in model.blocks() {
            if length >= MAX_RENDER_BLOCKS || !allowed(preset, block.id()) {
                continue;
            }
            ids[length] = Some(block.id());
            heights[length] = preferred_height(block);
            length += 1;
        }

        let available = usize::from(area.height);
        let mut preferred_total = heights[..length]
            .iter()
            .map(|height| usize::from(*height))
            .sum::<usize>();
        while preferred_total > available && heights[..length].iter().any(|height| *height > 1) {
            for height in heights.iter_mut().take(length) {
                if preferred_total <= available {
                    break;
                }
                if *height > 1 {
                    *height -= 1;
                    preferred_total -= 1;
                }
            }
        }

        let mut placements = [None; MAX_RENDER_BLOCKS];
        let mut y = area.y;
        let mut used_height = 0usize;
        let mut placement_length = 0;
        for index in 0..length {
            if used_height >= available {
                break;
            }
            let height = usize::from(heights[index]).min(available - used_height);
            let Some(id) = ids[index] else {
                continue;
            };
            let height = u16::try_from(height).map_err(|_| TerminalLayoutError::BoundsOverflow)?;
            placements[placement_length] = Some(Placement {
                id,
                area: Rect::new(area.x, y, area.width, height),
            });
            placement_length += 1;
            used_height += usize::from(height);
            y = y.saturating_add(height);
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
        DashboardPreset::Compact => matches!(id, BlockId::Title | BlockId::Value | BlockId::Trend),
        DashboardPreset::Standard => matches!(
            id,
            BlockId::Title | BlockId::Value | BlockId::Trend | BlockId::PrimarySparkline
        ),
        DashboardPreset::Widescreen => matches!(
            id,
            BlockId::Title
                | BlockId::Value
                | BlockId::Trend
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

fn preferred_height(block: RenderBlock) -> u16 {
    match block {
        RenderBlock::Chart(chart) => match chart.content() {
            ChartContent::Sparkline(_) | ChartContent::Area(_) => 3,
            ChartContent::Segments2(_)
            | ChartContent::Segments3(_)
            | ChartContent::Comparison(_)
            | ChartContent::Peaks(_) => 1,
        },
        RenderBlock::Title(_)
        | RenderBlock::Value(_)
        | RenderBlock::Trend(_)
        | RenderBlock::Indicators(_)
        | RenderBlock::Status(_)
        | RenderBlock::Footer(_) => 1,
    }
}
