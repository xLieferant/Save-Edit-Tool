from pathlib import Path
root = Path('E:/Coding Projects/Save Edit Tool/ets2-tool')
backend = root / 'src-tauri/src/features/logging'
def write(path, text):
    path.write_text(text, encoding='utf-8', newline='\n')

old = (backend / 'diagnostic_runtime.rs').read_text(encoding='utf-8')
operations = old[old.index('thread_local!'):old.index('#[path = "diagnostic_storage.rs"]')]
event = old[old.index('#[derive(Debug, Clone, Default, Serialize, Deserialize)]'):old.index('enum WriterMessage')]
privacy = old[old.index('pub fn bounded'):old.index('pub fn atomic_json')]
atomic = old[old.index('pub fn atomic_write'):old.index('fn read_small_json')]
write(backend / 'diagnostic_runtime.rs', '''//! Local TXT logging. Producers never wait for disk IO; one bounded writer preserves order.
use chrono::{Local, SecondsFormat, Utc};
use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::{BTreeMap, VecDeque};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, Weak, mpsc};
use std::time::{Duration, Instant};
use uuid::Uuid;

''' + operations + '''#[path = "diagnostic_storage.rs"]
pub mod storage;
pub const LOG_FILE: &str = "logs/application.log.txt";
pub const FILE_LIMIT: u64 = 5 * 1024 * 1024;
pub const TOTAL_LIMIT: u64 = 20 * 1024 * 1024;
pub const RECENT_LIMIT: usize = 500;
pub const TEXT_LIMIT: usize = 8192;
static ACTIVE: Lazy<Mutex<Weak<DiagnosticRuntime>>> = Lazy::new(|| Mutex::new(Weak::new()));
static LEGACY_LOGGER: DiagnosticLog = DiagnosticLog;

''' + event + '''enum WriterMessage {
    Event(Box<DiagnosticEvent>),
    Flush(mpsc::Sender<()>),
    Stop(mpsc::Sender<()>),
}

pub struct DiagnosticRuntime {
    pub root: PathBuf,
    pub session_id: String,
    pub recent: Mutex<VecDeque<DiagnosticEvent>>,
    pub storage_error: Mutex<Option<String>>,
    pub dropped: AtomicU64,
    pub maintenance: Mutex<()>,
    sender: mpsc::SyncSender<WriterMessage>,
    stopped: AtomicBool,
    // Timestamp assignment and enqueue happen under the same short-lived lock.
    dedup: Mutex<VecDeque<(String, Instant, Option<String>)>>,
}

pub fn active() -> Option<Arc<DiagnosticRuntime>> {
    ACTIVE.lock().ok().and_then(|value| value.upgrade())
}

pub fn install_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        let mut event = DiagnosticEvent::new("ERROR", "rust", "panic", "A Rust panic occurred.");
        event.technical = Some(sanitize(&info.to_string()));
        event.error_kind = Some("rust_panic".into());
        event.stack = Some(sanitize(&std::backtrace::Backtrace::force_capture().to_string()));
        event.operation_id = OPERATION.with(|slot| slot.try_borrow().ok().and_then(|s| s.clone()));
        if let Some(runtime) = active() {
            runtime.record_panic(event);
        } else {
            // This also covers failures before Tauri setup. Never write into the installation.
            let root = dirs::data_local_dir().unwrap_or_else(std::env::temp_dir)
                .join("com.xlieferant.saveedittool").join("support");
            event.timestamp = Local::now().to_rfc3339_opts(SecondsFormat::Millis, false);
            let _ = emergency_write(&root, &event);
        }
    }));
}

impl DiagnosticRuntime {
    pub fn start(root: PathBuf, version: String) -> Arc<Self> {
        Self::start_inner(root, version, true)
    }
    #[cfg(test)]
    pub fn start_isolated(root: PathBuf) -> Arc<Self> {
        Self::start_inner(root, "test".into(), false)
    }
    fn start_inner(root: PathBuf, version: String, publish: bool) -> Arc<Self> {
        let (sender, receiver) = mpsc::sync_channel(2048);
        let runtime = Arc::new(Self {
            root, session_id: format!("SESSION-{}", Uuid::new_v4()),
            recent: Mutex::new(VecDeque::new()), storage_error: Mutex::new(None),
            dropped: AtomicU64::new(0), maintenance: Mutex::new(()), sender,
            stopped: AtomicBool::new(false), dedup: Mutex::new(VecDeque::new()),
        });
        if publish {
            if let Ok(mut slot) = ACTIVE.lock() { *slot = Arc::downgrade(&runtime); }
            let _ = log::set_logger(&LEGACY_LOGGER);
            log::set_max_level(log::LevelFilter::Info);
        }
        let prepared = (|| {
            for name in ["logs/archive", "cache"] {
                fs::create_dir_all(checked_path(&runtime.root, Path::new(name))?)
                    .map_err(|e| e.to_string())?;
            }
            storage::automatic(&runtime)
        })();
        if let Err(error) = prepared { runtime.set_error(error); }
        let weak = Arc::downgrade(&runtime);
        if let Err(error) = std::thread::Builder::new().name("application-log-writer".into()).spawn(move || {
            while let Ok(message) = receiver.recv() {
                let Some(runtime) = weak.upgrade() else { break; };
                match message {
                    WriterMessage::Event(event) => {
                        if let Err(error) = runtime.persist(&event) { runtime.set_error(error); }
                    }
                    WriterMessage::Flush(done) => { let _ = done.send(()); }
                    WriterMessage::Stop(done) => { let _ = done.send(()); break; }
                }
            }
        }) { runtime.set_error(error.to_string()); }
        runtime.record(DiagnosticEvent::new("INFO", "application", "startup",
            &format!("ETS2 TOOLBOX started. Version: {}", sanitize(&version))));
        runtime
    }

    pub fn set_error(&self, error: String) {
        if let Ok(mut slot) = self.storage_error.lock() { *slot = Some(sanitize(&error)); }
    }

    fn prepare_event(&self, mut event: DiagnosticEvent) -> Option<DiagnosticEvent> {
        event.level = match event.level.to_ascii_uppercase().as_str() {
            "INFO" => "INFO", "WARN" | "WARNING" => "WARN", "ERROR" | "FATAL" => "ERROR",
            _ => return None,
        }.into();
        event.timestamp = Local::now().to_rfc3339_opts(SecondsFormat::Millis, false);
        event.session_id = self.session_id.clone();
        event.operation_id = event.operation_id.or_else(|| OPERATION.with(|s| s.try_borrow().ok().and_then(|v| v.clone())));
        event.operation_id = event.operation_id.filter(|s| s.starts_with("OP-") && s.len() <= 80
            && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'));
        event.error_id = (event.level != "INFO").then(|| format!("ERR-{}", Uuid::new_v4()));
        // Headers stay on one line; all text passes redaction before memory or disk storage.
        event.module = bounded(&sanitize(&event.module).replace(['\\r', '\\n'], " "), 100);
        event.action = bounded(&sanitize(&event.action).replace(['\\r', '\\n'], " "), 150);
        event.message = sanitize(&event.message);
        event.technical = event.technical.as_deref().map(sanitize);
        event.stack = event.stack.as_deref().map(sanitize);
        event.causes = event.causes.iter().take(8).map(|s| bounded(&sanitize(s), 1024)).collect();
        event.error_kind = event.error_kind.as_deref().map(sanitize);
        event.outcome = event.outcome.as_deref().map(sanitize);
        event.backup_id = event.backup_id.as_deref().map(sanitize);
        event.context.retain(|key, _| ["fileName", "line", "column", "backupType", "fileCount", "durationMs", "step"].contains(&key.as_str()));
        for value in event.context.values_mut() { *value = bounded(&sanitize(value), 512); }
        Some(event)
    }

    pub fn record(&self, event: DiagnosticEvent) -> Option<String> {
        if self.stopped.load(Ordering::Relaxed) { return None; }
        let mut dedup = self.dedup.lock().ok()?;
        let event = self.prepare_event(event)?;
        let phase = match event.message.as_str() {
            "Operation started." | "Command started." => "started",
            "Operation completed." | "Command completed." => "completed",
            _ => event.technical.as_deref().unwrap_or(&event.message),
        };
        // Backend and IPC failures with the same cause and operation share one record.
        let fingerprint = if let Some(op) = &event.operation_id {
            format!("{}|{}|{}", event.level, op, phase)
        } else { format!("{}|{}|{}|{}", event.level, event.module, event.action, phase) };
        let now = Instant::now();
        dedup.retain(|(_, time, _)| now.duration_since(*time) < Duration::from_secs(5));
        if let Some((_, _, id)) = dedup.iter().find(|(key, _, _)| key == &fingerprint) { return id.clone(); }
        let id = event.error_id.clone();
        match self.sender.try_send(WriterMessage::Event(Box::new(event.clone()))) {
            Ok(()) => {
                if dedup.len() >= 128 { dedup.pop_front(); }
                dedup.push_back((fingerprint, now, id.clone()));
                if let Ok(mut recent) = self.recent.lock() {
                    if recent.len() >= RECENT_LIMIT { recent.pop_front(); }
                    recent.push_back(event);
                }
            }
            Err(_) => { self.dropped.fetch_add(1, Ordering::Relaxed); }
        }
        id
    }

    pub fn legacy(&self, level: &str, module: &str, message: &str) {
        let level = if message.starts_with("[trace] ERROR") { "ERROR" } else { level };
        self.record(DiagnosticEvent::new(level, module, "legacy_event", message));
    }

    fn persist(&self, event: &DiagnosticEvent) -> Result<(), String> {
        let _guard = self.maintenance.lock().map_err(|_| "Log storage lock poisoned")?;
        self.append_locked(event)
    }

    fn append_locked(&self, event: &DiagnosticEvent) -> Result<(), String> {
        let path = checked_path(&self.root, Path::new(LOG_FILE))?;
        let bytes = format_event(event).into_bytes();
        let rotated = rotate(&self.root, &path, bytes.len() as u64)?;
        let mut file = OpenOptions::new().create(true).append(true).read(true).open(&path).map_err(|e| e.to_string())?;
        // Preserve a partial last write and separate the next complete entry after a crash.
        if file.metadata().map_err(|e| e.to_string())?.len() > 0 {
            file.seek(SeekFrom::End(-1)).map_err(|e| e.to_string())?;
            let mut last = [0u8];
            file.read_exact(&mut last).map_err(|e| e.to_string())?;
            if last[0] != b'\\n' { file.write_all(b"\\n").map_err(|e| e.to_string())?; }
        }
        file.write_all(&bytes).map_err(|e| e.to_string())?;
        if event.level != "INFO" { file.sync_data().map_err(|e| e.to_string())?; }
        drop(file);
        if rotated { storage::automatic(self)?; }
        Ok(())
    }

    fn record_panic(&self, event: DiagnosticEvent) {
        let Some(event) = self.prepare_event(event) else { return; };
        // Do not wait for the writer when the writer itself panicked.
        if std::thread::current().name() != Some("application-log-writer") {
            let _ = self.flush_timeout(Duration::from_millis(500));
        }
        let persisted = self.maintenance.try_lock().ok().is_some_and(|_guard| self.append_locked(&event).is_ok());
        if !persisted { let _ = emergency_write(&self.root, &event); }
    }

    fn flush_timeout(&self, timeout: Duration) -> Result<(), String> {
        let (sender, receiver) = mpsc::channel();
        self.sender.try_send(WriterMessage::Flush(sender)).map_err(|_| "diagnostics.errors.writer_busy")?;
        receiver.recv_timeout(timeout).map_err(|_| "diagnostics.errors.writer_busy".to_string())?;
        self.storage_error.lock().map_err(|_| "diagnostics.errors.unavailable".to_string())?
            .clone().map_or(Ok(()), Err)
    }
    pub fn flush(&self) -> Result<(), String> { self.flush_timeout(Duration::from_secs(5)) }

    pub fn read_text(&self) -> Result<String, String> {
        self.flush()?;
        let _guard = self.maintenance.lock().map_err(|_| "diagnostics.errors.unavailable")?;
        let path = checked_path(&self.root, Path::new(LOG_FILE))?;
        let file = File::open(path).map_err(|e| e.to_string())?;
        if file.metadata().map_err(|e| e.to_string())?.len() > FILE_LIMIT + 128 * 1024 {
            return Err("diagnostics.errors.storage_limit".into());
        }
        let mut text = String::new();
        file.take(FILE_LIMIT + 128 * 1024).read_to_string(&mut text).map_err(|e| e.to_string())?;
        Ok(text)
    }
    pub fn preview(&self, lines: usize) -> Result<String, String> {
        let text = self.read_text()?;
        let lines = lines.clamp(50, 100);
        let mut tail = text.lines().rev().take(lines).collect::<Vec<_>>();
        tail.reverse();
        Ok(tail.join("\\n"))
    }
    pub fn shutdown(&self) {
        if self.stopped.load(Ordering::Relaxed) { return; }
        let lost = self.dropped.load(Ordering::Relaxed);
        if lost > 0 { self.record(DiagnosticEvent::new("WARN", "logging", "overload", &format!("{} entries dropped because the bounded log queue was full.", lost))); }
        self.record(DiagnosticEvent::new("INFO", "application", "shutdown", "Application exited normally."));
        let _ = self.flush();
        self.stopped.store(true, Ordering::Relaxed);
        let (sender, receiver) = mpsc::channel();
        if self.sender.try_send(WriterMessage::Stop(sender)).is_ok() { let _ = receiver.recv_timeout(Duration::from_secs(2)); }
    }
}

pub fn format_event(event: &DiagnosticEvent) -> String {
    let mut text = format!("[{}] [{}] [{}::{}] {}\\nSession: {}", event.timestamp, event.level, event.module, event.action, event.message, event.session_id);
    if let Some(op) = &event.operation_id { text.push_str(&format!(" | Operation: {}", op)); }
    text.push('\\n');
    for (label, value) in [("Reason", &event.technical), ("Type", &event.error_kind), ("Error", &event.error_id),
        ("Outcome", &event.outcome), ("Backup", &event.backup_id)] {
        if let Some(value) = value { text.push_str(&format!("{}: {}\\n", label, value)); }
    }
    for cause in &event.causes { text.push_str(&format!("Caused by: {}\\n", cause)); }
    for (key, value) in &event.context { text.push_str(&format!("{}: {}\\n", key, value)); }
    if let Some(value) = event.original_modified { text.push_str(&format!("Original modified: {}\\n", value)); }
    if let Some(value) = event.rollback_verified { text.push_str(&format!("Rollback verified: {}\\n", value)); }
    if let Some(stack) = &event.stack { text.push_str(&format!("Stack: {}\\n", stack)); }
    text
}

fn emergency_write(root: &Path, event: &DiagnosticEvent) -> Result<(), String> {
    let directory = checked_path(root, Path::new("logs/archive"))?;
    fs::create_dir_all(directory).map_err(|e| e.to_string())?;
    let path = checked_path(root, &PathBuf::from("logs/archive").join(format!("emergency-{}.txt", Uuid::new_v4())))?;
    atomic_write(&path, format_event(event).as_bytes())
}

fn rotate(root: &Path, path: &Path, additional: u64) -> Result<bool, String> {
    if fs::metadata(path).map(|m| m.len() > 0 && m.len().saturating_add(additional) > FILE_LIMIT).unwrap_or(false) {
        let destination = checked_path(root, &PathBuf::from("logs/archive").join(format!("application-{}-{}.txt", Utc::now().format("%Y%m%dT%H%M%S%3f"), Uuid::new_v4())))?;
        fs::rename(path, destination).map_err(|e| e.to_string())?;
        return Ok(true);
    }
    Ok(false)
}

struct DiagnosticLog;
impl log::Log for DiagnosticLog {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool { metadata.level() <= log::Level::Info }
    fn log(&self, record: &log::Record<'_>) {
        if self.enabled(record.metadata()) {
            if let Some(runtime) = active() { runtime.legacy(record.level().as_str(), record.target(), &record.args().to_string()); }
        }
    }
    fn flush(&self) { if let Some(runtime) = active() { let _ = runtime.flush(); } }
}

''' + privacy + atomic + '''#[cfg(test)]
#[path = "diagnostic_tests.rs"]
mod tests;
''')

write(backend / 'diagnostic_storage.rs', '''use super::{DiagnosticRuntime, LOG_FILE, TOTAL_LIMIT, checked_path};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupPreview { pub scope: String, pub token: String, pub bytes: u64, pub files: usize }

fn cache_files(runtime: &DiagnosticRuntime, scope: &str) -> Result<Vec<(PathBuf, u64, SystemTime)>, String> {
    if scope != "cache" { return Err("diagnostics.errors.invalid_cleanup".into()); }
    let directory = checked_path(&runtime.root, Path::new("cache"))?;
    if !directory.exists() { return Ok(Vec::new()); }
    let mut files = Vec::new();
    // Only this logger-owned temporary directory is eligible. No app config or game paths.
    for (index, entry) in walkdir::WalkDir::new(directory).follow_links(false).max_depth(12).into_iter().enumerate() {
        if index >= 10000 { return Err("diagnostics.errors.storage_limit".into()); }
        let entry = entry.map_err(|e| e.to_string())?;
        let relative = entry.path().strip_prefix(&runtime.root).map_err(|_| "diagnostics.errors.unsafe_path")?;
        checked_path(&runtime.root, relative)?;
        if entry.file_type().is_file() {
            let metadata = entry.metadata().map_err(|e| e.to_string())?;
            files.push((entry.path().to_owned(), metadata.len(), metadata.modified().map_err(|e| e.to_string())?));
        }
    }
    files.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(files)
}
fn preview_for(runtime: &DiagnosticRuntime, files: &[(PathBuf, u64, SystemTime)]) -> CleanupPreview {
    let mut digest = Sha256::new();
    digest.update(runtime.session_id.as_bytes());
    for (path, size, time) in files {
        digest.update(path.to_string_lossy().as_bytes()); digest.update(size.to_le_bytes()); digest.update(format!("{time:?}").as_bytes());
    }
    CleanupPreview { scope: "cache".into(), token: format!("{:x}", digest.finalize()), bytes: files.iter().map(|(_, size, _)| size).sum(), files: files.len() }
}
pub fn preview(runtime: &DiagnosticRuntime, scope: &str) -> Result<CleanupPreview, String> {
    let _guard = runtime.maintenance.lock().map_err(|_| "diagnostics.errors.unavailable")?;
    Ok(preview_for(runtime, &cache_files(runtime, scope)?))
}
pub fn cleanup(runtime: &DiagnosticRuntime, scope: &str, token: &str, confirmed: bool) -> Result<u64, String> {
    if !confirmed { return Err("diagnostics.errors.confirmation_required".into()); }
    let _guard = runtime.maintenance.lock().map_err(|_| "diagnostics.errors.unavailable")?;
    let files = cache_files(runtime, scope)?;
    if preview_for(runtime, &files).token != token { return Err("diagnostics.errors.cleanup_changed".into()); }
    let mut freed = 0;
    for (path, size, _) in files {
        let relative = path.strip_prefix(&runtime.root).map_err(|_| "diagnostics.errors.unsafe_path")?;
        fs::remove_file(checked_path(&runtime.root, relative)?).map_err(|e| e.to_string())?;
        freed += size;
    }
    Ok(freed)
}

/// Retain only managed TXT archives for at most 14 days and 20 MiB including the active log.
/// Existing legacy reports, game data, configuration and exports are never cleanup candidates.
pub fn automatic(runtime: &DiagnosticRuntime) -> Result<(), String> {
    let directory = checked_path(&runtime.root, Path::new("logs/archive"))?;
    if !directory.exists() { return Ok(()); }
    let mut archives = Vec::new();
    for entry in fs::read_dir(directory).map_err(|e| e.to_string())?.take(10001) {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if !(name.starts_with("application-") || name.starts_with("emergency-")) || !name.ends_with(".txt") { continue; }
        let path = entry.path();
        checked_path(&runtime.root, path.strip_prefix(&runtime.root).map_err(|_| "diagnostics.errors.unsafe_path")?)?;
        let metadata = entry.metadata().map_err(|e| e.to_string())?;
        if metadata.is_file() { archives.push((path, metadata.len(), metadata.modified().map_err(|e| e.to_string())?)); }
    }
    archives.sort_by_key(|(_, _, time)| *time);
    let active = checked_path(&runtime.root, Path::new(LOG_FILE))?;
    let mut total = fs::metadata(active).map(|m| m.len()).unwrap_or(0) + archives.iter().map(|(_, size, _)| size).sum::<u64>();
    let cutoff = SystemTime::now().checked_sub(Duration::from_secs(14 * 86400)).unwrap_or(SystemTime::UNIX_EPOCH);
    for (path, size, time) in archives {
        if time < cutoff || total > TOTAL_LIMIT {
            let relative = path.strip_prefix(&runtime.root).map_err(|_| "diagnostics.errors.unsafe_path")?;
            fs::remove_file(checked_path(&runtime.root, relative)?).map_err(|e| e.to_string())?;
            total = total.saturating_sub(size);
        }
    }
    Ok(())
}
''')

write(backend / 'report.rs', '''//! Compatibility export adapter: native Save dialog, full current TXT log only.
use std::fs;
use std::path::Path;
use tauri::AppHandle;
use tauri_plugin_dialog::DialogExt;
use crate::shared::logs::diagnostic::{DiagnosticRuntime, atomic_write, checked_path, LOG_FILE};

pub fn save_text(runtime: &DiagnosticRuntime, destination: &Path) -> Result<(), String> {
    if !destination.extension().is_some_and(|s| s.eq_ignore_ascii_case("txt")) {
        return Err("simple_logging.errors.txt_only".into());
    }
    // Replacing the active log, settings, backups or other application data is never an export.
    let parent = destination.parent().ok_or("diagnostics.errors.unsafe_path")?;
    let name = destination.file_name().ok_or("diagnostics.errors.unsafe_path")?;
    let destination = checked_path(parent, Path::new(name))?;
    let canonical_parent = parent.canonicalize().map_err(|e| e.to_string())?;
    let canonical_root = runtime.root.canonicalize().map_err(|e| e.to_string())?;
    if canonical_parent.starts_with(&canonical_root) { return Err("diagnostics.errors.unsafe_path".into()); }
    let text = runtime.read_text()?;
    atomic_write(&destination, text.as_bytes())
}

pub fn export(runtime: &DiagnosticRuntime, app: &AppHandle) -> Result<Option<String>, String> {
    let selected = app.dialog().file().add_filter("TXT", &["txt"])
        .set_title(crate::features::language::translator::t("simple_logging.export"))
        .set_file_name("ETS2-Toolbox-application.log.txt").blocking_save_file();
    let Some(selected) = selected else { return Ok(None); };
    let mut destination = selected.into_path().map_err(|_| "diagnostics.errors.unsafe_path")?;
    if destination.extension().is_none() { destination.set_extension("txt"); }
    save_text(runtime, &destination)?;
    Ok(Some(destination.display().to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::logs::diagnostic::DiagnosticEvent;
    #[test]
    fn txt_export_contains_full_file_and_preserves_source() {
        let temp = tempfile::tempdir().unwrap();
        let runtime = DiagnosticRuntime::start_isolated(temp.path().join("support"));
        for n in 0..160 { runtime.record(DiagnosticEvent::new("INFO", "test", "export", &format!("line-{n}"))); }
        let before = runtime.read_text().unwrap();
        let destination = temp.path().join("export.txt");
        save_text(&runtime, &destination).unwrap();
        assert_eq!(fs::read_to_string(destination).unwrap(), before);
        assert!(before.contains("line-0\\n")); assert!(before.contains("line-159\\n"));
        assert_eq!(runtime.read_text().unwrap(), before);
        assert!(save_text(&runtime, &runtime.root.join(LOG_FILE)).is_err());
        assert!(save_text(&runtime, &temp.path().join("export.pdf")).is_err());
        assert!(save_text(&runtime, &temp.path().join("export.zip")).is_err());
        runtime.shutdown();
    }
}
''')

write(backend / 'diagnostics.rs', '''use tauri::{AppHandle, State};
use crate::shared::logs::diagnostic::{DiagnosticEvent, storage};
use crate::state::DiagnosticsState;

#[tauri::command]
pub fn diagnostics_record(mut event: DiagnosticEvent, state: State<'_, DiagnosticsState>) -> Result<Option<String>, String> {
    event.original_modified = None; event.rollback_verified = None; event.backup_id = None;
    event.error_id = None; event.session_id.clear(); event.timestamp.clear();
    Ok(state.0.record(event))
}
#[tauri::command]
pub async fn get_log_preview(state: State<'_, DiagnosticsState>) -> Result<String, String> {
    let runtime = state.0.clone();
    tauri::async_runtime::spawn_blocking(move || runtime.preview(100)).await.map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn export_log_file(app: AppHandle, state: State<'_, DiagnosticsState>) -> Result<Option<String>, String> {
    let runtime = state.0.clone();
    tauri::async_runtime::spawn_blocking(move || super::report::export(&runtime, &app)).await.map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn diagnostics_cleanup_preview(scope: String, state: State<'_, DiagnosticsState>) -> Result<storage::CleanupPreview, String> {
    let runtime = state.0.clone();
    tauri::async_runtime::spawn_blocking(move || storage::preview(&runtime, &scope)).await.map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn diagnostics_cleanup(scope: String, token: String, confirmed: bool, state: State<'_, DiagnosticsState>) -> Result<u64, String> {
    let runtime = state.0.clone();
    let result = tauri::async_runtime::spawn_blocking(move || storage::cleanup(&runtime, &scope, &token, confirmed)).await.map_err(|e| e.to_string())?;
    if let Err(error) = &result {
        let mut event = DiagnosticEvent::new("WARN", "cache", "clear", "Temporary cache could not be cleared.");
        event.technical = Some(error.clone()); state.0.record(event);
    }
    result
}
''')

p = root / 'src-tauri/src/main.rs'
s = p.read_text(encoding='utf-8')
for name in ['diagnostics_status', 'diagnostics_events', 'diagnostics_settings', 'diagnostics_preview', 'diagnostics_export', 'diagnostics_open', 'diagnostics_email']:
    s = s.replace(f'            features::logging::diagnostics::{name},\n', '')
s = s.replace('            features::logging::diagnostics::diagnostics_record,', '            features::logging::diagnostics::diagnostics_record,\n            features::logging::diagnostics::get_log_preview,\n            features::logging::diagnostics::export_log_file,')
write(p, s)
p = backend / 'service.rs'
s = p.read_text(encoding='utf-8')
s = s.replace('        let report = super::report::snapshot(&runtime, super::report::ReportRequest::default())?;\n        return serde_json::to_string_pretty(&report).map_err(|error| error.to_string());', '        return runtime.read_text();')
start = s.index('pub fn export_user_logs(')
end = s.index('fn log_to_user_log', start)
s = s[:start] + '''pub fn export_user_logs(app: &AppHandle, _profile_state: &AppProfileState) -> Result<String, String> {
    let runtime = logs::diagnostic::active().ok_or("diagnostics.errors.unavailable")?;
    super::report::export(&runtime, app).map(|path| path.unwrap_or_default())
}

pub fn export_logs_bundle(app: &AppHandle, _profile_state: &AppProfileState) -> Result<Option<String>, String> {
    let runtime = logs::diagnostic::active().ok_or("diagnostics.errors.unavailable")?;
    super::report::export(&runtime, app)
}

''' + s[end:]
write(p, s)

p = root / 'src-tauri/src/shared/logs.rs'
s = p.read_text(encoding='utf-8').replace('ets2_tool_debug.log', 'application.log.txt')
s = s.replace('format!("{}.{}", value, index)', 'format!("{}.{}.txt", value.trim_end_matches(".txt"), index)')
s = s[:s.index('pub fn write_log(msg: String)')] + '''pub fn write_log(msg: String) {
    if let Some(runtime) = diagnostic::active() { runtime.legacy("DEBUG", "backend", &msg); return; }
    // Before logger initialization, preserve real errors without flooding startup with trace data.
    if !msg.starts_with("[trace] ERROR") { return; }
    let entry = format!("[{}] [ERROR] [backend] {}\\n", Local::now().format("%Y-%m-%d %H:%M:%S"), diagnostic::sanitize(&msg));
    let _ = append_log_line(&technical_log_path(), &entry);
}
'''
write(p, s)
p = root / 'src-tauri/src/shared/user_log.rs'
s = p.read_text(encoding='utf-8').replace('ets2_tool_user.log', 'application.log.txt').replace('ets2_tool_errors.log', 'application.log.txt')
start = s.index('    match normalized_level.as_str() {')
end = s.index('\n    Ok(())', start)
s = s[:start] + '    if normalized_level != "DEBUG" { logs::append_log_line(&user_log_path(), &entry)?; }\n' + s[end:]
s = s.replace('runtime.root.join("logs/application.log")', 'runtime.root.join(logs::diagnostic::LOG_FILE)').replace('runtime.root.join("logs/events.jsonl")', 'runtime.root.join(logs::diagnostic::LOG_FILE)')
write(p, s)
p = root / 'src-tauri/src/features/language/translator.rs'
s = p.read_text(encoding='utf-8').replace('    println!("{}", line);\n    crate::dev_log!("{}", line);', '    crate::dev_log!("{}", line);')
write(p, s)
