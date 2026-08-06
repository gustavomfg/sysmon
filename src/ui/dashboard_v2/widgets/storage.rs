use super::super::{
    RenderError, StorageMetricModel, WidgetContext, WidgetRenderModel, WidgetRenderer,
};

pub fn compose<const HISTORY: usize>(
    model: StorageMetricModel<HISTORY>,
    context: WidgetContext,
) -> Result<Option<WidgetRenderModel>, RenderError> {
    WidgetRenderer::render_storage(model, context)
}
