use serde::{de, Deserialize, Deserializer, Serialize};

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkshopMod {
    #[serde(deserialize_with = "deserialize_u64_from_string_or_number")]
    pub id: u64,
    #[serde(alias = "title")]
    pub name: String,
    #[serde(default = "default_ets2_app_id")]
    pub app_id: u32,
    pub enabled: bool,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkshopInstallStatus {
    pub mod_id: String,
    pub app_id: String,
    pub installed: bool,
    pub workshop_path: Option<String>,
    pub checked_libraries: Vec<String>,
    pub checked_paths: Vec<String>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModSandbox {
    pub id: String,
    pub title: String,
    pub description: String,
    pub mods: Vec<WorkshopMod>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct SandboxCollection {
    pub sandboxes: Vec<ModSandbox>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ApplySandboxResult {
    pub sandbox_id: String,
    pub sandbox_title: String,
    pub game_sii_path: String,
    pub backup_path: String,
    pub applied_mods: Vec<AppliedWorkshopMod>,
    pub skipped_mods: Vec<SkippedWorkshopMod>,
    pub removed_existing_mod_count: usize,
    pub applied_mod_count: usize,
    pub validation: ValidateActivePresetModsResult,
    pub success: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AppliedWorkshopMod {
    pub mod_id: String,
    pub title: Option<String>,
    pub workshop_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkippedWorkshopMod {
    pub mod_id: String,
    pub title: Option<String>,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReplaceActivePresetModsResult {
    pub content: String,
    pub removed_mod_count: usize,
    pub written_mod_count: usize,
    pub expected_mod_refs: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ValidateActivePresetModsResult {
    pub success: bool,
    pub expected_count: usize,
    pub actual_count: usize,
    pub expected_mod_refs: Vec<String>,
    pub actual_mod_refs: Vec<String>,
    pub missing_mod_refs: Vec<String>,
    pub unexpected_mod_refs: Vec<String>,
    pub order_matches: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ActiveModBlockSnapshot {
    pub field_name: String,
    pub count: usize,
    pub mod_refs: Vec<String>,
    pub indices: Vec<usize>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct SandboxPresetCollection {
    pub sandbox_presets: Vec<SandboxModPreset>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct SandboxModPreset {
    pub id: String,
    #[serde(alias = "name")]
    pub title: String,
    pub description: Option<String>,
    #[serde(default)]
    pub game: Option<String>,
    #[serde(default)]
    pub app_id: Option<u32>,
    #[serde(default)]
    pub enabled: Option<bool>,
    #[serde(default)]
    pub load_order_locked: Option<bool>,
    #[serde(default)]
    pub mods: Vec<SandboxPresetMod>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct SandboxPresetMod {
    #[serde(default, alias = "workshop_id", alias = "id")]
    pub steam_id: String,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub package_id: Option<String>,
    #[serde(default)]
    pub active_mods_value: Option<String>,
    #[serde(default = "default_ets2_app_id")]
    pub app_id: u32,
    #[serde(default)]
    pub required: bool,
    #[serde(default, alias = "name")]
    pub display_name: Option<String>,
    #[serde(default)]
    pub load_order: usize,
    #[serde(default)]
    pub workshop_url: Option<String>,
    #[serde(default)]
    pub steam_protocol_url: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct SandboxPresetModStatus {
    pub steam_id: String,
    pub source: Option<String>,
    pub package_id: Option<String>,
    pub active_mods_value: Option<String>,
    pub app_id: u32,
    pub game: String,
    pub display_name: Option<String>,
    pub required: bool,
    pub load_order: usize,
    pub found: bool,
    pub available: bool,
    pub reachable: bool,
    pub status: String,
    pub local_path: Option<String>,
    pub workshop_url: String,
    pub steam_protocol_url: String,
    pub steamcmd_command: String,
    pub checked_paths: Vec<String>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct SandboxPresetCheckResult {
    pub preset_id: String,
    pub title: String,
    pub ready: bool,
    pub can_activate: bool,
    pub mods: Vec<SandboxPresetModStatus>,
    pub missing_required_mods: Vec<SandboxPresetModStatus>,
    pub missing_mods: Vec<SandboxPresetModStatus>,
    pub found_mods: Vec<SandboxPresetModStatus>,
    pub all_mods: Vec<SandboxPresetModStatus>,
    pub checked_libraries: Vec<String>,
    pub checked_at: String,
    pub message: String,
    pub progress_log: Vec<String>,
    pub cache_path: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct SandboxPresetActivationResult {
    pub preset_id: String,
    pub preset_name: String,
    pub title: String,
    pub success: bool,
    pub error_code: Option<String>,
    pub written_mods: Vec<String>,
    pub verified_mods: Vec<String>,
    pub written_mod_refs: Vec<String>,
    pub backup_path: Option<String>,
    pub cache_path: Option<String>,
    pub mod_cache_path: Option<String>,
    pub save_path: Option<String>,
    pub profile_id: String,
    pub save_name: Option<String>,
    pub app_id: u32,
    pub target_profile_sii_path: String,
    pub mods_written: Vec<ActivatedModEntry>,
    pub missing_mods: Vec<ActivationMissingModEntry>,
    pub verification: ActivationVerification,
    pub message: String,
    pub progress_log: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ActivatedModEntry {
    pub index: usize,
    pub workshop_id: Option<String>,
    pub app_id: Option<u32>,
    pub display_name: String,
    pub active_mods_value: String,
    pub local_path: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ActivationMissingModEntry {
    pub workshop_id: String,
    pub app_id: u32,
    pub display_name: Option<String>,
    pub workshop_url: String,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ActivationVerification {
    pub expected_count: usize,
    pub actual_count: usize,
    pub order_matches: bool,
    pub values_match: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct SandboxModCacheEntry {
    pub preset_id: String,
    pub title: String,
    pub checked_at: String,
    pub checked_libraries: Vec<String>,
    pub mods: Vec<SandboxPresetModStatus>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct SandboxModCacheFile {
    pub entries: Vec<SandboxModCacheEntry>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct SteamWorkshopMod {
    pub game: String,
    pub app_id: u32,
    pub workshop_id: String,
    pub installed: bool,
    pub available: bool,
    pub reachable: bool,
    pub local_path: String,
    pub workshop_url: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct SteamWorkshopCache {
    pub generated_at: String,
    pub mods: Vec<SteamWorkshopMod>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct SandboxActiveModsBackupCacheEntry {
    pub preset_id: String,
    pub title: String,
    pub save_path: String,
    pub file_path: String,
    pub field_name: String,
    pub original_count: usize,
    pub original_mod_refs: Vec<String>,
    pub original_indices: Vec<usize>,
    pub timestamp: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct SandboxActiveModsBackupCacheFile {
    pub entries: Vec<SandboxActiveModsBackupCacheEntry>,
}

fn default_ets2_app_id() -> u32 {
    227300
}

fn deserialize_u64_from_string_or_number<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: Deserializer<'de>,
{
    struct U64StringOrNumberVisitor;

    impl<'de> de::Visitor<'de> for U64StringOrNumberVisitor {
        type Value = u64;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a numeric Workshop ID as number or string")
        }

        fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E> {
            Ok(value)
        }

        fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            u64::try_from(value).map_err(|_| E::custom("Workshop ID must not be negative"))
        }

        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            value
                .trim()
                .parse::<u64>()
                .map_err(|error| E::custom(format!("Invalid Workshop ID: {error}")))
        }

        fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            self.visit_str(&value)
        }
    }

    deserializer.deserialize_any(U64StringOrNumberVisitor)
}
