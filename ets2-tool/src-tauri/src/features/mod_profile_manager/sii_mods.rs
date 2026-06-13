use super::models::{
    ActivationVerification, ActiveModBlockSnapshot, ReplaceActivePresetModsResult,
    ValidateActivePresetModsResult,
};
use crate::shared::decrypt::decrypt_if_needed;
use chrono::Local;
use regex::Regex;
use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

const ACTIVE_MOD_FIELDS: [&str; 3] = ["active_mods", "actived_mods", "mod_activated"];

#[derive(Debug, Clone)]
struct ParsedActiveModSections {
    field_name: String,
    section_present: bool,
    has_count_line: bool,
    actual_count: usize,
    actual_mod_refs: Vec<String>,
    actual_indices: Vec<usize>,
    other_mod_refs: Vec<String>,
}

pub fn read_active_mods_from_text(text: &str) -> Result<Vec<u64>, String> {
    let body = economy_block_body(text)?;
    let parsed = parse_active_mod_sections(body)?;

    Ok(parsed
        .actual_mod_refs
        .iter()
        .filter_map(|mod_ref| {
            workshop_mod_ref_to_id(mod_ref).or_else(|| legacy_mod_ref_to_id(mod_ref))
        })
        .collect())
}

pub fn overwrite_active_mods_in_text(text: &str, mod_ids: &[u64]) -> Result<String, String> {
    let workshop_mod_ids = mod_ids.iter().map(u64::to_string).collect::<Vec<_>>();
    replace_active_preset_mods_in_game_sii(text, &workshop_mod_ids).map(|result| result.content)
}

pub fn inspect_active_mod_block(path: &Path) -> Result<ActiveModBlockSnapshot, String> {
    let text = decrypt_if_needed(path)?;
    inspect_active_mod_block_from_text(&text)
}

pub fn inspect_active_mod_block_from_text(text: &str) -> Result<ActiveModBlockSnapshot, String> {
    if !looks_like_decoded_sii(text) {
        return Err("game.sii does not look like decoded UTF-8 SII content.".to_string());
    }

    let parsed = parse_active_mod_sections(economy_block_body(text)?)?;
    if !parsed.section_present {
        return Err("No active_mods/actived_mods block found in game.sii.".to_string());
    }

    Ok(ActiveModBlockSnapshot {
        field_name: parsed.field_name,
        count: parsed.actual_count,
        mod_refs: parsed.actual_mod_refs,
        indices: parsed.actual_indices,
    })
}

pub fn replace_active_workshop_mods_in_game_sii(
    game_sii_content: &str,
    workshop_mod_ids: &[String],
) -> Result<(String, usize), String> {
    let result = replace_active_preset_mods_in_game_sii(game_sii_content, workshop_mod_ids)?;
    Ok((result.content, result.removed_mod_count))
}

pub fn replace_active_preset_mods_in_game_sii(
    game_sii_content: &str,
    workshop_mod_ids: &[String],
) -> Result<ReplaceActivePresetModsResult, String> {
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
    let existing = parse_active_mod_sections(body)?;
    if !existing.section_present {
        return Err("No active_mods/actived_mods block found in game.sii.".to_string());
    }
    let workshop_ref_prefix = detect_workshop_ref_prefix(&existing.actual_mod_refs);
    let expected_mod_refs = build_expected_mod_refs(workshop_mod_ids, workshop_ref_prefix)?;
    let preferred_field_name = existing.field_name.clone();
    let count_regex = active_mod_count_regex()?;
    let entry_regex = active_mod_entry_regex()?;
    let mut prefix = String::new();
    let mut suffix = String::new();
    let mut found_active_mod_section = false;
    let mut removed_mod_count = 0usize;
    let mut indent = None::<String>;

    for raw_line in body.split_inclusive('\n') {
        let (line_content, _) = split_line_ending(raw_line);
        let count_match = count_regex.captures(line_content);
        let entry_match = entry_regex.captures(line_content);
        let is_active_mod_line = count_match.is_some() || entry_match.is_some();

        if is_active_mod_line {
            found_active_mod_section = true;
            if indent.is_none() {
                indent = count_match
                    .as_ref()
                    .and_then(|captures| captures.get(1))
                    .or_else(|| entry_match.as_ref().and_then(|captures| captures.get(1)))
                    .map(|match_| match_.as_str().to_string());
            }
            if entry_match.is_some() {
                removed_mod_count += 1;
            }
            continue;
        }

        if found_active_mod_section {
            suffix.push_str(raw_line);
        } else {
            prefix.push_str(raw_line);
        }
    }

    let indent = indent.unwrap_or_else(|| infer_body_indent(body));
    let mut new_block_lines = vec![format!(
        "{indent}{preferred_field_name}: {}",
        expected_mod_refs.len()
    )];
    new_block_lines.extend(
        expected_mod_refs
            .iter()
            .enumerate()
            .map(|(index, mod_ref)| {
                format!("{indent}{preferred_field_name}[{index}]: \"{mod_ref}\"")
            }),
    );
    let new_block = new_block_lines.join(newline);
    let new_body = if found_active_mod_section {
        let mut composed = String::new();
        composed.push_str(&prefix);
        if !composed.is_empty() && !composed.ends_with('\n') {
            composed.push_str(newline);
        }
        composed.push_str(&new_block);
        if !suffix.is_empty() {
            if !composed.ends_with('\n') {
                composed.push_str(newline);
            }
            composed.push_str(&suffix);
        }
        composed
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

    Ok(ReplaceActivePresetModsResult {
        content: output,
        removed_mod_count,
        written_mod_count: expected_mod_refs.len(),
        expected_mod_refs,
    })
}

pub fn validate_active_preset_mods_in_game_sii(
    game_sii_content: &str,
    expected_workshop_mod_ids: &[String],
) -> Result<ValidateActivePresetModsResult, String> {
    if !looks_like_decoded_sii(game_sii_content) {
        return Err("game.sii does not look like decoded UTF-8 SII content.".to_string());
    }

    let parsed = parse_active_mod_sections(economy_block_body(game_sii_content)?)?;
    if !parsed.section_present {
        return Err("No active_mods/actived_mods block found in game.sii.".to_string());
    }
    let workshop_ref_prefix = detect_workshop_ref_prefix(&parsed.actual_mod_refs);
    let expected_mod_refs =
        build_expected_mod_refs(expected_workshop_mod_ids, workshop_ref_prefix)?;
    let order_matches = parsed.actual_indices
        == (0..parsed.actual_mod_refs.len()).collect::<Vec<_>>()
        && expected_mod_refs == parsed.actual_mod_refs;
    let missing_mod_refs = expected_mod_refs
        .iter()
        .filter(|mod_ref| !parsed.actual_mod_refs.contains(mod_ref))
        .cloned()
        .collect::<Vec<_>>();
    let mut unexpected_mod_refs = parsed
        .actual_mod_refs
        .iter()
        .filter(|mod_ref| !expected_mod_refs.contains(mod_ref))
        .cloned()
        .collect::<Vec<_>>();
    unexpected_mod_refs.extend(parsed.other_mod_refs.clone());

    let success = parsed.has_count_line
        && expected_mod_refs.len() == parsed.actual_count
        && expected_mod_refs == parsed.actual_mod_refs
        && order_matches
        && missing_mod_refs.is_empty()
        && unexpected_mod_refs.is_empty();

    Ok(ValidateActivePresetModsResult {
        success,
        expected_count: expected_mod_refs.len(),
        actual_count: parsed.actual_count,
        expected_mod_refs,
        actual_mod_refs: parsed.actual_mod_refs,
        missing_mod_refs,
        unexpected_mod_refs,
        order_matches,
    })
}

pub fn workshop_id_to_sii_mod_ref(id: &str) -> Result<String, String> {
    workshop_id_to_sii_mod_ref_with_prefix(id, "mod_workshop_package.")
}

pub fn workshop_id_to_scs_package_id(workshop_id: &str) -> Result<String, String> {
    let value = workshop_id.trim();
    if value.is_empty() {
        return Err("Workshop ID is required.".to_string());
    }
    let id = value
        .parse::<u64>()
        .map_err(|error| format!("Invalid Workshop ID {value}: {error}"))?;
    Ok(format!("mod_workshop_package.{:016X}", id))
}

pub fn parse_active_mod_values_from_profile_text(
    profile_text: &str,
) -> Result<Vec<String>, String> {
    if !looks_like_decoded_profile_sii(profile_text) {
        return Err("profile.sii does not look like decoded UTF-8 SII content.".to_string());
    }

    let count_regex =
        Regex::new(r#"^\s*active_mods\s*:\s*(\d+)\s*$"#).map_err(|error| error.to_string())?;
    let entry_regex = Regex::new(r#"^\s*active_mods\[(\d+)\]\s*:\s*"(.*)"\s*$"#)
        .map_err(|error| error.to_string())?;
    let mut expected_count = None::<usize>;
    let mut entries = BTreeMap::<usize, String>::new();

    for line in profile_text.lines() {
        if let Some(captures) = count_regex.captures(line) {
            expected_count = captures
                .get(1)
                .and_then(|value| value.as_str().parse::<usize>().ok());
            continue;
        }

        if let Some(captures) = entry_regex.captures(line) {
            let Some(index) = captures
                .get(1)
                .and_then(|value| value.as_str().parse::<usize>().ok())
            else {
                continue;
            };
            let value = captures
                .get(2)
                .map(|value| value.as_str().to_string())
                .unwrap_or_default();
            entries.insert(index, value);
        }
    }

    let count =
        expected_count.ok_or_else(|| "No active_mods block found in profile.sii.".to_string())?;
    let actual_indices = entries.keys().copied().collect::<Vec<_>>();
    let expected_indices = (0..count).collect::<Vec<_>>();
    if actual_indices != expected_indices {
        return Err(format!(
            "active_mods indices mismatch in profile.sii: expected={:?} actual={:?}",
            expected_indices, actual_indices
        ));
    }
    let values = entries.into_values().collect::<Vec<_>>();
    if count != values.len() {
        return Err(format!(
            "active_mods count mismatch in profile.sii: header={} entries={}",
            count,
            values.len()
        ));
    }

    Ok(values)
}

pub fn validate_active_mods_in_profile_text(
    profile_text: &str,
    expected_values: &[String],
) -> Result<ActivationVerification, String> {
    let actual_values = parse_active_mod_values_from_profile_text(profile_text)?;
    Ok(ActivationVerification {
        expected_count: expected_values.len(),
        actual_count: actual_values.len(),
        order_matches: actual_values == expected_values,
        values_match: actual_values == expected_values,
    })
}

pub fn replace_active_mods_block(
    profile_text: &str,
    new_mods: &[String],
) -> Result<String, String> {
    if !looks_like_decoded_profile_sii(profile_text) {
        return Err("profile.sii does not look like decoded UTF-8 SII content.".to_string());
    }

    let newline = if profile_text.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    let had_trailing_newline = profile_text.ends_with('\n');
    let count_regex = Regex::new(r#"^([ \t]*)(active_mods|actived_mods)\s*:\s*\d+\s*$"#)
        .map_err(|error| error.to_string())?;
    let entry_regex = Regex::new(r#"^([ \t]*)(active_mods|actived_mods)\[\d+\]\s*:\s*".*"\s*$"#)
        .map_err(|error| error.to_string())?;

    let mut output = Vec::<String>::new();
    let mut insert_at = None::<usize>;
    let mut indent = None::<String>;

    for raw_line in profile_text.split('\n') {
        let line = raw_line.strip_suffix('\r').unwrap_or(raw_line);
        let count_match = count_regex.captures(line);
        let entry_match = entry_regex.captures(line);
        let is_active_mod_line = count_match.is_some() || entry_match.is_some();

        if is_active_mod_line {
            if insert_at.is_none() {
                insert_at = Some(output.len());
            }
            if indent.is_none() {
                indent = count_match
                    .as_ref()
                    .and_then(|captures| captures.get(1))
                    .or_else(|| entry_match.as_ref().and_then(|captures| captures.get(1)))
                    .map(|value| value.as_str().to_string());
            }
            continue;
        }

        output.push(line.to_string());
    }

    if had_trailing_newline && output.last().map(|line| line.is_empty()).unwrap_or(false) {
        output.pop();
    }

    let insert_at = match insert_at {
        Some(index) => index,
        None => output
            .iter()
            .rposition(|line| line.trim() == "}")
            .ok_or_else(|| {
                "No active_mods block found and no safe insertion point exists in profile.sii."
                    .to_string()
            })?,
    };
    let indent = indent.unwrap_or_else(|| infer_profile_body_indent(&output, insert_at));

    let mut block = Vec::with_capacity(new_mods.len() + 1);
    block.push(format!("{indent}active_mods: {}", new_mods.len()));
    for (index, value) in new_mods.iter().enumerate() {
        block.push(format!("{indent}active_mods[{index}]: \"{}\"", value));
    }

    output.splice(insert_at..insert_at, block);
    let mut result = output.join(newline);
    if had_trailing_newline {
        result.push_str(newline);
    }
    Ok(result)
}

pub fn write_text_flush_sync(path: &Path, content: &str) -> Result<(), String> {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(path)
        .map_err(|error| format!("Failed to open {} for writing: {}", path.display(), error))?;
    file.write_all(content.as_bytes())
        .map_err(|error| format!("Failed to write {}: {}", path.display(), error))?;
    file.flush()
        .map_err(|error| format!("Failed to flush {}: {}", path.display(), error))?;
    file.sync_all()
        .map_err(|error| format!("Failed to sync {}: {}", path.display(), error))
}

pub fn workshop_id_to_sii_mod_ref_with_prefix(id: &str, prefix: &str) -> Result<String, String> {
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
    Ok(format!("{prefix}{id:08x}"))
}

pub fn read_active_mods(path: &Path) -> Result<Vec<u64>, String> {
    let text = decrypt_if_needed(path)?;
    read_active_mods_from_text(&text)
}

pub fn overwrite_active_mods(path: &Path, mod_ids: &[u64]) -> Result<PathBuf, String> {
    let workshop_mod_ids = mod_ids.iter().map(u64::to_string).collect::<Vec<_>>();
    overwrite_active_preset_mods(path, &workshop_mod_ids).map(|(backup_path, _)| backup_path)
}

pub fn overwrite_active_workshop_mods(
    path: &Path,
    workshop_mod_ids: &[String],
) -> Result<(PathBuf, usize), String> {
    let (backup_path, result) = overwrite_active_preset_mods(path, workshop_mod_ids)?;
    Ok((backup_path, result.removed_mod_count))
}

pub fn overwrite_active_preset_mods(
    path: &Path,
    workshop_mod_ids: &[String],
) -> Result<(PathBuf, ReplaceActivePresetModsResult), String> {
    if !path.is_file() {
        return Err(format!("game.sii not found: {}", path.display()));
    }

    let text = decrypt_if_needed(path)?;
    let result = replace_active_preset_mods_in_game_sii(&text, workshop_mod_ids)?;
    let backup_path = backup_game_sii(path)?;
    println!(
        "[mod-profile-manager] overwrite_active_preset_mods backup_path={} removed_mod_count={} written_mod_count={}",
        backup_path.display(),
        result.removed_mod_count,
        result.written_mod_count
    );
    write_text_atomic(path, &result.content)?;
    Ok((backup_path, result))
}

pub fn write_active_preset_mods_atomic(
    path: &Path,
    workshop_mod_ids: &[String],
) -> Result<ReplaceActivePresetModsResult, String> {
    if !path.is_file() {
        return Err(format!("game.sii not found: {}", path.display()));
    }

    let text = decrypt_if_needed(path)?;
    let result = replace_active_preset_mods_in_game_sii(&text, workshop_mod_ids)?;
    write_text_atomic(path, &result.content)?;
    Ok(result)
}

fn build_expected_mod_refs(
    workshop_mod_ids: &[String],
    prefix: &str,
) -> Result<Vec<String>, String> {
    workshop_mod_ids
        .iter()
        .map(|id| workshop_id_to_sii_mod_ref_with_prefix(id, prefix))
        .collect()
}

fn parse_active_mod_sections(body: &str) -> Result<ParsedActiveModSections, String> {
    let count_regex = active_mod_count_regex()?;
    let entry_regex = active_mod_entry_regex()?;
    let mut counts = BTreeMap::<String, usize>::new();
    let mut entries = BTreeMap::<String, Vec<(usize, String)>>::new();

    for line in body.lines() {
        if let Some(captures) = count_regex.captures(line) {
            let field_name = captures
                .get(2)
                .map(|match_| match_.as_str().to_string())
                .unwrap_or_default();
            let count = captures
                .get(3)
                .and_then(|match_| match_.as_str().parse::<usize>().ok())
                .unwrap_or(0);
            counts.insert(field_name, count);
            continue;
        }

        if let Some(captures) = entry_regex.captures(line) {
            let field_name = captures
                .get(2)
                .map(|match_| match_.as_str().to_string())
                .unwrap_or_default();
            let index = captures
                .get(3)
                .and_then(|match_| match_.as_str().parse::<usize>().ok())
                .unwrap_or(0);
            let mod_ref = captures
                .get(4)
                .map(|match_| match_.as_str().to_string())
                .unwrap_or_default();
            entries
                .entry(field_name)
                .or_default()
                .push((index, mod_ref));
        }
    }

    let preferred_field_name = detect_preferred_field_name(body).to_string();
    let mut preferred_entries = entries.remove(&preferred_field_name).unwrap_or_default();
    preferred_entries.sort_by_key(|(index, _)| *index);
    let actual_indices = preferred_entries
        .iter()
        .map(|(index, _)| *index)
        .collect::<Vec<_>>();
    let actual_mod_refs = preferred_entries
        .into_iter()
        .map(|(_, mod_ref)| mod_ref)
        .collect::<Vec<_>>();
    let other_mod_refs = ACTIVE_MOD_FIELDS
        .iter()
        .filter(|field_name| **field_name != preferred_field_name)
        .flat_map(|field_name| {
            let mut field_entries = entries.remove(*field_name).unwrap_or_default();
            field_entries.sort_by_key(|(index, _)| *index);
            field_entries
                .into_iter()
                .map(|(_, mod_ref)| mod_ref)
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let has_count_line = counts.contains_key(&preferred_field_name);
    let section_present = has_count_line
        || !actual_mod_refs.is_empty()
        || ACTIVE_MOD_FIELDS
            .iter()
            .filter(|field_name| **field_name != preferred_field_name)
            .any(|field_name| counts.contains_key(*field_name))
        || !other_mod_refs.is_empty();
    let actual_count = counts
        .get(&preferred_field_name)
        .copied()
        .unwrap_or(actual_mod_refs.len());

    Ok(ParsedActiveModSections {
        field_name: preferred_field_name,
        section_present,
        has_count_line,
        actual_count,
        actual_mod_refs,
        actual_indices,
        other_mod_refs,
    })
}

fn detect_preferred_field_name(body: &str) -> &'static str {
    ACTIVE_MOD_FIELDS
        .iter()
        .find(|field_name| {
            let count_pattern = format!("{field_name}:");
            let entry_pattern = format!("{field_name}[");
            body.contains(&count_pattern) || body.contains(&entry_pattern)
        })
        .copied()
        .unwrap_or("active_mods")
}

fn active_mod_count_regex() -> Result<Regex, String> {
    Regex::new(r#"^([ \t]*)(active_mods|actived_mods|mod_activated):\s*(\d+)\s*$"#)
        .map_err(|error| error.to_string())
}

fn active_mod_entry_regex() -> Result<Regex, String> {
    Regex::new(r#"^([ \t]*)(active_mods|actived_mods|mod_activated)\[(\d+)\]:\s*"([^"]*)"\s*$"#)
        .map_err(|error| error.to_string())
}

fn workshop_mod_ref_to_id(mod_ref: &str) -> Option<u64> {
    mod_ref
        .strip_prefix("mod_workshop_package.")
        .or_else(|| mod_ref.strip_prefix("workshop_package."))
        .and_then(|value| u64::from_str_radix(value, 16).ok())
}

fn legacy_mod_ref_to_id(mod_ref: &str) -> Option<u64> {
    mod_ref
        .strip_prefix("mod/")
        .and_then(|value| value.strip_suffix(".scs"))
        .and_then(|value| value.parse::<u64>().ok())
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

fn looks_like_decoded_profile_sii(text: &str) -> bool {
    text.contains("SiiNunit") && text.contains("profile")
}

fn infer_profile_body_indent(lines: &[String], insert_at: usize) -> String {
    for line in lines[..insert_at.min(lines.len())].iter().rev() {
        if line.trim().is_empty() || line.trim() == "{" || line.trim() == "}" {
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

    " ".repeat(4)
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

fn detect_workshop_ref_prefix(mod_refs: &[String]) -> &'static str {
    for mod_ref in mod_refs {
        if mod_ref.starts_with("workshop_package.") {
            return "workshop_package.";
        }
        if mod_ref.starts_with("mod_workshop_package.") {
            return "mod_workshop_package.";
        }
    }

    "mod_workshop_package."
}

fn write_text_atomic(path: &Path, content: &str) -> Result<(), String> {
    let tmp_path = path.with_extension("tmp");
    fs::write(&tmp_path, content).map_err(|error| {
        format!(
            "Failed to write temporary file {}: {}",
            tmp_path.display(),
            error
        )
    })?;
    replace_file(&tmp_path, path)
}

#[cfg(target_os = "windows")]
fn replace_file(tmp_path: &Path, target_path: &Path) -> Result<(), String> {
    use windows_sys::Win32::Storage::FileSystem::{
        MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
    };

    fn wide_null(value: &Path) -> Vec<u16> {
        value
            .as_os_str()
            .to_string_lossy()
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect()
    }

    let source = wide_null(tmp_path);
    let target = wide_null(target_path);
    let moved = unsafe {
        MoveFileExW(
            source.as_ptr(),
            target.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };

    if moved == 0 {
        return Err(format!(
            "Atomic replace failed for {}",
            target_path.display()
        ));
    }

    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn replace_file(tmp_path: &Path, target_path: &Path) -> Result<(), String> {
    if target_path.exists() {
        fs::remove_file(target_path)
            .map_err(|error| format!("Failed to replace {}: {}", target_path.display(), error))?;
    }

    fs::rename(tmp_path, target_path).map_err(|error| {
        format!(
            "Atomic rename failed for {}: {}",
            target_path.display(),
            error
        )
    })
}

fn split_line_ending(line: &str) -> (&str, &str) {
    if let Some(content) = line.strip_suffix("\r\n") {
        return (content, "\r\n");
    }
    if let Some(content) = line.strip_suffix('\n') {
        return (content, "\n");
    }
    (line, "")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_active_mods_from_active_mods_text() {
        let mods = read_active_mods_from_text(test_active_mods_sii_text()).unwrap();
        assert_eq!(mods, vec![3710074411, 456]);
    }

    #[test]
    fn replaces_active_mods_block_and_count() {
        let result = replace_active_preset_mods_in_game_sii(
            test_active_mods_sii_text(),
            &["3710074411".to_string()],
        )
        .unwrap();

        assert_eq!(result.removed_mod_count, 2);
        assert_eq!(result.written_mod_count, 1);
        assert_eq!(
            result.expected_mod_refs,
            vec!["mod_workshop_package.dd233e2b".to_string()]
        );
        assert!(result.content.contains(r#"active_mods: 1"#));
        assert!(result
            .content
            .contains(r#"active_mods[0]: "mod_workshop_package.dd233e2b""#));
        assert!(!result.content.contains(r#"active_mods[1]"#));
    }

    #[test]
    fn preserves_actived_mods_field_name_when_present() {
        let result = replace_active_preset_mods_in_game_sii(
            test_actived_mods_sii_text(),
            &["456".to_string()],
        )
        .unwrap();

        assert!(result.content.contains(r#"actived_mods: 1"#));
        assert!(result
            .content
            .contains(r#"actived_mods[0]: "mod_workshop_package.000001c8""#));
        assert!(!result.content.contains("active_mods[0]"));
    }

    #[test]
    fn validates_expected_active_mods_successfully() {
        let validation = validate_active_preset_mods_in_game_sii(
            test_single_mod_active_mods_sii_text(),
            &["3710074411".to_string()],
        )
        .unwrap();

        assert!(validation.success);
        assert_eq!(validation.expected_count, 1);
        assert_eq!(validation.actual_count, 1);
        assert_eq!(
            validation.actual_mod_refs,
            vec!["mod_workshop_package.dd233e2b".to_string()]
        );
        assert!(validation.missing_mod_refs.is_empty());
        assert!(validation.unexpected_mod_refs.is_empty());
        assert!(validation.order_matches);
    }

    #[test]
    fn validation_fails_when_order_is_wrong() {
        let validation = validate_active_preset_mods_in_game_sii(
            test_reversed_active_mods_sii_text(),
            &["3710074411".to_string(), "456".to_string()],
        )
        .unwrap();

        assert!(!validation.success);
        assert!(!validation.order_matches);
    }

    #[test]
    fn converts_workshop_id_to_sii_mod_ref() {
        assert_eq!(
            workshop_id_to_sii_mod_ref("3710074411").unwrap(),
            "mod_workshop_package.dd233e2b"
        );
        assert_eq!(
            workshop_id_to_sii_mod_ref("456").unwrap(),
            "mod_workshop_package.000001c8"
        );
    }

    #[test]
    fn converts_workshop_id_to_scs_package_id() {
        assert_eq!(
            workshop_id_to_scs_package_id("3710074411").unwrap(),
            "mod_workshop_package.00000000DD233E2B"
        );
    }

    #[test]
    fn replaces_profile_active_mods_and_removes_legacy_actived_mods() {
        let result = replace_active_mods_block(
            test_profile_actived_mods_sii_text(),
            &["mod_workshop_package.00000000DD233E2B|Test".to_string()],
        )
        .unwrap();

        assert!(result.contains("active_mods: 1"));
        assert!(result.contains(r#"active_mods[0]: "mod_workshop_package.00000000DD233E2B|Test""#));
        assert!(!result.contains("actived_mods"));
    }

    #[test]
    fn validates_profile_active_mods_order_and_values() {
        let expected = vec!["mod_workshop_package.00000000DD233E2B|Test".to_string()];
        let validation =
            validate_active_mods_in_profile_text(test_profile_active_mods_sii_text(), &expected)
                .unwrap();

        assert_eq!(validation.expected_count, 1);
        assert_eq!(validation.actual_count, 1);
        assert!(validation.order_matches);
        assert!(validation.values_match);
    }

    fn test_active_mods_sii_text() -> &'static str {
        r#"SiiNunit
{
economy : _nameless.123 {
    bank: 1000
    active_mods: 2
    active_mods[0]: "mod_workshop_package.dd233e2b"
    active_mods[1]: "mod_workshop_package.000001c8"
}
}
"#
    }

    fn test_actived_mods_sii_text() -> &'static str {
        r#"SiiNunit
{
economy : _nameless.123 {
    bank: 1000
    actived_mods: 2
    actived_mods[0]: "mod_workshop_package.dd233e2b"
    actived_mods[1]: "mod_workshop_package.000001c8"
}
}
"#
    }

    fn test_single_mod_active_mods_sii_text() -> &'static str {
        r#"SiiNunit
{
economy : _nameless.123 {
    active_mods: 1
    active_mods[0]: "mod_workshop_package.dd233e2b"
}
}
"#
    }

    fn test_reversed_active_mods_sii_text() -> &'static str {
        r#"SiiNunit
{
economy : _nameless.123 {
    active_mods: 2
    active_mods[0]: "mod_workshop_package.000001c8"
    active_mods[1]: "mod_workshop_package.dd233e2b"
}
}
"#
    }

    fn test_profile_actived_mods_sii_text() -> &'static str {
        r#"SiiNunit
{
profile : _nameless.profile {
    profile_name: "Test"
    actived_mods: 2
    actived_mods[0]: "mod_workshop_package.dd233e2b"
    actived_mods[1]: "mod_workshop_package.000001c8"
    cached_discovery: true
}
}
"#
    }

    fn test_profile_active_mods_sii_text() -> &'static str {
        r#"SiiNunit
{
profile : _nameless.profile {
    profile_name: "Test"
    active_mods: 1
    active_mods[0]: "mod_workshop_package.00000000DD233E2B|Test"
}
}
"#
    }
}
