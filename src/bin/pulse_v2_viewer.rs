#[path = "../ui/widgets/pulse_v2/mod.rs"]
mod pulse_v2;

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

use std::io::{self, Write as IoWrite};
use std::time::{Duration, Instant};

use crossterm::{
    cursor::{Hide, MoveTo, Show},
    event::{self, Event, KeyCode, KeyEventKind},
    execute, queue,
    style::{Color as TerminalColor, Print, ResetColor, SetForegroundColor},
    terminal::{
        Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode,
        enable_raw_mode, size,
    },
};
use ratatui::style::Color as RatatuiColor;
use theme::Theme;

use pulse_v2::{
    ColorRasterFrame, FieldDimensions, NormalizedPulseInput, PulseV2Engine, PulseV2InputTransition,
    PulseV2Preset, PulseV2Rasterizer, SemanticColorField,
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
    Active,
    Busy,
    Critical,
}

impl Scenario {
    fn label(self) -> &'static str {
        match self {
            Self::Calm => "Calm",
            Self::Active => "Active",
            Self::Busy => "Busy",
            Self::Critical => "Critical",
        }
    }

    fn input(self) -> NormalizedPulseInput {
        let values = match self {
            Self::Calm => (0.12, 0.16, None, 0.02, 0.10, 0.01),
            Self::Active => (0.28, 0.32, Some(0.16), 0.16, 0.28, 0.08),
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
    transition_from: NormalizedPulseInput,
    transition_started: Duration,
}

const TRANSITION_DURATION: Duration = Duration::from_millis(320);

impl ViewerState {
    fn new() -> Self {
        Self {
            scenario: Scenario::Calm,
            preset_index: 2,
            reduced_motion: false,
            paused: false,
            elapsed: Duration::ZERO,
            transition_from: Scenario::Calm.input(),
            transition_started: Duration::ZERO,
        }
    }

    fn preset(self) -> PulseV2Preset {
        PRESETS[self.preset_index]
    }

    fn next_preset(&mut self) {
        self.preset_index = (self.preset_index + 1) % PRESETS.len();
    }

    fn set_scenario(&mut self, scenario: Scenario) {
        let current = self.input();
        self.scenario = scenario;
        self.transition_from = current;
        self.transition_started = self.elapsed;
    }

    fn toggle_reduced_motion(&mut self) {
        self.reduced_motion = !self.reduced_motion;
    }

    fn toggle_paused(&mut self) {
        self.paused = !self.paused;
    }

    fn transition_progress(self) -> f32 {
        if self.reduced_motion {
            return 1.0;
        }
        self.elapsed
            .saturating_sub(self.transition_started)
            .as_secs_f32()
            / TRANSITION_DURATION.as_secs_f32()
    }

    fn input(self) -> NormalizedPulseInput {
        self.transition_from
            .interpolate_toward(self.scenario.input(), self.transition_progress())
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
    let theme = Theme::pulse();

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

        last_frame_time = draw_frame(&mut stdout, &engine, &theme, state, last_frame_time)?;
    }
}

fn handle_key(code: KeyCode, state: &mut ViewerState) -> bool {
    match code {
        KeyCode::Char('1') => state.set_scenario(Scenario::Calm),
        KeyCode::Char('2') => state.set_scenario(Scenario::Active),
        KeyCode::Char('3') => state.set_scenario(Scenario::Busy),
        KeyCode::Char('4') => state.set_scenario(Scenario::Critical),
        KeyCode::Char('s') | KeyCode::Char('S') => {
            let next = match state.scenario {
                Scenario::Calm => Scenario::Active,
                Scenario::Active => Scenario::Busy,
                Scenario::Busy => Scenario::Critical,
                Scenario::Critical => Scenario::Calm,
            };
            state.set_scenario(next);
        }
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
    theme: &Theme,
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
        .generate_with_preset_transition(
            field_dimensions,
            PulseV2InputTransition::new(
                state.transition_from,
                state.scenario.input(),
                state.transition_progress(),
            ),
            state.preset(),
            state.elapsed,
            state.reduced_motion,
        )
        .map_err(|error| io::Error::other(format!("Pulse V2 generation failed: {error:?}")))?;
    let semantic_colors = SemanticColorField::from_lighting(frame.lighting(), frame.input(), theme)
        .map_err(|error| io::Error::other(format!("Pulse V2 color mapping failed: {error:?}")))?;
    let raster = PulseV2Rasterizer::braille_colored(frame.lighting(), &semantic_colors)
        .map_err(|error| io::Error::other(format!("Pulse V2 rasterization failed: {error:?}")))?;

    let header = format!(
        "Pulse V2 Debug Viewer\r\nPreset: {} | State: {} | Time: {:.2}s{}\r\n",
        preset_label(state.preset()),
        state.scenario.label(),
        state.elapsed.as_secs_f32(),
        if state.reduced_motion {
            " | Reduced Motion"
        } else {
            ""
        }
    );

    let frame_time = started.elapsed();
    let fps = if previous_frame_time.is_zero() {
        0.0
    } else {
        1.0 / previous_frame_time.as_secs_f64()
    };
    let footer = format!(
        "\r\nPreset: {} | FPS: {:.1} | Frame Time: {:.2} ms | Time: {:.2}s\r\n",
        preset_label(state.preset()),
        fps,
        frame_time.as_secs_f64() * 1_000.0,
        state.elapsed.as_secs_f32()
    );

    let mut output = Vec::with_capacity(raster.width().saturating_mul(raster.height()) * 4);
    queue!(
        output,
        MoveTo(0, 0),
        Clear(ClearType::All),
        ResetColor,
        Print(header)
    )?;
    write_colored_raster(&mut output, &raster)?;
    queue!(output, ResetColor, Print(footer))?;
    stdout.write_all(&output)?;
    stdout.flush()?;
    Ok(frame_time)
}

fn write_colored_raster(output: &mut Vec<u8>, raster: &ColorRasterFrame) -> io::Result<()> {
    for y in 0..raster.height() {
        let mut active_color = None;
        for x in 0..raster.width() {
            let cell = raster
                .read(x, y)
                .ok_or_else(|| io::Error::other("colored raster cell out of bounds"))?;
            if cell.character() == ' ' {
                queue!(output, Print(' '))?;
            } else {
                let color = to_terminal_color(cell.foreground());
                if active_color != Some(color) {
                    queue!(output, SetForegroundColor(color))?;
                    active_color = Some(color);
                }
                queue!(output, Print(cell.character()))?;
            }
        }
        queue!(output, ResetColor, Print("\r\n"))?;
    }
    Ok(())
}

fn to_terminal_color(color: RatatuiColor) -> TerminalColor {
    match color {
        RatatuiColor::Reset => TerminalColor::Reset,
        RatatuiColor::Black => TerminalColor::Black,
        RatatuiColor::Red => TerminalColor::DarkRed,
        RatatuiColor::Green => TerminalColor::DarkGreen,
        RatatuiColor::Yellow => TerminalColor::DarkYellow,
        RatatuiColor::Blue => TerminalColor::DarkBlue,
        RatatuiColor::Magenta => TerminalColor::DarkMagenta,
        RatatuiColor::Cyan => TerminalColor::DarkCyan,
        RatatuiColor::Gray => TerminalColor::Grey,
        RatatuiColor::DarkGray => TerminalColor::DarkGrey,
        RatatuiColor::LightRed => TerminalColor::Red,
        RatatuiColor::LightGreen => TerminalColor::Green,
        RatatuiColor::LightYellow => TerminalColor::Yellow,
        RatatuiColor::LightBlue => TerminalColor::Blue,
        RatatuiColor::LightMagenta => TerminalColor::Magenta,
        RatatuiColor::LightCyan => TerminalColor::Cyan,
        RatatuiColor::White => TerminalColor::White,
        RatatuiColor::Rgb(r, g, b) => TerminalColor::Rgb { r, g, b },
        RatatuiColor::Indexed(value) => TerminalColor::AnsiValue(value),
    }
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

        state.set_scenario(Scenario::Active);
        assert_eq!(state.scenario.label(), "Active");
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
    fn scenario_changes_are_eased_from_the_current_visual_input() {
        let mut state = ViewerState::new();
        state.elapsed = Duration::from_millis(160);
        state.set_scenario(Scenario::Critical);

        let start = state.input();
        state.elapsed = state.transition_started + TRANSITION_DURATION / 2;
        let middle = state.input();
        state.elapsed = state.transition_started + TRANSITION_DURATION;
        let end = state.input();

        assert!(middle.cpu() > start.cpu());
        assert!(middle.cpu() < end.cpu());
        assert_eq!(end, Scenario::Critical.input());
    }

    #[test]
    fn viewer_scenarios_and_presets_generate_without_panicking() {
        let engine = PulseV2Engine::new();
        let dimensions = FieldDimensions::new(160, 80).unwrap();

        for scenario in [
            Scenario::Calm,
            Scenario::Active,
            Scenario::Busy,
            Scenario::Critical,
        ] {
            for preset in PRESETS {
                let frame = engine
                    .generate_with_preset_transition(
                        dimensions,
                        PulseV2InputTransition::new(Scenario::Calm.input(), scenario.input(), 1.0),
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
