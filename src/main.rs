mod app;
mod system;
mod ui;

use std::{
    io,
    time::{Duration, Instant},
};

use app::AppState;

use crossterm::{
    cursor::Show,
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};

use ratatui::{Terminal, backend::CrosstermBackend};

use sysinfo::{Networks, System};

use system::info::SystemInfo;

struct TerminalGuard;

impl TerminalGuard {
    fn enter() -> io::Result<Self> {
        enable_raw_mode()?;

        if let Err(error) = execute!(io::stdout(), EnterAlternateScreen) {
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

fn should_quit(event: Event) -> bool {
    matches!(
        event,
        Event::Key(key)
            if key.kind == KeyEventKind::Press
                && (key.code == KeyCode::Char('q')
                    || key.code == KeyCode::Esc
                    || (key.code == KeyCode::Char('c')
                        && key.modifiers.contains(KeyModifiers::CONTROL)))
    )
}

fn main() -> io::Result<()> {
    let _terminal_guard = TerminalGuard::enter()?;

    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let mut sys = System::new_all();
    let mut networks = Networks::new_with_refreshed_list();
    let mut app = AppState::new();

    run_app(&mut terminal, &mut sys, &mut networks, &mut app)
}

fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    sys: &mut System,
    networks: &mut Networks,
    app: &mut AppState,
) -> io::Result<()> {
    let tick_rate = Duration::from_secs(1);
    let mut last_tick = Instant::now() - tick_rate;

    loop {
        if last_tick.elapsed() >= tick_rate {
            let info = SystemInfo::get(sys, networks);

            app.update_cpu_history(info.cpu_usage);

            terminal.draw(|frame| {
                ui::dashboard::draw(frame, &info, app);
            })?;

            last_tick = Instant::now();
        }

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or(Duration::ZERO);

        if event::poll(timeout)? {
            loop {
                if should_quit(event::read()?) {
                    return Ok(());
                }

                if !event::poll(Duration::ZERO)? {
                    break;
                }
            }
        }
    }
}
