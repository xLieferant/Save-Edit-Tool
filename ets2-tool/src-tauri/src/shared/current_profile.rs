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
