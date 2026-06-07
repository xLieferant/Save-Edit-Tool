use chrono::Local;
use regex::Regex;
use std::fs;
use std::path::{Path, PathBuf};

pub fn read_active_mods_from_text(text: &str) -> Result<Vec<u64>, String> {
    let body = economy_block_body(text)?;
    let legacy_regex =
        Regex::new(r#"mod_activated\[\d+\]:\s*"mod/(\d+)\.scs""#).map_err(|e| e.to_string())?;
    let workshop_regex =
        Regex::new(r#"mod_activated\[\d+\]:\s*"mod_workshop_package\.([0-9a-fA-F]+)""#)
            .map_err(|e| e.to_string())?;

    let mut mods = legacy_regex
        .captures_iter(body)
        .filter_map(|captures| captures.get(1))
        .filter_map(|id| id.as_str().parse::<u64>().ok())
        .collect::<Vec<_>>();
    mods.extend(
        workshop_regex
            .captures_iter(body)
            .filter_map(|captures| captures.get(1))
            .filter_map(|id| u64::from_str_radix(id.as_str(), 16).ok()),
    );
    Ok(mods)
}

pub fn overwrite_active_mods_in_text(text: &str, mod_ids: &[u64]) -> Result<String, String> {
    let workshop_mod_ids = mod_ids.iter().map(u64::to_string).collect::<Vec<_>>();
    replace_active_workshop_mods_in_game_sii(text, &workshop_mod_ids).map(|(content, _)| content)
}

pub fn replace_active_workshop_mods_in_game_sii(
    game_sii_content: &str,
    workshop_mod_ids: &[String],
) -> Result<(String, usize), String> {
    if !looks_like_decoded_sii(game_sii_content) {
        return Err("game.sii does not look like decoded UTF-8 SII content.".to_string());
    }

    let economy_regex =
        Regex::new(r"(?s)(economy\s*:\s*[^{]+\{\r?\n)(?P<body>.*?)(?P<close>\r?\n\})")
            .map_err(|error| error.to_string())?;
    let captures = economy_regex
        .captures(game_sii_content)
        .ok_or_else(|| "No economy block found in game.sii.".to_string())?;
    let full_match = captures
        .get(0)
        .ok_or_else(|| "No economy block match found.".to_string())?;
    let header = captures
        .get(1)
        .ok_or_else(|| "No economy block header found.".to_string())?
        .as_str();
    let body = captures
        .name("body")
        .ok_or_else(|| "No economy block body found.".to_string())?
        .as_str();
    let close = captures
        .name("close")
        .ok_or_else(|| "No economy block close found.".to_string())?
        .as_str();
    let newline = if game_sii_content.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    let mod_line_regex = Regex::new(r#"(?m)^([ \t]*)mod_activated\[\d+\]:[^\r\n]*(?:\r?\n)?"#)
        .map_err(|error| error.to_string())?;

    let first_mod_line = mod_line_regex.find(body);
    let removed_existing_mod_count = mod_line_regex.find_iter(body).count();
    println!(
        "[mod-profile-manager] replace_active_workshop_mods removed_existing_mod_count={removed_existing_mod_count}"
    );
    let indent = mod_line_regex
        .captures(body)
        .and_then(|captures| captures.get(1))
        .map(|match_| match_.as_str().to_string())
        .unwrap_or_else(|| infer_body_indent(body));
    let new_block = workshop_mod_ids
        .iter()
        .enumerate()
        .map(|(index, id)| {
            workshop_id_to_sii_mod_ref(id)
                .map(|mod_ref| format!("{indent}mod_activated[{index}]: \"{mod_ref}\""))
        })
        .collect::<Result<Vec<_>, _>>()?
        .join(newline);

    let new_body = if let Some(first_mod_line) = first_mod_line {
        let before = &body[..first_mod_line.start()];
        let after = mod_line_regex.replace_all(&body[first_mod_line.start()..], "");
        if new_block.is_empty() {
            format!("{before}{after}")
        } else if after.is_empty() {
            format!("{before}{new_block}")
        } else {
            format!("{before}{new_block}{newline}{after}")
        }
    } else if new_block.is_empty() {
        body.to_string()
    } else if body.trim().is_empty() {
        new_block
    } else if body.ends_with('\n') {
        format!("{body}{new_block}")
    } else {
        format!("{body}{newline}{new_block}")
    };

    let replacement = format!("{header}{new_body}{close}");
    let mut output = game_sii_content.to_string();
    output.replace_range(full_match.start()..full_match.end(), &replacement);
    Ok((output, removed_existing_mod_count))
}

pub fn workshop_id_to_sii_mod_ref(id: &str) -> Result<String, String> {
    let value = id.trim();
    if value.is_empty() {
        return Err("Workshop ID is required.".to_string());
    }
    if !value.chars().all(|character| character.is_ascii_digit()) {
        return Err(format!("Invalid Workshop ID: {value}"));
    }

    let id = value
        .parse::<u64>()
        .map_err(|error| format!("Invalid Workshop ID {value}: {error}"))?;
    Ok(format!("mod_workshop_package.{id:08x}"))
}

pub fn read_active_mods(path: &Path) -> Result<Vec<u64>, String> {
    let text = fs::read_to_string(path)
        .map_err(|error| format!("Failed to read {}: {}", path.display(), error))?;
    read_active_mods_from_text(&text)
}

pub fn overwrite_active_mods(path: &Path, mod_ids: &[u64]) -> Result<PathBuf, String> {
    let workshop_mod_ids = mod_ids.iter().map(u64::to_string).collect::<Vec<_>>();
    overwrite_active_workshop_mods(path, &workshop_mod_ids).map(|(backup_path, _)| backup_path)
}

pub fn overwrite_active_workshop_mods(
    path: &Path,
    workshop_mod_ids: &[String],
) -> Result<(PathBuf, usize), String> {
    if !path.is_file() {
        return Err(format!("game.sii not found: {}", path.display()));
    }

    let text = fs::read_to_string(path)
        .map_err(|error| format!("Failed to read {}: {}", path.display(), error))?;
    let (output, removed_existing_mod_count) =
        replace_active_workshop_mods_in_game_sii(&text, workshop_mod_ids)?;
    let backup_path = backup_game_sii(path)?;
    println!(
        "[mod-profile-manager] overwrite_active_workshop_mods backup_path={} new_mod_count={}",
        backup_path.display(),
        workshop_mod_ids.len()
    );
    fs::write(path, output)
        .map_err(|error| format!("Failed to write {}: {}", path.display(), error))?;
    Ok((backup_path, removed_existing_mod_count))
}

fn economy_block_body(text: &str) -> Result<&str, String> {
    let regex = Regex::new(r"(?s)economy\s*:\s*[^{]+\{\r?\n(?P<body>.*?)(?:\r?\n\})")
        .map_err(|error| error.to_string())?;

    regex
        .captures(text)
        .and_then(|captures| captures.name("body"))
        .map(|body| body.as_str())
        .ok_or_else(|| "No economy block found in game.sii.".to_string())
}

fn infer_body_indent(body: &str) -> String {
    for line in body.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let indent = line
            .chars()
            .take_while(|character| matches!(character, ' ' | '\t'))
            .collect::<String>();
        if !indent.is_empty() {
            return indent;
        }
    }
    "    ".to_string()
}

fn looks_like_decoded_sii(text: &str) -> bool {
    text.contains("SiiNunit") && text.contains("economy")
}

fn backup_game_sii(path: &Path) -> Result<PathBuf, String> {
    let timestamp = Local::now().format("%Y%m%d_%H%M%S");
    let backup_path = path.with_file_name(format!("game.sii.bak_{timestamp}"));
    fs::copy(path, &backup_path).map_err(|error| {
        format!(
            "Failed to create backup {}: {}",
            backup_path.display(),
            error
        )
    })?;
    Ok(backup_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_active_mods_from_sii_text() {
        let mods = read_active_mods_from_text(test_sii_text()).unwrap();
        assert_eq!(mods, vec![111, 222]);
    }

    #[test]
    fn replaces_old_mods_with_new_mods() {
        let output = overwrite_active_mods_in_text(test_sii_text(), &[3710074411]).unwrap();

        assert!(output.contains(r#"mod_activated[0]: "mod_workshop_package.dd233e2b""#));
        assert!(!output.contains(r#"mod/111.scs"#));
        assert!(!output.contains(r#"mod/222.scs"#));
    }

    #[test]
    fn converts_workshop_id_to_sii_mod_ref() {
        assert_eq!(
            workshop_id_to_sii_mod_ref("123").unwrap(),
            "mod_workshop_package.0000007b"
        );
        assert_eq!(
            workshop_id_to_sii_mod_ref("456").unwrap(),
            "mod_workshop_package.000001c8"
        );
    }

    #[test]
    fn replaces_all_mod_activated_lines_and_returns_removed_count() {
        let (output, removed_count) =
            replace_active_workshop_mods_in_game_sii(test_sii_text(), &["123".to_string()])
                .unwrap();

        assert_eq!(removed_count, 2);
        assert!(output.contains(r#"mod_activated[0]: "mod_workshop_package.0000007b""#));
        assert!(!output.contains("mod_activated[1]"));
    }

    fn test_sii_text() -> &'static str {
        r#"SiiNunit
{
economy : _nameless.123 {
    bank: 1000
    mod_activated[0]: "mod/111.scs"
    mod_activated[1]: "mod/222.scs"
}
}
"#
    }
}
