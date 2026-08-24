use std::{
    io,
    sync::{
        Arc, Mutex,
        mpsc::{self, Receiver, Sender},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use crossterm::{
    cursor::Show,
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};

use ratatui::{Terminal, backend::CrosstermBackend};

use sysmon::{SystemInfo, SystemInfoCollector, ui::dashboard::DashboardApp};

const COLLECTOR_SHUTDOWN_TIMEOUT: Duration = Duration::from_millis(750);
const COLLECTOR_SHUTDOWN_POLL_INTERVAL: Duration = Duration::from_millis(5);
const RENDER_INTERVAL: Duration = Duration::from_millis(50);
const SNAPSHOT_POLL_INTERVAL: Duration = Duration::from_millis(50);

struct TerminalGuard;

enum CollectorCommand {
    Stop,
}

struct SnapshotSlot {
    latest: Arc<Mutex<Option<(Instant, SystemInfo)>>>,
}

impl SnapshotSlot {
    fn take(&self) -> Option<(Instant, SystemInfo)> {
        self.latest.lock().ok()?.take()
    }
}

struct CollectorWorker {
    commands: Sender<CollectorCommand>,
    handle: Option<JoinHandle<()>>,
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

fn should_quit(event: Event) -> bool {
    let Event::Key(key) = event else {
        return false;
    };

    if key.kind != KeyEventKind::Press {
        return false;
    }

    matches!(key.code, KeyCode::Esc | KeyCode::Char('q'))
        || (key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL))
}

fn main() -> io::Result<()> {
    let _terminal_guard = TerminalGuard::enter()?;

    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let collector = SystemInfoCollector::new();
    let mut app = DashboardApp::new();

    run_app(&mut terminal, collector, &mut app)
}

fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    collector: SystemInfoCollector,
    app: &mut DashboardApp,
) -> io::Result<()> {
    let (snapshots, _collector_worker) = spawn_collector(collector, app.refresh_interval());
    let mut last_render = Instant::now() - RENDER_INTERVAL;
    let mut last_animation_update = Instant::now();
    let mut render_pending = true;

    loop {
        let now = Instant::now();
        let mut received_snapshot = false;
        if let Some((collected_at, info)) = snapshots.take() {
            app.update(info, collected_at);
            received_snapshot = true;
        }
        if received_snapshot {
            render_pending = true;
        }

        let animation_delta = now.saturating_duration_since(last_animation_update);
        last_animation_update = now;
        if app.update_animations(animation_delta) {
            render_pending = true;
        }

        let render_due = last_render.elapsed() >= RENDER_INTERVAL;
        if render_pending && (received_snapshot || render_due || !app.animations_active()) {
            terminal.draw(|frame| app.draw(frame))?;
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
    latest_snapshot: Arc<Mutex<Option<(Instant, SystemInfo)>>>,
    commands: Receiver<CollectorCommand>,
    initial_interval: Duration,
) {
    let interval = initial_interval;
    let mut next_collection = Instant::now();

    loop {
        let wait = next_collection.saturating_duration_since(Instant::now());
        match commands.recv_timeout(wait) {
            Ok(CollectorCommand::Stop) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                let collected_at = Instant::now();
                let snapshot = collector.collect(collected_at);
                let Ok(mut latest_snapshot) = latest_snapshot.lock() else {
                    break;
                };
                *latest_snapshot = Some((collected_at, snapshot));
                next_collection = collected_at + interval;
            }
        }
    }
}
