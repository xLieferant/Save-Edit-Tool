use std::fmt::Display;
use std::ops::{Deref, DerefMut};
use std::sync::{Mutex, MutexGuard};
use std::time::Instant;

pub struct TraceScope {
    name: String,
    start: Instant,
    finished: bool,
}

impl TraceScope {
    pub fn new(name: impl Into<String>) -> Self {
        Self::with_fields(name, &[])
    }

    pub fn with_fields(name: impl Into<String>, fields: &[(&str, String)]) -> Self {
        let name = name.into();
        let suffix = format_fields(fields);
        crate::dev_log!("[trace] START {}{}", name, suffix);
        Self {
            name,
            start: Instant::now(),
            finished: false,
        }
    }

    pub fn finish_ok(&mut self) {
        if self.finished {
            return;
        }
        self.finished = true;
        crate::dev_log!(
            "[trace] END {} duration_ms={}",
            self.name,
            self.start.elapsed().as_millis()
        );
    }

    pub fn finish_error(&mut self, error: impl Display) {
        crate::dev_log!("[trace] ERROR {}: {}", self.name, error);
        self.finish_ok();
    }
}

impl Drop for TraceScope {
    fn drop(&mut self) {
        self.finish_ok();
    }
}

pub struct LoggedMutexGuard<'a, T> {
    name: String,
    guard: MutexGuard<'a, T>,
}

impl<'a, T> Deref for LoggedMutexGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.guard
    }
}

impl<'a, T> DerefMut for LoggedMutexGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.guard
    }
}

impl<'a, T> Drop for LoggedMutexGuard<'a, T> {
    fn drop(&mut self) {
        log_lock_released(&self.name);
    }
}

pub fn lock_mutex<'a, T>(
    name: impl Into<String>,
    mutex: &'a Mutex<T>,
) -> Result<LoggedMutexGuard<'a, T>, String> {
    let name = name.into();
    log_lock_wait(&name);
    let guard = mutex
        .lock()
        .map_err(|_| format!("{} lock poisoned", name))?;
    log_lock_acquired(&name);
    Ok(LoggedMutexGuard { name, guard })
}

pub fn log_lock_wait(name: impl AsRef<str>) {
    crate::dev_log!("[trace] LOCK WAIT name={}", name.as_ref());
}

pub fn log_lock_acquired(name: impl AsRef<str>) {
    crate::dev_log!("[trace] LOCK ACQUIRED name={}", name.as_ref());
}

pub fn log_lock_released(name: impl AsRef<str>) {
    crate::dev_log!("[trace] LOCK RELEASED name={}", name.as_ref());
}

fn format_fields(fields: &[(&str, String)]) -> String {
    if fields.is_empty() {
        String::new()
    } else {
        format!(
            " {}",
            fields
                .iter()
                .map(|(key, value)| format!("{}={}", key, value))
                .collect::<Vec<_>>()
                .join(" ")
        )
    }
}
