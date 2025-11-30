use serde::Serialize;

#[derive(Serialize)]
pub struct SaveGameConfig {
    pub factor_parked: Option<i64>,
}
