use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct CachedProfile {
    pub path: String,
    pub name: Option<String>,
    pub success: bool,
    pub message: Option<String>,
}
