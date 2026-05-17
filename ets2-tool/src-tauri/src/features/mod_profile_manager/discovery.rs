use super::category_detector::detect_categories;
use super::manifest_reader::{ManifestMetadata, parse_manifest_text, read_plain_text_lossy};
use super::models::{
    DiscoveredMod, GameType, ModManagerLogPaths, ModProfileManagerState, ModScanSummary, ModSource,
};
use super::presets;
use super::steam_paths::discover_workshop_sources;
use crate::shared::decrypt::decrypt_if_needed;
use crate::shared::paths::mod_directory_path;
use crate::shared::{logs, user_log};
use crate::state::AppProfileState;
use regex::Regex;
use std::collections::{BTreeSet, HashSet};
use std::fs::{self, File};
use std::io::Read;
use std::path::Path;
use std::time::{Duration, Instant, UNIX_EPOCH};
use tauri::AppHandle;
use walkdir::WalkDir;
use zip::ZipArchive;

const LIGHT_SCAN_TIMEOUT: Duration = Duration::from_secs(5);
const DEEP_SCAN_TIMEOUT: Duration = Duration::from_secs(20);
const LIGHT_SCAN_MAX_ROOT_ENTRIES: usize = 800;
const DEEP_SCAN_MAX_ROOT_ENTRIES: usize = 4_000;
const LIGHT_SCAN_MAX_FOLDER_DEPTH: usize = 2;
const DEEP_SCAN_MAX_FOLDER_DEPTH: usize = 8;
const LIGHT_SCAN_MAX_FOLDER_FILES: usize = 250;
const DEEP_SCAN_MAX_FOLDER_FILES: usize = 4_000;
const LIGHT_SCAN_MAX_ARCHIVE_ENTRIES: usize = 250;
const DEEP_SCAN_MAX_ARCHIVE_ENTRIES: usize = 4_000;
const LIGHT_SCAN_MAX_MANIFEST_BYTES: u64 = 128 * 1024;
const DEEP_SCAN_MAX_MANIFEST_BYTES: u64 = 512 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanMode {
    Light,
    Deep,
}

impl ScanMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Deep => "deep",
        }
    }

    fn timeout(self) -> Duration {
        match self {
            Self::Light => LIGHT_SCAN_TIMEOUT,
            Self::Deep => DEEP_SCAN_TIMEOUT,
        }
    }

    fn max_root_entries(self) -> usize {
        match self {
            Self::Light => LIGHT_SCAN_MAX_ROOT_ENTRIES,
            Self::Deep => DEEP_SCAN_MAX_ROOT_ENTRIES,
        }
    }

    fn max_folder_depth(self) -> usize {
        match self {
            Self::Light => LIGHT_SCAN_MAX_FOLDER_DEPTH,
            Self::Deep => DEEP_SCAN_MAX_FOLDER_DEPTH,
        }
    }

    fn max_folder_files(self) -> usize {
        match self {
            Self::Light => LIGHT_SCAN_MAX_FOLDER_FILES,
            Self::Deep => DEEP_SCAN_MAX_FOLDER_FILES,
        }
    }

    fn max_archive_entries(self) -> usize {
        match self {
            Self::Light => LIGHT_SCAN_MAX_ARCHIVE_ENTRIES,
            Self::Deep => DEEP_SCAN_MAX_ARCHIVE_ENTRIES,
        }
    }

    fn max_manifest_bytes(self) -> u64 {
        match self {
            Self::Light => LIGHT_SCAN_MAX_MANIFEST_BYTES,
            Self::Deep => DEEP_SCAN_MAX_MANIFEST_BYTES,
        }
    }
}

struct ScanContext {
    mode: ScanMode,
    started_at: Instant,
    timed_out: bool,
}

impl ScanContext {
    fn new(mode: ScanMode) -> Self {
        Self {
            mode,
            started_at: Instant::now(),
            timed_out: false,
        }
    }

    fn elapsed_ms(&self) -> u128 {
        self.started_at.elapsed().as_millis()
    }

    fn check_timeout(&mut self, warnings: &mut Vec<String>) -> bool {
        if self.timed_out {
            return true;
        }
        if self.started_at.elapsed() <= self.mode.timeout() {
            return false;
        }
        self.timed_out = true;
        crate::dev_log!(
            "[trace] MOD_SCAN timeout after_ms={}",
            self.elapsed_ms()
        );
        record_warning(
            warnings,
            format!(
                "Mod scan timed out. Some mods were skipped. mode={} after_ms={}",
                self.mode.as_str(),
                self.elapsed_ms()
            ),
        );
        true
    }
}

#[derive(Debug, Clone, Default)]
struct ActiveModEntry {
    raw: String,
    identifier: String,
    display_name: String,
    tokens: HashSet<String>,
    index: i32,
}

#[derive(Debug, Clone)]
struct ScannedMod {
    mod_info: DiscoveredMod,
    match_tokens: HashSet<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ScanInventoryResult {
    pub summary: ModScanSummary,
    pub mods: Vec<DiscoveredMod>,
    pub warnings: Vec<String>,
    pub current_profile_path: Option<String>,
    pub logs: ModManagerLogPaths,
}

fn record_warning(warnings: &mut Vec<String>, message: String) {
    crate::dev_log!("[mod-profile-manager] {}", message);
    let _ = user_log::user_log_warn("ModScanner", &message);
    warnings.push(message);
}

pub fn load_manager_state(
    app: &AppHandle,
    profile_state: &AppProfileState,
    requested_game: Option<&str>,
) -> Result<ModProfileManagerState, String> {
    let inventory = scan_inventory_with_mode(app, profile_state, requested_game, ScanMode::Light)?;
    let presets = presets::list_presets(app, Some(inventory.summary.selected_game))?;

    Ok(ModProfileManagerState {
        summary: ModScanSummary {
            presets_saved: presets.len(),
            ..inventory.summary
        },
        mods: inventory.mods,
        presets,
        warnings: inventory.warnings,
        current_profile_path: inventory.current_profile_path,
        logs: inventory.logs,
    })
}

pub fn scan_inventory(
    app: &AppHandle,
    profile_state: &AppProfileState,
    requested_game: Option<&str>,
) -> Result<ScanInventoryResult, String> {
    scan_inventory_with_mode(app, profile_state, requested_game, ScanMode::Deep)
}

pub fn scan_inventory_with_mode(
    app: &AppHandle,
    profile_state: &AppProfileState,
    requested_game: Option<&str>,
    mode: ScanMode,
) -> Result<ScanInventoryResult, String> {
    let game = resolve_game(profile_state, requested_game)?;
    let current_profile_path = current_profile_path(profile_state)?;
    let mut scan_context = ScanContext::new(mode);
    crate::dev_log!(
        "[trace] START mod_scan_{} path={}",
        mode.as_str(),
        current_profile_path.as_deref().unwrap_or("-")
    );
    crate::dev_log!(
        "[mod-profile-manager] scan inventory started game={} requested_game={:?} profile={:?} mode={}",
        game.as_str(),
        requested_game,
        current_profile_path,
        mode.as_str()
    );
    let mut warnings = Vec::new();
    let profile_content = read_profile_content(current_profile_path.as_deref(), &mut warnings);
    let active_mods = profile_content
        .as_deref()
        .map(parse_active_mods)
        .unwrap_or_default();
    let active_mods_reliably_known = profile_content.is_some();
    let manual_workshop_path = presets::get_manual_workshop_path(app, game).ok().flatten();
    let steam_discovery = discover_workshop_sources(game, manual_workshop_path.as_deref());

    crate::dev_log!(
        "[mod-profile-manager] steam discovery game={} steam_install_found={} libraries={} workshop_sources={} manual_workshop_path={}",
        game.as_str(),
        steam_discovery.steam_install_found,
        steam_discovery.libraries.len(),
        steam_discovery.workshop_sources.len(),
        manual_workshop_path.as_deref().unwrap_or("-")
    );
    for library in &steam_discovery.libraries {
        crate::dev_log!(
            "[mod-profile-manager] steam library found game={} path={}",
            game.as_str(),
            library.display()
        );
    }
    for source in &steam_discovery.workshop_sources {
        crate::dev_log!(
            "[mod-profile-manager] workshop folder game={} path={} exists={} manual={}",
            game.as_str(),
            source.path,
            source.exists,
            source.manual
        );
    }
    for warning in steam_discovery.warnings.clone() {
        record_warning(&mut warnings, warning);
    }
    let mut scanned_mods = Vec::new();

    let local_mod_folder = mod_directory_path(game.as_str());
    if let Some(local_mod_folder) = local_mod_folder.as_ref() {
        crate::dev_log!(
            "[mod-profile-manager] local mod folder game={} path={} exists={}",
            game.as_str(),
            local_mod_folder.display(),
            local_mod_folder.is_dir()
        );
        scanned_mods.extend(scan_local_mods(local_mod_folder, &mut warnings, &mut scan_context));
    } else {
        crate::dev_log!(
            "[mod-profile-manager] local mod folder could not be resolved game={}",
            game.as_str()
        );
    }

    scanned_mods.extend(scan_workshop_mods(
        &steam_discovery.workshop_sources,
        &mut warnings,
        &mut scan_context,
    ));
    apply_active_state(&mut scanned_mods, &active_mods, active_mods_reliably_known);
    sort_scanned_mods(&mut scanned_mods);

    let mods = scanned_mods
        .into_iter()
        .map(|item| item.mod_info)
        .collect::<Vec<_>>();
    let logs = ModManagerLogPaths {
        technical_log_path: Some(logs::technical_log_path().display().to_string()),
        user_log_path: Some(user_log::user_log_path().display().to_string()),
        log_directory_path: logs::log_directory_path().map(|path| path.display().to_string()),
    };
    let load_order_source = if active_mods_reliably_known {
        "detected".to_string()
    } else if mods.is_empty() {
        "unknown".to_string()
    } else {
        "estimated".to_string()
    };

    let summary = ModScanSummary {
        selected_game: game,
        scan_mode: mode.as_str().to_string(),
        scan_timed_out: scan_context.timed_out,
        local_mod_folder_path: local_mod_folder.as_ref().map(|path| path.display().to_string()),
        local_mod_folder_found: local_mod_folder.as_ref().map(|path| path.is_dir()).unwrap_or(false),
        steam_install_found: steam_discovery.steam_install_found,
        steam_library_paths: steam_discovery
            .libraries
            .iter()
            .map(|path| path.display().to_string())
            .collect(),
        workshop_sources: steam_discovery.workshop_sources,
        manual_workshop_path,
        local_mods_found: mods
            .iter()
            .filter(|item| item.source == ModSource::LocalModFolder)
            .count(),
        steam_workshop_mods_found: mods
            .iter()
            .filter(|item| item.source == ModSource::SteamWorkshop)
            .count(),
        unreadable_mods_count: mods.iter().filter(|item| !item.readable).count(),
        presets_saved: 0,
        active_mods_reliably_known,
        active_mods_count: mods
            .iter()
            .filter(|item| item.enabled == Some(true))
            .count(),
        load_order_source: load_order_source.clone(),
    };
    crate::dev_log!(
        "[mod-profile-manager] scan inventory completed game={} local_mods={} workshop_mods={} unreadable={} load_order_source={}",
        game.as_str(),
        summary.local_mods_found,
        summary.steam_workshop_mods_found,
        summary.unreadable_mods_count,
        load_order_source
    );
    crate::dev_log!(
        "[trace] END mod_scan_{} duration_ms={}",
        mode.as_str(),
        scan_context.elapsed_ms()
    );

    Ok(ScanInventoryResult {
        summary,
        mods,
        warnings,
        current_profile_path,
        logs,
    })
}

fn resolve_game(profile_state: &AppProfileState, requested_game: Option<&str>) -> Result<GameType, String> {
    if let Some(game) = requested_game {
        return GameType::try_from(game);
    }

    let selected_game = profile_state
        .selected_game
        .lock()
        .map_err(|_| "selected_game lock poisoned".to_string())?
        .clone();
    GameType::try_from(selected_game.as_str())
}

fn current_profile_path(profile_state: &AppProfileState) -> Result<Option<String>, String> {
    profile_state
        .current_profile
        .lock()
        .map_err(|_| "current_profile lock poisoned".to_string())
        .map(|guard| guard.clone())
}

fn read_profile_content(profile_path: Option<&str>, warnings: &mut Vec<String>) -> Option<String> {
    let Some(profile_path) = profile_path else {
        return None;
    };

    let path = Path::new(profile_path).join("profile.sii");
    if !path.is_file() {
        record_warning(
            warnings,
            format!("No profile.sii found at {}", path.display()),
        );
        return None;
    }

    match decrypt_if_needed(&path) {
        Ok(content) => Some(content),
        Err(error) => {
            record_warning(
                warnings,
                format!("Failed to read profile.sii at {}: {}", path.display(), error),
            );
            None
        }
    }
}

fn parse_active_mods(profile_content: &str) -> Vec<ActiveModEntry> {
    let Some(regex) = Regex::new(r#"active_mods\[(\d+)\]:\s*"([^"]+)""#).ok() else {
        return Vec::new();
    };

    regex
        .captures_iter(profile_content)
        .filter_map(|capture| {
            let index = capture.get(1)?.as_str().parse::<i32>().ok()?;
            let raw = capture.get(2)?.as_str().to_string();
            let parts = raw
                .split('|')
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .collect::<Vec<_>>();
            let identifier = parts.first().copied().unwrap_or(raw.as_str()).to_string();
            let display_name = parts.last().copied().unwrap_or(raw.as_str()).to_string();

            let mut tokens = tokenize_to_set(&raw);
            tokens.extend(tokenize_to_set(&identifier));
            tokens.extend(tokenize_to_set(&display_name));

            Some(ActiveModEntry {
                raw,
                identifier,
                display_name,
                tokens,
                index,
            })
        })
        .collect()
}

fn scan_local_mods(
    mod_dir: &Path,
    warnings: &mut Vec<String>,
    scan_context: &mut ScanContext,
) -> Vec<ScannedMod> {
    if !mod_dir.is_dir() {
        return Vec::new();
    }

    let mut mods = Vec::new();
    let Ok(entries) = fs::read_dir(mod_dir) else {
        record_warning(
            warnings,
            format!("Could not read local mod folder {}", mod_dir.display()),
        );
        return Vec::new();
    };

    for (index, entry) in entries.enumerate() {
        if scan_context.check_timeout(warnings) {
            break;
        }
        if index >= scan_context.mode.max_root_entries() {
            record_warning(
                warnings,
                format!(
                    "Skipping remaining local mod entries in {} after limit {}.",
                    mod_dir.display(),
                    scan_context.mode.max_root_entries()
                ),
            );
            break;
        }
        let Ok(entry) = entry else {
            continue;
        };
        let path = entry.path();
        if let Some(mod_entry) = inspect_generic_entry(
            &path,
            ModSource::LocalModFolder,
            None,
            None,
            warnings,
            scan_context,
        ) {
            mods.push(mod_entry);
        }
    }

    mods
}

fn scan_workshop_mods(
    sources: &[super::models::WorkshopFolderSource],
    warnings: &mut Vec<String>,
    scan_context: &mut ScanContext,
) -> Vec<ScannedMod> {
    let mut mods = Vec::new();

    for source in sources {
        if scan_context.check_timeout(warnings) {
            break;
        }
        let root = Path::new(&source.path);
        if !root.is_dir() {
            continue;
        }

        if looks_like_direct_mod_root(root) {
            if let Some(entry) = inspect_generic_entry(
                root,
                ModSource::SteamWorkshop,
                extract_numeric_component(root.file_name().and_then(|value| value.to_str())),
                Some(source.app_id.as_str()),
                warnings,
                scan_context,
            ) {
                mods.push(entry);
            }
            continue;
        }

        let Ok(entries) = fs::read_dir(root) else {
            record_warning(
                warnings,
                format!("Could not read workshop folder {}", root.display()),
            );
            continue;
        };

        for (index, entry) in entries.enumerate() {
            if scan_context.check_timeout(warnings) {
                break;
            }
            if index >= scan_context.mode.max_root_entries() {
                record_warning(
                    warnings,
                    format!(
                        "Skipping remaining workshop entries in {} after limit {}.",
                        root.display(),
                        scan_context.mode.max_root_entries()
                    ),
                );
                break;
            }
            let Ok(entry) = entry else {
                continue;
            };
            let path = entry.path();
            let workshop_id = extract_numeric_component(path.file_name().and_then(|value| value.to_str()));
            if path.is_dir() && workshop_id.is_some() {
                mods.extend(scan_workshop_item_dir(
                    &path,
                    workshop_id,
                    Some(source.app_id.as_str()),
                    warnings,
                    scan_context,
                ));
                continue;
            }

            if let Some(item) = inspect_generic_entry(
                &path,
                ModSource::SteamWorkshop,
                workshop_id,
                Some(source.app_id.as_str()),
                warnings,
                scan_context,
            ) {
                mods.push(item);
            }
        }
    }

    mods
}

fn scan_workshop_item_dir(
    item_dir: &Path,
    workshop_id: Option<String>,
    app_id: Option<&str>,
    warnings: &mut Vec<String>,
    scan_context: &mut ScanContext,
) -> Vec<ScannedMod> {
    let mut mods = Vec::new();
    let Ok(entries) = fs::read_dir(item_dir) else {
        record_warning(
            warnings,
            format!("Could not read workshop item {}", item_dir.display()),
        );
        return vec![invalid_workshop_item(item_dir, workshop_id, app_id)];
    };

    let mut child_mod_found = false;
    for (index, entry) in entries.enumerate() {
        if scan_context.check_timeout(warnings) {
            break;
        }
        if index >= scan_context.mode.max_root_entries() {
            record_warning(
                warnings,
                format!(
                    "Skipping remaining workshop item entries in {} after limit {}.",
                    item_dir.display(),
                    scan_context.mode.max_root_entries()
                ),
            );
            break;
        }
        let Ok(entry) = entry else {
            continue;
        };
        let path = entry.path();
        if let Some(item) = inspect_generic_entry(
            &path,
            ModSource::SteamWorkshop,
            workshop_id.clone(),
            app_id,
            warnings,
            scan_context,
        ) {
            child_mod_found = true;
            mods.push(item);
        }
    }

    if !child_mod_found {
        if let Some(item) = inspect_generic_entry(
            item_dir,
            ModSource::SteamWorkshop,
            workshop_id.clone(),
            app_id,
            warnings,
            scan_context,
        ) {
            mods.push(item);
        } else {
            mods.push(invalid_workshop_item(item_dir, workshop_id, app_id));
        }
    }

    mods
}

fn inspect_generic_entry(
    path: &Path,
    source: ModSource,
    workshop_id: Option<String>,
    app_id: Option<&str>,
    warnings: &mut Vec<String>,
    scan_context: &mut ScanContext,
) -> Option<ScannedMod> {
    if scan_context.check_timeout(warnings) {
        return None;
    }
    if is_symlink_path(path) {
        crate::dev_log!(
            "[trace] MOD_SCAN skipped_file path={} reason=symlink",
            path.display()
        );
        record_warning(
            warnings,
            format!("Skipped symbolic link or junction {}", path.display()),
        );
        return None;
    }

    let is_archive = path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| matches!(value.to_ascii_lowercase().as_str(), "scs" | "zip"))
        .unwrap_or(false);

    if !path.is_dir() && !is_archive {
        if let Some(extension) = path.extension().and_then(|value| value.to_str()) {
            if matches!(extension.to_ascii_lowercase().as_str(), "rar" | "7z") {
                crate::dev_log!(
                    "[trace] MOD_SCAN skipped_file path={} reason=unsupported_archive",
                    path.display()
                );
            }
        }
        return None;
    }

    let inspected = if is_archive {
        inspect_archive_mod(path, source.clone(), workshop_id.clone(), app_id, scan_context)
    } else {
        inspect_folder_mod(path, source.clone(), workshop_id.clone(), app_id, scan_context)
    };

    match inspected {
        Ok(item) => Some(item),
        Err(error) => {
            crate::dev_log!(
                "[trace] MOD_SCAN error path={} error={}",
                path.display(),
                error
            );
            record_warning(
                warnings,
                format!("Could not inspect {}: {}", path.display(), error),
            );
            Some(fallback_mod(path, source, workshop_id, app_id, "unreadable"))
        }
    }
}

fn inspect_folder_mod(
    path: &Path,
    source: ModSource,
    workshop_id: Option<String>,
    app_id: Option<&str>,
    scan_context: &mut ScanContext,
) -> Result<ScannedMod, String> {
    let mut indexed_paths = Vec::new();
    let mut manifest_metadata = ManifestMetadata::default();
    let mut manifest_present = false;
    let mut readable = true;
    let mut files_seen = 0usize;

    for entry in WalkDir::new(path)
        .follow_links(false)
        .max_depth(scan_context.mode.max_folder_depth())
    {
        let entry = match entry {
            Ok(value) => value,
            Err(error) => {
                readable = false;
                crate::dev_log!(
                    "[mod-profile-manager] walkdir failed for {}: {}",
                    path.display(),
                    error
                );
                continue;
            }
        };
        if !entry.file_type().is_file() {
            continue;
        }
        if scan_context.started_at.elapsed() > scan_context.mode.timeout() {
            scan_context.timed_out = true;
            break;
        }
        files_seen += 1;
        if files_seen > scan_context.mode.max_folder_files() {
            break;
        }

        let Ok(relative) = entry.path().strip_prefix(path) else {
            continue;
        };
        let normalized = normalize_archive_path(relative);
        if normalized.is_empty() {
            continue;
        }
        indexed_paths.push(normalized.clone());

        if normalized == "manifest.sii" || normalized.ends_with("/manifest.sii") {
            manifest_present = true;
            match read_plain_text_lossy(entry.path()) {
                Ok(content) => {
                    if content.len() as u64 <= scan_context.mode.max_manifest_bytes() {
                        manifest_metadata = parse_manifest_text(&content);
                    }
                }
                Err(error) => {
                    crate::dev_log!(
                        "[mod-profile-manager] manifest read failed path={} error={}",
                        entry.path().display(),
                        error
                    );
                }
            }
        }
    }

    Ok(build_scanned_mod(
        path,
        source,
        workshop_id,
        app_id,
        file_kind_label(path),
        file_size(path),
        file_modified_at(path),
        manifest_metadata,
        manifest_present,
        readable,
        indexed_paths,
        if manifest_present { "ok" } else { "manifest_missing" },
    ))
}

fn inspect_archive_mod(
    path: &Path,
    source: ModSource,
    workshop_id: Option<String>,
    app_id: Option<&str>,
    scan_context: &mut ScanContext,
) -> Result<ScannedMod, String> {
    let file = File::open(path).map_err(|error| format!("open failed: {}", error))?;
    let mut archive = ZipArchive::new(file).map_err(|error| format!("zip read failed: {}", error))?;
    let mut indexed_paths = Vec::new();
    let mut manifest_metadata = ManifestMetadata::default();
    let mut manifest_present = false;

    let entry_limit = archive.len().min(scan_context.mode.max_archive_entries());
    for index in 0..entry_limit {
        if scan_context.started_at.elapsed() > scan_context.mode.timeout() {
            scan_context.timed_out = true;
            break;
        }
        let mut entry = archive
            .by_index(index)
            .map_err(|error| format!("archive entry failed: {}", error))?;
        let normalized = normalize_zip_name(entry.name());
        if normalized.is_empty() {
            continue;
        }
        indexed_paths.push(normalized.clone());

        if normalized == "manifest.sii" || normalized.ends_with("/manifest.sii") {
            manifest_present = true;
            if entry.size() <= scan_context.mode.max_manifest_bytes() {
                let mut bytes = Vec::new();
                entry.read_to_end(&mut bytes)
                    .map_err(|error| format!("manifest read failed: {}", error))?;
                manifest_metadata = parse_manifest_text(&String::from_utf8_lossy(&bytes));
            }
        }
    }

    Ok(build_scanned_mod(
        path,
        source,
        workshop_id,
        app_id,
        file_kind_label(path),
        file_size(path),
        file_modified_at(path),
        manifest_metadata,
        manifest_present,
        true,
        indexed_paths,
        if manifest_present { "ok" } else { "manifest_missing" },
    ))
}

fn build_scanned_mod(
    path: &Path,
    source: ModSource,
    workshop_id: Option<String>,
    app_id: Option<&str>,
    file_kind: String,
    size_bytes: Option<u64>,
    modified_at: Option<String>,
    manifest_metadata: ManifestMetadata,
    manifest_present: bool,
    readable: bool,
    indexed_paths: Vec<String>,
    status: &str,
) -> ScannedMod {
    let fallback_name = path
        .file_stem()
        .or_else(|| path.file_name())
        .and_then(|value| value.to_str())
        .map(|value| value.to_string())
        .unwrap_or_else(|| path.display().to_string());
    let name = manifest_metadata
        .display_name
        .clone()
        .or_else(|| manifest_metadata.package_name.clone())
        .unwrap_or_else(|| fallback_name.clone());
    let duplicate_key = build_duplicate_key(
        source.clone(),
        workshop_id.as_deref(),
        app_id,
        manifest_metadata.package_name.as_deref(),
        manifest_metadata.display_name.as_deref(),
        &fallback_name,
    );
    let categories = detect_categories(
        &indexed_paths,
        &manifest_metadata.categories,
        &[
            name.clone(),
            manifest_metadata
                .package_name
                .clone()
                .unwrap_or_default(),
            manifest_metadata.description.clone().unwrap_or_default(),
        ],
    );
    let workshop_url = workshop_id
        .as_ref()
        .map(|value| format!("https://steamcommunity.com/sharedfiles/filedetails/?id={}", value));

    let mut match_tokens = tokenize_to_set(&name);
    match_tokens.extend(tokenize_to_set(&fallback_name));
    if let Some(value) = manifest_metadata.package_name.as_deref() {
        match_tokens.extend(tokenize_to_set(value));
    }
    if let Some(value) = manifest_metadata.display_name.as_deref() {
        match_tokens.extend(tokenize_to_set(value));
    }
    if let Some(value) = workshop_id.as_deref() {
        match_tokens.insert(value.to_string());
    }
    match_tokens.extend(tokenize_to_set(&duplicate_key));

    let mut warnings = Vec::new();
    if !readable {
        warnings.push("Unreadable mod entry".to_string());
    }
    if !manifest_present {
        warnings.push("manifest.sii not found".to_string());
    }

    ScannedMod {
        mod_info: DiscoveredMod {
            id: duplicate_key.clone(),
            source,
            name,
            file_path: path.display().to_string(),
            file_kind,
            size_bytes,
            modified_at,
            workshop_id,
            app_id: app_id.map(|value| value.to_string()),
            manifest_name: manifest_metadata.display_name,
            version: manifest_metadata.version,
            author: manifest_metadata.author,
            categories,
            readable,
            enabled: None,
            load_order_index: None,
            load_order_source: "unknown".to_string(),
            status: if readable {
                status.to_string()
            } else {
                "unreadable".to_string()
            },
            workshop_url,
            manifest_present,
            duplicate_key,
            warnings,
        },
        match_tokens,
    }
}

fn fallback_mod(
    path: &Path,
    source: ModSource,
    workshop_id: Option<String>,
    app_id: Option<&str>,
    status: &str,
) -> ScannedMod {
    build_scanned_mod(
        path,
        source,
        workshop_id,
        app_id,
        file_kind_label(path),
        file_size(path),
        file_modified_at(path),
        ManifestMetadata::default(),
        false,
        false,
        Vec::new(),
        status,
    )
}

fn invalid_workshop_item(item_dir: &Path, workshop_id: Option<String>, app_id: Option<&str>) -> ScannedMod {
    let mut item = fallback_mod(
        item_dir,
        ModSource::SteamWorkshop,
        workshop_id,
        app_id,
        "invalid_workshop_item",
    );
    item.mod_info.status = "invalid_workshop_item".to_string();
    item.mod_info
        .warnings
        .push("The workshop item did not contain a valid mod archive or folder.".to_string());
    item
}

fn apply_active_state(mods: &mut [ScannedMod], active_mods: &[ActiveModEntry], active_mods_reliably_known: bool) {
    if !active_mods_reliably_known {
        apply_estimated_state(mods);
        return;
    }

    let mut used_mod_indexes = HashSet::new();

    for active_mod in active_mods {
        let mut best_index = None;
        let mut best_score = 0i32;

        for (index, scanned_mod) in mods.iter().enumerate() {
            if used_mod_indexes.contains(&index) {
                continue;
            }
            let score = match_active_entry(scanned_mod, active_mod);
            if score > best_score {
                best_score = score;
                best_index = Some(index);
            }
        }

        if let Some(index) = best_index {
            if best_score > 0 {
                used_mod_indexes.insert(index);
                let mod_info = &mut mods[index].mod_info;
                mod_info.enabled = Some(true);
                mod_info.load_order_index = Some(active_mod.index);
                mod_info.load_order_source = "detected".to_string();
                if mod_info.status == "manifest_missing" {
                    mod_info.status = "active_manifest_missing".to_string();
                }
            }
        }
    }

    for item in mods.iter_mut() {
        if item.mod_info.enabled.is_none() {
            item.mod_info.enabled = Some(false);
            item.mod_info.load_order_source = "detected".to_string();
        }
    }
}

fn apply_estimated_state(mods: &mut [ScannedMod]) {
    mods.sort_by(|left, right| {
        estimated_source_rank(&left.mod_info.source)
            .cmp(&estimated_source_rank(&right.mod_info.source))
            .then_with(|| {
                left.mod_info
                    .name
                    .to_ascii_lowercase()
                    .cmp(&right.mod_info.name.to_ascii_lowercase())
            })
            .then_with(|| {
                left.mod_info
                    .file_path
                    .to_ascii_lowercase()
                    .cmp(&right.mod_info.file_path.to_ascii_lowercase())
            })
    });

    for (index, item) in mods.iter_mut().enumerate() {
        item.mod_info.enabled = None;
        item.mod_info.load_order_index = Some(index as i32);
        item.mod_info.load_order_source = "estimated".to_string();
    }
}

fn estimated_source_rank(source: &ModSource) -> i32 {
    match source {
        ModSource::LocalModFolder => 0,
        ModSource::SteamWorkshop => 1,
        ModSource::Unknown => 2,
    }
}

fn match_active_entry(scanned_mod: &ScannedMod, active_mod: &ActiveModEntry) -> i32 {
    let mut score = 0;

    if same_normalized(&scanned_mod.mod_info.duplicate_key, &active_mod.identifier)
        || same_normalized(&scanned_mod.mod_info.duplicate_key, &active_mod.display_name)
        || same_normalized(&scanned_mod.mod_info.duplicate_key, &active_mod.raw)
    {
        score += 90;
    }

    if contains_normalized(&active_mod.raw, &scanned_mod.mod_info.name)
        || contains_normalized(&active_mod.raw, &scanned_mod.mod_info.duplicate_key)
        || contains_normalized(&active_mod.identifier, &scanned_mod.mod_info.name)
        || contains_normalized(&active_mod.display_name, &scanned_mod.mod_info.name)
    {
        score += 55;
    }

    let overlap = scanned_mod
        .match_tokens
        .intersection(&active_mod.tokens)
        .filter(|value| value.len() > 2)
        .count() as i32;
    score + overlap * 12
}

fn sort_scanned_mods(mods: &mut [ScannedMod]) {
    mods.sort_by(|left, right| {
        right
            .mod_info
            .enabled
            .cmp(&left.mod_info.enabled)
            .then_with(|| left.mod_info.load_order_index.cmp(&right.mod_info.load_order_index))
            .then_with(|| left.mod_info.name.to_ascii_lowercase().cmp(&right.mod_info.name.to_ascii_lowercase()))
    });
}

fn looks_like_direct_mod_root(path: &Path) -> bool {
    if !path.is_dir() {
        return false;
    }

    let markers = ["manifest.sii", "def", "map", "vehicle", "ui", "sound", "material"];
    markers.iter().any(|marker| path.join(marker).exists())
        || fs::read_dir(path)
            .ok()
            .into_iter()
            .flatten()
            .flatten()
            .any(|entry| {
                entry
                    .path()
                    .extension()
                    .and_then(|value| value.to_str())
                    .map(|value| matches!(value.to_ascii_lowercase().as_str(), "scs" | "zip"))
                    .unwrap_or(false)
            })
}

fn extract_numeric_component(value: Option<&str>) -> Option<String> {
    let value = value?.trim();
    if !value.is_empty() && value.chars().all(|character| character.is_ascii_digit()) {
        Some(value.to_string())
    } else {
        None
    }
}

fn normalize_archive_path(path: &Path) -> String {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy().to_string())
        .collect::<Vec<_>>()
        .join("/")
        .trim_matches('/')
        .to_ascii_lowercase()
}

fn normalize_zip_name(path: &str) -> String {
    path.trim_matches('/').replace('\\', "/").to_ascii_lowercase()
}

fn build_duplicate_key(
    source: ModSource,
    workshop_id: Option<&str>,
    app_id: Option<&str>,
    package_name: Option<&str>,
    manifest_name: Option<&str>,
    fallback_name: &str,
) -> String {
    if let Some(workshop_id) = workshop_id {
        let app_id = app_id.unwrap_or("unknown");
        return format!(
            "workshop:{}:{}:{}",
            app_id,
            workshop_id,
            normalize_token(
                package_name
                    .or(manifest_name)
                    .unwrap_or(fallback_name)
            )
        );
    }

    let source_key = match source {
        ModSource::LocalModFolder => "local",
        ModSource::SteamWorkshop => "workshop_local",
        ModSource::Unknown => "unknown",
    };

    format!(
        "{}:{}",
        source_key,
        normalize_token(package_name.or(manifest_name).unwrap_or(fallback_name))
    )
}

fn same_normalized(left: &str, right: &str) -> bool {
    normalize_token(left) == normalize_token(right)
}

fn contains_normalized(haystack: &str, needle: &str) -> bool {
    let haystack = normalize_token(haystack);
    let needle = normalize_token(needle);
    !needle.is_empty() && haystack.contains(&needle)
}

fn tokenize_to_set(value: &str) -> HashSet<String> {
    tokenize(value).into_iter().collect()
}

fn tokenize(value: &str) -> Vec<String> {
    value
        .split(|character: char| !character.is_ascii_alphanumeric())
        .map(|part| part.trim().to_ascii_lowercase())
        .filter(|part| part.len() > 2)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn normalize_token(value: &str) -> String {
    value
        .trim()
        .replace('\\', "/")
        .replace('|', " ")
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("_")
        .to_ascii_lowercase()
}

fn is_symlink_path(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .map(|metadata| metadata.file_type().is_symlink())
        .unwrap_or(false)
}

fn file_kind_label(path: &Path) -> String {
    if path.is_dir() {
        return "folder".to_string();
    }
    match path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
    {
        Some(value) if value == "scs" => "scs".to_string(),
        Some(value) if value == "zip" => "zip".to_string(),
        Some(value) => value,
        None => "unknown".to_string(),
    }
}

fn file_size(path: &Path) -> Option<u64> {
    fs::metadata(path).ok().map(|metadata| metadata.len())
}

fn file_modified_at(path: &Path) -> Option<String> {
    let modified = fs::metadata(path).ok()?.modified().ok()?;
    let seconds = modified.duration_since(UNIX_EPOCH).ok()?.as_secs() as i64;
    chrono::DateTime::<chrono::Utc>::from_timestamp(seconds, 0).map(|value| value.to_rfc3339())
}
