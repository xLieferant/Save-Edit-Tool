use serde::Serialize;

#[derive(Serialize, Clone)]
pub struct SaveGameConfig {
    pub factor_parking_doubles: Option<i64>,
    pub use_miles: Option<bool>,
}
