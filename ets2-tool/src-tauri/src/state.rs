use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

#[derive(Default)]
pub struct DecryptCache {
    pub files: Mutex<HashMap<PathBuf, String>>,
}

pub struct AppProfileState {
    pub current_profile: Mutex<Option<String>>,
    pub current_save: Mutex<Option<String>>,
    pub selected_game: Mutex<String>,
}

impl Default for AppProfileState {
    fn default() -> Self {
        Self {
            current_profile: Mutex::new(None),
            current_save: Mutex::new(None),
            selected_game: Mutex::new("ets2".to_string()),
        }
    }
}
