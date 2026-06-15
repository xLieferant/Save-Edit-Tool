use super::models::{
    ActivatedModEntry, ActivationMissingModEntry, ActivationVerification, ActiveModBlockSnapshot,
    AppliedWorkshopMod, ApplySandboxResult, ModSandbox, SandboxActiveModsBackupCacheEntry,
    SandboxActiveModsBackupCacheFile, SandboxCollection, SandboxModCacheEntry, SandboxModCacheFile,
    SandboxModPreset, SandboxPresetActivationResult, SandboxPresetCheckResult,
    SandboxPresetCollection, SandboxPresetModStatus, SkippedWorkshopMod, SteamWorkshopCache,
    SteamWorkshopMod, ValidateActivePresetModsResult, WorkshopMod,
};
use super::sii_mods;
use super::steam_paths;
use super::workshop_api;
use crate::features::backup::service as backup_service;
use crate::shared::current_profile::snapshot_active_save_selection;
use crate::shared::decrypt::decrypt_if_needed;
use crate::shared::paths::{game_sii_from_save, get_base_path, mod_directory_path};
use crate::state::{AppProfileState, DecryptCache, ProfileCache};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};
use uuid::Uuid;

const STORAGE_FOLDER: &str = "save-edit-tool/mod_profile_manager";
const SANDBOXES_FILE_NAME: &str = "sandboxes.json";
const MOD_SANDBOX_PRESETS_JSON: &str = include_str!("../../../data/mod_sandbox_presets.json");
const MOD_SANDBOX_CACHE_FILE_NAME: &str = "modCacheSandbox.json";
const SANDBOX_TEST_CACHE_FILE_NAME: &str = "sandboxTestCache.json";
const STEAM_WORKSHOP_CACHE_FILE_NAME: &str = "steam_workshop_mods_cache.json";

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
    let sandbox = collection
        .sandboxes
        .iter()
        .find(|sandbox| sandbox.id == sandbox_id)
        .ok_or_else(|| "No active sandbox was found.".to_string())?;
    println!(
        "[mod-profile-manager] apply sandbox_id={} sandbox_title={} force_clear={}",
        sandbox.id, sandbox.title, force_clear
    );

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
    println!(
        "[mod-profile-manager] apply game_sii_path={}",
        game_sii.display()
    );

    let (applied_mods, skipped_mods) = classify_sandbox_mods_for_apply(sandbox)?;
    if applied_mods.is_empty() && !force_clear {
        return Err(
            "No installed enabled mods found. Refusing to clear all active mods without force."
                .to_string(),
        );
    }

    let old_mod_count = sii_mods::read_active_mods(&game_sii)
        .map(|mods| mods.len())
        .unwrap_or(0);
    let workshop_mod_ids = applied_mods
        .iter()
        .map(|mod_| mod_.mod_id.clone())
        .collect::<Vec<_>>();
    println!(
        "[mod-profile-manager] apply expected_workshop_ids={:?}",
        workshop_mod_ids
    );
    println!(
        "[mod-profile-manager] apply old_mod_count={}",
        old_mod_count
    );
    let (backup_path, replace_result) =
        sii_mods::overwrite_active_preset_mods(&game_sii, &workshop_mod_ids)?;
    println!(
        "[mod-profile-manager] apply backup_path={} removed_existing_mod_count={} new_mod_count={} expected_mod_refs={:?}",
        backup_path.display(),
        replace_result.removed_mod_count,
        replace_result.written_mod_count,
        replace_result.expected_mod_refs
    );
    let reapplied_content = fs::read_to_string(&game_sii)
        .map_err(|error| format!("Failed to re-read {}: {}", game_sii.display(), error));
    let (validation, success, message) = match reapplied_content {
        Ok(content) => {
            match sii_mods::validate_active_preset_mods_in_game_sii(&content, &workshop_mod_ids) {
                Ok(validation) => {
                    println!(
                        "[mod-profile-manager] apply actual_mod_refs={:?} validation_success={} validation_order_matches={} missing_mod_refs={:?} unexpected_mod_refs={:?}",
                        validation.actual_mod_refs,
                        validation.success,
                        validation.order_matches,
                        validation.missing_mod_refs,
                        validation.unexpected_mod_refs
                    );
                    let success = validation.success;
                    let message = if success {
                        "Preset wurde erfolgreich angewendet und validiert.".to_string()
                    } else {
                        validation_failure_message(&validation)
                    };
                    (validation, success, message)
                }
                Err(error) => {
                    let validation = fallback_validation_result(
                        &workshop_mod_ids,
                        &replace_result.expected_mod_refs,
                    );
                    println!(
                        "[mod-profile-manager] apply validation_failed error={} actual_mod_refs={:?}",
                        error, validation.actual_mod_refs
                    );
                    (
                        validation,
                        false,
                        format!(
                            "Preset wurde geschrieben, aber die Validierung ist fehlgeschlagen. {}",
                            error
                        ),
                    )
                }
            }
        }
        Err(error) => {
            let validation =
                fallback_validation_result(&workshop_mod_ids, &replace_result.expected_mod_refs);
            println!(
                "[mod-profile-manager] apply reread_failed error={} validation_success={}",
                error, validation.success
            );
            (
                validation,
                false,
                format!(
                    "Preset wurde geschrieben, aber die Datei konnte nicht erneut gelesen werden. {}",
                    error
                ),
            )
        }
    };
    let removed_existing_mod_count = replace_result.removed_mod_count;
    let applied_mod_count = replace_result.written_mod_count;

    Ok(ApplySandboxResult {
        sandbox_id: sandbox.id.clone(),
        sandbox_title: sandbox.title.clone(),
        game_sii_path: game_sii.display().to_string(),
        backup_path: backup_path.display().to_string(),
        applied_mods,
        skipped_mods,
        removed_existing_mod_count,
        applied_mod_count,
        validation,
        success,
        message,
    })
}

pub fn sandboxes_path(app: &AppHandle) -> Result<PathBuf, String> {
    storage_dir(app).map(|dir| dir.join(SANDBOXES_FILE_NAME))
}

pub fn load_sandbox_mod_presets() -> Result<Vec<SandboxModPreset>, String> {
    let collection: SandboxPresetCollection = serde_json::from_str(MOD_SANDBOX_PRESETS_JSON)
        .map_err(|error| format!("Failed to parse bundled sandbox presets: {}", error))?;
    crate::dev_log!(
        "[SandboxPreset] loaded bundled presets count={}",
        collection.sandbox_presets.len()
    );
    Ok(collection.sandbox_presets)
}

pub fn scan_steam_workshop_mods(app: &AppHandle) -> Result<SteamWorkshopCache, String> {
    crate::dev_log!("[SteamWorkshopCache] scan requested");
    let mods = workshop_api::scan_installed_workshop_mods()?;
    let cache = SteamWorkshopCache {
        generated_at: chrono::Utc::now().to_rfc3339(),
        mods,
    };
    write_steam_workshop_cache(app, &cache)?;
    crate::dev_log!(
        "[SteamWorkshopCache] scan completed mods={} cache_path={}",
        cache.mods.len(),
        steam_workshop_cache_path(app)?.display()
    );
    Ok(cache)
}

pub fn refresh_workshop_mod_cache(app: &AppHandle) -> Result<SteamWorkshopCache, String> {
    crate::dev_log!("[SteamWorkshopCache] refresh requested");
    scan_steam_workshop_mods(app)
}

pub fn load_steam_workshop_mod_cache(app: &AppHandle) -> Result<SteamWorkshopCache, String> {
    let path = steam_workshop_cache_path(app)?;
    crate::dev_log!("[SteamWorkshopCache] load path={}", path.display());
    let content = fs::read_to_string(&path).map_err(|error| {
        format!(
            "Failed to read Steam Workshop cache {}: {}",
            path.display(),
            error
        )
    })?;
    let cache = serde_json::from_str::<SteamWorkshopCache>(&content).map_err(|error| {
        format!(
            "Failed to parse Steam Workshop cache {}: {}",
            path.display(),
            error
        )
    })?;
    crate::dev_log!(
        "[SteamWorkshopCache] loaded generated_at={} mods={}",
        cache.generated_at,
        cache.mods.len()
    );
    Ok(cache)
}

pub fn check_workshop_mod_available(
    app: &AppHandle,
    app_id: u32,
    workshop_id: &str,
) -> Result<SteamWorkshopMod, String> {
    let cache = ensure_steam_workshop_cache(app)?;
    resolve_workshop_mod_from_cache(&cache, app_id, workshop_id)
}

fn ensure_steam_workshop_cache(app: &AppHandle) -> Result<SteamWorkshopCache, String> {
    match load_steam_workshop_mod_cache(app) {
        Ok(cache) => Ok(cache),
        Err(error) => {
            crate::dev_log!(
                "[SteamWorkshopCache] cache load failed, rescanning error={}",
                error
            );
            scan_steam_workshop_mods(app)
        }
    }
}

fn resolve_workshop_mod_from_cache(
    cache: &SteamWorkshopCache,
    app_id: u32,
    workshop_id: &str,
) -> Result<SteamWorkshopMod, String> {
    let workshop_id = workshop_id.trim();
    if workshop_id.is_empty()
        || !workshop_id
            .chars()
            .all(|character| character.is_ascii_digit())
    {
        return Err(format!("Invalid Workshop ID: {}", workshop_id));
    }
    if !workshop_api::is_supported_workshop_app_id(app_id) {
        return Err(format!("Unsupported Workshop AppID: {}", app_id));
    }

    crate::dev_log!(
        "[SteamWorkshopCache] check command called app_id={} workshop_id={}",
        app_id,
        workshop_id
    );
    crate::dev_log!(
        "[SteamWorkshopCache] checking preset_mod app_id={} workshop_id={}",
        app_id,
        workshop_id
    );

    let cached = cache
        .mods
        .iter()
        .find(|item| item.app_id == app_id && item.workshop_id == workshop_id)
        .cloned();

    let install_status =
        workshop_api::check_workshop_mod_installed(workshop_id, &app_id.to_string())?;
    let local_path = install_status
        .workshop_path
        .clone()
        .or_else(|| cached.as_ref().map(|item| item.local_path.clone()))
        .unwrap_or_default();
    let reachable = if !local_path.is_empty() {
        Path::new(&local_path).exists()
    } else {
        false
    };
    let installed =
        install_status.installed || (!local_path.is_empty() && Path::new(&local_path).is_dir());
    let available = install_status.installed;
    let result = SteamWorkshopMod {
        game: workshop_api::game_name_for_app_id(app_id).to_string(),
        app_id,
        workshop_id: workshop_id.to_string(),
        installed,
        available,
        reachable,
        local_path,
        workshop_url: workshop_api::workshop_page_url(workshop_id),
    };

    crate::dev_log!(
        "[SteamWorkshopCache] check result app_id={} workshop_id={} installed={} available={} reachable={} local_path={} reason={}",
        result.app_id,
        result.workshop_id,
        result.installed,
        result.available,
        result.reachable,
        if result.local_path.is_empty() {
            "-"
        } else {
            &result.local_path
        },
        install_status.reason.as_deref().unwrap_or("none")
    );

    Ok(result)
}

pub fn check_sandbox_mod_preset(
    app: &AppHandle,
    preset_id: &str,
) -> Result<SandboxPresetCheckResult, String> {
    crate::dev_log!("[SandboxPreset] Checking preset: {}", preset_id);
    let preset = find_sandbox_preset(preset_id)?;
    let mut progress_log = vec![
        "Preset geladen".to_string(),
        format!("Loading preset {}.", preset.id),
    ];
    if preset.mods.is_empty() {
        let checked_at = chrono::Local::now().to_rfc3339();
        progress_log.push("Preset has no mods configured.".to_string());
        return Ok(SandboxPresetCheckResult {
            preset_id: preset.id,
            title: preset.title,
            ready: false,
            can_activate: false,
            mods: Vec::new(),
            missing_required_mods: Vec::new(),
            missing_mods: Vec::new(),
            found_mods: Vec::new(),
            all_mods: Vec::new(),
            checked_libraries: Vec::new(),
            checked_at,
            message: "Preset has no mods configured.".to_string(),
            progress_log,
            cache_path: None,
        });
    }
    let statuses = collect_preset_mod_statuses(app, &preset, &mut progress_log)?;
    let checked_at = chrono::Local::now().to_rfc3339();
    let mod_cache_path = match write_mod_cache_entry(
        app,
        SandboxModCacheEntry {
            preset_id: preset.id.clone(),
            title: preset.title.clone(),
            checked_at: checked_at.clone(),
            checked_libraries: collect_checked_libraries(&statuses),
            mods: statuses.clone(),
        },
    ) {
        Ok(path) => Some(path),
        Err(error) => {
            crate::dev_log!(
                "[SandboxPreset] mod cache write failed preset_id={} error={}",
                preset.id,
                error
            );
            progress_log.push(format!(
                "Warning: mod cache could not be written. {}",
                error
            ));
            None
        }
    };
    progress_log.push("Mods geprüft".to_string());
    let (found_mods, missing_mods) = split_mod_statuses(&statuses);
    let missing_required_mods = statuses
        .iter()
        .filter(|status| status.required && !(status.available && status.reachable))
        .cloned()
        .collect::<Vec<_>>();
    let ready = missing_required_mods.is_empty();
    let checked_libraries = collect_checked_libraries(&statuses);
    let message = if ready {
        optional_missing_mods_summary(&statuses)
            .unwrap_or_else(|| "All required mods found.".to_string())
    } else {
        missing_mods_summary(&missing_required_mods)
    };
    if ready {
        if let Some(optional_warning) = optional_missing_mods_summary(&statuses) {
            progress_log.push(optional_warning);
        }
    }
    progress_log.push(if ready {
        "Preset is ready.".to_string()
    } else {
        "Required mods are missing.".to_string()
    });

    Ok(SandboxPresetCheckResult {
        preset_id: preset.id,
        title: preset.title,
        ready,
        can_activate: ready,
        mods: statuses.clone(),
        missing_required_mods,
        missing_mods,
        found_mods,
        all_mods: statuses,
        checked_libraries: checked_libraries.clone(),
        checked_at,
        message,
        progress_log,
        cache_path: mod_cache_path.map(|path| path.display().to_string()),
    })
}

#[allow(dead_code, unreachable_code)]
pub fn activate_sandbox_mod_preset(
    app: &AppHandle,
    profile_state: &AppProfileState,
    profile_cache: &ProfileCache,
    decrypt_cache: &DecryptCache,
    preset_id: &str,
) -> Result<SandboxPresetActivationResult, String> {
    crate::dev_log!("[SandboxPreset] Activating preset: {}", preset_id);
    crate::dev_log!(
        "[SandboxPreset] unexpected game.sii usage removed from preset activation flow"
    );
    return activate_sandbox_mod_preset_profile_sii(
        app,
        profile_state,
        profile_cache,
        decrypt_cache,
        preset_id,
        None,
        None,
        None,
        None,
    );

    let preset = match find_sandbox_preset(preset_id) {
        Ok(preset) => preset,
        Err(error) => {
            return Ok(failure_activation_result(
                preset_id,
                "Preset nicht gefunden",
                "preset_not_found",
                error,
                Vec::new(),
                None,
            ));
        }
    };

    let mut progress_log = vec![
        "Preset geladen".to_string(),
        format!("Loading preset {}.", preset.id),
    ];
    let statuses = collect_preset_mod_statuses(app, &preset, &mut progress_log)?;
    let checked_at = chrono::Local::now().to_rfc3339();
    let mod_cache_path = write_mod_cache_entry(
        app,
        SandboxModCacheEntry {
            preset_id: preset.id.clone(),
            title: preset.title.clone(),
            checked_at,
            checked_libraries: collect_checked_libraries(&statuses),
            mods: statuses.clone(),
        },
    )
    .map(|path| path.display().to_string())
    .ok();
    progress_log.push("Mods geprüft".to_string());

    let required_missing = statuses
        .iter()
        .filter(|status| status.required && !(status.available && status.reachable))
        .cloned()
        .collect::<Vec<_>>();
    if !required_missing.is_empty() {
        return Ok(failure_activation_result(
            &preset.id,
            &preset.title,
            "mod_not_found",
            missing_mods_summary(&required_missing),
            progress_log,
            mod_cache_path.clone(),
        ));
    }

    let selection = snapshot_active_save_selection(profile_state)?;
    if selection.profile_path.is_none() || selection.save_path.is_none() {
        return Ok(failure_activation_result(
            &preset.id,
            &preset.title,
            "no_active_save",
            "Bitte zuerst ein aktives Profil und einen aktiven Save auswahlen.".to_string(),
            progress_log,
            mod_cache_path.clone(),
        ));
    }

    progress_log.push("Profil geöffnet".to_string());
    let save_path = selection.save_path.unwrap_or_default();
    let game_sii = game_sii_from_save(Path::new(&save_path));
    crate::dev_log!("[SandboxPreset] active save dir={}", save_path);
    crate::dev_log!("[SandboxPreset] game_sii path={}", game_sii.display());
    crate::dev_log!("[SandboxPreset] game_sii exists={}", game_sii.exists());
    if !game_sii.is_file() {
        return Ok(failure_activation_result(
            &preset.id,
            &preset.title,
            "save_not_found",
            format!(
                "Die aktive Save-Datei wurde nicht gefunden: {}",
                game_sii.display()
            ),
            progress_log,
            mod_cache_path.clone(),
        ));
    }

    let original_state = match sii_mods::inspect_active_mod_block(&game_sii) {
        Ok(snapshot) => {
            crate::dev_log!("[SandboxPreset] active_mods existing block found=true");
            snapshot
        }
        Err(error) => {
            if error.contains("No active_mods/actived_mods block found") {
                crate::dev_log!("[SandboxPreset] active_mods existing block found=false");
                crate::dev_log!("[SandboxPreset] active_mods block not found, inserting new block");
                ActiveModBlockSnapshot {
                    field_name: "active_mods".to_string(),
                    count: 0,
                    mod_refs: Vec::new(),
                    indices: Vec::new(),
                }
            } else {
                return Ok(failure_activation_result(
                    &preset.id,
                    &preset.title,
                    "save_read_failed",
                    error,
                    progress_log,
                    mod_cache_path.clone(),
                ));
            }
        }
    };
    progress_log.push("actived_mods gelesen".to_string());

    let backup = match backup_service::create_backup_for_targets(
        profile_state,
        "before sandbox preset activation",
        &backup_service::recommended_targets(&game_sii),
    ) {
        Ok(backup) => backup,
        Err(error) => {
            return Ok(failure_activation_result(
                &preset.id,
                &preset.title,
                "backup_failed",
                format!("Backup fehlgeschlagen: {}", error),
                progress_log,
                mod_cache_path.clone(),
            ));
        }
    };
    let backup_path = backup_service::get_backup_storage_dir(&backup.backup_id)
        .ok()
        .map(|path| path.display().to_string());
    progress_log.push("Backup erstellt".to_string());

    let sandbox_cache_path = write_sandbox_test_cache_entry(
        app,
        SandboxActiveModsBackupCacheEntry {
            preset_id: preset.id.clone(),
            title: preset.title.clone(),
            save_path: save_path.clone(),
            file_path: game_sii.display().to_string(),
            field_name: original_state.field_name.clone(),
            original_count: original_state.count,
            original_mod_refs: original_state.mod_refs.clone(),
            original_indices: original_state.indices.clone(),
            timestamp: chrono::Local::now().to_rfc3339(),
        },
    )?;

    let mods_written = match build_activated_mod_entries(&statuses) {
        Ok(entries) => entries,
        Err(error) => {
            return Ok(failure_activation_result(
                &preset.id,
                &preset.title,
                "invalid_mod_identifier",
                error,
                progress_log,
                mod_cache_path.clone(),
            ));
        }
    };
    let active_mod_values = mods_written
        .iter()
        .map(|entry| entry.active_mods_value.clone())
        .collect::<Vec<_>>();
    let mods_to_write = active_mod_values.clone();
    if mods_to_write.is_empty() {
        return Ok(failure_activation_result(
            &preset.id,
            &preset.title,
            "no_mods_to_write",
            "Es wurden keine lokal verfügbaren Mods gefunden, die geschrieben werden konnten."
                .to_string(),
            progress_log,
            mod_cache_path.clone(),
        ));
    }

    let replace_result = match sii_mods::write_active_mod_values_atomic(&game_sii, &mods_to_write) {
        Ok(result) => result,
        Err(error) => {
            return Ok(SandboxPresetActivationResult {
                preset_id: preset.id.clone(),
                preset_name: preset.title.clone(),
                title: preset.title.clone(),
                success: false,
                error_code: Some("save_write_failed".to_string()),
                written_mods: mods_to_write.clone(),
                verified_mods: Vec::new(),
                written_mod_refs: Vec::new(),
                backup_path,
                cache_path: Some(sandbox_cache_path.display().to_string()),
                mod_cache_path: mod_cache_path.clone(),
                save_path: Some(game_sii.display().to_string()),
                message: format!("Speichern fehlgeschlagen: {}", error),
                progress_log,
                app_id: preset.app_id.unwrap_or(227300),
                target_profile_sii_path: game_sii.display().to_string(),
                ..Default::default()
            });
        }
    };
    decrypt_cache.invalidate_path(&game_sii);
    profile_cache.invalidate_save_data();
    profile_cache.invalidate_vehicle_data();
    progress_log.push("actived_mods geschrieben".to_string());

    let reread_content = match decrypt_if_needed(&game_sii) {
        Ok(content) => content,
        Err(error) => {
            return Ok(SandboxPresetActivationResult {
                preset_id: preset.id.clone(),
                preset_name: preset.title.clone(),
                title: preset.title.clone(),
                success: false,
                error_code: Some("save_reread_failed".to_string()),
                written_mods: mods_to_write.clone(),
                verified_mods: Vec::new(),
                written_mod_refs: replace_result.expected_mod_refs.clone(),
                backup_path,
                cache_path: Some(sandbox_cache_path.display().to_string()),
                mod_cache_path: mod_cache_path.clone(),
                save_path: Some(game_sii.display().to_string()),
                message: format!(
                    "Die Datei wurde geschrieben, aber konnte nicht erneut gelesen werden. {}",
                    error
                ),
                progress_log,
                app_id: preset.app_id.unwrap_or(227300),
                target_profile_sii_path: game_sii.display().to_string(),
                ..Default::default()
            });
        }
    };

    let validation = match sii_mods::validate_active_mod_values_in_game_sii(
        &reread_content,
        &mods_to_write,
    ) {
        Ok(validation) => validation,
        Err(error) => {
            return Ok(SandboxPresetActivationResult {
                preset_id: preset.id.clone(),
                preset_name: preset.title.clone(),
                title: preset.title.clone(),
                success: false,
                error_code: Some("verification_failed".to_string()),
                written_mods: mods_to_write.clone(),
                verified_mods: Vec::new(),
                written_mod_refs: replace_result.expected_mod_refs.clone(),
                backup_path,
                cache_path: Some(sandbox_cache_path.display().to_string()),
                mod_cache_path: mod_cache_path.clone(),
                save_path: Some(game_sii.display().to_string()),
                message: format!(
                    "Die Datei wurde geschrieben, aber die Mod-ID konnte nach dem erneuten Auslesen nicht korrekt bestätigt werden. {}",
                    error
                ),
                progress_log,
                app_id: preset.app_id.unwrap_or(227300),
                target_profile_sii_path: game_sii.display().to_string(),
                ..Default::default()
            });
        }
    };

    if !validation.success {
        return Ok(SandboxPresetActivationResult {
            preset_id: preset.id.clone(),
            preset_name: preset.title.clone(),
            title: preset.title.clone(),
            success: false,
            error_code: Some("verification_failed".to_string()),
            written_mods: mods_to_write.clone(),
            verified_mods: validation.actual_mod_refs.clone(),
            written_mod_refs: replace_result.expected_mod_refs.clone(),
            backup_path,
            cache_path: Some(sandbox_cache_path.display().to_string()),
            mod_cache_path: mod_cache_path.clone(),
            save_path: Some(game_sii.display().to_string()),
            message: "Die Datei wurde geschrieben, aber die Mod-ID konnte nach dem erneuten Auslesen nicht korrekt bestätigt werden.".to_string(),
            progress_log,
            app_id: preset.app_id.unwrap_or(227300),
            target_profile_sii_path: game_sii.display().to_string(),
            ..Default::default()
        });
    }

    progress_log.push("Follow-up Check erfolgreich".to_string());
    let optional_missing = statuses
        .iter()
        .filter(|status| !status.required && !(status.available && status.reachable))
        .cloned()
        .collect::<Vec<_>>();
    let base_success_message = if replace_result.block_created {
        "Preset activated successfully. active_mods block was created.".to_string()
    } else {
        "Preset activated successfully. active_mods block was updated.".to_string()
    };
    let success_message = optional_missing_mods_summary(&statuses)
        .map(|warning| format!("{} {}", base_success_message, warning))
        .unwrap_or(base_success_message);
    Ok(SandboxPresetActivationResult {
        preset_id: preset.id.clone(),
        preset_name: preset.title.clone(),
        title: preset.title.clone(),
        success: true,
        error_code: None,
        written_mods: mods_to_write.clone(),
        verified_mods: validation.actual_mod_refs.clone(),
        written_mod_refs: replace_result.expected_mod_refs,
        backup_path,
        cache_path: Some(sandbox_cache_path.display().to_string()),
        mod_cache_path,
        save_path: Some(game_sii.display().to_string()),
        app_id: preset.app_id.unwrap_or(227300),
        target_profile_sii_path: game_sii.display().to_string(),
        mods_written,
        missing_mods: activation_missing_entries(&optional_missing),
        message: success_message,
        progress_log,
        ..Default::default()
    })
}

pub fn activate_sandbox_mod_preset_profile_sii(
    app: &AppHandle,
    profile_state: &AppProfileState,
    profile_cache: &ProfileCache,
    decrypt_cache: &DecryptCache,
    preset_id: &str,
    profile_id: Option<String>,
    save_name: Option<String>,
    game: Option<String>,
    app_id: Option<u32>,
) -> Result<SandboxPresetActivationResult, String> {
    crate::dev_log!(
        "[SandboxPreset] activate START profile_id={} preset_id={}",
        profile_id.as_deref().unwrap_or(""),
        preset_id
    );
    let preset = match find_sandbox_preset(preset_id) {
        Ok(preset) => preset,
        Err(error) => {
            return Ok(profile_activation_failure_result(
                preset_id,
                "Preset not found",
                "preset_not_found",
                error,
                Vec::new(),
                None,
                None,
                profile_id.unwrap_or_default(),
                save_name,
                app_id.unwrap_or(227300),
                String::new(),
                Vec::new(),
            ));
        }
    };
    crate::dev_log!(
        "[mod_profile_manager] loaded preset id={} title={} mods={}",
        preset.id,
        preset.title,
        preset.mods.len()
    );

    let resolved_app_id = app_id
        .or(preset.app_id)
        .or_else(|| preset.mods.first().map(|preset_mod| preset_mod.app_id))
        .unwrap_or(227300);
    let resolved_game = game
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(|value| value.trim().to_ascii_lowercase())
        .unwrap_or_else(|| game_key_from_app_id(resolved_app_id).to_string());
    let mut progress_log = vec![
        "Preset geladen".to_string(),
        format!("Loading preset {}.", preset.id),
    ];

    if preset.mods.is_empty() {
        return Ok(profile_activation_failure_result(
            &preset.id,
            &preset.title,
            "no_mods_to_write",
            "Preset has no mods configured and cannot be activated.".to_string(),
            progress_log,
            None,
            None,
            profile_id.unwrap_or_default(),
            save_name,
            resolved_app_id,
            String::new(),
            Vec::new(),
        ));
    }

    let statuses = collect_preset_mod_statuses(app, &preset, &mut progress_log)?;
    let checked_at = chrono::Local::now().to_rfc3339();
    let mod_cache_path = write_mod_cache_entry(
        app,
        SandboxModCacheEntry {
            preset_id: preset.id.clone(),
            title: preset.title.clone(),
            checked_at,
            checked_libraries: collect_checked_libraries(&statuses),
            mods: statuses.clone(),
        },
    )
    .map(|path| path.display().to_string())
    .ok();
    progress_log.push("Mods geprÃ¼ft".to_string());

    let required_missing = statuses
        .iter()
        .filter(|status| status.required && !(status.available && status.reachable))
        .cloned()
        .collect::<Vec<_>>();
    crate::dev_log!(
        "[mod_profile_manager] validation result missing_required={} total={}",
        required_missing.len(),
        statuses.len()
    );
    if !required_missing.is_empty() {
        return Ok(profile_activation_failure_result(
            &preset.id,
            &preset.title,
            "mod_not_found",
            missing_mods_summary(&required_missing),
            progress_log,
            mod_cache_path,
            None,
            profile_id.unwrap_or_default(),
            save_name,
            resolved_app_id,
            String::new(),
            activation_missing_entries(&required_missing),
        ));
    }

    let selection = snapshot_active_save_selection(profile_state)?;
    let active_profile_path = match selection.profile_path.as_deref() {
        Some(value) if !value.trim().is_empty() => value.to_string(),
        _ => {
            return Ok(profile_activation_failure_result(
                &preset.id,
                &preset.title,
                "no_active_profile",
                "Please select an active profile before activating a mod preset.".to_string(),
                progress_log,
                mod_cache_path,
                None,
                profile_id.unwrap_or_default(),
                save_name,
                resolved_app_id,
                String::new(),
                Vec::new(),
            ));
        }
    };
    if is_game_process_running_for_app_id(resolved_app_id) {
        return Ok(profile_activation_failure_result(
            &preset.id,
            &preset.title,
            "game_running",
            "Please close ETS2/ATS before activating a mod preset.".to_string(),
            progress_log,
            mod_cache_path,
            None,
            profile_id.unwrap_or_default(),
            save_name,
            resolved_app_id,
            String::new(),
            Vec::new(),
        ));
    }

    progress_log.push("Profil geÃ¶ffnet".to_string());
    let resolved_profile_id = profile_id
        .filter(|value| !value.trim().is_empty())
        .or_else(|| derive_profile_id_from_path(&active_profile_path))
        .unwrap_or_default();
    let resolved_save_name = save_name.filter(|value| !value.trim().is_empty());
    let profile_sii = match resolve_activation_profile_sii(
        profile_state,
        &resolved_profile_id,
        &resolved_game,
        resolved_app_id,
    ) {
        Ok(path) => path,
        Err(error) => {
            return Ok(profile_activation_failure_result(
                &preset.id,
                &preset.title,
                "profile_sii_not_found",
                error,
                progress_log,
                mod_cache_path,
                None,
                resolved_profile_id,
                resolved_save_name,
                resolved_app_id,
                String::new(),
                Vec::new(),
            ));
        }
    };
    crate::dev_log!(
        "[SandboxPreset] profile_dir={}",
        profile_sii
            .parent()
            .map(|path| path.display().to_string())
            .unwrap_or_default()
    );
    crate::dev_log!(
        "[SandboxPreset] target profile_sii={}",
        profile_sii.display()
    );
    crate::dev_log!(
        "[SandboxPreset] profile_sii exists={}",
        profile_sii.exists()
    );

    let backup_path = match backup_profile_sii(&profile_sii) {
        Ok(path) => path,
        Err(error) => {
            return Ok(profile_activation_failure_result(
                &preset.id,
                &preset.title,
                "backup_failed",
                format!("Backup failed: {}", error),
                progress_log,
                mod_cache_path,
                None,
                resolved_profile_id,
                resolved_save_name,
                resolved_app_id,
                profile_sii.display().to_string(),
                Vec::new(),
            ));
        }
    };
    crate::dev_log!(
        "[SandboxPreset] backup created path={}",
        backup_path.display()
    );
    let backup_path_string = backup_path.display().to_string();
    progress_log.push("Backup erstellt".to_string());

    crate::dev_log!("[SandboxPreset] decrypt profile_sii START");
    let profile_text = match decrypt_if_needed(&profile_sii) {
        Ok(content) => content,
        Err(error) => {
            return Ok(profile_activation_failure_result(
                &preset.id,
                &preset.title,
                "profile_read_failed",
                format!("Could not read or decrypt profile.sii: {}", error),
                progress_log,
                mod_cache_path,
                Some(backup_path_string),
                resolved_profile_id,
                resolved_save_name,
                resolved_app_id,
                profile_sii.display().to_string(),
                Vec::new(),
            ));
        }
    };
    crate::dev_log!("[SandboxPreset] decrypt profile_sii END");
    crate::dev_log!(
        "[SandboxPreset] active_mods block found in profile_sii={}",
        sii_mods::parse_active_mod_values_from_profile_text(&profile_text).is_ok()
    );
    let active_before =
        sii_mods::parse_active_mod_values_from_profile_text(&profile_text).unwrap_or_default();
    crate::dev_log!(
        "[mod_profile_manager] active_mods before count={} values={:?}",
        active_before.len(),
        active_before
    );
    progress_log.push("actived_mods gelesen".to_string());

    let mods_written = match build_activated_mod_entries(&statuses) {
        Ok(entries) => entries,
        Err(error) => {
            return Ok(profile_activation_failure_result(
                &preset.id,
                &preset.title,
                "invalid_mod_identifier",
                error,
                progress_log,
                mod_cache_path,
                Some(backup_path_string),
                resolved_profile_id,
                resolved_save_name,
                resolved_app_id,
                profile_sii.display().to_string(),
                Vec::new(),
            ));
        }
    };
    if mods_written.is_empty() {
        return Ok(profile_activation_failure_result(
            &preset.id,
            &preset.title,
            "no_mods_to_write",
            "No locally available preset mods were found to write.".to_string(),
            progress_log,
            mod_cache_path,
            Some(backup_path_string),
            resolved_profile_id,
            resolved_save_name,
            resolved_app_id,
            profile_sii.display().to_string(),
            Vec::new(),
        ));
    }
    let expected_values = mods_written
        .iter()
        .map(|entry| entry.active_mods_value.clone())
        .collect::<Vec<_>>();
    crate::dev_log!(
        "[SandboxPreset] writing active_mods count={}",
        expected_values.len()
    );

    let updated_profile_text =
        match sii_mods::replace_active_mods_block(&profile_text, &expected_values) {
            Ok(content) => content,
            Err(error) => {
                return Ok(profile_activation_failure_result(
                    &preset.id,
                    &preset.title,
                    "active_mods_missing",
                    error,
                    progress_log,
                    mod_cache_path,
                    Some(backup_path_string),
                    resolved_profile_id,
                    resolved_save_name,
                    resolved_app_id,
                    profile_sii.display().to_string(),
                    Vec::new(),
                ));
            }
        };

    if let Err(error) = sii_mods::write_text_flush_sync(&profile_sii, &updated_profile_text) {
        return Ok(profile_activation_failure_result(
            &preset.id,
            &preset.title,
            "save_write_failed",
            format!("Failed to write profile.sii: {}", error),
            progress_log,
            mod_cache_path,
            Some(backup_path_string),
            resolved_profile_id,
            resolved_save_name,
            resolved_app_id,
            profile_sii.display().to_string(),
            Vec::new(),
        ));
    }
    crate::dev_log!(
        "[SandboxPreset] profile_sii write success bytes={}",
        updated_profile_text.len()
    );
    decrypt_cache.invalidate_path(&profile_sii);
    profile_cache.invalidate_save_data();
    profile_cache.invalidate_vehicle_data();
    progress_log.push("actived_mods geschrieben".to_string());

    let reread_content = match decrypt_if_needed(&profile_sii) {
        Ok(content) => content,
        Err(error) => {
            return Ok(profile_activation_failure_result(
                &preset.id,
                &preset.title,
                "save_reread_failed",
                format!(
                    "profile.sii was written, but could not be read again. {}",
                    error
                ),
                progress_log,
                mod_cache_path,
                Some(backup_path_string),
                resolved_profile_id,
                resolved_save_name,
                resolved_app_id,
                profile_sii.display().to_string(),
                Vec::new(),
            ));
        }
    };

    let verification =
        match sii_mods::validate_active_mods_in_profile_text(&reread_content, &expected_values) {
            Ok(verification) => verification,
            Err(error) => {
                return Ok(profile_activation_failure_result(
                    &preset.id,
                    &preset.title,
                    "verification_failed",
                    format!(
                        "profile.sii was written, but active_mods could not be verified. {}",
                        error
                    ),
                    progress_log,
                    mod_cache_path,
                    Some(backup_path_string),
                    resolved_profile_id,
                    resolved_save_name,
                    resolved_app_id,
                    profile_sii.display().to_string(),
                    Vec::new(),
                ));
            }
        };
    let active_after =
        sii_mods::parse_active_mod_values_from_profile_text(&reread_content).unwrap_or_default();
    crate::dev_log!(
        "[mod_profile_manager] active_mods after count={} values={:?}",
        active_after.len(),
        active_after
    );
    crate::dev_log!(
        "[mod_profile_manager] verification result expected_count={} actual_count={} order_matches={} values_match={}",
        verification.expected_count,
        verification.actual_count,
        verification.order_matches,
        verification.values_match
    );
    if !(verification.expected_count == verification.actual_count
        && verification.order_matches
        && verification.values_match)
    {
        return Ok(SandboxPresetActivationResult {
            preset_id: preset.id.clone(),
            preset_name: preset.title.clone(),
            title: preset.title.clone(),
            success: false,
            error_code: Some("verification_failed".to_string()),
            written_mods: expected_values.clone(),
            verified_mods: active_after,
            written_mod_refs: expected_values.clone(),
            backup_path: Some(backup_path_string),
            cache_path: None,
            mod_cache_path,
            save_path: Some(profile_sii.display().to_string()),
            profile_id: resolved_profile_id,
            save_name: resolved_save_name,
            app_id: resolved_app_id,
            target_profile_sii_path: profile_sii.display().to_string(),
            mods_written,
            missing_mods: Vec::new(),
            verification,
            message: "profile.sii was written, but active_mods verification failed.".to_string(),
            progress_log,
        });
    }
    crate::dev_log!(
        "[SandboxPreset] validation success active_mods_count={}",
        expected_values.len()
    );

    progress_log.push("Follow-up Check erfolgreich".to_string());
    let result = SandboxPresetActivationResult {
        preset_id: preset.id.clone(),
        preset_name: preset.title.clone(),
        title: preset.title.clone(),
        success: true,
        error_code: None,
        written_mods: expected_values.clone(),
        verified_mods: active_after,
        written_mod_refs: expected_values,
        backup_path: Some(backup_path_string),
        cache_path: None,
        mod_cache_path,
        save_path: Some(profile_sii.display().to_string()),
        profile_id: resolved_profile_id,
        save_name: resolved_save_name,
        app_id: resolved_app_id,
        target_profile_sii_path: profile_sii.display().to_string(),
        mods_written,
        missing_mods: Vec::new(),
        verification,
        message: "Preset activated successfully.".to_string(),
        progress_log,
    };
    let _ = write_activation_operation_log(app, &result);
    Ok(result)
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

        let install_status = workshop_api::check_workshop_mod_installed(
            &workshop_mod.id.to_string(),
            &workshop_mod.app_id.to_string(),
        )?;
        if install_status.installed {
            if let Some(workshop_path) = install_status.workshop_path {
                println!(
                    "[mod-profile-manager] apply installed mod_id={} path={}",
                    workshop_mod.id, workshop_path
                );
                applied_mods.push(AppliedWorkshopMod {
                    mod_id: workshop_mod.id.to_string(),
                    title: Some(workshop_mod.name.clone()),
                    workshop_path,
                });
            }
        } else {
            let reason = install_status.reason.as_deref().unwrap_or("not_installed");
            skipped_mods.push(skipped_mod(workshop_mod, reason));
            println!(
                "[mod-profile-manager] apply skipped mod_id={} reason={} checked_paths={:?}",
                workshop_mod.id, reason, install_status.checked_paths
            );
        }
    }

    Ok((applied_mods, skipped_mods))
}

fn skipped_mod(workshop_mod: &WorkshopMod, reason: &str) -> SkippedWorkshopMod {
    SkippedWorkshopMod {
        mod_id: workshop_mod.id.to_string(),
        title: Some(workshop_mod.name.clone()),
        reason: reason.to_string(),
    }
}

fn validation_failure_message(validation: &ValidateActivePresetModsResult) -> String {
    if validation.expected_count != validation.actual_count {
        return "Mod-Count stimmt nicht.".to_string();
    }
    if !validation.order_matches {
        return "Mod-Reihenfolge stimmt nicht.".to_string();
    }
    if !validation.missing_mod_refs.is_empty() {
        return "Nicht alle erwarteten Mods wurden gefunden.".to_string();
    }
    if !validation.unexpected_mod_refs.is_empty() {
        return "Unerwartete alte Mods sind noch aktiv.".to_string();
    }
    "Preset wurde geschrieben, aber die Validierung ist fehlgeschlagen.".to_string()
}

fn fallback_validation_result(
    expected_workshop_mod_ids: &[String],
    expected_mod_refs: &[String],
) -> ValidateActivePresetModsResult {
    ValidateActivePresetModsResult {
        success: false,
        expected_count: expected_workshop_mod_ids.len(),
        actual_count: 0,
        expected_mod_refs: expected_mod_refs.to_vec(),
        actual_mod_refs: Vec::new(),
        missing_mod_refs: expected_mod_refs.to_vec(),
        unexpected_mod_refs: Vec::new(),
        order_matches: false,
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

fn find_sandbox_preset(preset_id: &str) -> Result<SandboxModPreset, String> {
    load_sandbox_mod_presets()?
        .into_iter()
        .find(|preset| preset.id == preset_id)
        .ok_or_else(|| format!("Sandbox preset not found: {}", preset_id))
}

fn collect_preset_mod_statuses(
    app: &AppHandle,
    preset: &SandboxModPreset,
    progress_log: &mut Vec<String>,
) -> Result<Vec<SandboxPresetModStatus>, String> {
    let cache = ensure_steam_workshop_cache(app)?;
    let mut ordered_mods = preset.mods.iter().collect::<Vec<_>>();
    ordered_mods.sort_by_key(|preset_mod| preset_mod.load_order);

    ordered_mods
        .into_iter()
        .map(|preset_mod| {
            let source =
                preset_mod
                    .source
                    .as_deref()
                    .unwrap_or(if preset_mod.steam_id.trim().is_empty() {
                        "local"
                    } else {
                        "workshop"
                    });
            if source.eq_ignore_ascii_case("local") {
                let package_id = preset_mod
                    .package_id
                    .as_deref()
                    .or(preset_mod.active_mod_ref.as_deref())
                    .or(preset_mod.local_mod_id.as_deref())
                    .filter(|value| !value.trim().is_empty())
                    .ok_or_else(|| {
                        format!(
                            "Local preset mod '{}' is missing package_id.",
                            preset_mod
                                .display_name
                                .as_deref()
                                .unwrap_or("unknown local mod")
                        )
                    })?;
                let active_mods_value = preset_mod
                    .active_mods_value
                    .as_deref()
                    .filter(|value| !value.trim().is_empty())
                    .ok_or_else(|| {
                        format!(
                            "Local preset mod '{}' is missing active_mods_value.",
                            preset_mod.display_name.as_deref().unwrap_or(package_id)
                        )
                    })?;
                let local_mod_id = preset_mod
                    .local_mod_id
                    .as_deref()
                    .or(preset_mod.active_mod_ref.as_deref())
                    .or(Some(package_id))
                    .unwrap_or(package_id);
                let local_check = check_local_preset_mod(
                    game_key_from_app_id(preset_mod.app_id),
                    &[local_mod_id, package_id],
                );
                crate::dev_log!(
                    "[SandboxPreset] Local mod configured package_id={} found={} display_name={}",
                    package_id,
                    local_check.found,
                    preset_mod.display_name.as_deref().unwrap_or("-")
                );
                progress_log.push(format!(
                    "Local mod configured: {}.",
                    preset_mod.display_name.as_deref().unwrap_or(package_id)
                ));
                return Ok(SandboxPresetModStatus {
                    steam_id: String::new(),
                    source: Some("local".to_string()),
                    package_id: Some(package_id.to_string()),
                    active_mod_ref: Some(package_id.to_string()),
                    local_mod_id: Some(local_mod_id.to_string()),
                    active_mods_value: Some(active_mods_value.to_string()),
                    app_id: preset_mod.app_id,
                    game: workshop_api::game_name_for_app_id(preset_mod.app_id).to_string(),
                    display_name: preset_mod.display_name.clone(),
                    required: preset_mod.required,
                    load_order: preset_mod.load_order,
                    found: local_check.found,
                    available: local_check.found,
                    reachable: local_check.found,
                    status: if local_check.found {
                        "local_found".to_string()
                    } else {
                        "local_missing".to_string()
                    },
                    local_path: local_check.local_path,
                    workshop_url: String::new(),
                    download_url: String::new(),
                    steam_protocol_url: String::new(),
                    steamcmd_command: String::new(),
                    checked_paths: local_check.checked_paths,
                    reason: local_check.reason,
                    note: preset_mod.note.clone(),
                });
            }
            crate::dev_log!(
                "[SandboxPreset] Checking Steam ID={} app_id={}",
                preset_mod.steam_id,
                preset_mod.app_id
            );
            progress_log.push(format!(
                "Checking Steam Workshop folder for {} (AppID {})...",
                preset_mod.steam_id, preset_mod.app_id
            ));
            let cache_status =
                resolve_workshop_mod_from_cache(&cache, preset_mod.app_id, &preset_mod.steam_id)?;
            let install_status = workshop_api::check_workshop_mod_installed(
                &preset_mod.steam_id,
                &preset_mod.app_id.to_string(),
            )?;
            for checked_path in &install_status.checked_paths {
                crate::dev_log!("[SandboxPreset] Checking path: {}", checked_path);
                progress_log.push(format!("Checked path: {}", checked_path));
            }
            if cache_status.available {
                if !cache_status.local_path.is_empty() {
                    crate::dev_log!(
                        "[SandboxPreset] Found mod {} for app_id={} at {}",
                        preset_mod.steam_id,
                        preset_mod.app_id,
                        cache_status.local_path
                    );
                    progress_log.push(format!(
                        "Found mod {} at {}.",
                        preset_mod.steam_id, cache_status.local_path
                    ));
                }
            } else {
                crate::dev_log!(
                    "[SandboxPreset] Mod {} app_id={} was not found. reason={}",
                    preset_mod.steam_id,
                    preset_mod.app_id,
                    install_status.reason.as_deref().unwrap_or("unknown")
                );
                progress_log.push(format!(
                    "Mod {} was not found. reason={}",
                    preset_mod.steam_id,
                    install_status.reason.as_deref().unwrap_or("unknown")
                ));
            }
            Ok(SandboxPresetModStatus {
                steam_id: preset_mod.steam_id.clone(),
                source: Some("workshop".to_string()),
                package_id: preset_mod.package_id.clone(),
                active_mod_ref: preset_mod
                    .active_mod_ref
                    .clone()
                    .or_else(|| preset_mod.package_id.clone()),
                local_mod_id: preset_mod.local_mod_id.clone(),
                active_mods_value: preset_mod.active_mods_value.clone(),
                app_id: preset_mod.app_id,
                game: workshop_api::game_name_for_app_id(preset_mod.app_id).to_string(),
                display_name: preset_mod.display_name.clone(),
                required: preset_mod.required,
                load_order: preset_mod.load_order,
                found: cache_status.available,
                available: cache_status.available,
                reachable: cache_status.reachable,
                status: sandbox_mod_status_value(&cache_status, install_status.reason.as_deref()),
                local_path: if cache_status.local_path.is_empty() {
                    None
                } else {
                    Some(cache_status.local_path.clone())
                },
                workshop_url: preset_mod
                    .workshop_url
                    .clone()
                    .unwrap_or_else(|| workshop_api::workshop_page_url(&preset_mod.steam_id)),
                download_url: preset_mod.download_url.clone().unwrap_or_else(|| {
                    preset_mod
                        .workshop_url
                        .clone()
                        .unwrap_or_else(|| workshop_api::workshop_page_url(&preset_mod.steam_id))
                }),
                steam_protocol_url: preset_mod
                    .steam_protocol_url
                    .clone()
                    .unwrap_or_else(|| workshop_api::steam_protocol_url(&preset_mod.steam_id)),
                steamcmd_command: workshop_api::steamcmd_download_command(
                    preset_mod.app_id,
                    &preset_mod.steam_id,
                ),
                checked_paths: install_status.checked_paths,
                reason: install_status.reason,
                note: preset_mod.note.clone(),
            })
        })
        .collect()
}

#[derive(Debug, Clone, Default)]
struct LocalPresetModCheck {
    found: bool,
    local_path: Option<String>,
    checked_paths: Vec<String>,
    reason: Option<String>,
}

fn check_local_preset_mod(game: &str, identifiers: &[&str]) -> LocalPresetModCheck {
    let identifiers = identifiers
        .iter()
        .map(|value| normalize_local_mod_identifier(value))
        .filter(|value| !value.is_empty())
        .collect::<BTreeSet<_>>();
    if identifiers.is_empty() {
        return LocalPresetModCheck {
            reason: Some("local_mod_identifier_missing".to_string()),
            ..Default::default()
        };
    }

    let Some(mod_dir) = mod_directory_path(game) else {
        return LocalPresetModCheck {
            reason: Some("local_mod_folder_not_found".to_string()),
            ..Default::default()
        };
    };
    let mut checked_paths = identifiers
        .iter()
        .map(|identifier| {
            mod_dir
                .join(format!("{identifier}.scs"))
                .display()
                .to_string()
        })
        .collect::<Vec<_>>();
    checked_paths.push(mod_dir.display().to_string());

    if !mod_dir.is_dir() {
        return LocalPresetModCheck {
            checked_paths,
            reason: Some("local_mod_folder_not_found".to_string()),
            ..Default::default()
        };
    }

    let Ok(entries) = fs::read_dir(&mod_dir) else {
        return LocalPresetModCheck {
            checked_paths,
            reason: Some("local_mod_folder_unreadable".to_string()),
            ..Default::default()
        };
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let file_name = path
            .file_name()
            .and_then(|value| value.to_str())
            .map(normalize_local_mod_identifier)
            .unwrap_or_default();
        let file_stem = path
            .file_stem()
            .and_then(|value| value.to_str())
            .map(normalize_local_mod_identifier)
            .unwrap_or_default();
        if identifiers.contains(&file_name) || identifiers.contains(&file_stem) {
            return LocalPresetModCheck {
                found: true,
                local_path: Some(path.display().to_string()),
                checked_paths,
                reason: None,
            };
        }
    }

    LocalPresetModCheck {
        checked_paths,
        reason: Some("local_mod_not_found".to_string()),
        ..Default::default()
    }
}

fn normalize_local_mod_identifier(value: &str) -> String {
    value
        .trim()
        .trim_end_matches(".scs")
        .trim()
        .to_ascii_lowercase()
}

fn collect_checked_libraries(statuses: &[SandboxPresetModStatus]) -> Vec<String> {
    let mut libraries = BTreeSet::new();
    for status in statuses {
        for checked_path in &status.checked_paths {
            let candidate = Path::new(checked_path)
                .ancestors()
                .nth(5)
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| checked_path.to_string());
            libraries.insert(candidate);
        }
    }
    libraries.into_iter().collect()
}

fn split_mod_statuses(
    statuses: &[SandboxPresetModStatus],
) -> (Vec<SandboxPresetModStatus>, Vec<SandboxPresetModStatus>) {
    let mut found_mods = Vec::new();
    let mut missing_mods = Vec::new();

    for status in statuses {
        if status.found {
            found_mods.push(status.clone());
        } else {
            missing_mods.push(status.clone());
        }
    }

    (found_mods, missing_mods)
}

fn mod_cache_path(app: &AppHandle) -> Result<PathBuf, String> {
    storage_dir(app).map(|dir| dir.join(MOD_SANDBOX_CACHE_FILE_NAME))
}

fn steam_workshop_cache_path(app: &AppHandle) -> Result<PathBuf, String> {
    storage_dir(app).map(|dir| dir.join(STEAM_WORKSHOP_CACHE_FILE_NAME))
}

fn sandbox_test_cache_path(app: &AppHandle) -> Result<PathBuf, String> {
    storage_dir(app).map(|dir| dir.join(SANDBOX_TEST_CACHE_FILE_NAME))
}

fn write_steam_workshop_cache(
    app: &AppHandle,
    cache: &SteamWorkshopCache,
) -> Result<PathBuf, String> {
    let path = steam_workshop_cache_path(app)?;
    let body = serde_json::to_string_pretty(cache)
        .map_err(|error| format!("Failed to serialize Steam Workshop cache: {}", error))?;
    fs::write(&path, body).map_err(|error| {
        format!(
            "Failed to write Steam Workshop cache {}: {}",
            path.display(),
            error
        )
    })?;
    crate::dev_log!(
        "[SteamWorkshopCache] cache written path={} mods={}",
        path.display(),
        cache.mods.len()
    );
    Ok(path)
}

fn write_mod_cache_entry(app: &AppHandle, entry: SandboxModCacheEntry) -> Result<PathBuf, String> {
    let path = mod_cache_path(app)?;
    let mut file = if path.is_file() {
        let content = fs::read_to_string(&path)
            .map_err(|error| format!("Failed to read {}: {}", path.display(), error))?;
        match serde_json::from_str::<SandboxModCacheFile>(&content) {
            Ok(parsed) => parsed,
            Err(error) => {
                crate::dev_log!(
                    "[SandboxPreset] invalid mod cache path={} error={} resetting file",
                    path.display(),
                    error
                );
                SandboxModCacheFile::default()
            }
        }
    } else {
        SandboxModCacheFile::default()
    };

    file.entries
        .retain(|item| item.preset_id != entry.preset_id);
    file.entries.push(entry);
    let body = serde_json::to_string_pretty(&file)
        .map_err(|error| format!("Failed to serialize mod cache: {}", error))?;
    fs::write(&path, body)
        .map_err(|error| format!("Failed to write {}: {}", path.display(), error))?;
    Ok(path)
}

fn write_sandbox_test_cache_entry(
    app: &AppHandle,
    entry: SandboxActiveModsBackupCacheEntry,
) -> Result<PathBuf, String> {
    let path = sandbox_test_cache_path(app)?;
    let mut file = if path.is_file() {
        let content = fs::read_to_string(&path)
            .map_err(|error| format!("Failed to read {}: {}", path.display(), error))?;
        match serde_json::from_str::<SandboxActiveModsBackupCacheFile>(&content) {
            Ok(parsed) => parsed,
            Err(error) => {
                crate::dev_log!(
                    "[SandboxPreset] invalid sandbox backup cache path={} error={} resetting file",
                    path.display(),
                    error
                );
                SandboxActiveModsBackupCacheFile::default()
            }
        }
    } else {
        SandboxActiveModsBackupCacheFile::default()
    };

    file.entries
        .retain(|item| !(item.preset_id == entry.preset_id && item.file_path == entry.file_path));
    file.entries.push(entry);
    let body = serde_json::to_string_pretty(&file)
        .map_err(|error| format!("Failed to serialize sandbox test cache: {}", error))?;
    fs::write(&path, body)
        .map_err(|error| format!("Failed to write {}: {}", path.display(), error))?;
    Ok(path)
}

fn profile_activation_failure_result(
    preset_id: &str,
    title: &str,
    error_code: &str,
    message: String,
    progress_log: Vec<String>,
    mod_cache_path: Option<String>,
    backup_path: Option<String>,
    profile_id: String,
    save_name: Option<String>,
    app_id: u32,
    target_profile_sii_path: String,
    missing_mods: Vec<ActivationMissingModEntry>,
) -> SandboxPresetActivationResult {
    SandboxPresetActivationResult {
        preset_id: preset_id.to_string(),
        preset_name: title.to_string(),
        title: title.to_string(),
        success: false,
        error_code: Some(error_code.to_string()),
        written_mods: Vec::new(),
        verified_mods: Vec::new(),
        written_mod_refs: Vec::new(),
        backup_path,
        cache_path: None,
        mod_cache_path,
        save_path: if target_profile_sii_path.is_empty() {
            None
        } else {
            Some(target_profile_sii_path.clone())
        },
        profile_id,
        save_name,
        app_id,
        target_profile_sii_path,
        mods_written: Vec::new(),
        missing_mods,
        verification: ActivationVerification::default(),
        message,
        progress_log,
    }
}

fn activation_missing_entries(
    statuses: &[SandboxPresetModStatus],
) -> Vec<ActivationMissingModEntry> {
    statuses
        .iter()
        .map(|status| ActivationMissingModEntry {
            workshop_id: status.steam_id.clone(),
            active_mod_ref: status.active_mod_ref.clone(),
            local_mod_id: status.local_mod_id.clone(),
            app_id: status.app_id,
            display_name: status.display_name.clone(),
            workshop_url: status.workshop_url.clone(),
            download_url: status.download_url.clone(),
            required: status.required,
            reason: status.reason.clone(),
        })
        .collect()
}

fn build_activated_mod_entries(
    statuses: &[SandboxPresetModStatus],
) -> Result<Vec<ActivatedModEntry>, String> {
    let mut writable = statuses.iter().collect::<Vec<_>>();
    writable.sort_by_key(|status| status.load_order);

    writable
        .into_iter()
        .enumerate()
        .map(|(index, status)| {
            let active_mods_value = if let Some(value) = status.active_mods_value.as_deref() {
                value.to_string()
            } else {
                let package_id = status
                    .active_mod_ref
                    .clone()
                    .or_else(|| status.package_id.clone())
                    .or_else(|| {
                        if status.steam_id.trim().is_empty() {
                            status.local_mod_id.clone()
                        } else {
                            sii_mods::workshop_id_to_scs_package_id(&status.steam_id).ok()
                        }
                    })
                    .ok_or_else(|| {
                        format!(
                            "Preset mod '{}' is missing an active_mods identifier.",
                            status.display_name.as_deref().unwrap_or("unknown")
                        )
                    })?;
                let display_name = status
                    .display_name
                    .clone()
                    .unwrap_or_else(|| package_id.clone());
                let safe_display_name = sanitize_active_mod_display_name(&display_name);
                format!("{package_id}|{safe_display_name}")
            };
            let display_name = status.display_name.clone().unwrap_or_else(|| {
                status
                    .active_mod_ref
                    .clone()
                    .or_else(|| status.local_mod_id.clone())
                    .or_else(|| {
                        if status.steam_id.trim().is_empty() {
                            None
                        } else {
                            Some(format!("Workshop Mod {}", status.steam_id))
                        }
                    })
                    .or_else(|| status.package_id.clone())
                    .unwrap_or_else(|| "Preset Mod".to_string())
            });
            Ok(ActivatedModEntry {
                index,
                workshop_id: if status.steam_id.trim().is_empty() {
                    None
                } else {
                    Some(status.steam_id.clone())
                },
                app_id: Some(status.app_id),
                display_name,
                active_mods_value,
                local_path: status.local_path.clone(),
            })
        })
        .collect()
}

fn sanitize_active_mod_display_name(value: &str) -> String {
    value.replace('"', "'").replace('|', "/").trim().to_string()
}

fn derive_profile_id_from_path(path: &str) -> Option<String> {
    Path::new(path)
        .file_name()
        .and_then(|value| value.to_str())
        .map(|value| value.to_string())
}

fn derive_save_name_from_path(path: &str) -> Option<String> {
    Path::new(path)
        .file_name()
        .and_then(|value| value.to_str())
        .map(|value| value.to_string())
}

fn resolve_activation_profile_sii(
    profile_state: &AppProfileState,
    profile_id: &str,
    game: &str,
    app_id: u32,
) -> Result<PathBuf, String> {
    let selection = snapshot_active_save_selection(profile_state)?;
    if let Some(active_profile) = selection.profile_path.as_deref() {
        let active_candidate = profile_sii_from_profile_path(active_profile);
        if active_candidate.is_file() {
            return Ok(active_candidate);
        }
    }

    let candidates = collect_profile_sii_candidates(profile_id, game, app_id);
    match candidates.len() {
        0 => Err(format!(
            "profile.sii not found for profile_id={profile_id} game={game}"
        )),
        1 => Ok(candidates[0].clone()),
        _ => Err(format!(
            "Multiple profile.sii candidates found for profile_id={profile_id}. Select the active profile explicitly before activating the preset."
        )),
    }
}

fn profile_sii_from_profile_path(profile_path: &str) -> PathBuf {
    let path = PathBuf::from(profile_path);
    if path
        .file_name()
        .and_then(|value| value.to_str())
        .map(|value| value.eq_ignore_ascii_case("profile.sii"))
        .unwrap_or(false)
    {
        path
    } else {
        path.join("profile.sii")
    }
}

fn collect_profile_sii_candidates(profile_id: &str, game: &str, app_id: u32) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    let mut seen = BTreeSet::new();
    if profile_id.trim().is_empty() {
        return candidates;
    }

    if let Some(base_path) = get_base_path(game) {
        push_profile_sii_candidate(
            &mut candidates,
            &mut seen,
            base_path
                .join("profiles")
                .join(profile_id)
                .join("profile.sii"),
        );
        push_profile_sii_candidate(
            &mut candidates,
            &mut seen,
            base_path
                .join("steam_profiles")
                .join(profile_id)
                .join("profile.sii"),
        );
    }

    let app_id_string = app_id.to_string();
    let mut steam_roots = Vec::new();
    if let Some(steam_root) = steam_paths::find_steam_install_dir() {
        steam_roots.push(steam_root);
    }
    if let Ok(libraries) = steam_paths::get_steam_library_dirs() {
        steam_roots.extend(libraries);
    }

    for root in steam_roots {
        let userdata = root.join("userdata");
        let Ok(entries) = fs::read_dir(userdata) else {
            continue;
        };
        for entry in entries.flatten() {
            push_profile_sii_candidate(
                &mut candidates,
                &mut seen,
                entry
                    .path()
                    .join(&app_id_string)
                    .join("remote")
                    .join("profiles")
                    .join(profile_id)
                    .join("profile.sii"),
            );
        }
    }

    candidates
}

fn push_profile_sii_candidate(
    candidates: &mut Vec<PathBuf>,
    seen: &mut BTreeSet<String>,
    path: PathBuf,
) {
    if !path.is_file() {
        return;
    }
    let key = path.display().to_string().replace('\\', "/");
    if seen.insert(key) {
        candidates.push(path);
    }
}

fn backup_profile_sii(path: &Path) -> Result<PathBuf, String> {
    if !path.is_file() {
        return Err(format!("profile.sii not found: {}", path.display()));
    }
    let timestamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let backup_path = path.with_file_name(format!("profile.sii.sandbox-preset-{timestamp}.bak"));
    fs::copy(path, &backup_path).map_err(|error| {
        format!(
            "Failed to create backup {}: {}",
            backup_path.display(),
            error
        )
    })?;
    Ok(backup_path)
}

fn write_activation_operation_log(
    app: &AppHandle,
    result: &SandboxPresetActivationResult,
) -> Result<PathBuf, String> {
    let path = storage_dir(app)?.join("sandboxActivationLog.json");
    let mut entries = if path.is_file() {
        let content = fs::read_to_string(&path)
            .map_err(|error| format!("Failed to read {}: {}", path.display(), error))?;
        serde_json::from_str::<Vec<serde_json::Value>>(&content).unwrap_or_default()
    } else {
        Vec::new()
    };

    entries.push(serde_json::json!({
        "timestamp": chrono::Local::now().to_rfc3339(),
        "preset_id": &result.preset_id,
        "preset_name": &result.preset_name,
        "profile_id": &result.profile_id,
        "save_name": &result.save_name,
        "app_id": result.app_id,
        "target_profile_sii_path": &result.target_profile_sii_path,
        "backup_path": &result.backup_path,
        "mods_written": &result.mods_written,
        "verification_success": result.verification.order_matches && result.verification.values_match,
        "missing_mods": &result.missing_mods,
        "error_message": if result.success { serde_json::Value::Null } else { serde_json::json!(&result.message) },
    }));
    let body = serde_json::to_string_pretty(&entries)
        .map_err(|error| format!("Failed to serialize activation log: {}", error))?;
    fs::write(&path, body)
        .map_err(|error| format!("Failed to write {}: {}", path.display(), error))?;
    Ok(path)
}

fn game_key_from_app_id(app_id: u32) -> &'static str {
    match app_id {
        270880 => "ats",
        _ => "ets2",
    }
}

fn is_game_process_running_for_app_id(app_id: u32) -> bool {
    let exe_name = match app_id {
        270880 => "amtrucks.exe",
        _ => "eurotrucks2.exe",
    };
    is_process_running(exe_name)
}

#[cfg(target_os = "windows")]
fn is_process_running(exe_name: &str) -> bool {
    use std::os::windows::process::CommandExt;
    use std::process::Command;

    let filter = format!("IMAGENAME eq {}", exe_name);
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    let output = Command::new("tasklist")
        .creation_flags(CREATE_NO_WINDOW)
        .arg("/FI")
        .arg(filter)
        .arg("/NH")
        .output();

    let Ok(output) = output else {
        return false;
    };

    let stdout = String::from_utf8_lossy(&output.stdout).to_lowercase();
    stdout.contains(&exe_name.to_lowercase())
}

#[cfg(not(target_os = "windows"))]
fn is_process_running(_exe_name: &str) -> bool {
    false
}

fn failure_activation_result(
    preset_id: &str,
    title: &str,
    error_code: &str,
    message: String,
    progress_log: Vec<String>,
    mod_cache_path: Option<String>,
) -> SandboxPresetActivationResult {
    SandboxPresetActivationResult {
        preset_id: preset_id.to_string(),
        title: title.to_string(),
        success: false,
        error_code: Some(error_code.to_string()),
        written_mods: Vec::new(),
        verified_mods: Vec::new(),
        written_mod_refs: Vec::new(),
        backup_path: None,
        cache_path: None,
        mod_cache_path,
        save_path: None,
        message,
        progress_log,
        ..Default::default()
    }
}

fn sandbox_mod_status_value(cache_status: &SteamWorkshopMod, reason: Option<&str>) -> String {
    if cache_status.available {
        return "available".to_string();
    }
    if !cache_status.local_path.is_empty() && !cache_status.reachable {
        return "path_missing".to_string();
    }
    match reason.unwrap_or("unknown") {
        "workshop_folder_empty" => "downloaded".to_string(),
        "subscribed_but_content_missing" | "workshop_folder_not_found" | "not_found" => {
            "not_downloaded".to_string()
        }
        "steam_not_found" | "no_steam_libraries_found" => "unknown".to_string(),
        "invalid_mod_id" | "invalid_app_id" => "invalid".to_string(),
        _ => "unknown".to_string(),
    }
}

fn missing_mods_summary(statuses: &[SandboxPresetModStatus]) -> String {
    let required_missing = statuses
        .iter()
        .filter(|status| status.required && !(status.available && status.reachable))
        .collect::<Vec<_>>();
    if required_missing.is_empty() {
        return "All required mods found.".to_string();
    }

    let details = required_missing
        .iter()
        .map(|status| {
            if status.steam_id.trim().is_empty() {
                return format!(
                    "{}: {}",
                    status
                        .active_mod_ref
                        .as_deref()
                        .or(status.local_mod_id.as_deref())
                        .or(status.package_id.as_deref())
                        .unwrap_or("local mod"),
                    status.reason.as_deref().unwrap_or("not_found")
                );
            }
            format!(
                "{} ({} / AppID {}): {}",
                status.steam_id, status.game, status.app_id, status.workshop_url
            )
        })
        .collect::<Vec<_>>()
        .join(" | ");

    format!("Required mods are missing: {}", details)
}

fn optional_missing_mods_summary(statuses: &[SandboxPresetModStatus]) -> Option<String> {
    let optional_missing = statuses
        .iter()
        .filter(|status| !status.required && !(status.available && status.reachable))
        .collect::<Vec<_>>();
    if optional_missing.is_empty() {
        return None;
    }

    if optional_missing.len() == 1 {
        let display_name = optional_missing[0]
            .display_name
            .as_deref()
            .or(optional_missing[0].active_mod_ref.as_deref())
            .or(optional_missing[0].local_mod_id.as_deref())
            .or(optional_missing[0].package_id.as_deref())
            .unwrap_or("optional mod");
        return Some(format!(
            "Optional mod missing: {}. Preset can still be activated.",
            display_name
        ));
    }

    let display_names = optional_missing
        .iter()
        .map(|status| {
            status
                .display_name
                .as_deref()
                .or(status.active_mod_ref.as_deref())
                .or(status.local_mod_id.as_deref())
                .or(status.package_id.as_deref())
                .unwrap_or("optional mod")
                .to_string()
        })
        .collect::<Vec<_>>()
        .join(", ");
    Some(format!(
        "Optional mods missing: {}. Preset can still be activated.",
        display_names
    ))
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
