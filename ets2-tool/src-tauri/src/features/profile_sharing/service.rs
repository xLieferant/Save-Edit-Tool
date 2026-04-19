use super::models::{
    ProfileShareContext, ProfileShareExportResult, ProfileShareImportPreview,
    ProfileShareImportResult, SharedProfileManifest,
};
use crate::dev_log;
use crate::shared::decrypt::decrypt_if_needed;
use crate::shared::extract::extract_profile_name;
use crate::shared::hex_float::{decode_hex_folder_name, text_to_hex};
use crate::shared::paths::get_base_path;
use crate::state::AppProfileState;
use chrono::Local;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use tauri::AppHandle;
use tauri_plugin_dialog::DialogExt;
use uuid::Uuid;
use walkdir::WalkDir;
use zip::write::FileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

const ARCHIVE_FORMAT: &str = "ets2-tool.profile-share";
const ARCHIVE_VERSION: u32 = 1;
const ARCHIVE_ROOT: &str = "profile";
const MANIFEST_NAME: &str = "profile_share_manifest.json";

pub fn get_profile_share_context(
    profile_path: Option<&str>,
    profile_state: &AppProfileState,
) -> Result<ProfileShareContext, String> {
    let selected_game = selected_game(profile_state)?;
    let default_export_dir = resolve_export_dir(&selected_game).ok();
    let import_target_dir = resolve_import_root(&selected_game).ok();
    let path_resolution_error = if import_target_dir.is_some() {
        None
    } else {
        Some(
            "Der ETS2-/ATS-Profilpfad konnte nicht aufgeloest werden. Bitte pruefe den Spielordner unter Dokumente."
                .to_string(),
        )
    };
    let profile_details = match profile_path.filter(|value| !value.trim().is_empty()) {
        Some(profile_path) => {
            let profile_dir = require_profile_dir(profile_path)?;
            let profile_name = resolve_profile_name(&profile_dir)?;
            let archive_name = build_archive_name(&profile_name);
            Some((
                profile_name,
                profile_dir.display().to_string(),
                archive_name,
            ))
        }
        None => None,
    };

    Ok(ProfileShareContext {
        selected_game,
        profile_name: profile_details.as_ref().map(|value| value.0.clone()),
        profile_path: profile_details.as_ref().map(|value| value.1.clone()),
        default_export_dir: default_export_dir
            .as_ref()
            .map(|value| value.display().to_string()),
        default_archive_name: profile_details.as_ref().map(|value| value.2.clone()),
        import_target_dir: import_target_dir
            .as_ref()
            .map(|value| value.display().to_string()),
        can_export: default_export_dir.is_some(),
        can_import: import_target_dir.is_some(),
        path_resolution_error,
    })
}

pub fn pick_shared_profile_import_archive(
    app: &AppHandle,
    profile_state: &AppProfileState,
) -> Result<Option<String>, String> {
    let selected_game = selected_game(profile_state)?;
    let mut dialog = app
        .dialog()
        .file()
        .add_filter("ZIP Archive", &["zip"])
        .set_title("ZIP archive");

    if let Some(start_dir) = resolve_import_archive_start_dir(&selected_game) {
        dialog = dialog.set_directory(start_dir);
    }

    dialog
        .blocking_pick_file()
        .map(file_path_to_string)
        .transpose()
}

pub fn pick_shared_profile_export_directory(
    app: &AppHandle,
    profile_state: &AppProfileState,
) -> Result<Option<String>, String> {
    let selected_game = selected_game(profile_state)?;
    let mut dialog = app.dialog().file().set_title("Export folder");

    if let Ok(default_export_dir) = resolve_export_dir(&selected_game) {
        dialog = dialog.set_directory(default_export_dir);
    }

    dialog
        .blocking_pick_folder()
        .map(file_path_to_string)
        .transpose()
}

pub fn export_shared_profile(
    profile_path: &str,
    export_dir_override: Option<String>,
    profile_state: &AppProfileState,
) -> Result<ProfileShareExportResult, String> {
    let profile_dir = require_profile_dir(profile_path)?;
    let selected_game = selected_game(profile_state)?;
    let profile_name = resolve_profile_name(&profile_dir)?;
    let export_dir = resolve_requested_export_dir(export_dir_override.as_deref(), &selected_game)?;
    fs::create_dir_all(&export_dir)
        .map_err(|error| format!("Der Zielordner konnte nicht vorbereitet werden: {}", error))?;

    let archive_name = ensure_unique_archive_name(&export_dir, &profile_name);
    let archive_path = export_dir.join(&archive_name);
    let temp_archive_path = export_dir.join(format!(".{}.tmp", Uuid::new_v4().simple()));
    let manifest = SharedProfileManifest {
        archive_format: ARCHIVE_FORMAT.to_string(),
        archive_version: ARCHIVE_VERSION,
        exported_at: Local::now().to_rfc3339(),
        game: selected_game,
        profile_name: profile_name.clone(),
        source_profile_folder: profile_dir
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_string(),
        profile_root: ARCHIVE_ROOT.to_string(),
    };

    dev_log!(
        "[profile_sharing] export start profile={} archive={}",
        profile_dir.display(),
        archive_path.display()
    );

    let export_result = write_profile_archive(&profile_dir, &temp_archive_path, &manifest).and_then(
        |exported_files| {
            fs::rename(&temp_archive_path, &archive_path).map_err(|error| {
                format!(
                    "Das ZIP-Archiv konnte nicht in den Zielordner verschoben werden: {}",
                    error
                )
            })?;
            Ok(exported_files)
        },
    );
    if export_result.is_err() && temp_archive_path.exists() {
        let _ = fs::remove_file(&temp_archive_path);
    }
    let exported_files = export_result?;

    dev_log!(
        "[profile_sharing] export success archive={} files={}",
        archive_path.display(),
        exported_files
    );

    Ok(ProfileShareExportResult {
        profile_name,
        archive_name,
        archive_path: archive_path.display().to_string(),
        export_dir: export_dir.display().to_string(),
        exported_files,
    })
}

pub fn inspect_shared_profile_archive(
    archive_path: &str,
    profile_name_override: Option<String>,
    profile_state: &AppProfileState,
) -> Result<ProfileShareImportPreview, String> {
    let archive_path = require_archive_path(archive_path)?;
    let selected_game = selected_game(profile_state)?;
    let import_target_dir = resolve_import_root(&selected_game)?;
    let mut archive = open_archive(&archive_path)?;
    let inspection = inspect_archive(&mut archive)?;
    let import_plan = plan_import_target(
        &import_target_dir,
        &inspection.detected_profile_name,
        profile_name_override,
    );

    Ok(ProfileShareImportPreview {
        archive_path: archive_path.display().to_string(),
        detected_profile_name: inspection.detected_profile_name.clone(),
        suggested_profile_name: inspection.detected_profile_name,
        final_profile_name: import_plan.final_profile_name,
        target_profile_path: import_plan.target_profile_dir.display().to_string(),
        import_target_dir: import_target_dir.display().to_string(),
        archive_root: inspection.archive_root,
        has_manifest: inspection.has_manifest,
        file_count: inspection.file_count,
        profile_name_conflict: import_plan.profile_name_conflict,
    })
}

pub fn import_shared_profile(
    archive_path: &str,
    profile_name_override: Option<String>,
    profile_state: &AppProfileState,
) -> Result<ProfileShareImportResult, String> {
    let archive_path = require_archive_path(archive_path)?;
    let selected_game = selected_game(profile_state)?;
    let import_root = resolve_import_root(&selected_game)?;
    fs::create_dir_all(&import_root)
        .map_err(|error| format!("Der Zielordner fuer den Import konnte nicht vorbereitet werden: {}", error))?;

    let mut archive = open_archive(&archive_path)?;
    let inspection = inspect_archive(&mut archive)?;
    let import_plan = plan_import_target(
        &import_root,
        &inspection.detected_profile_name,
        profile_name_override,
    );
    let final_profile_name = import_plan.final_profile_name;
    let final_profile_dir = import_plan.target_profile_dir;
    let staging_dir = import_root.join(format!(".profile_import_{}", Uuid::new_v4().simple()));

    if staging_dir.exists() {
        fs::remove_dir_all(&staging_dir)
            .map_err(|error| format!("Ein alter Import-Zwischenspeicher konnte nicht entfernt werden: {}", error))?;
    }
    fs::create_dir_all(&staging_dir)
        .map_err(|error| format!("Der temporaere Importordner konnte nicht erstellt werden: {}", error))?;

    dev_log!(
        "[profile_sharing] import start archive={} target={}",
        archive_path.display(),
        final_profile_dir.display()
    );

    let import_result = (|| -> Result<ProfileShareImportResult, String> {
        let imported_files =
            extract_profile_from_archive(&mut archive, &inspection.archive_root, &staging_dir)?;
        if imported_files == 0 {
            return Err("Im ZIP-Archiv wurden keine importierbaren Dateien gefunden.".to_string());
        }

        let imported_profile_sii = staging_dir.join("profile.sii");
        if !imported_profile_sii.exists() {
            return Err("profile.sii wurde im importierten Archiv nicht gefunden.".to_string());
        }

        if final_profile_name != inspection.detected_profile_name {
            rewrite_profile_name(&imported_profile_sii, &final_profile_name)?;
        }

        if final_profile_dir.exists() {
            return Err(format!(
                "Das Zielprofil '{}' existiert bereits. Bitte waehle einen anderen Profilnamen.",
                final_profile_name
            ));
        }

        fs::rename(&staging_dir, &final_profile_dir).map_err(|error| {
            format!(
                "Der Import konnte nicht in den ETS2-/ATS-Profilordner verschoben werden: {}",
                error
            )
        })?;

        dev_log!(
            "[profile_sharing] import success target={} files={}",
            final_profile_dir.display(),
            imported_files
        );

        Ok(ProfileShareImportResult {
            profile_name: final_profile_name,
            profile_path: final_profile_dir.display().to_string(),
            archive_path: archive_path.display().to_string(),
            imported_files,
            import_target_dir: import_root.display().to_string(),
        })
    })();

    if staging_dir.exists() {
        let _ = fs::remove_dir_all(&staging_dir);
    }

    import_result
}

struct ArchiveInspection {
    detected_profile_name: String,
    archive_root: String,
    has_manifest: bool,
    file_count: usize,
}

struct ImportPlan {
    final_profile_name: String,
    target_profile_dir: PathBuf,
    profile_name_conflict: bool,
}

fn require_profile_dir(profile_path: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(profile_path);
    if !path.exists() || !path.is_dir() {
        return Err("Der Profilpfad existiert nicht oder ist kein Verzeichnis.".to_string());
    }
    if !path.join("profile.sii").exists() {
        return Err("profile.sii wurde im Profilverzeichnis nicht gefunden.".to_string());
    }
    Ok(path)
}

fn require_archive_path(archive_path: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(archive_path.trim());
    if archive_path.trim().is_empty() {
        return Err("Bitte waehle zuerst eine ZIP-Datei aus.".to_string());
    }
    if !path.exists() || !path.is_file() {
        return Err("Die angegebene ZIP-Datei wurde nicht gefunden.".to_string());
    }
    if path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| !value.eq_ignore_ascii_case("zip"))
        .unwrap_or(true)
    {
        return Err("Es werden nur ZIP-Dateien unterstuetzt.".to_string());
    }
    Ok(path)
}

fn selected_game(profile_state: &AppProfileState) -> Result<String, String> {
    profile_state
        .selected_game
        .lock()
        .map_err(|_| "selected_game konnte nicht gelesen werden.".to_string())
        .map(|value| value.clone())
}

fn resolve_export_dir(selected_game: &str) -> Result<PathBuf, String> {
    if let Some(download_dir) = dirs::download_dir() {
        if fs::create_dir_all(&download_dir).is_ok() {
            return Ok(download_dir);
        }
    }

    if let Some(base_path) = get_base_path(selected_game) {
        let fallback = base_path.join("Save Edit Tool Exports");
        fs::create_dir_all(&fallback).map_err(|error| error.to_string())?;
        return Ok(fallback);
    }

    Err("Es konnte kein Exportverzeichnis aufgeloest werden.".to_string())
}

fn resolve_requested_export_dir(
    export_dir_override: Option<&str>,
    selected_game: &str,
) -> Result<PathBuf, String> {
    match export_dir_override.map(str::trim) {
        Some(path) if !path.is_empty() => {
            let target = PathBuf::from(path);
            if target.exists() && !target.is_dir() {
                return Err("Der ausgewaehlte Exportpfad ist kein Ordner.".to_string());
            }
            Ok(target)
        }
        _ => resolve_export_dir(selected_game),
    }
}

fn resolve_import_root(selected_game: &str) -> Result<PathBuf, String> {
    let base_path = get_base_path(selected_game)
        .ok_or("Das Spielbasisverzeichnis konnte nicht aufgeloest werden.".to_string())?;
    Ok(base_path.join("profiles"))
}

fn resolve_import_archive_start_dir(selected_game: &str) -> Option<PathBuf> {
    dirs::download_dir().or_else(|| get_base_path(selected_game))
}

fn resolve_profile_name(profile_dir: &Path) -> Result<String, String> {
    let profile_sii = profile_dir.join("profile.sii");
    if let Ok(content) = decrypt_if_needed(&profile_sii) {
        if let Some(profile_name) = extract_profile_name(&content) {
            return Ok(profile_name);
        }
    }

    if let Some(folder_name) = profile_dir.file_name().and_then(|value| value.to_str()) {
        if let Some(decoded) = decode_hex_folder_name(folder_name) {
            return Ok(decoded);
        }
        return Ok(folder_name.to_string());
    }

    Err("Der Profilname konnte nicht aufgeloest werden.".to_string())
}

fn build_archive_name(profile_name: &str) -> String {
    let safe_name = sanitize_filename_component(profile_name);
    let timestamp = Local::now().format("%Y-%m-%d_%H-%M-%S_%3f");
    format!("profile_{}_{}.zip", safe_name, timestamp)
}

fn ensure_unique_archive_name(export_dir: &Path, profile_name: &str) -> String {
    let mut archive_name = build_archive_name(profile_name);
    let mut target_path = export_dir.join(&archive_name);
    let mut attempt = 1usize;

    while target_path.exists() {
        archive_name = format!(
            "profile_{}_{}_{}.zip",
            sanitize_filename_component(profile_name),
            Local::now().format("%Y-%m-%d_%H-%M-%S_%3f"),
            attempt
        );
        target_path = export_dir.join(&archive_name);
        attempt += 1;
    }

    archive_name
}

fn file_path_to_string(path: tauri_plugin_dialog::FilePath) -> Result<String, String> {
    path.into_path()
        .map(|value| value.display().to_string())
        .map_err(|_| "Der ausgewaehlte Dateidialog-Pfad konnte nicht gelesen werden.".to_string())
}

fn open_archive(archive_path: &Path) -> Result<ZipArchive<File>, String> {
    let file = File::open(archive_path).map_err(|error| {
        format!(
            "Die ZIP-Datei konnte nicht geoeffnet werden: {}",
            error
        )
    })?;
    ZipArchive::new(file).map_err(|error| format!("Die ZIP-Datei ist ungueltig oder beschaedigt: {}", error))
}

fn sanitize_filename_component(value: &str) -> String {
    let mut sanitized = String::new();
    let mut last_was_separator = false;

    for character in value.trim().chars() {
        let safe = match character {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            c if c.is_control() => '_',
            c if c.is_whitespace() => '_',
            c => c,
        };

        if safe == '_' {
            if !last_was_separator {
                sanitized.push('_');
            }
            last_was_separator = true;
        } else {
            sanitized.push(safe);
            last_was_separator = false;
        }
    }

    let sanitized = sanitized.trim_matches(|character| matches!(character, '_' | '.' | ' '));
    if sanitized.is_empty() {
        "profile".to_string()
    } else {
        sanitized.to_ascii_lowercase()
    }
}

fn write_profile_archive(
    profile_dir: &Path,
    archive_path: &Path,
    manifest: &SharedProfileManifest,
) -> Result<usize, String> {
    let file = File::create(archive_path).map_err(|error| {
        format!(
            "Die ZIP-Datei konnte nicht erstellt werden: {}",
            error
        )
    })?;
    let mut zip = ZipWriter::new(file);
    let options: FileOptions<()> =
        FileOptions::default().compression_method(CompressionMethod::Deflated);

    let manifest_json =
        serde_json::to_vec_pretty(manifest).map_err(|error| format!("Das Export-Manifest konnte nicht erstellt werden: {}", error))?;
    zip.start_file(MANIFEST_NAME, options)
        .map_err(|error| format!("Das Export-Manifest konnte nicht in das Archiv geschrieben werden: {}", error))?;
    zip.write_all(&manifest_json)
        .map_err(|error| format!("Das Export-Manifest konnte nicht geschrieben werden: {}", error))?;

    let mut exported_files = 0usize;
    for entry in WalkDir::new(profile_dir) {
        let entry = entry.map_err(|error| error.to_string())?;
        if !entry.file_type().is_file() {
            continue;
        }

        let relative = entry
            .path()
            .strip_prefix(profile_dir)
            .map_err(|error| format!("Ein Profildateipfad konnte nicht vorbereitet werden: {}", error))?;
        let archive_name = format!(
            "{}/{}",
            ARCHIVE_ROOT,
            relative.to_string_lossy().replace('\\', "/")
        );

        zip.start_file(archive_name, options)
            .map_err(|error| format!("Eine Profildatei konnte nicht in das ZIP-Archiv aufgenommen werden: {}", error))?;
        let data = fs::read(entry.path()).map_err(|error| {
            format!(
                "Eine Profildatei konnte nicht gelesen werden ({}): {}",
                entry.path().display(),
                error
            )
        })?;
        zip.write_all(&data)
            .map_err(|error| format!("Eine Profildatei konnte nicht in das Archiv geschrieben werden: {}", error))?;
        exported_files += 1;
    }

    if exported_files == 0 {
        return Err("Im ausgewaehlten Profil wurden keine exportierbaren Dateien gefunden.".to_string());
    }

    zip.finish()
        .map_err(|error| format!("Das ZIP-Archiv konnte nicht abgeschlossen werden: {}", error))?;
    Ok(exported_files)
}

fn inspect_archive<R: Read + std::io::Seek>(
    archive: &mut ZipArchive<R>,
) -> Result<ArchiveInspection, String> {
    let manifest = read_manifest(archive)?;
    let (archive_root, file_count) = locate_archive_root(archive, manifest.as_ref())?;
    let detected_profile_name =
        read_profile_name_from_archive(archive, &archive_root, manifest.as_ref())?;

    Ok(ArchiveInspection {
        detected_profile_name,
        archive_root,
        has_manifest: manifest.is_some(),
        file_count,
    })
}

fn read_manifest<R: Read + std::io::Seek>(
    archive: &mut ZipArchive<R>,
) -> Result<Option<SharedProfileManifest>, String> {
    for index in 0..archive.len() {
        let mut file = archive.by_index(index).map_err(|error| error.to_string())?;
        if normalize_archive_name(file.name()) != MANIFEST_NAME {
            continue;
        }

        let mut content = String::new();
        file.read_to_string(&mut content)
            .map_err(|error| error.to_string())?;
        let manifest: SharedProfileManifest =
            serde_json::from_str(&content).map_err(|error| error.to_string())?;

        if manifest.archive_format != ARCHIVE_FORMAT {
            return Err("Das ZIP-Archiv stammt nicht aus dem Profile-Sharing-Format.".to_string());
        }

        return Ok(Some(manifest));
    }

    Ok(None)
}

fn locate_archive_root<R: Read + std::io::Seek>(
    archive: &mut ZipArchive<R>,
    manifest: Option<&SharedProfileManifest>,
) -> Result<(String, usize), String> {
    if let Some(manifest) = manifest {
        let root = normalize_archive_name(&manifest.profile_root);
        let file_count = count_files_under_root(archive, &root)?;
        if file_count > 0 {
            return Ok((root, file_count));
        }
    }

    for index in 0..archive.len() {
        let file = archive.by_index(index).map_err(|error| error.to_string())?;
        let normalized = normalize_archive_name(file.name());
        if normalized == "profile.sii" {
            drop(file);
            let file_count = count_files_under_root(archive, "")?;
            return Ok((String::new(), file_count));
        }
        if let Some(root) = normalized.strip_suffix("/profile.sii") {
            drop(file);
            let file_count = count_files_under_root(archive, root)?;
            if file_count > 0 {
                return Ok((root.to_string(), file_count));
            }
        }
    }

    Err("Im ZIP-Archiv wurde kein importierbares Profil gefunden.".to_string())
}

fn count_files_under_root<R: Read + std::io::Seek>(
    archive: &mut ZipArchive<R>,
    archive_root: &str,
) -> Result<usize, String> {
    let prefix = if archive_root.is_empty() {
        String::new()
    } else {
        format!("{}/", archive_root)
    };
    let mut count = 0usize;

    for index in 0..archive.len() {
        let file = archive.by_index(index).map_err(|error| error.to_string())?;
        let normalized = normalize_archive_name(file.name());
        if normalized == MANIFEST_NAME {
            continue;
        }
        if file.is_dir() {
            continue;
        }
        if archive_root.is_empty() || normalized.starts_with(&prefix) {
            count += 1;
        }
    }

    Ok(count)
}

fn read_profile_name_from_archive<R: Read + std::io::Seek>(
    archive: &mut ZipArchive<R>,
    archive_root: &str,
    manifest: Option<&SharedProfileManifest>,
) -> Result<String, String> {
    let profile_sii_path = if archive_root.is_empty() {
        "profile.sii".to_string()
    } else {
        format!("{}/profile.sii", archive_root)
    };

    let mut profile_file = archive
        .by_name(&profile_sii_path)
        .map_err(|_| "profile.sii wurde im Archiv nicht gefunden.".to_string())?;
    let mut profile_bytes = Vec::new();
    profile_file
        .read_to_end(&mut profile_bytes)
        .map_err(|error| error.to_string())?;
    drop(profile_file);

    let temp_dir = std::env::temp_dir().join(format!(
        "ets2_tool_profile_share_preview_{}.sii",
        Local::now().timestamp_millis()
    ));
    fs::write(&temp_dir, &profile_bytes).map_err(|error| error.to_string())?;
    let profile_name = decrypt_if_needed(&temp_dir)
        .ok()
        .and_then(|content| extract_profile_name(&content));
    let _ = fs::remove_file(&temp_dir);

    if let Some(profile_name) = profile_name {
        return Ok(profile_name);
    }

    if let Some(manifest) = manifest {
        if !manifest.profile_name.trim().is_empty() {
            return Ok(manifest.profile_name.trim().to_string());
        }
    }

    Err("Der Profilname konnte im ZIP-Archiv nicht gelesen werden.".to_string())
}

fn extract_profile_from_archive<R: Read + std::io::Seek>(
    archive: &mut ZipArchive<R>,
    archive_root: &str,
    destination_dir: &Path,
) -> Result<usize, String> {
    let prefix = if archive_root.is_empty() {
        String::new()
    } else {
        format!("{}/", archive_root)
    };
    let mut extracted_files = 0usize;

    for index in 0..archive.len() {
        let mut file = archive
            .by_index(index)
            .map_err(|error| format!("Ein ZIP-Eintrag konnte nicht gelesen werden: {}", error))?;
        let normalized = normalize_archive_name(file.name());

        if normalized == MANIFEST_NAME || file.is_dir() {
            continue;
        }

        let relative = if archive_root.is_empty() {
            normalized
        } else if normalized.starts_with(&prefix) {
            normalized[prefix.len()..].to_string()
        } else {
            continue;
        };

        if relative.is_empty() {
            continue;
        }

        let safe_path = safe_relative_path(&relative)?;
        let target_path = destination_dir.join(safe_path);

        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                format!(
                    "Ein Zielordner fuer den Import konnte nicht erstellt werden: {}",
                    error
                )
            })?;
        }

        let mut output = File::create(&target_path).map_err(|error| {
            format!(
                "Eine importierte Datei konnte nicht erstellt werden ({}): {}",
                target_path.display(),
                error
            )
        })?;
        std::io::copy(&mut file, &mut output).map_err(|error| {
            format!(
                "Eine Datei aus dem ZIP-Archiv konnte nicht entpackt werden ({}): {}",
                target_path.display(),
                error
            )
        })?;
        extracted_files += 1;
    }

    Ok(extracted_files)
}

fn safe_relative_path(value: &str) -> Result<PathBuf, String> {
    let mut path = PathBuf::new();

    for component in Path::new(value).components() {
        match component {
            Component::Normal(value) => path.push(value),
            Component::CurDir => {}
            _ => {
                return Err("Das ZIP-Archiv enthaelt einen ungueltigen Pfad.".to_string());
            }
        }
    }

    if path.as_os_str().is_empty() {
        return Err("Das ZIP-Archiv enthaelt einen leeren Pfad.".to_string());
    }

    Ok(path)
}

fn normalize_archive_name(value: &str) -> String {
    value
        .replace('\\', "/")
        .trim_start_matches("./")
        .trim_start_matches('/')
        .trim_end_matches('/')
        .to_string()
}

fn plan_import_target(
    import_root: &Path,
    detected_profile_name: &str,
    profile_name_override: Option<String>,
) -> ImportPlan {
    let requested_name = profile_name_override.unwrap_or_default().trim().to_string();
    let base_name = if requested_name.is_empty() {
        detected_profile_name.trim()
    } else {
        requested_name.trim()
    };
    let fallback_name = if base_name.is_empty() {
        "Imported Profile"
    } else {
        base_name
    };

    let direct_path = import_root.join(text_to_hex(fallback_name));
    if !direct_path.exists() {
        return ImportPlan {
            final_profile_name: fallback_name.to_string(),
            target_profile_dir: direct_path,
            profile_name_conflict: false,
        };
    }

    let final_profile_name = format!(
        "{} Imported {}",
        fallback_name,
        Local::now().format("%Y-%m-%d %H-%M-%S")
    );

    ImportPlan {
        target_profile_dir: import_root.join(text_to_hex(&final_profile_name)),
        final_profile_name,
        profile_name_conflict: true,
    }
}

fn rewrite_profile_name(profile_sii: &Path, new_name: &str) -> Result<(), String> {
    let content = decrypt_if_needed(profile_sii).map_err(|error| error.to_string())?;
    let updated = change_profile_name_in_sii(&content, new_name)?;
    fs::write(profile_sii, updated).map_err(|error| error.to_string())
}

fn change_profile_name_in_sii(content: &str, new_name: &str) -> Result<String, String> {
    let pattern =
        regex::Regex::new(r#"(?m)^(?P<indent>\s*)profile_name\s*:\s*"?(?P<name>[^"\r\n]*)"?"#)
            .map_err(|error| error.to_string())?;

    if !pattern.is_match(content) {
        return Err("Der Profilname konnte in profile.sii nicht ersetzt werden.".to_string());
    }

    Ok(pattern
        .replace(content, |caps: &regex::Captures| {
            format!("{}profile_name: \"{}\"", &caps["indent"], new_name)
        })
        .to_string())
}
