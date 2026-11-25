//! Read-only save synchronization. Process and telemetry status are deliberately unrelated.
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant, SystemTime};

use notify::{RecursiveMode, Watcher};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use super::{document::Document, safety};
use crate::models::save_game_data::SaveGameData;
use crate::shared::decrypt::decode_text_bytes;
use crate::shared::user_log;
use crate::state::{AppProfileState, DecryptCache, ProfileCache, SaveSyncState};

const POLL_INTERVAL: Duration = Duration::from_secs(2);
pub(super) const DEBOUNCE: Duration = Duration::from_millis(750);
const RETRIES: usize = 3;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct FileStamp {
    size: u64,
    modified: Option<SystemTime>,
}

fn stamp(path: &Path) -> Result<FileStamp, String> {
    let metadata = fs::metadata(path).map_err(|error| format!("save_metadata_failed: {error}"))?;
    Ok(FileStamp {
        size: metadata.len(),
        modified: metadata.modified().ok(),
    })
}

pub(crate) struct StableSave {
    pub path: PathBuf,
    pub text: String,
    pub info: Option<String>,
    pub hash: String,
    pub revision: String,
}

fn read_pair_once(path: &Path) -> Result<StableSave, String> {
    let info_path = path.with_file_name("info.sii");
    let before = stamp(path)?;
    let info_before = match fs::metadata(&info_path) {
        Ok(_) => Some(stamp(&info_path)?),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(format!("save_metadata_failed: {error}")),
    };
    let raw = fs::read(path).map_err(|error| format!("save_read_failed: {error}"))?;
    let info_raw = if info_before.is_some() {
        Some(fs::read(&info_path).map_err(|error| format!("save_read_failed: {error}"))?)
    } else {
        None
    };
    let text = decode_text_bytes(&raw, "sync_game", &[])?;
    Document::parse(&text)?;
    let info = if info_before.is_some() {
        let text = decode_text_bytes(
            info_raw.as_ref().ok_or("save_read_failed")?,
            "sync_info",
            &[],
        )?;
        let doc = Document::parse(&text)?;
        doc.unique("save_container")?;
        Some(text)
    } else {
        None
    };
    // Compare bytes as well: a replacement can retain both timestamp and size.
    if stamp(path)? != before
        || fs::read(path).map_err(|error| format!("save_read_failed: {error}"))? != raw
        || info_before
            != if info_path.exists() {
                Some(stamp(&info_path)?)
            } else {
                None
            }
        || info_raw
            != if info_before.is_some() {
                Some(fs::read(&info_path).map_err(|error| format!("save_read_failed: {error}"))?)
            } else {
                None
            }
    {
        return Err("save_changed_during_read".into());
    }
    let hash = crate::shared::ets2data::validate::sha256_hex_bytes(&raw);
    let revision = crate::shared::ets2data::validate::sha256_hex_bytes(
        format!("{hash}\0{}", info.as_deref().unwrap_or("")).as_bytes(),
    );
    Ok(StableSave {
        path: path.to_path_buf(),
        text,
        info,
        hash,
        revision,
    })
}

pub(crate) fn read_stable_save(path: &Path) -> Result<StableSave, String> {
    let mut last_error = String::new();
    for attempt in 0..RETRIES {
        match read_pair_once(path) {
            Ok(save) => return Ok(save),
            Err(error) => {
                // OS sharing violations and partial writes are retryable, like parse failures.
                let _ = user_log::user_log_warn(
                    "SaveSync",
                    format!(
                        "Read attempt {}/{} failed (file may be locked or incomplete): {}",
                        attempt + 1,
                        RETRIES,
                        error
                    ),
                );
                last_error = error;
                if attempt + 1 < RETRIES {
                    std::thread::sleep(DEBOUNCE);
                }
            }
        }
    }
    Err(last_error)
}

pub(crate) fn publish_save(
    save: &StableSave,
    cache: &DecryptCache,
    profile_cache: &ProfileCache,
) -> Result<SaveGameData, String> {
    // Stage all required parsing before invalidating the last successful snapshot.
    let staged = DecryptCache::default();
    staged
        .files
        .lock()
        .map_err(|_| "cache_unavailable")?
        .insert(save.path.clone(), save.text.clone());
    staged
        .fingerprints
        .lock()
        .map_err(|_| "cache_unavailable")?
        .insert(
            save.path.clone(),
            crate::state::FileFingerprint::read(&save.path)?,
        );
    if let Some(info) = &save.info {
        staged
            .files
            .lock()
            .map_err(|_| "cache_unavailable")?
            .insert(save.path.with_file_name("info.sii"), info.clone());
        let path = save.path.with_file_name("info.sii");
        staged
            .fingerprints
            .lock()
            .map_err(|_| "cache_unavailable")?
            .insert(path.clone(), crate::state::FileFingerprint::read(&path)?);
    }
    let data = crate::features::save_analysis::reader::read_all_save_data_from_path(
        save.path.clone(),
        &staged,
    )?;
    if read_pair_once(&save.path)?.revision != save.revision {
        return Err("save_changed_during_read".into());
    }
    // Readers also use non-canonical Windows paths. Clear their aliases and
    // in-flight generations as well, or vehicles/skills could reuse old content.
    crate::shared::decrypt::clear_decrypt_cache(cache)?;
    {
        let mut files = cache.files.lock().map_err(|_| "cache_unavailable")?;
        files.insert(save.path.clone(), save.text.clone());
        if let Some(info) = &save.info {
            files.insert(save.path.with_file_name("info.sii"), info.clone());
        }
    }
    profile_cache.invalidate_save_data();
    *cache.fingerprints.lock().map_err(|_| "cache_unavailable")? = staged
        .fingerprints
        .lock()
        .map_err(|_| "cache_unavailable")?
        .clone();
    profile_cache.invalidate_vehicle_data();
    profile_cache.set_save_path(Some(save.path.display().to_string()));
    profile_cache.cache_save_game_data(save.path.display().to_string(), data.clone());
    let _ = user_log::user_log_info(
        "SaveSync",
        "Validated save snapshot published; save and vehicle caches invalidated.",
    );
    Ok(data)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncResult {
    game: String,
    profile_path: Option<String>,
    save_path: Option<String>,
    revision: Option<String>,
    data: Option<SaveGameData>,
    status: &'static str,
}

fn check_selection(
    state: &AppProfileState,
    game: &str,
    profile: &Option<String>,
    save: &Option<String>,
) -> Result<(), String> {
    if state
        .selected_game
        .lock()
        .map_err(|_| "game_state_unavailable")?
        .as_str()
        != game
        || *state
            .current_profile
            .lock()
            .map_err(|_| "profile_state_unavailable")?
            != *profile
        || *state
            .current_save
            .lock()
            .map_err(|_| "save_state_unavailable")?
            != *save
    {
        return Err("save_selection_changed".into());
    }
    Ok(())
}

#[tauri::command]
pub async fn refresh_savegame<R: tauri::Runtime>(
    app: AppHandle<R>,
    game: String,
    profile_path: Option<String>,
    save_path: Option<String>,
    force: Option<bool>,
) -> Result<SyncResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let sync = app.state::<SaveSyncState>();
        let _refresh = sync
            .refresh_lock
            .try_lock()
            .map_err(|_| "save_refresh_in_progress")?;
        let _mutation = safety::mutation_lock()?;
        let state = app.state::<AppProfileState>();
        check_selection(&state, &game, &profile_path, &save_path)?;
        let mut result = SyncResult {
            game,
            profile_path,
            save_path,
            revision: None,
            data: None,
            status: "waiting",
        };
        if result.save_path.is_none() {
            return Ok(result);
        }
        let path = safety::active_save(&state)?;
        let save = read_stable_save(&path)?;
        check_selection(
            &state,
            &result.game,
            &result.profile_path,
            &result.save_path,
        )?;
        if force.unwrap_or(false) {
            crate::shared::decrypt::clear_decrypt_cache(&app.state::<DecryptCache>())?;
            app.state::<ProfileCache>().invalidate_base_config();
            app.state::<ProfileCache>().invalidate_save_config();
        }
        result.data = Some(publish_save(
            &save,
            &app.state::<DecryptCache>(),
            &app.state::<ProfileCache>(),
        )?);
        result.revision = Some(save.revision);
        result.status = "synchronized";
        // The loaded hash stays unchanged until the UI accepts this revision. An
        // open edit must still reject an external change with save_changed_since_load.
        Ok(result)
    })
    .await
    .map_err(|error| format!("save_refresh_failed: {error}"))?
}

#[tauri::command]
pub async fn accept_savegame_refresh<R: tauri::Runtime>(
    app: AppHandle<R>,
    game: String,
    profile_path: Option<String>,
    save_path: Option<String>,
    revision: String,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let _mutation = safety::mutation_lock()?;
        let state = app.state::<AppProfileState>();
        check_selection(&state, &game, &profile_path, &save_path)?;
        let save = read_pair_once(&safety::active_save(&state)?)?;
        if save.revision != revision {
            return Err("save_changed_during_read".into());
        }
        *state
            .save_baseline
            .lock()
            .map_err(|_| "save_state_unavailable")? = Some((save.path, save.hash));
        let _ = user_log::user_log_info(
            "SaveSync",
            "Savegame synchronized; loaded hash accepted by the UI.",
        );
        Ok(())
    })
    .await
    .map_err(|error| format!("save_refresh_failed: {error}"))?
}

type Inventory = BTreeMap<PathBuf, Option<FileStamp>>;

fn add_file(files: &mut Inventory, path: PathBuf) -> Result<(), String> {
    match fs::metadata(&path) {
        Ok(_) => {
            files.insert(path.clone(), Some(stamp(&path)?));
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(format!("save_scan_failed: {error}")),
    }
    Ok(())
}

pub(super) fn inventory(
    roots: &[PathBuf],
    selected_profile: Option<&Path>,
) -> Result<Inventory, String> {
    let mut files = BTreeMap::new();
    let mut profiles = Vec::new();
    for root in roots {
        let entries = match fs::read_dir(root) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(format!("profile_scan_failed: {error}")),
        };
        for entry in entries {
            let entry = entry.map_err(|error| format!("profile_scan_failed: {error}"))?;
            if entry
                .file_type()
                .map_err(|error| error.to_string())?
                .is_dir()
            {
                profiles.push(entry.path());
            }
        }
    }
    if let Some(profile) = selected_profile {
        profiles.push(profile.to_path_buf());
    }
    profiles.sort();
    profiles.dedup();
    for profile in profiles {
        add_file(&mut files, profile.join("profile.sii"))?;
        let entries = match fs::read_dir(profile.join("save")) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(format!("save_scan_failed: {error}")),
        };
        for entry in entries {
            let entry = entry.map_err(|error| error.to_string())?;
            if !entry
                .file_type()
                .map_err(|error| error.to_string())?
                .is_dir()
            {
                continue;
            }
            let path = entry.path();
            files.insert(path.clone(), None);
            add_file(&mut files, path.join("game.sii"))?;
            add_file(&mut files, path.join("info.sii"))?;
        }
    }
    Ok(files)
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ChangeEvent {
    game: String,
    profile_path: Option<String>,
    save_path: Option<String>,
}

pub fn start<R: tauri::Runtime>(app: AppHandle<R>) {
    start_with_roots(app, None, true);
}

pub(super) fn start_with_roots<R: tauri::Runtime>(
    app: AppHandle<R>,
    roots_override: Option<Vec<PathBuf>>,
    native: bool,
) {
    let stop = app.state::<SaveSyncState>().stop.clone();
    let worker_app = app.clone();
    let result = std::thread::Builder::new()
        .name("savegame-sync".into())
        .spawn(move || {
            let app = worker_app;
            let dirty = Arc::new(AtomicBool::new(false));
            let watcher_error = Arc::new(AtomicBool::new(false));
            let mut watcher = if native {
                notify::recommended_watcher({
                    let dirty = dirty.clone();
                    let watcher_error = watcher_error.clone();
                    move |event: notify::Result<notify::Event>| match event {
                        Ok(event)
                            if !matches!(event.kind, notify::EventKind::Access(_))
                                && (event.paths.iter().any(|path| {
                                    path.file_name().is_some_and(|name| {
                                        name == "game.sii"
                                            || name == "info.sii"
                                            || name == "profile.sii"
                                    })
                                }) || matches!(
                                    event.kind,
                                    notify::EventKind::Create(notify::event::CreateKind::Folder)
                                        | notify::EventKind::Remove(
                                            notify::event::RemoveKind::Folder
                                        )
                                        | notify::EventKind::Modify(
                                            notify::event::ModifyKind::Name(_)
                                        )
                                )) =>
                        {
                            dirty.store(true, Ordering::Relaxed);
                        }
                        Err(_) => {
                            watcher_error.store(true, Ordering::Relaxed);
                        }
                        _ => {}
                    }
                })
                .ok()
            } else {
                None
            };
            let _ = user_log::user_log_info(
                "SaveSync",
                if watcher.is_some() {
                    "File watcher started with 2-second polling fallback."
                } else {
                    "File watcher unavailable; 2-second polling started."
                },
            );
            let mut watched: Vec<PathBuf> = Vec::new();
            let mut previous_context = None;
            let mut previous = BTreeMap::new();
            let mut selected_fingerprint = None;
            let mut pending = None;
            let mut last_poll = Instant::now() - POLL_INTERVAL;
            let mut scan_failed = false;
            while !stop.load(Ordering::Relaxed) {
                std::thread::sleep(Duration::from_millis(100));
                let poll = last_poll.elapsed() >= POLL_INTERVAL;
                let notified = dirty.swap(false, Ordering::Relaxed);
                if poll || notified {
                    if poll {
                        last_poll = Instant::now();
                    }
                    if notified {
                        pending = Some(Instant::now());
                    }
                    let state = app.state::<AppProfileState>();
                    let context = (|| -> Result<_, String> {
                        Ok((
                            state
                                .selected_game
                                .lock()
                                .map_err(|_| "game_state_unavailable")?
                                .clone(),
                            state
                                .current_profile
                                .lock()
                                .map_err(|_| "profile_state_unavailable")?
                                .clone(),
                            state
                                .current_save
                                .lock()
                                .map_err(|_| "save_state_unavailable")?
                                .clone(),
                        ))
                    })();
                    let Ok(context) = context else {
                        continue;
                    };
                    // The polling fallback also checks the selected slot's bytes:
                    // overwrites can preserve both length and modification time.
                    let fingerprint = context.2.as_ref().map(|save| {
                        let path = crate::shared::paths::game_sii_from_save(Path::new(save));
                        (
                            crate::state::FileFingerprint::read(&path).ok(),
                            crate::state::FileFingerprint::read(&path.with_file_name("info.sii"))
                                .ok(),
                        )
                    });
                    if fingerprint != selected_fingerprint {
                        selected_fingerprint = fingerprint;
                        pending = Some(Instant::now());
                    }
                    let roots = roots_override
                        .clone()
                        .unwrap_or_else(|| crate::shared::paths::profile_search_roots(&context.0));
                    let mut watch_roots = roots.clone();
                    if let Some(profile) = &context.1 {
                        watch_roots.push(PathBuf::from(profile));
                    }
                    watch_roots.sort();
                    watch_roots.dedup();
                    let available = watch_roots
                        .iter()
                        .filter(|path| path.is_dir())
                        .cloned()
                        .collect::<Vec<_>>();
                    if available != watched {
                        if let Some(watcher) = watcher.as_mut() {
                            for path in &watched {
                                let _ = watcher.unwatch(path);
                            }
                            for path in &available {
                                if watcher.watch(path, RecursiveMode::Recursive).is_err() {
                                    watcher_error.store(true, Ordering::Relaxed);
                                }
                            }
                        }
                        watched = available;
                    }
                    match inventory(&roots, context.1.as_deref().map(Path::new)) {
                        Ok(current) => {
                            scan_failed = false;
                            if previous_context.as_ref() != Some(&context) {
                                previous_context = Some(context.clone());
                                previous = current;
                                pending = Some(Instant::now());
                            } else if current != previous {
                                previous = current;
                                pending = Some(Instant::now());
                            }
                        }
                        Err(error) => {
                            if !scan_failed {
                                let _ = user_log::user_log_warn("SaveSync", error);
                            }
                            scan_failed = true;
                        }
                    }
                }
                if watcher_error.swap(false, Ordering::Relaxed) {
                    let _ = user_log::user_log_warn(
                        "SaveSync",
                        "File watcher reported an error; polling fallback remains active.",
                    );
                    // Avoid repeated watcher errors; metadata polling continues independently.
                    watcher = None;
                }
                if !scan_failed && pending.is_some_and(|since| since.elapsed() >= DEBOUNCE) {
                    pending = None;
                    if let Some((game, profile_path, save_path)) = previous_context.clone() {
                        let _ = user_log::user_log_info(
                            "SaveSync",
                            "Savegame/profile change detected (debounced).",
                        );
                        let _ = app.emit(
                            "savegame://changed",
                            ChangeEvent {
                                game,
                                profile_path,
                                save_path,
                            },
                        );
                    }
                }
            }
            drop(watcher);
            let _ = user_log::user_log_info("SaveSync", "File watcher and polling stopped.");
        });
    match result {
        Ok(worker) => {
            if let Ok(mut handle) = app.state::<SaveSyncState>().worker.lock() {
                *handle = Some(worker);
            }
        }
        Err(error) => {
            let _ = user_log::user_log_warn("SaveSync", format!("Watcher startup failed: {error}"));
        }
    }
}

pub fn stop<R: tauri::Runtime>(app: &AppHandle<R>) {
    let state = app.state::<SaveSyncState>();
    state.stop.store(true, Ordering::Relaxed);
    if let Ok(mut worker) = state.worker.lock() {
        if let Some(worker) = worker.take() {
            let _ = worker.join();
        }
    }
}
