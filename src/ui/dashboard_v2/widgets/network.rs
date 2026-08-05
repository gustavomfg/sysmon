use super::super::{
    NetworkMetricModel, RenderError, WidgetContext, WidgetRenderModel, WidgetRenderer,
};

pub fn compose<const HISTORY: usize>(
    model: NetworkMetricModel<HISTORY>,
    context: WidgetContext,
) -> Result<Option<WidgetRenderModel>, RenderError> {
    WidgetRenderer::render_network(model, context)
}
