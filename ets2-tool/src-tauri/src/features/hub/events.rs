use serde::Serialize;

use crate::state::AppMode;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ModeChanged {
    pub mode: AppMode,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct CareerStatus {
    pub ets2_running: bool,
    pub ats_running: bool,
    pub telemetry_running: bool,
    pub plugin_installed: bool,
    pub bridge_connected: bool,
    pub active_game: Option<String>,
}
