use serde::Serialize;

#[derive(Serialize, Clone)]
pub struct SaveGameData {
    pub money: Option<i64>,
    pub xp: Option<i64>,
    pub recruitments: Option<i64>,
    pub dealers: Option<i64>,
    pub visited_cities: Option<i64>,
}
