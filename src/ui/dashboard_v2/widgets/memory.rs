use super::super::{
    MemoryMetricModel, RenderError, WidgetContext, WidgetRenderModel, WidgetRenderer,
};

pub fn compose<const HISTORY: usize>(
    model: MemoryMetricModel<HISTORY>,
    context: WidgetContext,
) -> Result<Option<WidgetRenderModel>, RenderError> {
    WidgetRenderer::render_memory(model, context)
}
