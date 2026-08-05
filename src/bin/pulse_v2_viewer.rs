#[path = "../ui/widgets/pulse_v2/mod.rs"]
mod pulse_v2;

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
    fmt::Write,
    io::{self, Write as IoWrite},
    time::{Duration, Instant},
};

use crossterm::{
    cursor::{Hide, MoveTo, Show},
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{
        Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode,
        enable_raw_mode, size,
    },
};

use pulse_v2::{
    FieldDimensions, NormalizedPulseInput, PulseV2Engine, PulseV2Preset, PulseV2Rasterizer,
};

const FRAME_INTERVAL: Duration = Duration::from_millis(33);
const PRESETS: [PulseV2Preset; 4] = [
    PulseV2Preset::Compact,
    PulseV2Preset::Standard,
    PulseV2Preset::Widescreen,
    PulseV2Preset::LargeWidescreen,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Scenario {
    Calm,
    Busy,
    Critical,
}

impl Scenario {
    fn label(self) -> &'static str {
        match self {
            Self::Calm => "Calm",
            Self::Busy => "Busy",
            Self::Critical => "Critical",
        }
    }

    fn input(self) -> NormalizedPulseInput {
        let values = match self {
            Self::Calm => (0.12, 0.16, None, 0.02, 0.10, 0.01),
            Self::Busy => (0.40, 0.45, Some(0.30), 0.32, 0.45, 0.18),
            Self::Critical => (0.58, 0.62, Some(0.55), 0.52, 0.60, 0.35),
        };

        NormalizedPulseInput::try_new(values.0, values.1, values.2, values.3, values.4, values.5)
            .unwrap_or_default()
    }
}

#[derive(Debug, Clone, Copy)]
struct ViewerState {
    scenario: Scenario,
    preset_index: usize,
    reduced_motion: bool,
    paused: bool,
    elapsed: Duration,
}

impl ViewerState {
    fn new() -> Self {
        Self {
            scenario: Scenario::Calm,
            preset_index: 2,
            reduced_motion: false,
            paused: false,
            elapsed: Duration::ZERO,
        }
    }

    fn preset(self) -> PulseV2Preset {
        PRESETS[self.preset_index]
    }

    fn next_preset(&mut self) {
        self.preset_index = (self.preset_index + 1) % PRESETS.len();
    }

    fn set_scenario(&mut self, scenario: Scenario) {
        self.scenario = scenario;
    }

    fn toggle_reduced_motion(&mut self) {
        self.reduced_motion = !self.reduced_motion;
    }

    fn toggle_paused(&mut self) {
        self.paused = !self.paused;
    }
}

struct TerminalGuard;

impl TerminalGuard {
    fn enter() -> io::Result<Self> {
        enable_raw_mode()?;
        if let Err(error) = execute!(
            io::stdout(),
            EnterAlternateScreen,
            Hide,
            Clear(ClearType::All)
        ) {
            let _ = disable_raw_mode();
            return Err(error);
        }
        Ok(Self)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), Show, LeaveAlternateScreen);
    }
}

fn main() -> io::Result<()> {
    let _terminal_guard = TerminalGuard::enter()?;
    run_viewer()
}

fn run_viewer() -> io::Result<()> {
    let engine = PulseV2Engine::new();
    let mut state = ViewerState::new();
    let mut last_tick = Instant::now();
    let mut last_frame_time = Duration::ZERO;
    let mut stdout = io::stdout();

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

        last_frame_time = draw_frame(&mut stdout, &engine, state, last_frame_time)?;
    }
}

fn handle_key(code: KeyCode, state: &mut ViewerState) -> bool {
    match code {
        KeyCode::Char('1') => state.set_scenario(Scenario::Calm),
        KeyCode::Char('2') => state.set_scenario(Scenario::Busy),
        KeyCode::Char('3') => state.set_scenario(Scenario::Critical),
        KeyCode::Char('p') | KeyCode::Char('P') => state.next_preset(),
        KeyCode::Char('r') | KeyCode::Char('R') => state.toggle_reduced_motion(),
        KeyCode::Char(' ') => state.toggle_paused(),
        KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc => return true,
        _ => {}
    }
    false
}

fn draw_frame(
    stdout: &mut io::Stdout,
    engine: &PulseV2Engine,
    state: ViewerState,
    previous_frame_time: Duration,
) -> io::Result<Duration> {
    let started = Instant::now();
    let (terminal_width, terminal_height) = size()?;
    let panel_height = usize::from(terminal_height.saturating_sub(3));
    let field_dimensions = FieldDimensions::new(
        usize::from(terminal_width).saturating_mul(2),
        panel_height.saturating_mul(4),
    )
    .map_err(|error| io::Error::other(format!("invalid viewer dimensions: {error:?}")))?;
    let frame = engine
        .generate_with_preset(
            field_dimensions,
            state.scenario.input(),
            state.preset(),
            state.elapsed,
            state.reduced_motion,
        )
        .map_err(|error| io::Error::other(format!("Pulse V2 generation failed: {error:?}")))?;
    let raster = PulseV2Rasterizer::braille(frame.lighting())
        .map_err(|error| io::Error::other(format!("Pulse V2 rasterization failed: {error:?}")))?;

    let mut output = String::new();
    writeln!(&mut output, "Pulse V2 Debug Viewer")
        .map_err(|error| io::Error::other(error.to_string()))?;
    writeln!(
        &mut output,
        "Preset: {} | State: {} | Time: {:.2}s{}",
        preset_label(state.preset()),
        state.scenario.label(),
        state.elapsed.as_secs_f32(),
        if state.reduced_motion {
            " | Reduced Motion"
        } else {
            ""
        }
    )
    .map_err(|error| io::Error::other(error.to_string()))?;
    if !raster.is_empty() {
        output.push_str(&raster.to_text());
        output.push('\n');
    }

    let frame_time = started.elapsed();
    let fps = if previous_frame_time.is_zero() {
        0.0
    } else {
        1.0 / previous_frame_time.as_secs_f64()
    };
    writeln!(
        &mut output,
        "Preset: {} | FPS: {:.1} | Frame Time: {:.2} ms | Time: {:.2}s",
        preset_label(state.preset()),
        fps,
        frame_time.as_secs_f64() * 1_000.0,
        state.elapsed.as_secs_f32()
    )
    .map_err(|error| io::Error::other(error.to_string()))?;

    execute!(
        stdout,
        MoveTo(0, 0),
        Clear(ClearType::All),
        crossterm::style::Print(output)
    )?;
    stdout.flush()?;
    Ok(frame_time)
}

fn preset_label(preset: PulseV2Preset) -> &'static str {
    match preset {
        PulseV2Preset::Compact => "Compact",
        PulseV2Preset::Standard => "Standard",
        PulseV2Preset::Widescreen => "Widescreen",
        PulseV2Preset::LargeWidescreen => "Large Widescreen",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn viewer_state_has_a_valid_initial_configuration() {
        let state = ViewerState::new();

        assert_eq!(state.scenario, Scenario::Calm);
        assert_eq!(state.preset(), PulseV2Preset::Widescreen);
        assert!(!state.reduced_motion);
        assert!(!state.paused);
        assert_eq!(state.elapsed, Duration::ZERO);
    }

    #[test]
    fn viewer_changes_scenarios_and_presets_without_rebuilding_state() {
        let mut state = ViewerState::new();

        state.set_scenario(Scenario::Busy);
        assert_eq!(state.scenario.label(), "Busy");
        assert_ne!(state.scenario.input(), Scenario::Calm.input());

        let initial = state.preset();
        state.next_preset();
        assert_ne!(state.preset(), initial);
        for _ in 0..PRESETS.len() - 1 {
            state.next_preset();
        }
        assert_eq!(state.preset(), initial);
    }

    #[test]
    fn viewer_toggles_reduced_motion_and_pause() {
        let mut state = ViewerState::new();

        state.toggle_reduced_motion();
        assert!(state.reduced_motion);
        state.toggle_paused();
        assert!(state.paused);
        state.toggle_reduced_motion();
        state.toggle_paused();
        assert!(!state.reduced_motion);
        assert!(!state.paused);
    }

    #[test]
    fn viewer_scenarios_and_presets_generate_without_panicking() {
        let engine = PulseV2Engine::new();
        let dimensions = FieldDimensions::new(160, 80).unwrap();

        for scenario in [Scenario::Calm, Scenario::Busy, Scenario::Critical] {
            for preset in PRESETS {
                let frame = engine
                    .generate_with_preset(
                        dimensions,
                        scenario.input(),
                        preset,
                        Duration::from_millis(500),
                        false,
                    )
                    .unwrap();
                assert_eq!(frame.preset(), preset);
            }
        }
    }
}
