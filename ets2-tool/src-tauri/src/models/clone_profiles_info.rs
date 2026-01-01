use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloneOptions {
    pub backup: bool,
    pub replace_hex: bool,
    pub replace_text: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct CloneTargetStatus {
    pub valid: bool,
    pub message: String,
    pub target_path: Option<String>,
}
