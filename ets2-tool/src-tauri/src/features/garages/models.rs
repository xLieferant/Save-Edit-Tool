use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GarageActionResult {
    pub action: String,
    pub implemented: bool,
}
