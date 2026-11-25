//! Shared selection checks and optimistic, verified atomic writes for both games.
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

use super::document::Document;
use crate::features::ets2save::sii_codec::replace_file_atomic;
use crate::shared::decrypt::{decode_text_bytes, decrypt_if_needed};
use crate::shared::game::GameType;
use crate::state::AppProfileState;

const IO_ATTEMPTS: usize = 5;

fn retryable(error: &std::io::Error) -> bool {
    matches!(error.raw_os_error(), Some(32 | 33))
        || matches!(
            error.kind(),
            std::io::ErrorKind::PermissionDenied
                | std::io::ErrorKind::WouldBlock
                | std::io::ErrorKind::Interrupted
        )
}

fn io_failure(operation: &str, path: &Path, error: &std::io::Error) -> String {
    let _ = crate::shared::user_log::user_log_error(
        "SaveWrite",
        format!(
            "operation={operation} path={} os_code={:?} error={error}",
            path.display(),
            error.raw_os_error()
        ),
    );
    if error.kind() == std::io::ErrorKind::NotFound {
        "save_errors.not_found"
    } else if retryable(error) {
        "save_errors.locked"
    } else {
        "save_errors.write_failed"
    }
    .into()
}

fn backoff(attempt: usize) {
    std::thread::sleep(Duration::from_millis(40 << attempt));
}

pub(crate) fn read_retry(path: &Path) -> Result<Vec<u8>, String> {
    crate::shared::decrypt::read_file_retry(path)
}

/// Two identical byte and metadata observations are required before editing.
/// This also catches replacements that retain both length and modification time.
pub(crate) fn read_stable_bytes(path: &Path) -> Result<Vec<u8>, String> {
    for attempt in 0..IO_ATTEMPTS {
        let before = fs::metadata(path).map_err(|error| io_failure("metadata", path, &error))?;
        let raw = read_retry(path)?;
        std::thread::sleep(Duration::from_millis(60));
        let after = fs::metadata(path).map_err(|error| io_failure("metadata", path, &error))?;
        if before.len() == after.len()
            && before.modified().ok() == after.modified().ok()
            && read_retry(path)? == raw
        {
            return Ok(raw);
        }
        if attempt + 1 < IO_ATTEMPTS {
            backoff(attempt);
        }
    }
    Err("save_errors.writing".into())
}

fn read_validated_bytes(path: &Path) -> Result<Vec<u8>, String> {
    let mut failure = "save_errors.decrypt_failed";
    for attempt in 0..3 {
        let raw = read_stable_bytes(path)?;
        let result = decode_text_bytes(&raw, "active_save", &[]).and_then(|text| {
            failure = if text.trim_end().ends_with('}') {
                "save_errors.verification_failed"
            } else {
                "save_errors.writing"
            };
            Document::parse(&text).map(|_| ())
        });
        match result {
            Ok(()) => return Ok(raw),
            Err(error) => {
                let _ = crate::shared::user_log::user_log_warn(
                    "SaveWrite",
                    format!(
                        "operation=validate_source path={} attempt={} error={error}",
                        path.display(),
                        attempt + 1
                    ),
                );
                if attempt < 2 {
                    backoff(attempt + 1);
                }
            }
        }
    }
    Err(failure.into())
}

/// Lock the source while comparing it. Windows requires releasing that handle
/// before replacement; recheck the baseline immediately after releasing it.
pub(crate) fn replace_checked(
    temporary: &Path,
    path: &Path,
    expected: Option<&[u8]>,
) -> Result<(), String> {
    for attempt in 0..IO_ATTEMPTS {
        let result = (|| -> Result<(), std::io::Error> {
            let mut options = fs::OpenOptions::new();
            options.read(true).write(true);
            let mut guard = match options.open(path) {
                Ok(file) => Some(file),
                Err(error)
                    if error.kind() == std::io::ErrorKind::NotFound && expected.is_none() =>
                {
                    None
                }
                Err(error) => return Err(error),
            };
            let actual = if let Some(file) = guard.as_mut() {
                file.try_lock().map_err(|error| match error {
                    std::fs::TryLockError::WouldBlock => {
                        std::io::Error::from(std::io::ErrorKind::WouldBlock)
                    }
                    std::fs::TryLockError::Error(error) => error,
                })?;
                let mut bytes = Vec::new();
                std::io::Read::read_to_end(file, &mut bytes)?;
                Some(bytes)
            } else {
                None
            };
            if actual.as_deref() != expected {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "save_changed_since_load",
                ));
            }
            drop(guard);
            let actual = match fs::read(path) {
                Ok(bytes) => Some(bytes),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
                Err(error) => return Err(error),
            };
            if actual.as_deref() != expected {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "save_changed_since_load",
                ));
            }
            crate::features::ets2save::sii_codec::replace_file_atomic_io(temporary, path)
        })();
        match result {
            Ok(()) => return Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::InvalidData => {
                return Err("save_changed_since_load".into());
            }
            Err(error) if retryable(&error) && attempt + 1 < IO_ATTEMPTS => {
                let _ = crate::shared::user_log::user_log_warn(
                    "SaveWrite",
                    format!(
                        "save_errors.temporary_lock operation=replace path={} attempt={} os_code={:?} error={error}",
                        path.display(),
                        attempt + 1,
                        error.raw_os_error()
                    ),
                );
                backoff(attempt);
            }
            Err(error) => return Err(io_failure("replace", path, &error)),
        }
    }
    unreachable!()
}

static MUTATION_LOCK: Mutex<()> = Mutex::new(());

#[cfg(test)]
pub(crate) static TEST_COMMAND_LOCK: Mutex<()> = Mutex::new(());

pub fn mutation_lock() -> Result<MutexGuard<'static, ()>, String> {
    MUTATION_LOCK
        .try_lock()
        .map_err(|_| "save_mutation_in_progress".to_string())
}

pub fn selected_game(state: &AppProfileState) -> Result<GameType, String> {
    GameType::try_from(
        state
            .selected_game
            .lock()
            .map_err(|_| "game_state_unavailable")?
            .as_str(),
    )
}

pub fn config_path(state: &AppProfileState) -> Result<PathBuf, String> {
    crate::shared::paths::resolve_game_user_paths(selected_game(state)?.as_str())
        .map(|paths| paths.config_file)
        .ok_or_else(|| "game_path_unavailable".into())
}

pub fn validate_profile_game(profile: &Path, game: GameType) -> Result<(), String> {
    // Check explicit game directory / Steam app identity before inspecting profile data.
    for part in profile
        .components()
        .filter_map(|part| part.as_os_str().to_str())
    {
        let identified = match part.to_ascii_lowercase().as_str() {
            "american truck simulator" | "270880" => Some(GameType::Ats),
            "euro truck simulator 2" | "227300" => Some(GameType::Ets2),
            _ => None,
        };
        if identified.is_some_and(|identity| identity != game) {
            return Err("profile_game_mismatch".into());
        }
    }
    let path = profile.join("profile.sii");
    if path.is_file() {
        let text = decrypt_if_needed(&path)?;
        let doc = Document::parse(&text)?;
        let unit = doc.unique("user_profile")?;
        if let Ok(map) = doc.field(unit, "map_path") {
            let other_map = match game {
                GameType::Ats => "/map/europe.mbd",
                GameType::Ets2 => "/map/usa.mbd",
            };
            if map.trim_matches('"').eq_ignore_ascii_case(other_map) {
                return Err("profile_game_mismatch".into());
            }
        }
    }
    Ok(())
}

pub fn resolve_save(profile: &Path, save: &Path) -> Result<PathBuf, String> {
    let root = fs::canonicalize(profile.join("save")).map_err(|_| "profile_save_root_missing")?;
    let game = crate::shared::paths::game_sii_from_save(save);
    let game = fs::canonicalize(game).map_err(|_| "game_sii_not_found")?;
    let directory = game.parent().ok_or("save_path_invalid")?;
    if directory.parent() != Some(root.as_path()) {
        return Err("save_outside_loaded_profile".into());
    }
    Ok(game)
}

pub fn active_save(state: &AppProfileState) -> Result<PathBuf, String> {
    let profile = state
        .current_profile
        .lock()
        .map_err(|_| "profile_state_unavailable")?
        .clone()
        .ok_or("profile_not_loaded")?;
    let save = state
        .current_save
        .lock()
        .map_err(|_| "save_state_unavailable")?
        .clone()
        .ok_or("save_not_loaded")?;
    validate_profile_game(Path::new(&profile), selected_game(state)?)?;
    resolve_save(Path::new(&profile), Path::new(&save))
}

pub(crate) struct ReadRevision {
    path: PathBuf,
    fingerprint: crate::state::FileFingerprint,
    info_fingerprint: Option<crate::state::FileFingerprint>,
}

pub(crate) fn read_revision(state: &AppProfileState, path: &Path) -> Result<ReadRevision, String> {
    let selected = active_save(state)?;
    if fs::canonicalize(path).map_err(|_| "game_sii_not_found")? != selected {
        return Err("save_selection_changed".into());
    }
    Ok(ReadRevision {
        fingerprint: crate::state::FileFingerprint::read(&selected)?,
        info_fingerprint: crate::state::FileFingerprint::read(&selected.with_file_name("info.sii"))
            .ok(),
        path: selected,
    })
}

pub(crate) fn ensure_read_current(
    state: &AppProfileState,
    revision: &ReadRevision,
) -> Result<(), String> {
    if active_save(state)? != revision.path
        || crate::state::FileFingerprint::read(&revision.path)? != revision.fingerprint
        || crate::state::FileFingerprint::read(&revision.path.with_file_name("info.sii")).ok()
            != revision.info_fingerprint
    {
        return Err("save_changed_during_read".into());
    }
    Ok(())
}

pub fn ensure_game_closed(game: GameType) -> Result<(), String> {
    #[cfg(test)]
    if let Some(running) = TEST_GAME_RUNNING.with(std::cell::Cell::get) {
        return if running {
            Err("game_running".into())
        } else {
            Ok(())
        };
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let exe = match game {
            GameType::Ets2 => "eurotrucks2.exe",
            GameType::Ats => "amtrucks.exe",
        };
        let output = std::process::Command::new("tasklist.exe")
            .args(["/FI", &format!("IMAGENAME eq {exe}"), "/FO", "CSV", "/NH"])
            .creation_flags(0x08000000)
            .output()
            .map_err(|_| "game_status_unavailable")?;
        if !output.status.success() {
            return Err("game_status_unavailable".into());
        }
        if String::from_utf8_lossy(&output.stdout)
            .to_ascii_lowercase()
            .contains(exe)
        {
            return Err("game_running".into());
        }
    }
    #[cfg(not(windows))]
    {
        let name = match game {
            GameType::Ets2 => "eurotrucks2",
            GameType::Ats => "amtrucks",
        };
        let result = std::process::Command::new("pgrep")
            .args(["-x", name])
            .status()
            .map_err(|_| "game_status_unavailable")?;
        match result.code() {
            Some(1) => {}
            Some(0) => return Err("game_running".into()),
            _ => return Err("game_status_unavailable".into()),
        }
    }
    Ok(())
}

#[cfg(test)]
thread_local! {
    static TEST_GAME_RUNNING: std::cell::Cell<Option<bool>> = const { std::cell::Cell::new(None) };
}

/// Mock only the external process probe for isolated IPC fixture slots.
/// Production builds always execute the OS process check above.
#[cfg(test)]
pub(crate) fn with_test_game_running<T>(running: bool, operation: impl FnOnce() -> T) -> T {
    struct Reset(Option<bool>);
    impl Drop for Reset {
        fn drop(&mut self) {
            TEST_GAME_RUNNING.with(|probe| probe.set(self.0));
        }
    }
    let _reset = Reset(TEST_GAME_RUNNING.with(|probe| probe.replace(Some(running))));
    operation()
}

pub fn remember_save(state: &AppProfileState, path: &Path) -> Result<(), String> {
    let bytes = read_retry(path)?;
    *state
        .save_baseline
        .lock()
        .map_err(|_| "save_state_unavailable")? = Some((
        path.to_path_buf(),
        crate::shared::ets2data::validate::sha256_hex_bytes(&bytes),
    ));
    Ok(())
}

pub fn check_loaded_hash(state: &AppProfileState, path: &Path, raw: &[u8]) -> Result<(), String> {
    if let Some((loaded_path, hash)) = state
        .save_baseline
        .lock()
        .map_err(|_| "save_state_unavailable")?
        .as_ref()
    {
        if loaded_path == path && *hash != crate::shared::ets2data::validate::sha256_hex_bytes(raw)
        {
            return Err("save_changed_since_load".into());
        }
    }
    Ok(())
}

/// Call under mutation_lock, after creating the full persistent backup.
/// Validation runs on the temporary file before replacement; failures leave the source intact.
pub fn write_atomic_checked(
    path: &Path,
    expected_raw: &[u8],
    content: &str,
    verify: impl FnOnce(&Path) -> Result<(), String>,
) -> Result<(), String> {
    use std::io::Write;
    if path.extension().is_some_and(|ext| ext == "sii") {
        Document::parse(content)?;
    }
    let temporary = path.with_file_name(format!(".save-edit-{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| {
        let mut file = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)
            .map_err(|_| "save_temporary_create_failed")?;
        file.write_all(content.as_bytes())
            .map_err(|_| "save_temporary_write_failed")?;
        file.sync_all().map_err(|_| "save_temporary_sync_failed")?;
        drop(file);
        let actual = fs::read_to_string(&temporary).map_err(|_| "save_temporary_read_failed")?;
        if actual != content {
            return Err("save_verification_failed".into());
        }
        if path.extension().is_some_and(|ext| ext == "sii") {
            Document::parse(&actual)?;
        }
        verify(&temporary)?;
        if read_retry(path)? != expected_raw {
            return Err("save_changed_since_load".into());
        }
        replace_checked(&temporary, path, Some(expected_raw))?;
        // Reopen the selected original path. A validated temporary file alone
        // does not prove that the live save contains the requested edit.
        let persisted = read_retry(path)?;
        if persisted != content.as_bytes() {
            return Err("save_post_write_verification_failed".into());
        }
        if path.extension().is_some_and(|ext| ext == "sii") {
            let decoded = decode_text_bytes(&persisted, "written_save", &[])?;
            Document::parse(&decoded)?;
            if decoded != content {
                return Err("save_post_write_verification_failed".into());
            }
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

pub fn read_source(state: &AppProfileState, path: &Path) -> Result<Vec<u8>, String> {
    let game = selected_game(state)?;
    let is_save = path
        .file_name()
        .is_some_and(|name| name == "game.sii" || name == "info.sii");
    let raw = if is_save {
        read_validated_bytes(path)?
    } else {
        read_stable_bytes(path)?
    };
    if is_save {
        let selected = active_save(state)?;
        if path.parent() != selected.parent() {
            return Err("save_selection_changed".into());
        }
        check_loaded_hash(state, path, &raw)?;
    } else {
        ensure_game_closed(game)?;
        let global = config_path(state)?;
        let profile = state
            .current_profile
            .lock()
            .map_err(|_| "profile_state_unavailable")?
            .clone();
        let local = profile
            .as_ref()
            .map(|profile| Path::new(profile).join("config.cfg"));
        if path != global && local.as_deref() != Some(path) {
            return Err("config_selection_changed".into());
        }
        if let Some(profile) = profile {
            validate_profile_game(Path::new(&profile), game)?;
        }
    }
    Ok(raw)
}

/// A save and its metadata form one logical edit. Restore the exact original
/// encoding if committing the metadata fails after the save was replaced.
pub fn write_save_pair(
    game_path: &Path,
    game_before: &[u8],
    game_after: &str,
    info_path: &Path,
    info_before: &[u8],
    info_after: &str,
    verify_info: impl FnOnce(&Path) -> Result<(), String>,
) -> Result<(), String> {
    Document::parse(game_after)?;
    Document::parse(info_after)?;
    if fs::read(info_path).map_err(|_| "save_read_failed")? != info_before {
        return Err("save_changed_since_load".into());
    }
    write_atomic_checked(game_path, game_before, game_after, |_| Ok(()))?;
    if let Err(error) = write_atomic_checked(info_path, info_before, info_after, verify_info) {
        if fs::read(game_path).map_err(|_| "save_rollback_failed")? != game_after.as_bytes() {
            return Err("save_rollback_failed:external_change".into());
        }
        let temporary = game_path.with_file_name(format!(".rollback-{}.tmp", uuid::Uuid::new_v4()));
        let rollback = (|| {
            use std::io::Write;
            let mut file = fs::OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&temporary)
                .map_err(|_| "save_rollback_failed")?;
            file.write_all(game_before)
                .map_err(|_| "save_rollback_failed")?;
            file.sync_all().map_err(|_| "save_rollback_failed")?;
            drop(file);
            replace_checked(&temporary, game_path, Some(game_after.as_bytes()))
                .map_err(|_| "save_rollback_failed")?;
            if fs::read(game_path).map_err(|_| "save_rollback_failed")? != game_before {
                return Err("save_rollback_failed");
            }
            Ok(())
        })();
        if rollback.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        rollback?;
        return Err(error);
    }
    Ok(())
}
