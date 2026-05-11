use serde::{Deserialize, Serialize};

fn default_load_order_unknown() -> String {
    "unknown".to_string()
}

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ModSource {
    #[default]
    Unknown,
    LocalModFolder,
    SteamWorkshop,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
pub enum ModCategory {
    Truck,
    Trailer,
    Map,
    Cargo,
    Traffic,
    Economy,
    Sound,
    Ui,
    Graphics,
    Tuning,
    Skin,
    Other,
    #[default]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DiscoveredMod {
    pub id: String,
    pub source: ModSource,
    pub name: String,
    pub file_path: String,
    pub file_kind: String,
    pub size_bytes: Option<u64>,
    pub modified_at: Option<String>,
    pub workshop_id: Option<String>,
    pub app_id: Option<String>,
    pub manifest_name: Option<String>,
    pub version: Option<String>,
    pub author: Option<String>,
    pub categories: Vec<ModCategory>,
    pub readable: bool,
    pub enabled: Option<bool>,
    pub load_order_index: Option<i32>,
    pub load_order_source: String,
    pub status: String,
    pub workshop_url: Option<String>,
    pub manifest_present: bool,
    pub duplicate_key: String,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PresetModEntry {
    pub mod_id: String,
    pub name: String,
    pub source: ModSource,
    pub file_path: String,
    pub workshop_id: Option<String>,
    pub app_id: Option<String>,
    pub enabled: bool,
    pub load_order_index: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModPreset {
    pub id: String,
    pub name: String,
    pub game: GameType,
    pub created_at: String,
    pub updated_at: String,
    pub mods: Vec<PresetModEntry>,
    pub notes: Option<String>,
    pub preset_label: Option<String>,
    #[serde(default = "default_load_order_unknown")]
    pub load_order_source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WorkshopFolderSource {
    pub game: GameType,
    pub app_id: String,
    pub path: String,
    pub exists: bool,
    pub manual: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModScanSummary {
    pub selected_game: GameType,
    pub scan_mode: String,
    pub scan_timed_out: bool,
    pub local_mod_folder_path: Option<String>,
    pub local_mod_folder_found: bool,
    pub steam_install_found: bool,
    pub steam_library_paths: Vec<String>,
    pub workshop_sources: Vec<WorkshopFolderSource>,
    pub manual_workshop_path: Option<String>,
    pub local_mods_found: usize,
    pub steam_workshop_mods_found: usize,
    pub unreadable_mods_count: usize,
    pub presets_saved: usize,
    pub active_mods_reliably_known: bool,
    pub active_mods_count: usize,
    pub load_order_source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModManagerLogPaths {
    pub technical_log_path: Option<String>,
    pub user_log_path: Option<String>,
    pub log_directory_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModProfileManagerState {
    pub summary: ModScanSummary,
    pub mods: Vec<DiscoveredMod>,
    pub presets: Vec<ModPreset>,
    pub warnings: Vec<String>,
    pub current_profile_path: Option<String>,
    pub logs: ModManagerLogPaths,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MissingModEntry {
    pub preset_mod: PresetModEntry,
    pub reason: String,
    pub workshop_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ExtraModEntry {
    pub current_mod: DiscoveredMod,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ChangedPathEntry {
    pub preset_mod: PresetModEntry,
    pub current_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LoadOrderDifference {
    pub preset_mod: PresetModEntry,
    pub current_index: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DuplicateModEntry {
    pub mod_id: String,
    pub name: String,
    pub file_paths: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CompareSummary {
    pub missing_mods_count: usize,
    pub extra_mods_count: usize,
    pub changed_paths_count: usize,
    pub load_order_differences_count: usize,
    pub unreadable_mods_count: usize,
    pub duplicate_mods_count: usize,
    pub workshop_links_count: usize,
    pub active_mods_reliably_known: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PresetCompareResult {
    pub preset: ModPreset,
    pub game: GameType,
    pub generated_at: String,
    pub missing_mods: Vec<MissingModEntry>,
    pub extra_mods: Vec<ExtraModEntry>,
    pub changed_paths: Vec<ChangedPathEntry>,
    pub load_order_differences: Vec<LoadOrderDifference>,
    pub unreadable_mods: Vec<DiscoveredMod>,
    pub duplicate_mods: Vec<DuplicateModEntry>,
    pub workshop_links: Vec<String>,
    pub summary: CompareSummary,
    pub warnings: Vec<String>,
    pub load_order_source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub(crate) struct ManualWorkshopPath {
    pub game: GameType,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub(crate) struct ModProfileManagerSettings {
    pub manual_workshop_paths: Vec<ManualWorkshopPath>,
}
