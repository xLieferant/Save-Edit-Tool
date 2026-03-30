use tauri::command;
use tauri::{AppHandle, Emitter, State};

use crate::features::hub::events::ModeChanged;
use crate::state::{AppMode, HubState};

#[command]
pub fn hub_get_mode(hub: State<'_, HubState>) -> Result<AppMode, String> {
    let mode = *hub
        .mode
        .read()
        .map_err(|_| "HubState mode lock poisoned".to_string())?;
    Ok(mode)
}

#[command]
pub fn hub_set_mode(mode: AppMode, app: AppHandle, hub: State<'_, HubState>) -> Result<AppMode, String> {
    {
        let mut guard = hub
            .mode
            .write()
            .map_err(|_| "HubState mode lock poisoned".to_string())?;
        if *guard == mode {
            return Ok(mode);
        }
        *guard = mode;
    }

    app.emit("hub://mode_changed", ModeChanged { mode })
        .map_err(|e| e.to_string())?;

    Ok(mode)
}
