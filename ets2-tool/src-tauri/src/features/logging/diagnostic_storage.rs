use super::{DiagnosticRuntime, LOG_FILE, TOTAL_LIMIT, checked_path};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupPreview {
    pub scope: String,
    pub token: String,
    pub bytes: u64,
    pub files: usize,
}

fn cache_files(
    runtime: &DiagnosticRuntime,
    scope: &str,
) -> Result<Vec<(PathBuf, u64, SystemTime)>, String> {
    if scope != "cache" {
        return Err("diagnostics.errors.invalid_cleanup".into());
    }
    let directory = checked_path(&runtime.root, Path::new("cache"))?;
    if !directory.exists() {
        return Ok(Vec::new());
    }
    let mut files = Vec::new();
    // Only this logger-owned temporary directory is eligible. No app config or game paths.
    for (index, entry) in walkdir::WalkDir::new(directory)
        .follow_links(false)
        .max_depth(12)
        .into_iter()
        .filter_entry(|entry| {
            !entry.file_type().is_dir()
                || ![
                    "profiles",
                    "save",
                    "saves",
                    "backups",
                    "save_backups",
                    "settings",
                ]
                .contains(
                    &entry
                        .file_name()
                        .to_string_lossy()
                        .to_ascii_lowercase()
                        .as_str(),
                )
        })
        .enumerate()
    {
        if index >= 10000 {
            return Err("diagnostics.errors.storage_limit".into());
        }
        let entry = entry.map_err(|e| e.to_string())?;
        let relative = entry
            .path()
            .strip_prefix(&runtime.root)
            .map_err(|_| "diagnostics.errors.unsafe_path")?;
        checked_path(&runtime.root, relative)?;
        if entry.file_type().is_file()
            && entry
                .path()
                .extension()
                .and_then(|s| s.to_str())
                .is_some_and(|s| ["tmp", "cache"].contains(&s.to_ascii_lowercase().as_str()))
        {
            let metadata = entry.metadata().map_err(|e| e.to_string())?;
            files.push((
                entry.path().to_owned(),
                metadata.len(),
                metadata.modified().map_err(|e| e.to_string())?,
            ));
        }
    }
    files.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(files)
}
fn preview_for(
    runtime: &DiagnosticRuntime,
    files: &[(PathBuf, u64, SystemTime)],
) -> CleanupPreview {
    let mut digest = Sha256::new();
    digest.update(runtime.session_id.as_bytes());
    for (path, size, time) in files {
        digest.update(path.to_string_lossy().as_bytes());
        digest.update(size.to_le_bytes());
        digest.update(format!("{time:?}").as_bytes());
    }
    CleanupPreview {
        scope: "cache".into(),
        token: format!("{:x}", digest.finalize()),
        bytes: files.iter().map(|(_, size, _)| size).sum(),
        files: files.len(),
    }
}
pub fn preview(runtime: &DiagnosticRuntime, scope: &str) -> Result<CleanupPreview, String> {
    let _guard = runtime
        .maintenance
        .lock()
        .map_err(|_| "diagnostics.errors.unavailable")?;
    Ok(preview_for(runtime, &cache_files(runtime, scope)?))
}
pub fn cleanup(
    runtime: &DiagnosticRuntime,
    scope: &str,
    token: &str,
    confirmed: bool,
) -> Result<u64, String> {
    if !confirmed {
        return Err("diagnostics.errors.confirmation_required".into());
    }
    let _guard = runtime
        .maintenance
        .lock()
        .map_err(|_| "diagnostics.errors.unavailable")?;
    let files = cache_files(runtime, scope)?;
    if preview_for(runtime, &files).token != token {
        return Err("diagnostics.errors.cleanup_changed".into());
    }
    let mut freed = 0;
    for (path, size, _) in files {
        let relative = path
            .strip_prefix(&runtime.root)
            .map_err(|_| "diagnostics.errors.unsafe_path")?;
        fs::remove_file(checked_path(&runtime.root, relative)?).map_err(|e| e.to_string())?;
        freed += size;
    }
    Ok(freed)
}

/// Retain only managed TXT archives for at most 14 days and 20 MiB including the active log.
/// Existing legacy reports, game data, configuration and exports are never cleanup candidates.
pub fn automatic(runtime: &DiagnosticRuntime) -> Result<(), String> {
    let directory = checked_path(&runtime.root, Path::new("logs/archive"))?;
    if !directory.exists() {
        return Ok(());
    }
    let mut archives = Vec::new();
    for entry in fs::read_dir(directory)
        .map_err(|e| e.to_string())?
        .take(10001)
    {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if !(name.starts_with("application-") || name.starts_with("emergency-"))
            || !name.ends_with(".txt")
        {
            continue;
        }
        let path = entry.path();
        checked_path(
            &runtime.root,
            path.strip_prefix(&runtime.root)
                .map_err(|_| "diagnostics.errors.unsafe_path")?,
        )?;
        let metadata = entry.metadata().map_err(|e| e.to_string())?;
        if metadata.is_file() {
            archives.push((
                path,
                metadata.len(),
                metadata.modified().map_err(|e| e.to_string())?,
            ));
        }
    }
    archives.sort_by_key(|(_, _, time)| *time);
    let active = checked_path(&runtime.root, Path::new(LOG_FILE))?;
    let mut total = fs::metadata(active).map(|m| m.len()).unwrap_or(0)
        + archives.iter().map(|(_, size, _)| size).sum::<u64>();
    let cutoff = SystemTime::now()
        .checked_sub(Duration::from_secs(14 * 86400))
        .unwrap_or(SystemTime::UNIX_EPOCH);
    for (path, size, time) in archives {
        if time < cutoff || total > TOTAL_LIMIT {
            let relative = path
                .strip_prefix(&runtime.root)
                .map_err(|_| "diagnostics.errors.unsafe_path")?;
            fs::remove_file(checked_path(&runtime.root, relative)?).map_err(|e| e.to_string())?;
            total = total.saturating_sub(size);
        }
    }
    Ok(())
}
