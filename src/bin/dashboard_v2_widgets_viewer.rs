#[allow(dead_code)]
#[path = "../ui/dashboard_v2/mod.rs"]
pub mod dashboard_v2;

#[allow(unused_imports)]
mod ui {
    pub use crate::dashboard_v2;
}
#[allow(dead_code)]
#[path = "../theme/mod.rs"]
mod theme;

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
    CpuMetricInput, DashboardBounds, DashboardPreset, DashboardState, GpuMetricInput,
    MemoryMetricInput, MetricAvailability, MetricReading, MetricUnit, NetworkMetricInput,
    NetworkMetricModel, PrimaryWidget, StorageMetricInput, TerminalRenderer, TimeSample,
    TimeSeries, WidgetAvailability, WidgetContext, WidgetRenderModel, widgets,
};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};
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

    const fn label(self) -> &'static str {
        match self {
            Self::Normal => "Normal",
            Self::Active => "Active",
            Self::Warning => "Warning",
            Self::Critical => "Critical",
        }
    }

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
}

#[derive(Debug, Clone, Copy)]
struct ViewerState {
    selected: Option<PrimaryWidget>,
    preset: DashboardPreset,
    scenario: Scenario,
    gpu_available: bool,
    reduced_motion: bool,
    elapsed: Duration,
}

impl ViewerState {
    const fn new() -> Self {
        Self {
            selected: Some(PrimaryWidget::Cpu),
            preset: DashboardPreset::Widescreen,
            scenario: Scenario::Normal,
            gpu_available: true,
            reduced_motion: false,
            elapsed: Duration::ZERO,
        }
    }

    fn context(self, widget: PrimaryWidget) -> WidgetContext {
        WidgetContext::new(self.preset, dashboard_state(self.scenario), self.elapsed)
            .with_availability(WidgetAvailability::all())
            .with_focus((self.selected == Some(widget)).then_some(widget.id()))
            .with_reduced_motion(self.reduced_motion)
    }

    fn next_preset(&mut self) {
        self.preset = match self.preset {
            DashboardPreset::Compact => DashboardPreset::Standard,
            DashboardPreset::Standard => DashboardPreset::Widescreen,
            DashboardPreset::Widescreen => DashboardPreset::LargeWidescreen,
            DashboardPreset::LargeWidescreen => DashboardPreset::Compact,
        };
    }

    fn select(&mut self, widget: PrimaryWidget) {
        self.selected = Some(widget);
    }

    fn toggle_all(&mut self) {
        self.selected = if self.selected.is_none() {
            Some(PrimaryWidget::Cpu)
        } else {
            None
        };
    }
}

fn dashboard_state(scenario: Scenario) -> DashboardState {
    match scenario {
        Scenario::Normal => DashboardState::Calm,
        Scenario::Active => DashboardState::Active,
        Scenario::Warning => DashboardState::Busy,
        Scenario::Critical => DashboardState::Critical,
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
        state.elapsed = state.elapsed.saturating_add(delta);
        terminal.draw(|frame| draw_frame(frame, state, &theme))?;
    }
}

fn handle_key(code: KeyCode, state: &mut ViewerState) -> bool {
    match code {
        KeyCode::Char('1') => state.select(PrimaryWidget::Cpu),
        KeyCode::Char('2') => state.select(PrimaryWidget::Memory),
        KeyCode::Char('3') => state.select(PrimaryWidget::Gpu),
        KeyCode::Char('4') => state.select(PrimaryWidget::Network),
        KeyCode::Char('5') => state.select(PrimaryWidget::Storage),
        KeyCode::Char('a') | KeyCode::Char('A') => state.toggle_all(),
        KeyCode::Char('p') | KeyCode::Char('P') => state.next_preset(),
        KeyCode::Char('s') | KeyCode::Char('S') => state.scenario = state.scenario.next(),
        KeyCode::Char('g') | KeyCode::Char('G') => state.gpu_available = !state.gpu_available,
        KeyCode::Char('r') | KeyCode::Char('R') => state.reduced_motion = !state.reduced_motion,
        KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc => return true,
        _ => {}
    }
    false
}

fn draw_frame(frame: &mut ratatui::Frame<'_>, state: ViewerState, theme: &Theme) {
    let areas = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(frame.area());
    frame
        .buffer_mut()
        .set_style(areas[0], Style::default().bg(theme.background));
    frame
        .buffer_mut()
        .set_style(areas[1], Style::default().bg(theme.background));
    frame
        .buffer_mut()
        .set_style(areas[2], Style::default().bg(theme.background));

    let selected = state
        .selected
        .map_or("All widgets".to_owned(), |widget| widget.label().to_owned());
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                "Dashboard V2 Widgets  ",
                Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(
                    "{selected} | {} | {} | {}{}",
                    preset_label(state.preset),
                    state.scenario.label(),
                    format_time(state.elapsed),
                    if state.reduced_motion {
                        " | Reduced Motion"
                    } else {
                        ""
                    }
                ),
                Style::default().fg(theme.text_muted),
            ),
        ]))
        .style(Style::default().bg(theme.background)),
        areas[0],
    );

    if let Some(widget) = state.selected {
        render_widget(frame, widget, state, areas[1], theme);
    } else {
        render_all(frame, state, areas[1], theme);
    }

    frame.render_widget(
        Paragraph::new(
            "1 CPU  2 Memory  3 GPU  4 Network  5 Storage  A all  P preset  S state  G GPU  R reduced motion  Q quit",
        )
        .style(Style::default().fg(theme.text_muted).bg(theme.background)),
        areas[2],
    );
}

fn render_all(frame: &mut ratatui::Frame<'_>, state: ViewerState, area: Rect, theme: &Theme) {
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(33),
            Constraint::Percentage(34),
            Constraint::Percentage(33),
        ])
        .split(area);
    let rows = [rows(columns[0]), rows(columns[1]), rows(columns[2])];
    for (widget, widget_area) in [
        (PrimaryWidget::Cpu, rows[0][0]),
        (PrimaryWidget::Memory, rows[1][0]),
        (PrimaryWidget::Gpu, rows[2][0]),
        (PrimaryWidget::Network, rows[0][1]),
        (PrimaryWidget::Storage, rows[1][1]),
    ] {
        render_widget(frame, widget, state, widget_area, theme);
    }
}

fn rows(area: Rect) -> [Rect; 2] {
    let split = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);
    [split[0], split[1]]
}

fn render_widget(
    frame: &mut ratatui::Frame<'_>,
    widget: PrimaryWidget,
    state: ViewerState,
    area: Rect,
    theme: &Theme,
) {
    let Some(model) = compose(widget, state) else {
        return;
    };
    let area = inset(area, 1, 1);
    let bounds = match DashboardBounds::new(
        usize::from(area.x),
        usize::from(area.y),
        usize::from(area.width),
        usize::from(area.height),
    ) {
        Ok(bounds) => bounds,
        Err(_) => return,
    };
    let _ = TerminalRenderer::new().render(model, state.preset, bounds, theme, frame.buffer_mut());
}

fn inset(area: Rect, horizontal: u16, vertical: u16) -> Rect {
    let horizontal = horizontal.min(area.width / 2);
    let vertical = vertical.min(area.height / 2);
    Rect::new(
        area.x.saturating_add(horizontal),
        area.y.saturating_add(vertical),
        area.width.saturating_sub(horizontal.saturating_mul(2)),
        area.height.saturating_sub(vertical.saturating_mul(2)),
    )
}

fn compose(widget: PrimaryWidget, state: ViewerState) -> Option<WidgetRenderModel> {
    let context = state.context(widget);
    match widget {
        PrimaryWidget::Cpu => widgets::compose_cpu(cpu_model(state.scenario), context),
        PrimaryWidget::Memory => widgets::compose_memory(memory_model(state.scenario), context),
        PrimaryWidget::Gpu => {
            widgets::compose_gpu(gpu_model(state.scenario, state.gpu_available), context)
        }
        PrimaryWidget::Network => widgets::compose_network(network_model(state.scenario), context),
        PrimaryWidget::Storage => widgets::compose_storage(storage_model(state.scenario), context),
    }
    .ok()
    .flatten()
}

fn cpu_model(scenario: Scenario) -> dashboard_v2::CpuMetricModel<HISTORY> {
    let (cpu, memory, _, _, _, _) = scenario.values();
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
        .unwrap(),
    )
    .unwrap_or_else(|_| {
        let _ = memory;
        unreachable!("viewer fixtures are validated")
    })
}

fn memory_model(scenario: Scenario) -> dashboard_v2::MemoryMetricModel<HISTORY> {
    let (_, memory, _, _, _, _) = scenario.values();
    dashboard_v2::MemoryMetricModel::from_input(
        MemoryMetricInput::new(
            non_negative(memory * 100.0),
            non_negative((1.0 - memory) * 100.0),
            non_negative(100.0),
            non_negative((memory * 0.16) * 100.0),
            non_negative(if scenario == Scenario::Critical {
                12.0
            } else {
                0.0
            }),
            normalized(memory),
            history(MetricUnit::Percent, &scaled_history(memory)),
        )
        .unwrap(),
    )
    .unwrap()
}

fn gpu_model(scenario: Scenario, available: bool) -> dashboard_v2::GpuMetricModel<HISTORY> {
    let (_, _, gpu, _, _, _) = scenario.values();
    let absent = || MetricReading::unavailable();
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
        .unwrap(),
    )
    .unwrap()
}

fn network_model(scenario: Scenario) -> NetworkMetricModel<HISTORY> {
    let (_, _, _, network, _, _) = scenario.values();
    NetworkMetricModel::from_input(
        NetworkMetricInput::new(
            non_negative(network * 80.0),
            non_negative(network * 25.0),
            non_negative(2_000.0),
            non_negative(900.0),
            non_negative(100.0),
            history(MetricUnit::BytesPerSecond, &scaled_history(network)),
            history(MetricUnit::BytesPerSecond, &scaled_history(network * 0.55)),
        )
        .unwrap(),
    )
    .unwrap()
}

fn storage_model(scenario: Scenario) -> dashboard_v2::StorageMetricModel<HISTORY> {
    let (_, _, _, _, occupancy, activity) = scenario.values();
    StorageMetricInput::new(
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
    .map(|input| dashboard_v2::StorageMetricModel::from_input(input).unwrap())
    .unwrap()
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
        let _ = series.push(
            TimeSample::new(
                Duration::from_secs(index as u64),
                Some(value),
                MetricAvailability::Available,
                None,
            )
            .unwrap(),
        );
    }
    series
}

fn normalized(value: f64) -> MetricReading {
    MetricReading::normalized(Some(value.clamp(0.0, 1.0)), MetricAvailability::Available).unwrap()
}

fn non_negative(value: f64) -> MetricReading {
    MetricReading::non_negative(Some(value.max(0.0)), MetricAvailability::Available).unwrap()
}

fn format_time(elapsed: Duration) -> String {
    format!("{:.1}s", elapsed.as_secs_f32())
}

fn preset_label(preset: DashboardPreset) -> &'static str {
    match preset {
        DashboardPreset::Compact => "Compact",
        DashboardPreset::Standard => "Standard",
        DashboardPreset::Widescreen => "Widescreen",
        DashboardPreset::LargeWidescreen => "Large Widescreen",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn viewer_starts_with_a_valid_widget_configuration() {
        let state = ViewerState::new();
        assert_eq!(state.selected, Some(PrimaryWidget::Cpu));
        assert_eq!(state.preset, DashboardPreset::Widescreen);
        assert_eq!(state.scenario, Scenario::Normal);
        assert!(compose(PrimaryWidget::Cpu, state).is_some());
        assert!(compose(PrimaryWidget::Storage, state).is_some());
    }

    #[test]
    fn viewer_toggles_selection_preset_state_gpu_and_motion() {
        let mut state = ViewerState::new();
        state.toggle_all();
        assert_eq!(state.selected, None);
        state.next_preset();
        assert_eq!(state.preset, DashboardPreset::LargeWidescreen);
        let before_gpu = state.gpu_available;
        state.gpu_available = !state.gpu_available;
        assert_ne!(state.gpu_available, before_gpu);
        state.scenario = state.scenario.next();
        assert_eq!(state.scenario, Scenario::Active);
        state.reduced_motion = true;
        assert!(state.reduced_motion);
    }
}
