use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveHealthProblemDto {
    pub id: String,
    pub severity: String,
    pub category: String,
    pub title: String,
    pub description: String,
    pub suggestion: String,
    pub auto_fix_available: bool,
    pub fix_id: Option<String>,
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveHealthReportDto {
    pub generated_at_utc: String,
    pub status: String,
    pub profile_name: Option<String>,
    pub save_name: Option<String>,
    pub summary: String,
    pub problem_count: usize,
    pub fixable_count: usize,
    pub problems: Vec<SaveHealthProblemDto>,
    pub mod_scan_pending: bool,
    pub mod_scan_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveHealthFixResultDto {
    pub fix_id: String,
    pub applied: bool,
    pub message: String,
}
