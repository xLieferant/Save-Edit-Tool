use serde::Serialize;

#[derive(Serialize)]
pub struct ProfileInfo {
    pub path: String,
    pub name: Option<String>,
    pub success: bool,
    pub message: Option<String>,
}
