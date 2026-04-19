use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileShareContext {
    pub selected_game: String,
    pub profile_name: Option<String>,
    pub profile_path: Option<String>,
    pub default_export_dir: Option<String>,
    pub default_archive_name: Option<String>,
    pub import_target_dir: Option<String>,
    pub can_export: bool,
    pub can_import: bool,
    pub path_resolution_error: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileShareExportResult {
    pub profile_name: String,
    pub archive_name: String,
    pub archive_path: String,
    pub export_dir: String,
    pub exported_files: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileShareImportPreview {
    pub archive_path: String,
    pub detected_profile_name: String,
    pub suggested_profile_name: String,
    pub final_profile_name: String,
    pub target_profile_path: String,
    pub import_target_dir: String,
    pub archive_root: String,
    pub has_manifest: bool,
    pub file_count: usize,
    pub profile_name_conflict: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileShareImportResult {
    pub profile_name: String,
    pub profile_path: String,
    pub archive_path: String,
    pub imported_files: usize,
    pub import_target_dir: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SharedProfileManifest {
    pub archive_format: String,
    pub archive_version: u32,
    pub exported_at: String,
    pub game: String,
    pub profile_name: String,
    pub source_profile_folder: String,
    pub profile_root: String,
}
