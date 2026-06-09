use super::models::{WorkshopInstallStatus, WorkshopMod};
use super::steam_paths;
use regex::Regex;
use serde::Deserialize;
use std::fs;
use std::path::Path;

const ETS2_APP_ID: u32 = 227300;
const ETS2_APP_ID_STR: &str = "227300";
const PUBLISHED_FILE_DETAILS_URL: &str =
    "https://api.steampowered.com/ISteamRemoteStorage/GetPublishedFileDetails/v1/";

#[derive(Debug, Deserialize)]
struct PublishedFileDetailsResponse {
    response: PublishedFileDetailsInner,
}

#[derive(Debug, Deserialize)]
struct PublishedFileDetailsInner {
    publishedfiledetails: Vec<PublishedFileDetails>,
}

#[derive(Debug, Deserialize)]
struct PublishedFileDetails {
    publishedfileid: Option<String>,
    title: Option<String>,
    consumer_app_id: Option<u32>,
    app_id: Option<u32>,
    creator_app_id: Option<u32>,
    result: Option<u32>,
}

pub fn parse_workshop_id(input: &str) -> Result<u64, String> {
    let value = input.trim();
    if value.is_empty() {
        return Err("Workshop ID or URL is required.".to_string());
    }

    if value.chars().all(|character| character.is_ascii_digit()) {
        return value.parse::<u64>().map_err(|error| error.to_string());
    }

    let regex = Regex::new(r"[?&]id=(\d+)").map_err(|error| error.to_string())?;
    let id = regex
        .captures(value)
        .and_then(|captures| captures.get(1))
        .ok_or_else(|| "No Steam Workshop ID was found in the input.".to_string())?
        .as_str();

    id.parse::<u64>().map_err(|error| error.to_string())
}

pub fn fetch_workshop_mod(input: &str) -> Result<WorkshopMod, String> {
    let requested_id = parse_workshop_id(input)?;
    let params = [
        ("itemcount".to_string(), "1".to_string()),
        ("publishedfileids[0]".to_string(), requested_id.to_string()),
    ];

    let response: PublishedFileDetailsResponse = reqwest::blocking::Client::new()
        .post(PUBLISHED_FILE_DETAILS_URL)
        .form(&params)
        .send()
        .map_err(|error| format!("Failed to read Steam Workshop metadata: {}", error))?
        .error_for_status()
        .map_err(|error| format!("Steam Workshop request failed: {}", error))?
        .json()
        .map_err(|error| format!("Failed to parse Steam Workshop response: {}", error))?;

    let details = response
        .response
        .publishedfiledetails
        .into_iter()
        .next()
        .ok_or_else(|| "Steam returned no Workshop details.".to_string())?;

    if let Some(result) = details.result {
        if result != 1 {
            return Err(format!(
                "Steam returned result {} for this Workshop item.",
                result
            ));
        }
    }

    let id = details
        .publishedfileid
        .as_deref()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(requested_id);
    let name = details
        .title
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| "Steam Workshop metadata did not include a title.".to_string())?;
    let app_id = details
        .consumer_app_id
        .or(details.app_id)
        .or(details.creator_app_id)
        .ok_or_else(|| "Steam Workshop metadata did not include an AppID.".to_string())?;

    if app_id != ETS2_APP_ID {
        return Err(format!(
            "Only Euro Truck Simulator 2 Workshop mods are supported. Expected AppID {}, got {}.",
            ETS2_APP_ID, app_id
        ));
    }

    Ok(WorkshopMod {
        id,
        name,
        app_id,
        enabled: true,
        url: Some(format!(
            "https://steamcommunity.com/sharedfiles/filedetails/?id={id}"
        )),
        status: Some("verified".to_string()),
    })
}

pub fn check_workshop_mod_installed(
    mod_id: &str,
    app_id: &str,
) -> Result<WorkshopInstallStatus, String> {
    let trimmed_mod_id = mod_id.trim();
    let trimmed_app_id = app_id.trim();
    if !trimmed_mod_id.chars().all(|character| character.is_ascii_digit()) {
        return Ok(failed_status(trimmed_mod_id, trimmed_app_id, "invalid_mod_id"));
    }
    if !trimmed_app_id.chars().all(|character| character.is_ascii_digit()) {
        return Ok(failed_status(trimmed_mod_id, trimmed_app_id, "invalid_app_id"));
    }

    crate::dev_log!(
        "[mod-profile-manager] check workshop install mod_id={} app_id={}",
        trimmed_mod_id,
        trimmed_app_id
    );

    let libraries = match steam_paths::resolve_steam_libraries_for_app(Some(trimmed_app_id)) {
        Ok(libraries) => libraries,
        Err(error) if error == "steam_not_found" => {
            return Ok(failed_status(trimmed_mod_id, trimmed_app_id, "steam_not_found"));
        }
        Err(error) if error == "no_steam_libraries_found" => {
            return Ok(failed_status(
                trimmed_mod_id,
                trimmed_app_id,
                "no_steam_libraries_found",
            ));
        }
        Err(error) => return Err(error),
    };

    let mut checked_libraries = Vec::new();
    let mut checked_paths = Vec::new();
    let mut subscribed_but_missing = false;
    let mut workshop_folder_empty = false;
    let mut workshop_content_root_missing = false;

    for (library_dir, contains_app) in libraries {
        checked_libraries.push(library_dir.display().to_string());
        crate::dev_log!(
            "[mod-profile-manager] check workshop library={} app_id={} listed_in_library={}",
            library_dir.display(),
            trimmed_app_id,
            contains_app
        );
        let workshop_content_root = library_dir
            .join("steamapps")
            .join("workshop")
            .join("content")
            .join(trimmed_app_id);
        let workshop_mod_path = workshop_content_root.join(trimmed_mod_id);
        checked_paths.push(workshop_mod_path.display().to_string());

        let content_exists = workshop_mod_path.is_dir();
        let content_is_empty = if content_exists {
            is_directory_empty(&workshop_mod_path).unwrap_or(true)
        } else {
            false
        };
        let acf_exists = library_dir
            .join("steamapps")
            .join("workshop")
            .join(format!("appworkshop_{}.acf", trimmed_app_id))
            .is_file();
        let acf_contains_mod =
            check_workshop_acf_contains_mod(&library_dir, trimmed_app_id, trimmed_mod_id)?;

        crate::dev_log!(
            "[mod-profile-manager] check workshop mod_id={} checked_path={} content_exists={} content_empty={} appworkshop_exists={} acf_contains_mod={}",
            trimmed_mod_id,
            workshop_mod_path.display(),
            content_exists,
            content_is_empty,
            acf_exists,
            acf_contains_mod
        );

        if content_exists && !content_is_empty {
            crate::dev_log!(
                "[mod-profile-manager] check workshop final_status=installed path={}",
                workshop_mod_path.display()
            );
            return Ok(WorkshopInstallStatus {
                mod_id: trimmed_mod_id.to_string(),
                app_id: trimmed_app_id.to_string(),
                installed: true,
                workshop_path: Some(workshop_mod_path.display().to_string()),
                checked_libraries,
                checked_paths,
                reason: None,
            });
        }

        if content_exists && content_is_empty {
            workshop_folder_empty = true;
        } else if !workshop_content_root.is_dir() {
            workshop_content_root_missing = true;
        }

        if acf_contains_mod {
            subscribed_but_missing = true;
        }
    }

    let reason = if workshop_folder_empty {
        "workshop_folder_empty"
    } else if subscribed_but_missing {
        "subscribed_but_content_missing"
    } else if workshop_content_root_missing {
        "workshop_folder_not_found"
    } else {
        "not_found"
    };

    crate::dev_log!(
        "[mod-profile-manager] check workshop final_status=not_installed mod_id={} app_id={} reason={} checked_paths={:?}",
        trimmed_mod_id,
        trimmed_app_id,
        reason,
        checked_paths
    );

    Ok(WorkshopInstallStatus {
        mod_id: trimmed_mod_id.to_string(),
        app_id: trimmed_app_id.to_string(),
        installed: false,
        workshop_path: None,
        checked_libraries,
        checked_paths,
        reason: Some(reason.to_string()),
    })
}

pub fn check_workshop_acf_contains_mod(
    library_dir: &Path,
    app_id: &str,
    mod_id: &str,
) -> Result<bool, String> {
    let acf_path = library_dir
        .join("steamapps")
        .join("workshop")
        .join(format!("appworkshop_{}.acf", app_id));
    if !acf_path.is_file() {
        crate::dev_log!(
            "[mod-profile-manager] appworkshop file missing library={} app_id={} path={}",
            library_dir.display(),
            app_id,
            acf_path.display()
        );
        return Ok(false);
    }

    let content = match fs::read_to_string(&acf_path) {
        Ok(content) => content,
        Err(error) => {
            crate::dev_log!(
                "[mod-profile-manager] appworkshop read failed library={} app_id={} path={} error={}",
                library_dir.display(),
                app_id,
                acf_path.display(),
                error
            );
            return Ok(false);
        }
    };
    let quoted_id = format!("\"{}\"", mod_id);
    let found = content.contains(&quoted_id);
    crate::dev_log!(
        "[mod-profile-manager] appworkshop check library={} app_id={} mod_id={} found={}",
        library_dir.display(),
        app_id,
        mod_id,
        found
    );
    Ok(found)
}

pub fn is_workshop_mod_downloaded(app_id: u32, mod_id: u64) -> bool {
    check_workshop_mod_installed(&mod_id.to_string(), &app_id.to_string())
        .map(|status| status.installed)
        .unwrap_or(false)
}

pub fn check_ets2_workshop_mod_installed(mod_id: &str) -> Result<WorkshopInstallStatus, String> {
    check_workshop_mod_installed(mod_id, ETS2_APP_ID_STR)
}

pub fn discover_ets2_workshop_libraries() -> Result<Vec<String>, String> {
    steam_paths::resolve_steam_libraries_for_app(Some(ETS2_APP_ID_STR)).map(|libraries| {
        libraries
            .into_iter()
            .map(|(path, _)| path.display().to_string())
            .collect()
    })
}

fn is_directory_empty(path: &Path) -> Result<bool, String> {
    let mut entries = fs::read_dir(path)
        .map_err(|error| format!("Failed to read {}: {}", path.display(), error))?;
    Ok(entries.next().is_none())
}

fn failed_status(mod_id: &str, app_id: &str, reason: &str) -> WorkshopInstallStatus {
    crate::dev_log!(
        "[mod-profile-manager] check workshop failed mod_id={} app_id={} reason={}",
        mod_id,
        app_id,
        reason
    );
    WorkshopInstallStatus {
        mod_id: mod_id.to_string(),
        app_id: app_id.to_string(),
        installed: false,
        workshop_path: None,
        checked_libraries: Vec::new(),
        checked_paths: Vec::new(),
        reason: Some(reason.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_raw_workshop_id() {
        assert_eq!(parse_workshop_id("3710074411").unwrap(), 3710074411);
    }

    #[test]
    fn parses_workshop_url() {
        let input = "https://steamcommunity.com/sharedfiles/filedetails/?id=3710074411";
        assert_eq!(parse_workshop_id(input).unwrap(), 3710074411);
    }

    #[test]
    fn rejects_invalid_workshop_id_for_install_status() {
        let status = check_workshop_mod_installed("abc", ETS2_APP_ID_STR).unwrap();
        assert!(!status.installed);
        assert_eq!(status.reason.as_deref(), Some("invalid_mod_id"));
    }

    #[test]
    #[ignore]
    fn fetches_realistic_cabin_soundproofing_from_steam() {
        let workshop_mod = fetch_workshop_mod("3710074411").unwrap();
        assert_eq!(workshop_mod.id, 3710074411);
        assert_eq!(workshop_mod.app_id, ETS2_APP_ID);
    }
}
