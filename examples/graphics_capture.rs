//! Offline visual fixture. All telemetry in this example is synthetic.
//! Usage: cargo run --release --example graphics_capture -- OUTPUT_DIR [--bench] [--animate] [--status=normal|warning|critical] [--all-focus]
//! It drives the real app, focus/expansion controls and ratatui TestBackend.
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    Terminal,
    backend::TestBackend,
    style::{Color, Modifier},
};
use std::{
    fmt::Write,
    fs,
    time::{Duration, Instant},
};
use sysmon::ui::dashboard::DashboardApp;
#[path = "support/telemetry.rs"]
mod telemetry;
use telemetry::snapshot;
fn key(app: &mut DashboardApp, code: KeyCode, modifiers: KeyModifiers) {
    app.handle_event(Event::Key(KeyEvent::new(code, modifiers)));
}
fn color(c: Color) -> String {
    match c {
        Color::Rgb(r, g, b) => format!("{r},{g},{b}"),
        _ => "217,226,232".into(),
    }
}
fn main() {
    let folder = std::env::args()
        .nth(1)
        .expect("provide a capture directory");
    fs::create_dir_all(&folder).unwrap();
    let benchmark = std::env::args().any(|arg| arg == "--bench");
    let animate = std::env::args().any(|arg| arg == "--animate");
    let status = std::env::args()
        .find_map(|arg| arg.strip_prefix("--status=").map(str::to_owned))
        .unwrap_or_else(|| "normal".to_owned());
    assert!(
        matches!(status.as_str(), "normal" | "warning" | "critical"),
        "unknown fixture status"
    );
    let mut cases = vec![
        (80, 24, 0, 0),
        (100, 30, 0, 0),
        (120, 35, 0, 0),
        (160, 45, 0, 0),
        (200, 55, 0, 0),
        (120, 35, 1, 0),
        (120, 35, 2, 0),
        (160, 45, 0, 3),
        (160, 45, 0, 4),
        (160, 45, 0, 5),
        (160, 45, 0, 10),
        (160, 45, 0, 7),
    ];
    if std::env::args().any(|arg| arg == "--all-focus") {
        for (w, h) in [(80, 24), (100, 30), (120, 35), (160, 45), (200, 55)] {
            for tabs in 1..=10 {
                cases.push((w, h, 0, tabs));
            }
        }
        cases.sort_unstable();
        cases.dedup();
    }
    println!("case,width,height,frames,ns_per_frame");
    for (w, h, theme, tabs) in cases {
        let mut app = DashboardApp::new();
        let start = Instant::now() - Duration::from_secs(240);
        for i in 0..120 {
            app.update(
                snapshot(i, &status),
                start + Duration::from_secs(2 * i as u64),
            );
            app.update_animations(Duration::from_secs(2));
        }
        for _ in 0..theme {
            key(&mut app, KeyCode::Char('p'), KeyModifiers::CONTROL);
            for ch in "theme".chars() {
                key(&mut app, KeyCode::Char(ch), KeyModifiers::NONE);
            }
            key(&mut app, KeyCode::Enter, KeyModifiers::NONE);
        }
        if tabs > 0 {
            for _ in 0..tabs {
                key(&mut app, KeyCode::Tab, KeyModifiers::NONE);
            }
            key(&mut app, KeyCode::Char('e'), KeyModifiers::NONE);
        }
        let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
        terminal.draw(|frame| app.draw(frame)).unwrap();
        let mut data = format!("{w} {h}\n");
        for cell in &terminal.backend().buffer().content {
            writeln!(
                data,
                "{}|{}|{}|{}",
                cell.symbol()
                    .chars()
                    .map(|ch| format!("{:x}", ch as u32))
                    .collect::<Vec<_>>()
                    .join(","),
                color(cell.fg),
                color(cell.bg),
                u8::from(cell.modifier.contains(Modifier::BOLD))
            )
            .unwrap();
        }
        let name = format!("{w}x{h}-theme{theme}-focus{tabs}");
        fs::write(format!("{folder}/{name}.cells"), data).unwrap();
        let mut plain = String::new();
        for row in terminal.backend().buffer().content.chunks(w as usize) {
            for cell in row {
                plain.push_str(cell.symbol());
            }
            plain.push('\n');
        }
        fs::write(format!("{folder}/{name}.txt"), plain).unwrap();
        if benchmark {
            for _ in 0..10 {
                if animate {
                    app.update_animations(Duration::from_millis(50));
                }
                terminal.draw(|frame| app.draw(frame)).unwrap();
            }
            let begin = Instant::now();
            let frames = 100;
            for _ in 0..frames {
                if animate {
                    app.update_animations(Duration::from_millis(50));
                }
                terminal.draw(|frame| app.draw(frame)).unwrap();
            }
            println!(
                "{name},{w},{h},{frames},{}",
                begin.elapsed().as_nanos() / frames
            );
        }
    }
}
