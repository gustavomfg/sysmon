//! Terminal compatibility is applied after rendering. Telemetry and Pulse's
//! field geometry remain identical; only the terminal's output alphabet and
//! color encoding change. Detection runs once, outside the frame loop.
use ratatui::{buffer::Buffer, style::Color};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorDepth {
    TrueColor,
    Ansi16,
    Ansi256,
    Monochrome,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalCapabilities {
    pub color: ColorDepth,
    pub ascii: bool,
}

impl TerminalCapabilities {
    pub fn detect() -> Self {
        Self::from_options(
            std::env::var("SYSMON_COLOR").ok().as_deref(),
            std::env::var("SYSMON_GRAPHICS").ok().as_deref(),
            std::env::var("TERM").ok().as_deref(),
            std::env::var_os("NO_COLOR").is_some(),
        )
    }
    pub fn from_options(
        color: Option<&str>,
        graphics: Option<&str>,
        term: Option<&str>,
        no_color: bool,
    ) -> Self {
        let console = matches!(term, Some("linux" | "dumb"));
        let color = match color {
            Some("truecolor") => ColorDepth::TrueColor,
            Some("mono") => ColorDepth::Monochrome,
            Some("ansi256") => ColorDepth::Ansi256,
            Some("ansi" | "ansi16") => ColorDepth::Ansi16,
            _ if no_color => ColorDepth::Monochrome,
            _ if console => ColorDepth::Ansi16,
            _ => ColorDepth::TrueColor,
        };
        Self {
            color,
            ascii: match graphics {
                Some("unicode") => false,
                Some("ascii") => true,
                _ => console,
            },
        }
    }
    pub fn apply(self, buffer: &mut Buffer) {
        if self.color == ColorDepth::TrueColor && !self.ascii {
            return;
        }
        for cell in &mut buffer.content {
            match self.color {
                ColorDepth::Ansi16 => {
                    if let Color::Rgb(r, g, b) = cell.fg {
                        cell.set_fg(ansi16(r, g, b));
                    }
                    if let Color::Rgb(r, g, b) = cell.bg {
                        cell.set_bg(ansi16(r, g, b));
                    }
                }
                ColorDepth::Ansi256 => {
                    if let Color::Rgb(r, g, b) = cell.fg {
                        cell.set_fg(Color::Indexed(ansi256(r, g, b)));
                    }
                    if let Color::Rgb(r, g, b) = cell.bg {
                        cell.set_bg(Color::Indexed(ansi256(r, g, b)));
                    }
                }
                ColorDepth::Monochrome => {
                    cell.set_fg(Color::Reset).set_bg(Color::Reset);
                }
                ColorDepth::TrueColor => {}
            }
            if self.ascii && !cell.symbol().is_ascii() {
                if cell
                    .symbol()
                    .chars()
                    .any(|ch| ('\u{2801}'..='\u{28ff}').contains(&ch))
                {
                    cell.modifier.insert(ratatui::style::Modifier::BOLD);
                }
                let text = cell.symbol().chars().map(ascii_glyph).collect::<String>();
                cell.set_symbol(&text);
            }
        }
    }
}

fn ascii_glyph(ch: char) -> char {
    if ('\u{2800}'..='\u{28ff}').contains(&ch) {
        return match (ch as u32 - 0x2800).count_ones() {
            0 => ' ',
            1..=2 => '.',
            3..=5 => '*',
            _ => '#',
        };
    }
    match ch {
        '─' => '-',
        '━' | '╸' => '=',
        '│' | '┃' => '|',
        '╭' | '╮' | '╰' | '╯' | '┌' | '┐' | '└' | '┘' | '├' | '┤' | '┬' | '┴' | '┼' => {
            '+'
        }
        '↑' => '^',
        '↓' => 'v',
        '→' | '›' => '>',
        '↗' => '/',
        '↘' => '\\',
        '·' | '•' | '◇' => '.',
        '◆' | '●' => '*',
        '—' => '-',
        '°' => ' ',
        'Σ' => 'S',
        '▁' | '▂' => '_',
        '▃' | '▄' | '▅' => '=',
        '▆' | '▇' | '█' => '#',
        ch if ch.is_ascii() => ch,
        _ => '?',
    }
}

fn ansi256(r: u8, g: u8, b: u8) -> u8 {
    const LEVELS: [u8; 6] = [0, 95, 135, 175, 215, 255];
    let closest =
        |value: u8| -> usize { (0..6).min_by_key(|&i| value.abs_diff(LEVELS[i])).unwrap() };
    let (ri, gi, bi) = (closest(r), closest(g), closest(b));
    let cube = 16 + 36 * ri + 6 * gi + bi;
    let distance = |red: u8, green: u8, blue: u8| -> i32 {
        (i32::from(r) - i32::from(red)).pow(2)
            + (i32::from(g) - i32::from(green)).pow(2)
            + (i32::from(b) - i32::from(blue)).pow(2)
    };
    let average = (u16::from(r) + u16::from(g) + u16::from(b)) / 3;
    let gray_index = ((i32::from(average) - 8 + 5) / 10).clamp(0, 23) as u8;
    let gray = 8 + 10 * gray_index;
    if distance(gray, gray, gray) < distance(LEVELS[ri], LEVELS[gi], LEVELS[bi]) {
        232 + gray_index
    } else {
        cube as u8
    }
}

fn ansi16(r: u8, g: u8, b: u8) -> Color {
    const COLORS: [(Color, (u8, u8, u8)); 16] = [
        (Color::Black, (0, 0, 0)),
        (Color::Red, (128, 0, 0)),
        (Color::Green, (0, 128, 0)),
        (Color::Yellow, (128, 128, 0)),
        (Color::Blue, (0, 0, 128)),
        (Color::Magenta, (128, 0, 128)),
        (Color::Cyan, (0, 128, 128)),
        (Color::Gray, (192, 192, 192)),
        (Color::DarkGray, (128, 128, 128)),
        (Color::LightRed, (255, 0, 0)),
        (Color::LightGreen, (0, 255, 0)),
        (Color::LightYellow, (255, 255, 0)),
        (Color::LightBlue, (0, 0, 255)),
        (Color::LightMagenta, (255, 0, 255)),
        (Color::LightCyan, (0, 255, 255)),
        (Color::White, (255, 255, 255)),
    ];
    COLORS
        .iter()
        .min_by_key(|(_, (red, green, blue))| {
            (i32::from(r) - i32::from(*red)).pow(2)
                + (i32::from(g) - i32::from(*green)).pow(2)
                + (i32::from(b) - i32::from(*blue)).pow(2)
        })
        .unwrap()
        .0
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{layout::Rect, style::Style};
    #[test]
    fn capability_choices_are_explicit_and_do_not_guess_modern_terminal_limits() {
        assert_eq!(
            TerminalCapabilities::from_options(None, None, Some("xterm-256color"), false),
            TerminalCapabilities {
                color: ColorDepth::TrueColor,
                ascii: false
            }
        );
        assert!(TerminalCapabilities::from_options(None, None, Some("linux"), false).ascii);
        assert_eq!(
            TerminalCapabilities::from_options(None, None, None, true).color,
            ColorDepth::Monochrome
        );
        assert!(
            !TerminalCapabilities::from_options(
                Some("truecolor"),
                Some("unicode"),
                Some("linux"),
                false
            )
            .ascii
        );
    }
    #[test]
    fn fallback_preserves_numbers_units_and_cell_bounds() {
        let area = Rect::new(0, 0, 12, 1);
        let mut buffer = Buffer::empty(area);
        buffer.set_string(
            0,
            0,
            "42% ↓ 2MiB/s",
            Style::default()
                .fg(Color::Rgb(120, 210, 140))
                .bg(Color::Rgb(11, 16, 20)),
        );
        TerminalCapabilities {
            color: ColorDepth::Ansi256,
            ascii: true,
        }
        .apply(&mut buffer);
        let text: String = buffer.content.iter().map(|cell| cell.symbol()).collect();
        assert_eq!(text, "42% v 2MiB/s");
        assert_eq!(*buffer.area(), area);
        assert!(
            buffer
                .content
                .iter()
                .all(|cell| matches!(cell.fg, Color::Indexed(_)))
        );
        assert_eq!(ansi256(0, 0, 0), 16);
        assert_eq!(ansi256(255, 255, 255), 231);
    }
    #[test]
    fn monochrome_keeps_available_and_missing_telemetry_distinct() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 8, 1));
        buffer.set_string(
            0,
            0,
            "0%  —   ",
            Style::default().fg(Color::Rgb(120, 210, 140)),
        );
        TerminalCapabilities {
            color: ColorDepth::Monochrome,
            ascii: true,
        }
        .apply(&mut buffer);
        assert_eq!(
            buffer
                .content
                .iter()
                .map(|cell| cell.symbol())
                .collect::<String>(),
            "0%  -   "
        );
    }
}
