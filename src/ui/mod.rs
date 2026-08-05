pub mod dashboard;
// Dashboard V2 is an exported experimental API whose consumers arrive in later phases.
#[allow(dead_code)]
pub mod dashboard_v2;
mod layout;
mod renderer;
mod widgets;

pub const MIN_DASHBOARD_WIDTH: u16 = 80;
pub const MIN_DASHBOARD_HEIGHT: u16 = 24;
