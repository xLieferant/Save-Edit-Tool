use super::models::{
    AppliedWorkshopMod, ApplySandboxResult, ModSandbox, SandboxActiveModsBackupCacheEntry,
    SandboxActiveModsBackupCacheFile, SandboxCollection, SandboxModCacheEntry,
    SandboxModCacheFile, SandboxModPreset, SandboxPresetActivationResult,
    SandboxPresetCheckResult, SandboxPresetCollection, SandboxPresetModStatus,
    SkippedWorkshopMod, ValidateActivePresetModsResult, WorkshopMod,
};
use super::sii_mods;
use super::workshop_api;
use crate::features::backup::service as backup_service;
use crate::shared::current_profile::snapshot_active_save_selection;
use crate::shared::decrypt::decrypt_if_needed;
use crate::shared::paths::game_sii_from_save;
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
        backup_path.display()
        ,
        replace_result.removed_mod_count,
        replace_result.written_mod_count,
        replace_result.expected_mod_refs
    );
    let reapplied_content = fs::read_to_string(&game_sii)
        .map_err(|error| format!("Failed to re-read {}: {}", game_sii.display(), error));
    let (validation, success, message) = match reapplied_content {
        Ok(content) => match sii_mods::validate_active_preset_mods_in_game_sii(&content, &workshop_mod_ids) {
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
                    error,
                    validation.actual_mod_refs
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
        },
        Err(error) => {
            let validation =
                fallback_validation_result(&workshop_mod_ids, &replace_result.expected_mod_refs);
            println!(
                "[mod-profile-manager] apply reread_failed error={} validation_success={}",
                error,
                validation.success
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
    let statuses = collect_preset_mod_statuses(&preset, &mut progress_log)?;
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
            progress_log.push(format!("Warning: mod cache could not be written. {}", error));
            None
        }
    };
    progress_log.push("Mods geprüft".to_string());
    let (found_mods, missing_mods) = split_mod_statuses(&statuses);
    let ready = !missing_mods.iter().any(|status| status.required);
    let checked_libraries = collect_checked_libraries(&statuses);
    let message = if ready {
        "All required mods found.".to_string()
    } else {
        "Required mods are missing.".to_string()
    };
    progress_log.push(if ready {
        "Preset is ready.".to_string()
    } else {
        "Required mods are missing.".to_string()
    });

    Ok(SandboxPresetCheckResult {
        preset_id: preset.id,
        title: preset.title,
        ready,
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

pub fn activate_sandbox_mod_preset(
    app: &AppHandle,
    profile_state: &AppProfileState,
    profile_cache: &ProfileCache,
    decrypt_cache: &DecryptCache,
    preset_id: &str,
) -> Result<SandboxPresetActivationResult, String> {
    crate::dev_log!("[SandboxPreset] Activating preset: {}", preset_id);
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
    let statuses = collect_preset_mod_statuses(&preset, &mut progress_log)?;
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
        .filter(|status| status.required && !status.found)
        .cloned()
        .collect::<Vec<_>>();
    if let Some(missing) = required_missing.first() {
        return Ok(failure_activation_result(
            &preset.id,
            &preset.title,
            "mod_not_found",
            format!(
                "Die Steam Workshop Mod-ID {} wurde lokal nicht gefunden. Bitte stelle sicher, dass die Mod abonniert und heruntergeladen ist.",
                missing.steam_id
            ),
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
    if !game_sii.is_file() {
        return Ok(failure_activation_result(
            &preset.id,
            &preset.title,
            "save_not_found",
            format!("Die aktive Save-Datei wurde nicht gefunden: {}", game_sii.display()),
            progress_log,
            mod_cache_path.clone(),
        ));
    }

    let original_state = match sii_mods::inspect_active_mod_block(&game_sii) {
        Ok(snapshot) => snapshot,
        Err(error) => {
            let error_code = if error.contains("No active_mods/actived_mods block found") {
                "actived_mods_missing"
            } else {
                "save_read_failed"
            };
            return Ok(failure_activation_result(
                &preset.id,
                &preset.title,
                error_code,
                error,
                progress_log,
                mod_cache_path.clone(),
            ));
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

    let mods_to_write = statuses
        .iter()
        .filter(|status| status.found)
        .map(|status| status.steam_id.clone())
        .collect::<Vec<_>>();
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

    let replace_result = match sii_mods::write_active_preset_mods_atomic(&game_sii, &mods_to_write) {
        Ok(result) => result,
        Err(error) => {
            return Ok(SandboxPresetActivationResult {
                preset_id: preset.id.clone(),
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
            });
        }
    };

    let validation = match sii_mods::validate_active_preset_mods_in_game_sii(
        &reread_content,
        &mods_to_write,
    ) {
        Ok(validation) => validation,
        Err(error) => {
            return Ok(SandboxPresetActivationResult {
                preset_id: preset.id.clone(),
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
            });
        }
    };

    if !validation.success {
        return Ok(SandboxPresetActivationResult {
            preset_id: preset.id.clone(),
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
        });
    }

    progress_log.push("Follow-up Check erfolgreich".to_string());
    Ok(SandboxPresetActivationResult {
        preset_id: preset.id.clone(),
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
        message: format!(
            "Sandbox Preset \"{}\" wurde erfolgreich aktiviert.",
            preset.title
        ),
        progress_log,
    })
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
                    workshop_mod.id,
                    workshop_path
                );
                applied_mods.push(AppliedWorkshopMod {
                    mod_id: workshop_mod.id.to_string(),
                    title: Some(workshop_mod.name.clone()),
                    workshop_path,
                });
            }
        } else {
            let reason = install_status
                .reason
                .as_deref()
                .unwrap_or("not_installed");
            skipped_mods.push(skipped_mod(workshop_mod, reason));
            println!(
                "[mod-profile-manager] apply skipped mod_id={} reason={} checked_paths={:?}",
                workshop_mod.id,
                reason,
                install_status.checked_paths
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
    preset: &SandboxModPreset,
    progress_log: &mut Vec<String>,
) -> Result<Vec<SandboxPresetModStatus>, String> {
    let mut ordered_mods = preset.mods.iter().collect::<Vec<_>>();
    ordered_mods.sort_by_key(|preset_mod| preset_mod.load_order);

    ordered_mods
        .into_iter()
        .map(|preset_mod| {
            crate::dev_log!("[SandboxPreset] Checking Steam ID: {}", preset_mod.steam_id);
            progress_log.push(format!(
                "Checking Steam Workshop folder for {}...",
                preset_mod.steam_id
            ));
            let install_status =
                workshop_api::check_ets2_workshop_mod_installed(&preset_mod.steam_id)?;
            for checked_path in &install_status.checked_paths {
                crate::dev_log!("[SandboxPreset] Checking path: {}", checked_path);
                progress_log.push(format!("Checked path: {}", checked_path));
            }
            if install_status.installed {
                if let Some(found_path) = install_status.workshop_path.clone() {
                    crate::dev_log!(
                        "[SandboxPreset] Found mod {} at {}",
                        preset_mod.steam_id,
                        found_path
                    );
                    progress_log.push(format!(
                        "Found mod {} at {}.",
                        preset_mod.steam_id,
                        found_path
                    ));
                }
            } else {
                crate::dev_log!(
                    "[SandboxPreset] Mod {} was not found. reason={}",
                    preset_mod.steam_id,
                    install_status.reason.as_deref().unwrap_or("unknown")
                );
                progress_log.push(format!("Mod {} was not found.", preset_mod.steam_id));
            }
            Ok(SandboxPresetModStatus {
                steam_id: preset_mod.steam_id.clone(),
                display_name: preset_mod.display_name.clone(),
                required: preset_mod.required,
                load_order: preset_mod.load_order,
                found: install_status.installed,
                local_path: install_status.workshop_path,
                workshop_url: preset_mod
                    .workshop_url
                    .clone()
                    .unwrap_or_else(|| workshop_page_url(&preset_mod.steam_id)),
                steam_protocol_url: preset_mod
                    .steam_protocol_url
                    .clone()
                    .unwrap_or_else(|| steam_protocol_url(&preset_mod.steam_id)),
                checked_paths: install_status.checked_paths,
                reason: install_status.reason,
            })
        })
        .collect()
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

fn workshop_page_url(steam_id: &str) -> String {
    format!(
        "https://steamcommunity.com/sharedfiles/filedetails/?id={}",
        steam_id.trim()
    )
}

fn steam_protocol_url(steam_id: &str) -> String {
    format!("steam://url/CommunityFilePage/{}", steam_id.trim())
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

fn sandbox_test_cache_path(app: &AppHandle) -> Result<PathBuf, String> {
    storage_dir(app).map(|dir| dir.join(SANDBOX_TEST_CACHE_FILE_NAME))
}

fn write_mod_cache_entry(
    app: &AppHandle,
    entry: SandboxModCacheEntry,
) -> Result<PathBuf, String> {
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

    file.entries.retain(|item| item.preset_id != entry.preset_id);
    file.entries.push(entry);
    let body = serde_json::to_string_pretty(&file)
        .map_err(|error| format!("Failed to serialize mod cache: {}", error))?;
    fs::write(&path, body).map_err(|error| format!("Failed to write {}: {}", path.display(), error))?;
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
    fs::write(&path, body).map_err(|error| format!("Failed to write {}: {}", path.display(), error))?;
    Ok(path)
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
    }
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
