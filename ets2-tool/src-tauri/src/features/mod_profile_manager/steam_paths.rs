use super::models::{GameType, WorkshopFolderSource};
use regex::Regex;
use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

#[cfg(target_os = "windows")]
use winreg::{RegKey, enums::*};

#[derive(Debug, Clone, Default)]
pub struct SteamDiscovery {
    pub steam_install_found: bool,
    pub steam_root: Option<PathBuf>,
    pub libraries: Vec<PathBuf>,
    pub workshop_sources: Vec<WorkshopFolderSource>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Default)]
struct LibraryFolderEntry {
    path: PathBuf,
    app_ids: BTreeSet<String>,
}

#[derive(Debug, Clone, Default)]
struct ResolvedSteamLibraries {
    steam_install_dir: PathBuf,
    libraryfolders_vdf_path: PathBuf,
    libraries: Vec<LibraryFolderEntry>,
    libraryfolders_found: bool,
}

pub fn find_steam_install_dir() -> Option<PathBuf> {
    let mut warnings = Vec::new();
    find_steam_install_dir_with_warnings(&mut warnings)
}

pub fn get_steam_library_dirs() -> Result<Vec<PathBuf>, String> {
    resolve_steam_libraries(None).map(|resolved| {
        resolved
            .libraries
            .into_iter()
            .map(|entry| entry.path)
            .collect()
    })
}

pub fn discover_workshop_sources(game: GameType, manual_path: Option<&str>) -> SteamDiscovery {
    let mut warnings = Vec::new();
    let steam_root = find_steam_install_dir_with_warnings(&mut warnings);
    let resolved = match resolve_steam_libraries(Some(game.app_id())) {
        Ok(resolved) => {
            crate::dev_log!(
                "[mod-profile-manager] steam discovery install_dir={} libraryfolders_vdf={} libraryfolders_found={} libraries={}",
                resolved.steam_install_dir.display(),
                resolved.libraryfolders_vdf_path.display(),
                resolved.libraryfolders_found,
                resolved.libraries.len()
            );
            Some(resolved)
        }
        Err(error) => {
            warnings.push(error.clone());
            crate::dev_log!(
                "[mod-profile-manager] steam discovery failed game={} error={}",
                game.as_str(),
                error
            );
            None
        }
    };

    let mut libraries = BTreeSet::new();
    if let Some(resolved) = resolved.as_ref() {
        for library in &resolved.libraries {
            libraries.insert(normalize_key(&library.path));
            crate::dev_log!(
                "[mod-profile-manager] steam library dir={} app_id={} in_library={}",
                library.path.display(),
                game.app_id(),
                library.app_ids.contains(game.app_id())
            );
        }
    } else if let Some(root) = steam_root.as_ref().filter(|path| path.is_dir()) {
        libraries.insert(normalize_key(root));
    }

    let library_paths = libraries
        .into_iter()
        .map(PathBuf::from)
        .collect::<Vec<_>>();
    let mut workshop_sources = library_paths
        .iter()
        .map(|library| WorkshopFolderSource {
            game,
            app_id: game.app_id().to_string(),
            path: library
                .join("steamapps")
                .join("workshop")
                .join("content")
                .join(game.app_id())
                .display()
                .to_string(),
            exists: library
                .join("steamapps")
                .join("workshop")
                .join("content")
                .join(game.app_id())
                .is_dir(),
            manual: false,
        })
        .collect::<Vec<_>>();

    if let Some(path) = manual_path.map(str::trim).filter(|value| !value.is_empty()) {
        let manual = PathBuf::from(path);
        if !workshop_sources
            .iter()
            .any(|item| normalize_key(Path::new(&item.path)) == normalize_key(&manual))
        {
            workshop_sources.push(WorkshopFolderSource {
                game,
                app_id: game.app_id().to_string(),
                path: manual.display().to_string(),
                exists: manual.is_dir(),
                manual: true,
            });
        }
    }

    SteamDiscovery {
        steam_install_found: steam_root
            .as_ref()
            .map(|path| path.is_dir())
            .unwrap_or(false),
        steam_root,
        libraries: library_paths,
        workshop_sources,
        warnings,
    }
}

pub(super) fn resolve_steam_libraries_for_app(
    app_id: Option<&str>,
) -> Result<Vec<(PathBuf, bool)>, String> {
    resolve_steam_libraries(app_id).map(|resolved| {
        resolved
            .libraries
            .into_iter()
            .map(|entry| {
                let contains_app = app_id
                    .map(|value| entry.app_ids.contains(value))
                    .unwrap_or(false);
                (entry.path, contains_app)
            })
            .collect()
    })
}

fn resolve_steam_libraries(app_id: Option<&str>) -> Result<ResolvedSteamLibraries, String> {
    let mut warnings = Vec::new();
    let steam_install_dir = find_steam_install_dir_with_warnings(&mut warnings)
        .ok_or_else(|| "steam_not_found".to_string())?;
    let libraryfolders_vdf_path = steam_install_dir.join("steamapps").join("libraryfolders.vdf");
    crate::dev_log!(
        "[mod-profile-manager] steam install dir={}",
        steam_install_dir.display()
    );
    crate::dev_log!(
        "[mod-profile-manager] libraryfolders_vdf={}",
        libraryfolders_vdf_path.display()
    );

    let mut deduped = BTreeSet::new();
    let mut libraries = Vec::new();
    let default_entry = LibraryFolderEntry {
        path: steam_install_dir.clone(),
        app_ids: BTreeSet::new(),
    };
    push_library_entry(&mut deduped, &mut libraries, default_entry);

    let (parsed_entries, libraryfolders_found) = if libraryfolders_vdf_path.is_file() {
        match read_library_folders(&libraryfolders_vdf_path) {
            Ok(entries) => (entries, true),
            Err(error) => {
                warnings.push(error.clone());
                crate::dev_log!(
                    "[mod-profile-manager] libraryfolders read failed path={} error={}",
                    libraryfolders_vdf_path.display(),
                    error
                );
                (Vec::new(), true)
            }
        }
    } else {
        crate::dev_log!(
            "[mod-profile-manager] libraryfolders.vdf not found at {}",
            libraryfolders_vdf_path.display()
        );
        (Vec::new(), false)
    };

    for entry in parsed_entries {
        push_library_entry(&mut deduped, &mut libraries, entry);
    }

    libraries.retain(|entry| entry.path.is_dir());
    if libraries.is_empty() {
        return Err("no_steam_libraries_found".to_string());
    }

    if let Some(app_id) = app_id {
        libraries.sort_by(|left, right| {
            right
                .app_ids
                .contains(app_id)
                .cmp(&left.app_ids.contains(app_id))
                .then_with(|| left.path.cmp(&right.path))
        });
    } else {
        libraries.sort_by(|left, right| left.path.cmp(&right.path));
    }

    for warning in warnings {
        crate::dev_log!("[mod-profile-manager] steam warning={}", warning);
    }
    for library in &libraries {
        crate::dev_log!(
            "[mod-profile-manager] steam library dir={} app_id={} in_library={}",
            library.path.display(),
            app_id.unwrap_or("-"),
            app_id
                .map(|value| library.app_ids.contains(value))
                .unwrap_or(false)
        );
    }

    Ok(ResolvedSteamLibraries {
        steam_install_dir,
        libraryfolders_vdf_path,
        libraries,
        libraryfolders_found,
    })
}

fn read_library_folders(vdf_path: &Path) -> Result<Vec<LibraryFolderEntry>, String> {
    let content = fs::read_to_string(vdf_path)
        .map_err(|error| format!("Failed to read {}: {}", vdf_path.display(), error))?;
    Ok(parse_library_folders_vdf(&content))
}

fn parse_library_folders_vdf(content: &str) -> Vec<LibraryFolderEntry> {
    let direct_value_regex = Regex::new(r#"^\s*"(\d+)"\s*"([^"]+)""#).ok();
    let block_header_open_regex = Regex::new(r#"^\s*"(\d+)"\s*\{\s*$"#).ok();
    let block_header_regex = Regex::new(r#"^\s*"(\d+)"\s*$"#).ok();
    let path_regex = Regex::new(r#"^\s*"path"\s*"([^"]+)""#).ok();
    let apps_open_regex = Regex::new(r#"^\s*"apps"\s*\{\s*$"#).ok();
    let apps_regex = Regex::new(r#"^\s*"apps"\s*$"#).ok();
    let app_entry_regex = Regex::new(r#"^\s*"(\d+)"\s*"([^"]*)""#).ok();

    #[derive(Debug, Default)]
    struct PendingLibrary {
        path: Option<PathBuf>,
        app_ids: BTreeSet<String>,
    }

    let mut libraries = Vec::new();
    let mut current_library = None::<PendingLibrary>;
    let mut pending_library_block = false;
    let mut pending_apps_block = false;
    let mut block_stack = Vec::<&str>::new();

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        if let Some(regex) = direct_value_regex.as_ref() {
            if let Some(captures) = regex.captures(trimmed) {
                let inside_apps = block_stack.last().copied() == Some("apps");
                if current_library.is_none() && !inside_apps {
                    let path = captures
                        .get(2)
                        .map(|value| normalize_vdf_path(value.as_str()))
                        .unwrap_or_default();
                    if !path.as_os_str().is_empty() {
                        libraries.push(LibraryFolderEntry {
                            path,
                            app_ids: BTreeSet::new(),
                        });
                    }
                    continue;
                }
            }
        }

        if let Some(regex) = block_header_open_regex.as_ref() {
            if regex.is_match(trimmed) {
                current_library = Some(PendingLibrary::default());
                block_stack.push("library");
                continue;
            }
        }

        if let Some(regex) = block_header_regex.as_ref() {
            if regex.is_match(trimmed) {
                pending_library_block = true;
                continue;
            }
        }

        if let Some(regex) = path_regex.as_ref() {
            if let Some(captures) = regex.captures(trimmed) {
                if let Some(library) = current_library.as_mut() {
                    library.path = captures
                        .get(1)
                        .map(|value| normalize_vdf_path(value.as_str()));
                }
                continue;
            }
        }

        if let Some(regex) = apps_open_regex.as_ref() {
            if regex.is_match(trimmed) {
                pending_apps_block = false;
                block_stack.push("apps");
                continue;
            }
        }

        if let Some(regex) = apps_regex.as_ref() {
            if regex.is_match(trimmed) {
                pending_apps_block = true;
                continue;
            }
        }

        if let Some(regex) = app_entry_regex.as_ref() {
            if let Some(captures) = regex.captures(trimmed) {
                let inside_apps = block_stack.last().copied() == Some("apps");
                if inside_apps {
                    if let Some(library) = current_library.as_mut() {
                        if let Some(app_id) = captures.get(1).map(|value| value.as_str().to_string()) {
                            library.app_ids.insert(app_id);
                        }
                    }
                }
                continue;
            }
        }

        if trimmed == "{" {
            if pending_library_block {
                current_library = Some(PendingLibrary::default());
                block_stack.push("library");
                pending_library_block = false;
                continue;
            }
            if pending_apps_block {
                block_stack.push("apps");
                pending_apps_block = false;
                continue;
            }
            block_stack.push("other");
            continue;
        }

        if trimmed == "}" {
            if let Some(block) = block_stack.pop() {
                if block == "library" {
                    if let Some(library) = current_library.take() {
                        if let Some(path) = library.path.filter(|value| !value.as_os_str().is_empty()) {
                            libraries.push(LibraryFolderEntry {
                                path,
                                app_ids: library.app_ids,
                            });
                        }
                    }
                }
            }
        }
    }

    libraries
}

fn push_library_entry(
    deduped: &mut BTreeSet<String>,
    libraries: &mut Vec<LibraryFolderEntry>,
    entry: LibraryFolderEntry,
) {
    let key = normalize_key(&entry.path);
    if !deduped.insert(key.clone()) {
        if let Some(existing) = libraries
            .iter_mut()
            .find(|library| normalize_key(&library.path) == key)
        {
            existing.app_ids.extend(entry.app_ids);
        }
        return;
    }
    libraries.push(entry);
}

fn find_steam_install_dir_with_warnings(warnings: &mut Vec<String>) -> Option<PathBuf> {
    let mut candidates = registry_steam_candidates();
    candidates.extend(standard_steam_roots());

    for candidate in dedupe_paths(candidates) {
        let normalized = normalize_path(candidate);
        let library_vdf = normalized.join("steamapps").join("libraryfolders.vdf");
        crate::dev_log!(
            "[mod-profile-manager] steam candidate={} libraryfolders_exists={}",
            normalized.display(),
            library_vdf.is_file()
        );
        if normalized.is_dir() && library_vdf.is_file() {
            return Some(normalized);
        }
    }

    for candidate in dedupe_paths(registry_steam_candidates()) {
        let normalized = normalize_path(candidate);
        if normalized.is_dir() {
            warnings.push(format!(
                "Steam installation found at {}, but libraryfolders.vdf is missing.",
                normalized.display()
            ));
            return Some(normalized);
        }
    }

    for candidate in dedupe_paths(standard_steam_roots()) {
        let normalized = normalize_path(candidate);
        if normalized.is_dir() {
            warnings.push(format!(
                "Steam fallback path found at {}, but libraryfolders.vdf is missing.",
                normalized.display()
            ));
            return Some(normalized);
        }
    }

    warnings.push("Steam installation directory could not be found.".to_string());
    None
}

#[cfg(target_os = "windows")]
fn registry_steam_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);

    push_registry_value(
        &mut candidates,
        &hkcu,
        r"Software\Valve\Steam",
        "SteamPath",
    );
    push_registry_value(
        &mut candidates,
        &hkcu,
        r"Software\Valve\Steam",
        "SteamExe",
    );
    push_registry_value(
        &mut candidates,
        &hkcu,
        r"Software\Valve\Steam",
        "InstallPath",
    );
    push_registry_value(
        &mut candidates,
        &hklm,
        r"SOFTWARE\WOW6432Node\Valve\Steam",
        "InstallPath",
    );
    push_registry_value(
        &mut candidates,
        &hklm,
        r"SOFTWARE\Valve\Steam",
        "InstallPath",
    );

    candidates
}

#[cfg(target_os = "windows")]
fn push_registry_value(
    candidates: &mut Vec<PathBuf>,
    root: &RegKey,
    key_path: &str,
    value_name: &str,
) {
    if let Ok(key) = root.open_subkey(key_path) {
        if let Ok(value) = key.get_value::<String, _>(value_name) {
            let path = PathBuf::from(value);
            let normalized = if path
                .extension()
                .and_then(|value| value.to_str())
                .map(|value| value.eq_ignore_ascii_case("exe"))
                .unwrap_or(false)
            {
                path.parent().map(PathBuf::from).unwrap_or(path)
            } else {
                path
            };
            candidates.push(normalized);
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn registry_steam_candidates() -> Vec<PathBuf> {
    Vec::new()
}

fn standard_steam_roots() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    if let Some(value) = env::var_os("ProgramFiles(x86)") {
        paths.push(PathBuf::from(value).join("Steam"));
    }
    if let Some(value) = env::var_os("PROGRAMFILES(X86)") {
        paths.push(PathBuf::from(value).join("Steam"));
    }
    if let Some(value) = env::var_os("ProgramFiles") {
        paths.push(PathBuf::from(value).join("Steam"));
    }
    if let Some(value) = env::var_os("PROGRAMFILES") {
        paths.push(PathBuf::from(value).join("Steam"));
    }

    paths.push(PathBuf::from(r"C:\Program Files (x86)\Steam"));
    paths.push(PathBuf::from(r"C:\Program Files\Steam"));

    paths
}

fn dedupe_paths(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut unique = BTreeSet::new();
    paths
        .into_iter()
        .map(normalize_path)
        .filter(|path| unique.insert(normalize_key(path)))
        .collect()
}

fn normalize_vdf_path(value: &str) -> PathBuf {
    normalize_path(PathBuf::from(value.replace("\\\\", "\\")))
}

fn normalize_path(path: PathBuf) -> PathBuf {
    let text = path
        .display()
        .to_string()
        .replace('/', "\\")
        .trim()
        .trim_matches('"')
        .to_string();
    PathBuf::from(text)
}

fn normalize_key(path: &Path) -> String {
    path.display().to_string().replace('\\', "/").to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_old_libraryfolders_format() {
        let content = r#"
"LibraryFolders"
{
  "TimeNextStatsReport" "123"
  "1" "D:\\SteamLibrary"
  "2" "E:\\SteamLibrary"
}
"#;

        let entries = parse_library_folders_vdf(content);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].path, PathBuf::from(r"D:\SteamLibrary"));
        assert_eq!(entries[1].path, PathBuf::from(r"E:\SteamLibrary"));
    }

    #[test]
    fn parses_new_libraryfolders_format_and_apps() {
        let content = r#"
"libraryfolders"
{
  "0"
  {
    "path" "C:\\Program Files (x86)\\Steam"
    "apps"
    {
      "227300" "123456"
    }
  }
  "1"
  {
    "path" "D:\\SteamLibrary"
    "apps"
    {
      "730" "1"
    }
  }
}
"#;

        let entries = parse_library_folders_vdf(content);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].path, PathBuf::from(r"C:\Program Files (x86)\Steam"));
        assert!(entries[0].app_ids.contains("227300"));
        assert_eq!(entries[1].path, PathBuf::from(r"D:\SteamLibrary"));
        assert!(entries[1].app_ids.contains("730"));
    }
}
