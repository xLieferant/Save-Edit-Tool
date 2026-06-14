use super::translator::{
    ensure_translator_initialized, get_available_languages, get_current_language, set_language, t,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tauri::{AppHandle, Runtime};

#[derive(Serialize, Deserialize)]
pub struct LanguageInfo {
    pub code: String,
    pub name: String,
}

fn language_name_registry() -> HashMap<&'static str, &'static str> {
    HashMap::from([
        ("de", "Deutsch"),
        ("en", "English"),
        ("es", "Español"),
        ("fr", "Français"),
        ("it", "Italiano"),
        ("pl", "Polski"),
    ])
}

#[tauri::command]
pub fn get_available_languages_command<R: Runtime>(app: AppHandle<R>) -> Vec<LanguageInfo> {
    if let Err(error) = ensure_translator_initialized(&app) {
        crate::dev_log!(
            "[language] get_available_languages_command init failed: {}",
            error
        );
    }

    let languages = get_available_languages();
    let registry = language_name_registry();

    let mut language_infos: Vec<LanguageInfo> = languages
        .into_iter()
        .map(|code| {
            let name = registry
                .get(code.as_str())
                .copied()
                .unwrap_or(code.as_str())
                .to_string();

            LanguageInfo { code, name }
        })
        .collect();

    language_infos.sort_by(|left, right| left.code.cmp(&right.code));
    language_infos
}

#[tauri::command]
pub fn get_current_language_command<R: Runtime>(app: AppHandle<R>) -> String {
    if let Err(error) = ensure_translator_initialized(&app) {
        crate::dev_log!(
            "[language] get_current_language_command init failed: {}",
            error
        );
    }

    get_current_language()
}

#[tauri::command]
pub fn set_language_command<R: Runtime>(
    app: AppHandle<R>,
    language: String,
) -> Result<String, String> {
    ensure_translator_initialized(&app)?;
    set_language(&language)?;

    Ok(t("toasts.language_updated"))
}

#[tauri::command]
pub fn translate_command<R: Runtime>(app: AppHandle<R>, key: String) -> String {
    if let Err(error) = ensure_translator_initialized(&app) {
        crate::dev_log!("[language] translate_command init failed: {}", error);
    }

    t(&key)
}
