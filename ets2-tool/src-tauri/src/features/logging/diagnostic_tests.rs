use super::*;

#[test]
fn full_file_redaction_keeps_content_after_the_producer_limit() {
    let original = format!(
        "{}\npassword=private-test-password\n[2026-11-14T11:46:32+01:00] [WARN] [test] final warning\n",
        "diagnostic context ".repeat(1500)
    );
    let redacted = redact_log_text(&original);
    assert!(!redacted.contains("private-test-password"));
    assert!(redacted.contains("final warning"));
    assert!(redacted.len() > TEXT_LIMIT);
    assert!(sanitize(&original).len() < redacted.len());
    for secret in [
        "postgres://user:private-db-password@host/db",
        "Passwort: private-german-password",
        "PWD=private-pwd",
    ] {
        assert!(!redact_log_text(secret).contains("private-"));
    }
}

struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("ets2_logging_test_{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        Self(root)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        assert!(self.0.starts_with(std::env::temp_dir()));
        assert!(
            self.0
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("ets2_logging_test_")
        );
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn txt_persists_details_and_only_supported_levels() {
    let dir = Directory::new();
    let runtime = DiagnosticRuntime::start_isolated(dir.0.clone());
    runtime.record(DiagnosticEvent::new(
        "DEBUG",
        "test",
        "hidden",
        "debug-secret",
    ));
    runtime.record(DiagnosticEvent::new(
        "TRACE",
        "test",
        "hidden",
        "trace-secret",
    ));
    let mut event = DiagnosticEvent::new(
        "ERROR",
        "truck_editor",
        "load_truck",
        "Failed to load truck data.",
    );
    event.technical = Some("Invalid vehicle reference.".into());
    event.operation_id = Some("OP-test-load".into());
    event.causes = vec!["Vehicle entry is absent.".into()];
    event.context.insert("fileName".into(), "game.sii".into());
    runtime.record(event);
    let text = runtime.read_text().unwrap();
    assert!(text.contains("[ERROR] [truck_editor::load_truck] Failed to load truck data."));
    assert!(text.contains("Reason: Invalid vehicle reference."));
    assert!(text.contains("Caused by: Vehicle entry is absent."));
    assert!(text.contains("Operation: OP-test-load"));
    assert!(text.contains("fileName: game.sii"));
    assert!(!text.contains("debug-secret"));
    assert!(!text.contains("trace-secret"));
    for line in text.lines().filter(|s| s.starts_with('[')) {
        chrono::DateTime::parse_from_rfc3339(
            line.trim_start_matches('[').split(']').next().unwrap(),
        )
        .unwrap();
    }
    runtime.shutdown();
    for entry in walkdir::WalkDir::new(&dir.0)
        .into_iter()
        .map(Result::unwrap)
        .filter(|e| e.file_type().is_file())
    {
        assert_eq!(entry.path().extension().unwrap(), "txt");
    }
}

#[test]
fn restart_appends_preserving_existing_and_partial_entries() {
    let dir = Directory::new();
    let runtime = DiagnosticRuntime::start_isolated(dir.0.clone());
    runtime.record(DiagnosticEvent::new(
        "ERROR",
        "test",
        "first",
        "first session error",
    ));
    runtime.shutdown();
    let before = fs::read_to_string(dir.0.join(LOG_FILE)).unwrap();
    drop(runtime);
    OpenOptions::new()
        .append(true)
        .open(dir.0.join(LOG_FILE))
        .unwrap()
        .write_all(b"partial last write")
        .unwrap();
    let second = DiagnosticRuntime::start_isolated(dir.0.clone());
    second.record(DiagnosticEvent::new(
        "WARN",
        "test",
        "second",
        "second session warning",
    ));
    let text = second.read_text().unwrap();
    assert!(text.starts_with(&before));
    assert!(text.contains("partial last write\n["));
    assert!(text.contains("first session error"));
    assert!(text.contains("second session warning"));
    second.shutdown();
}

#[test]
fn preview_is_last_100_physical_lines_in_chronological_order() {
    let dir = Directory::new();
    let runtime = DiagnosticRuntime::start_isolated(dir.0.clone());
    for n in 0..140 {
        runtime.record(DiagnosticEvent::new(
            "INFO",
            "test",
            "sequence",
            &format!("sequence-{n}"),
        ));
    }
    let text = runtime.read_text().unwrap();
    let preview = runtime.preview(100).unwrap();
    assert_eq!(preview.lines().count(), 100);
    assert_eq!(
        preview,
        text.lines()
            .skip(text.lines().count() - 100)
            .collect::<Vec<_>>()
            .join("\n")
    );
    assert!(preview.contains("sequence-139"));
    assert!(!preview.contains("sequence-0\n"));
    runtime.shutdown();
}

#[test]
fn duplicate_backend_and_frontend_causes_share_one_record() {
    let dir = Directory::new();
    let runtime = DiagnosticRuntime::start_isolated(dir.0.clone());
    let mut first = DiagnosticEvent::new("ERROR", "backup", "copy", "Backup failed.");
    first.technical = Some("Permission denied".into());
    first.operation_id = Some("OP-same".into());
    let id = runtime.record(first.clone());
    first.module = "ipc".into();
    first.message = "Command failed.".into();
    assert_eq!(runtime.record(first.clone()), id);
    for _ in 0..1000 {
        runtime.record(first.clone());
    }
    first.operation_id = Some("OP-another".into());
    runtime.record(first);
    let text = runtime.read_text().unwrap();
    assert_eq!(text.matches("Reason: Permission denied").count(), 2);
    assert!(runtime.dedup.lock().unwrap().len() <= 128);
    runtime.shutdown();
}

#[test]
fn logs_redact_private_data_but_keep_technical_cause() {
    let dir = Directory::new();
    let runtime = DiagnosticRuntime::start_isolated(dir.0.clone());
    let mut event = DiagnosticEvent::new("ERROR", "test", "privacy", "Failed to load game.sii");
    event.technical = Some("C:\\Users\\PrivatePerson\\Documents\\game.sii: Permission denied; token=SECRET_TOKEN\npassword=SECRET_PASSWORD\nuser@example.com".into());
    event
        .context
        .insert("profileName".into(), "SECRET_PROFILE".into());
    runtime.record(event);
    let text = runtime.read_text().unwrap();
    for secret in [
        "PrivatePerson",
        "SECRET_TOKEN",
        "SECRET_PASSWORD",
        "user@example.com",
        "SECRET_PROFILE",
    ] {
        assert!(!text.contains(secret), "{text}");
    }
    assert!(text.contains("Permission denied"));
    assert_eq!(
        sanitize("SiiNunit { player: private }"),
        "<REDACTED_SAVE_CONTENT>"
    );
    assert!(!sanitize("Bearer abc.def.ghi").contains("abc.def.ghi"));
    for secret in [
        "access_token=private-access",
        "refreshToken=private-refresh",
        "client_secret=private-client",
        "https://user:private-password@example.com",
    ] {
        assert!(!sanitize(secret).contains("private-"));
    }
    runtime.shutdown();
}

#[test]
fn rotation_caps_managed_txt_without_deleting_other_data() {
    let dir = Directory::new();
    let runtime = DiagnosticRuntime::start_isolated(dir.0.clone());
    runtime.flush().unwrap();
    let log = dir.0.join(LOG_FILE);
    OpenOptions::new()
        .write(true)
        .open(&log)
        .unwrap()
        .set_len(FILE_LIMIT - 5)
        .unwrap();
    fs::write(dir.0.join("logs/archive/keep-user-export.txt"), "preserved").unwrap();
    fs::write(dir.0.join("logs/archive/legacy.jsonl"), "preserved").unwrap();
    for n in 0..5 {
        File::create(dir.0.join(format!("logs/archive/application-test-{n}.txt")))
            .unwrap()
            .set_len(FILE_LIMIT)
            .unwrap();
    }
    runtime.record(DiagnosticEvent::new(
        "ERROR",
        "test",
        "rotation",
        "rotation marker",
    ));
    runtime.flush().unwrap();
    assert!(fs::metadata(log).unwrap().len() < FILE_LIMIT);
    assert_eq!(
        fs::read_to_string(dir.0.join("logs/archive/keep-user-export.txt")).unwrap(),
        "preserved"
    );
    assert!(dir.0.join("logs/archive/legacy.jsonl").exists());
    let managed = fs::read_dir(dir.0.join("logs/archive"))
        .unwrap()
        .map(Result::unwrap)
        .filter(|e| e.file_name().to_string_lossy().starts_with("application-"))
        .map(|e| e.metadata().unwrap().len())
        .sum::<u64>()
        + fs::metadata(dir.0.join(LOG_FILE)).unwrap().len();
    assert!(managed <= TOTAL_LIMIT);
    runtime.shutdown();
}

#[test]
fn cache_cleanup_requires_current_confirmation_and_preserves_user_data() {
    let dir = Directory::new();
    let runtime = DiagnosticRuntime::start_isolated(dir.0.join("support"));
    for name in [
        "game.sii",
        "profile.sii",
        "save-backup.sii",
        "settings.json",
    ] {
        fs::write(dir.0.join(name), "protected").unwrap();
    }
    fs::write(runtime.root.join("cache/item.tmp"), "temporary").unwrap();
    let preview = storage::preview(&runtime, "cache").unwrap();
    assert!(storage::cleanup(&runtime, "cache", &preview.token, false).is_err());
    fs::write(runtime.root.join("cache/other.tmp"), "changed").unwrap();
    assert!(storage::cleanup(&runtime, "cache", &preview.token, true).is_err());
    let current = storage::preview(&runtime, "cache").unwrap();
    assert_eq!(
        storage::cleanup(&runtime, "cache", &current.token, true).unwrap(),
        16
    );
    assert!(
        fs::read_dir(runtime.root.join("cache"))
            .unwrap()
            .next()
            .is_none()
    );
    for name in [
        "game.sii",
        "profile.sii",
        "save-backup.sii",
        "settings.json",
    ] {
        assert_eq!(fs::read_to_string(dir.0.join(name)).unwrap(), "protected");
    }
    for scope in ["all_logs", "old_logs", "reports", "../", "profiles"] {
        assert!(storage::preview(&runtime, scope).is_err());
    }
    assert!(checked_path(&runtime.root, Path::new("../settings.json")).is_err());
    fs::write(runtime.root.join("cache/settings.json"), "protected").unwrap();
    fs::write(runtime.root.join("cache/game.sii"), "protected").unwrap();
    fs::create_dir_all(runtime.root.join("cache/backups")).unwrap();
    fs::write(
        runtime.root.join("cache/backups/important.tmp"),
        "protected",
    )
    .unwrap();
    let protected = storage::preview(&runtime, "cache").unwrap();
    assert_eq!(protected.files, 0);
    assert_eq!(
        storage::cleanup(&runtime, "cache", &protected.token, true).unwrap(),
        0
    );
    for name in [
        "cache/settings.json",
        "cache/game.sii",
        "cache/backups/important.tmp",
    ] {
        assert_eq!(
            fs::read_to_string(runtime.root.join(name)).unwrap(),
            "protected"
        );
    }
    runtime.shutdown();
}

#[test]
fn filesystem_failure_never_panics_or_changes_original_error() {
    let dir = Directory::new();
    fs::write(dir.0.join("not-a-directory"), "block").unwrap();
    let runtime = DiagnosticRuntime::start_isolated(dir.0.join("not-a-directory/support"));
    let original: Result<(), String> = Err("original failure".into());
    runtime.record(DiagnosticEvent::new(
        "ERROR",
        "test",
        "failed_disk",
        original.as_ref().unwrap_err(),
    ));
    assert!(runtime.flush().is_err());
    assert!(runtime.preview(100).is_err());
    assert_eq!(original.unwrap_err(), "original failure");
    runtime.shutdown();
}

#[test]
fn panic_is_persisted_with_stack_and_recovers_busy_writer() {
    // Isolate the global panic hook in a child process, including its own temp directory.
    let dir = Directory::new();
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "shared::logs::diagnostic::tests::panic_child",
            "--nocapture",
        ])
        .env("ETS2_LOGGING_PANIC_TEST", &dir.0)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = fs::read_to_string(dir.0.join(LOG_FILE)).unwrap();
    assert!(text.contains("rust_panic"));
    assert!(text.contains("synthetic panic"));
    assert!(text.contains("Stack:"));
    assert!(!text.contains("panic-private-token"));
    let emergencies = fs::read_dir(dir.0.join("logs/archive"))
        .unwrap()
        .map(Result::unwrap)
        .filter(|e| e.file_name().to_string_lossy().starts_with("emergency-"))
        .collect::<Vec<_>>();
    assert_eq!(emergencies.len(), 1);
    assert_eq!(emergencies[0].path().extension().unwrap(), "txt");
    let restarted = DiagnosticRuntime::start_isolated(dir.0.clone());
    let recovered = restarted.read_text().unwrap();
    assert!(recovered.contains("synthetic busy writer panic"));
    assert!(!emergencies[0].path().exists());
    restarted.shutdown();
}

#[test]
fn retention_removes_only_expired_managed_txt_archives() {
    let dir = Directory::new();
    let runtime = DiagnosticRuntime::start_isolated(dir.0.clone());
    runtime.flush().unwrap();
    let old = dir.0.join("logs/archive/application-old.txt");
    File::create(&old)
        .unwrap()
        .set_times(
            std::fs::FileTimes::new()
                .set_modified(std::time::SystemTime::now() - Duration::from_secs(15 * 86400)),
        )
        .unwrap();
    let recent = dir.0.join("logs/archive/application-recent.txt");
    fs::write(&recent, "recent").unwrap();
    storage::automatic(&runtime).unwrap();
    assert!(!old.exists());
    assert!(recent.exists());
    runtime.shutdown();
}

#[test]
fn panic_child() {
    let Some(root) = std::env::var_os("ETS2_LOGGING_PANIC_TEST") else {
        return;
    };
    let runtime = DiagnosticRuntime::start(PathBuf::from(root), "test".into());
    install_panic_hook();
    runtime.flush().unwrap();
    assert!(
        std::panic::catch_unwind(|| panic!("synthetic panic; token=panic-private-token")).is_err()
    );
    let _guard = runtime.maintenance.lock().unwrap();
    // Pretend the disk writer holds the journal lock. Panic logging must never deadlock.
    assert!(std::panic::catch_unwind(|| panic!("synthetic busy writer panic")).is_err());
    drop(_guard);
    runtime.shutdown();
}

#[cfg(windows)]
#[test]
fn cache_rejects_windows_junctions_including_ancestors() {
    let dir = Directory::new();
    let runtime = DiagnosticRuntime::start_isolated(dir.0.join("support"));
    let outside = dir.0.join("protected");
    fs::create_dir_all(&outside).unwrap();
    fs::write(outside.join("game.sii"), "protected").unwrap();
    let junction = runtime.root.join("cache").join("linked");
    let status = std::process::Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(&junction)
        .arg(&outside)
        .output()
        .unwrap();
    assert!(
        status.status.success(),
        "{} {}",
        String::from_utf8_lossy(&status.stdout),
        String::from_utf8_lossy(&status.stderr)
    );
    assert!(storage::preview(&runtime, "cache").is_err());
    assert!(checked_path(&junction.join("child"), Path::new("file.txt")).is_err());
    assert_eq!(
        fs::read_to_string(outside.join("game.sii")).unwrap(),
        "protected"
    );
    fs::remove_dir(junction).unwrap();
    runtime.shutdown();
}
