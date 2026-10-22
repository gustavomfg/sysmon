mod action;
pub mod animation;
mod app;
mod command_palette;
mod config;
mod export;
mod focus;
mod process_search;
pub mod pulse_v2;
#[allow(clippy::new_without_default)]
pub mod system;
pub mod terminal_capabilities;
pub mod theme;
mod timeline;
pub mod ui;
mod widget;

pub use system::info::{SystemInfo, SystemInfoCollector};
pub use ui::dashboard_v2;
