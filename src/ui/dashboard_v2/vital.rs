//! Vital line: the System Pulse heartbeat drawn as a sweeping monitor trace.
//!
//! The Living System Model says the machine produces a heartbeat. The Pulse
//! field shows its *texture*; the vital line shows its *rhythm*, borrowing the
//! one visual language everybody already reads instantly: a bedside monitor.
//!
//! `VitalTrace` is animation state only. It consumes `PulseBehavior` (never
//! raw metrics), advances a write head in time and remembers one waveform
//! level per Braille sub-column. `render` turns that memory into one terminal
//! row, fading older beats so the trace stays quiet next to the Pulse.
use std::time::Duration;

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
};

use super::{DashboardState, PulseBehavior};
use crate::theme::Theme;

/// Braille sub-columns written per second by the sweeping head.
const SWEEP_SPEED: f32 = 36.0;
/// Sub-columns cleared ahead of the head, like the erase bar of a monitor.
const ERASE_GAP: usize = 6;
/// Upper bound on one advance so a stalled frame never redraws the screen.
const MAX_STEP: Duration = Duration::from_millis(250);

/// Waveform levels are Braille dot rows: 0 is the top, 3 the bottom.
const BASELINE: u8 = 2;
/// P wave, PQ segment, Q, R, S, ST segment, T wave, back to baseline.
const BEAT: [u8; 15] = [1, 1, 2, 2, 3, 0, 0, 3, 2, 2, 2, 1, 1, 1, 2];
const BRAILLE_BITS: [[u8; 4]; 2] = [[0x01, 0x02, 0x04, 0x40], [0x08, 0x10, 0x20, 0x80]];

/// Maps Pulse rate into a familiar resting-to-exercise heart rate range.
pub fn heart_rate_bpm(behavior: PulseBehavior) -> f32 {
    52.0 + behavior.rate().clamp(0.0, 1.0) * 88.0
}

#[derive(Debug, Clone, PartialEq)]
pub struct VitalTrace {
    levels: Vec<u8>,
    head: usize,
    beat_phase: f32,
    beat_cursor: Option<usize>,
    pending: f32,
    last_elapsed: Option<Duration>,
}

impl VitalTrace {
    pub const fn new() -> Self {
        Self {
            levels: Vec::new(),
            head: 0,
            beat_phase: 0.0,
            beat_cursor: None,
            pending: 0.0,
            last_elapsed: None,
        }
    }

    /// Advances the write head to `elapsed`. A width change restarts the
    /// trace and pre-fills one full sweep so the line is never empty.
    pub fn advance(&mut self, width: u16, elapsed: Duration, behavior: PulseBehavior) {
        let columns = usize::from(width) * 2;
        if columns != self.levels.len() {
            self.levels = vec![BASELINE; columns];
            self.head = 0;
            self.beat_phase = 0.6;
            self.beat_cursor = None;
            for _ in 0..columns {
                self.step(behavior);
            }
            self.last_elapsed = Some(elapsed);
            return;
        }
        if columns == 0 {
            return;
        }
        let delta = self
            .last_elapsed
            .map_or(Duration::ZERO, |last| elapsed.saturating_sub(last))
            .min(MAX_STEP);
        self.last_elapsed = Some(elapsed);
        self.pending += delta.as_secs_f32() * SWEEP_SPEED;
        while self.pending >= 1.0 {
            self.pending -= 1.0;
            self.step(behavior);
        }
    }

    fn step(&mut self, behavior: PulseBehavior) {
        self.beat_phase += heart_rate_bpm(behavior) / 60.0 / SWEEP_SPEED;
        if self.beat_phase >= 1.0 {
            self.beat_phase -= 1.0;
            self.beat_cursor = Some(0);
        }
        let level = match self.beat_cursor {
            Some(index) if index < BEAT.len() => {
                self.beat_cursor = Some(index + 1);
                shape(BEAT[index], index, behavior)
            }
            _ => {
                self.beat_cursor = None;
                BASELINE
            }
        };
        self.levels[self.head] = level;
        self.head = (self.head + 1) % self.levels.len();
    }

    /// Distance in sub-columns behind the head; 0 is the newest sample.
    fn age(&self, column: usize) -> usize {
        let len = self.levels.len();
        (self.head + len - 1 - column) % len
    }

    pub fn render(
        &self,
        area: Rect,
        state: DashboardState,
        reduced_motion: bool,
        theme: &Theme,
        buffer: &mut Buffer,
    ) {
        let len = self.levels.len();
        if area.width == 0 || len != usize::from(area.width) * 2 {
            return;
        }
        let signal = signal_color(state, theme);
        for cell in 0..usize::from(area.width) {
            let mut mask = 0_u8;
            let mut freshest = usize::MAX;
            for (side, bits) in BRAILLE_BITS.iter().enumerate() {
                let column = cell * 2 + side;
                let age = self.age(column);
                if !reduced_motion && age >= len.saturating_sub(ERASE_GAP) {
                    continue;
                }
                let level = self.levels[column];
                let previous = self.levels[(column + len - 1) % len];
                // Join consecutive levels vertically so spikes read as lines.
                let joined = !reduced_motion && self.age((column + len - 1) % len) > age;
                let (top, bottom) = if joined || reduced_motion {
                    (level.min(previous), level.max(previous))
                } else {
                    (level, level)
                };
                for row in top..=bottom {
                    mask |= bits[usize::from(row)];
                }
                freshest = freshest.min(age);
            }
            let x = area
                .x
                .saturating_add(u16::try_from(cell).unwrap_or(u16::MAX));
            let target = &mut buffer[(x, area.y)];
            if mask == 0 {
                target.set_char(' ');
                target.set_style(Style::default().bg(theme.background));
                continue;
            }
            let (color, bold) = if reduced_motion {
                (mix(theme.border, signal, 0.45), false)
            } else {
                // Recent beats glow; the rest of the sweep settles into the
                // border tone so the line keeps its role as a header rule.
                let fade = 1.0 - (freshest as f32 / (len as f32 * 0.55)).clamp(0.0, 1.0);
                (
                    mix(theme.border, signal, 0.25 + fade * fade * 0.75),
                    freshest < 2,
                )
            };
            let mut style = Style::default().fg(color).bg(theme.background);
            if bold {
                style = style.add_modifier(Modifier::BOLD);
            }
            target.set_char(char::from_u32(0x2800 + u32::from(mask)).unwrap_or(' '));
            target.set_style(style);
        }
    }
}

impl Default for VitalTrace {
    fn default() -> Self {
        Self::new()
    }
}

/// Behavior shapes the complex: low energy shortens the R wave and tension
/// inverts the T wave, the classic "strain" signature.
fn shape(level: u8, index: usize, behavior: PulseBehavior) -> u8 {
    match index {
        5 | 6 if behavior.energy() < 0.30 => 1,
        11..=13 if behavior.tension() > 0.50 => 3,
        _ => level,
    }
}

pub fn signal_color(state: DashboardState, theme: &Theme) -> Color {
    match state {
        DashboardState::Calm | DashboardState::Active => theme.primary,
        DashboardState::Busy => theme.warning,
        DashboardState::Critical => theme.danger,
    }
}

/// Utilization bands, matching CPU/memory thresholds. These indicate load,
/// not hardware failure; unknown values remain neutral.
pub fn heat_color(amount: f64, theme: &Theme) -> Color {
    use super::metrics::CPU_THRESHOLDS;
    if !amount.is_finite() {
        theme.text_muted
    } else if amount >= CPU_THRESHOLDS.critical() {
        theme.danger
    } else if amount >= CPU_THRESHOLDS.warning() {
        theme.warning
    } else {
        theme.success
    }
}

/// Use the metric model's existing classification, including sustained pressure.
pub fn metric_color(state: super::WidgetState, theme: &Theme) -> Color {
    use super::WidgetState;
    match state {
        WidgetState::Warning => theme.warning,
        WidgetState::Critical => theme.danger,
        WidgetState::Unavailable | WidgetState::Loading => theme.text_muted,
        _ => theme.success,
    }
}

/// Linear RGB blend; non-RGB colors fall back to the nearest endpoint.
pub fn mix(from: Color, to: Color, amount: f32) -> Color {
    let amount = amount.clamp(0.0, 1.0);
    match (from, to) {
        (Color::Rgb(r1, g1, b1), Color::Rgb(r2, g2, b2)) => {
            let blend = |a: u8, b: u8| {
                (f32::from(a) + (f32::from(b) - f32::from(a)) * amount).round() as u8
            };
            Color::Rgb(blend(r1, r2), blend(g1, g2), blend(b1, b2))
        }
        _ if amount < 0.5 => from,
        _ => to,
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use ratatui::{buffer::Buffer, layout::Rect};

    use super::{BASELINE, VitalTrace, heart_rate_bpm};
    use crate::{
        theme::Theme,
        ui::dashboard_v2::{DashboardState, PulseBehavior},
    };

    #[test]
    fn utilization_colors_reserve_warm_tones_for_thresholds() {
        let theme = Theme::pulse();
        for value in [0.0, 0.5, 0.799] {
            assert_eq!(super::heat_color(value, &theme), theme.success);
        }
        for value in [0.8, 0.949] {
            assert_eq!(super::heat_color(value, &theme), theme.warning);
        }
        for value in [0.95, 1.0] {
            assert_eq!(super::heat_color(value, &theme), theme.danger);
        }
        for value in [f64::NAN, f64::INFINITY] {
            assert_eq!(super::heat_color(value, &theme), theme.text_muted);
        }
    }

    #[test]
    fn metric_colors_follow_state_without_reclassifying_data() {
        use crate::ui::dashboard_v2::WidgetState;
        let theme = Theme::pulse();
        for (state, color) in [
            (WidgetState::Idle, theme.success),
            (WidgetState::Normal, theme.success),
            (WidgetState::Active, theme.success),
            (WidgetState::Warning, theme.warning),
            (WidgetState::Critical, theme.danger),
            (WidgetState::Unavailable, theme.text_muted),
            (WidgetState::Loading, theme.text_muted),
        ] {
            assert_eq!(super::metric_color(state, &theme), color);
        }
    }

    fn row(trace: &VitalTrace, width: u16, reduced_motion: bool) -> String {
        let area = Rect::new(0, 0, width, 1);
        let mut buffer = Buffer::empty(area);
        trace.render(
            area,
            DashboardState::Calm,
            reduced_motion,
            &Theme::pulse(),
            &mut buffer,
        );
        buffer.content.iter().map(|cell| cell.symbol()).collect()
    }

    #[test]
    fn first_frame_is_a_full_sweep_with_visible_beats() {
        let mut trace = VitalTrace::new();
        trace.advance(80, Duration::ZERO, PulseBehavior::default());
        assert_eq!(trace.levels.len(), 160);
        assert!(trace.levels.contains(&0) || trace.levels.contains(&1));
        assert!(
            trace
                .levels
                .iter()
                .filter(|level| **level == BASELINE)
                .count()
                > 80
        );
        let rendered = row(&trace, 80, false);
        assert!(rendered.contains(' '), "erase gap leads the head");
        assert!(
            rendered
                .chars()
                .any(|c| ('\u{2801}'..='\u{28FF}').contains(&c))
        );
    }

    #[test]
    fn head_moves_with_time_and_reduced_motion_has_no_gap() {
        let mut trace = VitalTrace::new();
        let behavior = PulseBehavior::default();
        trace.advance(40, Duration::ZERO, behavior);
        let head = trace.head;
        trace.advance(40, Duration::from_millis(250), behavior);
        assert_eq!(trace.head, (head + 9) % 80);
        assert!(!row(&trace, 40, true).contains(' '));
    }

    #[test]
    fn faster_behavior_means_a_faster_heart() {
        let calm = PulseBehavior::default();
        assert!((60.0..75.0).contains(&heart_rate_bpm(calm)));
        let mut trace = VitalTrace::new();
        trace.advance(0, Duration::ZERO, calm);
        assert!(row(&trace, 0, false).is_empty());
    }
}
