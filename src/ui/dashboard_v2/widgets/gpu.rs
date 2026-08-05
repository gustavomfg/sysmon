use super::super::{GpuMetricModel, RenderError, WidgetContext, WidgetRenderModel, WidgetRenderer};

pub fn compose<const HISTORY: usize>(
    model: GpuMetricModel<HISTORY>,
    context: WidgetContext,
) -> Result<Option<WidgetRenderModel>, RenderError> {
    WidgetRenderer::render_gpu(model, context)
}
