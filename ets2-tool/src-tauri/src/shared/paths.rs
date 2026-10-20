use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

const ETS2_FOLDER_NAME: &str = "Euro Truck Simulator 2";
const ATS_FOLDER_NAME: &str = "American Truck Simulator";
const SETTINGS_FOLDER_NAME: &str = "save-edit-tool";
const ETS2_LOCATION_SETTINGS_FILE: &str = "ets2_data_location.json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Ets2PathMode {
    Default,
    Custom,
}

impl Default for Ets2PathMode {
    fn default() -> Self {
        Self::Default
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Ets2PathSettings {
    pub mode: Ets2PathMode,
    pub custom_homedir: Option<String>,
}

impl Default for Ets2PathSettings {
    fn default() -> Self {
        Self {
            mode: Ets2PathMode::Default,
            custom_homedir: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameUserPaths {
    pub game: String,
    pub mode: Ets2PathMode,
    pub custom_homedir: Option<PathBuf>,
    pub homedir_root: PathBuf,
    pub game_data_dir: PathBuf,
    pub profiles_dir: PathBuf,
    pub steam_profiles_dir: PathBuf,
    pub profiles_backup_dir: PathBuf,
    pub local_mod_dir: PathBuf,
    pub config_file: PathBuf,
    pub game_log_file: PathBuf,
    pub game_crash_file: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Ets2DataLocationDto {
    pub mode: Ets2PathMode,
    pub custom_homedir: Option<String>,
    pub homedir_root: Option<String>,
    pub game_data_dir: Option<String>,
    pub profiles_dir: Option<String>,
    pub steam_profiles_dir: Option<String>,
    pub profiles_backup_dir: Option<String>,
    pub local_mod_dir: Option<String>,
    pub config_file: Option<String>,
    pub game_log_file: Option<String>,
    pub game_crash_file: Option<String>,
    pub settings_file: Option<String>,
    pub validation: Ets2DataLocationValidationDto,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Ets2DataLocationValidationDto {
    pub status: String,
    pub message_key: String,
    pub homedir_exists: bool,
    pub game_data_dir_exists: bool,
    pub profiles_dir_exists: bool,
    pub steam_profiles_dir_exists: bool,
    pub local_mod_dir_exists: bool,
    pub profiles_count: usize,
    pub steam_profiles_count: usize,
    pub local_mods_count: usize,
}

pub fn ets2_data_location_config_path() -> Option<PathBuf> {
    dirs::config_dir().map(|dir| {
        dir.join(SETTINGS_FOLDER_NAME)
            .join(ETS2_LOCATION_SETTINGS_FILE)
    })
}

pub fn read_ets2_path_settings() -> Ets2PathSettings {
    let Some(path) = ets2_data_location_config_path() else {
        return Ets2PathSettings::default();
    };
    let Ok(content) = fs::read_to_string(path) else {
        return Ets2PathSettings::default();
    };
    serde_json::from_str(&content).unwrap_or_default()
}

pub fn save_ets2_path_settings(settings: &Ets2PathSettings) -> Result<(), String> {
    let path = ets2_data_location_config_path()
        .ok_or_else(|| "Could not resolve app config directory.".to_string())?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Could not create settings directory: {}", error))?;
    }
    let body = serde_json::to_string_pretty(settings)
        .map_err(|error| format!("Could not serialize ETS2 data location: {}", error))?;
    fs::write(&path, body)
        .map_err(|error| format!("Could not write ETS2 data location settings: {}", error))
}

pub fn normalize_user_path_input(value: &str) -> Option<PathBuf> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }

    let lower = trimmed.to_ascii_lowercase();
    let mut path_text = if lower.starts_with("-homedir") {
        trimmed
            .get("-homedir".len()..)
            .unwrap_or_default()
            .trim()
            .trim_start_matches('=')
            .trim()
    } else {
        trimmed
    };

    if path_text.len() >= 2 {
        let starts_and_ends_with_double = path_text.starts_with('"') && path_text.ends_with('"');
        let starts_and_ends_with_single = path_text.starts_with('\'') && path_text.ends_with('\'');
        if starts_and_ends_with_double || starts_and_ends_with_single {
            path_text = &path_text[1..path_text.len() - 1];
        }
    }

    let normalized = path_text.trim();
    if normalized.is_empty() {
        None
    } else {
        Some(PathBuf::from(normalized))
    }
}

pub fn ets2_default_homedir_root() -> Option<PathBuf> {
    if cfg!(target_os = "linux") {
        dirs::data_local_dir()
    } else {
        dirs::document_dir()
    }
}

pub fn ats_default_homedir_root() -> Option<PathBuf> {
    if cfg!(target_os = "linux") {
        dirs::data_local_dir()
    } else {
        dirs::document_dir()
    }
}

pub fn ets2_base_path() -> Option<PathBuf> {
    resolve_game_user_paths("ets2").map(|paths| paths.game_data_dir)
}

pub fn ats_base_path() -> Option<PathBuf> {
    resolve_game_user_paths("ats").map(|paths| paths.game_data_dir)
}

pub fn get_base_path(game: &str) -> Option<PathBuf> {
    resolve_game_user_paths(game).map(|paths| paths.game_data_dir)
}

pub fn resolve_game_user_paths(game: &str) -> Option<GameUserPaths> {
    let normalized_game = if game.eq_ignore_ascii_case("ats") {
        "ats"
    } else {
        "ets2"
    };

    if normalized_game == "ats" {
        let homedir_root = ats_default_homedir_root()?;
        return Some(build_game_user_paths(
            "ats",
            Ets2PathMode::Default,
            None,
            homedir_root.clone(),
            homedir_root.join(ATS_FOLDER_NAME),
        ));
    }

    let settings = read_ets2_path_settings();
    if settings.mode == Ets2PathMode::Custom {
        if let Some(custom_homedir) = settings
            .custom_homedir
            .as_deref()
            .and_then(normalize_user_path_input)
        {
            let (homedir_root, game_data_dir) = resolve_custom_ets2_location_parts(&custom_homedir);
            return Some(build_game_user_paths(
                "ets2",
                Ets2PathMode::Custom,
                Some(custom_homedir),
                homedir_root,
                game_data_dir,
            ));
        }
    }

    let homedir_root = ets2_default_homedir_root()?;
    Some(build_game_user_paths(
        "ets2",
        Ets2PathMode::Default,
        None,
        homedir_root.clone(),
        homedir_root.join(ETS2_FOLDER_NAME),
    ))
}

fn build_game_user_paths(
    game: &str,
    mode: Ets2PathMode,
    custom_homedir: Option<PathBuf>,
    homedir_root: PathBuf,
    game_data_dir: PathBuf,
) -> GameUserPaths {
    GameUserPaths {
        game: game.to_string(),
        mode,
        custom_homedir,
        profiles_dir: game_data_dir.join("profiles"),
        steam_profiles_dir: game_data_dir.join("steam_profiles"),
        profiles_backup_dir: game_data_dir.join("profiles.backup"),
        local_mod_dir: game_data_dir.join("mod"),
        config_file: game_data_dir.join("config.cfg"),
        game_log_file: game_data_dir.join("game.log.txt"),
        game_crash_file: game_data_dir.join("game.crash.txt"),
        homedir_root,
        game_data_dir,
    }
}

fn resolve_custom_ets2_location_parts(selected_path: &Path) -> (PathBuf, PathBuf) {
    if is_ets2_data_dir_selection(selected_path) {
        let homedir_root = selected_path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| selected_path.to_path_buf());
        return (homedir_root, selected_path.to_path_buf());
    }

    let game_data_dir = selected_path.join(ETS2_FOLDER_NAME);
    (selected_path.to_path_buf(), game_data_dir)
}

fn is_ets2_data_dir_selection(path: &Path) -> bool {
    path.file_name()
        .and_then(|value| value.to_str())
        .map(|value| value.eq_ignore_ascii_case(ETS2_FOLDER_NAME))
        .unwrap_or(false)
        || path.join("profiles").is_dir()
        || path.join("steam_profiles").is_dir()
        || path.join("profiles.backup").is_dir()
        || path.join("config.cfg").is_file()
        || path.join("game.log.txt").is_file()
        || path.join("game.crash.txt").is_file()
}

pub fn profile_search_roots(game: &str) -> Vec<PathBuf> {
    let Some(paths) = resolve_game_user_paths(game) else {
        return Vec::new();
    };

    let mut roots = Vec::new();
    push_unique_path(&mut roots, paths.profiles_dir);
    push_unique_path(&mut roots, paths.steam_profiles_dir);
    push_unique_path(&mut roots, paths.profiles_backup_dir);
    push_unique_path(&mut roots, paths.game_data_dir);
    roots
}

pub fn profiles_directory_path(game: &str) -> Option<PathBuf> {
    resolve_game_user_paths(game).map(|paths| paths.profiles_dir)
}

pub fn steam_profiles_directory_path(game: &str) -> Option<PathBuf> {
    resolve_game_user_paths(game).map(|paths| paths.steam_profiles_dir)
}

pub fn profiles_backup_directory_path(game: &str) -> Option<PathBuf> {
    resolve_game_user_paths(game).map(|paths| paths.profiles_backup_dir)
}

pub fn game_log_path(game: &str) -> Option<PathBuf> {
    resolve_game_user_paths(game).map(|paths| paths.game_log_file)
}

pub fn game_crash_path(game: &str) -> Option<PathBuf> {
    resolve_game_user_paths(game).map(|paths| paths.game_crash_file)
}

pub fn mod_directory_path(game: &str) -> Option<PathBuf> {
    resolve_game_user_paths(game).map(|paths| paths.local_mod_dir)
}

pub fn ets2_data_location_dto() -> Ets2DataLocationDto {
    let settings = read_ets2_path_settings();
    let paths = resolve_game_user_paths("ets2");
    let validation = paths
        .as_ref()
        .map(validate_ets2_data_location)
        .unwrap_or_else(|| Ets2DataLocationValidationDto {
            status: "unresolved".to_string(),
            message_key: "modals.ets2_data_location.validation.unresolved".to_string(),
            homedir_exists: false,
            game_data_dir_exists: false,
            profiles_dir_exists: false,
            steam_profiles_dir_exists: false,
            local_mod_dir_exists: false,
            profiles_count: 0,
            steam_profiles_count: 0,
            local_mods_count: 0,
        });

    Ets2DataLocationDto {
        mode: settings.mode,
        custom_homedir: settings.custom_homedir,
        homedir_root: paths
            .as_ref()
            .map(|value| path_to_string(&value.homedir_root)),
        game_data_dir: paths
            .as_ref()
            .map(|value| path_to_string(&value.game_data_dir)),
        profiles_dir: paths
            .as_ref()
            .map(|value| path_to_string(&value.profiles_dir)),
        steam_profiles_dir: paths
            .as_ref()
            .map(|value| path_to_string(&value.steam_profiles_dir)),
        profiles_backup_dir: paths
            .as_ref()
            .map(|value| path_to_string(&value.profiles_backup_dir)),
        local_mod_dir: paths
            .as_ref()
            .map(|value| path_to_string(&value.local_mod_dir)),
        config_file: paths
            .as_ref()
            .map(|value| path_to_string(&value.config_file)),
        game_log_file: paths
            .as_ref()
            .map(|value| path_to_string(&value.game_log_file)),
        game_crash_file: paths
            .as_ref()
            .map(|value| path_to_string(&value.game_crash_file)),
        settings_file: ets2_data_location_config_path().map(|value| path_to_string(&value)),
        validation,
    }
}

fn validate_ets2_data_location(paths: &GameUserPaths) -> Ets2DataLocationValidationDto {
    let homedir_exists = paths.homedir_root.is_dir();
    let game_data_dir_exists = paths.game_data_dir.is_dir();
    let profiles_dir_exists = paths.profiles_dir.is_dir();
    let steam_profiles_dir_exists = paths.steam_profiles_dir.is_dir();
    let local_mod_dir_exists = paths.local_mod_dir.is_dir();
    let profiles_count =
        count_profile_dirs(&paths.profiles_dir) + count_profile_dirs(&paths.profiles_backup_dir);
    let steam_profiles_count = count_profile_dirs(&paths.steam_profiles_dir);
    let local_mods_count = count_directory_entries(&paths.local_mod_dir);
    let has_marker = profiles_count > 0
        || steam_profiles_count > 0
        || local_mods_count > 0
        || paths.config_file.is_file()
        || paths.game_log_file.is_file()
        || paths.game_crash_file.is_file();

    let (status, message_key) = if !homedir_exists {
        (
            "homedir_missing",
            "modals.ets2_data_location.validation.homedir_missing",
        )
    } else if !game_data_dir_exists {
        (
            "game_data_missing",
            "modals.ets2_data_location.validation.game_data_missing",
        )
    } else if !has_marker {
        (
            "markers_missing",
            "modals.ets2_data_location.validation.markers_missing",
        )
    } else {
        ("ok", "modals.ets2_data_location.validation.ok")
    };

    Ets2DataLocationValidationDto {
        status: status.to_string(),
        message_key: message_key.to_string(),
        homedir_exists,
        game_data_dir_exists,
        profiles_dir_exists,
        steam_profiles_dir_exists,
        local_mod_dir_exists,
        profiles_count,
        steam_profiles_count,
        local_mods_count,
    }
}

fn count_profile_dirs(root: &Path) -> usize {
    let Ok(entries) = fs::read_dir(root) else {
        return 0;
    };
    entries
        .flatten()
        .filter(|entry| {
            let path = entry.path();
            path.is_dir() && path.join("profile.sii").is_file()
        })
        .count()
}

fn count_directory_entries(root: &Path) -> usize {
    let Ok(entries) = fs::read_dir(root) else {
        return 0;
    };
    entries
        .flatten()
        .filter(|entry| {
            let path = entry.path();
            path.is_dir()
                || path
                    .extension()
                    .and_then(|value| value.to_str())
                    .map(|value| matches!(value.to_ascii_lowercase().as_str(), "scs" | "zip"))
                    .unwrap_or(false)
        })
        .count()
}

fn push_unique_path(paths: &mut Vec<PathBuf>, path: PathBuf) {
    let key = path_to_string(&path)
        .replace('\\', "/")
        .to_ascii_lowercase();
    if !paths.iter().any(|existing| {
        path_to_string(existing)
            .replace('\\', "/")
            .to_ascii_lowercase()
            == key
    }) {
        paths.push(path);
    }
}

fn path_to_string(path: &Path) -> String {
    path.display().to_string()
}

pub fn autosave_path(profile_path: &str) -> PathBuf {
    Path::new(profile_path)
        .join("save")
        .join("quicksave")
        .join("info.sii")
}

pub fn quicksave_game_path(profile_path: &str) -> PathBuf {
    Path::new(profile_path)
        .join("save")
        .join("quicksave")
        .join("game.sii")
}

pub fn quicksave_config_path(profile_dir: &str) -> PathBuf {
    Path::new(profile_dir).join("config.cfg")
}

/// Pfad zur globalen config.cfg (Basis-Verzeichnis)
pub fn ets2_base_config_path() -> Option<PathBuf> {
    resolve_game_user_paths("ets2").map(|paths| paths.config_file)
}

pub fn game_sii_from_save(save_path: &Path) -> PathBuf {
    if save_path.is_file() {
        return save_path
            .parent()
            .map(|parent| parent.join("game.sii"))
            .unwrap_or_else(|| save_path.with_file_name("game.sii"));
    } else {
        save_path.join("game.sii")
    }
}

pub fn info_sii_from_save(save_path: &Path) -> PathBuf {
    if save_path.is_file() {
        return save_path
            .parent()
            .map(|parent| parent.join("info.sii"))
            .unwrap_or_else(|| save_path.with_file_name("info.sii"));
    } else {
        save_path.join("info.sii")
    }
}

/* --------------------------------------------------
   WRAPPER FUER APPLY_SETTING
-------------------------------------------------- */

pub fn autosave_path_current() -> Result<PathBuf, String> {
    let profile = env::var("CURRENT_PROFILE").map_err(|_| "CURRENT_PROFILE not set".to_string())?;

    Ok(PathBuf::from(profile).join("save").join("autosave.sii"))
}

pub fn base_config_path() -> Result<PathBuf, String> {
    ets2_base_config_path().ok_or_else(|| "Could not resolve ETS2 base config path".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn custom_homedir_root_appends_ets2_folder() {
        let selected = PathBuf::from("X:/Mods");
        let (homedir_root, game_data_dir) = resolve_custom_ets2_location_parts(&selected);

        assert_eq!(homedir_root, selected);
        assert_eq!(
            game_data_dir,
            PathBuf::from("X:/Mods").join(ETS2_FOLDER_NAME)
        );
    }

    #[test]
    fn selected_ets2_folder_is_not_appended_twice() {
        let selected = PathBuf::from("X:/Mods").join(ETS2_FOLDER_NAME);
        let (homedir_root, game_data_dir) = resolve_custom_ets2_location_parts(&selected);

        assert_eq!(homedir_root, PathBuf::from("X:/Mods"));
        assert_eq!(game_data_dir, selected);
    }

    #[test]
    fn homedir_argument_is_normalized() {
        let parsed = normalize_user_path_input(r#"-homedir "X:\Mods""#);

        assert_eq!(parsed, Some(PathBuf::from(r#"X:\Mods"#)));
    }

    #[test]
    fn profile_roots_include_steam_profiles_shape() {
        let selected = PathBuf::from("X:/Mods").join(ETS2_FOLDER_NAME);
        let paths = build_game_user_paths(
            "ets2",
            Ets2PathMode::Custom,
            Some(selected.clone()),
            PathBuf::from("X:/Mods"),
            selected,
        );
        let roots = vec![
            paths.profiles_dir,
            paths.steam_profiles_dir.clone(),
            paths.profiles_backup_dir,
            paths.game_data_dir,
        ];

        assert!(roots.iter().any(|path| path.ends_with("steam_profiles")));
    }
}
