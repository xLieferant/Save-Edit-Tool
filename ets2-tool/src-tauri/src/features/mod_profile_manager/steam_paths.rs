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

pub fn discover_workshop_sources(game: GameType, manual_path: Option<&str>) -> SteamDiscovery {
    let mut warnings = Vec::new();
    let steam_root = detect_steam_root(&mut warnings);
    let mut libraries = BTreeSet::new();

    if let Some(root) = steam_root.as_ref() {
        libraries.insert(normalize_key(root));
    }

    if let Some(root) = steam_root.as_ref() {
        let vdf_path = root.join("steamapps").join("libraryfolders.vdf");
        for library in read_library_folders(&vdf_path, &mut warnings) {
            libraries.insert(normalize_key(&library));
        }
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

fn read_library_folders(vdf_path: &Path, warnings: &mut Vec<String>) -> Vec<PathBuf> {
    if !vdf_path.is_file() {
        return Vec::new();
    }

    let content = match fs::read(vdf_path) {
        Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
        Err(error) => {
            warnings.push(format!(
                "Could not read Steam library file {}: {}",
                vdf_path.display(),
                error
            ));
            return Vec::new();
        }
    };

    let Some(regex) = Regex::new(r#""path"\s*"([^"]+)""#).ok() else {
        warnings.push("Could not parse Steam library folders because the regex failed.".to_string());
        return Vec::new();
    };

    regex
        .captures_iter(&content)
        .filter_map(|capture| capture.get(1).map(|value| value.as_str().replace("\\\\", "\\")))
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from)
        .collect()
}

fn detect_steam_root(warnings: &mut Vec<String>) -> Option<PathBuf> {
    let mut candidates = Vec::new();

    if let Some(path) = registry_steam_root() {
        candidates.push(path);
    } else {
        warnings.push("Steam registry path was not available.".to_string());
    }

    candidates.extend(standard_steam_roots());

    candidates.into_iter().find(|path| path.is_dir())
}

#[cfg(target_os = "windows")]
fn registry_steam_root() -> Option<PathBuf> {
    let key = RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey(r"Software\Valve\Steam")
        .ok()?;
    let path: String = key
        .get_value("SteamPath")
        .or_else(|_| key.get_value("InstallPath"))
        .ok()?;
    Some(PathBuf::from(path))
}

#[cfg(not(target_os = "windows"))]
fn registry_steam_root() -> Option<PathBuf> {
    None
}

fn standard_steam_roots() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    if let Ok(value) = env::var("PROGRAMFILES(X86)") {
        paths.push(PathBuf::from(value).join("Steam"));
    }
    if let Ok(value) = env::var("PROGRAMFILES") {
        paths.push(PathBuf::from(value).join("Steam"));
    }

    paths.push(PathBuf::from(r"C:\Program Files (x86)\Steam"));
    paths.push(PathBuf::from(r"C:\Program Files\Steam"));

    let mut unique = BTreeSet::new();
    paths
        .into_iter()
        .filter(|path| unique.insert(normalize_key(path)))
        .collect()
}

fn normalize_key(path: &Path) -> String {
    path.display().to_string().replace('\\', "/").to_ascii_lowercase()
}
