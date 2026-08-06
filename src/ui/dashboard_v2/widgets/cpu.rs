use super::super::{CpuMetricModel, RenderError, WidgetContext, WidgetRenderModel, WidgetRenderer};

pub fn compose<const HISTORY: usize>(
    model: CpuMetricModel<HISTORY>,
    context: WidgetContext,
) -> Result<Option<WidgetRenderModel>, RenderError> {
    WidgetRenderer::render_cpu(model, context)
}
