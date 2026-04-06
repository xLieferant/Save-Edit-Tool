use crate::shared::models::profile::ActiveSaveSelection;
use crate::shared::models::save_context::SaveContext;
use crate::state::AppProfileState;
use tauri::State;

pub fn set_current_profile(state: State<'_, AppProfileState>, path: String) {
    *state.current_profile.lock().unwrap() = Some(path);
}

pub fn clear_current_profile(state: State<'_, AppProfileState>) {
    *state.current_profile.lock().unwrap() = None;
}

pub fn get_current_profile(state: State<'_, AppProfileState>) -> Option<String> {
    state.current_profile.lock().unwrap().clone()
}

pub fn require_current_profile(state: State<'_, AppProfileState>) -> Result<String, String> {
    get_current_profile(state).ok_or_else(|| "Kein Profil geladen.".to_string())
}

pub fn require_current_save(state: State<'_, AppProfileState>) -> Result<String, String> {
    state
        .current_save
        .lock()
        .unwrap()
        .clone()
        .ok_or_else(|| "Kein Save geladen.".to_string())
}

pub fn snapshot_active_save_selection(
    state: &AppProfileState,
) -> Result<ActiveSaveSelection, String> {
    let profile_path = state
        .current_profile
        .lock()
        .map_err(|_| "AppProfileState current_profile lock poisoned".to_string())?
        .clone();
    let save_path = state
        .current_save
        .lock()
        .map_err(|_| "AppProfileState current_save lock poisoned".to_string())?
        .clone();

    Ok(ActiveSaveSelection {
        profile_path,
        save_path,
    })
}

pub fn snapshot_save_context(state: &AppProfileState) -> Result<SaveContext, String> {
    let selection = snapshot_active_save_selection(state)?;
    Ok(SaveContext::from_paths(
        selection.profile_path,
        selection.save_path,
    ))
}
