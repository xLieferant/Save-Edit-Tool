use serde_json::Value;
use crate::utils::paths;


pub struct SettingMap {
pub path: fn(&str) -> String,
pub regex: &'static str,
pub replace: &'static str,
pub format: fn(&Value) -> Result<String, String>,
}


pub fn get(key: &str) -> Option<SettingMap> {
match key {
"money" => Some(SettingMap {
path: paths::autosave_path,
regex: r"info_money_account:\s*\\d+",
replace: "info_money_account:",
format: |v| Ok(v.as_i64().unwrap().to_string()),
}),


"xp" => Some(SettingMap {
path: paths::autosave_path,
regex: r"info_players_experience:\s*\\d+",
replace: "info_players_experience:",
format: |v| Ok(v.as_i64().unwrap().to_string()),
}),


"developer" => Some(SettingMap {
path: paths::base_config_path,
regex: r"uset g_developer \"\\d\"",
replace: "uset g_developer",
format: |v| Ok(format!("\"{}\"", v.as_i64().unwrap())),
}),


_ => None,
}
}