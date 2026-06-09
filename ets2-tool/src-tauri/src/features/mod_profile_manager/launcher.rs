use std::process::Command;

const STEAM_PROTOCOL_ERROR: &str = "Steam protocol could not be opened. Please check if Steam is installed and steam:// protocol is registered.";

pub fn open_steam_console() -> Result<(), String> {
    open_steam_uri("steam://open/console")
}

pub fn open_workshop_page(mod_id: &str) -> Result<(), String> {
    let mod_id = validate_workshop_id(mod_id)?;
    open_steam_uri(&format!("steam://url/SteamWorkshopPage/{mod_id}"))
}

pub fn open_sandbox_mod_workshop_page(mod_id: &str) -> Result<(), String> {
    let mod_id = validate_workshop_id(mod_id)?;
    open_external_url(&format!(
        "https://steamcommunity.com/sharedfiles/filedetails/?id={mod_id}"
    ))
}

pub fn open_sandbox_mod_in_steam(mod_id: &str) -> Result<(), String> {
    let mod_id = validate_workshop_id(mod_id)?;
    open_steam_uri(&format!("steam://url/CommunityFilePage/{mod_id}"))
}

pub fn open_workshop_subscribe_page(mod_id: &str) -> Result<(), String> {
    let mod_id = validate_workshop_id(mod_id)?;
    open_steam_uri(&format!(
        "steam://openurl/https://steamcommunity.com/sharedfiles/filedetails/?id={mod_id}"
    ))
}

pub fn open_protocol_url(url: &str) -> Result<(), String> {
    open_steam_uri(url)
}

pub fn open_external_url(url: &str) -> Result<(), String> {
    let value = sanitize_external_url(url)?;

    if cfg!(target_os = "windows") {
        for (name, command, args) in [
            ("explorer", "explorer.exe", vec![value.as_str()]),
            ("cmd_start", "cmd", vec!["/C", "start", "", value.as_str()]),
        ] {
            let result = run_command_status(command, &args);
            match result {
                Ok(true) => {
                    println!("[mod-profile-manager] open_external_url fallback={name} success");
                    return Ok(());
                }
                Ok(false) => {
                    println!("[mod-profile-manager] open_external_url fallback={name} failed status");
                }
                Err(error) => {
                    println!("[mod-profile-manager] open_external_url fallback={name} error={error}");
                }
            }
        }

        return Err("External URL could not be opened.".to_string());
    }

    let result = if cfg!(target_os = "linux") {
        run_command_status("xdg-open", &[value.as_str()])
    } else if cfg!(target_os = "macos") {
        run_command_status("open", &[value.as_str()])
    } else {
        return Err("Unsupported operating system.".to_string());
    };

    match result {
        Ok(true) => Ok(()),
        Ok(false) => Err("External URL could not be opened.".to_string()),
        Err(error) => Err(format!("External URL could not be opened. ({error})")),
    }
}

pub fn sanitize_steam_uri(input: &str) -> Result<String, String> {
    println!("[mod-profile-manager] sanitize_steam_uri input={input:?}");
    let mut value = input.trim();
    loop {
        let trimmed = value.trim_matches(|character| {
            matches!(character, '"' | '\'' | '`' | '\\') || character.is_whitespace()
        });
        if trimmed.len() == value.len() {
            break;
        }
        value = trimmed;
    }

    if !value.starts_with("steam://") {
        return Err(format!("Invalid Steam URI: {value}"));
    }
    if value.contains('\0') {
        return Err("Invalid Steam URI: contains NUL byte.".to_string());
    }

    println!("[mod-profile-manager] sanitize_steam_uri output={value:?}");
    Ok(value.to_string())
}

pub fn sanitize_external_url(input: &str) -> Result<String, String> {
    let value = input.trim().trim_matches(|character| {
        matches!(character, '"' | '\'' | '`' | '\\') || character.is_whitespace()
    });
    if !(value.starts_with("https://") || value.starts_with("http://")) {
        return Err(format!("Invalid external URL: {value}"));
    }
    if value.contains('\0') {
        return Err("Invalid external URL: contains NUL byte.".to_string());
    }
    Ok(value.to_string())
}

pub fn open_steam_uri(uri: &str) -> Result<(), String> {
    let uri = sanitize_steam_uri(uri)?;

    if cfg!(target_os = "windows") {
        for (name, command, args) in [
            (
                "rundll32_url_file_protocol_handler",
                "rundll32.exe",
                vec!["url.dll,FileProtocolHandler", uri.as_str()],
            ),
            ("explorer", "explorer.exe", vec![uri.as_str()]),
            ("cmd_start", "cmd", vec!["/C", "start", "", uri.as_str()]),
        ] {
            let result = run_command_status(command, &args);
            match result {
                Ok(true) => {
                    println!("[mod-profile-manager] open_steam_uri fallback={name} success");
                    return Ok(());
                }
                Ok(false) => {
                    println!("[mod-profile-manager] open_steam_uri fallback={name} failed status");
                }
                Err(error) => {
                    println!("[mod-profile-manager] open_steam_uri fallback={name} error={error}");
                }
            }
        }

        return Err(STEAM_PROTOCOL_ERROR.to_string());
    }

    let result = if cfg!(target_os = "linux") {
        run_command_status("xdg-open", &[uri.as_str()])
    } else if cfg!(target_os = "macos") {
        run_command_status("open", &[uri.as_str()])
    } else {
        return Err("Unsupported operating system.".to_string());
    };

    match result {
        Ok(true) => Ok(()),
        Ok(false) => Err(STEAM_PROTOCOL_ERROR.to_string()),
        Err(error) => Err(format!("{STEAM_PROTOCOL_ERROR} ({error})")),
    }
}

pub fn validate_workshop_id(input: &str) -> Result<String, String> {
    let value = input.trim();
    if value.is_empty() {
        return Err("Workshop ID is required.".to_string());
    }
    if !value.chars().all(|character| character.is_ascii_digit()) {
        return Err(format!("Invalid Workshop ID: {value}"));
    }
    Ok(value.to_string())
}

fn run_command_status(command: &str, args: &[&str]) -> Result<bool, String> {
    Command::new(command)
        .args(args)
        .status()
        .map(|status| status.success())
        .map_err(|error| format!("{command}: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitizes_backslashed_steam_uri() {
        assert_eq!(
            sanitize_steam_uri(r#"\steam://open/console\"#).unwrap(),
            "steam://open/console"
        );
    }

    #[test]
    fn rejects_plain_https_url() {
        assert!(
            sanitize_steam_uri("https://steamcommunity.com/sharedfiles/filedetails/?id=3710074411")
                .is_err()
        );
    }

    #[test]
    fn validates_workshop_ids() {
        assert_eq!(validate_workshop_id("3710074411").unwrap(), "3710074411");
        assert!(validate_workshop_id(r#"3710074411\"#).is_err());
    }
}
