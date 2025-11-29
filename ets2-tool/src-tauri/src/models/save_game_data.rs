use serde::Serialize;

#[derive(Serialize)]
pub struct SaveGameData {
    pub money: Option<i64>,
    pub xp: Option<i64>,
    pub level: Option<i64>,
    pub garages: Option<i64>,
    pub trucks_owned: Option<i64>,
    pub trailers_owned: Option<i64>,
    pub kilometers_total: Option<i64>,
}
