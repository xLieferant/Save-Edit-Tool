use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogContext {
    pub selected_game: Option<String>,
    pub profile_name: Option<String>,
    pub save_name: Option<String>,
    pub profile_reference: Option<String>,
    pub save_reference: Option<String>,
    pub extra: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeReportEntry {
    pub report_id: String,
    pub created_at_utc: String,
    pub level: String,
    pub action: String,
    pub profile_name: Option<String>,
    pub save_name: Option<String>,
    pub error_code: Option<String>,
    pub user_message: String,
    pub technical_details: Option<String>,
    pub context: LogContext,
}
