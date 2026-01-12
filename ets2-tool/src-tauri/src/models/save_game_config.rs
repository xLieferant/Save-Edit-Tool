use serde::Serialize;

#[derive(Serialize, Clone)]
pub struct SaveGameConfig {
    pub factor_parking_doubles: Option<i64>,
}
