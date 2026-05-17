use serde::{Deserialize, Serialize};

fn default_backup_type() -> String {
    "Auto".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupFileRecord {
    pub relative_path: String,
    pub live_path: String,
    pub stored_path: String,
    pub size_bytes: u64,
    pub checksum: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupMetadataFile {
    pub backup_id: String,
    pub save_session_id: Option<String>,
    pub profile_reference: Option<String>,
    pub save_reference: Option<String>,
    pub profile_name: Option<String>,
    pub save_name: Option<String>,
    pub action_reason: String,
    #[serde(default = "default_backup_type")]
    pub backup_type: String,
    pub created_at_utc: String,
    pub files: Vec<BackupFileRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupVersionDto {
    pub backup_id: String,
    pub created_at_utc: String,
    pub profile_name: Option<String>,
    pub save_name: Option<String>,
    pub action_reason: String,
    pub backup_type: String,
    pub file_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupDiffValueDto {
    pub key: String,
    pub previous_value: String,
    pub next_value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupDiffFileDto {
    pub relative_path: String,
    pub status: String,
    pub parseable: bool,
    pub change_count: usize,
    pub checksum_before: String,
    pub checksum_after: String,
    pub changes: Vec<BackupDiffValueDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupRestorePreviewDto {
    pub backup_id: String,
    pub created_at_utc: String,
    pub profile_name: Option<String>,
    pub save_name: Option<String>,
    pub action_reason: String,
    pub backup_type: String,
    pub files: Vec<BackupDiffFileDto>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupCreateResultDto {
    pub backup_id: String,
    pub created_at_utc: String,
    pub backup_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupRestoreResultDto {
    pub backup_id: String,
    pub restored_file_count: usize,
    pub safety_backup_id: Option<String>,
}
