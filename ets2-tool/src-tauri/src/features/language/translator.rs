use dirs;
use once_cell::sync::Lazy;
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::RwLock;

static TRANSLATOR: Lazy<RwLock<Translator>> = Lazy::new(|| RwLock::new(Translator::new()));

pub struct Translator {
    current_language: String,
    translations: HashMap<String, Value>,
}

impl Translator {
    pub fn new() -> Self {
        let mut translator = Self {
            current_language: "en".to_string(),
            translations: HashMap::new(),
        };

        // Load all available translations
        translator.load_all_translations();

        // Load saved language preference or use default
        if let Ok(saved_lang) = load_language_preference() {
            if translator.translations.contains_key(&saved_lang) {
                translator.current_language = saved_lang;
            }
        }

        translator
    }

    fn load_all_translations(&mut self) {
        let locales_dir = get_locales_dir();

        // List of supported languages
        let languages = vec!["en", "de"];

        for lang in languages {
            let file_path = locales_dir.join(format!("{}.json", lang));
            if let Ok(content) = fs::read_to_string(&file_path) {
                if let Ok(json) = serde_json::from_str::<Value>(&content) {
                    self.translations.insert(lang.to_string(), json);
                }
            }
        }
    }

    pub fn set_language(&mut self, lang: &str) -> Result<(), String> {
        if !self.translations.contains_key(lang) {
            return Err(format!("Language '{}' not available", lang));
        }

        self.current_language = lang.to_string();
        save_language_preference(lang)?;
        Ok(())
    }

    pub fn get_current_language(&self) -> String {
        self.current_language.clone()
    }

    pub fn get_available_languages(&self) -> Vec<String> {
        self.translations.keys().cloned().collect()
    }

    pub fn translate(&self, key: &str) -> String {
        let lang_data = match self.translations.get(&self.current_language) {
            Some(data) => data,
            None => return key.to_string(), // Fallback to key if language not found
        };

        // Split the key by dots to navigate nested structure
        let parts: Vec<&str> = key.split('.').collect();
        let mut current = lang_data;

        for part in parts {
            match current.get(part) {
                Some(value) => current = value,
                None => return key.to_string(), // Fallback to key if not found
            }
        }

        // Return the string value
        match current.as_str() {
            Some(s) => s.to_string(),
            None => key.to_string(),
        }
    }
}

// Global translation function
pub fn t(key: &str) -> String {
    TRANSLATOR.read().unwrap().translate(key)
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

fn get_locales_dir() -> PathBuf {
    // Get the resource directory where locales are stored
    let exe_dir = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();

    // In development, locales are in src-tauri/locales
    // In production, they should be bundled with the app
    let dev_path = exe_dir.join("../../locales");
    if dev_path.exists() {
        return dev_path;
    }

    // Production path (adjust based on your Tauri config)
    exe_dir.join("locales")
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
