use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use chrono::Utc;
use rusqlite::{Connection, OptionalExtension, params};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::db::sqlite;
use crate::features::logging::service as logging_service;
use crate::shared::current_profile::snapshot_resolved_save_context;
use crate::shared::decrypt::decode_text_bytes;
use crate::shared::paths::{ets2_base_config_path, game_sii_from_save, info_sii_from_save};
use crate::state::AppProfileState;

use super::models::{
    BackupCreateResultDto, BackupDiffFileDto, BackupDiffValueDto, BackupFileRecord,
    BackupMetadataFile, BackupRestorePreviewDto, BackupRestoreResultDto, BackupVersionDto,
};

const BACKUP_METADATA_FILE: &str = "metadata.json";
pub const BACKUP_TYPE_AUTO: &str = "Auto";
pub const BACKUP_TYPE_MANUAL: &str = "Manual";
pub const BACKUP_TYPE_UNDO_BEFORE_EDIT: &str = "UndoBeforeEdit";

pub struct RestoreExecution {
    pub result: BackupRestoreResultDto,
    pub touched_paths: Vec<PathBuf>,
}

struct LoadedBackup {
    metadata: BackupMetadataFile,
    storage_dir: PathBuf,
}

pub fn recommended_targets(primary_path: &Path) -> Vec<PathBuf> {
    let mut targets = Vec::new();
    targets.push(primary_path.to_path_buf());

    let file_name = primary_path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    if file_name == "game.sii" {
        let sibling = info_sii_from_save(primary_path);
        if sibling.exists() {
            targets.push(sibling);
        }
    } else if file_name == "info.sii" {
        let sibling = game_sii_from_save(primary_path);
        if sibling.exists() {
            targets.push(sibling);
        }
    }

    dedupe_paths(targets)
}

pub fn create_backup_for_targets(
    profile_state: &AppProfileState,
    action_reason: &str,
    target_files: &[PathBuf],
) -> Result<BackupCreateResultDto, String> {
    create_backup_for_targets_with_type(
        profile_state,
        BACKUP_TYPE_AUTO,
        action_reason,
        target_files,
    )
}

pub fn create_backup_for_targets_with_type(
    profile_state: &AppProfileState,
    backup_type: &str,
    action_reason: &str,
    target_files: &[PathBuf],
) -> Result<BackupCreateResultDto, String> {
    let context = logging_service::resolve_active_context(profile_state);
    let resolved = snapshot_resolved_save_context(profile_state).ok();
    let normalized_targets = normalize_existing_targets(target_files);
    if normalized_targets.is_empty() {
        return Err("No existing files were available for backup.".to_string());
    }

    let created_at_utc = Utc::now().to_rfc3339();
    let backup_id = format!("backup-{}", Uuid::new_v4());
    let save_session_id = resolved
        .as_ref()
        .and_then(|item| item.context.save_session_id.clone())
        .or_else(|| Some(format!("manual-{}", Uuid::new_v4())));
    let storage_dir = backup_root_dir()
        .join(save_session_id.as_deref().unwrap_or("manual"))
        .join(&backup_id);
    fs::create_dir_all(&storage_dir).map_err(|error| error.to_string())?;

    let mut files = Vec::new();
    for live_path in normalized_targets {
        let bytes = fs::read(&live_path)
            .map_err(|error| format!("Backup could not read {}: {}", live_path.display(), error))?;
        let relative_path = relative_label_for_target(
            &live_path,
            resolved
                .as_ref()
                .and_then(|item| item.context.profile_reference.as_deref()),
            resolved
                .as_ref()
                .and_then(|item| item.context.save_reference.as_deref()),
        );
        let stored_path = PathBuf::from("files").join(&relative_path);
        let absolute_stored_path = storage_dir.join(&stored_path);
        if let Some(parent) = absolute_stored_path.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        fs::write(&absolute_stored_path, &bytes).map_err(|error| {
            format!(
                "Backup could not write {}: {}",
                absolute_stored_path.display(),
                error
            )
        })?;

        files.push(BackupFileRecord {
            relative_path,
            live_path: live_path.display().to_string(),
            stored_path: stored_path.to_string_lossy().replace('\\', "/"),
            size_bytes: bytes.len() as u64,
            checksum: sha256_hex(&bytes),
        });
    }

    let metadata = BackupMetadataFile {
        backup_id: backup_id.clone(),
        save_session_id: save_session_id.clone(),
        profile_reference: resolved
            .as_ref()
            .and_then(|item| item.context.profile_reference.clone()),
        save_reference: resolved
            .as_ref()
            .and_then(|item| item.context.save_reference.clone()),
        profile_name: context.profile_name.clone(),
        save_name: context.save_name.clone(),
        action_reason: action_reason.to_string(),
        backup_type: backup_type.to_string(),
        created_at_utc: created_at_utc.clone(),
        files: files.clone(),
    };

    let metadata_json =
        serde_json::to_string_pretty(&metadata).map_err(|error| error.to_string())?;
    fs::write(storage_dir.join(BACKUP_METADATA_FILE), metadata_json).map_err(|error| {
        format!(
            "Backup metadata could not be written to {}: {}",
            storage_dir.display(),
            error
        )
    })?;

    let conn = open_runtime_connection()?;
    conn.execute(
        r#"
        INSERT INTO ets_save_backups (
            backup_id, save_session_id, profile_reference, save_reference, profile_name, save_name,
            action_reason, backup_type, created_at_utc, storage_dir, files_json, file_count
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
        "#,
        params![
            metadata.backup_id,
            metadata.save_session_id,
            metadata.profile_reference,
            metadata.save_reference,
            metadata.profile_name,
            metadata.save_name,
            metadata.action_reason,
            metadata.backup_type,
            metadata.created_at_utc,
            storage_dir.display().to_string(),
            serde_json::to_string(&files).map_err(|error| error.to_string())?,
            files.len() as i64
        ],
    )
    .map_err(|error| error.to_string())?;

    let mut log_context = context;
    log_context
        .extra
        .insert("backupId".to_string(), backup_id.clone());
    log_context
        .extra
        .insert("backupType".to_string(), backup_type.to_string());
    log_context
        .extra
        .insert("fileCount".to_string(), files.len().to_string());
    let log_action = if backup_type == BACKUP_TYPE_UNDO_BEFORE_EDIT {
        "undo_snapshot_created"
    } else {
        "auto_backup_created"
    };
    let log_message = if backup_type == BACKUP_TYPE_UNDO_BEFORE_EDIT {
        format!("Undo snapshot created: {}.", action_reason)
    } else {
        format!("Automatic backup created before {}.", action_reason)
    };
    let _ = logging_service::record_info(log_action, &log_message, &log_context);

    Ok(BackupCreateResultDto {
        backup_id,
        created_at_utc,
        backup_type: backup_type.to_string(),
    })
}

pub fn list_backups_for_active_save(
    profile_state: &AppProfileState,
) -> Result<Vec<BackupVersionDto>, String> {
    let resolved = snapshot_resolved_save_context(profile_state).ok();
    let save_session_id = resolved
        .as_ref()
        .and_then(|item| item.context.save_session_id.clone());
    list_backups_for_save_session(save_session_id)
}

pub fn list_backups_for_save_session(
    save_session_id: Option<String>,
) -> Result<Vec<BackupVersionDto>, String> {
    let Some(save_session_id) = save_session_id else {
        return Ok(Vec::new());
    };

    let conn = open_runtime_connection()?;
    let mut stmt = conn
        .prepare(
            r#"
            SELECT backup_id, created_at_utc, profile_name, save_name, action_reason, file_count
                 , backup_type
            FROM ets_save_backups
            WHERE save_session_id = ?1
            ORDER BY created_at_utc DESC
            LIMIT 40
            "#,
        )
        .map_err(|error| error.to_string())?;

    let rows = stmt
        .query_map([save_session_id], |row| {
            Ok(BackupVersionDto {
                backup_id: row.get("backup_id")?,
                created_at_utc: row.get("created_at_utc")?,
                profile_name: row.get("profile_name")?,
                save_name: row.get("save_name")?,
                action_reason: row.get("action_reason")?,
                backup_type: row.get("backup_type")?,
                file_count: row.get::<_, i64>("file_count")? as usize,
            })
        })
        .map_err(|error| error.to_string())?;

    let mut backups = Vec::new();
    for row in rows {
        backups.push(row.map_err(|error| error.to_string())?);
    }
    Ok(backups)
}

pub fn build_restore_preview(backup_id: &str) -> Result<BackupRestorePreviewDto, String> {
    let loaded = load_backup(backup_id)?;
    let metadata = loaded.metadata;
    let mut files = Vec::new();
    let mut notes = Vec::new();

    for file in &metadata.files {
        let backup_bytes =
            fs::read(loaded.storage_dir.join(&file.stored_path)).map_err(|error| {
                format!(
                    "The stored backup file {} could not be read: {}",
                    file.relative_path, error
                )
            })?;
        let current_bytes = fs::read(&file.live_path).ok();
        let checksum_before = current_bytes
            .as_deref()
            .map(sha256_hex)
            .unwrap_or_else(|| "-".to_string());
        let checksum_after = sha256_hex(&backup_bytes);

        let status = match current_bytes.as_deref() {
            Some(bytes) if sha256_hex(bytes) == checksum_after => "identical",
            Some(_) => "changed",
            None => "missing_current",
        }
        .to_string();

        if status == "missing_current" {
            notes.push(format!(
                "The live file for {} is currently missing and would be recreated.",
                file.relative_path
            ));
        }

        let current_text = current_bytes
            .as_deref()
            .and_then(|bytes| decode_text_bytes(bytes, &file.live_path, &[]).ok());
        let backup_text = decode_text_bytes(&backup_bytes, &file.relative_path, &[]).ok();

        let changes = match (current_text.as_deref(), backup_text.as_deref()) {
            (Some(before), Some(after)) => build_value_diffs(before, after),
            _ => Vec::new(),
        };

        files.push(BackupDiffFileDto {
            relative_path: file.relative_path.clone(),
            status,
            parseable: !changes.is_empty(),
            change_count: changes.len(),
            checksum_before,
            checksum_after,
            changes,
        });
    }

    Ok(BackupRestorePreviewDto {
        backup_id: metadata.backup_id,
        created_at_utc: metadata.created_at_utc,
        profile_name: metadata.profile_name,
        save_name: metadata.save_name,
        action_reason: metadata.action_reason,
        backup_type: metadata.backup_type,
        files,
        notes,
    })
}

pub fn find_latest_backup_for_active_save_by_type(
    profile_state: &AppProfileState,
    backup_type: &str,
) -> Result<Option<BackupVersionDto>, String> {
    let resolved = snapshot_resolved_save_context(profile_state).ok();
    let save_session_id = resolved
        .as_ref()
        .and_then(|item| item.context.save_session_id.clone());
    find_latest_backup_for_save_session_by_type(save_session_id, backup_type)
}

pub fn find_latest_backup_for_save_session_by_type(
    save_session_id: Option<String>,
    backup_type: &str,
) -> Result<Option<BackupVersionDto>, String> {
    let Some(save_session_id) = save_session_id else {
        return Ok(None);
    };

    let conn = open_runtime_connection()?;
    conn.query_row(
        r#"
        SELECT backup_id, created_at_utc, profile_name, save_name, action_reason, backup_type, file_count
        FROM ets_save_backups
        WHERE save_session_id = ?1 AND backup_type = ?2
        ORDER BY created_at_utc DESC
        LIMIT 1
        "#,
        params![save_session_id, backup_type],
        |row| {
            Ok(BackupVersionDto {
                backup_id: row.get("backup_id")?,
                created_at_utc: row.get("created_at_utc")?,
                profile_name: row.get("profile_name")?,
                save_name: row.get("save_name")?,
                action_reason: row.get("action_reason")?,
                backup_type: row.get("backup_type")?,
                file_count: row.get::<_, i64>("file_count")? as usize,
            })
        },
    )
    .optional()
    .map_err(|error| error.to_string())
}

pub fn get_backup_storage_dir(backup_id: &str) -> Result<PathBuf, String> {
    Ok(load_backup(backup_id)?.storage_dir)
}

pub fn restore_backup(
    profile_state: &AppProfileState,
    backup_id: &str,
    confirmed: bool,
) -> Result<RestoreExecution, String> {
    if !confirmed {
        return Err("Restore requires explicit confirmation.".to_string());
    }

    let loaded = load_backup(backup_id)?;
    let metadata = loaded.metadata;
    let live_targets = metadata
        .files
        .iter()
        .map(|file| PathBuf::from(&file.live_path))
        .collect::<Vec<_>>();
    let safety_backup = create_backup_for_targets(
        profile_state,
        &format!("before restore {}", metadata.action_reason),
        &live_targets,
    )
    .ok();

    let mut touched_paths = Vec::new();
    for file in &metadata.files {
        let source = loaded.storage_dir.join(&file.stored_path);
        let target = PathBuf::from(&file.live_path);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        fs::copy(&source, &target).map_err(|error| {
            format!(
                "Restore could not copy {} to {}: {}",
                source.display(),
                target.display(),
                error
            )
        })?;
        touched_paths.push(target);
    }

    let mut context = logging_service::resolve_active_context(profile_state);
    context
        .extra
        .insert("backupId".to_string(), backup_id.to_string());
    if let Some(safety_backup) = safety_backup.as_ref() {
        context.extra.insert(
            "safetyBackupId".to_string(),
            safety_backup.backup_id.clone(),
        );
    }
    let _ = logging_service::record_info(
        "backup_restore",
        "Backup restored after explicit confirmation.",
        &context,
    );

    Ok(RestoreExecution {
        result: BackupRestoreResultDto {
            backup_id: backup_id.to_string(),
            restored_file_count: metadata.files.len(),
            safety_backup_id: safety_backup.map(|item| item.backup_id),
        },
        touched_paths,
    })
}

fn load_backup(backup_id: &str) -> Result<LoadedBackup, String> {
    let conn = open_runtime_connection()?;
    let row = conn
        .query_row(
            r#"
            SELECT storage_dir, files_json, backup_id, save_session_id, profile_reference, save_reference,
                   profile_name, save_name, action_reason, backup_type, created_at_utc
            FROM ets_save_backups
            WHERE backup_id = ?1
            "#,
            [backup_id],
            |row| {
                Ok((
                    row.get::<_, String>("storage_dir")?,
                    row.get::<_, String>("files_json")?,
                    row.get::<_, String>("backup_id")?,
                    row.get::<_, Option<String>>("save_session_id")?,
                    row.get::<_, Option<String>>("profile_reference")?,
                    row.get::<_, Option<String>>("save_reference")?,
                    row.get::<_, Option<String>>("profile_name")?,
                    row.get::<_, Option<String>>("save_name")?,
                    row.get::<_, String>("action_reason")?,
                    row.get::<_, String>("backup_type")?,
                    row.get::<_, String>("created_at_utc")?,
                ))
            },
        )
        .optional()
        .map_err(|error| error.to_string())?;

    let Some((
        storage_dir,
        files_json,
        backup_id,
        save_session_id,
        profile_reference,
        save_reference,
        profile_name,
        save_name,
        action_reason,
        backup_type,
        created_at_utc,
    )) = row
    else {
        return Err(format!("Backup {} was not found.", backup_id));
    };

    let files = serde_json::from_str::<Vec<BackupFileRecord>>(&files_json)
        .map_err(|error| error.to_string())?;
    let storage_dir_path = PathBuf::from(&storage_dir);
    let metadata_path = storage_dir_path.join(BACKUP_METADATA_FILE);
    if metadata_path.exists() {
        if let Ok(content) = fs::read_to_string(&metadata_path) {
            if let Ok(metadata) = serde_json::from_str::<BackupMetadataFile>(&content) {
                return Ok(LoadedBackup {
                    metadata,
                    storage_dir: storage_dir_path.clone(),
                });
            }
        }
    }

    Ok(LoadedBackup {
        metadata: BackupMetadataFile {
            backup_id,
            save_session_id,
            profile_reference,
            save_reference,
            profile_name,
            save_name,
            action_reason,
            backup_type,
            created_at_utc,
            files,
        },
        storage_dir: storage_dir_path,
    })
}

fn relative_label_for_target(
    target: &Path,
    profile_reference: Option<&str>,
    save_reference: Option<&str>,
) -> String {
    let normalized_target = target.display().to_string().replace('\\', "/");
    let normalized_target_lower = normalized_target.to_ascii_lowercase();

    if let Some(save_reference) = save_reference {
        let normalized_save = save_reference.replace('\\', "/");
        if normalized_target_lower.starts_with(&normalized_save.to_ascii_lowercase()) {
            let file_name = target
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("game.sii");
            return format!("save/{}", file_name);
        }
    }

    if let Some(profile_reference) = profile_reference {
        let normalized_profile = profile_reference.replace('\\', "/");
        if normalized_target_lower.starts_with(&normalized_profile.to_ascii_lowercase()) {
            let file_name = target
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("config.cfg");
            return format!("profile/{}", file_name);
        }
    }

    if ets2_base_config_path()
        .as_ref()
        .map(|path| path == target)
        .unwrap_or(false)
    {
        return "global/config.cfg".to_string();
    }

    let fallback_name = target
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("file.bin");
    format!("other/{}", fallback_name)
}

fn normalize_existing_targets(targets: &[PathBuf]) -> Vec<PathBuf> {
    let mut seen = HashSet::new();
    let mut normalized = Vec::new();
    for target in targets {
        if !target.exists() {
            continue;
        }
        let key = target.display().to_string().replace('\\', "/");
        if seen.insert(key) {
            normalized.push(target.clone());
        }
    }
    normalized
}

fn dedupe_paths(targets: Vec<PathBuf>) -> Vec<PathBuf> {
    normalize_existing_targets(&targets)
}

fn build_value_diffs(before: &str, after: &str) -> Vec<BackupDiffValueDto> {
    let before_values = extract_key_values(before);
    let after_values = extract_key_values(after);
    let mut keys = before_values
        .keys()
        .chain(after_values.keys())
        .cloned()
        .collect::<Vec<_>>();
    keys.sort();
    keys.dedup();

    keys.into_iter()
        .filter_map(|key| {
            let previous = before_values.get(&key).cloned().unwrap_or_default();
            let next = after_values.get(&key).cloned().unwrap_or_default();
            if previous == next {
                return None;
            }
            Some(BackupDiffValueDto {
                key,
                previous_value: previous,
                next_value: next,
            })
        })
        .take(40)
        .collect()
}

fn extract_key_values(text: &str) -> BTreeMap<String, String> {
    let mut values = BTreeMap::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        if let Some(rest) = trimmed.strip_prefix("uset ") {
            if let Some((key, value)) = rest.split_once(' ') {
                values.insert(
                    key.trim().to_string(),
                    value.trim().trim_matches('"').to_string(),
                );
                continue;
            }
        }

        if let Some((key, value)) = trimmed.split_once(':') {
            let normalized_key = key.trim();
            let normalized_value = value.trim();
            if !normalized_key.is_empty() && !normalized_value.is_empty() {
                values.insert(normalized_key.to_string(), normalized_value.to_string());
            }
        }
    }
    values
}

fn open_runtime_connection() -> Result<Connection, String> {
    let db_path = sqlite::app_db_path();
    let conn = Connection::open(db_path).map_err(|error| error.to_string())?;
    conn.busy_timeout(Duration::from_secs(5))
        .map_err(|error| error.to_string())?;
    Ok(conn)
}

fn backup_root_dir() -> PathBuf {
    sqlite::app_db_path()
        .parent()
        .map(|path| path.join("save_backups"))
        .unwrap_or_else(|| PathBuf::from("save_backups"))
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}
