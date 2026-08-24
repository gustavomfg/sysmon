pub mod animation;
pub mod pulse;
#[path = "ui/widgets/pulse_v2/mod.rs"]
pub mod pulse_v2;
#[allow(clippy::new_without_default)]
pub mod system;
pub mod theme;
pub mod ui;

pub use system::info::{SystemInfo, SystemInfoCollector};
pub use ui::dashboard_v2;
