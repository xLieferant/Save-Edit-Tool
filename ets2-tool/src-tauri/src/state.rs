use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

#[derive(Default)]
pub struct DecryptCache {
    pub files: Mutex<HashMap<PathBuf, String>>,
}

#[derive(Default)]
pub struct AppProfileState {
    pub current_profile: Mutex<Option<String>>,
}

