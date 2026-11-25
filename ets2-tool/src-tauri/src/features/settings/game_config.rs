// Make sure these imports match your project structure
use crate::dev_log;
use crate::models::global_config_info::BaseGameConfig;
use crate::models::save_game_config::SaveGameConfig;
use crate::shared::decrypt::decrypt_if_needed;
use crate::shared::paths::quicksave_config_path;
use crate::shared::trace::TraceScope;
use crate::state::{AppProfileState, ProfileCache};
use regex::Regex;
use std::fs;
use tauri::{State, command};

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveFormatStatus {
    status: &'static str,
    revision: Option<String>,
}

fn save_format_revision(path: &std::path::Path, raw: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hash = Sha256::new();
    hash.update(path.as_os_str().as_encoded_bytes());
    hash.update([0]);
    hash.update(raw);
    format!("{:x}", hash.finalize())
}

// Match only active, exact keys; preserve BOM, whitespace, comments and line endings.
fn save_format_value(content: &str) -> Result<Option<std::ops::Range<usize>>, String> {
    let keys = Regex::new(r"(?m)^(?:\x{FEFF})?[ \t]*uset[ \t]+g_save_format(?:[ \t][^\r\n]*)?\r?$")
        .map_err(|_| "save_format.invalid")?;
    let mut entries = keys.find_iter(content);
    let Some(entry) = entries.next() else {
        return Ok(None);
    };
    if entries.next().is_some() {
        return Err("save_format.invalid".into());
    }
    let value = Regex::new(r#"^(?:\x{FEFF})?[ \t]*uset[ \t]+g_save_format[ \t]+"([^"\r\n]*)"[ \t]*(?:(?:#|//)[^\r\n]*)?\r?$"#)
        .map_err(|_| "save_format.invalid")?;
    let captured = value
        .captures(entry.as_str())
        .ok_or("save_format.invalid")?;
    let span = captured.get(1).ok_or("save_format.invalid")?;
    Ok(Some(
        entry.start() + span.start()..entry.start() + span.end(),
    ))
}

fn read_save_format(path: &std::path::Path) -> Result<SaveFormatStatus, String> {
    let raw = match fs::read(path) {
        Ok(raw) => raw,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(SaveFormatStatus {
                status: "missing_file",
                revision: None,
            });
        }
        Err(_) => return Err("save_format.unavailable".into()),
    };
    let content = std::str::from_utf8(&raw).map_err(|_| "save_format.invalid")?;
    let status = match save_format_value(content)? {
        Some(span) if &content[span.clone()] == "2" => "correct",
        Some(_) => "incorrect",
        None => "missing_entry",
    };
    Ok(SaveFormatStatus {
        status,
        revision: Some(save_format_revision(path, &raw)),
    })
}

fn write_save_format(
    path: &std::path::Path,
    revision: &str,
    confirmed: bool,
) -> Result<SaveFormatStatus, String> {
    use std::io::Write;
    if !confirmed {
        return Err("save_format.confirmation_required".into());
    }
    let raw = fs::read(path).map_err(|_| "save_format.unavailable")?;
    if save_format_revision(path, &raw) != revision {
        return Err("save_format.changed".into());
    }
    let content = std::str::from_utf8(&raw).map_err(|_| "save_format.invalid")?;
    let mut updated = content.to_string();
    match save_format_value(content)? {
        Some(span) if &content[span.clone()] == "2" => return read_save_format(path),
        Some(span) => updated.replace_range(span, "2"),
        None => {
            let newline = if content.contains("\r\n") {
                "\r\n"
            } else {
                "\n"
            };
            if !content.is_empty() && !content.ends_with('\n') {
                updated.push_str(newline);
            }
            updated.push_str("uset g_save_format \"2\"");
            updated.push_str(newline);
        }
    }

    // A unique sibling backup must be durable and byte-identical before replacement.
    let backup = path.with_file_name(format!(
        "config.cfg.save-format-{}.bak",
        uuid::Uuid::new_v4()
    ));
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&backup)
        .map_err(|_| "save_format.backup_failed")?;
    file.write_all(&raw)
        .map_err(|_| "save_format.backup_failed")?;
    file.sync_all().map_err(|_| "save_format.backup_failed")?;
    drop(file);
    if fs::read(&backup).map_err(|_| "save_format.backup_failed")? != raw {
        return Err("save_format.backup_failed".into());
    }
    crate::features::save_editor::safety::write_atomic_checked(path, &raw, &updated, |_| Ok(()))
        .map_err(|error| {
            if error == "save_changed_since_load" {
                "save_format.changed".to_string()
            } else {
                "save_format.write_failed".to_string()
            }
        })?;
    read_save_format(path)
}

#[cfg_attr(not(test), tauri::command(async))]
#[cfg_attr(test, tauri::command)]
pub fn check_save_format(
    game: String,
    profile_state: State<'_, AppProfileState>,
) -> Result<SaveFormatStatus, String> {
    let selected = profile_state
        .selected_game
        .lock()
        .map_err(|_| "save_format.unavailable")?;
    if *selected != game {
        return Err("save_format.changed".into());
    }
    let paths =
        crate::shared::paths::resolve_game_user_paths(&game).ok_or("save_format.unavailable")?;
    read_save_format(&paths.config_file)
}

#[cfg_attr(not(test), tauri::command(async))]
#[cfg_attr(test, tauri::command)]
pub fn configure_save_format(
    game: String,
    revision: String,
    confirmed: bool,
    profile_state: State<'_, AppProfileState>,
    cache: State<'_, ProfileCache>,
) -> Result<SaveFormatStatus, String> {
    let _mutation = crate::features::save_editor::safety::mutation_lock()
        .map_err(|_| "save_format.unavailable")?;
    let selected = profile_state
        .selected_game
        .lock()
        .map_err(|_| "save_format.unavailable")?;
    if *selected != game {
        return Err("save_format.changed".into());
    }
    let paths =
        crate::shared::paths::resolve_game_user_paths(&game).ok_or("save_format.unavailable")?;
    let result = write_save_format(&paths.config_file, &revision, confirmed)?;
    cache.invalidate_base_config();
    Ok(result)
}

pub(crate) fn patch_config_value(content: &str, key: &str, value: &str) -> Result<String, String> {
    if key.is_empty()
        || !key.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
        || value.contains(['"', '\r', '\n', '\0', '\\'])
    {
        return Err("config_value_invalid".into());
    }
    let pattern = Regex::new(&format!(
        r#"(?m)^[ \t]*uset[ \t]+{}[ \t]+"([^"\r\n]*)""#,
        regex::escape(key)
    ))
    .map_err(|_| "config_key_invalid")?;
    let mut matches = pattern.captures_iter(content);
    let capture = matches.next().ok_or("config_key_missing")?;
    if matches.next().is_some() {
        return Err("config_key_ambiguous".into());
    }
    let span = capture.get(1).ok_or("config_value_invalid")?;
    let mut updated = content.to_string();
    updated.replace_range(span.range(), value);
    Ok(updated)
}

#[cfg(test)]
mod tests {
    fn config_fixture(content: &[u8]) -> std::path::PathBuf {
        let directory =
            std::env::temp_dir().join(format!("save-format-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("config.cfg");
        std::fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn save_format_preserves_every_other_byte_and_creates_exact_backup() {
        let _test = crate::features::save_editor::safety::TEST_COMMAND_LOCK
            .lock()
            .unwrap();
        let _mutation = crate::features::save_editor::safety::mutation_lock().unwrap();
        for value in ["0", "1", "3", "99", "unexpected", ""] {
            let original = format!(
                "\u{feff}# config\r\n# uset g_save_format \"9\"\r\n\tuset\tg_save_format\t\"{value}\" # keep\r\nuset g_save_format_extra \"0\"\r\nuset g_traffic \"1\""
            );
            let path = config_fixture(original.as_bytes());
            let before = super::read_save_format(&path).unwrap();
            assert_eq!(before.status, "incorrect");
            assert_eq!(
                super::write_save_format(&path, &before.revision.unwrap(), true)
                    .unwrap()
                    .status,
                "correct"
            );
            assert_eq!(
                std::fs::read_to_string(&path).unwrap(),
                original.replacen(&format!("\t\"{value}\""), "\t\"2\"", 1)
            );
            let backups: Vec<_> = std::fs::read_dir(path.parent().unwrap())
                .unwrap()
                .map(|entry| entry.unwrap().path())
                .filter(|file| file.extension().is_some_and(|ext| ext == "bak"))
                .collect();
            assert_eq!(backups.len(), 1);
            assert_eq!(std::fs::read(&backups[0]).unwrap(), original.as_bytes());
        }
    }

    #[test]
    fn save_format_requires_consent_and_rejects_changed_content_or_location() {
        let original = b"uset g_save_format \"0\"\n";
        let path = config_fixture(original);
        let revision = super::read_save_format(&path).unwrap().revision.unwrap();
        assert_eq!(
            super::write_save_format(&path, &revision, false)
                .err()
                .unwrap(),
            "save_format.confirmation_required"
        );
        assert_eq!(std::fs::read(&path).unwrap(), original);
        assert_eq!(
            std::fs::read_dir(path.parent().unwrap()).unwrap().count(),
            1
        );
        let other_path = config_fixture(original);
        assert_eq!(
            super::write_save_format(&other_path, &revision, true)
                .err()
                .unwrap(),
            "save_format.changed"
        );
        let external_change = b"uset g_save_format \"0\"\nuset g_traffic \"3\"\n";
        std::fs::write(&path, external_change).unwrap();
        assert_eq!(
            super::write_save_format(&path, &revision, true)
                .err()
                .unwrap(),
            "save_format.changed"
        );
        assert_eq!(std::fs::read(&path).unwrap(), external_change);
        assert_eq!(
            std::fs::read_dir(path.parent().unwrap()).unwrap().count(),
            1
        );
    }

    #[test]
    fn save_format_adds_missing_entry_only_after_confirmation() {
        let _test = crate::features::save_editor::safety::TEST_COMMAND_LOCK
            .lock()
            .unwrap();
        let _mutation = crate::features::save_editor::safety::mutation_lock().unwrap();
        for original in [
            "",
            "uset g_traffic \"1\"",
            "# config\r\nuset g_traffic \"1\"\r\n",
        ] {
            let path = config_fixture(original.as_bytes());
            let before = super::read_save_format(&path).unwrap();
            assert_eq!(before.status, "missing_entry");
            let revision = before.revision.unwrap();
            assert!(super::write_save_format(&path, &revision, false).is_err());
            assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
            assert_eq!(
                super::write_save_format(&path, &revision, true)
                    .unwrap()
                    .status,
                "correct"
            );
            let updated = std::fs::read_to_string(&path).unwrap();
            let newline = if original.contains("\r\n") {
                "\r\n"
            } else {
                "\n"
            };
            let separator = if !original.is_empty() && !original.ends_with('\n') {
                newline
            } else {
                ""
            };
            assert_eq!(
                updated,
                format!("{original}{separator}uset g_save_format \"2\"{newline}")
            );
        }
    }

    #[test]
    fn save_format_handles_missing_correct_and_ambiguous_configs_without_writes() {
        let path = config_fixture(b"\xef\xbb\xbfuset g_save_format \"2\"\r\n");
        let before = super::read_save_format(&path).unwrap();
        assert_eq!(before.status, "correct");
        assert_eq!(
            super::write_save_format(&path, &before.revision.unwrap(), true)
                .unwrap()
                .status,
            "correct"
        );
        assert_eq!(
            std::fs::read_dir(path.parent().unwrap()).unwrap().count(),
            1
        );
        let missing = path.with_file_name("missing.cfg");
        assert_eq!(
            super::read_save_format(&missing).unwrap().status,
            "missing_file"
        );
        assert!(super::write_save_format(&missing, "", true).is_err());
        assert!(!missing.exists());
        for content in [
            "uset g_save_format \"0\"\nuset g_save_format \"2\"",
            "uset g_save_format",
            "uset g_save_format 0",
        ] {
            assert_eq!(
                super::save_format_value(content).err().unwrap(),
                "save_format.invalid"
            );
            let path = config_fixture(content.as_bytes());
            let revision = super::save_format_revision(&path, content.as_bytes());
            assert!(super::write_save_format(&path, &revision, true).is_err());
            assert_eq!(std::fs::read_to_string(&path).unwrap(), content);
            assert_eq!(
                std::fs::read_dir(path.parent().unwrap()).unwrap().count(),
                1
            );
        }
    }

    #[test]
    fn ats_config_patch_preserves_comments_similar_keys_and_quotes() {
        let original = "# uset g_traffic \"9\"\r\nuset g_traffic \"1\" # retained\r\nuset g_traffic_extra \"2\"\r\n";
        let updated = super::patch_config_value(original, "g_traffic", "3").unwrap();
        assert_eq!(
            updated,
            original.replacen("uset g_traffic \"1\"", "uset g_traffic \"3\"", 1)
        );
        assert!(super::patch_config_value(original, "g_traffic.*", "3").is_err());
        assert!(super::patch_config_value(original, "g_traffic", "3\"\nuset other \"1").is_err());
    }
}

//* Liest die globale config.cfg im Basis-Verzeichnis des Spiels *//
#[command]
pub fn read_base_config(
    cache: State<'_, ProfileCache>,
    profile_state: State<'_, AppProfileState>,
) -> Result<BaseGameConfig, String> {
    let mut trace = TraceScope::new("read_base_config");
    dev_log!("Lese globale Config");

    if let Some(cached) = cache.get_base_config() {
        dev_log!("Base config aus Cache geliefert");
        trace.finish_ok();
        return Ok(cached);
    }

    // Pfad zur globalen config.cfg ermitteln
    let path = match crate::features::save_editor::safety::config_path(profile_state.inner()) {
        Ok(p) => p,
        Err(_) => {
            let err_msg = "Konnte Standardpfad zur globalen config.cfg nicht ermitteln.";
            dev_log!("{}", err_msg);
            trace.finish_error(err_msg);
            return Err(err_msg.into());
        }
    };

    // Prüfe, ob die Datei existiert
    if !path.exists() {
        let err_msg = format!("Die Datei wurde nicht gefunden unter: {:?}", path);
        dev_log!("{}", err_msg);
        trace.finish_error(&err_msg);
        return Err(err_msg);
    }

    // config.cfg wird **nicht entschlüsselt**, nur gelesen
    let content =
        fs::read_to_string(&path).map_err(|e| format!("Fehler beim Lesen der Datei: {}", e))?;

    // Sicherstellen, dass Regex korrekt funktioniert
    let re = |pat: &str| Regex::new(pat).unwrap();
    let data = BaseGameConfig {
        max_convoy_size: re(r#"uset g_max_convoy_size\s*"?(\d+)"??"#)
            .captures(&content)
            .and_then(|c| c[1].parse().ok()),
        traffic: re(r#"uset g_traffic\s*"?(\d+)"??"#)
            .captures(&content)
            .and_then(|c| c[1].parse().ok()),
        developer: re(r#"uset g_developer\s*"?(\d+)"??"#)
            .captures(&content)
            .and_then(|c| c[1].parse().ok()),
        console: re(r#"uset g_console\s*"?(\d+)"??"#)
            .captures(&content)
            .and_then(|c| c[1].parse().ok()),
    };

    dev_log!(
        "Gefundene Daten: max_convoy_size {:?}, traffic {:?}, developer {:?}, console {:?}",
        data.max_convoy_size,
        data.traffic,
        data.developer,
        data.console,
    );

    cache.cache_base_config(data.clone());
    trace.finish_ok();
    Ok(data)
}

// Hiermit wird die config.cfg im Basis-Ordner des Profils gelesen
// (z.B. .../Euro Truck Simulator 2/profiles/12345/config.cfg)
#[command]
// Der Funktionskopf akzeptiert den Pfad direkt
pub fn read_save_config(
    profile_path: &str,
    cache: State<'_, ProfileCache>,
) -> Result<SaveGameConfig, String> {
    let mut trace =
        TraceScope::with_fields("read_save_config", &[("path", profile_path.to_string())]);
    // Wir nehmen den übergebenen profile_path direkt.

    dev_log!("Lese Config aus Profilpfad: {}", profile_path);

    if let Some(cached) = cache.get_save_config() {
        dev_log!("Save config aus Cache geliefert");
        trace.finish_ok();
        return Ok(cached);
    }

    // Verwende die korrekte Hilfsfunktion, um den vollen Pfad zur config.cfg innerhalb des Profilordners zu erhalten
    let path = quicksave_config_path(profile_path);

    // Prüfe, ob die Datei existiert, bevor du versuchst, sie zu entschlüsseln/lesen
    if !path.exists() {
        let err_msg = format!("Error: Die Datei wurde nicht gefunden unter: {:?}", path);
        dev_log!("{}", err_msg);
        trace.finish_error(&err_msg);
        return Err(err_msg);
    }

    let content = decrypt_if_needed(&path)?;

    // Sicherstellen, dass Regex korrekt funktioniert
    let re = |pat: &str| Regex::new(pat).unwrap();
    let data = SaveGameConfig {
        // SCS profile setting g_mph: 0 = kilometers, 1 = miles.
        use_miles: re(r#"(?m)^\s*uset g_mph\s+"?([01])"?"#)
            .captures(&content)
            .map(|captures| &captures[1] == "1"),
        // Der Regex-Pattern sucht nach dem genauen String in der config.cfg
        // c[1] ist korrekt für die erste Capture-Gruppe (die Ziffern).
        factor_parking_doubles: re(r#"uset g_simple_parking_doubles\s*"?(\d+)"??"#)
            .captures(&content)
            .and_then(|c| c[1].parse().ok()),
    };
    dev_log!(
        "Gefundene Daten: uset g_simple_parking_doubles {:?}",
        data.factor_parking_doubles,
    );

    cache.cache_save_config(data.clone());
    trace.finish_ok();
    Ok(data)
}
