use std::time::{Duration, Instant};

use ratatui::{Frame, layout::Rect, style::Style};

use crate::{
    system::info::SystemInfo,
    theme::Theme,
    ui::dashboard_v2::{
        DashboardContext, DashboardDimensions, DashboardPreset, DashboardState, DashboardV2Content,
        DashboardV2Renderer, HealthEngine, HealthSnapshot, LiveMetricModels, LiveMetricsHistory,
        LiveSnapshot, PulseBehavior, PulseBehaviorEngine, WidgetContext, WidgetRenderer,
        WidgetState,
    },
};

const COLLECTION_INTERVAL: Duration = Duration::from_secs(1);

/// Runtime state for the official Dashboard V2 pipeline.
///
/// Collection enters this state as `LiveSnapshot`; health interpretation and
/// visual models are then prepared before the terminal renderer is called.
/// The renderer never reaches into `SystemInfo` or the collector directly.
pub struct DashboardApp {
    live_metrics: LiveMetricsHistory,
    system_context: Option<crate::ui::dashboard_v2::DashboardV2SystemContext>,
    health_engine: HealthEngine,
    pulse_behavior_engine: PulseBehaviorEngine,
    system_health: crate::ui::dashboard_v2::SystemHealth,
    pulse_behavior: PulseBehavior,
    elapsed: Duration,
    theme: Theme,
    has_snapshot: bool,
}

impl DashboardApp {
    pub fn new() -> Self {
        let health_engine = HealthEngine::new();
        let system_health = health_engine.evaluate(HealthSnapshot::default());
        let pulse_behavior = PulseBehavior::from_system_health(system_health);
        Self {
            live_metrics: LiveMetricsHistory::new(),
            system_context: None,
            health_engine,
            pulse_behavior_engine: PulseBehaviorEngine::new(),
            system_health,
            pulse_behavior,
            elapsed: Duration::ZERO,
            theme: Theme::pulse(),
            has_snapshot: false,
        }
    }

    pub const fn refresh_interval(&self) -> Duration {
        COLLECTION_INTERVAL
    }

    pub fn update(&mut self, snapshot: SystemInfo, collected_at: Instant) {
        self.system_context =
            Some(crate::ui::dashboard_v2::DashboardV2SystemContext::from_system_info(&snapshot));
        let live_snapshot = LiveSnapshot::from_system_info(&snapshot);
        let _ = self
            .live_metrics
            .push_snapshot(collected_at, &live_snapshot);
        self.has_snapshot = true;
    }

    pub fn update_animations(&mut self, delta: Duration) -> bool {
        self.elapsed = self.elapsed.saturating_add(delta);
        let health_snapshot = self
            .live_metrics
            .models()
            .map(|models| HealthSnapshot::from_models(&models))
            .unwrap_or_default();
        self.system_health = self.health_engine.update(health_snapshot, delta);
        self.pulse_behavior = self.pulse_behavior_engine.update(self.system_health, delta);
        true
    }

    pub const fn animations_active(&self) -> bool {
        true
    }

    pub fn draw(&self, frame: &mut Frame<'_>) {
        let area = frame.area();
        let dimensions =
            match DashboardDimensions::new(usize::from(area.width), usize::from(area.height)) {
                Ok(dimensions) => dimensions,
                Err(_) => {
                    frame.buffer_mut().set_string(
                        area.x,
                        area.y,
                        "Resize terminal to render Dashboard V2",
                        Style::default(),
                    );
                    return;
                }
            };
        let preset = DashboardPreset::from_dimensions(dimensions);
        let context = context_for(area);
        let models = self
            .has_snapshot
            .then(|| self.live_metrics.models().ok())
            .flatten();
        let dashboard_state = models
            .as_ref()
            .map(dashboard_state_for_models)
            .unwrap_or(DashboardState::Calm);
        let widget_context = WidgetContext::new(preset, dashboard_state, self.elapsed);
        let primary = models
            .as_ref()
            .map(|models| primary_models(models, widget_context))
            .unwrap_or([None; 5]);
        let inventory = models
            .as_ref()
            .map(|models| models.inventory().clone())
            .unwrap_or_default();

        let content =
            DashboardV2Content::new(primary, self.pulse_behavior.input(), dashboard_state)
                .with_pulse_behavior(self.pulse_behavior)
                .with_inventory(inventory)
                .with_live_mode(self.has_snapshot);
        let content = if let Some(system_context) = self.system_context.clone() {
            content.with_system_context(system_context)
        } else {
            content
        };

        if DashboardV2Renderer::new()
            .render(
                preset,
                context,
                content,
                self.elapsed,
                false,
                &self.theme,
                frame.buffer_mut(),
            )
            .is_err()
        {
            frame.buffer_mut().set_string(
                area.x,
                area.y,
                "Resize terminal or choose a smaller preset",
                Style::default(),
            );
        }
    }
}

impl Default for DashboardApp {
    fn default() -> Self {
        Self::new()
    }
}

fn context_for(area: Rect) -> DashboardContext {
    if area.width >= 100 && area.height >= 28 {
        return DashboardContext::default();
    }

    let mut context = DashboardContext::default();
    for region in [
        crate::ui::dashboard_v2::DashboardRegion::SecondaryMetrics,
        crate::ui::dashboard_v2::DashboardRegion::Charts,
        crate::ui::dashboard_v2::DashboardRegion::Processes,
        crate::ui::dashboard_v2::DashboardRegion::Alerts,
    ] {
        context = context.without(region);
    }
    context
}

fn primary_models(
    models: &LiveMetricModels,
    context: WidgetContext,
) -> [Option<crate::ui::dashboard_v2::WidgetRenderModel>; 5] {
    [
        WidgetRenderer::render_cpu(models.cpu(), context)
            .ok()
            .flatten(),
        WidgetRenderer::render_memory(models.memory(), context)
            .ok()
            .flatten(),
        WidgetRenderer::render_gpu(models.gpu(), context)
            .ok()
            .flatten(),
        WidgetRenderer::render_network(models.network(), context)
            .ok()
            .flatten(),
        WidgetRenderer::render_storage(models.storage(), context)
            .ok()
            .flatten(),
    ]
}

fn dashboard_state_for_models(models: &LiveMetricModels) -> DashboardState {
    let states = [
        models.cpu().state(),
        models.memory().state(),
        models.gpu().state(),
        models.network().state(),
        models.storage().state(),
    ];
    if states.contains(&WidgetState::Critical) {
        DashboardState::Critical
    } else if states.contains(&WidgetState::Warning) {
        DashboardState::Busy
    } else if states.contains(&WidgetState::Active) {
        DashboardState::Active
    } else {
        DashboardState::Calm
    }
}

#[cfg(test)]
mod tests {
    use ratatui::{Terminal, backend::TestBackend};

    use super::DashboardApp;

    #[test]
    fn renders_the_official_v2_surface_at_representative_sizes() {
        let app = DashboardApp::new();

        for (width, height) in [(80, 24), (100, 30), (140, 40), (180, 50)] {
            let backend = TestBackend::new(width, height);
            let mut terminal = Terminal::new(backend).expect("test terminal should be created");

            terminal
                .draw(|frame| app.draw(frame))
                .expect("dashboard should render");

            let rendered = terminal
                .backend()
                .buffer()
                .content
                .iter()
                .map(|cell| cell.symbol())
                .collect::<String>();
            assert!(rendered.contains("SYSMON V2"));
            assert!(rendered.contains("PULSE V2"));
        }
    }
}
