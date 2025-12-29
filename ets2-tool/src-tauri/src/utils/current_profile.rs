use std::sync::RwLock;

static CURRENT_PROFILE: RwLock<Option<String>> = RwLock::new(None);

pub fn set_current_profile(path: String) {
    let mut profile = CURRENT_PROFILE
        .write()
        .expect("CURRENT_PROFILE write lock failed");
    *profile = Some(path);
}

pub fn get_current_profile() -> Option<String> {
    CURRENT_PROFILE
        .read()
        .expect("CURRENT_PROFILE read lock failed")
        .clone()
}

pub fn clear_current_profile() {
    let mut profile = CURRENT_PROFILE
        .write()
        .expect("CURRENT_PROFILE write lock failed");
    *profile = None;
}

pub fn require_current_profile() -> Result<String, String> {
    get_current_profile().ok_or("Kein Profil geladen.".to_string())
}