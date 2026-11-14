exec((__import__('pathlib').Path(__file__).with_name('simplify_logging.py')).read_text(encoding='utf-8').split('old = ')[0])
write(backend / 'diagnostic_tests.rs', r'''use super::*;

struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("ets2_logging_test_{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap(); Self(root)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        assert!(self.0.starts_with(std::env::temp_dir()));
        assert!(self.0.file_name().unwrap().to_string_lossy().starts_with("ets2_logging_test_"));
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn txt_persists_details_and_only_supported_levels() {
    let dir = Directory::new();
    let runtime = DiagnosticRuntime::start_isolated(dir.0.clone());
    runtime.record(DiagnosticEvent::new("DEBUG", "test", "hidden", "debug-secret"));
    runtime.record(DiagnosticEvent::new("TRACE", "test", "hidden", "trace-secret"));
    let mut event = DiagnosticEvent::new("ERROR", "truck_editor", "load_truck", "Failed to load truck data.");
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
    assert!(!text.contains("debug-secret")); assert!(!text.contains("trace-secret"));
    for line in text.lines().filter(|s| s.starts_with('[')) {
        chrono::DateTime::parse_from_rfc3339(line.trim_start_matches('[').split(']').next().unwrap()).unwrap();
    }
    runtime.shutdown();
    for entry in walkdir::WalkDir::new(&dir.0).into_iter().map(Result::unwrap).filter(|e| e.file_type().is_file()) {
        assert_eq!(entry.path().extension().unwrap(), "txt");
    }
}

#[test]
fn restart_appends_preserving_existing_and_partial_entries() {
    let dir = Directory::new();
    let runtime = DiagnosticRuntime::start_isolated(dir.0.clone());
    runtime.record(DiagnosticEvent::new("ERROR", "test", "first", "first session error"));
    runtime.shutdown();
    let before = fs::read_to_string(dir.0.join(LOG_FILE)).unwrap();
    drop(runtime);
    OpenOptions::new().append(true).open(dir.0.join(LOG_FILE)).unwrap().write_all(b"partial last write").unwrap();
    let second = DiagnosticRuntime::start_isolated(dir.0.clone());
    second.record(DiagnosticEvent::new("WARN", "test", "second", "second session warning"));
    let text = second.read_text().unwrap();
    assert!(text.starts_with(&before)); assert!(text.contains("partial last write\n["));
    assert!(text.contains("first session error")); assert!(text.contains("second session warning"));
    second.shutdown();
}

#[test]
fn preview_is_last_100_physical_lines_in_chronological_order() {
    let dir = Directory::new();
    let runtime = DiagnosticRuntime::start_isolated(dir.0.clone());
    for n in 0..140 { runtime.record(DiagnosticEvent::new("INFO", "test", "sequence", &format!("sequence-{n}"))); }
    let text = runtime.read_text().unwrap();
    let preview = runtime.preview(100).unwrap();
    assert_eq!(preview.lines().count(), 100);
    assert_eq!(preview, text.lines().skip(text.lines().count() - 100).collect::<Vec<_>>().join("\n"));
    assert!(preview.contains("sequence-139")); assert!(!preview.contains("sequence-0\n"));
    runtime.shutdown();
}

#[test]
fn duplicate_backend_and_frontend_causes_share_one_record() {
    let dir = Directory::new();
    let runtime = DiagnosticRuntime::start_isolated(dir.0.clone());
    let mut first = DiagnosticEvent::new("ERROR", "backup", "copy", "Backup failed.");
    first.technical = Some("Permission denied".into()); first.operation_id = Some("OP-same".into());
    let id = runtime.record(first.clone());
    first.module = "ipc".into(); first.message = "Command failed.".into();
    assert_eq!(runtime.record(first.clone()), id);
    for _ in 0..1000 { runtime.record(first.clone()); }
    first.operation_id = Some("OP-another".into()); runtime.record(first);
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
    event.context.insert("profileName".into(), "SECRET_PROFILE".into());
    runtime.record(event);
    let text = runtime.read_text().unwrap();
    for secret in ["PrivatePerson", "SECRET_TOKEN", "SECRET_PASSWORD", "user@example.com", "SECRET_PROFILE"] { assert!(!text.contains(secret), "{text}"); }
    assert!(text.contains("Permission denied"));
    assert_eq!(sanitize("SiiNunit { player: private }"), "<REDACTED_SAVE_CONTENT>");
    assert!(!sanitize("Bearer abc.def.ghi").contains("abc.def.ghi"));
    runtime.shutdown();
}

#[test]
fn rotation_caps_managed_txt_without_deleting_other_data() {
    let dir = Directory::new();
    let runtime = DiagnosticRuntime::start_isolated(dir.0.clone()); runtime.flush().unwrap();
    let log = dir.0.join(LOG_FILE);
    OpenOptions::new().write(true).open(&log).unwrap().set_len(FILE_LIMIT - 5).unwrap();
    fs::write(dir.0.join("logs/archive/keep-user-export.txt"), "preserved").unwrap();
    fs::write(dir.0.join("logs/archive/legacy.jsonl"), "preserved").unwrap();
    for n in 0..5 {
        File::create(dir.0.join(format!("logs/archive/application-test-{n}.txt"))).unwrap().set_len(FILE_LIMIT).unwrap();
    }
    runtime.record(DiagnosticEvent::new("ERROR", "test", "rotation", "rotation marker"));
    runtime.flush().unwrap();
    assert!(fs::metadata(log).unwrap().len() < FILE_LIMIT);
    assert_eq!(fs::read_to_string(dir.0.join("logs/archive/keep-user-export.txt")).unwrap(), "preserved");
    assert!(dir.0.join("logs/archive/legacy.jsonl").exists());
    let managed = fs::read_dir(dir.0.join("logs/archive")).unwrap().map(Result::unwrap)
        .filter(|e| e.file_name().to_string_lossy().starts_with("application-"))
        .map(|e| e.metadata().unwrap().len()).sum::<u64>() + fs::metadata(dir.0.join(LOG_FILE)).unwrap().len();
    assert!(managed <= TOTAL_LIMIT);
    runtime.shutdown();
}

#[test]
fn cache_cleanup_requires_current_confirmation_and_preserves_user_data() {
    let dir = Directory::new();
    let runtime = DiagnosticRuntime::start_isolated(dir.0.join("support"));
    for name in ["game.sii", "profile.sii", "save-backup.sii", "settings.json"] { fs::write(dir.0.join(name), "protected").unwrap(); }
    fs::write(runtime.root.join("cache/item.tmp"), "temporary").unwrap();
    let preview = storage::preview(&runtime, "cache").unwrap();
    assert!(storage::cleanup(&runtime, "cache", &preview.token, false).is_err());
    fs::write(runtime.root.join("cache/other.tmp"), "changed").unwrap();
    assert!(storage::cleanup(&runtime, "cache", &preview.token, true).is_err());
    let current = storage::preview(&runtime, "cache").unwrap();
    assert_eq!(storage::cleanup(&runtime, "cache", &current.token, true).unwrap(), 16);
    assert!(fs::read_dir(runtime.root.join("cache")).unwrap().next().is_none());
    for name in ["game.sii", "profile.sii", "save-backup.sii", "settings.json"] { assert_eq!(fs::read_to_string(dir.0.join(name)).unwrap(), "protected"); }
    for scope in ["all_logs", "old_logs", "reports", "../", "profiles"] { assert!(storage::preview(&runtime, scope).is_err()); }
    assert!(checked_path(&runtime.root, Path::new("../settings.json")).is_err());
    runtime.shutdown();
}

#[test]
fn filesystem_failure_never_panics_or_changes_original_error() {
    let dir = Directory::new(); fs::write(dir.0.join("not-a-directory"), "block").unwrap();
    let runtime = DiagnosticRuntime::start_isolated(dir.0.join("not-a-directory/support"));
    let original: Result<(), String> = Err("original failure".into());
    runtime.record(DiagnosticEvent::new("ERROR", "test", "failed_disk", original.as_ref().unwrap_err()));
    assert!(runtime.flush().is_err()); assert!(runtime.preview(100).is_err());
    assert_eq!(original.unwrap_err(), "original failure"); runtime.shutdown();
}

#[test]
fn panic_is_persisted_with_stack_and_recovers_busy_writer() {
    // Isolate the global panic hook in a child process, including its own temp directory.
    let dir = Directory::new();
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "shared::logs::diagnostic::tests::panic_child", "--nocapture"])
        .env("ETS2_LOGGING_PANIC_TEST", &dir.0).output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = fs::read_to_string(dir.0.join(LOG_FILE)).unwrap();
    assert!(text.contains("rust_panic")); assert!(text.contains("synthetic panic")); assert!(text.contains("Stack:"));
    assert!(!text.contains("panic-private-token"));
    let emergencies = fs::read_dir(dir.0.join("logs/archive")).unwrap().map(Result::unwrap).filter(|e| e.file_name().to_string_lossy().starts_with("emergency-")).collect::<Vec<_>>();
    assert_eq!(emergencies.len(), 1); assert_eq!(emergencies[0].path().extension().unwrap(), "txt");
}

#[test]
fn panic_child() {
    let Some(root) = std::env::var_os("ETS2_LOGGING_PANIC_TEST") else { return; };
    let runtime = DiagnosticRuntime::start(PathBuf::from(root), "test".into());
    install_panic_hook(); runtime.flush().unwrap();
    assert!(std::panic::catch_unwind(|| panic!("synthetic panic; token=panic-private-token")).is_err());
    let _guard = runtime.maintenance.lock().unwrap();
    // Pretend the disk writer holds the journal lock. Panic logging must never deadlock.
    assert!(std::panic::catch_unwind(|| panic!("synthetic busy writer panic")).is_err());
    drop(_guard); runtime.shutdown();
}

#[cfg(windows)]
#[test]
fn cache_rejects_windows_junctions_including_ancestors() {
    let dir = Directory::new();
    let runtime = DiagnosticRuntime::start_isolated(dir.0.join("support"));
    let outside = dir.0.join("protected"); fs::create_dir_all(&outside).unwrap();
    fs::write(outside.join("game.sii"), "protected").unwrap();
    let junction = runtime.root.join("cache/linked");
    let status = std::process::Command::new("cmd").args(["/C", "mklink", "/J"])
        .arg(&junction).arg(&outside).output().unwrap();
    assert!(status.status.success());
    assert!(storage::preview(&runtime, "cache").is_err());
    assert!(checked_path(&junction.join("child"), Path::new("file.txt")).is_err());
    assert_eq!(fs::read_to_string(outside.join("game.sii")).unwrap(), "protected");
    fs::remove_dir(junction).unwrap(); runtime.shutdown();
}
''')
p = backend / 'report.rs'
s = p.read_text(encoding='utf-8').replace('use std::fs;\n', '').replace('DiagnosticRuntime, atomic_write, checked_path, LOG_FILE', 'DiagnosticRuntime, atomic_write, checked_path')
s = s.replace('s.eq_ignore_ascii_case("txt")', 's.to_str().is_some_and(|s| s.eq_ignore_ascii_case("txt"))')
s = s.replace('    use super::*;', '    use super::*;\n    use std::fs;\n    use crate::shared::logs::diagnostic::LOG_FILE;')
s = s.replace('        let temp = tempfile::tempdir().unwrap();\n        let runtime = DiagnosticRuntime::start_isolated(temp.path().join("support"));', '        let directory = std::env::temp_dir().join(format!("ets2_logging_export_test_{}", uuid::Uuid::new_v4()));\n        fs::create_dir_all(&directory).unwrap();\n        let runtime = DiagnosticRuntime::start_isolated(directory.join("support"));')
s = s.replace('temp.path()', 'directory').replace('        runtime.shutdown();', '        runtime.shutdown();\n        assert!(directory.starts_with(std::env::temp_dir()));\n        fs::remove_dir_all(directory).unwrap();')
write(p, s)

p = backend / 'frontend_tests.mjs'
s = p.read_text(encoding='utf-8').replace('"test_success"', '"save_test_success"')
start = s.index('test("repeated failures')
end = s.index('test("known user', start)
s = s[:start] + '''test("a rejected IPC error is not logged again as an unhandled rejection", async () => {
  calls = [];
  await assert.rejects(runtime.diagnosticInvoke("test_failed_command"));
  const before = calls.filter((call) => call.command === "diagnostics_record").length;
  listeners.get("unhandledrejection")[0]({ reason: original });
  await new Promise((resolve) => setImmediate(resolve));
  assert.equal(calls.filter((call) => call.command === "diagnostics_record").length, before);
});

test("repeated global errors are bounded and ordinary reads do not flood the log", async () => {
  calls = [];
  for (let i = 0; i < 500; i += 1) listeners.get("unhandledrejection")[0]({ reason: "repeating rejection" });
  await runtime.diagnosticInvoke("get_test_data");
  assert.equal(calls.filter((call) => call.command === "diagnostics_record").length, 1);
  assert(window.__ETS2_DIAGNOSTICS__.events.size <= 128);
});

test("console failures are captured without serializing arbitrary objects", async () => {
  calls = [];
  console.error("handled JS failure", { token: "private-token-object" });
  await new Promise((resolve) => setImmediate(resolve));
  const entries = calls.filter((call) => call.command === "diagnostics_record");
  assert.equal(entries.length, 1);
  assert.equal(entries[0].args.event.action, "console_error");
  assert(!JSON.stringify(entries).includes("private-token-object"));
});

''' + s[end:]
write(p, s)

p = backend / 'ui_smoke.mjs'
s = p.read_text(encoding='utf-8')
start = s.index('const result = await call("Runtime.evaluate"')
s = s[:start] + r'''const result = await call("Runtime.evaluate", { awaitPromise: true, returnByValue: true, expression: `
(async () => {
  const invoke = (...args) => window.__TAURI__.core.invoke(...args);
  const initialLanguage = await invoke("get_current_language_command");
  const { openLogFile } = await import("/modules/shared/diagnostics_ui.js");
  const results = [];
  try {
    await invoke("diagnostics_record", { event: {
      level: "ERROR", module: "logging_validation", action: "ui_smoke", message: "TXT logging validation event.",
      technical: "C:\\\\Users\\\\ValidationUser\\\\Documents\\\\game.sii: permission denied; token=validation_secret",
      operationId: "OP-simple-logging-validation"
    }});
    for (const language of ["en", "de"]) {
      await invoke("set_language_command", { language });
      await Promise.all([openLogFile(), openLogFile()]);
      const dialogs = document.querySelectorAll("dialog.log-file-dialog[open]");
      if (dialogs.length !== 1) throw new Error("Expected one log modal");
      const dialog = dialogs[0];
      const preview = dialog.querySelector("pre").textContent;
      if (!preview.includes("TXT logging validation event.")) throw new Error("Missing latest event");
      if (preview.includes("ValidationUser") || preview.includes("validation_secret")) throw new Error("Private data leaked");
      if (preview.split("\\n").length > 100) throw new Error("Too many preview lines");
      if (dialog.querySelectorAll("button").length !== 2) throw new Error("Expected only export and close");
      if (dialog.querySelectorAll("textarea, select, table").length) throw new Error("Unexpected diagnostic controls");
      results.push({ language, title: dialog.querySelector("h2").textContent, buttons: [...dialog.querySelectorAll("button")].map(b => b.textContent), lines: preview.split("\\n").length });
      dialog.close();
      await new Promise(resolve => setTimeout(resolve, 20));
    }
    const preview = await invoke("diagnostics_cleanup_preview", { scope: "cache" });
    let confirmationEnforced = false;
    try { await invoke("diagnostics_cleanup", { scope: "cache", token: preview.token, confirmed: false }); }
    catch { confirmationEnforced = true; }
    let logDeletionRejected = false;
    try { await invoke("diagnostics_cleanup_preview", { scope: "all_logs" }); }
    catch { logDeletionRejected = true; }
    return { results, confirmationEnforced, logDeletionRejected };
  } finally {
    document.querySelector("dialog.log-file-dialog[open]")?.close();
    await invoke("set_language_command", { language: initialLanguage });
  }
})()
` });
assert(!result.exceptionDetails, JSON.stringify(result.exceptionDetails));
const summary = result.result.value;
assert(summary.confirmationEnforced); assert(summary.logDeletionRejected);
assert.equal(summary.results.length, 2); assert.equal(exceptions.length, 0, exceptions.join("\n"));
const directory = fileURLToPath(new URL("../../../../target/diagnostic-validation/", import.meta.url));
await mkdir(directory, { recursive: true });
await writeFile(path.join(directory, "simple-logging-ui-smoke.json"), JSON.stringify(summary, null, 2));
await call("Runtime.evaluate", { expression: 'import("/modules/shared/diagnostics_ui.js").then(ui => ui.openLogFile())', awaitPromise: true });
const screenshot = await call("Page.captureScreenshot", { format: "png" });
await writeFile(path.join(directory, "simple-logging-ui.png"), Buffer.from(screenshot.data, "base64"));
console.log(JSON.stringify(summary, null, 2));
socket.close();
'''
write(p, s)
