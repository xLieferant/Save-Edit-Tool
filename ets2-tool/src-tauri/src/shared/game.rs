use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum GameType {
    #[default]
    #[serde(rename = "ets2")]
    Ets2,
    #[serde(rename = "ats")]
    Ats,
}

impl GameType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ets2 => "ets2",
            Self::Ats => "ats",
        }
    }

    pub fn app_id(self) -> &'static str {
        match self {
            Self::Ets2 => "227300",
            Self::Ats => "270880",
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::Ets2 => "ETS2",
            Self::Ats => "ATS",
        }
    }
}

impl TryFrom<&str> for GameType {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value.trim().to_ascii_lowercase().as_str() {
            "ets2" => Ok(Self::Ets2),
            "ats" => Ok(Self::Ats),
            other => Err(format!("Unsupported game: {}", other)),
        }
    }
}

/// Steam userdata lives under the client installation, not a game's library.
pub fn steam_install_dir() -> Option<std::path::PathBuf> {
    #[cfg(windows)]
    {
        use winreg::{RegKey, enums::*};
        let user = RegKey::predef(HKEY_CURRENT_USER);
        if let Ok(key) = user.open_subkey("Software\\Valve\\Steam") {
            if let Ok(path) = key.get_value::<String, _>("SteamPath") {
                return Some(path.into());
            }
        }
        for env in ["ProgramFiles(x86)", "ProgramFiles"] {
            if let Some(base) = std::env::var_os(env) {
                let path = std::path::PathBuf::from(base).join("Steam");
                if path.is_dir() {
                    return Some(path);
                }
            }
        }
    }
    let home = dirs::home_dir()?;
    [
        home.join(".steam/steam"),
        home.join(".local/share/Steam"),
        home.join("Library/Application Support/Steam"),
    ]
    .into_iter()
    .find(|path| path.is_dir())
}
