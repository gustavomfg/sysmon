mod system;
mod ui;

use std::io;

use crossterm::{
    execute,
    terminal::{
        disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
    },
};

use ratatui::{backend::CrosstermBackend, Terminal};

use sysinfo::System;

use system::info::SystemInfo;

fn main() -> io::Result<()> {
    enable_raw_mode()?;

    let mut stdout = io::stdout();

    execute!(stdout, EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(stdout);

    let mut terminal = Terminal::new(backend)?;

    let mut sys = System::new_all();

    let info = SystemInfo::get(&mut sys);

    terminal.draw(|frame| {
        ui::dashboard::draw(frame, &info);
    })?;

    std::thread::sleep(std::time::Duration::from_secs(5));

    disable_raw_mode()?;

    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;

    Ok(())
}