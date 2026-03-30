#![allow(dead_code)]

use std::path::{Path, PathBuf};

#[cfg(target_os = "windows")]
use regex::Regex;
#[cfg(target_os = "windows")]
use winreg::{enums::*, RegKey};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScsGame {
    Ets2,
    Ats,
}

impl ScsGame {
    fn app_id(self) -> u32 {
        match self {
            Self::Ets2 => 227300,
            Self::Ats => 270880,
        }
    }

    fn registry_game_name(self) -> &'static str {
        match self {
            Self::Ets2 => "Euro Truck Simulator 2",
            Self::Ats => "American Truck Simulator",
        }
    }

    fn common_dir_name(self) -> &'static str {
        self.registry_game_name()
    }

    fn exe_name(self) -> &'static str {
        match self {
            Self::Ets2 => "eurotrucks2.exe",
            Self::Ats => "amtrucks.exe",
        }
    }
}

#[derive(Debug, Clone)]
pub struct GameInstall {
    pub game: ScsGame,
    pub install_root: PathBuf,
    pub binary_dir: PathBuf,
    pub exe_path: PathBuf,
    pub plugin_dir: PathBuf,
}

#[cfg(target_os = "windows")]
pub fn install_plugin_registry_entry(
    game: ScsGame,
    value_name: &str,
    plugin_dll: &Path,
) -> Result<GameInstall, String> {
    let install = find_game_installation(game)?;
    let dll = plugin_dll
        .canonicalize()
        .map_err(|e| format!("Failed to canonicalize DLL: {e}"))?;

    if !dll.is_file() {
        return Err(format!("Plugin DLL not found: {}", dll.display()));
    }

    let flags = if install.binary_dir.ends_with("win_x86") {
        KEY_WRITE | KEY_WOW64_32KEY
    } else {
        KEY_WRITE | KEY_WOW64_64KEY
    };

    let key_path = format!(
        r"SOFTWARE\SCS Software\{}\Plugins",
        game.registry_game_name()
    );

    let hkml = RegKey::predef(HKEY_LOCAL_MACHINE);
    let (plugins_key, _) = hkml
        .create_subkey_with_flags(&key_path, flags)
        .map_err(|e| format!("Failed to create HKLM key. Run elevated: {e}"))?;

    plugins_key
        .set_value(value_name, &dll.to_string_lossy().to_string())
        .map_err(|e| format!("Failed to write registry value: {e}"))?;

    Ok(install)
}

#[cfg(not(target_os = "windows"))]
pub fn install_plugin_registry_entry(
    _game: ScsGame,
    _value_name: &str,
    _plugin_dll: &Path,
) -> Result<GameInstall, String> {
    Err("Plugin registry installation is only supported on Windows".to_string())
}

#[cfg(target_os = "windows")]
pub fn find_game_installation(game: ScsGame) -> Result<GameInstall, String> {
    let mut candidates = uninstall_locations(game);
    candidates.extend(steam_library_locations(game)?);

    for root in candidates {
        if let Some(found) = validate_install_root(game, &root) {
            return Ok(found);
        }
    }

    Err(format!(
        "Could not locate {} installation",
        game.registry_game_name()
    ))
}

#[cfg(not(target_os = "windows"))]
pub fn find_game_installation(_game: ScsGame) -> Result<GameInstall, String> {
    Err("Game installation lookup is only supported on Windows".to_string())
}

#[cfg(target_os = "windows")]
fn uninstall_locations(game: ScsGame) -> Vec<PathBuf> {
    let uninstall_key = format!(
        r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\Steam App {}",
        game.app_id()
    );

    let mut out = Vec::new();
    for flags in [KEY_READ | KEY_WOW64_64KEY, KEY_READ | KEY_WOW64_32KEY] {
        if let Ok(key) = RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey_with_flags(&uninstall_key, flags) {
            if let Ok(path) = key.get_value::<String, _>("InstallLocation") {
                out.push(PathBuf::from(path));
            }
        }
    }
    out
}

#[cfg(target_os = "windows")]
fn steam_library_locations(game: ScsGame) -> Result<Vec<PathBuf>, String> {
    let steam_key = RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey(r"Software\Valve\Steam")
        .map_err(|e| format!("Steam registry key not found: {e}"))?;

    let steam_path: String = steam_key
        .get_value("SteamPath")
        .or_else(|_| steam_key.get_value("InstallPath"))
        .map_err(|e| format!("Steam install path missing: {e}"))?;

    let steam_root = PathBuf::from(steam_path);
    let vdf_path = steam_root.join("steamapps").join("libraryfolders.vdf");
    let vdf = std::fs::read_to_string(&vdf_path)
        .map_err(|e| format!("Failed to read {}: {e}", vdf_path.display()))?;

    let re = Regex::new(r#""path"\s*"([^"]+)""#).map_err(|e| e.to_string())?;
    let mut libraries = vec![steam_root];

    for caps in re.captures_iter(&vdf) {
        libraries.push(PathBuf::from(caps[1].replace("\\\\", "\\")));
    }

    let manifest = format!("appmanifest_{}.acf", game.app_id());

    Ok(libraries
        .into_iter()
        .filter(|lib| lib.join("steamapps").join(&manifest).is_file())
        .map(|lib| lib.join("steamapps").join("common").join(game.common_dir_name()))
        .collect())
}

#[cfg(target_os = "windows")]
fn validate_install_root(game: ScsGame, root: &Path) -> Option<GameInstall> {
    for arch in ["win_x64", "win_x86"] {
        let binary_dir = root.join("bin").join(arch);
        let exe_path = binary_dir.join(game.exe_name());
        if exe_path.is_file() {
            return Some(GameInstall {
                game,
                install_root: root.to_path_buf(),
                plugin_dir: binary_dir.join("plugins"),
                binary_dir,
                exe_path,
            });
        }
    }
    None
}
