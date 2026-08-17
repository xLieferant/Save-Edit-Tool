use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AiDriverPoolEntry {
    pub index: usize,
    pub driver_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AiDriverPoolDiagnostics {
    pub declared_count: usize,
    pub parsed_entry_count: usize,
    pub unique_driver_count: usize,
    pub duplicate_driver_count: usize,
    pub assigned_driver_count: usize,
    pub available_driver_count: usize,
    pub missing_driver_block_count: usize,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AiDriverPoolSnapshot {
    pub profile_id: String,
    pub save_id: String,
    pub save_hash: String,
    pub driver_pool_count: usize,
    pub available_driver_count: usize,
    pub assigned_driver_count: usize,
    pub drivers: Vec<AiDriverPoolEntry>,
    pub available_driver_ids: Vec<String>,
    pub assigned_driver_ids: Vec<String>,
    pub diagnostics: AiDriverPoolDiagnostics,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DriverAssignmentRequest {
    pub garage_id: String,
    pub expected_save_hash: String,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DriverRefAssignmentRequest {
    pub garage_id: String,
    pub expected_save_hash: String,
    pub driver_ref: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DriverAssignmentResult {
    pub garage_id: String,
    pub assigned_count: usize,
    pub assigned_driver_ids: Vec<String>,
    pub remaining_free_slots: usize,
    pub remaining_pool_size: usize,
    pub backup_id: String,
    pub backup_created: bool,
    pub verified: bool,
    pub save_hash: String,
    pub warnings: Vec<String>,
}
