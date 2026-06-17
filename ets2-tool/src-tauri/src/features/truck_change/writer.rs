use std::fs;
use std::path::{Path, PathBuf};

use crate::features::ets2save::errors::{AppError, AppErrorCode};
use crate::features::ets2save::sii_codec::replace_file_atomic;

use super::parser::{extract_field_value, parse_unit_blocks};

pub fn set_unit_field_value(
    content: &str,
    unit_id: &str,
    field: &str,
    value: &str,
) -> Result<(String, bool), String> {
    let blocks = parse_unit_blocks(content);
    let block = blocks
        .iter()
        .find(|block| block.id.eq_ignore_ascii_case(unit_id))
        .ok_or_else(|| format!("unit_not_found:{}", unit_id))?;
    let mut lines = content
        .lines()
        .map(|line| line.to_string())
        .collect::<Vec<_>>();
    let prefix = format!("{}:", field);

    for index in block.start_line..=block.end_line {
        let Some(line) = lines.get(index) else {
            continue;
        };
        let trimmed = line.trim_start();
        if !trimmed.starts_with(&prefix) {
            continue;
        }
        let indent_len = line.len() - trimmed.len();
        let indent = &line[..indent_len];
        lines[index] = format!("{}{}: {}", indent, field, value);
        return Ok((join_content_lines(lines, content.ends_with('\n')), true));
    }

    Ok((content.to_string(), false))
}

pub fn unit_field_exists(content: &str, unit_id: &str, field: &str) -> bool {
    parse_unit_blocks(content)
        .into_iter()
        .find(|block| block.id.eq_ignore_ascii_case(unit_id))
        .and_then(|block| extract_field_value(&block.raw_block, field))
        .is_some()
}

pub fn write_verified_content(
    target_path: &Path,
    content: &str,
    backup_id: &str,
    verify_temp: impl Fn(&str) -> Result<(), String>,
) -> Result<(), String> {
    if backup_id.trim().is_empty() {
        return Err("backup_required_before_write".to_string());
    }

    verify_temp(content)?;

    let tmp_path = temp_path_for(target_path);
    fs::write(&tmp_path, content)
        .map_err(|error| format!("temporary_write_failed:{}:{}", target_path.display(), error))?;

    let temp_content = fs::read_to_string(&tmp_path)
        .map_err(|error| format!("temporary_readback_failed:{}:{}", tmp_path.display(), error))?;
    if let Err(error) = verify_temp(&temp_content) {
        let _ = fs::remove_file(&tmp_path);
        return Err(error);
    }

    replace_file_atomic(&tmp_path, target_path).map_err(format_app_error)
}

fn temp_path_for(target_path: &Path) -> PathBuf {
    let file_name = target_path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("game.sii");
    target_path.with_file_name(format!("{}.truck_change.tmp", file_name))
}

fn join_content_lines(lines: Vec<String>, trailing_newline: bool) -> String {
    let mut content = lines.join("\n");
    if trailing_newline {
        content.push('\n');
    }
    content
}

fn format_app_error(error: AppError) -> String {
    match error.code {
        AppErrorCode::WriteFailed => format!("atomic_replace_failed:{}", error.message),
        AppErrorCode::DecodeFailed => format!("decode_failed:{}", error.message),
        AppErrorCode::BackupFailed => format!("backup_failed:{}", error.message),
        _ => format!(
            "write_pipeline_failed:{}:{}",
            error.code.as_key(),
            error.message
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::{set_unit_field_value, write_verified_content};

    #[test]
    fn set_unit_field_value_updates_only_target_unit() {
        let content = r#"SiiNunit
{
player : _nameless.player {
 my_truck: _nameless.truck.a
}
player_job : _nameless.job {
 my_truck: _nameless.truck.old
}
}
"#;
        let (updated, changed) =
            set_unit_field_value(content, "_nameless.player", "my_truck", "_nameless.truck.b")
                .unwrap();
        assert!(changed);
        assert!(updated.contains(" my_truck: _nameless.truck.b"));
        assert!(updated.contains(" my_truck: _nameless.truck.old"));
    }

    #[test]
    fn write_requires_backup_id() {
        let path =
            std::env::temp_dir().join(format!("truck_change_no_backup_{}.sii", std::process::id()));
        let result = write_verified_content(&path, "SiiNunit\n{\n}\n", "", |_| Ok(()));
        assert_eq!(result.unwrap_err(), "backup_required_before_write");
        assert!(!path.exists());
    }
}
