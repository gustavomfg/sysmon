#[allow(dead_code)]
#[path = "../ui/dashboard_v2/mod.rs"]
pub mod dashboard_v2;

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
    time::{Duration, Instant},
};

use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use dashboard_v2::{
    CpuMetricInput, DashboardContext, DashboardPreset, DashboardState, DashboardV2Content,
    DashboardV2Renderer, GpuMetricInput, MemoryMetricInput, MetricAvailability, MetricReading,
    MetricUnit, NetworkMetricInput, StorageMetricInput, TimeSample, TimeSeries, WidgetContext,
    WidgetRenderer,
};
use pulse_v2::NormalizedPulseInput;
use ratatui::{Terminal, backend::CrosstermBackend};
use theme::Theme;

const FRAME_INTERVAL: Duration = Duration::from_millis(33);
const HISTORY: usize = 12;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Scenario {
    Normal,
    Active,
    Warning,
    Critical,
}

impl Scenario {
    const ALL: [Self; 4] = [Self::Normal, Self::Active, Self::Warning, Self::Critical];

    const fn index(self) -> usize {
        match self {
            Self::Normal => 0,
            Self::Active => 1,
            Self::Warning => 2,
            Self::Critical => 3,
        }
    }

    const fn next(self) -> Self {
        Self::ALL[(self.index() + 1) % Self::ALL.len()]
    }

    const fn values(self) -> (f64, f64, f64, f64, f64, f64) {
        match self {
            Self::Normal => (0.25, 0.35, 0.25, 0.12, 0.35, 0.20),
            Self::Active => (0.65, 0.68, 0.65, 0.55, 0.72, 0.50),
            Self::Warning => (0.82, 0.86, 0.82, 0.70, 0.88, 0.90),
            Self::Critical => (0.98, 0.97, 0.98, 0.95, 0.98, 0.98),
        }
    }

    const fn dashboard_state(self) -> DashboardState {
        match self {
            Self::Normal => DashboardState::Calm,
            Self::Active => DashboardState::Active,
            Self::Warning => DashboardState::Busy,
            Self::Critical => DashboardState::Critical,
        }
    }

    fn pulse_input(self, gpu_available: bool) -> NormalizedPulseInput {
        let (cpu, memory, gpu, network, load, volatility) = self.values();
        NormalizedPulseInput::try_new(
            cpu as f32,
            memory as f32,
            gpu_available.then_some(gpu as f32),
            network as f32,
            load as f32,
            volatility as f32,
        )
        .unwrap_or_default()
    }
}

#[derive(Debug, Clone, Copy)]
struct ViewerState {
    preset: DashboardPreset,
    scenario: Scenario,
    gpu_available: bool,
    reduced_motion: bool,
    paused: bool,
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

        let now = Instant::now();
        let delta = now.saturating_duration_since(last_tick);
        last_tick = now;
        if !state.paused {
            state.elapsed = state.elapsed.saturating_add(delta);
        }

        terminal.draw(|frame| {
            let content = content(state);
            let context = context_for(frame.area());
            if renderer
                .render(
                    state.preset,
                    context,
                    content,
                    state.elapsed,
                    state.reduced_motion,
                    &theme,
                    frame.buffer_mut(),
                )
                .is_err()
            {
                frame.buffer_mut().set_string(
                    0,
                    0,
                    "Resize terminal or choose a smaller preset",
                    ratatui::style::Style::default().fg(theme.text),
                );
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

fn handle_key(code: KeyCode, state: &mut ViewerState) -> bool {
    match code {
        KeyCode::Char('p') | KeyCode::Char('P') => state.next_preset(),
        KeyCode::Char('s') | KeyCode::Char('S') => state.scenario = state.scenario.next(),
        KeyCode::Char('g') | KeyCode::Char('G') => state.gpu_available = !state.gpu_available,
        KeyCode::Char('r') | KeyCode::Char('R') => state.reduced_motion = !state.reduced_motion,
        KeyCode::Char(' ') => state.paused = !state.paused,
        KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc => return true,
        _ => {}
    }
    false
}

fn content(state: ViewerState) -> DashboardV2Content {
    let dashboard_state = state.scenario.dashboard_state();
    let context = WidgetContext::new(state.preset, dashboard_state, state.elapsed);
    let primary = [
        compose_cpu(state, context),
        compose_memory(state, context),
        compose_gpu(state, context),
        compose_network(state, context),
        compose_storage(state, context),
    ];
    DashboardV2Content::new(
        primary,
        state.scenario.pulse_input(state.gpu_available),
        dashboard_state,
    )
}

fn compose_cpu(
    state: ViewerState,
    context: WidgetContext,
) -> Option<dashboard_v2::WidgetRenderModel> {
    WidgetRenderer::render_cpu(cpu_model(state.scenario), context)
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
    let (cpu, _, _, _, _, _) = scenario.values();
    dashboard_v2::CpuMetricModel::from_input(
        CpuMetricInput::new(
            normalized(cpu),
            normalized((cpu * 0.68).min(1.0)),
            normalized((cpu * 0.22).min(1.0)),
            normalized((1.0 - cpu * 0.90).max(0.0)),
            non_negative(3.2),
            non_negative(match scenario {
                Scenario::Normal => 52.0,
                Scenario::Active => 68.0,
                Scenario::Warning => 84.0,
                Scenario::Critical => 96.0,
            }),
            8,
            history(MetricUnit::Percent, &scaled_history(cpu)),
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
            non_negative(if scenario == Scenario::Critical {
                12.0
            } else {
                0.0
            }),
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
                    Scenario::Normal => 54.0,
                    Scenario::Active => 70.0,
                    Scenario::Warning => 85.0,
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
            Scenario::Normal => 0.04,
            Scenario::Active => 0.08,
            Scenario::Warning => 0.20,
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

fn history<const CAPACITY: usize>(unit: MetricUnit, values: &[f64]) -> TimeSeries<CAPACITY> {
    let mut series = TimeSeries::new(unit);
    for (index, value) in values.iter().copied().enumerate() {
        series
            .push(
                TimeSample::new(
                    Duration::from_secs(index as u64),
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
        for scenario in Scenario::ALL {
            state.scenario = scenario;
            let content = content(state);
            assert_eq!(content.state(), scenario.dashboard_state());
            assert!(content.primary().iter().all(Option::is_some));
        }
        state.gpu_available = false;
        let unavailable = content(state).primary()[2].unwrap();
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
        let first = content(state);
        let second = content(state);
        assert_eq!(first, second);
        assert!(state.reduced_motion);
        assert!(state.paused);
    }
}
