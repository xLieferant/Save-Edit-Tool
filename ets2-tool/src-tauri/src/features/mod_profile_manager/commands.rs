use super::compare;
use super::discovery::{load_manager_state, scan_inventory, scan_inventory_with_mode, ScanMode};
use super::models::{
    ApplySandboxResult, DiscoveredMod, GameType, ModPreset, ModSandbox, PresetCompareResult,
    PresetModEntry, SandboxCollection, SandboxModPreset, SandboxPresetActivationResult,
    SandboxPresetCheckResult, SteamWorkshopCache, SteamWorkshopMod, WorkshopInstallStatus,
    WorkshopMod,
};
use super::presets;
use super::{launcher, sandbox, workshop_api};
use crate::shared::user_log;
use crate::state::{AppProfileState, DecryptCache, ProfileCache};
use std::any::Any;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, State};
use uuid::Uuid;

static MOD_SCAN_RUNNING: AtomicBool = AtomicBool::new(false);

fn panic_message(payload: Box<dyn Any + Send>) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        return (*message).to_string();
    }
    if let Some(message) = payload.downcast_ref::<String>() {
        return message.clone();
    }
    "unknown panic payload".to_string()
}

fn log_user_event(action: &str, stage: &str) {
    if let Err(error) = user_log::write_user_log(action, stage) {
        crate::dev_log!(
            "[mod-profile-manager] user log write failed action='{}' stage='{}': {}",
            action,
            stage,
            error
        );
    }
}

fn trace_log(message: &str) {
    if let Err(error) = user_log::user_log_info("ModScanner", message) {
        crate::dev_log!("[mod-profile-manager] trace user log failed: {}", error);
    }
}

struct ScanGuard;

impl ScanGuard {
    fn acquire() -> Result<Self, String> {
        if MOD_SCAN_RUNNING.swap(true, Ordering::SeqCst) {
            return Err("Mod scan already running".to_string());
        }
        Ok(Self)
    }
}

impl Drop for ScanGuard {
    fn drop(&mut self) {
        MOD_SCAN_RUNNING.store(false, Ordering::SeqCst);
    }
}

fn catch_command<T, F>(label: &str, action: F) -> Result<T, String>
where
    F: FnOnce() -> Result<T, String>,
{
    match catch_unwind(AssertUnwindSafe(action)) {
        Ok(Ok(result)) => Ok(result),
        Ok(Err(error)) => {
            crate::dev_log!(
                "[mod-profile-manager] command failed label='{}': {}",
                label,
                error
            );
            log_user_event(
                &format!("mod_profile_manager error | {} | {}", label, error),
                "error",
            );
            Err(error)
        }
        Err(payload) => {
            let message = panic_message(payload);
            crate::dev_log!(
                "[mod-profile-manager] command panic caught label='{}': {}",
                label,
                message
            );
            log_user_event(
                &format!("mod_profile_manager panic | {} | {}", label, message),
                "error",
            );
            Err("The Mod Profile Manager failed unexpectedly.".to_string())
        }
    }
}

fn include_in_estimated_preset(item: &DiscoveredMod) -> bool {
    item.readable && item.status != "invalid_workshop_item"
}

fn build_preset_mods(
    mods: &[DiscoveredMod],
    active_mods_reliably_known: bool,
) -> Vec<PresetModEntry> {
    let mut preset_mods = mods
        .iter()
        .filter(|item| {
            if active_mods_reliably_known {
                item.enabled == Some(true)
            } else {
                include_in_estimated_preset(item)
            }
        })
        .map(|item| PresetModEntry {
            mod_id: item.id.clone(),
            name: item.name.clone(),
            source: item.source.clone(),
            file_path: item.file_path.clone(),
            workshop_id: item.workshop_id.clone(),
            app_id: item.app_id.clone(),
            enabled: true,
            load_order_index: item.load_order_index.unwrap_or_default(),
        })
        .collect::<Vec<_>>();

    preset_mods.sort_by(|left, right| {
        left.load_order_index
            .cmp(&right.load_order_index)
            .then_with(|| {
                left.name
                    .to_ascii_lowercase()
                    .cmp(&right.name.to_ascii_lowercase())
            })
    });

    preset_mods
}

#[tauri::command]
pub fn load_mod_profile_manager_state(
    app: AppHandle,
    profile_state: State<'_, AppProfileState>,
    game: Option<String>,
) -> Result<super::models::ModProfileManagerState, String> {
    let started_at = std::time::Instant::now();
    crate::dev_log!("[trace] START open_mod_manager");
    trace_log("START open_mod_manager");
    crate::dev_log!("[mod-profile-manager] load state requested game={:?}", game);
    log_user_event("mod_profile_manager opened", "start");

    let result = catch_command("load_mod_profile_manager_state", || {
        let _scan_guard = ScanGuard::acquire()?;
        let state = load_manager_state(&app, profile_state.inner(), game.as_deref())?;
        crate::dev_log!(
            "[mod-profile-manager] load state completed game={} mods={} presets={} workshop_mods={} unreadable={}",
            state.summary.selected_game.as_str(),
            state.mods.len(),
            state.presets.len(),
            state.summary.steam_workshop_mods_found,
            state.summary.unreadable_mods_count
        );
        log_user_event(
            &format!(
                "mod_profile_manager success | mods={} presets={}",
                state.mods.len(),
                state.presets.len()
            ),
            "success",
        );
        Ok(state)
    });
    crate::dev_log!(
        "[trace] END open_mod_manager duration_ms={}",
        started_at.elapsed().as_millis()
    );
    trace_log(&format!(
        "END open_mod_manager duration_ms={}",
        started_at.elapsed().as_millis()
    ));
    if let Err(error) = &result {
        crate::dev_log!(
            "[trace] ERROR mod_manager command=load_mod_profile_manager_state error={}",
            error
        );
        let _ = user_log::user_log_error(
            "ModManager",
            format!(
                "ERROR mod_manager command=load_mod_profile_manager_state error={}",
                error
            ),
        );
    }
    result
}

#[tauri::command]
pub fn scan_mods(
    app: AppHandle,
    profile_state: State<'_, AppProfileState>,
    game: Option<String>,
) -> Result<Vec<DiscoveredMod>, String> {
    crate::dev_log!("[mod-profile-manager] scan requested game={:?}", game);
    log_user_event("mod_profile_manager scan", "start");

    catch_command("scan_mods", || {
        let _scan_guard = ScanGuard::acquire()?;
        let inventory = scan_inventory(&app, profile_state.inner(), game.as_deref())?;
        crate::dev_log!(
            "[mod-profile-manager] scan completed game={} local_mods={} workshop_mods={} unreadable={} steam_libraries={}",
            inventory.summary.selected_game.as_str(),
            inventory.summary.local_mods_found,
            inventory.summary.steam_workshop_mods_found,
            inventory.summary.unreadable_mods_count,
            inventory.summary.steam_library_paths.len()
        );
        log_user_event(
            &format!(
                "mod_profile_manager scan success | local={} workshop={} unreadable={}",
                inventory.summary.local_mods_found,
                inventory.summary.steam_workshop_mods_found,
                inventory.summary.unreadable_mods_count
            ),
            "success",
        );
        Ok(inventory.mods)
    })
}

#[tauri::command]
pub fn scan_mods_light(
    app: AppHandle,
    profile_state: State<'_, AppProfileState>,
    game: Option<String>,
) -> Result<super::models::ModProfileManagerState, String> {
    let result = catch_command("scan_mods_light", || {
        let _scan_guard = ScanGuard::acquire()?;
        let inventory = scan_inventory_with_mode(
            &app,
            profile_state.inner(),
            game.as_deref(),
            ScanMode::Light,
        )?;
        let presets = presets::list_presets(&app, Some(inventory.summary.selected_game))?;
        Ok(super::models::ModProfileManagerState {
            summary: super::models::ModScanSummary {
                presets_saved: presets.len(),
                ..inventory.summary
            },
            mods: inventory.mods,
            presets,
            warnings: inventory.warnings,
            current_profile_path: inventory.current_profile_path,
            logs: inventory.logs,
        })
    });
    if let Err(error) = &result {
        crate::dev_log!(
            "[trace] ERROR mod_manager command=scan_mods_light error={}",
            error
        );
    }
    result
}

#[tauri::command]
pub fn scan_mods_deep(
    app: AppHandle,
    profile_state: State<'_, AppProfileState>,
    game: Option<String>,
) -> Result<super::models::ModProfileManagerState, String> {
    let result = catch_command("scan_mods_deep", || {
        let _scan_guard = ScanGuard::acquire()?;
        let inventory =
            scan_inventory_with_mode(&app, profile_state.inner(), game.as_deref(), ScanMode::Deep)?;
        let presets = presets::list_presets(&app, Some(inventory.summary.selected_game))?;
        Ok(super::models::ModProfileManagerState {
            summary: super::models::ModScanSummary {
                presets_saved: presets.len(),
                ..inventory.summary
            },
            mods: inventory.mods,
            presets,
            warnings: inventory.warnings,
            current_profile_path: inventory.current_profile_path,
            logs: inventory.logs,
        })
    });
    if let Err(error) = &result {
        crate::dev_log!(
            "[trace] ERROR mod_manager command=scan_mods_deep error={}",
            error
        );
    }
    result
}

#[tauri::command]
pub fn list_mod_presets(app: AppHandle, game: Option<String>) -> Result<Vec<ModPreset>, String> {
    catch_command("list_mod_presets", || {
        let game = match game.as_deref() {
            Some(value) => Some(GameType::try_from(value)?),
            None => None,
        };
        presets::list_presets(&app, game)
    })
}

#[tauri::command]
pub fn create_mod_preset(
    app: AppHandle,
    profile_state: State<'_, AppProfileState>,
    name: String,
    game: String,
    notes: Option<String>,
    preset_label: Option<String>,
) -> Result<ModPreset, String> {
    crate::dev_log!(
        "[mod-profile-manager] create preset requested game={} name={}",
        game,
        name
    );
    log_user_event(
        &format!("mod_profile_manager create preset | {}", name),
        "start",
    );

    catch_command("create_mod_preset", || {
        let game = GameType::try_from(game.as_str())?;
        let trimmed_name = name.trim();
        if trimmed_name.is_empty() {
            return Err("Preset name is required.".to_string());
        }

        let inventory = scan_inventory(&app, profile_state.inner(), Some(game.as_str()))?;
        let preset_mods = build_preset_mods(
            &inventory.mods,
            inventory.summary.active_mods_reliably_known,
        );
        let preset_load_order_source = if inventory.summary.active_mods_reliably_known {
            "detected".to_string()
        } else if preset_mods.is_empty() {
            "unknown".to_string()
        } else {
            inventory.summary.load_order_source.clone()
        };

        let timestamp = chrono::Local::now().to_rfc3339();
        let preset = ModPreset {
            id: Uuid::new_v4().to_string(),
            name: trimmed_name.to_string(),
            game,
            created_at: timestamp.clone(),
            updated_at: timestamp,
            mods: preset_mods,
            notes: notes.and_then(|value| {
                let trimmed = value.trim().to_string();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed)
                }
            }),
            preset_label: preset_label.and_then(|value| {
                let trimmed = value.trim().to_string();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed)
                }
            }),
            load_order_source: preset_load_order_source,
        };

        let preset = presets::save_preset(&app, preset)?;
        crate::dev_log!(
            "[mod-profile-manager] preset created id={} name={} mods={} load_order_source={}",
            preset.id,
            preset.name,
            preset.mods.len(),
            preset.load_order_source
        );
        log_user_event(
            &format!(
                "mod_profile_manager preset created | {} | mods={} | load_order_source={}",
                preset.name,
                preset.mods.len(),
                preset.load_order_source
            ),
            "success",
        );
        Ok(preset)
    })
}

#[tauri::command]
pub fn compare_mod_preset(
    app: AppHandle,
    profile_state: State<'_, AppProfileState>,
    preset_id: String,
    game: String,
) -> Result<PresetCompareResult, String> {
    crate::dev_log!(
        "[mod-profile-manager] compare preset requested preset_id={} game={}",
        preset_id,
        game
    );
    log_user_event(
        &format!("mod_profile_manager compare preset | {}", preset_id),
        "start",
    );

    catch_command("compare_mod_preset", || {
        let game = GameType::try_from(game.as_str())?;
        let result = compare::compare_preset(&app, profile_state.inner(), &preset_id, game)?;
        crate::dev_log!(
            "[mod-profile-manager] compare completed preset_id={} missing={} extra={} path_changes={} load_order_diff={}",
            preset_id,
            result.summary.missing_mods_count,
            result.summary.extra_mods_count,
            result.summary.changed_paths_count,
            result.summary.load_order_differences_count
        );
        log_user_event(
            &format!(
                "mod_profile_manager compare success | missing={} extra={}",
                result.summary.missing_mods_count, result.summary.extra_mods_count
            ),
            "success",
        );
        Ok(result)
    })
}

#[tauri::command]
pub fn export_mod_preset(app: AppHandle, preset_id: String) -> Result<String, String> {
    crate::dev_log!(
        "[mod-profile-manager] export preset requested preset_id={}",
        preset_id
    );
    log_user_event(
        &format!("mod_profile_manager export preset | {}", preset_id),
        "start",
    );

    catch_command("export_mod_preset", || {
        let exported = presets::export_preset(&app, &preset_id)?;
        if let Some(path) = exported {
            crate::dev_log!(
                "[mod-profile-manager] export preset completed preset_id={} path={}",
                preset_id,
                path
            );
            log_user_event(
                &format!("mod_profile_manager export success | {}", path),
                "success",
            );
            Ok(path)
        } else {
            crate::dev_log!("[mod-profile-manager] export preset canceled");
            Ok(String::new())
        }
    })
}

#[tauri::command]
pub fn import_mod_preset(app: AppHandle) -> Result<Option<ModPreset>, String> {
    crate::dev_log!("[mod-profile-manager] import preset requested");
    log_user_event("mod_profile_manager import preset", "start");

    catch_command("import_mod_preset", || {
        let imported = presets::import_preset(&app)?;
        if let Some(preset) = imported.as_ref() {
            crate::dev_log!(
                "[mod-profile-manager] import preset completed id={} name={}",
                preset.id,
                preset.name
            );
            log_user_event(
                &format!("mod_profile_manager import success | {}", preset.name),
                "success",
            );
        } else {
            crate::dev_log!("[mod-profile-manager] import preset canceled");
        }
        Ok(imported)
    })
}

#[tauri::command]
pub fn delete_mod_preset(app: AppHandle, preset_id: String) -> Result<(), String> {
    crate::dev_log!(
        "[mod-profile-manager] delete preset requested preset_id={}",
        preset_id
    );
    log_user_event(
        &format!("mod_profile_manager delete preset | {}", preset_id),
        "start",
    );

    catch_command("delete_mod_preset", || {
        presets::delete_preset(&app, &preset_id)?;
        log_user_event(
            &format!("mod_profile_manager delete success | {}", preset_id),
            "success",
        );
        Ok(())
    })
}

#[tauri::command]
pub fn select_manual_workshop_directory(
    app: AppHandle,
    game: String,
) -> Result<Option<String>, String> {
    catch_command("select_manual_workshop_directory", || {
        let game = GameType::try_from(game.as_str())?;
        let picked = presets::pick_workshop_directory(&app)?;
        if let Some(path) = picked.as_ref() {
            presets::set_manual_workshop_path(&app, game, path.clone())?;
            crate::dev_log!(
                "[mod-profile-manager] manual workshop path saved game={} path={}",
                game.as_str(),
                path
            );
        }
        Ok(picked)
    })
}

#[tauri::command]
pub fn clear_manual_workshop_directory(app: AppHandle, game: String) -> Result<(), String> {
    catch_command("clear_manual_workshop_directory", || {
        let game = GameType::try_from(game.as_str())?;
        presets::clear_manual_workshop_path(&app, game)?;
        crate::dev_log!(
            "[mod-profile-manager] manual workshop path cleared game={}",
            game.as_str()
        );
        Ok(())
    })
}

#[tauri::command]
pub fn fetch_workshop_mod(input: String) -> Result<WorkshopMod, String> {
    catch_command("fetch_workshop_mod", || {
        workshop_api::fetch_workshop_mod(&input)
    })
}

#[tauri::command]
pub fn load_mod_sandboxes(app: AppHandle) -> Result<SandboxCollection, String> {
    catch_command("load_mod_sandboxes", || sandbox::load_sandboxes(&app))
}

#[tauri::command]
pub fn save_mod_sandboxes(app: AppHandle, collection: SandboxCollection) -> Result<(), String> {
    catch_command("save_mod_sandboxes", || {
        sandbox::save_sandboxes(&app, &collection)
    })
}

#[tauri::command]
pub fn create_mod_sandbox(
    app: AppHandle,
    title: String,
    description: String,
) -> Result<SandboxCollection, String> {
    catch_command("create_mod_sandbox", || {
        sandbox::add_sandbox(&app, title, description)
    })
}

#[tauri::command]
pub fn delete_mod_sandbox(app: AppHandle, sandbox_id: String) -> Result<SandboxCollection, String> {
    catch_command("delete_mod_sandbox", || {
        sandbox::remove_sandbox(&app, &sandbox_id)
    })
}

#[tauri::command]
pub fn add_mod_to_sandbox(
    app: AppHandle,
    sandbox_id: String,
    workshop_input: String,
    manual_fallback: Option<bool>,
) -> Result<SandboxCollection, String> {
    catch_command("add_mod_to_sandbox", || {
        sandbox::add_workshop_mod_to_sandbox(
            &app,
            &sandbox_id,
            &workshop_input,
            manual_fallback.unwrap_or(false),
        )
    })
}

#[tauri::command]
pub fn upsert_sandbox_preset(
    app: AppHandle,
    sandbox_preset: ModSandbox,
) -> Result<SandboxCollection, String> {
    catch_command("upsert_sandbox_preset", || {
        sandbox::upsert_sandbox_preset(&app, sandbox_preset)
    })
}

#[tauri::command]
pub fn upsert_test_sandbox_preset(app: AppHandle) -> Result<SandboxCollection, String> {
    catch_command("upsert_test_sandbox_preset", || {
        sandbox::upsert_test_sandbox_preset(&app)
    })
}

#[tauri::command]
pub fn remove_mod_from_sandbox(
    app: AppHandle,
    sandbox_id: String,
    mod_id: u64,
) -> Result<SandboxCollection, String> {
    catch_command("remove_mod_from_sandbox", || {
        sandbox::remove_workshop_mod_from_sandbox(&app, &sandbox_id, mod_id)
    })
}

#[tauri::command]
pub fn toggle_mod_in_sandbox(
    app: AppHandle,
    sandbox_id: String,
    mod_id: u64,
    enabled: bool,
) -> Result<SandboxCollection, String> {
    catch_command("toggle_mod_in_sandbox", || {
        sandbox::toggle_workshop_mod_enabled(&app, &sandbox_id, mod_id, enabled)
    })
}

#[tauri::command]
pub fn apply_sandbox_to_active_profile(
    app: AppHandle,
    profile_state: State<'_, AppProfileState>,
    sandbox_id: String,
    force_clear: bool,
) -> Result<ApplySandboxResult, String> {
    catch_command("apply_sandbox_to_active_profile", || {
        sandbox::apply_sandbox_to_active_profile_with_force(
            &app,
            profile_state.inner(),
            &sandbox_id,
            force_clear,
        )
    })
}

#[tauri::command]
pub fn open_steam_console() -> Result<(), String> {
    catch_command("open_steam_console", launcher::open_steam_console)
}

#[tauri::command]
pub fn open_workshop_page(mod_id: String) -> Result<(), String> {
    catch_command("open_workshop_page", || {
        launcher::open_workshop_page(&mod_id)
    })
}

#[tauri::command]
pub fn open_workshop_subscribe_page(mod_id: String) -> Result<(), String> {
    catch_command("open_workshop_subscribe_page", || {
        launcher::open_workshop_subscribe_page(&mod_id)
    })
}

#[tauri::command]
pub fn open_sandbox_mod_workshop_page(steam_id: String) -> Result<(), String> {
    catch_command("open_sandbox_mod_workshop_page", || {
        launcher::open_sandbox_mod_workshop_page(&steam_id)
    })
}

#[tauri::command]
pub fn open_sandbox_mod_in_steam(steam_id: String) -> Result<(), String> {
    catch_command("open_sandbox_mod_in_steam", || {
        launcher::open_sandbox_mod_in_steam(&steam_id)
    })
}

#[tauri::command]
pub fn check_workshop_mod_downloaded(app_id: u32, mod_id: u64) -> Result<bool, String> {
    catch_command("check_workshop_mod_downloaded", || {
        Ok(workshop_api::is_workshop_mod_downloaded(app_id, mod_id))
    })
}

#[tauri::command]
pub fn check_workshop_mod_download_status(mod_id: String) -> Result<WorkshopInstallStatus, String> {
    catch_command("check_workshop_mod_download_status", || {
        workshop_api::check_ets2_workshop_mod_installed(&mod_id)
    })
}

#[tauri::command]
pub fn check_workshop_mods_download_status(
    mod_ids: Vec<String>,
) -> Result<Vec<WorkshopInstallStatus>, String> {
    catch_command("check_workshop_mods_download_status", || {
        mod_ids
            .iter()
            .map(|mod_id| workshop_api::check_ets2_workshop_mod_installed(mod_id))
            .collect()
    })
}

#[tauri::command]
pub fn scan_steam_workshop_mods(app: AppHandle) -> Result<SteamWorkshopCache, String> {
    crate::dev_log!("[SteamWorkshopCache] scan command called");
    catch_command("scan_steam_workshop_mods", || {
        sandbox::scan_steam_workshop_mods(&app)
    })
}

#[tauri::command]
pub fn load_steam_workshop_mod_cache(app: AppHandle) -> Result<SteamWorkshopCache, String> {
    crate::dev_log!("[SteamWorkshopCache] load command called");
    catch_command("load_steam_workshop_mod_cache", || {
        sandbox::load_steam_workshop_mod_cache(&app)
    })
}

#[tauri::command]
pub fn refresh_workshop_mod_cache(app: AppHandle) -> Result<SteamWorkshopCache, String> {
    crate::dev_log!("[SteamWorkshopCache] refresh command called");
    catch_command("refresh_workshop_mod_cache", || {
        sandbox::refresh_workshop_mod_cache(&app)
    })
}

#[tauri::command]
pub fn check_workshop_mod_available(
    app: AppHandle,
    app_id: u32,
    workshop_id: String,
) -> Result<SteamWorkshopMod, String> {
    crate::dev_log!(
        "[SteamWorkshopCache] check command called app_id={} workshop_id={}",
        app_id,
        workshop_id
    );
    catch_command("check_workshop_mod_available", || {
        sandbox::check_workshop_mod_available(&app, app_id, &workshop_id)
    })
}

#[tauri::command]
pub fn load_sandbox_mod_presets() -> Result<Vec<SandboxModPreset>, String> {
    crate::dev_log!("[SandboxPreset] command load_sandbox_mod_presets");
    catch_command(
        "load_sandbox_mod_presets",
        sandbox::load_sandbox_mod_presets,
    )
}

#[tauri::command]
pub fn check_sandbox_preset_mods(
    app: AppHandle,
    preset_id: String,
) -> Result<SandboxPresetCheckResult, String> {
    crate::dev_log!(
        "[SandboxPreset] check command called preset_id={}",
        preset_id
    );
    catch_command("check_sandbox_preset_mods", || {
        sandbox::check_sandbox_mod_preset(&app, &preset_id)
    })
}

#[tauri::command(rename_all = "snake_case")]
pub fn activate_sandbox_mod_preset(
    app: AppHandle,
    profile_state: State<'_, AppProfileState>,
    profile_cache: State<'_, ProfileCache>,
    decrypt_cache: State<'_, DecryptCache>,
    preset_id: String,
    profile_id: Option<String>,
    save_name: Option<String>,
    game: Option<String>,
    app_id: Option<u32>,
) -> Result<SandboxPresetActivationResult, String> {
    crate::dev_log!(
        "[mod_profile_manager] activate_sandbox_mod_preset entered preset_id={} profile_id={:?} save_name={:?} game={:?} app_id={:?}",
        preset_id,
        profile_id,
        save_name,
        game,
        app_id
    );
    catch_command("activate_sandbox_mod_preset", || {
        sandbox::activate_sandbox_mod_preset_profile_sii(
            &app,
            profile_state.inner(),
            profile_cache.inner(),
            decrypt_cache.inner(),
            &preset_id,
            profile_id,
            None,
            game,
            app_id,
        )
    })
}
