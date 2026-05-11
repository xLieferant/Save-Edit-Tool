use super::models::{GameType, ManualWorkshopPath, ModPreset, ModProfileManagerSettings};
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::DialogExt;
use uuid::Uuid;

const STORAGE_FOLDER: &str = "save-edit-tool/mod_profile_manager";
const PRESETS_FILE_NAME: &str = "mod_profile_presets.json";
const SETTINGS_FILE_NAME: &str = "mod_profile_manager_settings.json";

pub fn list_presets(app: &AppHandle, game: Option<GameType>) -> Result<Vec<ModPreset>, String> {
    let mut presets = load_presets(app)?;
    if let Some(game) = game {
        presets.retain(|preset| preset.game == game);
    }
    presets.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
    Ok(presets)
}

pub fn load_presets(app: &AppHandle) -> Result<Vec<ModPreset>, String> {
    let path = presets_file_path(app)?;
    if !path.is_file() {
        return Ok(Vec::new());
    }

    let content = fs::read_to_string(&path)
        .map_err(|error| format!("Failed to read preset storage {}: {}", path.display(), error))?;
    serde_json::from_str(&content).map_err(|error| {
        format!(
            "Failed to parse preset storage {}: {}",
            path.display(),
            error
        )
    })
}

pub fn save_preset(app: &AppHandle, preset: ModPreset) -> Result<ModPreset, String> {
    let mut presets = load_presets(app)?;
    if let Some(index) = presets.iter().position(|item| item.id == preset.id) {
        presets[index] = preset.clone();
    } else {
        presets.push(preset.clone());
    }
    write_presets(app, &presets)?;
    Ok(preset)
}

pub fn delete_preset(app: &AppHandle, preset_id: &str) -> Result<(), String> {
    let mut presets = load_presets(app)?;
    let original_len = presets.len();
    presets.retain(|preset| preset.id != preset_id);
    if presets.len() == original_len {
        return Err("Preset not found.".to_string());
    }
    write_presets(app, &presets)
}

pub fn find_preset(app: &AppHandle, preset_id: &str) -> Result<ModPreset, String> {
    load_presets(app)?
        .into_iter()
        .find(|preset| preset.id == preset_id)
        .ok_or_else(|| "Preset not found.".to_string())
}

pub fn export_preset(app: &AppHandle, preset_id: &str) -> Result<Option<String>, String> {
    let preset = find_preset(app, preset_id)?;
    let file_path = app
        .dialog()
        .file()
        .add_filter("JSON", &["json"])
        .set_title("Export mod preset")
        .set_file_name(&format!("{}.json", sanitized_file_name(&preset.name)))
        .blocking_save_file();

    let Some(file_path) = file_path else {
        return Ok(None);
    };

    let path = file_path_to_path_buf(file_path)?;
    let body = serde_json::to_string_pretty(&preset)
        .map_err(|error| format!("Failed to serialize preset: {}", error))?;
    fs::write(&path, body)
        .map_err(|error| format!("Failed to write {}: {}", path.display(), error))?;
    Ok(Some(path.display().to_string()))
}

pub fn import_preset(app: &AppHandle) -> Result<Option<ModPreset>, String> {
    let file_path = app
        .dialog()
        .file()
        .add_filter("JSON", &["json"])
        .set_title("Import mod preset")
        .blocking_pick_file();

    let Some(file_path) = file_path else {
        return Ok(None);
    };

    let path = file_path_to_path_buf(file_path)?;
    let content = fs::read_to_string(&path)
        .map_err(|error| format!("Failed to read {}: {}", path.display(), error))?;
    let mut preset: ModPreset =
        serde_json::from_str(&content).map_err(|error| format!("Failed to parse preset JSON: {}", error))?;

    let mut presets = load_presets(app)?;
    if presets.iter().any(|item| item.id == preset.id) {
        preset.id = Uuid::new_v4().to_string();
    }
    preset.updated_at = chrono::Local::now().to_rfc3339();
    presets.push(preset.clone());
    write_presets(app, &presets)?;
    Ok(Some(preset))
}

pub fn load_settings(app: &AppHandle) -> Result<ModProfileManagerSettings, String> {
    let path = settings_file_path(app)?;
    if !path.is_file() {
        return Ok(ModProfileManagerSettings::default());
    }

    let content = fs::read_to_string(&path)
        .map_err(|error| format!("Failed to read settings {}: {}", path.display(), error))?;
    serde_json::from_str(&content).map_err(|error| {
        format!(
            "Failed to parse settings {}: {}",
            path.display(),
            error
        )
    })
}

pub fn get_manual_workshop_path(app: &AppHandle, game: GameType) -> Result<Option<String>, String> {
    let settings = load_settings(app)?;
    Ok(settings
        .manual_workshop_paths
        .into_iter()
        .find(|entry| entry.game == game)
        .map(|entry| entry.path))
}

pub fn set_manual_workshop_path(app: &AppHandle, game: GameType, path: String) -> Result<String, String> {
    let mut settings = load_settings(app)?;
    settings
        .manual_workshop_paths
        .retain(|entry| entry.game != game);
    settings.manual_workshop_paths.push(ManualWorkshopPath {
        game,
        path: path.clone(),
    });
    write_settings(app, &settings)?;
    Ok(path)
}

pub fn clear_manual_workshop_path(app: &AppHandle, game: GameType) -> Result<(), String> {
    let mut settings = load_settings(app)?;
    settings
        .manual_workshop_paths
        .retain(|entry| entry.game != game);
    write_settings(app, &settings)
}

pub fn pick_workshop_directory(app: &AppHandle) -> Result<Option<String>, String> {
    let folder = app
        .dialog()
        .file()
        .set_title("Select Steam Workshop folder")
        .blocking_pick_folder();

    let Some(folder) = folder else {
        return Ok(None);
    };

    let path = file_path_to_path_buf(folder)?;
    Ok(Some(path.display().to_string()))
}

fn write_presets(app: &AppHandle, presets: &[ModPreset]) -> Result<(), String> {
    let path = presets_file_path(app)?;
    let body = serde_json::to_string_pretty(presets)
        .map_err(|error| format!("Failed to serialize mod presets: {}", error))?;
    fs::write(&path, body)
        .map_err(|error| format!("Failed to write {}: {}", path.display(), error))
}

fn write_settings(app: &AppHandle, settings: &ModProfileManagerSettings) -> Result<(), String> {
    let path = settings_file_path(app)?;
    let body = serde_json::to_string_pretty(settings)
        .map_err(|error| format!("Failed to serialize settings: {}", error))?;
    fs::write(&path, body)
        .map_err(|error| format!("Failed to write {}: {}", path.display(), error))
}

fn presets_file_path(app: &AppHandle) -> Result<PathBuf, String> {
    storage_dir(app).map(|dir| dir.join(PRESETS_FILE_NAME))
}

fn settings_file_path(app: &AppHandle) -> Result<PathBuf, String> {
    storage_dir(app).map(|dir| dir.join(SETTINGS_FILE_NAME))
}

fn storage_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let mut dir = app
        .path()
        .config_dir()
        .map_err(|error| format!("Failed to resolve app config directory: {}", error))?;
    dir.push(STORAGE_FOLDER);
    fs::create_dir_all(&dir)
        .map_err(|error| format!("Failed to create {}: {}", dir.display(), error))?;
    Ok(dir)
}

fn file_path_to_path_buf(path: tauri_plugin_dialog::FilePath) -> Result<PathBuf, String> {
    path.into_path()
        .map_err(|_| "The selected path could not be resolved.".to_string())
}

fn sanitized_file_name(value: &str) -> String {
    let mut out = String::new();
    for character in value.chars() {
        if character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | ' ') {
            out.push(character);
        }
    }

    let trimmed = out.trim();
    if trimmed.is_empty() {
        "mod-preset".to_string()
    } else {
        trimmed.replace(' ', "_")
    }
}
