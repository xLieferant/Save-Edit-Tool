use crate::dev_log;
use crate::features::backup::models::BackupRestoreResultDto;
use crate::features::backup::service as backup_service;
use crate::features::logging::service as logging_service;
use crate::shared::decrypt::decrypt_if_needed;
use crate::shared::paths::{
    autosave_path, ets2_base_config_path, game_sii_from_save, quicksave_config_path,
};
use crate::shared::trace::TraceScope;
use crate::state::{AppProfileState, DecryptCache, ProfileCache};
use crate::xp::command::{calculate_level, total_xp_to_reach_level, xp_required_for_level};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use tauri::State;
use tauri::command;

type CommandResult<T> = Result<T, CommandFailure>;
const UNDO_SNAPSHOT_LABEL: &str = "Before last edit";

#[derive(Debug)]
struct CommandFailure {
    user_message: String,
    technical_details: String,
}

impl CommandFailure {
    fn new(user_message: impl Into<String>, technical_details: impl Into<String>) -> Self {
        Self {
            user_message: user_message.into(),
            technical_details: technical_details.into(),
        }
    }
}

fn failure(
    user_message: impl Into<String>,
    technical_details: impl Into<String>,
) -> CommandFailure {
    CommandFailure::new(user_message, technical_details)
}

fn current_profile_path(profile_state: &AppProfileState) -> CommandResult<String> {
    profile_state
        .current_profile
        .lock()
        .map_err(|_| {
            failure(
                "Profilstatus konnte nicht gelesen werden.",
                "current_profile lock poisoned",
            )
        })?
        .clone()
        .ok_or_else(|| failure("Kein Profil geladen.", "No current profile is selected."))
}

fn get_active_save_path(profile_state: &AppProfileState) -> CommandResult<PathBuf> {
    let current_save = profile_state
        .current_save
        .lock()
        .map_err(|_| {
            failure(
                "Savestatus konnte nicht gelesen werden.",
                "current_save lock poisoned",
            )
        })?
        .clone();
    if let Some(save) = current_save {
        return Ok(game_sii_from_save(Path::new(&save)));
    }

    let profile = current_profile_path(profile_state)?;
    Ok(autosave_path(&profile))
}

fn write_text_with_auto_backup<F>(
    profile_state: &AppProfileState,
    path: &Path,
    action: &str,
    action_reason: &str,
    success_message: &str,
    content: &str,
    verify_write: F,
) -> CommandResult<()>
where
    F: FnOnce(&Path) -> CommandResult<()>,
{
    let mut context = logging_service::resolve_active_context(profile_state);
    context.extra.insert(
        "target".to_string(),
        logging_service::redact_path(&path.display().to_string()),
    );
    context
        .extra
        .insert("reason".to_string(), action_reason.to_string());
    let _ = logging_service::record_info(
        action,
        "Write operation started for the active save.",
        &context,
    );

    let backup = match backup_service::create_backup_for_targets(
        profile_state,
        action_reason,
        &backup_service::recommended_targets(path),
    ) {
        Ok(backup) => backup,
        Err(error) => {
            let user_message = "Automatisches Backup konnte nicht erstellt werden.";
            let _ = logging_service::record_error(
                action,
                Some("auto_backup_failed"),
                user_message,
                Some(&error),
                &context,
            );
            return Err(failure(user_message, error));
        }
    };

    context
        .extra
        .insert("backupId".to_string(), backup.backup_id.clone());

    if let Err(error) = fs::write(path, content.as_bytes()) {
        let technical = error.to_string();
        let user_message = "Datei konnte nicht geschrieben werden.";
        let _ = logging_service::record_error(
            action,
            Some("write_failed"),
            user_message,
            Some(&technical),
            &context,
        );
        return Err(failure(user_message, technical));
    }

    if let Err(error) = verify_write(path) {
        let _ = logging_service::record_error(
            action,
            Some("write_verification_failed"),
            &error.user_message,
            Some(&error.technical_details),
            &context,
        );
        return Err(error);
    }

    let _ = logging_service::record_info(action, success_message, &context);

    Ok(())
}

fn verify_contains(path: &Path, expected_fragment: &str, user_message: &str) -> CommandResult<()> {
    let verify =
        fs::read_to_string(path).map_err(|error| failure(user_message, error.to_string()))?;
    if !verify.contains(expected_fragment) {
        return Err(failure(
            user_message,
            format!(
                "The expected fragment `{}` was not found after writing {}.",
                expected_fragment,
                path.display()
            ),
        ));
    }
    Ok(())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UndoStatusDto {
    pub can_undo: bool,
    pub last_undo_label: Option<String>,
    pub last_undo_timestamp: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyCustomResetValuesResultDto {
    pub undo_backup_id: String,
    pub applied_money: Option<i64>,
    pub applied_level: Option<u32>,
    pub applied_xp: Option<u64>,
}

#[derive(Debug, Clone)]
struct ActiveSaveTargets {
    save_dir: PathBuf,
    game_sii_path: PathBuf,
    info_sii_path: PathBuf,
}

#[derive(Debug, Clone, Copy)]
struct ResolvedCustomResetValues {
    money: Option<i64>,
    level: Option<u32>,
    xp: Option<u64>,
}

fn active_selected_save_targets(
    profile_state: &AppProfileState,
) -> CommandResult<ActiveSaveTargets> {
    let _profile = current_profile_path(profile_state)?;
    let save_dir = profile_state
        .current_save
        .lock()
        .map_err(|_| failure("No active save selected", "current_save lock poisoned"))?
        .clone()
        .ok_or_else(|| failure("No active save selected", "No current save is selected."))?;
    let save_dir = PathBuf::from(save_dir);

    Ok(ActiveSaveTargets {
        game_sii_path: game_sii_from_save(&save_dir),
        info_sii_path: crate::shared::paths::info_sii_from_save(&save_dir),
        save_dir,
    })
}

fn resolve_custom_reset_values(
    level: Option<u32>,
    xp: Option<u64>,
    money: Option<i64>,
) -> CommandResult<ResolvedCustomResetValues> {
    if level.is_none() && xp.is_none() && money.is_none() {
        return Err(failure(
            "No reset values provided",
            "apply_custom_reset_values was called without any money, level or xp value.",
        ));
    }

    let max_level = calculate_level(u64::MAX).level;
    let resolved_level = match level {
        Some(value) if value > max_level => {
            return Err(failure(
                "Invalid level value",
                format!("Requested level {} exceeds max level {}", value, max_level),
            ));
        }
        Some(value) => {
            let minimum_xp = total_xp_to_reach_level(value);
            let next_increase = xp_required_for_level(value);
            if value < max_level && minimum_xp == 0 && next_increase == 0 {
                return Err(failure(
                    "Invalid level value",
                    format!(
                        "Level table does not expose progression data for level {}",
                        value
                    ),
                ));
            }
            Some(value)
        }
        None => None,
    };

    let resolved_xp = match (resolved_level, xp) {
        (_, Some(value)) => Some(value),
        (Some(value), None) => Some(total_xp_to_reach_level(value)),
        (None, None) => None,
    };

    let resolved_money = money.map(|value| value.max(0));

    Ok(ResolvedCustomResetValues {
        money: resolved_money,
        level: resolved_level.or_else(|| resolved_xp.map(|value| calculate_level(value).level)),
        xp: resolved_xp,
    })
}

fn replace_numeric_fields(
    content: &str,
    fields: &[&str],
    value: i64,
    user_message: &str,
    technical_context: &str,
) -> CommandResult<String> {
    let mut replaced = false;
    let mut updated = content.to_string();

    for field in fields {
        let pattern = Regex::new(&format!(r"(?m)^(\s*){}:\s*-?\d+", regex::escape(field)))
            .map_err(|error| {
                failure(user_message, format!("{} regex invalid: {}", field, error))
            })?;
        if pattern.is_match(&updated) {
            updated = pattern
                .replace_all(&updated, format!("${{1}}{}: {}", field, value))
                .to_string();
            replaced = true;
        }
    }

    if !replaced {
        return Err(failure(
            user_message,
            format!(
                "{} missing expected fields: {}",
                technical_context,
                fields.join(", ")
            ),
        ));
    }

    Ok(updated)
}

fn update_game_save_content(
    content: &str,
    values: ResolvedCustomResetValues,
) -> CommandResult<String> {
    let mut updated = content.to_string();

    if let Some(money) = values.money {
        updated = replace_numeric_fields(
            &updated,
            &["money_account", "info_money_account"],
            money,
            "Could not read save file",
            "game.sii money replacement",
        )?;
    }

    if let Some(xp) = values.xp {
        let xp_value = i64::try_from(xp).map_err(|_| {
            failure(
                "Invalid level value",
                format!("XP value {} exceeds i64 range for game.sii", xp),
            )
        })?;
        updated = replace_numeric_fields(
            &updated,
            &["experience_points", "info_players_experience"],
            xp_value,
            "Could not read save file",
            "game.sii xp replacement",
        )?;
    }

    Ok(updated)
}

fn update_info_save_content(
    content: &str,
    values: ResolvedCustomResetValues,
) -> CommandResult<String> {
    let mut updated = content.to_string();

    if let Some(money) = values.money {
        updated = replace_numeric_fields(
            &updated,
            &["info_money_account", "money_account"],
            money,
            "Could not read save file",
            "info.sii money replacement",
        )?;
    }

    if let Some(xp) = values.xp {
        let xp_value = i64::try_from(xp).map_err(|_| {
            failure(
                "Invalid level value",
                format!("XP value {} exceeds i64 range for info.sii", xp),
            )
        })?;
        updated = replace_numeric_fields(
            &updated,
            &["info_players_experience", "experience_points"],
            xp_value,
            "Could not read save file",
            "info.sii xp replacement",
        )?;
    }

    Ok(updated)
}

fn write_save_text(path: &Path, content: &str) -> CommandResult<()> {
    fs::write(path, content.as_bytes()).map_err(|error| {
        failure(
            "Could not write save file",
            format!("{}: {}", path.display(), error),
        )
    })
}

fn invalidate_custom_reset_caches(
    profile_cache: &ProfileCache,
    decrypt_cache: &DecryptCache,
    targets: &ActiveSaveTargets,
) {
    decrypt_cache.invalidate_path(&targets.game_sii_path);
    decrypt_cache.invalidate_path(&targets.info_sii_path);
    profile_cache.invalidate_save_data();
    profile_cache.invalidate_vehicle_data();
}

#[command]
pub fn get_undo_status(profile_state: State<'_, AppProfileState>) -> Result<UndoStatusDto, String> {
    let mut trace = TraceScope::new("get_undo_status");
    let has_selected_save = profile_state
        .current_save
        .lock()
        .map_err(|_| "current_save lock poisoned".to_string())?
        .clone()
        .is_some();

    if !has_selected_save {
        trace.finish_ok();
        return Ok(UndoStatusDto {
            can_undo: false,
            last_undo_label: None,
            last_undo_timestamp: None,
        });
    }

    let latest = backup_service::find_latest_backup_for_active_save_by_type(
        profile_state.inner(),
        backup_service::BACKUP_TYPE_UNDO_BEFORE_EDIT,
    )
    .map_err(|error| {
        trace.finish_error(&error);
        error
    })?;

    trace.finish_ok();
    Ok(UndoStatusDto {
        can_undo: latest.is_some(),
        last_undo_label: latest.as_ref().map(|item| item.action_reason.clone()),
        last_undo_timestamp: latest.as_ref().map(|item| item.created_at_utc.clone()),
    })
}

#[command]
pub fn apply_custom_reset_values(
    level: Option<u32>,
    xp: Option<u64>,
    money: Option<i64>,
    profile_state: State<'_, AppProfileState>,
    profile_cache: State<'_, ProfileCache>,
    decrypt_cache: State<'_, DecryptCache>,
) -> Result<ApplyCustomResetValuesResultDto, String> {
    let mut trace = TraceScope::new("apply_custom_reset_values");
    let mut context = logging_service::resolve_active_context(profile_state.inner());
    let targets = active_selected_save_targets(profile_state.inner()).map_err(|error| {
        let _ = logging_service::record_error(
            "safe_value_reset",
            Some("active_save_missing"),
            &error.user_message,
            Some(&error.technical_details),
            &context,
        );
        trace.finish_error(&error.user_message);
        error.user_message
    })?;
    let values = resolve_custom_reset_values(level, xp, money).map_err(|error| {
        let _ = logging_service::record_error(
            "safe_value_reset",
            Some("invalid_reset_values"),
            &error.user_message,
            Some(&error.technical_details),
            &context,
        );
        trace.finish_error(&error.user_message);
        error.user_message
    })?;

    dev_log!("[trace] ACTIVE_SAVE path={}", targets.save_dir.display());
    dev_log!("[trace] CREATE_UNDO_SNAPSHOT label={}", UNDO_SNAPSHOT_LABEL);
    let undo_backup = backup_service::create_backup_for_targets_with_type(
        profile_state.inner(),
        backup_service::BACKUP_TYPE_UNDO_BEFORE_EDIT,
        UNDO_SNAPSHOT_LABEL,
        &backup_service::recommended_targets(&targets.game_sii_path),
    )
    .map_err(|error| {
        let user_message = "Could not create undo snapshot";
        let _ = logging_service::record_error(
            "safe_value_reset",
            Some("undo_snapshot_failed"),
            user_message,
            Some(&error),
            &context,
        );
        trace.finish_error(format!("{}: {}", user_message, error));
        user_message.to_string()
    })?;
    context
        .extra
        .insert("undoBackupId".to_string(), undo_backup.backup_id.clone());

    let game_content = decrypt_if_needed(&targets.game_sii_path).map_err(|error| {
        let _ = logging_service::record_error(
            "safe_value_reset",
            Some("read_game_sii_failed"),
            "Could not read save file",
            Some(&error.to_string()),
            &context,
        );
        trace.finish_error(format!("Could not read save file: {}", error));
        "Could not read save file".to_string()
    })?;
    let info_content = decrypt_if_needed(&targets.info_sii_path).map_err(|error| {
        let _ = logging_service::record_error(
            "safe_value_reset",
            Some("read_info_sii_failed"),
            "Could not read save file",
            Some(&error.to_string()),
            &context,
        );
        trace.finish_error(format!("Could not read save file: {}", error));
        "Could not read save file".to_string()
    })?;

    if let Some(value) = values.money {
        dev_log!("[trace] APPLY_VALUE money={}", value);
        context.extra.insert("money".to_string(), value.to_string());
    }
    if let Some(value) = values.level {
        dev_log!("[trace] APPLY_VALUE level={}", value);
        context.extra.insert("level".to_string(), value.to_string());
    }
    if let Some(value) = values.xp {
        dev_log!("[trace] APPLY_VALUE xp={}", value);
        context.extra.insert("xp".to_string(), value.to_string());
    }

    let updated_game = update_game_save_content(&game_content, values).map_err(|error| {
        trace.finish_error(&error.user_message);
        error.user_message
    })?;
    let updated_info = update_info_save_content(&info_content, values).map_err(|error| {
        trace.finish_error(&error.user_message);
        error.user_message
    })?;

    dev_log!(
        "[trace] WRITE_SAVE_FILE path={}",
        targets.game_sii_path.display()
    );
    write_save_text(&targets.game_sii_path, &updated_game).map_err(|error| {
        trace.finish_error(&error.user_message);
        error.user_message
    })?;

    dev_log!(
        "[trace] WRITE_SAVE_FILE path={}",
        targets.info_sii_path.display()
    );
    write_save_text(&targets.info_sii_path, &updated_info).map_err(|error| {
        trace.finish_error(&error.user_message);
        error.user_message
    })?;

    dev_log!(
        "[trace] INVALIDATE_CACHE save={}",
        targets.save_dir.display()
    );
    invalidate_custom_reset_caches(profile_cache.inner(), decrypt_cache.inner(), &targets);
    let _ = logging_service::record_info(
        "safe_value_reset",
        "Safe Value Reset applied to the active save.",
        &context,
    );
    trace.finish_ok();

    Ok(ApplyCustomResetValuesResultDto {
        undo_backup_id: undo_backup.backup_id,
        applied_money: values.money,
        applied_level: values.level,
        applied_xp: values.xp,
    })
}

#[command]
pub fn undo_last_save_change(
    profile_state: State<'_, AppProfileState>,
    profile_cache: State<'_, ProfileCache>,
    decrypt_cache: State<'_, DecryptCache>,
) -> Result<BackupRestoreResultDto, String> {
    let mut trace = TraceScope::new("undo_last_save_change");
    let mut context = logging_service::resolve_active_context(profile_state.inner());
    let targets = active_selected_save_targets(profile_state.inner()).map_err(|error| {
        let _ = logging_service::record_error(
            "undo_last_change",
            Some("active_save_missing"),
            &error.user_message,
            Some(&error.technical_details),
            &context,
        );
        trace.finish_error(&error.user_message);
        error.user_message
    })?;
    dev_log!("[trace] ACTIVE_SAVE path={}", targets.save_dir.display());

    let latest_undo = backup_service::find_latest_backup_for_active_save_by_type(
        profile_state.inner(),
        backup_service::BACKUP_TYPE_UNDO_BEFORE_EDIT,
    )
    .map_err(|error| {
        let _ = logging_service::record_error(
            "undo_last_change",
            Some("lookup_failed"),
            "Undo snapshot not available.",
            Some(&error),
            &context,
        );
        trace.finish_error(&error);
        error
    })?
    .ok_or_else(|| {
        let message = "Undo snapshot not found".to_string();
        let _ = logging_service::record_error(
            "undo_last_change",
            Some("snapshot_missing"),
            &message,
            None,
            &context,
        );
        trace.finish_error(&message);
        message
    })?;
    context
        .extra
        .insert("undoBackupId".to_string(), latest_undo.backup_id.clone());

    let storage_dir =
        backup_service::get_backup_storage_dir(&latest_undo.backup_id).map_err(|error| {
            trace.finish_error(&error);
            error
        })?;
    dev_log!(
        "[trace] RESTORE_UNDO_SNAPSHOT path={}",
        storage_dir.display()
    );

    let execution =
        backup_service::restore_backup(profile_state.inner(), &latest_undo.backup_id, true)
            .map_err(|error| {
                trace.finish_error(&error);
                error
            })?;

    dev_log!(
        "[trace] INVALIDATE_CACHE save={}",
        targets.save_dir.display()
    );
    for path in &execution.touched_paths {
        decrypt_cache.invalidate_path(path);
    }
    profile_cache.invalidate_save_data();
    profile_cache.invalidate_vehicle_data();
    let _ = logging_service::record_info(
        "undo_last_change",
        "The last save change was restored from the undo snapshot.",
        &context,
    );
    trace.finish_ok();

    Ok(execution.result)
}

#[command]
pub fn edit_money(
    amount: i64,
    profile_state: State<'_, AppProfileState>,
    profile_cache: State<'_, ProfileCache>,
    decrypt_cache: State<'_, DecryptCache>,
) -> Result<(), String> {
    let path = get_active_save_path(profile_state.inner()).map_err(|error| error.user_message)?;
    let content = decrypt_if_needed(&path)?;

    // 1. Info-Wert ersetzen
    let re_info = Regex::new(r"info_money_account:\s*\d+")
        .map_err(|error| format!("Money-Regel ungültig: {}", error))?;
    let content = re_info
        .replace(&content, format!("info_money_account: {}", amount))
        .to_string();

    // 2. Echten Wert ersetzen (unter Beibehaltung der Einrückung)
    let re_main = Regex::new(r"(?m)^(\s*)money_account:\s*\d+")
        .map_err(|error| format!("Money-Regel ungültig: {}", error))?;
    let content = re_main
        .replace(&content, format!("${{1}}money_account: {}", amount))
        .to_string();

    write_text_with_auto_backup(
        profile_state.inner(),
        &path,
        "edit_money",
        "before money edit",
        "Money values were updated for the active save.",
        &content,
        |_| Ok(()),
    )
    .map_err(|error| error.user_message)?;
    decrypt_cache.invalidate_path(&path);
    profile_cache.invalidate_save_data();
    dev_log!("Geld geändert: {}", amount);
    Ok(())
}

#[command]
pub fn edit_xp(
    xp: i64,
    profile_state: State<'_, AppProfileState>,
    profile_cache: State<'_, ProfileCache>,
    decrypt_cache: State<'_, DecryptCache>,
) -> Result<(), String> {
    let path = get_active_save_path(profile_state.inner()).map_err(|error| error.user_message)?;
    let content = decrypt_if_needed(&path)?;

    // 1. Info-Wert ersetzen
    let re_info = Regex::new(r"info_players_experience:\s*\d+")
        .map_err(|error| format!("XP-Regel ungültig: {}", error))?;
    let content = re_info
        .replace(&content, format!("info_players_experience: {}", xp))
        .to_string();

    // 2. Echten Wert ersetzen
    let re_main = Regex::new(r"(?m)^(\s*)experience_points:\s*\d+")
        .map_err(|error| format!("XP-Regel ungültig: {}", error))?;
    let content = re_main
        .replace(&content, format!("${{1}}experience_points: {}", xp))
        .to_string();

    write_text_with_auto_backup(
        profile_state.inner(),
        &path,
        "edit_xp",
        "before xp edit",
        "XP values were updated for the active save.",
        &content,
        |_| Ok(()),
    )
    .map_err(|error| error.user_message)?;
    decrypt_cache.invalidate_path(&path);
    profile_cache.invalidate_save_data();
    dev_log!("XP geändert: {}", xp);
    Ok(())
}

#[command]
pub fn edit_level(
    xp: i64,
    profile_state: State<'_, AppProfileState>,
    profile_cache: State<'_, ProfileCache>,
    decrypt_cache: State<'_, DecryptCache>,
) -> Result<(), String> {
    let path = get_active_save_path(profile_state.inner()).map_err(|error| error.user_message)?;
    let content = decrypt_if_needed(&path)?;

    // 1. Info-Wert ersetzen
    let re_info = Regex::new(r"info_players_experience:\s*\d+")
        .map_err(|error| format!("Level-Regel ungültig: {}", error))?;
    let content = re_info
        .replace(&content, format!("info_players_experience: {}", xp))
        .to_string();

    // 2. Echten Wert ersetzen
    let re_main = Regex::new(r"(?m)^(\s*)experience_points:\s*\d+")
        .map_err(|error| format!("Level-Regel ungültig: {}", error))?;
    let content = re_main
        .replace(&content, format!("${{1}}experience_points: {}", xp))
        .to_string();

    write_text_with_auto_backup(
        profile_state.inner(),
        &path,
        "edit_level",
        "before level edit",
        "Level-related XP values were updated for the active save.",
        &content,
        |_| Ok(()),
    )
    .map_err(|error| error.user_message)?;
    decrypt_cache.invalidate_path(&path);
    profile_cache.invalidate_save_data();
    dev_log!("XP (via edit_level) geändert: {}", xp);
    Ok(())
}

#[derive(Deserialize)]
pub struct EditValuePayload {
    value: String,
}

#[command]
pub fn edit_player_money(
    value: i64,
    profile_state: State<'_, AppProfileState>,
    profile_cache: State<'_, ProfileCache>,
    decrypt_cache: State<'_, DecryptCache>,
) -> Result<(), String> {
    dev_log!("--- edit_player_money START ---");

    // ✅ Use the helper - respects current_save if set
    let path = get_active_save_path(profile_state.inner()).map_err(|error| error.user_message)?;
    let content = decrypt_if_needed(&path)?;

    let re_money = Regex::new(r"money_account:\s*(\d+)").map_err(|e| e.to_string())?;

    if !re_money.is_match(&content) {
        return Err("money_account nicht gefunden".into());
    }

    let new_content = re_money.replace(&content, format!("money_account: {}", value));

    write_text_with_auto_backup(
        profile_state.inner(),
        &path,
        "edit_player_money",
        "before player money edit",
        "Player money was updated for the active save.",
        new_content.as_ref(),
        |_| Ok(()),
    )
    .map_err(|error| error.user_message)?;
    decrypt_cache.invalidate_path(&path);
    profile_cache.invalidate_save_data();

    dev_log!("Money erfolgreich geändert auf {}", value);
    dev_log!("--- edit_player_money END ---");

    Ok(())
}

#[command]
pub fn edit_player_experience(
    value: i64,
    profile_state: State<'_, AppProfileState>,
    profile_cache: State<'_, ProfileCache>,
    decrypt_cache: State<'_, DecryptCache>,
) -> Result<(), String> {
    dev_log!("--- edit_player_experience START ---");

    // ✅ Use the helper - respects current_save if set
    let path = get_active_save_path(profile_state.inner()).map_err(|error| error.user_message)?;
    let content = decrypt_if_needed(&path)?;

    let re_experience = Regex::new(r"experience_points:\s*(\d+)").map_err(|e| e.to_string())?;

    if !re_experience.is_match(&content) {
        return Err("experience_points: nicht gefunden".into());
    }

    let new_content = re_experience.replace(&content, format!("experience_points: {}", value));

    write_text_with_auto_backup(
        profile_state.inner(),
        &path,
        "edit_player_experience",
        "before player experience edit",
        "Player experience was updated for the active save.",
        new_content.as_ref(),
        |_| Ok(()),
    )
    .map_err(|error| error.user_message)?;
    decrypt_cache.invalidate_path(&path);
    profile_cache.invalidate_save_data();

    dev_log!("Experience erfolgreich geändert auf {}", value);
    dev_log!("--- edit_player_experience END ---");

    Ok(())
}

#[command]
pub fn edit_skill_value(
    skill: String,
    value: i64,
    profile_state: State<'_, AppProfileState>,
    profile_cache: State<'_, ProfileCache>,
    decrypt_cache: State<'_, DecryptCache>,
) -> Result<(), String> {
    dev_log!("--- edit_skill START ---");
    dev_log!("Skill: {}, Wert: {}", skill, value);

    // ✅ Use the helper - respects current_save if set
    let path = get_active_save_path(profile_state.inner()).map_err(|error| error.user_message)?;
    let content = decrypt_if_needed(&path)?;

    // Regex dynamisch je Skill
    let re = Regex::new(&format!(r"\b{}\s*:\s*\d+", regex::escape(&skill)))
        .map_err(|e| e.to_string())?;

    if !re.is_match(&content) {
        return Err(format!("Skill '{}' nicht gefunden", skill));
    }

    let new_content = re.replace(&content, format!("{}: {}", skill, value));

    write_text_with_auto_backup(
        profile_state.inner(),
        &path,
        "edit_skill_value",
        &format!("before skill edit {}", skill),
        "A player skill value was updated for the active save.",
        new_content.as_ref(),
        |_| Ok(()),
    )
    .map_err(|error| error.user_message)?;
    decrypt_cache.invalidate_path(&path);
    profile_cache.invalidate_save_data();

    dev_log!("Skill '{}' erfolgreich geändert auf {}", skill, value);
    dev_log!("--- edit_skill END ---");

    Ok(())
}

#[command]
pub fn edit_developer_value(
    value: i64,
    profile_state: State<'_, AppProfileState>,
    profile_cache: State<'_, ProfileCache>,
    decrypt_cache: State<'_, DecryptCache>,
) -> Result<(), String> {
    let path = ets2_base_config_path().ok_or("Globaler Config-Pfad nicht gefunden".to_string())?;

    dev_log!("Schreibe Developer Value in: {}", path.display());

    let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;

    let re = Regex::new(r#"uset g_developer\s+"[^"]+""#)
        .map_err(|error| format!("Developer-Regel ungültig: {}", error))?;

    if !re.is_match(&content) {
        return Err("g_developer nicht in config.cfg gefunden".into());
    }

    let new_content = re.replace(&content, format!(r#"uset g_developer "{}""#, value));
    let expected = format!(r#"uset g_developer "{}""#, value);

    write_text_with_auto_backup(
        profile_state.inner(),
        &path,
        "edit_developer_value",
        "before developer config edit",
        "The global developer config value was updated.",
        new_content.as_ref(),
        |written_path| {
            verify_contains(
                written_path,
                &expected,
                "Developer-Wert konnte nicht verifiziert werden.",
            )
        },
    )
    .map_err(|error| error.user_message)?;

    profile_cache.invalidate_base_config();
    decrypt_cache.invalidate_path(&path);
    dev_log!("Dev erfolgreich geändert auf {}", value);
    Ok(())
}

#[command]
pub fn edit_console_value(
    value: i64,
    profile_state: State<'_, AppProfileState>,
    profile_cache: State<'_, ProfileCache>,
    decrypt_cache: State<'_, DecryptCache>,
) -> Result<(), String> {
    let path = ets2_base_config_path().ok_or("Globaler Config-Pfad nicht gefunden".to_string())?;

    dev_log!("Schreibe Console Value in: {}", path.display());

    let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;

    let re = Regex::new(r#"uset g_console\s+"[^"]+""#)
        .map_err(|error| format!("Console-Regel ungültig: {}", error))?;

    if !re.is_match(&content) {
        return Err("g_console nicht in config.cfg gefunden".into());
    }

    let new_content = re.replace(&content, format!(r#"uset g_console "{}""#, value));
    let expected = format!(r#"uset g_console "{}""#, value);

    write_text_with_auto_backup(
        profile_state.inner(),
        &path,
        "edit_console_value",
        "before console config edit",
        "The global console config value was updated.",
        new_content.as_ref(),
        |written_path| {
            verify_contains(
                written_path,
                &expected,
                "Console-Wert konnte nicht verifiziert werden.",
            )
        },
    )
    .map_err(|error| error.user_message)?;

    profile_cache.invalidate_base_config();
    decrypt_cache.invalidate_path(&path);
    dev_log!("Dev erfolgreich geändert auf {}", value);
    Ok(())
}

#[command]
pub fn edit_convoy_value(
    value: i64,
    profile_state: State<'_, AppProfileState>,
    profile_cache: State<'_, ProfileCache>,
    decrypt_cache: State<'_, DecryptCache>,
) -> Result<(), String> {
    let path = ets2_base_config_path().ok_or("Globaler Config-Pfad nicht gefunden".to_string())?;

    dev_log!("Schreibe Convoy in: {}", path.display());

    let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;

    let re = Regex::new(r#"uset g_max_convoy_size\s+"[^"]+""#)
        .map_err(|error| format!("Convoy-Regel ungültig: {}", error))?;

    if !re.is_match(&content) {
        return Err("g_max_convoy_size nicht in config.cfg gefunden".into());
    }

    let new_content = re.replace(&content, format!(r#"uset g_max_convoy_size "{}""#, value));
    let expected = format!(r#"uset g_max_convoy_size "{}""#, value);

    write_text_with_auto_backup(
        profile_state.inner(),
        &path,
        "edit_convoy_value",
        "before convoy config edit",
        "The global convoy config value was updated.",
        new_content.as_ref(),
        |written_path| {
            verify_contains(
                written_path,
                &expected,
                "Convoy-Wert konnte nicht verifiziert werden.",
            )
        },
    )
    .map_err(|error| error.user_message)?;

    profile_cache.invalidate_base_config();
    decrypt_cache.invalidate_path(&path);
    dev_log!("Convoy erfolgreich geändert auf {}", value);
    Ok(())
}

#[command]
pub fn edit_traffic_value(
    value: i64,
    profile_state: State<'_, AppProfileState>,
    profile_cache: State<'_, ProfileCache>,
    decrypt_cache: State<'_, DecryptCache>,
) -> Result<(), String> {
    // 🔒 Clamping: garantiert 0–10
    let value = value.clamp(0, 10);

    let path = ets2_base_config_path().ok_or("Globaler Config-Pfad nicht gefunden".to_string())?;

    dev_log!("Schreibe Traffic in: {} (Wert: {})", path.display(), value);

    let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;

    let re = Regex::new(r#"uset g_traffic\s+"[^"]+""#).map_err(|e| e.to_string())?;

    if !re.is_match(&content) {
        return Err("g_traffic nicht in config.cfg gefunden".into());
    }

    let new_content = re.replace(&content, format!(r#"uset g_traffic "{}""#, value));

    write_text_with_auto_backup(
        profile_state.inner(),
        &path,
        "edit_traffic_value",
        "before traffic config edit",
        "The global traffic config value was updated.",
        new_content.as_ref(),
        |_| Ok(()),
    )
    .map_err(|error| error.user_message)?;

    profile_cache.invalidate_base_config();
    decrypt_cache.invalidate_path(&path);
    dev_log!("Traffic erfolgreich geändert auf {}", value);
    Ok(())
}

#[command]
pub fn edit_parking_doubles_value(
    value: i64,
    profile_state: State<'_, AppProfileState>,
    profile_cache: State<'_, ProfileCache>,
    decrypt_cache: State<'_, DecryptCache>,
) -> Result<(), String> {
    let profile =
        current_profile_path(profile_state.inner()).map_err(|error| error.user_message)?;

    let path = quicksave_config_path(&profile);

    dev_log!("Schreibe Parking Doubles Value in: {}", path.display());

    let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;

    let re = Regex::new(r#"uset g_simple_parking_doubles\s+"[^"]+""#)
        .map_err(|error| format!("Parking-Regel ungültig: {}", error))?;

    if !re.is_match(&content) {
        return Err("uset g_simple_parking_doubles nicht in player/config.cfg gefunden".into());
    }

    let new_content = re.replace(
        &content,
        format!(r#"uset g_simple_parking_doubles "{}""#, value),
    );
    let expected = format!(r#"uset g_simple_parking_doubles "{}""#, value);

    write_text_with_auto_backup(
        profile_state.inner(),
        &path,
        "edit_parking_doubles_value",
        "before parking doubles edit",
        "The profile parking doubles config value was updated.",
        new_content.as_ref(),
        |written_path| {
            verify_contains(
                written_path,
                &expected,
                "Simple Parking Doubles-Wert konnte nicht verifiziert werden.",
            )
        },
    )
    .map_err(|error| error.user_message)?;

    profile_cache.invalidate_save_config();
    decrypt_cache.invalidate_path(&path);
    dev_log!("Parking Doubles erfolgreich geändert auf {}", value);
    Ok(())
}

#[derive(Deserialize)]
pub struct KeyValuePayload {
    key: String,
    value: String,
}

#[command]
pub fn edit_config_value(
    payload: KeyValuePayload,
    profile_state: State<'_, AppProfileState>,
    profile_cache: State<'_, ProfileCache>,
    decrypt_cache: State<'_, DecryptCache>,
) -> Result<(), String> {
    let path = ets2_base_config_path().ok_or("Globaler Config-Pfad nicht gefunden".to_string())?;
    let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let re = Regex::new(&format!(r#"uset {}\s*"?.*"?"#, payload.key))
        .map_err(|error| format!("Config-Regel ungültig: {}", error))?;
    let new_content = re.replace(
        &content,
        format!(r#"uset {} "{}""#, payload.key, payload.value),
    );
    write_text_with_auto_backup(
        profile_state.inner(),
        &path,
        "edit_config_value",
        &format!("before global config edit {}", payload.key),
        "A global config value was updated.",
        new_content.as_ref(),
        |_| Ok(()),
    )
    .map_err(|error| error.user_message)?;
    profile_cache.invalidate_base_config();
    decrypt_cache.invalidate_path(&path);
    dev_log!(
        "Globalen Config-Wert geändert: {} -> {}",
        payload.key,
        payload.value
    );
    Ok(())
}

#[command]
pub fn edit_save_config_value(
    payload: KeyValuePayload,
    profile_state: State<'_, AppProfileState>,
    profile_cache: State<'_, ProfileCache>,
    decrypt_cache: State<'_, DecryptCache>,
) -> Result<(), String> {
    let profile =
        current_profile_path(profile_state.inner()).map_err(|error| error.user_message)?;
    let path = quicksave_config_path(&profile);
    let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let re = Regex::new(&format!(r#"uset {}\s*"?.*"?"#, payload.key))
        .map_err(|error| format!("Save-Config-Regel ungültig: {}", error))?;
    let new_content = re.replace(
        &content,
        format!(r#"uset {} "{}""#, payload.key, payload.value),
    );
    write_text_with_auto_backup(
        profile_state.inner(),
        &path,
        "edit_save_config_value",
        &format!("before save config edit {}", payload.key),
        "A profile save config value was updated.",
        new_content.as_ref(),
        |_| Ok(()),
    )
    .map_err(|error| error.user_message)?;
    profile_cache.invalidate_save_config();
    decrypt_cache.invalidate_path(&path);
    dev_log!(
        "Profil-Config-Wert geändert: {} -> {}",
        payload.key,
        payload.value
    );
    Ok(())
}
