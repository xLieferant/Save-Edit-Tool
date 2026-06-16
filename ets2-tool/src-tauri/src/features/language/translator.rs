use dirs;
use once_cell::sync::Lazy;
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::RwLock;
use tauri::path::BaseDirectory;
use tauri::{AppHandle, Manager, Runtime};

static TRANSLATOR: Lazy<RwLock<Translator>> = Lazy::new(|| RwLock::new(Translator::new()));

pub struct Translator {
    current_language: String,
    translations: HashMap<String, Value>,
    initialized: bool,
}

impl Translator {
    pub fn new() -> Self {
        Self {
            current_language: "en".to_string(),
            translations: HashMap::new(),
            initialized: false,
        }
    }

    pub fn initialize<R: Runtime>(&mut self, app: &AppHandle<R>) -> Result<(), String> {
        log_language("initializing translator with AppHandle");
        let locales_dir = get_locales_dir(app)?;
        self.current_language = "en".to_string();
        self.translations.clear();
        self.load_all_translations(app, &locales_dir);

        if let Ok(saved_lang) = load_language_preference() {
            if self.translations.contains_key(&saved_lang) {
                self.current_language = saved_lang;
            } else {
                log_language(format!(
                    "saved locale '{}' not available, falling back to en",
                    saved_lang
                ));
            }
        }

        log_language(format!(
            "initialization complete, current locale: {}",
            self.current_language
        ));

        self.initialized = true;

        if self.translations.len() <= 1 {
            let languages = self.available_language_codes();
            log_language(format!(
                "WARNING: only {} language(s) loaded: {:?}",
                self.translations.len(),
                languages
            ));
        }

        Ok(())
    }

    fn load_all_translations<R: Runtime>(&mut self, app: &AppHandle<R>, locales_dir: &PathBuf) {
        let resource_dir = app.path().resource_dir().ok();

        log_language(format!(
            "using locales directory: {}",
            locales_dir.display()
        ));
        if let Some(resource_dir) = resource_dir {
            log_language(format!(
                "tauri resource directory: {}",
                resource_dir.display()
            ));
        }

        let entries = match fs::read_dir(locales_dir) {
            Ok(entries) => entries,
            Err(error) => {
                log_language(format!(
                    "failed to read locales directory {}: {}",
                    locales_dir.display(),
                    error
                ));
                return;
            }
        };

        let mut discovered_files = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            let Some(extension) = path.extension().and_then(|value| value.to_str()) else {
                continue;
            };

            if !extension.eq_ignore_ascii_case("json") {
                continue;
            }

            let Some(code) = path.file_stem().and_then(|value| value.to_str()) else {
                log_language(format!(
                    "skipped locale file with invalid name: {}",
                    path.display()
                ));
                continue;
            };

            discovered_files.push(path.display().to_string());
            match fs::read_to_string(&path) {
                Ok(content) => {
                    let content = content.trim_start_matches('\u{feff}');
                    match serde_json::from_str::<Value>(content) {
                    Ok(json) => {
                        self.translations.insert(code.to_string(), json);
                        log_language(format!("loaded locale: {} ({})", code, path.display()));
                    }
                    Err(error) => {
                        log_language(format!(
                            "failed to parse locale file {}: {}",
                            path.display(),
                            error
                        ));
                    }
                }
                },
                Err(error) => {
                    log_language(format!(
                        "failed to read locale file {}: {}",
                        path.display(),
                        error
                    ));
                }
            }
        }

        discovered_files.sort();
        log_language(format!(
            "locale json files found: {}",
            if discovered_files.is_empty() {
                "<none>".to_string()
            } else {
                discovered_files.join(", ")
            }
        ));

        if !self.translations.contains_key("en") {
            log_language("warning: master locale 'en' was not loaded");
        }

        let detected_languages = self.available_language_codes();
        log_language(format!(
            "detected locales: {}",
            detected_languages.join(", ")
        ));
    }

    pub fn ensure_initialized<R: Runtime>(&mut self, app: &AppHandle<R>) -> Result<(), String> {
        if self.initialized {
            return Ok(());
        }

        log_language(format!(
            "translator not ready before command; initialized={}, loaded={}",
            self.initialized,
            self.translations.len()
        ));
        self.initialize(app)
    }

    fn available_language_codes(&self) -> Vec<String> {
        let mut languages: Vec<String> = self.translations.keys().cloned().collect();
        languages.sort();
        languages
    }

    fn resolve_translation<'a>(&'a self, language: &str, key: &str) -> Option<&'a str> {
        let lang_data = self.translations.get(language)?;
        let parts: Vec<&str> = key.split('.').collect();
        let mut current = lang_data;

        for part in parts {
            current = current.get(part)?;
        }

        current.as_str()
    }

    pub fn set_language(&mut self, lang: &str) -> Result<(), String> {
        if !self.translations.contains_key(lang) {
            return Err(format!("Language '{}' not available", lang));
        }

        self.current_language = lang.to_string();
        save_language_preference(lang)?;
        log_language(format!("active locale set to: {}", lang));
        Ok(())
    }

    pub fn get_current_language(&self) -> String {
        self.current_language.clone()
    }

    pub fn get_available_languages(&self) -> Vec<String> {
        let languages = self.available_language_codes();
        log_language(format!(
            "get_available_languages -> {}",
            if languages.is_empty() {
                "<none>".to_string()
            } else {
                languages.join(", ")
            }
        ));
        languages
    }

    pub fn translate(&self, key: &str) -> String {
        if let Some(value) = self.resolve_translation(&self.current_language, key) {
            return value.to_string();
        }

        if self.current_language != "en" {
            if let Some(value) = self.resolve_translation("en", key) {
                log_language(format!(
                    "fallback to en for key '{}' from locale '{}'",
                    key, self.current_language
                ));
                return value.to_string();
            }
        }

        if !self.translations.contains_key("en") {
            log_language(format!(
                "ERROR: fallback locale 'en' is not loaded; returning key '{}'",
                key
            ));
        }

        key.to_string()
    }
}

// Global translation function
pub fn t(key: &str) -> String {
    TRANSLATOR.read().unwrap().translate(key)
}

pub fn initialize_translator<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    TRANSLATOR.write().unwrap().initialize(app)
}

pub fn ensure_translator_initialized<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    TRANSLATOR.write().unwrap().ensure_initialized(app)
}

// Set language globally
pub fn set_language(lang: &str) -> Result<(), String> {
    TRANSLATOR.write().unwrap().set_language(lang)
}

// Get current language
pub fn get_current_language() -> String {
    TRANSLATOR.read().unwrap().get_current_language()
}

// Get available languages
pub fn get_available_languages() -> Vec<String> {
    TRANSLATOR.read().unwrap().get_available_languages()
}

fn get_locales_dir<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf, String> {
    let cargo_manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let dev_root_locales = cargo_manifest_dir.join("../locales");
    let dev_src_tauri_locales = cargo_manifest_dir.join("locales");

    log_language(format!(
        "CARGO_MANIFEST_DIR: {}",
        cargo_manifest_dir.display()
    ));

    let resolved_resource_path = app
        .path()
        .resolve("locales", BaseDirectory::Resource)
        .map_err(|error| format!("Failed to resolve locales via resource path: {}", error))?;
    log_language(format!(
        "resource resolver returned locales path: {}",
        resolved_resource_path.display()
    ));
    if resolved_resource_path.is_dir() {
        return Ok(resolved_resource_path);
    }

    log_language(format!(
        "resolved resource path is not a directory: {}",
        resolved_resource_path.display()
    ));

    if let Ok(resource_dir) = app.path().resource_dir() {
        let direct_resource_path = resource_dir.join("locales");
        log_language(format!(
            "checking direct resource_dir locales path: {}",
            direct_resource_path.display()
        ));
        if direct_resource_path.is_dir() {
            return Ok(direct_resource_path);
        }
    }

    log_language(format!(
        "checking dev root locales: {}",
        dev_root_locales.display()
    ));
    if dev_root_locales.is_dir() {
        return Ok(dev_root_locales);
    }

    log_language(format!(
        "checking src-tauri locales: {}",
        dev_src_tauri_locales.display()
    ));
    if dev_src_tauri_locales.is_dir() {
        return Ok(dev_src_tauri_locales);
    }

    Err(format!(
        "No locales directory found. Checked resource path '{}', dev root '{}', and src-tauri '{}'",
        resolved_resource_path.display(),
        dev_root_locales.display(),
        dev_src_tauri_locales.display()
    ))
}

fn log_language(message: impl AsRef<str>) {
    let line = format!("[language] {}", message.as_ref());
    println!("{}", line);
    crate::dev_log!("{}", line);
}

fn get_language_config_path() -> PathBuf {
    let config_dir = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("ets2-tool");

    // Create config directory if it doesn't exist
    let _ = fs::create_dir_all(&config_dir);

    config_dir.join("language_config.json")
}

fn save_language_preference(lang: &str) -> Result<(), String> {
    let config_path = get_language_config_path();
    let config = serde_json::json!({
        "language": lang
    });

    fs::write(&config_path, serde_json::to_string_pretty(&config).unwrap())
        .map_err(|e| format!("Failed to save language preference: {}", e))
}

fn load_language_preference() -> Result<String, String> {
    let config_path = get_language_config_path();

    if !config_path.exists() {
        return Ok("en".to_string()); // Default to English
    }

    let content = fs::read_to_string(&config_path)
        .map_err(|e| format!("Failed to read language config: {}", e))?;

    let config: Value = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse language config: {}", e))?;

    config["language"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| "Language field not found in config".to_string())
}
