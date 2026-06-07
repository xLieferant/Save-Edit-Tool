use super::models::{
    AppliedWorkshopMod, ApplySandboxResult, GameType, ModSandbox, SandboxCollection,
    SkippedWorkshopMod, WorkshopMod,
};
use super::presets;
use super::sii_mods;
use super::steam_paths;
use super::workshop_api;
use crate::shared::current_profile::snapshot_active_save_selection;
use crate::shared::paths::game_sii_from_save;
use crate::state::AppProfileState;
use std::fs;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};
use uuid::Uuid;

const STORAGE_FOLDER: &str = "save-edit-tool/mod_profile_manager";
const SANDBOXES_FILE_NAME: &str = "sandboxes.json";

pub fn load_sandboxes(app: &AppHandle) -> Result<SandboxCollection, String> {
    let path = sandboxes_path(app)?;
    println!(
        "[mod-profile-manager] load sandboxes path={}",
        path.display()
    );
    if !path.is_file() {
        return Ok(SandboxCollection::default());
    }

    let content = fs::read_to_string(&path)
        .map_err(|error| format!("Failed to read {}: {}", path.display(), error))?;
    serde_json::from_str(&content)
        .map_err(|error| format!("Failed to parse {}: {}", path.display(), error))
}

pub fn save_sandboxes(app: &AppHandle, collection: &SandboxCollection) -> Result<(), String> {
    let path = sandboxes_path(app)?;
    println!(
        "[mod-profile-manager] save sandboxes path={} count={}",
        path.display(),
        collection.sandboxes.len()
    );
    let body = serde_json::to_string_pretty(collection)
        .map_err(|error| format!("Failed to serialize sandboxes: {}", error))?;
    fs::write(&path, body).map_err(|error| format!("Failed to write {}: {}", path.display(), error))
}

pub fn add_sandbox(
    app: &AppHandle,
    title: String,
    description: String,
) -> Result<SandboxCollection, String> {
    let mut collection = load_sandboxes(app)?;
    let title = title.trim();
    if title.is_empty() {
        return Err("Sandbox title is required.".to_string());
    }

    let sandbox = ModSandbox {
        id: unique_sandbox_id(title, &collection),
        title: title.to_string(),
        description: description.trim().to_string(),
        mods: Vec::new(),
    };
    collection.sandboxes.push(sandbox);
    save_sandboxes(app, &collection)?;
    Ok(collection)
}

pub fn update_sandbox(
    app: &AppHandle,
    updated_sandbox: ModSandbox,
) -> Result<SandboxCollection, String> {
    let mut collection = load_sandboxes(app)?;
    let index = collection
        .sandboxes
        .iter()
        .position(|sandbox| sandbox.id == updated_sandbox.id)
        .ok_or_else(|| "Sandbox not found.".to_string())?;
    collection.sandboxes[index] = updated_sandbox;
    save_sandboxes(app, &collection)?;
    Ok(collection)
}

pub fn remove_sandbox(app: &AppHandle, sandbox_id: &str) -> Result<SandboxCollection, String> {
    let mut collection = load_sandboxes(app)?;
    let previous_len = collection.sandboxes.len();
    collection
        .sandboxes
        .retain(|sandbox| sandbox.id != sandbox_id);
    if previous_len == collection.sandboxes.len() {
        return Err("Sandbox not found.".to_string());
    }

    save_sandboxes(app, &collection)?;
    Ok(collection)
}

pub fn add_workshop_mod_to_sandbox(
    app: &AppHandle,
    sandbox_id: &str,
    workshop_input: &str,
    manual_fallback: bool,
) -> Result<SandboxCollection, String> {
    let workshop_mod = match workshop_api::fetch_workshop_mod(workshop_input) {
        Ok(workshop_mod) => workshop_mod,
        Err(error) if manual_fallback => {
            println!(
                "[mod-profile-manager] Steam metadata fetch failed, using manual fallback: {error}"
            );
            manual_workshop_mod_from_input(workshop_input)?
        }
        Err(error) => return Err(error),
    };
    let mut collection = load_sandboxes(app)?;
    let sandbox = find_sandbox_mut(&mut collection, sandbox_id)?;
    if sandbox.mods.iter().any(|mod_| mod_.id == workshop_mod.id) {
        return Err("This Workshop mod is already in the selected sandbox.".to_string());
    }
    sandbox.mods.push(workshop_mod);
    save_sandboxes(app, &collection)?;
    Ok(collection)
}

pub fn upsert_sandbox_preset(
    app: &AppHandle,
    mut sandbox: ModSandbox,
) -> Result<SandboxCollection, String> {
    let mut collection = load_sandboxes(app)?;
    let sandbox_id = sandbox.id.trim().to_string();
    if sandbox_id.is_empty() {
        return Err("Sandbox ID is required.".to_string());
    }
    sandbox.id = sandbox_id.clone();
    println!(
        "[mod-profile-manager] upsert sandbox_id={} title={}",
        sandbox.id, sandbox.title
    );

    match collection
        .sandboxes
        .iter()
        .position(|item| item.id == sandbox_id)
    {
        Some(index) => collection.sandboxes[index] = sandbox,
        None => collection.sandboxes.push(sandbox),
    }

    save_sandboxes(app, &collection)?;
    Ok(collection)
}

pub fn upsert_test_sandbox_preset(app: &AppHandle) -> Result<SandboxCollection, String> {
    upsert_sandbox_preset(
        app,
        ModSandbox {
            id: "test".to_string(),
            title: "Test".to_string(),
            description: "Test preset for Workshop mod 3710074411".to_string(),
            mods: vec![WorkshopMod {
                id: 3710074411,
                name: "Test".to_string(),
                app_id: 227300,
                enabled: true,
                url: Some(
                    "https://steamcommunity.com/sharedfiles/filedetails/?id=3710074411".to_string(),
                ),
                status: Some("metadata_unverified".to_string()),
            }],
        },
    )
}

pub fn remove_workshop_mod_from_sandbox(
    app: &AppHandle,
    sandbox_id: &str,
    mod_id: u64,
) -> Result<SandboxCollection, String> {
    let mut collection = load_sandboxes(app)?;
    let sandbox = find_sandbox_mut(&mut collection, sandbox_id)?;
    let previous_len = sandbox.mods.len();
    sandbox.mods.retain(|mod_| mod_.id != mod_id);
    if previous_len == sandbox.mods.len() {
        return Err("Workshop mod not found in sandbox.".to_string());
    }
    save_sandboxes(app, &collection)?;
    Ok(collection)
}

pub fn toggle_workshop_mod_enabled(
    app: &AppHandle,
    sandbox_id: &str,
    mod_id: u64,
    enabled: bool,
) -> Result<SandboxCollection, String> {
    let mut collection = load_sandboxes(app)?;
    let sandbox = find_sandbox_mut(&mut collection, sandbox_id)?;
    let workshop_mod = sandbox
        .mods
        .iter_mut()
        .find(|mod_| mod_.id == mod_id)
        .ok_or_else(|| "Workshop mod not found in sandbox.".to_string())?;
    workshop_mod.enabled = enabled;
    save_sandboxes(app, &collection)?;
    Ok(collection)
}

pub fn apply_sandbox_to_active_profile(
    app: &AppHandle,
    profile_state: &AppProfileState,
    sandbox_id: &str,
) -> Result<ApplySandboxResult, String> {
    apply_sandbox_to_active_profile_with_force(app, profile_state, sandbox_id, false)
}

pub fn apply_sandbox_to_active_profile_with_force(
    app: &AppHandle,
    profile_state: &AppProfileState,
    sandbox_id: &str,
    force_clear: bool,
) -> Result<ApplySandboxResult, String> {
    let collection = load_sandboxes(app)?;
    println!("[mod-profile-manager] apply sandbox_id={sandbox_id} force_clear={force_clear}");
    let sandbox = collection
        .sandboxes
        .iter()
        .find(|sandbox| sandbox.id == sandbox_id)
        .ok_or_else(|| "No active sandbox was found.".to_string())?;

    let selection = snapshot_active_save_selection(profile_state)?;
    if selection.profile_path.is_none() {
        return Err(
            "Bitte zuerst ein ETS2-Profil laden, bevor du eine Mod-Sandbox anwenden kannst."
                .to_string(),
        );
    }
    let save_path = selection
        .save_path
        .ok_or_else(|| "Bitte zuerst einen ETS2-Speicherstand auswählen.".to_string())?;
    let game_sii = game_sii_from_save(Path::new(&save_path));
    if !game_sii.is_file() {
        return Err(format!("game.sii not found: {}", game_sii.display()));
    }

    let (applied_mods, skipped_mods) = classify_sandbox_mods_for_apply(app, sandbox)?;
    if applied_mods.is_empty() && !force_clear {
        return Err(
            "No installed enabled mods found. Refusing to clear all active mods without force."
                .to_string(),
        );
    }

    let workshop_mod_ids = applied_mods
        .iter()
        .map(|mod_| mod_.mod_id.clone())
        .collect::<Vec<_>>();
    let (backup_path, removed_existing_mod_count) =
        sii_mods::overwrite_active_workshop_mods(&game_sii, &workshop_mod_ids)?;
    println!(
        "[mod-profile-manager] apply removed_existing_mod_count={} backup_path={}",
        removed_existing_mod_count,
        backup_path.display()
    );
    let applied_mod_count = applied_mods.len();

    Ok(ApplySandboxResult {
        sandbox_id: sandbox.id.clone(),
        sandbox_title: sandbox.title.clone(),
        game_sii_path: game_sii.display().to_string(),
        backup_path: backup_path.display().to_string(),
        applied_mods,
        skipped_mods,
        removed_existing_mod_count,
        applied_mod_count,
    })
}

pub fn sandboxes_path(app: &AppHandle) -> Result<PathBuf, String> {
    storage_dir(app).map(|dir| dir.join(SANDBOXES_FILE_NAME))
}

fn manual_workshop_mod_from_input(workshop_input: &str) -> Result<WorkshopMod, String> {
    let id = workshop_api::parse_workshop_id(workshop_input)?;
    Ok(WorkshopMod {
        id,
        name: format!("Workshop Mod {id}"),
        app_id: 227300,
        enabled: true,
        url: Some(format!(
            "https://steamcommunity.com/sharedfiles/filedetails/?id={id}"
        )),
        status: Some("metadata_unverified".to_string()),
    })
}

fn find_sandbox_mut<'a>(
    collection: &'a mut SandboxCollection,
    sandbox_id: &str,
) -> Result<&'a mut ModSandbox, String> {
    collection
        .sandboxes
        .iter_mut()
        .find(|sandbox| sandbox.id == sandbox_id)
        .ok_or_else(|| "Sandbox not found.".to_string())
}

fn classify_sandbox_mods_for_apply(
    app: &AppHandle,
    sandbox: &ModSandbox,
) -> Result<(Vec<AppliedWorkshopMod>, Vec<SkippedWorkshopMod>), String> {
    let mut applied_mods = Vec::new();
    let mut skipped_mods = Vec::new();

    for workshop_mod in &sandbox.mods {
        println!(
            "[mod-profile-manager] apply candidate mod_id={} enabled={} app_id={}",
            workshop_mod.id, workshop_mod.enabled, workshop_mod.app_id
        );
        if !workshop_mod.enabled {
            skipped_mods.push(skipped_mod(workshop_mod, "disabled_in_sandbox"));
            println!(
                "[mod-profile-manager] apply skipped mod_id={} reason=disabled_in_sandbox",
                workshop_mod.id
            );
            continue;
        }
        if workshop_mod.id == 0 {
            skipped_mods.push(skipped_mod(workshop_mod, "invalid_mod_id"));
            println!(
                "[mod-profile-manager] apply skipped mod_id={} reason=invalid_mod_id",
                workshop_mod.id
            );
            continue;
        }
        if workshop_mod.app_id != 227300 {
            skipped_mods.push(skipped_mod(workshop_mod, "not_ets2_workshop_mod"));
            println!(
                "[mod-profile-manager] apply skipped mod_id={} reason=not_ets2_workshop_mod",
                workshop_mod.id
            );
            continue;
        }

        match installed_workshop_mod_path(app, workshop_mod.id)? {
            Some(workshop_path) => {
                println!(
                    "[mod-profile-manager] apply installed mod_id={} path={}",
                    workshop_mod.id,
                    workshop_path.display()
                );
                applied_mods.push(AppliedWorkshopMod {
                    mod_id: workshop_mod.id.to_string(),
                    title: Some(workshop_mod.name.clone()),
                    workshop_path: workshop_path.display().to_string(),
                });
            }
            None => {
                skipped_mods.push(skipped_mod(workshop_mod, "not_installed"));
                println!(
                    "[mod-profile-manager] apply skipped mod_id={} reason=not_installed",
                    workshop_mod.id
                );
            }
        }
    }

    Ok((applied_mods, skipped_mods))
}

fn installed_workshop_mod_path(app: &AppHandle, mod_id: u64) -> Result<Option<PathBuf>, String> {
    let manual_path = presets::get_manual_workshop_path(app, GameType::Ets2)?;
    let discovery = steam_paths::discover_workshop_sources(GameType::Ets2, manual_path.as_deref());
    for source in discovery.workshop_sources {
        let candidate = Path::new(&source.path).join(mod_id.to_string());
        println!(
            "[mod-profile-manager] check installed mod_id={} candidate={}",
            mod_id,
            candidate.display()
        );
        if candidate.is_dir() {
            return Ok(Some(candidate));
        }
    }

    Ok(None)
}

fn skipped_mod(workshop_mod: &WorkshopMod, reason: &str) -> SkippedWorkshopMod {
    SkippedWorkshopMod {
        mod_id: workshop_mod.id.to_string(),
        title: Some(workshop_mod.name.clone()),
        reason: reason.to_string(),
    }
}

fn unique_sandbox_id(title: &str, collection: &SandboxCollection) -> String {
    let mut base = title
        .trim()
        .to_ascii_lowercase()
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                '_'
            }
        })
        .collect::<String>();
    base = base
        .split('_')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>()
        .join("_");
    if base.is_empty() {
        base = "sandbox".to_string();
    }

    if !collection
        .sandboxes
        .iter()
        .any(|sandbox| sandbox.id == base)
    {
        return base;
    }

    format!("{}_{}", base, Uuid::new_v4().simple())
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

#[cfg(test)]
mod tests {
    use super::super::models::WorkshopMod;
    use super::*;

    #[test]
    fn serializes_and_deserializes_sandbox_collection() {
        let collection = SandboxCollection {
            sandboxes: vec![ModSandbox {
                id: "realism_v1".to_string(),
                title: "Realism Setup V1".to_string(),
                description: "Fokus auf realistische Sound- und Physik-Mods".to_string(),
                mods: vec![WorkshopMod {
                    id: 3710074411,
                    name: "Realistic Cabin Soundproofing".to_string(),
                    app_id: 227300,
                    enabled: true,
                    url: None,
                    status: Some("verified".to_string()),
                }],
            }],
        };

        let json = serde_json::to_string_pretty(&collection).unwrap();
        let parsed: SandboxCollection = serde_json::from_str(&json).unwrap();
        assert_eq!(collection, parsed);
    }

    #[test]
    fn deserializes_test_preset_shape() {
        let json = r#"{
          "id": "test",
          "title": "Test",
          "description": "Test preset for Workshop mod 3710074411",
          "mods": [
            {
              "id": "3710074411",
              "title": "Test",
              "url": "https://steamcommunity.com/sharedfiles/filedetails/?id=3710074411",
              "enabled": true
            }
          ]
        }"#;

        let sandbox: ModSandbox = serde_json::from_str(json).unwrap();
        assert_eq!(sandbox.id, "test");
        assert_eq!(sandbox.mods[0].id, 3710074411);
        assert_eq!(sandbox.mods[0].name, "Test");
        assert_eq!(sandbox.mods[0].app_id, 227300);
    }
}
