//! Local TXT logging. Producers never wait for disk IO; one bounded writer preserves order.
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

thread_local! { static OPERATION: RefCell<Option<String>> = const { RefCell::new(None) }; }

pub struct OperationContext(Option<String>);
impl OperationContext {
    pub fn enter(id: Option<String>) -> Self {
        Self(OPERATION.with(|slot| slot.replace(id)))
    }
}
impl Drop for OperationContext {
    fn drop(&mut self) {
        OPERATION.with(|slot| {
            slot.replace(self.0.take());
        });
    }
}

pub fn with_operation<T>(
    module: &str,
    action: &str,
    work: impl FnOnce() -> Result<T, String>,
) -> Result<T, String> {
    let id = OPERATION
        .with(|slot| slot.borrow().clone())
        .unwrap_or_else(|| format!("OP-{}", Uuid::new_v4()));
    let _context = OperationContext::enter(Some(id));
    if let Some(runtime) = active() {
        runtime.record(DiagnosticEvent::new(
            "INFO",
            module,
            action,
            "Operation started.",
        ));
    }
    let result = work();
    if let Some(runtime) = active() {
        let expected = result.as_ref().err().is_some_and(|e| expected_error(e));
        let mut event = DiagnosticEvent::new(
            if result.is_ok() {
                "INFO"
            } else if expected {
                "WARN"
            } else {
                "ERROR"
            },
            module,
            action,
            if result.is_ok() {
                "Operation completed."
            } else {
                "Operation failed."
            },
        );
        if let Err(error) = &result {
            event.technical = Some(error.clone());
        }
        event.outcome = Some(
            if result.is_ok() {
                "completed"
            } else {
                "failed; file state unavailable"
            }
            .into(),
        );
        runtime.record(event);
    }
    result
}

pub fn expected_error(error: &str) -> bool {
    [
        "no_profile_selected",
        "no_save_selected",
        "garage_mutation_in_progress",
        "save_changed_since_preview",
        "owned_trailers_missing",
        "active_trailer_not_found",
        "Kein Save angegeben",
        "Save nicht gefunden",
        "Select a custom ETS2 data location first.",
        "The custom ETS2 data location is empty.",
    ]
    .iter()
    .any(|prefix| error.starts_with(prefix))
}

pub fn verified_write(
    module: &str,
    action: &str,
    backup_id: Option<String>,
    modified: bool,
    rollback: bool,
) {
    if let Some(runtime) = active() {
        let mut event = DiagnosticEvent::new(
            if rollback { "WARN" } else { "INFO" },
            module,
            action,
            if rollback {
                "Original content restored and verified."
            } else {
                "Written content verified."
            },
        );
        event.backup_id = backup_id;
        event.original_modified = Some(modified);
        event.rollback_verified = Some(rollback);
        event.outcome = Some(
            if rollback {
                "rolled_back_and_verified"
            } else {
                "written_and_verified"
            }
            .into(),
        );
        runtime.record(event);
    }
}

#[path = "diagnostic_storage.rs"]
pub mod storage;
pub const LOG_FILE: &str = "logs/application.log.txt";
pub const FILE_LIMIT: u64 = 5 * 1024 * 1024;
pub const TOTAL_LIMIT: u64 = 20 * 1024 * 1024;
pub const RECENT_LIMIT: usize = 500;
pub const TEXT_LIMIT: usize = 8192;

pub struct LogTextSnapshot {
    pub text: String,
    pub size_bytes: u64,
    pub modified: Option<std::time::SystemTime>,
    pub revision: String,
}
static ACTIVE: Lazy<Mutex<Weak<DiagnosticRuntime>>> = Lazy::new(|| Mutex::new(Weak::new()));
static LEGACY_LOGGER: DiagnosticLog = DiagnosticLog;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
pub struct DiagnosticEvent {
    pub timestamp: String,
    pub level: String,
    pub module: String,
    pub action: String,
    pub message: String,
    pub technical: Option<String>,
    pub error_id: Option<String>,
    pub session_id: String,
    pub operation_id: Option<String>,
    pub error_kind: Option<String>,
    pub causes: Vec<String>,
    pub stack: Option<String>,
    pub outcome: Option<String>,
    pub backup_id: Option<String>,
    pub original_modified: Option<bool>,
    pub rollback_verified: Option<bool>,
    pub context: BTreeMap<String, String>,
}

impl DiagnosticEvent {
    pub fn new(level: &str, module: &str, action: &str, message: &str) -> Self {
        Self {
            level: level.into(),
            module: module.into(),
            action: action.into(),
            message: message.into(),
            ..Self::default()
        }
    }
}

enum WriterMessage {
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
        event.stack = Some(sanitize(
            &std::backtrace::Backtrace::force_capture().to_string(),
        ));
        event.operation_id = OPERATION.with(|slot| slot.try_borrow().ok().and_then(|s| s.clone()));
        if let Some(runtime) = active() {
            runtime.record_panic(event);
        } else {
            // This also covers failures before Tauri setup. Never write into the installation.
            let root = dirs::data_local_dir()
                .unwrap_or_else(std::env::temp_dir)
                .join("com.xlieferant.saveedittool")
                .join("support");
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
            root,
            session_id: format!("SESSION-{}", Uuid::new_v4()),
            recent: Mutex::new(VecDeque::new()),
            storage_error: Mutex::new(None),
            dropped: AtomicU64::new(0),
            maintenance: Mutex::new(()),
            sender,
            stopped: AtomicBool::new(false),
            dedup: Mutex::new(VecDeque::new()),
        });
        if publish {
            if let Ok(mut slot) = ACTIVE.lock() {
                *slot = Arc::downgrade(&runtime);
            }
            let _ = log::set_logger(&LEGACY_LOGGER);
            log::set_max_level(log::LevelFilter::Info);
        }
        let prepared = (|| {
            for name in ["logs/archive", "cache"] {
                fs::create_dir_all(checked_path(&runtime.root, Path::new(name))?)
                    .map_err(|e| e.to_string())?;
            }
            runtime.recover_emergencies()?;
            storage::automatic(&runtime)
        })();
        if let Err(error) = prepared {
            runtime.set_error(error);
        }
        let weak = Arc::downgrade(&runtime);
        if let Err(error) = std::thread::Builder::new()
            .name("application-log-writer".into())
            .spawn(move || {
                while let Ok(message) = receiver.recv() {
                    let Some(runtime) = weak.upgrade() else {
                        break;
                    };
                    match message {
                        WriterMessage::Event(event) => {
                            if let Err(error) = runtime.persist(&event) {
                                runtime.set_error(error);
                            }
                        }
                        WriterMessage::Flush(done) => {
                            let _ = done.send(());
                        }
                        WriterMessage::Stop(done) => {
                            let _ = done.send(());
                            break;
                        }
                    }
                }
            })
        {
            runtime.set_error(error.to_string());
        }
        runtime.record(DiagnosticEvent::new(
            "INFO",
            "application",
            "startup",
            &format!("ETS2 TOOLBOX started. Version: {}", sanitize(&version)),
        ));
        runtime
    }

    pub fn set_error(&self, error: String) {
        if let Ok(mut slot) = self.storage_error.lock() {
            *slot = Some(sanitize(&error));
        }
    }

    fn prepare_event(&self, mut event: DiagnosticEvent) -> Option<DiagnosticEvent> {
        event.level = match event.level.to_ascii_uppercase().as_str() {
            "INFO" => "INFO",
            "WARN" | "WARNING" => "WARN",
            "ERROR" | "FATAL" => "ERROR",
            _ => return None,
        }
        .into();
        event.timestamp = Local::now().to_rfc3339_opts(SecondsFormat::Millis, false);
        event.session_id = self.session_id.clone();
        event.operation_id = event
            .operation_id
            .or_else(|| OPERATION.with(|s| s.try_borrow().ok().and_then(|v| v.clone())));
        event.operation_id = event.operation_id.filter(|s| {
            s.starts_with("OP-")
                && s.len() <= 80
                && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        });
        event.error_id = (event.level != "INFO").then(|| format!("ERR-{}", Uuid::new_v4()));
        // Headers stay on one line; all text passes redaction before memory or disk storage.
        event.module = bounded(&sanitize(&event.module).replace(['\r', '\n'], " "), 100);
        event.action = bounded(&sanitize(&event.action).replace(['\r', '\n'], " "), 150);
        event.message = sanitize(&event.message);
        event.technical = event.technical.as_deref().map(sanitize);
        event.stack = event.stack.as_deref().map(sanitize);
        event.causes = event
            .causes
            .iter()
            .take(8)
            .map(|s| bounded(&sanitize(s), 1024))
            .collect();
        event.error_kind = event.error_kind.as_deref().map(sanitize);
        event.outcome = event.outcome.as_deref().map(sanitize);
        event.backup_id = event.backup_id.as_deref().map(sanitize);
        event.context.retain(|key, _| {
            [
                "fileName",
                "line",
                "column",
                "backupType",
                "fileCount",
                "durationMs",
                "step",
            ]
            .contains(&key.as_str())
        });
        for value in event.context.values_mut() {
            *value = bounded(&sanitize(value), 512);
        }
        Some(event)
    }

    pub fn record(&self, event: DiagnosticEvent) -> Option<String> {
        if self.stopped.load(Ordering::Relaxed) {
            return None;
        }
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
        } else {
            format!(
                "{}|{}|{}|{}",
                event.level, event.module, event.action, phase
            )
        };
        let now = Instant::now();
        dedup.retain(|(_, time, _)| now.duration_since(*time) < Duration::from_secs(5));
        if let Some((_, _, id)) = dedup.iter().find(|(key, _, _)| key == &fingerprint) {
            return id.clone();
        }
        let id = event.error_id.clone();
        match self
            .sender
            .try_send(WriterMessage::Event(Box::new(event.clone())))
        {
            Ok(()) => {
                if dedup.len() >= 128 {
                    dedup.pop_front();
                }
                dedup.push_back((fingerprint, now, id.clone()));
                if let Ok(mut recent) = self.recent.lock() {
                    if recent.len() >= RECENT_LIMIT {
                        recent.pop_front();
                    }
                    recent.push_back(event);
                }
            }
            Err(_) => {
                self.dropped.fetch_add(1, Ordering::Relaxed);
            }
        }
        id
    }

    pub fn legacy(&self, level: &str, module: &str, message: &str) {
        static FAILURE: Lazy<Regex> =
            Lazy::new(|| Regex::new(r"(?i)\b(failed|error|fehler|panic)\b").unwrap());
        static WARNING: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)\b(warn|warning)\b").unwrap());
        let level = if level == "DEBUG" && FAILURE.is_match(message) {
            "ERROR"
        } else if level == "DEBUG" && WARNING.is_match(message) {
            "WARN"
        } else {
            level
        };
        self.record(DiagnosticEvent::new(level, module, "legacy_event", message));
    }

    fn recover_emergencies(&self) -> Result<(), String> {
        let directory = checked_path(&self.root, Path::new("logs/archive"))?;
        let mut emergencies = fs::read_dir(directory)
            .map_err(|e| e.to_string())?
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| {
                p.file_name()
                    .and_then(|s| s.to_str())
                    .is_some_and(|s| s.starts_with("emergency-") && s.ends_with(".txt"))
            })
            .collect::<Vec<_>>();
        emergencies.sort();
        for path in emergencies.into_iter().take(100) {
            checked_path(
                &self.root,
                path.strip_prefix(&self.root)
                    .map_err(|_| "diagnostics.errors.unsafe_path")?,
            )?;
            let file = File::open(&path).map_err(|e| e.to_string())?;
            if file.metadata().map_err(|e| e.to_string())?.len() > 128 * 1024 {
                continue;
            }
            let mut text = String::new();
            file.take(128 * 1024)
                .read_to_string(&mut text)
                .map_err(|e| e.to_string())?;
            let active = checked_path(&self.root, Path::new(LOG_FILE))?;
            // A restart between sync and removal must not duplicate the recovered panic.
            let existing = fs::read_to_string(&active).unwrap_or_default();
            if !existing.contains(&text) {
                rotate(&self.root, &active, text.len() as u64 + 1)?;
                let mut log = OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(active)
                    .map_err(|e| e.to_string())?;
                log.write_all(b"\n")
                    .and_then(|_| log.write_all(text.as_bytes()))
                    .and_then(|_| log.sync_all())
                    .map_err(|e| e.to_string())?;
            }
            fs::remove_file(path).map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    fn persist(&self, event: &DiagnosticEvent) -> Result<(), String> {
        let _guard = self
            .maintenance
            .lock()
            .map_err(|_| "Log storage lock poisoned")?;
        self.append_locked(event)
    }

    fn append_locked(&self, event: &DiagnosticEvent) -> Result<(), String> {
        let path = checked_path(&self.root, Path::new(LOG_FILE))?;
        let bytes = format_event(event).into_bytes();
        let rotated = rotate(&self.root, &path, bytes.len() as u64)?;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .read(true)
            .open(&path)
            .map_err(|e| e.to_string())?;
        // Preserve a partial last write and separate the next complete entry after a crash.
        if file.metadata().map_err(|e| e.to_string())?.len() > 0 {
            file.seek(SeekFrom::End(-1)).map_err(|e| e.to_string())?;
            let mut last = [0u8];
            file.read_exact(&mut last).map_err(|e| e.to_string())?;
            if last[0] != b'\n' {
                file.write_all(b"\n").map_err(|e| e.to_string())?;
            }
        }
        file.write_all(&bytes).map_err(|e| e.to_string())?;
        if event.level != "INFO" {
            file.sync_data().map_err(|e| e.to_string())?;
        }
        drop(file);
        if rotated {
            storage::automatic(self)?;
        }
        Ok(())
    }

    fn record_panic(&self, event: DiagnosticEvent) {
        let Some(event) = self.prepare_event(event) else {
            return;
        };
        // Do not wait for the writer when the writer itself panicked.
        if std::thread::current().name() != Some("application-log-writer") {
            let _ = self.flush_timeout(Duration::from_millis(500));
        }
        let persisted = self
            .maintenance
            .try_lock()
            .ok()
            .is_some_and(|_guard| self.append_locked(&event).is_ok());
        if !persisted {
            let _ = emergency_write(&self.root, &event);
        }
    }

    fn flush_timeout(&self, timeout: Duration) -> Result<(), String> {
        let (sender, receiver) = mpsc::channel();
        self.sender
            .try_send(WriterMessage::Flush(sender))
            .map_err(|_| "diagnostics.errors.writer_busy")?;
        receiver
            .recv_timeout(timeout)
            .map_err(|_| "diagnostics.errors.writer_busy".to_string())?;
        self.storage_error
            .lock()
            .map_err(|_| "diagnostics.errors.unavailable".to_string())?
            .clone()
            .map_or(Ok(()), Err)
    }
    pub fn flush(&self) -> Result<(), String> {
        self.flush_timeout(Duration::from_secs(5))
    }

    pub fn read_text(&self) -> Result<String, String> {
        self.read_text_snapshot(None)?
            .map(|snapshot| snapshot.text)
            .ok_or_else(|| "diagnostics.errors.unavailable".to_string())
    }

    pub fn read_text_snapshot(
        &self,
        known_revision: Option<&str>,
    ) -> Result<Option<LogTextSnapshot>, String> {
        self.flush()?;
        let _guard = self
            .maintenance
            .lock()
            .map_err(|_| "diagnostics.errors.unavailable")?;
        let path = checked_path(&self.root, Path::new(LOG_FILE))?;
        let file = File::open(path).map_err(|e| e.to_string())?;
        let metadata = file.metadata().map_err(|e| e.to_string())?;
        if metadata.len() > FILE_LIMIT + 128 * 1024 {
            return Err("diagnostics.errors.storage_limit".into());
        }
        let modified = metadata.modified().ok();
        let revision = format!(
            "{}:{:?}:{:?}",
            metadata.len(),
            modified,
            metadata.created().ok()
        );
        if known_revision == Some(revision.as_str()) {
            return Ok(None);
        }
        let mut text = String::new();
        file.take(FILE_LIMIT + 128 * 1024)
            .read_to_string(&mut text)
            .map_err(|e| e.to_string())?;
        Ok(Some(LogTextSnapshot {
            text,
            size_bytes: metadata.len(),
            modified,
            revision,
        }))
    }
    pub fn preview(&self, lines: usize) -> Result<String, String> {
        let text = self.read_text()?;
        let lines = lines.clamp(50, 100);
        let mut tail = text.lines().rev().take(lines).collect::<Vec<_>>();
        tail.reverse();
        Ok(tail.join("\n"))
    }
    pub fn shutdown(&self) {
        if self.stopped.load(Ordering::Relaxed) {
            return;
        }
        let lost = self.dropped.load(Ordering::Relaxed);
        if lost > 0 {
            self.record(DiagnosticEvent::new(
                "WARN",
                "logging",
                "overload",
                &format!(
                    "{} entries dropped because the bounded log queue was full.",
                    lost
                ),
            ));
        }
        self.record(DiagnosticEvent::new(
            "INFO",
            "application",
            "shutdown",
            "Application exited normally.",
        ));
        let _ = self.flush();
        self.stopped.store(true, Ordering::Relaxed);
        let (sender, receiver) = mpsc::channel();
        if self.sender.try_send(WriterMessage::Stop(sender)).is_ok() {
            let _ = receiver.recv_timeout(Duration::from_secs(2));
        }
    }
}

pub fn format_event(event: &DiagnosticEvent) -> String {
    let mut text = format!(
        "[{}] [{}] [{}::{}] {}\nSession: {}",
        event.timestamp, event.level, event.module, event.action, event.message, event.session_id
    );
    if let Some(op) = &event.operation_id {
        text.push_str(&format!(" | Operation: {}", op));
    }
    text.push('\n');
    for (label, value) in [
        ("Reason", &event.technical),
        ("Type", &event.error_kind),
        ("Error", &event.error_id),
        ("Outcome", &event.outcome),
        ("Backup", &event.backup_id),
    ] {
        if let Some(value) = value {
            text.push_str(&format!("{}: {}\n", label, value));
        }
    }
    for cause in &event.causes {
        text.push_str(&format!("Caused by: {}\n", cause));
    }
    for (key, value) in &event.context {
        text.push_str(&format!("{}: {}\n", key, value));
    }
    if let Some(value) = event.original_modified {
        text.push_str(&format!("Original modified: {}\n", value));
    }
    if let Some(value) = event.rollback_verified {
        text.push_str(&format!("Rollback verified: {}\n", value));
    }
    if let Some(stack) = &event.stack {
        text.push_str(&format!("Stack: {}\n", stack));
    }
    text
}

fn emergency_write(root: &Path, event: &DiagnosticEvent) -> Result<(), String> {
    let directory = checked_path(root, Path::new("logs/archive"))?;
    fs::create_dir_all(directory).map_err(|e| e.to_string())?;
    let path = checked_path(
        root,
        &PathBuf::from("logs/archive").join(format!("emergency-{}.txt", Uuid::new_v4())),
    )?;
    atomic_write(&path, format_event(event).as_bytes())
}

fn rotate(root: &Path, path: &Path, additional: u64) -> Result<bool, String> {
    if fs::metadata(path)
        .map(|m| m.len() > 0 && m.len().saturating_add(additional) > FILE_LIMIT)
        .unwrap_or(false)
    {
        let destination = checked_path(
            root,
            &PathBuf::from("logs/archive").join(format!(
                "application-{}-{}.txt",
                Utc::now().format("%Y%m%dT%H%M%S%3f"),
                Uuid::new_v4()
            )),
        )?;
        fs::rename(path, destination).map_err(|e| e.to_string())?;
        return Ok(true);
    }
    Ok(false)
}

struct DiagnosticLog;
impl log::Log for DiagnosticLog {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        metadata.level() <= log::Level::Info
    }
    fn log(&self, record: &log::Record<'_>) {
        if self.enabled(record.metadata()) {
            if let Some(runtime) = active() {
                runtime.legacy(
                    record.level().as_str(),
                    record.target(),
                    &record.args().to_string(),
                );
            }
        }
    }
    fn flush(&self) {
        if let Some(runtime) = active() {
            let _ = runtime.flush();
        }
    }
}

pub fn bounded(value: &str, limit: usize) -> String {
    value.chars().take(limit).collect()
}

pub fn sanitize(value: &str) -> String {
    if value.contains("SiiNunit") || value.contains("ScsC") {
        return "<REDACTED_SAVE_CONTENT>".into();
    }
    redact_log_text(&bounded(value, TEXT_LIMIT))
}

/// Reuse the logger's masks for a complete file without the producer's 8 KiB limit.
pub fn redact_log_text(value: &str) -> String {
    static HOME: Lazy<Regex> = Lazy::new(|| {
        Regex::new(r"(?i)(?:[A-Z]:[/\\](?:Users|Documents and Settings)[/\\][^/\\\s\x22<>]+|/(?:home|Users)/[^/\s\x22<>]+)").unwrap()
    });
    static PATH: Lazy<Regex> = Lazy::new(|| {
        Regex::new(r"(?i)(?:[A-Z]:[/\\]|\\\\[A-Z0-9_.-]+[/\\])[^:\r\n\x22<>|]*").unwrap()
    });
    static UNIX_PATH: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"(?:^|[\s(=\x22])/(?:[^/\s\x22<>]+/)*[^\s\x22<>:]+").unwrap());
    static SECRET: Lazy<Regex> = Lazy::new(|| {
        Regex::new(r"(?i)\b(password(?:_?confirm)?|passwd|passwort|pwd|(?:access[_-]?|refresh[_-]?|auth[_-]?|session[_-]?)?token|api[_-]?key|(?:client[_-]?)?secret|cookie|authorization|credentials?|private[_-]?key|aws_secret_access_key|recovery[_-]?code)\b[\s\x22']*[:=][\s\x22']*[^\r\n,;}]+|\b(?:Bearer|Basic)\s+[A-Za-z0-9._~+/-]+=*|\beyJ[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\b|[a-z][a-z0-9+.-]*://[^\s/@]+:[^\s/@]+@").unwrap()
    });
    static EMAIL: Lazy<Regex> = Lazy::new(|| {
        Regex::new(r"[A-Za-z0-9.!#$%&'*+/=?^_`{|}~-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}").unwrap()
    });
    static PERSONAL: Lazy<Regex> = Lazy::new(|| {
        Regex::new(r"(?i)(?:\b(?:profile|save|player|driver)[ _]?(?:name|reference)?\s*[:=]\s*|Profil gefunden:)[^\r\n,;]+|<USER_HOME>[/\\](?:Documents|OneDrive)[^:\r\n\x22<>|]*").unwrap()
    });
    static PRIVATE_KEY: Lazy<Regex> = Lazy::new(|| {
        Regex::new(r"(?s)-----BEGIN (?:[A-Z]+ )?PRIVATE KEY-----.*?(?:-----END (?:[A-Z]+ )?PRIVATE KEY-----|\z)").unwrap()
    });
    let mut text = PRIVATE_KEY
        .replace_all(value, "<REDACTED_PRIVATE_KEY>")
        .into_owned();
    // Mask URI credentials before a scheme suffix can resemble a Windows drive.
    text = SECRET.replace_all(&text, "<REDACTED_SECRET>").into_owned();
    if let Some(home) = dirs::home_dir().and_then(|p| p.to_str().map(str::to_owned)) {
        for home in [home.clone(), home.replace('\\', "/")] {
            text = text.replace(&home, "<USER_HOME>");
        }
    }
    text = HOME.replace_all(&text, "<USER_HOME>").into_owned();
    text = PATH.replace_all(&text, "<PRIVATE_PATH>").into_owned();
    text = UNIX_PATH.replace_all(&text, " <PRIVATE_PATH>").into_owned();
    text = EMAIL.replace_all(&text, "<PRIVATE_EMAIL>").into_owned();
    text = PERSONAL
        .replace_all(&text, "<PRIVATE_PROFILE_REFERENCE>")
        .into_owned();
    text.chars()
        .filter(|c| !c.is_control() || ['\n', '\r', '\t'].contains(c))
        .collect()
}

/// Reject every link/reparse point in the deletion/export boundary, including ancestors.
pub fn checked_path(root: &Path, relative: &Path) -> Result<PathBuf, String> {
    if !root.is_absolute()
        || relative
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err("diagnostics.errors.unsafe_path".into());
    }
    let path = root.join(relative);
    for ancestor in path.ancestors() {
        if let Ok(metadata) = fs::symlink_metadata(ancestor) {
            if metadata.file_type().is_symlink() || is_reparse(&metadata) {
                return Err("diagnostics.errors.unsafe_path".into());
            }
        }
    }
    Ok(path)
}

#[cfg(windows)]
fn is_reparse(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    metadata.file_attributes() & 0x400 != 0
}
#[cfg(not(windows))]
fn is_reparse(_: &fs::Metadata) -> bool {
    false
}

pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let temporary = path.with_extension(format!("{}.tmp", Uuid::new_v4()));
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|e| e.to_string())?;
        file.write_all(bytes).map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())?;
        drop(file);
        fs::rename(&temporary, path).map_err(|e| e.to_string())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

#[cfg(test)]
#[path = "diagnostic_tests.rs"]
mod tests;
