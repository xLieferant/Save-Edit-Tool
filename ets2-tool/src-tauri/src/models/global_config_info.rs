use serde::Serialize;

#[derive(Serialize)]
pub struct BaseGameConfig {
    pub max_convoy_size: Option<i64>,
    pub traffic: Option<i64>,
    pub developer: Option<i64>,
    pub console: Option<i64>,
}