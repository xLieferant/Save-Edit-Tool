use std::fs;
use std::path::{Path, PathBuf};

use chrono::Utc;

use crate::features::ets2save::errors::{AppError, AppErrorCode};
use crate::shared::decrypt::decrypt_if_needed;

pub fn decode_sii_lines(path: &Path) -> Result<Vec<String>, AppError> {
    let content = decrypt_if_needed(path).map_err(|error| {
        AppError::new(
            AppErrorCode::DecodeFailed,
            format!("Failed to decode {}: {}", path.display(), error),
        )
    })?;
    Ok(split_lines(&content))
}

pub fn split_lines(content: &str) -> Vec<String> {
    content
        .replace("\r\n", "\n")
        .lines()
        .map(|line| line.to_string())
        .collect()
}

pub fn join_lines(lines: &[String]) -> String {
    let mut output = lines.join("\n");
    if !output.ends_with('\n') {
        output.push('\n');
    }
    output
}

pub fn backup_path_for(path: &Path) -> PathBuf {
    let timestamp = Utc::now().format("%Y%m%d%H%M%S");
    let filename = format!("game_bak_{}.sii", timestamp);
    path.parent()
        .unwrap_or_else(|| Path::new("."))
        .join(filename)
}

pub fn write_lines_atomic(path: &Path, lines: &[String]) -> Result<PathBuf, AppError> {
    let backup_path = backup_path_for(path);
    fs::copy(path, &backup_path).map_err(|error| {
        AppError::new(
            AppErrorCode::BackupFailed,
            format!("Backup failed for {}: {}", path.display(), error),
        )
    })?;

    let tmp_path = path.with_extension("tmp");
    fs::write(&tmp_path, join_lines(lines)).map_err(|error| {
        AppError::new(
            AppErrorCode::WriteFailed,
            format!("Temporary write failed for {}: {}", path.display(), error),
        )
    })?;

    replace_file(&tmp_path, path)?;
    Ok(backup_path)
}

#[cfg(target_os = "windows")]
fn replace_file(tmp_path: &Path, target_path: &Path) -> Result<(), AppError> {
    use windows_sys::Win32::Storage::FileSystem::{
        MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
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
        return Err(AppError::new(
            AppErrorCode::WriteFailed,
            format!("Atomic replace failed for {}", target_path.display()),
        ));
    }

    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn replace_file(tmp_path: &Path, target_path: &Path) -> Result<(), AppError> {
    if target_path.exists() {
        fs::remove_file(target_path).map_err(|error| {
            AppError::new(
                AppErrorCode::WriteFailed,
                format!("Failed to replace {}: {}", target_path.display(), error),
            )
        })?;
    }

    fs::rename(tmp_path, target_path).map_err(|error| {
        AppError::new(
            AppErrorCode::WriteFailed,
            format!(
                "Atomic rename failed for {}: {}",
                target_path.display(),
                error
            ),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::{join_lines, split_lines};

    #[test]
    fn sii_codec_roundtrip() {
        let fixture = "SiiNunit\n{\ncompany : company.volatile.test.city {\n job_offer: 1\n}\n}\n";
        let lines = split_lines(fixture);
        let roundtrip = join_lines(&lines);
        assert_eq!(roundtrip, fixture);
    }
}
