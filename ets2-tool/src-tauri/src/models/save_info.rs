use crate::models::profile_info::SaveKind;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct SaveInfo {
    pub path: String,
    pub folder: String,
    pub name: Option<String>,
    pub success: bool,
    pub message: Option<String>,
    pub kind: SaveKind, // NEU
}
