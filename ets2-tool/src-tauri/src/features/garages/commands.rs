use super::models::GarageActionResult;
use super::service;

#[tauri::command]
pub fn buy_garage() -> Result<GarageActionResult, String> {
    Ok(service::buy_garage())
}

#[tauri::command]
pub fn upgrade_garage() -> Result<GarageActionResult, String> {
    Ok(service::upgrade_garage())
}

#[tauri::command]
pub fn buy_all_garages() -> Result<GarageActionResult, String> {
    Ok(service::buy_all_garages())
}

#[tauri::command]
pub fn relinquish_garage_ownership() -> Result<GarageActionResult, String> {
    Ok(service::relinquish_garage_ownership())
}
