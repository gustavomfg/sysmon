use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
};

use crate::{
    action::Action,
    app::AppState,
    command_palette::CommandStatus,
    config::ConfigField,
    focus::FocusTarget,
    system::info::SystemInfo,
    theme::Theme,
    ui::dashboard_v2::{
        DashboardContext, DashboardDimensions, DashboardPreset, DashboardState, DashboardV2Content,
        DashboardV2Renderer, HealthComponent, HealthEngine, HealthSeverity, HealthSnapshot,
        LiveMetricModels, LiveMetricsError, LiveMetricsHistory, LiveSnapshot, PulseBehavior,
        PulseBehaviorEngine, SystemHealth, WidgetAvailability, WidgetContext, WidgetRenderer,
        WidgetState,
    },
    widget::WidgetId,
};

/// Runtime state for the official Dashboard V2 pipeline.
///
/// Collection enters this state as `LiveSnapshot`; health interpretation and
/// visual models are then prepared before the terminal renderer is called.
/// The renderer never reaches into `SystemInfo` or the collector directly.
pub struct DashboardApp {
    live_metrics: LiveMetricsHistory,
    models: Option<LiveMetricModels>,
    system_context: Option<Arc<crate::ui::dashboard_v2::DashboardV2SystemContext>>,
    health_engine: HealthEngine,
    pulse_behavior_engine: PulseBehaviorEngine,
    system_health: crate::ui::dashboard_v2::SystemHealth,
    pulse_behavior: PulseBehavior,
    elapsed: Duration,
    ui: AppState,
    renderer: DashboardV2Renderer,
    last_error: Option<LiveMetricsError>,
    has_snapshot: bool,
    last_collected_at: Option<Instant>,
    sample_revision: u64,
    prepared_primary: Option<PreparedPrimary>,
    expanded_widget: Option<crate::ui::dashboard_v2::DashboardWidgetId>,
    narrator: HealthNarrator,
}

struct PreparedPrimary {
    revision: u64,
    context: WidgetContext,
    models: Arc<[Option<crate::ui::dashboard_v2::WidgetRenderModel>; 5]>,
}

/// Turns health transitions into short timeline sentences. A change must
/// hold for `NARRATION_DWELL` before it is told, so brief spikes and the
/// health engine's own smoothing never flood the event list.
#[derive(Debug, Clone, Copy)]
struct HealthNarrator {
    told: HealthSeverity,
    candidate: HealthSeverity,
    held: Duration,
}

const NARRATION_DWELL: Duration = Duration::from_secs(3);

impl HealthNarrator {
    const fn new() -> Self {
        Self {
            told: HealthSeverity::Healthy,
            candidate: HealthSeverity::Healthy,
            held: Duration::ZERO,
        }
    }

    fn observe(&mut self, health: SystemHealth, delta: Duration) -> Option<String> {
        let severity = health.severity();
        if severity != self.candidate {
            self.candidate = severity;
            self.held = Duration::ZERO;
        } else {
            self.held = self.held.saturating_add(delta);
        }
        if self.candidate == self.told || self.held < NARRATION_DWELL {
            return None;
        }
        let rising = self.candidate > self.told;
        self.told = self.candidate;
        let source = match health.dominant_source() {
            Some(HealthComponent::Temperature) => "Thermals",
            Some(component) => component.label(),
            None => "System",
        };
        Some(match (severity, rising) {
            (HealthSeverity::Healthy, _) => "Back to calm".to_owned(),
            (HealthSeverity::Active, true) => format!("{source} picked up the pace"),
            (HealthSeverity::Active, false) => format!("Easing off · {source} still active"),
            (HealthSeverity::Elevated, _) => format!("{source} under sustained load"),
            (HealthSeverity::Stressed, _) => format!("{source} pressure rising"),
            (HealthSeverity::Critical, _) => format!("{source} critical · needs attention"),
        })
    }
}

impl DashboardApp {
    pub fn new() -> Self {
        let health_engine = HealthEngine::new();
        let system_health = health_engine.evaluate(HealthSnapshot::default());
        let pulse_behavior = PulseBehavior::from_system_health(system_health);
        Self {
            live_metrics: LiveMetricsHistory::new(),
            models: None,
            system_context: None,
            health_engine,
            pulse_behavior_engine: PulseBehaviorEngine::new(),
            system_health,
            pulse_behavior,
            elapsed: Duration::ZERO,
            ui: AppState::new(),
            renderer: DashboardV2Renderer::new(),
            last_error: None,
            has_snapshot: false,
            last_collected_at: None,
            sample_revision: 0,
            prepared_primary: None,
            expanded_widget: None,
            narrator: HealthNarrator::new(),
        }
    }

    pub fn refresh_interval(&self) -> Duration {
        self.ui.refresh_interval()
    }

    pub fn handle_event(&mut self, event: Event) -> bool {
        match event {
            Event::Key(key) => self.handle_key(key),
            Event::Resize(_, _) => false,
            _ => false,
        }
    }

    pub fn update(&mut self, snapshot: SystemInfo, collected_at: Instant) {
        self.sample_revision = self.sample_revision.wrapping_add(1);
        self.ui.update(snapshot.clone());
        self.last_collected_at = Some(collected_at);
        self.system_context = Some(Arc::new(
            crate::ui::dashboard_v2::DashboardV2SystemContext::from_system_info(&snapshot),
        ));
        let live_snapshot = LiveSnapshot::from_system_info(&snapshot);
        match self
            .live_metrics
            .push_snapshot(collected_at, &live_snapshot)
        {
            Ok(()) => match self.live_metrics.models() {
                Ok(models) => {
                    self.models = Some(models);
                    self.last_error = None;
                }
                Err(error) => self.last_error = Some(error),
            },
            Err(error) => self.last_error = Some(error),
        }
        self.has_snapshot = true;
    }

    pub fn update_animations(&mut self, delta: Duration) -> bool {
        self.elapsed = self.elapsed.saturating_add(delta);
        let health_snapshot = self
            .models
            .as_ref()
            .map(HealthSnapshot::from_models)
            .unwrap_or_default();
        self.system_health = self.health_engine.update(health_snapshot, delta);
        if self.has_snapshot
            && let Some(event) = self.narrator.observe(self.system_health, delta)
        {
            self.ui.record_event(event);
        }
        self.pulse_behavior = self.pulse_behavior_engine.update(self.system_health, delta);
        self.has_snapshot && !self.ui.reduced_motion()
    }

    pub fn animations_active(&self) -> bool {
        self.has_snapshot && !self.ui.reduced_motion()
    }

    pub fn draw(&mut self, frame: &mut Frame<'_>) {
        let area = frame.area();
        let dimensions =
            match DashboardDimensions::new(usize::from(area.width), usize::from(area.height)) {
                Ok(dimensions) => dimensions,
                Err(_) => {
                    frame.buffer_mut().set_string(
                        area.x,
                        area.y,
                        "Resize terminal to render SysMon",
                        Style::default(),
                    );
                    return;
                }
            };
        let preset = DashboardPreset::from_dimensions(dimensions);
        let context = context_for(area, self.expanded_widget);
        let dashboard_state = self
            .models
            .as_ref()
            .map(dashboard_state_for_models)
            .unwrap_or(DashboardState::Calm);
        let focused_widget = focused_widget(&self.ui);
        let availability = widget_availability(&self.ui);
        let widget_context = WidgetContext::new(preset, dashboard_state, self.elapsed)
            .with_reduced_motion(self.ui.reduced_motion())
            .with_focus(focused_widget)
            .with_availability(availability);
        // The lateral instruments have no frame-driven animation. Their
        // semantic models change on telemetry, focus, availability or mode.
        // Pulse continues to receive the live elapsed clock independently.
        let cache_context = WidgetContext::new(preset, dashboard_state, Duration::ZERO)
            .with_reduced_motion(self.ui.reduced_motion())
            .with_focus(focused_widget)
            .with_availability(availability);
        if !self.prepared_primary.as_ref().is_some_and(|cache| {
            cache.revision == self.sample_revision && cache.context == cache_context
        }) {
            let models = self
                .models
                .as_ref()
                .map(|models| primary_models(models, widget_context))
                .unwrap_or([None; 5]);
            self.prepared_primary = Some(PreparedPrimary {
                revision: self.sample_revision,
                context: cache_context,
                models: Arc::new(models),
            });
        }
        let primary = Arc::clone(
            &self
                .prepared_primary
                .as_ref()
                .expect("prepared above")
                .models,
        );
        let inventory = self
            .models
            .as_ref()
            .map(|models| models.inventory().clone())
            .unwrap_or_default();

        let sample_age = self
            .last_collected_at
            .map(|collected_at| Instant::now().saturating_duration_since(collected_at));
        let data_stale = self.last_error.is_some()
            || sample_age.is_some_and(|age| {
                age > self
                    .refresh_interval()
                    .checked_mul(3)
                    .unwrap_or(Duration::MAX)
            });
        let content = DashboardV2Content::from_shared_primary(
            primary,
            self.pulse_behavior.input(),
            dashboard_state,
        )
        .with_pulse_behavior(self.pulse_behavior)
        .with_system_health(self.system_health)
        .with_inventory(inventory)
        .with_live_mode(self.has_snapshot)
        .with_focused_widget(focused_widget)
        .with_expanded_widget(self.expanded_widget)
        .with_data_stale(data_stale)
        .with_sample_age(sample_age)
        .with_recent_events(self.ui.timeline().events().to_vec())
        .with_event_ages(self.ui.timeline().ages(Instant::now()));
        let content = if let Some(system_context) = self.system_context.clone() {
            content.with_shared_system_context(system_context)
        } else {
            content
        };

        let theme = *self.ui.theme();
        if self
            .renderer
            .render(
                preset,
                context,
                content,
                self.elapsed,
                self.ui.reduced_motion(),
                &theme,
                frame.buffer_mut(),
            )
            .is_err()
        {
            frame.buffer_mut().set_string(
                area.x,
                area.y,
                "Resize terminal or choose a smaller layout",
                Style::default().fg(theme.warning),
            );
        }

        self.render_overlay(frame, &theme);
        theme.apply_text_palette(frame.buffer_mut());
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        if matches!(key.kind, KeyEventKind::Release) {
            return false;
        }
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            return true;
        }

        if self.ui.palette().is_open() {
            return self.handle_palette_key(key);
        }
        if self.ui.config().is_open() {
            return self.handle_config_key(key);
        }
        if self.ui.process_search().is_open() {
            return self.handle_search_key(key);
        }

        match key.code {
            KeyCode::Esc if self.expanded_widget.take().is_some() => {}
            KeyCode::Esc if self.ui.focus_active() => {
                self.ui.dispatch(Action::ExitFocus);
            }
            KeyCode::Esc | KeyCode::Char('q') => return true,
            KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.ui.dispatch(Action::OpenPalette);
            }
            KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.open_command("search");
            }
            KeyCode::Char('/') => self.open_command("search"),
            KeyCode::Tab => {
                self.ui
                    .dispatch(if key.modifiers.contains(KeyModifiers::SHIFT) {
                        Action::FocusPrevious
                    } else {
                        Action::FocusNext
                    });
            }
            KeyCode::Char('e') => self.toggle_expanded(),
            KeyCode::Char('j') | KeyCode::Down => {
                self.ui.dispatch(Action::FocusNext);
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.ui.dispatch(Action::FocusPrevious);
            }
            _ => {}
        }
        false
    }

    fn toggle_expanded(&mut self) {
        let focused = focused_widget(&self.ui);
        self.expanded_widget = match (self.expanded_widget, focused) {
            (Some(current), Some(next)) if current == next => None,
            (_, next) => next,
        };
    }

    fn open_command(&mut self, query: &str) {
        self.ui.dispatch(Action::OpenPalette);
        for character in query.chars() {
            self.ui.dispatch(Action::PaletteCharacter(character));
        }
        self.ui.dispatch(Action::PaletteSubmit);
    }

    fn handle_palette_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Esc => {
                self.ui.dispatch(Action::ClosePalette);
            }
            KeyCode::Up => {
                self.ui.dispatch(Action::PaletteUp);
            }
            KeyCode::Down => {
                self.ui.dispatch(Action::PaletteDown);
            }
            KeyCode::Backspace => {
                self.ui.dispatch(Action::PaletteBackspace);
            }
            KeyCode::Enter => {
                return self.ui.dispatch(Action::PaletteSubmit);
            }
            KeyCode::Char(character) => {
                self.ui.dispatch(Action::PaletteCharacter(character));
            }
            _ => {}
        }
        false
    }

    fn handle_config_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Esc => {
                self.ui.dispatch(Action::CloseConfig);
            }
            KeyCode::Up => {
                self.ui.dispatch(Action::ConfigUp);
            }
            KeyCode::Down => {
                self.ui.dispatch(Action::ConfigDown);
            }
            KeyCode::Left => {
                self.ui.dispatch(Action::ConfigPrevious);
            }
            KeyCode::Right => {
                self.ui.dispatch(Action::ConfigNext);
            }
            KeyCode::Enter | KeyCode::Char(' ') => {
                self.ui.dispatch(Action::ConfigToggle);
            }
            _ => {}
        }
        false
    }

    fn handle_search_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Esc => {
                self.ui.dispatch(Action::CloseProcessSearch);
            }
            KeyCode::Up => {
                self.ui.dispatch(Action::ProcessSearchUp);
            }
            KeyCode::Down => {
                self.ui.dispatch(Action::ProcessSearchDown);
            }
            KeyCode::Backspace => {
                self.ui.dispatch(Action::ProcessSearchBackspace);
            }
            KeyCode::Enter => {
                self.ui.dispatch(Action::ProcessSearchSubmit);
            }
            KeyCode::Char(character) => {
                self.ui.dispatch(Action::ProcessSearchCharacter(character));
            }
            _ => {}
        }
        false
    }

    fn render_overlay(&self, frame: &mut Frame<'_>, theme: &Theme) {
        if self.ui.palette().is_open() {
            render_palette(frame, theme, &self.ui);
        } else if self.ui.config().is_open() {
            render_settings(frame, theme, &self.ui);
        } else if self.ui.process_search().is_open() {
            render_process_search(frame, theme, &self.ui);
        } else if self.ui.is_focused(FocusTarget::Timeline) {
            render_timeline(frame, theme, &self.ui);
        }
    }
}

impl Default for DashboardApp {
    fn default() -> Self {
        Self::new()
    }
}

fn context_for(
    area: Rect,
    expanded_widget: Option<crate::ui::dashboard_v2::DashboardWidgetId>,
) -> DashboardContext {
    if let Some(widget) = expanded_widget {
        let mut context = DashboardContext::default();
        for region in [
            crate::ui::dashboard_v2::DashboardRegion::Pulse,
            crate::ui::dashboard_v2::DashboardRegion::PrimaryMetrics,
            crate::ui::dashboard_v2::DashboardRegion::SecondaryMetrics,
            crate::ui::dashboard_v2::DashboardRegion::Charts,
            crate::ui::dashboard_v2::DashboardRegion::Processes,
            crate::ui::dashboard_v2::DashboardRegion::Alerts,
        ] {
            context = context.without(region);
        }
        let region = match widget {
            crate::ui::dashboard_v2::DashboardWidgetId::Pulse => {
                crate::ui::dashboard_v2::DashboardRegion::Pulse
            }
            crate::ui::dashboard_v2::DashboardWidgetId::SystemSummary => {
                crate::ui::dashboard_v2::DashboardRegion::SecondaryMetrics
            }
            crate::ui::dashboard_v2::DashboardWidgetId::Cpu
            | crate::ui::dashboard_v2::DashboardWidgetId::Memory
            | crate::ui::dashboard_v2::DashboardWidgetId::Gpu
            | crate::ui::dashboard_v2::DashboardWidgetId::Network
            | crate::ui::dashboard_v2::DashboardWidgetId::Storage => {
                crate::ui::dashboard_v2::DashboardRegion::PrimaryMetrics
            }
            crate::ui::dashboard_v2::DashboardWidgetId::History => {
                crate::ui::dashboard_v2::DashboardRegion::Charts
            }
            crate::ui::dashboard_v2::DashboardWidgetId::Processes => {
                crate::ui::dashboard_v2::DashboardRegion::Processes
            }
            crate::ui::dashboard_v2::DashboardWidgetId::Alerts => {
                crate::ui::dashboard_v2::DashboardRegion::Alerts
            }
        };
        return context.with(region);
    }

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

fn widget_availability(ui: &AppState) -> WidgetAvailability {
    let mut availability = WidgetAvailability::all();
    for (widget, id) in [
        (
            WidgetId::Gpu,
            crate::ui::dashboard_v2::DashboardWidgetId::Gpu,
        ),
        (
            WidgetId::Memory,
            crate::ui::dashboard_v2::DashboardWidgetId::Memory,
        ),
        (
            WidgetId::Network,
            crate::ui::dashboard_v2::DashboardWidgetId::Network,
        ),
    ] {
        if !ui.widget_visible(widget) {
            availability = availability.without(id);
        }
    }
    availability
}

fn focused_widget(ui: &AppState) -> Option<crate::ui::dashboard_v2::DashboardWidgetId> {
    [
        (
            FocusTarget::Cpu,
            crate::ui::dashboard_v2::DashboardWidgetId::Cpu,
        ),
        (
            FocusTarget::Overview,
            crate::ui::dashboard_v2::DashboardWidgetId::Pulse,
        ),
        (
            FocusTarget::System,
            crate::ui::dashboard_v2::DashboardWidgetId::SystemSummary,
        ),
        (
            FocusTarget::Memory,
            crate::ui::dashboard_v2::DashboardWidgetId::Memory,
        ),
        (
            FocusTarget::Gpu,
            crate::ui::dashboard_v2::DashboardWidgetId::Gpu,
        ),
        (
            FocusTarget::Network,
            crate::ui::dashboard_v2::DashboardWidgetId::Network,
        ),
        (
            FocusTarget::Storage,
            crate::ui::dashboard_v2::DashboardWidgetId::Storage,
        ),
        (
            FocusTarget::History,
            crate::ui::dashboard_v2::DashboardWidgetId::History,
        ),
        (
            FocusTarget::Processes,
            crate::ui::dashboard_v2::DashboardWidgetId::Processes,
        ),
        (
            FocusTarget::Timeline,
            crate::ui::dashboard_v2::DashboardWidgetId::Alerts,
        ),
    ]
    .into_iter()
    .find_map(|(target, widget)| ui.is_focused(target).then_some(widget))
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

fn dialog_area(area: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(area.width.saturating_sub(4)).max(1);
    let height = height.min(area.height.saturating_sub(4)).max(1);
    Rect::new(
        area.x.saturating_add(area.width.saturating_sub(width) / 2),
        area.y
            .saturating_add(area.height.saturating_sub(height) / 2),
        width,
        height,
    )
}

fn dialog_block<'a>(title: &'a str, theme: &Theme) -> Block<'a> {
    Block::default()
        .title(Span::styled(
            format!(" {title} "),
            Style::default()
                .fg(theme.primary)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.border_focused))
        .style(Style::default().bg(theme.surface))
}

fn render_palette(frame: &mut Frame<'_>, theme: &Theme, ui: &AppState) {
    let area = dialog_area(frame.area(), 72, 16);
    frame.render_widget(Clear, area);
    let block = dialog_block("COMMANDS  Ctrl+P", theme);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.height == 0 {
        return;
    }
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(1)])
        .split(inner);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(": ", Style::default().fg(theme.primary)),
            Span::styled(ui.palette().query(), Style::default().fg(theme.text)),
        ])),
        sections[0],
    );

    let commands = ui.palette().filtered_commands();
    let items = if commands.is_empty() {
        vec![ListItem::new("no matching command").style(Style::default().fg(theme.text_muted))]
    } else {
        commands
            .iter()
            .map(|command| {
                let suffix = if command.status() == CommandStatus::Planned {
                    "  [planned]"
                } else {
                    ""
                };
                ListItem::new(format!("{}{}", command.label(), suffix))
            })
            .collect()
    };
    let mut state = ListState::default();
    if !commands.is_empty() {
        state.select(Some(ui.palette().selected_index()));
    }
    let list = List::new(items).highlight_symbol("> ").highlight_style(
        Style::default()
            .fg(theme.selection_text)
            .bg(theme.selection_background)
            .add_modifier(Modifier::BOLD),
    );
    frame.render_stateful_widget(list, sections[1], &mut state);
}

fn render_settings(frame: &mut Frame<'_>, theme: &Theme, ui: &AppState) {
    let area = dialog_area(frame.area(), 64, 11);
    frame.render_widget(Clear, area);
    let block = dialog_block("SETTINGS", theme);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let selected = ui.config().selected();
    let lines = ConfigField::ALL.into_iter().map(|field| {
        let marker = if field == selected { "> " } else { "  " };
        let value = match field {
            ConfigField::Theme => ui.theme_kind().label().to_owned(),
            ConfigField::ReducedMotion => if ui.reduced_motion() { "on" } else { "off" }.to_owned(),
            ConfigField::RefreshRate => format!("{}s", ui.refresh_interval().as_secs()),
        };
        Line::from(vec![
            Span::styled(
                marker,
                Style::default().fg(if field == selected {
                    theme.primary
                } else {
                    theme.text_muted
                }),
            ),
            Span::styled(
                format!("{:<18}", field.label()),
                Style::default().fg(theme.text_muted),
            ),
            Span::styled(
                value,
                Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
            ),
        ])
    });
    frame.render_widget(Paragraph::new(lines.collect::<Vec<_>>()), inner);
}

fn render_process_search(frame: &mut Frame<'_>, theme: &Theme, ui: &AppState) {
    let area = dialog_area(frame.area(), 72, 18);
    frame.render_widget(Clear, area);
    let block = dialog_block("SEARCH PROCESS  /", theme);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(1)])
        .split(inner);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("/ ", Style::default().fg(theme.primary)),
            Span::styled(ui.process_search().query(), Style::default().fg(theme.text)),
        ])),
        sections[0],
    );
    let results = ui.process_search_results();
    let items = if results.is_empty() {
        vec![ListItem::new("no matching process").style(Style::default().fg(theme.text_muted))]
    } else {
        results
            .iter()
            .map(|process| ListItem::new(format!("{:>5}  {}", process.pid, process.name)))
            .collect()
    };
    let mut state = ListState::default();
    if !results.is_empty() {
        state.select(Some(ui.process_search().selected_index()));
    }
    frame.render_stateful_widget(
        List::new(items)
            .highlight_symbol("> ")
            .highlight_style(Style::default().bg(theme.selection_background)),
        sections[1],
        &mut state,
    );
}

fn render_timeline(frame: &mut Frame<'_>, theme: &Theme, ui: &AppState) {
    let area = dialog_area(frame.area(), 76, 16);
    frame.render_widget(Clear, area);
    let block = dialog_block("TIMELINE  Esc close", theme);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let items = ui
        .timeline()
        .events()
        .iter()
        .rev()
        .take(usize::from(inner.height))
        .map(|event| ListItem::new(event.as_str()).style(Style::default().fg(theme.text_muted)))
        .collect::<Vec<_>>();
    frame.render_widget(List::new(items), inner);
}

#[cfg(test)]
mod tests {
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
    use ratatui::{Terminal, backend::TestBackend};

    use super::{DashboardApp, HealthNarrator, focused_widget};
    use crate::ui::dashboard_v2::{
        HealthComponent, HealthEngine, HealthSignals, HealthSnapshot, SystemHealth,
    };
    use std::time::Duration;

    fn health(cpu_activity: f32, memory_pressure: f32) -> SystemHealth {
        HealthEngine::new().evaluate(
            HealthSnapshot::default()
                .with_component(
                    HealthComponent::Cpu,
                    HealthSignals::new(Some(cpu_activity), None, None, None),
                )
                .with_component(
                    HealthComponent::Memory,
                    HealthSignals::new(None, Some(memory_pressure), None, None),
                ),
        )
    }

    #[test]
    fn narrator_tells_held_transitions_once_and_ignores_brief_spikes() {
        let mut narrator = HealthNarrator::new();
        let calm = health(0.0, 0.0);
        let busy = health(1.0, 0.95);
        assert!(busy.severity() > calm.severity());

        // A one-second spike is never told.
        assert_eq!(narrator.observe(busy, Duration::from_secs(1)), None);
        assert_eq!(narrator.observe(calm, Duration::from_secs(1)), None);

        // A held change is told exactly once.
        let mut told = Vec::new();
        for _ in 0..10 {
            told.extend(narrator.observe(busy, Duration::from_secs(1)));
        }
        assert_eq!(told.len(), 1);

        let mut recovered = Vec::new();
        for _ in 0..10 {
            recovered.extend(narrator.observe(calm, Duration::from_secs(1)));
        }
        assert_eq!(recovered, vec!["Back to calm".to_owned()]);
    }

    #[test]
    fn renders_a_terminal_native_surface_at_representative_sizes() {
        let mut app = DashboardApp::new();

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
            assert!(rendered.contains("SYSMON"));
            assert!(rendered.contains("SYSTEM PULSE"));
            assert!(!rendered.contains("16 GB RAM"));
            assert!(!rendered.contains("browser"));
        }
    }

    #[test]
    fn routes_terminal_commands_to_native_overlays() {
        let mut app = DashboardApp::new();
        let ctrl_p = KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL);
        assert!(!app.handle_event(Event::Key(ctrl_p)));

        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).expect("test terminal should be created");
        terminal
            .draw(|frame| app.draw(frame))
            .expect("palette should render");
        let palette = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(palette.contains("COMMANDS"));

        assert!(!app.handle_event(Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE,))));
        assert!(!app.handle_event(Event::Key(KeyEvent::new(
            KeyCode::Char('/'),
            KeyModifiers::NONE,
        ))));
        assert!(!app.handle_event(Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE,))));
        assert!(app.handle_event(Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE,))));
    }

    #[test]
    fn terminal_focus_can_expand_a_widget_and_step_back_cleanly() {
        let mut app = DashboardApp::new();
        assert_eq!(focused_widget(&app.ui), None);

        assert!(!app.handle_event(Event::Key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE,))));
        assert_eq!(
            focused_widget(&app.ui),
            Some(crate::ui::dashboard_v2::DashboardWidgetId::Pulse)
        );

        assert!(!app.handle_event(Event::Key(KeyEvent::new(
            KeyCode::Char('e'),
            KeyModifiers::NONE,
        ))));
        assert_eq!(
            app.expanded_widget,
            Some(crate::ui::dashboard_v2::DashboardWidgetId::Pulse)
        );

        assert!(!app.handle_event(Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE,))));
        assert_eq!(app.expanded_widget, None);
        assert!(app.ui.focus_active());

        assert!(!app.handle_event(Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE,))));
        assert!(!app.ui.focus_active());
    }
}
