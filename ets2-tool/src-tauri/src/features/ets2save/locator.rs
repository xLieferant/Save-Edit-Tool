use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use sqlx::SqlitePool;

use crate::features::ets2save::errors::{AppError, AppErrorCode};
use crate::features::ets2save::models::{EtsProfile, EtsSaveSlot};
use crate::shared::paths::{ats_base_path, ets2_base_path, quicksave_game_path};
use crate::state::AppProfileState;

fn normalize_profile_id(profile_id: &str) -> String {
    profile_id.trim().replace('\\', "/")
}

fn infer_game(profile_path: &str, selected_game: Option<String>) -> String {
    if let Some(selected_game) = selected_game {
        return selected_game;
    }

    let normalized = profile_path.to_ascii_lowercase();
    if normalized.contains("american truck simulator") {
        "ats".to_string()
    } else {
        "ets2".to_string()
    }
}

fn resolve_profile_candidate(profile_id: &str, state: &AppProfileState) -> Option<String> {
    let requested = profile_id.trim();
    if !requested.is_empty() && Path::new(requested).exists() {
        return Some(normalize_profile_id(requested));
    }

    let current_profile = state
        .current_profile
        .lock()
        .ok()
        .and_then(|guard| guard.clone());
    if let Some(current_profile) = current_profile {
        let normalized_current = normalize_profile_id(&current_profile);
        if requested.is_empty()
            || normalized_current == normalize_profile_id(requested)
            || Path::new(&normalized_current)
                .file_name()
                .and_then(|value| value.to_str())
                .map(|value| value.eq_ignore_ascii_case(requested))
                .unwrap_or(false)
        {
            return Some(normalized_current);
        }
    }

    let bases = [ets2_base_path(), ats_base_path()];
    for base in bases.into_iter().flatten() {
        for root in [base.join("profiles"), base.join("profiles.backup")] {
            if !root.exists() {
                continue;
            }

            if let Ok(entries) = fs::read_dir(root) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if !path.is_dir() {
                        continue;
                    }
                    let matches = path
                        .file_name()
                        .and_then(|value| value.to_str())
                        .map(|value| value.eq_ignore_ascii_case(requested))
                        .unwrap_or(false);
                    if matches {
                        return Some(normalize_profile_id(&path.display().to_string()));
                    }
                }
            }
        }
    }

    None
}

fn timestamp_to_rfc3339(metadata: &fs::Metadata) -> String {
    metadata
        .modified()
        .ok()
        .map(DateTime::<Utc>::from)
        .unwrap_or_else(Utc::now)
        .to_rfc3339()
}

async fn upsert_profile(
    pool: &SqlitePool,
    profile_id: &str,
    profile_path: &str,
    game: &str,
) -> Result<EtsProfile, AppError> {
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        r#"
        INSERT INTO ets_profiles (
            profile_id,
            profile_path,
            game,
            steam_cloud_enabled,
            created_at_utc,
            updated_at_utc
        )
        VALUES (?1, ?2, ?3, 0, ?4, ?4)
        ON CONFLICT(profile_id) DO UPDATE SET
            profile_path = excluded.profile_path,
            game = excluded.game,
            updated_at_utc = excluded.updated_at_utc
        "#,
    )
    .bind(profile_id)
    .bind(profile_path)
    .bind(game)
    .bind(&now)
    .execute(pool)
    .await?;

    sqlx::query_as::<_, EtsProfile>(
        r#"
        SELECT
            profile_id,
            profile_path,
            game,
            steam_cloud_enabled,
            created_at_utc,
            updated_at_utc
        FROM ets_profiles
        WHERE profile_id = ?1
        "#,
    )
    .bind(profile_id)
    .fetch_one(pool)
    .await
    .map_err(Into::into)
}

async fn upsert_save_slot(
    pool: &SqlitePool,
    profile_id: &str,
    slot_name: &str,
    save_path: &str,
    game_sii_path: &str,
    modified_at_utc: &str,
) -> Result<EtsSaveSlot, AppError> {
    let save_id = format!("{}::{}", profile_id, save_path.replace('\\', "/"));
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        r#"
        INSERT INTO ets_saves (
            save_id,
            profile_id,
            slot_name,
            save_path,
            game_sii_path,
            is_quicksave,
            modified_at_utc,
            created_at_utc,
            updated_at_utc,
            last_loaded_at_utc
        )
        VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6, ?7, ?7, ?7)
        ON CONFLICT(profile_id, save_path) DO UPDATE SET
            game_sii_path = excluded.game_sii_path,
            modified_at_utc = excluded.modified_at_utc,
            updated_at_utc = excluded.updated_at_utc,
            last_loaded_at_utc = excluded.last_loaded_at_utc
        "#,
    )
    .bind(&save_id)
    .bind(profile_id)
    .bind(slot_name)
    .bind(save_path)
    .bind(game_sii_path)
    .bind(modified_at_utc)
    .bind(&now)
    .execute(pool)
    .await?;

    sqlx::query_as::<_, EtsSaveSlot>(
        r#"
        SELECT
            save_id,
            profile_id,
            slot_name,
            save_path,
            game_sii_path,
            is_quicksave,
            modified_at_utc,
            created_at_utc,
            updated_at_utc,
            last_loaded_at_utc
        FROM ets_saves
        WHERE profile_id = ?1 AND save_path = ?2
        "#,
    )
    .bind(profile_id)
    .bind(save_path)
    .fetch_one(pool)
    .await
    .map_err(Into::into)
}

pub async fn resolve_last_quicksave(
    pool: &SqlitePool,
    profile_id: &str,
    state: &AppProfileState,
) -> Result<(EtsProfile, EtsSaveSlot), AppError> {
    let profile_path = resolve_profile_candidate(profile_id, state).ok_or_else(|| {
        AppError::new(
            AppErrorCode::ProfileNotFound,
            format!("Profile not found: {}", profile_id),
        )
    })?;

    let requested_profile_id = if profile_id.trim().is_empty() {
        profile_path.clone()
    } else {
        normalize_profile_id(profile_id)
    };

    let selected_game = state.selected_game.lock().ok().map(|guard| guard.clone());
    let profile = upsert_profile(
        pool,
        &requested_profile_id,
        &profile_path,
        &infer_game(&profile_path, selected_game),
    )
    .await?;

    let quicksave_dir = state
        .current_save
        .lock()
        .ok()
        .and_then(|guard| guard.clone())
        .filter(|save_path| normalize_profile_id(save_path).contains("quicksave"))
        .map(PathBuf::from)
        .unwrap_or_else(|| quicksave_game_path(&profile_path));

    let game_sii_path = if quicksave_dir.is_dir() {
        quicksave_dir.join("game.sii")
    } else {
        quicksave_dir
    };

    if !game_sii_path.exists() {
        return Err(AppError::new(
            AppErrorCode::SaveNotFound,
            format!("Quicksave not found for profile {}", profile.profile_id),
        ));
    }

    let metadata = fs::metadata(&game_sii_path).map_err(|error| {
        AppError::new(
            AppErrorCode::SaveNotFound,
            format!("Quicksave metadata unavailable: {}", error),
        )
    })?;
    let modified_at_utc = timestamp_to_rfc3339(&metadata);
    let save_folder = game_sii_path
        .parent()
        .ok_or_else(|| AppError::new(AppErrorCode::SaveNotFound, "Invalid quicksave path"))?;
    let save_slot = upsert_save_slot(
        pool,
        &profile.profile_id,
        "quicksave",
        &normalize_profile_id(&save_folder.display().to_string()),
        &normalize_profile_id(&game_sii_path.display().to_string()),
        &modified_at_utc,
    )
    .await?;

    Ok((profile, save_slot))
}
