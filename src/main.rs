mod action;
mod animation;
mod app;
mod command_palette;
mod focus;
mod history;
mod system;
mod theme;
mod timeline;
mod ui;

use std::{
    io,
    time::{Duration, Instant},
};

use action::Action;
use app::AppState;

use crossterm::{
    cursor::Show,
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};

use ratatui::{Terminal, backend::CrosstermBackend};

use system::info::SystemInfoCollector;

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

fn action_for_event(event: Event, palette_open: bool, focus_active: bool) -> Option<Action> {
    let Event::Key(key) = event else {
        return None;
    };

    if key.kind != KeyEventKind::Press {
        return None;
    }

    if palette_open {
        return match key.code {
            KeyCode::Esc => Some(Action::ClosePalette),
            KeyCode::Up => Some(Action::PaletteUp),
            KeyCode::Down => Some(Action::PaletteDown),
            KeyCode::Backspace => Some(Action::PaletteBackspace),
            KeyCode::Enter => Some(Action::PaletteSubmit),
            KeyCode::Char(character) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                Some(Action::PaletteCharacter(character))
            }
            _ => None,
        };
    }

    match key.code {
        KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Some(Action::OpenPalette)
        }
        KeyCode::Tab => Some(Action::FocusNext),
        KeyCode::BackTab => Some(Action::FocusPrevious),
        KeyCode::Esc if focus_active => Some(Action::ExitFocus),
        KeyCode::Esc | KeyCode::Char('q') => Some(Action::Quit),
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => Some(Action::Quit),
        _ => None,
    }
}

fn main() -> io::Result<()> {
    let _terminal_guard = TerminalGuard::enter()?;

    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let mut collector = SystemInfoCollector::new();
    let mut app = AppState::new();

    run_app(&mut terminal, &mut collector, &mut app)
}

fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    collector: &mut SystemInfoCollector,
    app: &mut AppState,
) -> io::Result<()> {
    let collection_interval = Duration::from_secs(1);
    let render_interval = Duration::from_millis(33);
    let mut last_collection = Instant::now() - collection_interval;
    let mut last_render = Instant::now() - render_interval;
    let mut last_animation_update = Instant::now();
    let mut render_pending = true;

    loop {
        let now = Instant::now();
        let mut collected = false;

        if last_collection.elapsed() >= collection_interval {
            let info = collector.collect(now);
            app.update(info);
            last_collection = now;
            collected = true;
            render_pending = true;
        }

        let animation_delta = now.saturating_duration_since(last_animation_update);
        last_animation_update = now;
        if app.update_animations(animation_delta) {
            render_pending = true;
        }

        let render_due = last_render.elapsed() >= render_interval;
        if render_pending && (collected || render_due || !app.animations_active()) {
            terminal.draw(|frame| {
                ui::dashboard::draw_app(frame, app);
            })?;
            last_render = now;
            render_pending = false;
        }

        let until_collection = collection_interval
            .checked_sub(last_collection.elapsed())
            .unwrap_or(Duration::ZERO);
        let until_render = render_interval
            .checked_sub(last_render.elapsed())
            .unwrap_or(Duration::ZERO);
        let timeout = if app.animations_active() || render_pending {
            until_collection.min(until_render)
        } else {
            until_collection
        };

        if event::poll(timeout)? {
            loop {
                if let Some(action) =
                    action_for_event(event::read()?, app.palette().is_open(), app.focus_active())
                {
                    if app.dispatch(action) {
                        return Ok(());
                    }
                    render_pending = true;
                }

                if !event::poll(Duration::ZERO)? {
                    break;
                }
            }
        }
    }
}
