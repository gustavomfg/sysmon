use super::chart::{AreaModel, PeakSet, SegmentedBars, SeriesComparison, SparklineModel};
use super::metrics::{
    CHART_POINTS, CpuMetricModel, GpuMetricModel, MAX_PEAKS, MemoryMetricModel, NetworkMetricModel,
    StorageMetricModel,
};
use super::{
    DashboardPreset, Highlight, Intensity, MetricAvailability, MetricIndicators, MetricValue,
    MetricVisual, Trend, VisualPriority, WidgetContext, WidgetState,
};

pub const MAX_RENDER_BLOCKS: usize = 13;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderError {
    DuplicateBlock,
    CapacityExceeded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockId {
    Title,
    Value,
    Trend,
    PrimarySparkline,
    SecondarySparkline,
    TertiarySparkline,
    Area,
    Segments,
    Comparison,
    Peaks,
    Indicators,
    Status,
    Footer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockRequirement {
    Required,
    Optional,
    Hideable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockVisibility {
    Visible,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockMetadata {
    id: BlockId,
    priority: VisualPriority,
    requirement: BlockRequirement,
    availability: MetricAvailability,
    visibility: BlockVisibility,
    focusable: bool,
    focused: bool,
    highlight: Highlight,
    state: WidgetState,
}

impl BlockMetadata {
    pub const fn id(self) -> BlockId {
        self.id
    }

    pub const fn priority(self) -> VisualPriority {
        self.priority
    }

    pub const fn requirement(self) -> BlockRequirement {
        self.requirement
    }

    pub const fn availability(self) -> MetricAvailability {
        self.availability
    }

    pub const fn visibility(self) -> BlockVisibility {
        self.visibility
    }

    pub const fn is_visible(self) -> bool {
        matches!(self.visibility, BlockVisibility::Visible)
    }

    pub const fn is_focusable(self) -> bool {
        self.focusable
    }

    pub const fn is_focused(self) -> bool {
        self.focused
    }

    pub const fn highlight(self) -> Highlight {
        self.highlight
    }

    pub const fn state(self) -> WidgetState {
        self.state
    }

    fn new(
        id: BlockId,
        priority: VisualPriority,
        requirement: BlockRequirement,
        availability: MetricAvailability,
        focusable: bool,
        focused: bool,
        visual: MetricVisual,
    ) -> Self {
        Self {
            id,
            priority,
            requirement,
            availability,
            visibility: if availability.is_usable() {
                BlockVisibility::Visible
            } else {
                BlockVisibility::Unavailable
            },
            focusable,
            focused: focusable && focused,
            highlight: visual.highlight(),
            state: visual.state(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TitleBlock {
    metadata: BlockMetadata,
    label: &'static str,
}

impl TitleBlock {
    pub const fn metadata(self) -> BlockMetadata {
        self.metadata
    }

    pub const fn label(self) -> &'static str {
        self.label
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ValueBlock {
    metadata: BlockMetadata,
    value: Option<MetricValue>,
}

impl ValueBlock {
    pub const fn metadata(self) -> BlockMetadata {
        self.metadata
    }

    pub const fn value(self) -> Option<MetricValue> {
        self.value
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TrendBlock {
    metadata: BlockMetadata,
    trend: Trend,
    intensity: Intensity,
}

impl TrendBlock {
    pub const fn metadata(self) -> BlockMetadata {
        self.metadata
    }

    pub const fn trend(self) -> Trend {
        self.trend
    }

    pub const fn intensity(self) -> Intensity {
        self.intensity
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
// Chart models are deliberately stored by value: their capacities are fixed by
// the dashboard contract, so this keeps the render model allocation-free.
#[allow(clippy::large_enum_variant)]
pub enum ChartContent {
    Sparkline(SparklineModel<CHART_POINTS>),
    Area(AreaModel),
    Segments2(SegmentedBars<2>),
    Segments3(SegmentedBars<3>),
    Comparison(SeriesComparison),
    Peaks(PeakSet<MAX_PEAKS>),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChartBlock {
    metadata: BlockMetadata,
    content: ChartContent,
}

impl ChartBlock {
    pub const fn metadata(self) -> BlockMetadata {
        self.metadata
    }

    pub const fn content(self) -> ChartContent {
        self.content
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IndicatorBlock {
    metadata: BlockMetadata,
    indicators: MetricIndicators,
}

impl IndicatorBlock {
    pub const fn metadata(self) -> BlockMetadata {
        self.metadata
    }

    pub const fn indicators(self) -> MetricIndicators {
        self.indicators
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StatusBlock {
    metadata: BlockMetadata,
    state: WidgetState,
    highlight: Highlight,
}

impl StatusBlock {
    pub const fn metadata(self) -> BlockMetadata {
        self.metadata
    }

    pub const fn state(self) -> WidgetState {
        self.state
    }

    pub const fn highlight(self) -> Highlight {
        self.highlight
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FooterBlock {
    metadata: BlockMetadata,
}

impl FooterBlock {
    pub const fn metadata(self) -> BlockMetadata {
        self.metadata
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
// See `ChartContent`: the bounded chart payload remains inline and predictable.
#[allow(clippy::large_enum_variant)]
pub enum RenderBlock {
    Title(TitleBlock),
    Value(ValueBlock),
    Trend(TrendBlock),
    Chart(ChartBlock),
    Indicators(IndicatorBlock),
    Status(StatusBlock),
    Footer(FooterBlock),
}

impl RenderBlock {
    pub const fn metadata(self) -> BlockMetadata {
        match self {
            Self::Title(block) => block.metadata(),
            Self::Value(block) => block.metadata(),
            Self::Trend(block) => block.metadata(),
            Self::Chart(block) => block.metadata(),
            Self::Indicators(block) => block.metadata(),
            Self::Status(block) => block.metadata(),
            Self::Footer(block) => block.metadata(),
        }
    }

    pub const fn id(self) -> BlockId {
        self.metadata().id()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WidgetRenderModel {
    visual: MetricVisual,
    preset: DashboardPreset,
    availability: MetricAvailability,
    focused: bool,
    blocks: [Option<RenderBlock>; MAX_RENDER_BLOCKS],
    length: usize,
}

impl WidgetRenderModel {
    fn new(
        visual: MetricVisual,
        preset: DashboardPreset,
        availability: MetricAvailability,
        focused: bool,
        title: &'static str,
    ) -> Result<Self, RenderError> {
        let mut model = Self {
            visual,
            preset,
            availability,
            focused,
            blocks: [None; MAX_RENDER_BLOCKS],
            length: 0,
        };
        model.push(RenderBlock::Title(TitleBlock {
            metadata: model.metadata(
                BlockId::Title,
                VisualPriority::Contextual,
                BlockRequirement::Required,
                false,
            ),
            label: title,
        }))?;
        model.push(RenderBlock::Value(ValueBlock {
            metadata: model.metadata(
                BlockId::Value,
                VisualPriority::Hero,
                BlockRequirement::Required,
                true,
            ),
            value: visual.primary(),
        }))?;
        model.push(RenderBlock::Trend(TrendBlock {
            metadata: model.metadata(
                BlockId::Trend,
                VisualPriority::Primary,
                BlockRequirement::Required,
                false,
            ),
            trend: visual.trend(),
            intensity: visual.intensity(),
        }))?;
        Ok(model)
    }

    fn metadata(
        &self,
        id: BlockId,
        priority: VisualPriority,
        requirement: BlockRequirement,
        focusable: bool,
    ) -> BlockMetadata {
        BlockMetadata::new(
            id,
            priority,
            requirement,
            self.availability,
            focusable,
            self.focused,
            self.visual,
        )
    }

    fn push(&mut self, block: RenderBlock) -> Result<(), RenderError> {
        if self.contains(block.id()) {
            return Err(RenderError::DuplicateBlock);
        }
        if self.length >= MAX_RENDER_BLOCKS {
            return Err(RenderError::CapacityExceeded);
        }
        self.blocks[self.length] = Some(block);
        self.length += 1;
        Ok(())
    }

    fn push_optional(&mut self, block: RenderBlock) -> Result<(), RenderError> {
        if !self.allows(block.id()) {
            return Ok(());
        }
        self.push(block)
    }

    fn contains(&self, id: BlockId) -> bool {
        self.blocks
            .iter()
            .take(self.length)
            .flatten()
            .any(|block| block.id() == id)
    }

    fn allows(&self, id: BlockId) -> bool {
        match self.preset {
            DashboardPreset::Compact => false,
            DashboardPreset::Standard => matches!(id, BlockId::PrimarySparkline),
            DashboardPreset::Widescreen => matches!(
                id,
                BlockId::PrimarySparkline
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

    pub const fn visual(self) -> MetricVisual {
        self.visual
    }

    pub const fn preset(self) -> DashboardPreset {
        self.preset
    }

    pub const fn availability(self) -> MetricAvailability {
        self.availability
    }

    pub const fn state(self) -> WidgetState {
        self.visual.state()
    }

    pub const fn highlight(self) -> Highlight {
        self.visual.highlight()
    }

    pub const fn is_focused(self) -> bool {
        self.focused
    }

    pub const fn block_count(self) -> usize {
        self.length
    }

    pub fn blocks(self) -> impl Iterator<Item = RenderBlock> {
        self.blocks.into_iter().take(self.length).flatten()
    }

    pub fn block(self, id: BlockId) -> Option<RenderBlock> {
        self.blocks
            .into_iter()
            .take(self.length)
            .flatten()
            .find(|block| block.id() == id)
    }

    pub fn add_sparkline(
        &mut self,
        id: BlockId,
        sparkline: SparklineModel<CHART_POINTS>,
    ) -> Result<(), RenderError> {
        self.push_optional(RenderBlock::Chart(ChartBlock {
            metadata: self.metadata(
                id,
                VisualPriority::Secondary,
                BlockRequirement::Optional,
                false,
            ),
            content: ChartContent::Sparkline(sparkline),
        }))
    }

    pub fn add_area(&mut self, area: AreaModel) -> Result<(), RenderError> {
        self.push_optional(RenderBlock::Chart(ChartBlock {
            metadata: self.metadata(
                BlockId::Area,
                VisualPriority::Secondary,
                BlockRequirement::Hideable,
                false,
            ),
            content: ChartContent::Area(area),
        }))
    }

    pub fn add_segments2(&mut self, segments: SegmentedBars<2>) -> Result<(), RenderError> {
        self.push_optional(RenderBlock::Chart(ChartBlock {
            metadata: self.metadata(
                BlockId::Segments,
                VisualPriority::Secondary,
                BlockRequirement::Optional,
                false,
            ),
            content: ChartContent::Segments2(segments),
        }))
    }

    pub fn add_segments3(&mut self, segments: SegmentedBars<3>) -> Result<(), RenderError> {
        self.push_optional(RenderBlock::Chart(ChartBlock {
            metadata: self.metadata(
                BlockId::Segments,
                VisualPriority::Secondary,
                BlockRequirement::Optional,
                false,
            ),
            content: ChartContent::Segments3(segments),
        }))
    }

    pub fn add_comparison(&mut self, comparison: SeriesComparison) -> Result<(), RenderError> {
        self.push_optional(RenderBlock::Chart(ChartBlock {
            metadata: self.metadata(
                BlockId::Comparison,
                VisualPriority::Secondary,
                BlockRequirement::Optional,
                false,
            ),
            content: ChartContent::Comparison(comparison),
        }))
    }

    pub fn add_peaks(&mut self, peaks: PeakSet<MAX_PEAKS>) -> Result<(), RenderError> {
        self.push_optional(RenderBlock::Chart(ChartBlock {
            metadata: self.metadata(
                BlockId::Peaks,
                VisualPriority::Contextual,
                BlockRequirement::Hideable,
                false,
            ),
            content: ChartContent::Peaks(peaks),
        }))
    }

    pub fn add_indicators(&mut self) -> Result<(), RenderError> {
        self.push_optional(RenderBlock::Indicators(IndicatorBlock {
            metadata: self.metadata(
                BlockId::Indicators,
                VisualPriority::Contextual,
                BlockRequirement::Optional,
                false,
            ),
            indicators: self.visual.indicators(),
        }))
    }

    pub fn add_status(&mut self) -> Result<(), RenderError> {
        self.push_optional(RenderBlock::Status(StatusBlock {
            metadata: self.metadata(
                BlockId::Status,
                VisualPriority::Contextual,
                BlockRequirement::Hideable,
                false,
            ),
            state: self.visual.state(),
            highlight: self.visual.highlight(),
        }))
    }
}

pub struct WidgetRenderer;

impl WidgetRenderer {
    pub fn render(
        visual: MetricVisual,
        context: WidgetContext,
        title: &'static str,
    ) -> Result<Option<WidgetRenderModel>, RenderError> {
        if !context.visible() || !context.availability().includes(visual.widget_id()) {
            return Ok(None);
        }
        WidgetRenderModel::new(
            visual,
            context.preset(),
            visual_availability(visual),
            context.focused_widget() == Some(visual.widget_id()),
            title,
        )
        .map(Some)
    }

    pub fn render_cpu<const HISTORY: usize>(
        model: CpuMetricModel<HISTORY>,
        context: WidgetContext,
    ) -> Result<Option<WidgetRenderModel>, RenderError> {
        let Some(mut render) = Self::render(model.visual(), context, "CPU")? else {
            return Ok(None);
        };
        render.add_sparkline(BlockId::PrimarySparkline, model.sparkline())?;
        render.add_segments3(model.composition())?;
        render.add_peaks(model.peaks())?;
        render.add_indicators()?;
        render.add_status()?;
        Ok(Some(render))
    }

    pub fn render_memory<const HISTORY: usize>(
        model: MemoryMetricModel<HISTORY>,
        context: WidgetContext,
    ) -> Result<Option<WidgetRenderModel>, RenderError> {
        let Some(mut render) = Self::render(model.visual(), context, "Memory")? else {
            return Ok(None);
        };
        render.add_sparkline(BlockId::PrimarySparkline, model.sparkline())?;
        if let Some(area) = model.area() {
            render.add_area(area)?;
        }
        render.add_segments3(model.composition())?;
        render.add_peaks(model.peaks())?;
        render.add_indicators()?;
        render.add_status()?;
        Ok(Some(render))
    }

    pub fn render_gpu<const HISTORY: usize>(
        model: GpuMetricModel<HISTORY>,
        context: WidgetContext,
    ) -> Result<Option<WidgetRenderModel>, RenderError> {
        let Some(mut render) = Self::render(model.visual(), context, "GPU")? else {
            return Ok(None);
        };
        render.add_sparkline(BlockId::PrimarySparkline, model.sparkline())?;
        render.add_segments2(model.vram())?;
        render.add_peaks(model.peaks())?;
        render.add_indicators()?;
        render.add_status()?;
        Ok(Some(render))
    }

    pub fn render_network<const HISTORY: usize>(
        model: NetworkMetricModel<HISTORY>,
        context: WidgetContext,
    ) -> Result<Option<WidgetRenderModel>, RenderError> {
        let Some(mut render) = Self::render(model.visual(), context, "Network")? else {
            return Ok(None);
        };
        render.add_sparkline(BlockId::PrimarySparkline, model.download_sparkline())?;
        render.add_sparkline(BlockId::SecondarySparkline, model.upload_sparkline())?;
        render.add_comparison(model.comparison())?;
        render.add_indicators()?;
        render.add_status()?;
        Ok(Some(render))
    }

    pub fn render_storage<const HISTORY: usize>(
        model: StorageMetricModel<HISTORY>,
        context: WidgetContext,
    ) -> Result<Option<WidgetRenderModel>, RenderError> {
        let Some(mut render) = Self::render(model.visual(), context, "Storage")? else {
            return Ok(None);
        };
        render.add_sparkline(BlockId::PrimarySparkline, model.occupancy_sparkline())?;
        render.add_sparkline(BlockId::SecondarySparkline, model.read_sparkline())?;
        render.add_sparkline(BlockId::TertiarySparkline, model.write_sparkline())?;
        render.add_segments2(model.capacity())?;
        render.add_comparison(model.comparison())?;
        render.add_peaks(model.peaks())?;
        render.add_indicators()?;
        render.add_status()?;
        Ok(Some(render))
    }
}

fn visual_availability(visual: MetricVisual) -> MetricAvailability {
    let values = [visual.primary(), visual.secondary()];
    values
        .into_iter()
        .flatten()
        .find(|value| value.is_usable())
        .map_or_else(
            || {
                values
                    .into_iter()
                    .flatten()
                    .next()
                    .map_or(MetricAvailability::Unavailable, MetricValue::availability)
            },
            MetricValue::availability,
        )
}

#[cfg(test)]
mod tests {
    use super::super::chart::TimeSample;
    use super::super::metrics::{CpuMetricInput, MetricReading};
    use super::super::{
        DashboardState, DashboardWidgetId, MetricAvailability, MetricUnit, TimeSeries,
        TrendDirection, WidgetAvailability,
    };
    use super::*;
    use std::time::Duration;

    fn context(preset: DashboardPreset) -> WidgetContext {
        WidgetContext::new(preset, DashboardState::Calm, Duration::from_secs(1))
            .with_availability(WidgetAvailability::all())
    }

    fn cpu() -> CpuMetricModel<8> {
        let mut history = TimeSeries::new(MetricUnit::Percent);
        for (index, value) in [0.20, 0.30, 0.40].into_iter().enumerate() {
            history
                .push(
                    TimeSample::new(
                        Duration::from_secs(index as u64),
                        Some(value),
                        MetricAvailability::Available,
                        None,
                    )
                    .unwrap(),
                )
                .unwrap();
        }
        CpuMetricModel::from_input(
            CpuMetricInput::new(
                MetricReading::normalized(Some(0.40), MetricAvailability::Available).unwrap(),
                MetricReading::normalized(Some(0.30), MetricAvailability::Available).unwrap(),
                MetricReading::normalized(Some(0.10), MetricAvailability::Available).unwrap(),
                MetricReading::normalized(Some(0.60), MetricAvailability::Available).unwrap(),
                MetricReading::non_negative(Some(3.0), MetricAvailability::Available).unwrap(),
                MetricReading::new(Some(50.0), MetricAvailability::Available).unwrap(),
                4,
                history,
            )
            .unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn base_model_contains_required_blocks_and_preserves_semantics() {
        let model =
            WidgetRenderer::render(cpu().visual(), context(DashboardPreset::Compact), "CPU")
                .unwrap()
                .unwrap();
        assert_eq!(model.block_count(), 3);
        assert!(model.block(BlockId::Title).is_some());
        assert!(model.block(BlockId::Value).is_some());
        assert!(model.block(BlockId::Trend).is_some());
        assert_eq!(model.state(), WidgetState::Normal);
        assert_eq!(model.highlight(), Highlight::None);
        assert_eq!(model.visual().widget_id(), DashboardWidgetId::Cpu);
    }

    #[test]
    fn responsive_profiles_reveal_more_semantic_blocks() {
        let compact = WidgetRenderer::render_cpu(cpu(), context(DashboardPreset::Compact))
            .unwrap()
            .unwrap();
        let standard = WidgetRenderer::render_cpu(cpu(), context(DashboardPreset::Standard))
            .unwrap()
            .unwrap();
        let wide = WidgetRenderer::render_cpu(cpu(), context(DashboardPreset::Widescreen))
            .unwrap()
            .unwrap();
        let large = WidgetRenderer::render_cpu(cpu(), context(DashboardPreset::LargeWidescreen))
            .unwrap()
            .unwrap();
        assert_eq!(compact.block_count(), 3);
        assert!(standard.block(BlockId::PrimarySparkline).is_some());
        assert!(wide.block(BlockId::Segments).is_some());
        assert!(wide.block(BlockId::Indicators).is_some());
        assert!(large.block(BlockId::Peaks).is_some());
        assert!(large.block_count() > wide.block_count());
    }

    #[test]
    fn chart_models_are_carried_without_terminal_primitives() {
        let model = WidgetRenderer::render_cpu(cpu(), context(DashboardPreset::LargeWidescreen))
            .unwrap()
            .unwrap();
        assert!(matches!(
            model.block(BlockId::PrimarySparkline),
            Some(RenderBlock::Chart(ChartBlock {
                content: ChartContent::Sparkline(_),
                ..
            }))
        ));
        assert!(matches!(
            model.block(BlockId::Segments),
            Some(RenderBlock::Chart(ChartBlock {
                content: ChartContent::Segments3(_),
                ..
            }))
        ));
        assert_eq!(
            model.block(BlockId::Trend).unwrap().metadata().priority(),
            VisualPriority::Primary
        );
    }

    #[test]
    fn focus_and_widget_availability_are_preserved() {
        let focused_context =
            context(DashboardPreset::Standard).with_focus(Some(DashboardWidgetId::Cpu));
        let focused = WidgetRenderer::render_cpu(cpu(), focused_context)
            .unwrap()
            .unwrap();
        assert!(focused.is_focused());
        assert!(
            focused
                .block(BlockId::Value)
                .unwrap()
                .metadata()
                .is_focused()
        );

        let hidden = focused_context.with_visibility(false);
        assert!(WidgetRenderer::render_cpu(cpu(), hidden).unwrap().is_none());
        let unavailable = focused_context.with_availability(WidgetAvailability::empty());
        assert!(
            WidgetRenderer::render_cpu(cpu(), unavailable)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn rendering_is_deterministic_and_keeps_metric_state() {
        let context = context(DashboardPreset::LargeWidescreen);
        let left = WidgetRenderer::render_cpu(cpu(), context).unwrap();
        let right = WidgetRenderer::render_cpu(cpu(), context).unwrap();
        assert_eq!(left, right);
        assert_eq!(
            left.unwrap().visual().trend().direction(),
            TrendDirection::Rising
        );
    }
}
