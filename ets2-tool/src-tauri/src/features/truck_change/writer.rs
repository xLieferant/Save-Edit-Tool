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
    let document = crate::shared::sii_document::Document::parse(content)?;
    let unit = document
        .units
        .iter()
        .find(|unit| unit.id.eq_ignore_ascii_case(unit_id))
        .ok_or_else(|| format!("unit_not_found:{unit_id}"))?;
    match document.patch(unit, field, value) {
        Ok(updated) => Ok((updated, true)),
        Err(error) if error == format!("sii_field_missing:{field}") => {
            Ok((content.to_string(), false))
        }
        Err(error) => Err(error),
    }
}

pub fn set_unit_array_value(
    content: &str,
    unit_id: &str,
    field: &str,
    array_index: usize,
    value: &str,
) -> Result<(String, bool), String> {
    set_unit_field_value(content, unit_id, &format!("{field}[{array_index}]"), value)
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
    verify_temp: impl Fn(&str) -> Result<(), String>,
) -> Result<(), String> {
    verify_temp(content)?;
    let before = crate::features::save_editor::safety::read_stable_bytes(target_path)?;
    write_verified_content_if_current(target_path, &before, content, verify_temp)
}

pub(crate) fn write_verified_content_if_current(
    target_path: &Path,
    before: &[u8],
    content: &str,
    verify_temp: impl Fn(&str) -> Result<(), String>,
) -> Result<(), String> {
    verify_temp(content)?;
    crate::features::save_editor::safety::write_atomic_checked(
        target_path,
        before,
        content,
        |temporary| {
            let candidate =
                fs::read_to_string(temporary).map_err(|_| "save_temporary_read_failed")?;
            verify_temp(&candidate)
        },
    )
}

#[derive(Debug)]
pub struct TemporaryRollbackSnapshot {
    target_path: PathBuf,
    snapshot_path: PathBuf,
    cleaned: bool,
}

impl TemporaryRollbackSnapshot {
    pub fn create(target_path: &Path) -> Result<Self, String> {
        if !target_path.exists() {
            return Err(format!(
                "temporary_rollback_source_missing:{}",
                target_path.display()
            ));
        }
        let snapshot_path = rollback_path_for(target_path);
        fs::copy(target_path, &snapshot_path).map_err(|error| {
            format!(
                "temporary_rollback_create_failed:{}:{}",
                snapshot_path.display(),
                error
            )
        })?;
        crate::dev_log!("[truck_change] temporary rollback snapshot created");
        Ok(Self {
            target_path: target_path.to_path_buf(),
            snapshot_path,
            cleaned: false,
        })
    }

    pub(crate) fn create_if_current(target_path: &Path, expected: &[u8]) -> Result<Self, String> {
        let mut snapshot = Self::create(target_path)?;
        if fs::read(&snapshot.snapshot_path).map_err(|_| "temporary_rollback_read_failed")?
            != expected
        {
            snapshot.cleanup()?;
            return Err("save_changed_since_load".into());
        }
        Ok(snapshot)
    }

    pub fn restore(&self) -> Result<(), String> {
        let expected = crate::features::save_editor::safety::read_stable_bytes(&self.target_path)?;
        self.restore_if_current(&expected)
    }

    pub(crate) fn restore_if_current(&self, expected: &[u8]) -> Result<(), String> {
        use std::io::Write;
        let bytes = fs::read(&self.snapshot_path).map_err(|_| "temporary_rollback_read_failed")?;
        let temporary = self
            .target_path
            .with_file_name(format!(".switch-rollback-{}.tmp", uuid::Uuid::new_v4()));
        let mut file = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)
            .map_err(|_| "temporary_rollback_create_failed")?;
        file.write_all(&bytes)
            .map_err(|_| "temporary_rollback_write_failed")?;
        file.sync_all()
            .map_err(|_| "temporary_rollback_sync_failed")?;
        drop(file);
        if let Err(error) = crate::features::save_editor::safety::replace_checked(
            &temporary,
            &self.target_path,
            Some(expected),
        ) {
            let _ = fs::remove_file(&temporary);
            return Err(error);
        }
        if fs::read(&self.target_path).map_err(|_| "temporary_rollback_read_failed")? != bytes {
            return Err("temporary_rollback_verification_failed".into());
        }
        Ok(())
    }

    pub fn cleanup(&mut self) -> Result<(), String> {
        if self.snapshot_path.exists() {
            fs::remove_file(&self.snapshot_path).map_err(|error| {
                format!(
                    "temporary_rollback_cleanup_failed:{}:{}",
                    self.snapshot_path.display(),
                    error
                )
            })?;
        }
        self.cleaned = true;
        Ok(())
    }

    pub fn cleaned(&self) -> bool {
        self.cleaned
    }
}

fn rollback_path_for(target_path: &Path) -> PathBuf {
    let file_name = target_path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("game.sii");
    target_path.with_file_name(format!("{}.truck_change.rollback.tmp", file_name))
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
    use super::{
        TemporaryRollbackSnapshot, set_unit_array_value, set_unit_field_value,
        write_verified_content,
    };

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
    fn write_verified_content_does_not_require_persistent_backup() {
        let path =
            std::env::temp_dir().join(format!("truck_change_no_backup_{}.sii", std::process::id()));
        std::fs::write(&path, "old").unwrap();
        let result = write_verified_content(&path, "SiiNunit\n{\n}\n", |_| Ok(()));
        assert!(result.is_ok(), "{:?}", result);
        assert!(path.exists());
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn temporary_rollback_snapshot_restores_original_file() {
        let path =
            std::env::temp_dir().join(format!("truck_change_rollback_{}.sii", std::process::id()));
        std::fs::write(&path, "before").unwrap();
        let mut snapshot = TemporaryRollbackSnapshot::create(&path).unwrap();
        std::fs::write(&path, "after").unwrap();
        snapshot.restore().unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "before");
        snapshot.cleanup().unwrap();
        assert!(snapshot.cleaned());
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn set_unit_array_value_updates_only_target_slot() {
        let content = r#"SiiNunit
{
garage : garage.berlin {
 drivers: 2
 drivers[0]: null
 drivers[1]: driver.1
}
}
"#;
        let (updated, changed) =
            set_unit_array_value(content, "garage.berlin", "drivers", 0, "driver.1").unwrap();
        assert!(changed);
        assert!(updated.contains(" drivers[0]: driver.1"));
        assert!(updated.contains(" drivers[1]: driver.1"));
    }
}
