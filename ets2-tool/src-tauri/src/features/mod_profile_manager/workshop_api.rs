use super::models::WorkshopMod;
use regex::Regex;
use serde::Deserialize;
use std::path::PathBuf;

const ETS2_APP_ID: u32 = 227300;
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

pub fn is_workshop_mod_downloaded(app_id: u32, mod_id: u64) -> bool {
    default_workshop_mod_path(app_id, mod_id)
        .map(|path| path.is_dir())
        .unwrap_or(false)
}

fn default_workshop_mod_path(app_id: u32, mod_id: u64) -> Option<PathBuf> {
    if cfg!(target_os = "windows") {
        std::env::var_os("ProgramFiles(x86)").map(|program_files| {
            PathBuf::from(program_files)
                .join("Steam")
                .join("steamapps")
                .join("workshop")
                .join("content")
                .join(app_id.to_string())
                .join(mod_id.to_string())
        })
    } else if cfg!(target_os = "linux") {
        dirs::home_dir().map(|home| {
            home.join(".steam")
                .join("steam")
                .join("steamapps")
                .join("workshop")
                .join("content")
                .join(app_id.to_string())
                .join(mod_id.to_string())
        })
    } else if cfg!(target_os = "macos") {
        dirs::home_dir().map(|home| {
            home.join("Library")
                .join("Application Support")
                .join("Steam")
                .join("steamapps")
                .join("workshop")
                .join("content")
                .join(app_id.to_string())
                .join(mod_id.to_string())
        })
    } else {
        None
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
    #[ignore]
    fn fetches_realistic_cabin_soundproofing_from_steam() {
        let workshop_mod = fetch_workshop_mod("3710074411").unwrap();
        assert_eq!(workshop_mod.id, 3710074411);
        assert_eq!(workshop_mod.app_id, ETS2_APP_ID);
    }
}
