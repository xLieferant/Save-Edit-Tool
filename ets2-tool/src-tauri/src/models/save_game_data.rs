use serde::Serialize;

#[derive(Serialize)]
pub struct SaveGameData {
    pub money: i64,
    pub xp: i64,
    pub level: i64,
}
