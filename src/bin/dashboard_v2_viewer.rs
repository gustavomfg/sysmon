#[allow(dead_code)]
#[path = "../ui/dashboard_v2/mod.rs"]
pub mod dashboard_v2;

#[path = "../system/mod.rs"]
#[allow(dead_code)]
mod system;

#[path = "../ui/widgets/pulse_v2/mod.rs"]
pub mod pulse_v2;

#[allow(unused_imports)]
mod ui {
    pub use crate::dashboard_v2;

    pub mod widgets {
        pub use crate::pulse_v2;
    }
}

#[allow(dead_code)]
#[path = "../theme/mod.rs"]
mod theme;

mod pulse {
    #[derive(Debug, Clone, Copy, Default, PartialEq)]
    pub struct PulseInput {
        pub cpu: f32,
        pub memory: f32,
        pub gpu: Option<f32>,
        pub network: f32,
        pub load: f32,
        pub volatility: f32,
    }
}

use std::{
    io,
    sync::mpsc::{self, Receiver, Sender},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use dashboard_v2::{
    CpuLiveSample, CpuMetricInput, DashboardContext, DashboardPreset, DashboardProcessSummary,
    DashboardState, DashboardV2Content, DashboardV2Renderer, DashboardV2SystemContext, GpuDevice,
    GpuMetricInput, HardwareId, HardwareInventory, HardwareKind, HealthComponent, HealthEngine,
    HealthSignals, HealthSnapshot, LiveGpuSnapshot, LiveMetricsHistory, LiveSnapshot,
    LiveStorageSnapshot, MemoryMetricInput, MetricAvailability, MetricReading, MetricUnit,
    NetworkInterface, NetworkMetricInput, PulseBehavior, PulseBehaviorEngine, StorageDevice,
    StorageMetricInput, TemperatureSensor, TerminalRenderer, TimeSample, TimeSeries, WidgetContext,
    WidgetRenderer,
};
use pulse_v2::NormalizedPulseInput;
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::Rect,
    style::Style,
    widgets::{Block, Borders},
};
use system::info::{SystemInfo, SystemInfoCollector};
use theme::Theme;

const FRAME_INTERVAL: Duration = Duration::from_millis(33);
const LIVE_COLLECTION_INTERVAL: Duration = Duration::from_secs(1);
const HISTORY: usize = 12;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Scenario {
    Normal,
    Active,
    GpuActivity,
    Pressure,
    Thermal,
    NetworkActivity,
    StoragePressure,
    Critical,
    Recovery,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CpuPreviewSize {
    Reference,
    Spacious,
    Large,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CpuSignalPreview {
    Scenario,
    Low,
    Mid,
    High,
}

impl CpuSignalPreview {
    const fn next(self) -> Self {
        match self {
            Self::Scenario => Self::Low,
            Self::Low => Self::Mid,
            Self::Mid => Self::High,
            Self::High => Self::Scenario,
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::Scenario => "SCENARIO",
            Self::Low => "LOW 8–12%",
            Self::Mid => "MID 45–56%",
            Self::High => "HIGH 86–95%",
        }
    }
}

impl CpuPreviewSize {
    const ALL: [Self; 3] = [Self::Reference, Self::Spacious, Self::Large];

    const fn dimensions(self) -> (u16, u16) {
        match self {
            Self::Reference => (45, 12),
            Self::Spacious => (50, 14),
            Self::Large => (60, 16),
        }
    }

    const fn next(self) -> Self {
        let index = match self {
            Self::Reference => 0,
            Self::Spacious => 1,
            Self::Large => 2,
        };
        Self::ALL[(index + 1) % Self::ALL.len()]
    }
}

impl Scenario {
    const ALL: [Self; 9] = [
        Self::Normal,
        Self::Active,
        Self::GpuActivity,
        Self::Pressure,
        Self::Thermal,
        Self::NetworkActivity,
        Self::StoragePressure,
        Self::Critical,
        Self::Recovery,
    ];

    const fn index(self) -> usize {
        match self {
            Self::Normal => 0,
            Self::Active => 1,
            Self::GpuActivity => 2,
            Self::Pressure => 3,
            Self::Thermal => 4,
            Self::NetworkActivity => 5,
            Self::StoragePressure => 6,
            Self::Critical => 7,
            Self::Recovery => 8,
        }
    }

    const fn next(self) -> Self {
        Self::ALL[(self.index() + 1) % Self::ALL.len()]
    }

    const fn values(self) -> (f64, f64, f64, f64, f64, f64) {
        match self {
            Self::Normal => (0.25, 0.35, 0.25, 0.12, 0.35, 0.20),
            // High activity with safe pressure: the Pulse should become
            // energetic without immediately becoming stressed.
            Self::Active => (0.95, 0.40, 0.75, 0.65, 0.40, 0.35),
            Self::GpuActivity => (0.25, 0.40, 0.98, 0.12, 0.40, 0.20),
            Self::Pressure => (0.35, 0.94, 0.25, 0.12, 0.40, 0.20),
            Self::Thermal => (0.45, 0.42, 0.35, 0.12, 0.40, 0.20),
            Self::NetworkActivity => (0.25, 0.40, 0.25, 0.95, 0.40, 0.20),
            Self::StoragePressure => (0.25, 0.40, 0.25, 0.12, 0.95, 0.30),
            Self::Critical => (0.98, 0.97, 0.98, 0.95, 0.98, 0.98),
            Self::Recovery => (0.20, 0.40, 0.20, 0.10, 0.40, 0.10),
        }
    }

    const fn dashboard_state(self) -> DashboardState {
        match self {
            Self::Normal => DashboardState::Calm,
            Self::Active | Self::GpuActivity | Self::NetworkActivity => DashboardState::Active,
            Self::Pressure | Self::Thermal | Self::StoragePressure => DashboardState::Busy,
            Self::Critical => DashboardState::Critical,
            Self::Recovery => DashboardState::Calm,
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::Normal => "IDLE",
            Self::Active => "HIGH ACTIVITY / HEALTHY",
            Self::GpuActivity => "GPU ACTIVITY",
            Self::Pressure => "MEMORY PRESSURE",
            Self::Thermal => "THERMAL PRESSURE",
            Self::NetworkActivity => "NETWORK ACTIVITY",
            Self::StoragePressure => "STORAGE PRESSURE",
            Self::Critical => "CRITICAL",
            Self::Recovery => "RECOVERY",
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct ViewerState {
    preset: DashboardPreset,
    scenario: Scenario,
    gpu_available: bool,
    reduced_motion: bool,
    paused: bool,
    live_metrics: bool,
    focused_cpu: bool,
    cpu_only: bool,
    health_debug: bool,
    cpu_preview_size: CpuPreviewSize,
    cpu_signal_preview: CpuSignalPreview,
    elapsed: Duration,
}

impl ViewerState {
    const fn new() -> Self {
        Self {
            preset: DashboardPreset::Standard,
            scenario: Scenario::Normal,
            gpu_available: true,
            reduced_motion: false,
            paused: false,
            live_metrics: false,
            focused_cpu: false,
            cpu_only: false,
            health_debug: false,
            cpu_preview_size: CpuPreviewSize::Reference,
            cpu_signal_preview: CpuSignalPreview::Scenario,
            elapsed: Duration::ZERO,
        }
    }

    fn next_preset(&mut self) {
        self.preset = match self.preset {
            DashboardPreset::Compact => DashboardPreset::Standard,
            DashboardPreset::Standard => DashboardPreset::Widescreen,
            DashboardPreset::Widescreen => DashboardPreset::LargeWidescreen,
            DashboardPreset::LargeWidescreen => DashboardPreset::Compact,
        };
    }
}

enum LiveCollectorCommand {
    Stop,
}

struct LiveCollectorGuard {
    commands: Sender<LiveCollectorCommand>,
    handle: Option<JoinHandle<()>>,
}

impl Drop for LiveCollectorGuard {
    fn drop(&mut self) {
        let _ = self.commands.send(LiveCollectorCommand::Stop);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn spawn_live_collector() -> (Receiver<(Instant, SystemInfo)>, LiveCollectorGuard) {
    let (snapshot_sender, snapshot_receiver) = mpsc::channel();
    let (command_sender, command_receiver) = mpsc::channel();
    let handle = thread::spawn(move || {
        let mut collector = SystemInfoCollector::new();
        let mut next_collection = Instant::now();
        loop {
            let wait = next_collection.saturating_duration_since(Instant::now());
            match command_receiver.recv_timeout(wait) {
                Ok(LiveCollectorCommand::Stop) | Err(mpsc::RecvTimeoutError::Disconnected) => {
                    break;
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    let collected_at = Instant::now();
                    let snapshot = collector.collect(collected_at);
                    if snapshot_sender.send((collected_at, snapshot)).is_err() {
                        break;
                    }
                    next_collection = collected_at + LIVE_COLLECTION_INTERVAL;
                }
            }
        }
    });
    (
        snapshot_receiver,
        LiveCollectorGuard {
            commands: command_sender,
            handle: Some(handle),
        },
    )
}

fn main() -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let result = run_viewer(&mut stdout);
    disable_raw_mode()?;
    execute!(stdout, LeaveAlternateScreen)?;
    result
}

fn run_viewer(stdout: &mut io::Stdout) -> io::Result<()> {
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let mut state = ViewerState::new();
    let (live_snapshots, _live_collector) = spawn_live_collector();
    let mut live_metrics = LiveMetricsHistory::new();
    let mut live_system_context = None;
    let mut health_engine = HealthEngine::new();
    let mut pulse_behavior_engine = PulseBehaviorEngine::new();
    let mut health_live_mode = false;
    let theme = Theme::pulse();
    let renderer = DashboardV2Renderer::new();
    let mut last_tick = Instant::now();

    loop {
        if event::poll(FRAME_INTERVAL)? {
            while event::poll(Duration::ZERO)? {
                if let Event::Key(key) = event::read()?
                    && key.kind == KeyEventKind::Press
                    && handle_key(key.code, &mut state)
                {
                    return Ok(());
                }
            }
        }

        for (collected_at, snapshot) in live_snapshots.try_iter() {
            live_system_context = Some(system_context_from_system(&snapshot));
            let _ = live_metrics.push_snapshot(collected_at, &live_snapshot_from_system(&snapshot));
        }

        let now = Instant::now();
        let delta = now.saturating_duration_since(last_tick);
        last_tick = now;
        if !state.paused {
            state.elapsed = state.elapsed.saturating_add(delta);
        }

        if state.live_metrics != health_live_mode {
            health_engine.reset();
            pulse_behavior_engine.reset();
            health_live_mode = state.live_metrics;
        }
        let health_snapshot = if state.live_metrics {
            live_metrics
                .models()
                .map(|models| {
                    HealthSnapshot::from_models_with_policy(&models, health_engine.policy())
                })
                .unwrap_or_default()
        } else {
            scenario_health_snapshot(state)
        };
        let system_health = health_engine.update(health_snapshot, delta);
        let pulse_behavior = pulse_behavior_engine.update(system_health, delta);

        terminal.draw(|frame| {
            if state.cpu_only {
                render_cpu_preview(frame, state, &live_metrics, &theme);
            } else {
                let content = content_with_health_and_behavior(
                    state,
                    &live_metrics,
                    live_system_context.as_ref(),
                    Some(system_health),
                    Some(pulse_behavior),
                );
                let context = context_for(frame.area());
                let rendered = renderer
                    .render(
                        state.preset,
                        context,
                        content,
                        state.elapsed,
                        state.reduced_motion,
                        &theme,
                        frame.buffer_mut(),
                    )
                    .is_ok();
                if rendered && state.health_debug {
                    render_health_debug(
                        frame,
                        state.scenario,
                        system_health,
                        pulse_behavior,
                        &theme,
                    );
                } else if !rendered {
                    frame.buffer_mut().set_string(
                        0,
                        0,
                        "Resize terminal or choose a smaller preset",
                        Style::default().fg(theme.text),
                    );
                }
            }
        })?;
    }
}

fn context_for(area: ratatui::layout::Rect) -> DashboardContext {
    if area.width >= 100 && area.height >= 28 {
        return DashboardContext::default();
    }

    let mut context = DashboardContext::default();
    for region in [
        dashboard_v2::DashboardRegion::SecondaryMetrics,
        dashboard_v2::DashboardRegion::Charts,
        dashboard_v2::DashboardRegion::Processes,
        dashboard_v2::DashboardRegion::Alerts,
    ] {
        context = context.without(region);
    }
    context
}

fn render_health_debug(
    frame: &mut ratatui::Frame<'_>,
    scenario: Scenario,
    health: dashboard_v2::SystemHealth,
    behavior: PulseBehavior,
    theme: &Theme,
) {
    let area = frame.area();
    if area.width == 0 || area.height == 0 {
        return;
    }
    let line = format!(
        "HEALTH DEBUG · {} · {} · PULSE {}",
        scenario.label(),
        health,
        behavior,
    );
    frame.buffer_mut().set_stringn(
        area.x,
        area.y,
        line,
        usize::from(area.width),
        Style::default().fg(theme.text_muted),
    );
}

fn handle_key(code: KeyCode, state: &mut ViewerState) -> bool {
    match code {
        KeyCode::Char('p') | KeyCode::Char('P') => state.next_preset(),
        KeyCode::Char('s') | KeyCode::Char('S') => state.scenario = state.scenario.next(),
        KeyCode::Char('g') | KeyCode::Char('G') => state.gpu_available = !state.gpu_available,
        KeyCode::Char('r') | KeyCode::Char('R') => state.reduced_motion = !state.reduced_motion,
        KeyCode::Char('l') | KeyCode::Char('L') => state.live_metrics = !state.live_metrics,
        KeyCode::Char('f') | KeyCode::Char('F') => state.focused_cpu = !state.focused_cpu,
        KeyCode::Char('c') | KeyCode::Char('C') => state.cpu_only = !state.cpu_only,
        KeyCode::Char('h') | KeyCode::Char('H') => state.health_debug = !state.health_debug,
        KeyCode::Char('v') | KeyCode::Char('V') => {
            state.cpu_preview_size = state.cpu_preview_size.next()
        }
        KeyCode::Char('d') | KeyCode::Char('D') => {
            state.cpu_signal_preview = state.cpu_signal_preview.next()
        }
        KeyCode::Char(' ') => state.paused = !state.paused,
        KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc => return true,
        _ => {}
    }
    false
}

fn render_cpu_preview(
    frame: &mut ratatui::Frame<'_>,
    state: ViewerState,
    live_metrics: &LiveMetricsHistory,
    theme: &Theme,
) {
    let area = frame.area();
    let (width, height) = state.cpu_preview_size.dimensions();
    if area.width < width || area.height < height {
        frame.buffer_mut().set_string(
            0,
            0,
            "Resize terminal to inspect the CPU preview",
            Style::default().fg(theme.text),
        );
        return;
    }

    let outer = Rect::new(
        area.x + (area.width - width) / 2,
        area.y + (area.height - height) / 2,
        width,
        height,
    );
    frame.buffer_mut().set_string(
        area.x,
        area.y,
        format!(
            "CPU SIGNAL · {} · D cycles",
            state.cpu_signal_preview.label()
        ),
        Style::default().fg(theme.text_muted),
    );
    let inner = Rect::new(
        outer.x + 1,
        outer.y + 1,
        outer.width.saturating_sub(2),
        outer.height.saturating_sub(2),
    );
    frame.render_widget(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.border))
            .style(Style::default().bg(theme.surface)),
        outer,
    );

    let context = WidgetContext::new(
        DashboardPreset::Widescreen,
        state.scenario.dashboard_state(),
        state.elapsed,
    )
    .with_focus(
        state
            .focused_cpu
            .then_some(dashboard_v2::DashboardWidgetId::Cpu),
    );
    let model = if state.live_metrics {
        compose_live_cpu(live_metrics, context)
    } else {
        compose_cpu_preview(state, context)
    };
    if let Some(model) = model {
        let _ = TerminalRenderer::new().render_rect(
            model,
            DashboardPreset::Widescreen,
            inner,
            theme,
            frame.buffer_mut(),
        );
    }
}

#[allow(dead_code)]
fn content(
    state: ViewerState,
    live_metrics: &LiveMetricsHistory,
    live_system_context: Option<&DashboardV2SystemContext>,
) -> DashboardV2Content {
    content_with_health(state, live_metrics, live_system_context, None)
}

fn content_with_health(
    state: ViewerState,
    live_metrics: &LiveMetricsHistory,
    live_system_context: Option<&DashboardV2SystemContext>,
    system_health: Option<dashboard_v2::SystemHealth>,
) -> DashboardV2Content {
    let pulse_behavior = system_health.map(PulseBehavior::from_system_health);
    content_with_health_and_behavior(
        state,
        live_metrics,
        live_system_context,
        system_health,
        pulse_behavior,
    )
}

fn content_with_health_and_behavior(
    state: ViewerState,
    live_metrics: &LiveMetricsHistory,
    live_system_context: Option<&DashboardV2SystemContext>,
    system_health: Option<dashboard_v2::SystemHealth>,
    pulse_behavior: Option<PulseBehavior>,
) -> DashboardV2Content {
    let live_models = state
        .live_metrics
        .then(|| live_metrics.models().ok())
        .flatten();
    let dashboard_state = if let Some(models) = live_models.as_ref() {
        dashboard_state_for_models(models)
    } else {
        state.scenario.dashboard_state()
    };
    let context = WidgetContext::new(state.preset, dashboard_state, state.elapsed).with_focus(
        state
            .focused_cpu
            .then_some(dashboard_v2::DashboardWidgetId::Cpu),
    );
    let primary = if let Some(models) = live_models.as_ref() {
        [
            compose_live_cpu_model(models.cpu(), context),
            compose_live_memory(models.memory(), context),
            compose_live_gpu(models.gpu(), context),
            compose_live_network(models.network(), context),
            compose_live_storage(models.storage(), context),
        ]
    } else if state.live_metrics {
        // Live mode has no meaningful metric until the collector publishes a
        // snapshot. Keep the surface honest instead of borrowing demo models.
        [None; 5]
    } else {
        [
            compose_cpu(state, context),
            compose_memory(state, context),
            compose_gpu(state, context),
            compose_network(state, context),
            compose_storage(state, context),
        ]
    };
    let inventory = live_models
        .as_ref()
        .map(|models| models.inventory().clone())
        .unwrap_or_default();
    let pulse_behavior = pulse_behavior
        .or_else(|| system_health.map(PulseBehavior::from_system_health))
        .or_else(|| {
            live_models.as_ref().map(|models| {
                let health = dashboard_v2::HealthEngine::new()
                    .evaluate(dashboard_v2::HealthSnapshot::from_models(models));
                PulseBehavior::from_system_health(health)
            })
        })
        .or_else(|| {
            (!state.live_metrics).then(|| {
                let health =
                    dashboard_v2::HealthEngine::new().evaluate(scenario_health_snapshot(state));
                PulseBehavior::from_system_health(health)
            })
        });
    let pulse_input = pulse_behavior.map(PulseBehavior::input).unwrap_or_else(|| {
        if state.live_metrics {
            // A live frame without its first snapshot is intentionally
            // quiet. It must not borrow the demo scenario while the
            // instruments are still waiting for real data.
            NormalizedPulseInput::default()
        } else {
            // Demo scenarios also pass through HealthEngine above. This
            // fallback is only a defensive empty boundary.
            NormalizedPulseInput::default()
        }
    });
    let content = DashboardV2Content::new(primary, pulse_input, dashboard_state)
        .with_inventory(inventory)
        .with_live_mode(state.live_metrics);
    let content = if let Some(behavior) = pulse_behavior {
        content.with_pulse_behavior(behavior)
    } else {
        content
    };
    if state.live_metrics {
        content.with_system_context(live_system_context.cloned().unwrap_or_default())
    } else {
        content
    }
}

fn scenario_health_snapshot(state: ViewerState) -> HealthSnapshot {
    let (cpu, memory, gpu, network, load, volatility) = state.scenario.values();
    let cpu_pressure = match state.scenario {
        Scenario::Normal | Scenario::Recovery => 0.06,
        Scenario::Active
        | Scenario::GpuActivity
        | Scenario::NetworkActivity
        | Scenario::StoragePressure => 0.12,
        Scenario::Pressure => 0.16,
        Scenario::Thermal => 0.22,
        Scenario::Critical => 0.98,
    };
    let memory_pressure = (memory - 0.65).max(0.0) / 0.35;
    let storage_pressure = ((load - 0.80) / 0.15).clamp(0.0, 1.0) as f32;
    let thermal = match state.scenario {
        Scenario::Thermal => 0.86,
        Scenario::Critical => 0.98,
        _ => 0.0,
    };
    let gpu_signals = state.gpu_available.then_some(HealthSignals::new(
        Some(gpu as f32),
        Some((gpu * 0.30).clamp(0.0, 1.0) as f32),
        None,
        Some(volatility as f32),
    ));
    HealthSnapshot::default()
        .with_component(
            HealthComponent::Cpu,
            HealthSignals::new(
                Some(cpu as f32),
                Some(cpu_pressure),
                None,
                Some(volatility as f32),
            ),
        )
        .with_component(
            HealthComponent::Memory,
            HealthSignals::new(None, Some(memory_pressure as f32), None, None),
        )
        .with_component(
            HealthComponent::Network,
            HealthSignals::new(Some(network as f32), None, None, None),
        )
        .with_component(HealthComponent::Gpu, gpu_signals.unwrap_or_default())
        .with_component(
            HealthComponent::Storage,
            HealthSignals::new(Some(volatility as f32), Some(storage_pressure), None, None),
        )
        .with_component(
            HealthComponent::Temperature,
            HealthSignals::new(None, None, (thermal > 0.0).then_some(thermal), None),
        )
}

fn system_context_from_system(snapshot: &SystemInfo) -> DashboardV2SystemContext {
    let mut processes = snapshot
        .processes_list
        .iter()
        .map(|process| DashboardProcessSummary {
            name: process.name.clone(),
            cpu_percent: process.cpu_usage,
            memory_bytes: if process.memory.is_finite() && process.memory >= 0.0 {
                (process.memory * 1024.0 * 1024.0) as u64
            } else {
                0
            },
        })
        .collect::<Vec<_>>();
    processes.sort_unstable_by(|left, right| {
        right
            .cpu_percent
            .total_cmp(&left.cpu_percent)
            .then_with(|| right.memory_bytes.cmp(&left.memory_bytes))
            .then_with(|| left.name.cmp(&right.name))
    });

    DashboardV2SystemContext {
        physical_cores: snapshot.cpu_physical_cores,
        logical_threads: Some(snapshot.cpu_logical_threads.max(snapshot.cpu_cores)),
        total_memory_gib: Some(snapshot.total_ram).filter(|value| value.is_finite()),
        uptime_seconds: Some(snapshot.uptime),
        processes,
    }
}

fn compose_live_cpu(
    live_metrics: &LiveMetricsHistory,
    context: WidgetContext,
) -> Option<dashboard_v2::WidgetRenderModel> {
    compose_live_cpu_model(live_metrics.models().ok()?.cpu(), context)
}

fn compose_live_cpu_model(
    model: dashboard_v2::CpuMetricModel<{ dashboard_v2::LIVE_METRICS_HISTORY_CAPACITY }>,
    context: WidgetContext,
) -> Option<dashboard_v2::WidgetRenderModel> {
    WidgetRenderer::render_cpu(model, context).ok().flatten()
}

fn compose_live_memory(
    model: dashboard_v2::MemoryMetricModel<{ dashboard_v2::LIVE_METRICS_HISTORY_CAPACITY }>,
    context: WidgetContext,
) -> Option<dashboard_v2::WidgetRenderModel> {
    WidgetRenderer::render_memory(model, context).ok().flatten()
}

fn compose_live_gpu(
    model: dashboard_v2::GpuMetricModel<{ dashboard_v2::LIVE_METRICS_HISTORY_CAPACITY }>,
    context: WidgetContext,
) -> Option<dashboard_v2::WidgetRenderModel> {
    WidgetRenderer::render_gpu(model, context).ok().flatten()
}

fn compose_live_network(
    model: dashboard_v2::NetworkMetricModel<{ dashboard_v2::LIVE_METRICS_HISTORY_CAPACITY }>,
    context: WidgetContext,
) -> Option<dashboard_v2::WidgetRenderModel> {
    WidgetRenderer::render_network(model, context)
        .ok()
        .flatten()
}

fn compose_live_storage(
    model: dashboard_v2::StorageMetricModel<{ dashboard_v2::LIVE_METRICS_HISTORY_CAPACITY }>,
    context: WidgetContext,
) -> Option<dashboard_v2::WidgetRenderModel> {
    WidgetRenderer::render_storage(model, context)
        .ok()
        .flatten()
}

fn dashboard_state_for_models(models: &dashboard_v2::LiveMetricModels) -> DashboardState {
    let states = [
        models.cpu().state(),
        models.memory().state(),
        models.gpu().state(),
        models.network().state(),
        models.storage().state(),
    ];
    if states.contains(&dashboard_v2::WidgetState::Critical) {
        DashboardState::Critical
    } else if states.contains(&dashboard_v2::WidgetState::Warning) {
        DashboardState::Busy
    } else if states.contains(&dashboard_v2::WidgetState::Active) {
        DashboardState::Active
    } else {
        DashboardState::Calm
    }
}

fn cpu_live_sample_from_snapshot(snapshot: &SystemInfo) -> CpuLiveSample {
    CpuLiveSample {
        utilization_percent: valid_percent(f64::from(snapshot.cpu_usage)),
        frequency_hz: valid_non_negative(snapshot.cpu_frequency_hz),
        temperature_celsius: valid_non_negative(snapshot.cpu_temperature_celsius),
        cores: snapshot.cpu_physical_cores.unwrap_or(snapshot.cpu_cores),
    }
}

fn live_snapshot_from_system(snapshot: &SystemInfo) -> LiveSnapshot {
    let gpus = snapshot
        .gpus
        .iter()
        .filter_map(|gpu| {
            Some(GpuDevice {
                id: HardwareId::new(HardwareKind::Gpu, gpu.id.clone())?,
                name: gpu.name.clone(),
                utilization_percent: gpu
                    .usage_percent
                    .or_else(|| parse_percent(&gpu.usage))
                    .and_then(valid_percent),
                vram_used_bytes: valid_non_negative(gpu.vram_used_bytes),
                vram_total_bytes: valid_non_negative(gpu.vram_total_bytes),
                temperature_celsius: valid_non_negative(gpu.temperature_celsius),
                frequency_hz: valid_non_negative(gpu.frequency_hz),
                power_watts: valid_non_negative(gpu.power_watts),
                availability: MetricAvailability::Available,
            })
        })
        .collect::<Vec<_>>();
    let storages = snapshot
        .storage_devices
        .iter()
        .filter(|disk| !disk.is_virtual)
        .filter_map(|disk| {
            Some(StorageDevice {
                id: HardwareId::new(HardwareKind::Storage, disk.id.clone())?,
                name: disk.name.clone(),
                model: disk.model.clone(),
                mount_points: disk.mount_points.clone(),
                used_bytes: valid_non_negative(Some(
                    (disk.total_space - disk.available_space).max(0.0) * GIBIBYTE,
                )),
                total_bytes: valid_non_negative(Some(disk.total_space.max(0.0) * GIBIBYTE)),
                read_bytes_per_sec: valid_non_negative(disk.read_bytes_per_sec),
                write_bytes_per_sec: valid_non_negative(disk.written_bytes_per_sec),
                latency_seconds: None,
                availability: MetricAvailability::Available,
                is_virtual: false,
            })
        })
        .collect::<Vec<_>>();
    let networks = snapshot
        .all_networks
        .iter()
        .filter_map(|network| {
            Some(NetworkInterface {
                id: HardwareId::new(HardwareKind::Network, network.id.clone())?,
                name: network.interface.clone(),
                download_bytes_per_sec: valid_non_negative(Some(
                    network.received_per_sec * 1_000_000.0,
                )),
                upload_bytes_per_sec: valid_non_negative(Some(
                    network.transmitted_per_sec * 1_000_000.0,
                )),
                availability: MetricAvailability::Available,
            })
        })
        .collect::<Vec<_>>();
    let temperatures = snapshot
        .temperatures
        .iter()
        .filter_map(|sensor| {
            Some(TemperatureSensor {
                id: HardwareId::new(HardwareKind::Temperature, sensor.id.clone())?,
                label: sensor.label.clone(),
                celsius: valid_non_negative(sensor.celsius),
                availability: MetricAvailability::Available,
            })
        })
        .collect::<Vec<_>>();
    let hardware = HardwareInventory::new(storages, gpus, networks, temperatures);
    let gpu = hardware.gpus.first().map(|gpu| LiveGpuSnapshot {
        utilization_percent: gpu.utilization_percent,
        vram_used_bytes: valid_non_negative(gpu.vram_used_bytes),
        vram_total_bytes: valid_non_negative(gpu.vram_total_bytes),
        temperature_celsius: valid_non_negative(gpu.temperature_celsius),
        frequency_hz: valid_non_negative(gpu.frequency_hz),
        power_watts: valid_non_negative(gpu.power_watts),
    });
    let storage = hardware.storages.first().map(|disk| LiveStorageSnapshot {
        used_bytes: disk.used_bytes,
        total_bytes: disk.total_bytes,
        read_bytes_per_sec: disk.read_bytes_per_sec,
        write_bytes_per_sec: disk.write_bytes_per_sec,
    });
    let (download, upload) = snapshot
        .all_networks
        .iter()
        .filter(|network| !is_loopback_interface(&network.interface))
        .fold((0.0, 0.0), |(down, up), network| {
            (
                down + network.received_per_sec * 1_000_000.0,
                up + network.transmitted_per_sec * 1_000_000.0,
            )
        });
    LiveSnapshot {
        cpu: cpu_live_sample_from_snapshot(snapshot),
        memory_used_bytes: valid_non_negative(Some(snapshot.used_ram * GIBIBYTE)),
        memory_available_bytes: valid_non_negative(Some(
            (snapshot.total_ram - snapshot.used_ram).max(0.0) * GIBIBYTE,
        )),
        memory_total_bytes: valid_non_negative(Some(snapshot.total_ram * GIBIBYTE)),
        memory_swap_bytes: valid_non_negative(Some(snapshot.used_swap * GIBIBYTE)),
        gpu,
        download_bytes_per_sec: Some(download),
        upload_bytes_per_sec: Some(upload),
        storage,
        hardware,
    }
}

const GIBIBYTE: f64 = 1024.0 * 1024.0 * 1024.0;

fn is_loopback_interface(name: &str) -> bool {
    name == "lo" || name.starts_with("lo:")
}

fn parse_percent(value: &str) -> Option<f64> {
    value.trim_end_matches('%').trim().parse::<f64>().ok()
}

fn valid_percent(value: f64) -> Option<f64> {
    value
        .is_finite()
        .then_some(value)
        .filter(|value| (0.0..=100.0).contains(value))
}

fn valid_non_negative(value: Option<f64>) -> Option<f64> {
    value.filter(|value| value.is_finite() && *value >= 0.0)
}

fn compose_cpu(
    state: ViewerState,
    context: WidgetContext,
) -> Option<dashboard_v2::WidgetRenderModel> {
    WidgetRenderer::render_cpu(cpu_model(state.scenario), context)
        .ok()
        .flatten()
}

fn compose_cpu_preview(
    state: ViewerState,
    context: WidgetContext,
) -> Option<dashboard_v2::WidgetRenderModel> {
    WidgetRenderer::render_cpu(cpu_preview_model(state), context)
        .ok()
        .flatten()
}

fn compose_memory(
    state: ViewerState,
    context: WidgetContext,
) -> Option<dashboard_v2::WidgetRenderModel> {
    WidgetRenderer::render_memory(memory_model(state.scenario), context)
        .ok()
        .flatten()
}

fn compose_gpu(
    state: ViewerState,
    context: WidgetContext,
) -> Option<dashboard_v2::WidgetRenderModel> {
    WidgetRenderer::render_gpu(gpu_model(state.scenario, state.gpu_available), context)
        .ok()
        .flatten()
}

fn compose_network(
    state: ViewerState,
    context: WidgetContext,
) -> Option<dashboard_v2::WidgetRenderModel> {
    WidgetRenderer::render_network(network_model(state.scenario), context)
        .ok()
        .flatten()
}

fn compose_storage(
    state: ViewerState,
    context: WidgetContext,
) -> Option<dashboard_v2::WidgetRenderModel> {
    WidgetRenderer::render_storage(storage_model(state.scenario), context)
        .ok()
        .flatten()
}

fn cpu_model(scenario: Scenario) -> dashboard_v2::CpuMetricModel<HISTORY> {
    let (scenario_cpu, _, _, _, _, _) = scenario.values();
    let cpu = if scenario == Scenario::Normal {
        0.42
    } else {
        scenario_cpu
    };
    cpu_model_with_history(scenario, &cpu_history(cpu))
}

fn cpu_preview_model(state: ViewerState) -> dashboard_v2::CpuMetricModel<HISTORY> {
    let values = match state.cpu_signal_preview {
        CpuSignalPreview::Scenario => return cpu_model(state.scenario),
        CpuSignalPreview::Low => [0.08, 0.09, 0.12, 0.07, 0.10, 0.11],
        CpuSignalPreview::Mid => [0.45, 0.52, 0.48, 0.56, 0.50, 0.53],
        CpuSignalPreview::High => [0.86, 0.92, 0.89, 0.95, 0.91, 0.93],
    };
    cpu_model_with_history(state.scenario, &values)
}

fn cpu_model_with_history(
    scenario: Scenario,
    values: &[f64],
) -> dashboard_v2::CpuMetricModel<HISTORY> {
    let cpu = values.last().copied().unwrap_or_else(|| {
        let (scenario_cpu, _, _, _, _, _) = scenario.values();
        if scenario == Scenario::Normal {
            0.42
        } else {
            scenario_cpu
        }
    });
    dashboard_v2::CpuMetricModel::from_input(
        CpuMetricInput::new(
            normalized(cpu),
            normalized((cpu * 0.68).min(1.0)),
            normalized((cpu * 0.22).min(1.0)),
            normalized((1.0 - cpu * 0.90).max(0.0)),
            non_negative(3_200_000_000.0),
            non_negative(match scenario {
                Scenario::Normal | Scenario::Recovery => 52.0,
                Scenario::Active => 62.0,
                Scenario::GpuActivity => 64.0,
                Scenario::Pressure => 58.0,
                Scenario::Thermal => 88.0,
                Scenario::NetworkActivity => 52.0,
                Scenario::StoragePressure => 54.0,
                Scenario::Critical => 96.0,
            }),
            8,
            history_with_step(MetricUnit::Percent, values, Duration::from_secs(5)),
        )
        .expect("deterministic CPU fixture is valid"),
    )
    .expect("deterministic CPU model is valid")
}

fn memory_model(scenario: Scenario) -> dashboard_v2::MemoryMetricModel<HISTORY> {
    let (_, memory, _, _, _, _) = scenario.values();
    dashboard_v2::MemoryMetricModel::from_input(
        MemoryMetricInput::new(
            non_negative(memory * 100.0),
            non_negative((1.0 - memory) * 100.0),
            non_negative(100.0),
            non_negative(memory * 16.0),
            non_negative(
                if matches!(scenario, Scenario::Pressure | Scenario::Critical) {
                    12.0
                } else {
                    0.0
                },
            ),
            normalized(memory),
            history(MetricUnit::Percent, &scaled_history(memory)),
        )
        .expect("deterministic memory fixture is valid"),
    )
    .expect("deterministic memory model is valid")
}

fn gpu_model(scenario: Scenario, available: bool) -> dashboard_v2::GpuMetricModel<HISTORY> {
    let (_, _, gpu, _, _, _) = scenario.values();
    let absent = MetricReading::unavailable;
    dashboard_v2::GpuMetricModel::from_input(
        GpuMetricInput::new(
            if available { normalized(gpu) } else { absent() },
            if available {
                non_negative(gpu * 80.0)
            } else {
                absent()
            },
            if available {
                non_negative(100.0)
            } else {
                absent()
            },
            if available {
                non_negative(match scenario {
                    Scenario::Normal | Scenario::Recovery => 54.0,
                    Scenario::Active => 68.0,
                    Scenario::GpuActivity => 80.0,
                    Scenario::Pressure => 58.0,
                    Scenario::Thermal => 82.0,
                    Scenario::NetworkActivity => 54.0,
                    Scenario::StoragePressure => 56.0,
                    Scenario::Critical => 96.0,
                })
            } else {
                absent()
            },
            if available {
                non_negative(1_800.0)
            } else {
                absent()
            },
            if available {
                non_negative(120.0)
            } else {
                absent()
            },
            history(MetricUnit::Percent, &scaled_history(gpu)),
        )
        .expect("deterministic GPU fixture is valid"),
    )
    .expect("deterministic GPU model is valid")
}

fn network_model(scenario: Scenario) -> dashboard_v2::NetworkMetricModel<HISTORY> {
    let (_, _, _, network, _, _) = scenario.values();
    dashboard_v2::NetworkMetricModel::from_input(
        NetworkMetricInput::new(
            non_negative(network * 80.0),
            non_negative(network * 25.0),
            non_negative(2_000.0),
            non_negative(900.0),
            non_negative(100.0),
            history(MetricUnit::BytesPerSecond, &scaled_history(network)),
            history(MetricUnit::BytesPerSecond, &scaled_history(network * 0.55)),
        )
        .expect("deterministic network fixture is valid"),
    )
    .expect("deterministic network model is valid")
}

fn storage_model(scenario: Scenario) -> dashboard_v2::StorageMetricModel<HISTORY> {
    let (_, _, _, _, occupancy, activity) = scenario.values();
    let input = StorageMetricInput::new(
        non_negative(occupancy * 1_000.0),
        non_negative(1_000.0),
        non_negative(activity * 120.0),
        non_negative(activity * 60.0),
        non_negative(match scenario {
            Scenario::Normal | Scenario::Recovery => 0.04,
            Scenario::Active | Scenario::GpuActivity => 0.08,
            Scenario::Pressure => 0.20,
            Scenario::Thermal => 0.12,
            Scenario::NetworkActivity => 0.08,
            Scenario::StoragePressure => 0.20,
            Scenario::Critical => 0.60,
        }),
        normalized(activity),
        history(MetricUnit::Percent, &scaled_history(occupancy)),
        history(MetricUnit::BytesPerSecond, &scaled_history(activity * 0.8)),
        history(MetricUnit::BytesPerSecond, &scaled_history(activity * 0.4)),
    )
    .expect("deterministic storage fixture is valid");
    dashboard_v2::StorageMetricModel::from_input(input)
        .expect("deterministic storage model is valid")
}

fn scaled_history(value: f64) -> [f64; HISTORY] {
    std::array::from_fn(|index| {
        let phase = index as f64 / (HISTORY.saturating_sub(1).max(1) as f64);
        (value * (0.78 + phase * 0.22)).clamp(0.0, 1.0)
    })
}

fn cpu_history(value: f64) -> [f64; HISTORY] {
    const SHAPE: [f64; HISTORY] = [
        0.76, 0.92, 0.82, 1.08, 0.86, 1.16, 0.90, 1.10, 0.94, 1.14, 0.81, 1.0,
    ];
    SHAPE.map(|factor| (value * factor).clamp(0.0, 1.0))
}

fn history<const CAPACITY: usize>(unit: MetricUnit, values: &[f64]) -> TimeSeries<CAPACITY> {
    history_with_step(unit, values, Duration::from_secs(1))
}

fn history_with_step<const CAPACITY: usize>(
    unit: MetricUnit,
    values: &[f64],
    step: Duration,
) -> TimeSeries<CAPACITY> {
    let mut series = TimeSeries::new(unit);
    for (index, value) in values.iter().copied().enumerate() {
        series
            .push(
                TimeSample::new(
                    step.saturating_mul(index as u32),
                    Some(value),
                    MetricAvailability::Available,
                    None,
                )
                .expect("deterministic time sample is valid"),
            )
            .expect("deterministic history has fixed capacity");
    }
    series
}

fn normalized(value: f64) -> MetricReading {
    MetricReading::normalized(Some(value.clamp(0.0, 1.0)), MetricAvailability::Available)
        .expect("normalized fixture is valid")
}

fn non_negative(value: f64) -> MetricReading {
    MetricReading::non_negative(Some(value.max(0.0)), MetricAvailability::Available)
        .expect("non-negative fixture is valid")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn viewer_fixture_covers_all_states_and_preserves_gpu_absence() {
        let mut state = ViewerState::new();
        let live_metrics = LiveMetricsHistory::new();
        for scenario in Scenario::ALL {
            state.scenario = scenario;
            let content = content(state, &live_metrics, None);
            assert_eq!(content.state(), scenario.dashboard_state());
            assert!(content.primary().iter().all(Option::is_some));
        }
        state.gpu_available = false;
        let unavailable = content(state, &live_metrics, None).primary()[2].unwrap();
        assert_eq!(unavailable.state(), dashboard_v2::WidgetState::Unavailable);
    }

    #[test]
    fn viewer_state_changes_are_deterministic_and_reduced_motion_is_preserved() {
        let mut state = ViewerState::new();
        state.next_preset();
        assert_eq!(state.preset, DashboardPreset::Widescreen);
        state.scenario = state.scenario.next();
        state.reduced_motion = true;
        state.paused = true;
        let live_metrics = LiveMetricsHistory::new();
        let first = content(state, &live_metrics, None);
        let second = content(state, &live_metrics, None);
        assert_eq!(first, second);
        assert!(state.reduced_motion);
        assert!(state.paused);
    }

    #[test]
    fn health_scenarios_use_the_same_health_to_behavior_pipeline() {
        let mut state = ViewerState::new();
        let idle = PulseBehavior::from_system_health(
            HealthEngine::new().evaluate(scenario_health_snapshot(state)),
        );

        state.scenario = Scenario::Active;
        let activity = PulseBehavior::from_system_health(
            HealthEngine::new().evaluate(scenario_health_snapshot(state)),
        );
        state.scenario = Scenario::Pressure;
        let pressure = PulseBehavior::from_system_health(
            HealthEngine::new().evaluate(scenario_health_snapshot(state)),
        );
        state.scenario = Scenario::Thermal;
        let thermal = PulseBehavior::from_system_health(
            HealthEngine::new().evaluate(scenario_health_snapshot(state)),
        );

        assert!(activity.energy() > idle.energy());
        assert!(activity.tension() < pressure.tension());
        assert!(thermal.tension() > idle.tension());
        assert!(
            Scenario::ALL
                .iter()
                .all(|scenario| !scenario.label().is_empty())
        );

        let mut health_engine = HealthEngine::new();
        let mut behavior_engine = PulseBehaviorEngine::new();
        state.scenario = Scenario::Critical;
        let critical = behavior_engine.update(
            health_engine.update(scenario_health_snapshot(state), Duration::from_secs(1)),
            Duration::from_secs(1),
        );
        state.scenario = Scenario::Recovery;
        let recovering = behavior_engine.update(
            health_engine.update(scenario_health_snapshot(state), Duration::from_millis(100)),
            Duration::from_millis(100),
        );
        assert!(recovering.tension() < critical.tension());
        assert!(recovering.tension() > idle.tension());
    }

    #[test]
    fn cpu_preview_cycles_through_reference_sizes_without_changing_semantics() {
        let mut state = ViewerState::new();
        assert_eq!(state.cpu_preview_size.dimensions(), (45, 12));
        handle_key(KeyCode::Char('c'), &mut state);
        handle_key(KeyCode::Char('v'), &mut state);
        assert!(state.cpu_only);
        assert_eq!(state.cpu_preview_size.dimensions(), (50, 14));
        let live_metrics = LiveMetricsHistory::new();
        let first = content(state, &live_metrics, None);
        let second = content(state, &live_metrics, None);
        assert_eq!(first, second);
    }

    #[test]
    fn cpu_signal_preview_cycles_through_low_mid_and_high_regions() {
        let mut state = ViewerState::new();
        handle_key(KeyCode::Char('c'), &mut state);
        assert_eq!(state.cpu_signal_preview, CpuSignalPreview::Scenario);

        handle_key(KeyCode::Char('d'), &mut state);
        assert_eq!(state.cpu_signal_preview, CpuSignalPreview::Low);
        handle_key(KeyCode::Char('d'), &mut state);
        assert_eq!(state.cpu_signal_preview, CpuSignalPreview::Mid);
        handle_key(KeyCode::Char('d'), &mut state);
        assert_eq!(state.cpu_signal_preview, CpuSignalPreview::High);
        handle_key(KeyCode::Char('d'), &mut state);
        assert_eq!(state.cpu_signal_preview, CpuSignalPreview::Scenario);
    }

    #[test]
    fn live_mode_uses_snapshot_cpu_fields_without_touching_demo_widgets() {
        let snapshot = SystemInfo {
            os_name: "Linux".to_owned(),
            kernel: "test".to_owned(),
            cpu_name: "CPU".to_owned(),
            cpu_cores: 12,
            cpu_physical_cores: Some(6),
            cpu_logical_threads: 12,
            cpu_usage: 63.5,
            cpu_frequency_hz: Some(3.4e9),
            cpu_temperature_celsius: Some(61.0),
            total_ram: 1.0,
            used_ram: 0.5,
            total_swap: 0.0,
            used_swap: 0.0,
            processes: 0,
            uptime: 0,
            load_average: (0.0, 0.0, 0.0),
            disks: Vec::new(),
            storage_devices: Vec::new(),
            processes_list: Vec::new(),
            gpus: Vec::new(),
            networks: Vec::new(),
            all_networks: Vec::new(),
            temperatures: Vec::new(),
        };
        let mut live_metrics = LiveMetricsHistory::new();
        let collected_at = Instant::now();
        live_metrics
            .push_snapshot(collected_at, &live_snapshot_from_system(&snapshot))
            .unwrap();

        let mut state = ViewerState::new();
        state.live_metrics = true;
        let content = content(state, &live_metrics, None);
        let cpu = content.primary()[0].unwrap();
        assert_eq!(cpu.visual().primary().unwrap().value(), 0.635);
        assert!(content.pulse_behavior().is_some());
        assert_ne!(content.pulse_input().cpu(), 0.635);
        assert_eq!(live_metrics.models().unwrap().cpu().cores(), 6);
        assert!(content.primary()[1].is_some());
        assert_eq!(
            content.primary()[2].unwrap().state(),
            dashboard_v2::WidgetState::Unavailable
        );
    }

    #[test]
    fn live_mode_starts_with_a_quiet_but_living_pulse_until_real_metrics_arrive() {
        let mut state = ViewerState::new();
        state.live_metrics = true;
        let content = content(state, &LiveMetricsHistory::new(), None);
        assert!(content.pulse_input().cpu() > 0.0);
        assert!(content.pulse_input().load() < 0.20);
        assert!(content.primary().iter().all(|model| {
            model.is_some_and(|model| model.state() == dashboard_v2::WidgetState::Unavailable)
        }));
    }

    #[test]
    fn dashboard_pulse_boundary_accepts_health_without_changing_instruments() {
        let state = ViewerState::new();
        let health = HealthEngine::new().evaluate(HealthSnapshot::default().with_component(
            HealthComponent::Memory,
            HealthSignals::new(None, Some(0.92), None, None),
        ));
        let content = content_with_health(state, &LiveMetricsHistory::new(), None, Some(health));

        assert_eq!(
            content
                .primary()
                .iter()
                .filter(|model| model.is_some())
                .count(),
            5
        );
        assert!(content.pulse_behavior().is_some());
        assert!(content.pulse_input().memory() > 0.0);
        assert!(content.pulse_input().load() > 0.0);
    }

    #[test]
    fn live_adapter_keeps_multiple_physical_storage_devices() {
        let disks = vec![
            system::info::DiskInfo {
                id: "sda".to_owned(),
                name: "sda".to_owned(),
                model: Some("SATA SSD".to_owned()),
                mount_point: "/".to_owned(),
                mount_points: vec!["/".to_owned(), "/home".to_owned()],
                total_space: 443.0,
                available_space: 100.0,
                is_virtual: false,
                read_bytes_per_sec: Some(0.0),
                written_bytes_per_sec: Some(0.0),
            },
            system::info::DiskInfo {
                id: "nvme0n1".to_owned(),
                name: "nvme0n1".to_owned(),
                model: Some("NVMe SSD".to_owned()),
                mount_point: "/games".to_owned(),
                mount_points: vec!["/games".to_owned()],
                total_space: 931.0,
                available_space: 800.0,
                is_virtual: false,
                read_bytes_per_sec: Some(1.0),
                written_bytes_per_sec: Some(2.0),
            },
        ];
        let snapshot = SystemInfo {
            os_name: "Linux".to_owned(),
            kernel: "test".to_owned(),
            cpu_name: "CPU".to_owned(),
            cpu_cores: 4,
            cpu_physical_cores: None,
            cpu_logical_threads: 4,
            cpu_usage: 10.0,
            cpu_frequency_hz: None,
            cpu_temperature_celsius: None,
            total_ram: 16.0,
            used_ram: 4.0,
            total_swap: 0.0,
            used_swap: 0.0,
            processes: 0,
            uptime: 0,
            load_average: (0.0, 0.0, 0.0),
            disks: disks.clone(),
            storage_devices: disks,
            processes_list: Vec::new(),
            gpus: Vec::new(),
            networks: Vec::new(),
            all_networks: Vec::new(),
            temperatures: Vec::new(),
        };
        let live = live_snapshot_from_system(&snapshot);

        assert_eq!(live.hardware.storages.len(), 2);
        assert_eq!(live.hardware.storages[0].id.key(), "nvme0n1");
        assert_eq!(live.hardware.storages[1].id.key(), "sda");
        assert_eq!(live.hardware.storages[1].mount_points.len(), 2);
    }

    #[test]
    fn live_standard_composition_keeps_both_storage_devices_visible() {
        let disks = vec![
            system::info::DiskInfo {
                id: "sda".to_owned(),
                name: "sda".to_owned(),
                model: Some("KINGSTON SA400S37480G".to_owned()),
                mount_point: "/".to_owned(),
                mount_points: vec!["/".to_owned()],
                total_space: 447.1,
                available_space: 35.0,
                is_virtual: false,
                read_bytes_per_sec: Some(0.0),
                written_bytes_per_sec: Some(0.0),
            },
            system::info::DiskInfo {
                id: "nvme0n1".to_owned(),
                name: "nvme0n1".to_owned(),
                model: Some("KINGSTON SNV2S1000G".to_owned()),
                mount_point: "/games".to_owned(),
                mount_points: vec!["/games".to_owned()],
                total_space: 931.5,
                available_space: 716.0,
                is_virtual: false,
                read_bytes_per_sec: Some(0.0),
                written_bytes_per_sec: Some(0.0),
            },
        ];
        let snapshot = SystemInfo {
            os_name: "Linux".to_owned(),
            kernel: "test".to_owned(),
            cpu_name: "CPU".to_owned(),
            cpu_cores: 12,
            cpu_physical_cores: Some(6),
            cpu_logical_threads: 12,
            cpu_usage: 10.0,
            cpu_frequency_hz: None,
            cpu_temperature_celsius: None,
            total_ram: 16.0,
            used_ram: 4.0,
            total_swap: 0.0,
            used_swap: 0.0,
            processes: 0,
            uptime: 0,
            load_average: (0.0, 0.0, 0.0),
            disks: disks.clone(),
            storage_devices: disks,
            processes_list: Vec::new(),
            gpus: Vec::new(),
            networks: Vec::new(),
            all_networks: Vec::new(),
            temperatures: Vec::new(),
        };
        let live = live_snapshot_from_system(&snapshot);
        let mut history = LiveMetricsHistory::new();
        history
            .push_snapshot(Instant::now(), &live)
            .expect("live fixture should be valid");
        let mut state = ViewerState::new();
        state.live_metrics = true;
        let system_context = system_context_from_system(&snapshot);
        let content = content(state, &history, Some(&system_context));
        let mut buffer = ratatui::buffer::Buffer::empty(ratatui::layout::Rect::new(0, 0, 160, 40));
        DashboardV2Renderer::new()
            .render(
                DashboardPreset::Standard,
                DashboardContext::default(),
                content,
                Duration::ZERO,
                true,
                &Theme::pulse(),
                &mut buffer,
            )
            .expect("live dashboard should render");
        let text: String = buffer.content.iter().map(|cell| cell.symbol()).collect();
        assert!(text.contains("sda"));
        assert!(text.contains("nvme0n1"));
        assert!(text.contains("447.1 GB"));
        assert!(text.contains("931.5 GB"));
        assert!(text.contains("6 physical"));
        assert!(text.contains("no live events"));
        assert!(text.contains("no live processes"));
        assert!(!text.contains("browser"));
        assert!(!text.contains("compiler"));
    }
}
