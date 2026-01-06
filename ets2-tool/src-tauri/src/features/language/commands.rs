use serde::{Deserialize, Serialize};
use super::translator::{set_language, get_current_language, get_available_languages, t};

#[derive(Serialize, Deserialize)]
pub struct LanguageInfo {
    pub code: String,
    pub name: String,
}

#[tauri::command]
pub fn get_available_languages_command() -> Vec<LanguageInfo> {
    let languages = get_available_languages();
    
    languages.into_iter().map(|code| {
        let name = match code.as_str() {
            "en" => "English".to_string(),
            "de" => "Deutsch".to_string(),
            "es" => "Español".to_string(),
            "fr" => "Français".to_string(),
            "it" => "Italiano".to_string(),
            _ => code.clone(), // Clone for the fallback case
        };
        
        LanguageInfo {
            code,
            name,
        }
    }).collect()
}

#[tauri::command]
pub fn get_current_language_command() -> String {
    get_current_language()
}

#[tauri::command]
pub fn set_language_command(language: String) -> Result<String, String> {
    set_language(&language)?;
    
    // Return success message in the newly set language
    Ok(t("toasts.language_updated"))
}

#[tauri::command]
pub fn translate_command(key: String) -> String {
    t(&key)
}