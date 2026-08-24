mod action;
mod animation;
mod app;
mod command_palette;
mod config;
mod export;
mod focus;
mod history;
mod process_search;
mod pulse;
mod system;
mod theme;
mod timeline;
mod ui;
mod widget;

use std::{
    io,
    sync::{
        Arc, Mutex,
        mpsc::{self, Receiver, Sender},
    },
    thread::{self, JoinHandle},
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

use system::info::{SystemInfo, SystemInfoCollector};

const COLLECTOR_SHUTDOWN_TIMEOUT: Duration = Duration::from_millis(750);
const COLLECTOR_SHUTDOWN_POLL_INTERVAL: Duration = Duration::from_millis(5);
const RENDER_INTERVAL: Duration = Duration::from_millis(50);
const SNAPSHOT_POLL_INTERVAL: Duration = Duration::from_millis(50);

struct TerminalGuard;

enum CollectorCommand {
    SetInterval(Duration),
    Stop,
}

struct SnapshotSlot {
    latest: Arc<Mutex<Option<SystemInfo>>>,
}

impl SnapshotSlot {
    fn take(&self) -> Option<SystemInfo> {
        self.latest.lock().ok()?.take()
    }
}

struct CollectorWorker {
    commands: Sender<CollectorCommand>,
    handle: Option<JoinHandle<()>>,
}

impl CollectorWorker {
    fn set_interval(&self, interval: Duration) {
        let _ = self.commands.send(CollectorCommand::SetInterval(interval));
    }
}

impl Drop for CollectorWorker {
    fn drop(&mut self) {
        let _ = self.commands.send(CollectorCommand::Stop);
        if let Some(handle) = self.handle.take() {
            let deadline = Instant::now() + COLLECTOR_SHUTDOWN_TIMEOUT;
            while !handle.is_finished() && Instant::now() < deadline {
                thread::sleep(COLLECTOR_SHUTDOWN_POLL_INTERVAL);
            }

            if handle.is_finished() {
                let _ = handle.join();
            }
        }
    }
}

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

fn action_for_event(
    event: Event,
    palette_open: bool,
    process_search_open: bool,
    config_open: bool,
    focus_active: bool,
) -> Option<Action> {
    let Event::Key(key) = event else {
        return None;
    };

    if key.kind != KeyEventKind::Press {
        return None;
    }

    if palette_open {
        return match key.code {
            KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                Some(Action::ClosePalette)
            }
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

    if process_search_open {
        return match key.code {
            KeyCode::Esc => Some(Action::CloseProcessSearch),
            KeyCode::Up => Some(Action::ProcessSearchUp),
            KeyCode::Down => Some(Action::ProcessSearchDown),
            KeyCode::Backspace => Some(Action::ProcessSearchBackspace),
            KeyCode::Enter => Some(Action::ProcessSearchSubmit),
            KeyCode::Char(character) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                Some(Action::ProcessSearchCharacter(character))
            }
            _ => None,
        };
    }

    if config_open {
        return match key.code {
            KeyCode::Esc => Some(Action::CloseConfig),
            KeyCode::Up => Some(Action::ConfigUp),
            KeyCode::Down => Some(Action::ConfigDown),
            KeyCode::Left => Some(Action::ConfigPrevious),
            KeyCode::Right => Some(Action::ConfigNext),
            KeyCode::Enter => Some(Action::ConfigToggle),
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

    let collector = SystemInfoCollector::new();
    let mut app = AppState::new();

    run_app(&mut terminal, collector, &mut app)
}

fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    collector: SystemInfoCollector,
    app: &mut AppState,
) -> io::Result<()> {
    let (snapshots, collector_worker) = spawn_collector(collector, app.refresh_interval());
    let mut last_render = Instant::now() - RENDER_INTERVAL;
    let mut last_animation_update = Instant::now();
    let mut render_pending = true;
    let mut requested_collection_interval = app.refresh_interval();

    loop {
        let now = Instant::now();
        let mut received_snapshot = false;
        if let Some(info) = snapshots.take() {
            app.update(info);
            received_snapshot = true;
        }
        if received_snapshot {
            render_pending = true;
        }

        if app.refresh_interval() != requested_collection_interval {
            requested_collection_interval = app.refresh_interval();
            collector_worker.set_interval(requested_collection_interval);
        }

        let animation_delta = now.saturating_duration_since(last_animation_update);
        last_animation_update = now;
        if app.update_animations(animation_delta) {
            render_pending = true;
        }

        let render_due = last_render.elapsed() >= RENDER_INTERVAL;
        if render_pending && (received_snapshot || render_due || !app.animations_active()) {
            terminal.draw(|frame| {
                ui::dashboard::draw_app(frame, app);
            })?;
            last_render = now;
            render_pending = false;
        }

        let until_render = RENDER_INTERVAL
            .checked_sub(last_render.elapsed())
            .unwrap_or(Duration::ZERO);
        let until_snapshot_poll = SNAPSHOT_POLL_INTERVAL;
        let timeout = if app.animations_active() || render_pending {
            until_snapshot_poll.min(until_render)
        } else {
            until_snapshot_poll
        };

        if event::poll(timeout)? {
            loop {
                if let Some(action) = action_for_event(
                    event::read()?,
                    app.palette().is_open(),
                    app.process_search().is_open(),
                    app.config().is_open(),
                    app.focus_active(),
                ) {
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

fn spawn_collector(
    mut collector: SystemInfoCollector,
    interval: Duration,
) -> (SnapshotSlot, CollectorWorker) {
    let latest_snapshot = Arc::new(Mutex::new(None));
    let collector_snapshot = Arc::clone(&latest_snapshot);
    let (command_sender, command_receiver) = mpsc::channel();
    let handle = thread::spawn(move || {
        collection_loop(
            &mut collector,
            collector_snapshot,
            command_receiver,
            interval,
        );
    });

    (
        SnapshotSlot {
            latest: latest_snapshot,
        },
        CollectorWorker {
            commands: command_sender,
            handle: Some(handle),
        },
    )
}

fn collection_loop(
    collector: &mut SystemInfoCollector,
    latest_snapshot: Arc<Mutex<Option<SystemInfo>>>,
    commands: Receiver<CollectorCommand>,
    initial_interval: Duration,
) {
    let mut interval = initial_interval;
    let mut next_collection = Instant::now();

    loop {
        let wait = next_collection.saturating_duration_since(Instant::now());
        match commands.recv_timeout(wait) {
            Ok(CollectorCommand::SetInterval(new_interval)) => {
                interval = new_interval;
                next_collection = Instant::now();
            }
            Ok(CollectorCommand::Stop) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                let collected_at = Instant::now();
                let snapshot = collector.collect(collected_at);
                let Ok(mut latest_snapshot) = latest_snapshot.lock() else {
                    break;
                };
                *latest_snapshot = Some(snapshot);
                next_collection = collected_at + interval;
            }
        }
    }
}
