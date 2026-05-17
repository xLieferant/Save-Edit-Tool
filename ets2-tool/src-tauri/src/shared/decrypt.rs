use crate::dev_log;
use crate::shared::trace::{lock_mutex, log_lock_acquired, log_lock_released, log_lock_wait, TraceScope};
use crate::state::{DecryptCache, InFlightDecrypt, InFlightState};
use decrypt_truck::decrypt_bin_file;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc;
use std::time::Duration;
use tauri::State;

const SIG_PLAINTEXT_SII: u32 = 1315531091;
const SIG_ENCRYPTED_AES: u32 = 1131635539;
const SIG_BSII: u32 = 1229542210;
const SIG_3NK: u32 = 21720627;
const DECRYPT_TIMEOUT_SECS: u64 = 20;
const INFLIGHT_WAIT_TIMEOUT_SECS: u64 = 20;
const MAX_DECRYPT_FILE_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodeProbe {
    pub signature: String,
    pub header_hex: String,
    pub suggested_tool: String,
}

fn probe_signature(bytes: &[u8]) -> DecodeProbe {
    let header = bytes
        .iter()
        .take(8)
        .map(|byte| format!("{:02X}", byte))
        .collect::<Vec<_>>()
        .join(" ");
    let signature = if bytes.starts_with(b"SiiNunit") {
        "plain_text_sii"
    } else if bytes.len() >= 4 {
        let value = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        match value {
            SIG_PLAINTEXT_SII => "plain_text_sii",
            SIG_ENCRYPTED_AES => "aes_text",
            SIG_BSII => "bsii",
            SIG_3NK => "3nk",
            _ if !bytes.iter().any(|byte| *byte == 0) => "utf8_text",
            _ => "unknown_binary",
        }
    } else if !bytes.iter().any(|byte| *byte == 0) {
        "utf8_text"
    } else {
        "unknown_binary"
    };

    DecodeProbe {
        signature: signature.to_string(),
        header_hex: header,
        suggested_tool: "decrypt_truck".to_string(),
    }
}

fn format_decode_error(
    source: &str,
    probe: &DecodeProbe,
    error: &str,
    provenance_missing: &[String],
) -> String {
    let mut parts = vec![
        format!("source={}", source),
        format!("signature={}", probe.signature),
        format!("header_hex={}", probe.header_hex),
        format!("suggested_tool={}", probe.suggested_tool),
        format!("reason={}", error),
    ];
    if !provenance_missing.is_empty() {
        parts.push(format!(
            "provenance_missing=[{}]",
            provenance_missing.join(", ")
        ));
    }
    format!("decode_failed | {}", parts.join(" | "))
}

pub fn detect_signature(bytes: &[u8]) -> DecodeProbe {
    probe_signature(bytes)
}

pub fn decode_bytes(bytes: &[u8], source: &str) -> Result<Vec<u8>, String> {
    let probe = probe_signature(bytes);
    match probe.signature.as_str() {
        "plain_text_sii" | "utf8_text" => Ok(bytes.to_vec()),
        "aes_text" | "bsii" | "3nk" | "unknown_binary" => decrypt_bin_file(&bytes.to_vec())
            .map_err(|error| format_decode_error(source, &probe, &error, &[])),
        _ => Err(format_decode_error(
            source,
            &probe,
            "unsupported_signature",
            &[],
        )),
    }
}

pub fn decode_text_bytes(
    bytes: &[u8],
    source: &str,
    provenance_missing: &[String],
) -> Result<String, String> {
    let probe = probe_signature(bytes);
    let decoded = match probe.signature.as_str() {
        "plain_text_sii" | "utf8_text" => bytes.to_vec(),
        "aes_text" | "bsii" | "3nk" | "unknown_binary" => decrypt_bin_file(&bytes.to_vec())
            .map_err(|error| format_decode_error(source, &probe, &error, provenance_missing))?,
        _ => {
            return Err(format_decode_error(
                source,
                &probe,
                "unsupported_signature",
                provenance_missing,
            ));
        }
    };

    String::from_utf8(decoded).map_err(|error| {
        format_decode_error(
            source,
            &probe,
            &format!("utf8_validation_failed: {}", error),
            provenance_missing,
        )
    })
}

pub fn decrypt_if_needed(path: &Path) -> Result<String, String> {
    let path_buf = path.to_path_buf();
    let mut trace = TraceScope::with_fields(
        "decrypt_if_needed",
        &[("path", path_buf.display().to_string())],
    );
    let result = run_decrypt_with_timeout(path_buf.clone());
    if let Err(error) = result.as_ref() {
        trace.finish_error(error);
        return Err(error.clone());
    }
    trace.finish_ok();
    result
}

pub fn decrypt_cached(path: &Path, cache: &State<DecryptCache>) -> Result<String, String> {
    decrypt_cached_with_cache(path, cache.inner())
}

pub fn decrypt_cached_with_cache(path: &Path, cache: &DecryptCache) -> Result<String, String> {
    if let Some(value) = cached_content(cache, path)? {
        return Ok(value);
    }

    let path_buf = path.to_path_buf();
    let entry = acquire_inflight_entry(cache, &path_buf)?;
    match entry {
        InflightRole::Follower(entry) => wait_for_inflight_result(&path_buf, &entry),
        InflightRole::Leader(entry) => {
            let result = decrypt_if_needed(path);
            if let Ok(content) = result.as_ref() {
                insert_cached_content(cache, &path_buf, content.clone())?;
            }
            complete_inflight(cache, &path_buf, &entry, result.clone())?;
            result
        }
    }
}

pub fn clear_decrypt_cache(cache: &DecryptCache) -> Result<(), String> {
    let mut guard = lock_mutex("decrypt_cache.files", &cache.files)?;
    guard.clear();
    Ok(())
}

pub fn backup_file(path: &Path) -> Result<(), String> {
    let backup_path = path.with_extension("bak");
    fs::copy(path, &backup_path).map_err(|e| e.to_string())?;
    dev_log!("Backup erstellt: {}", backup_path.display());
    Ok(())
}

enum InflightRole {
    Leader(Arc<InFlightDecrypt>),
    Follower(Arc<InFlightDecrypt>),
}

fn run_decrypt_with_timeout(path: PathBuf) -> Result<String, String> {
    let metadata = fs::metadata(&path).map_err(|error| {
        format!(
            "decode_failed | source={} | reason=metadata_failed: {}",
            path.display(),
            error
        )
    })?;

    if metadata.len() > MAX_DECRYPT_FILE_BYTES {
        return Err(format!(
            "decode_failed | source={} | reason=file_too_large: {} bytes",
            path.display(),
            metadata.len()
        ));
    }

    let (sender, receiver) = mpsc::channel();
    std::thread::Builder::new()
        .name("decrypt_if_needed".to_string())
        .spawn({
            let path = path.clone();
            move || {
                let result = decrypt_path(&path);
                let _ = sender.send(result);
            }
        })
        .map_err(|error| format!("decrypt_worker_spawn_failed | source={} | reason={}", path.display(), error))?;

    receiver.recv_timeout(Duration::from_secs(DECRYPT_TIMEOUT_SECS)).map_err(|_| {
        format!(
            "decode_failed | source={} | reason=timeout_after_{}s",
            path.display(),
            DECRYPT_TIMEOUT_SECS
        )
    })?
}

fn decrypt_path(path: &Path) -> Result<String, String> {
    let bytes = fs::read(path).map_err(|error| {
        format!(
            "decode_failed | source={} | reason=read_failed: {}",
            path.display(),
            error
        )
    })?;
    decode_text_bytes(&bytes, &path.display().to_string(), &[])
}

fn cached_content(cache: &DecryptCache, path: &Path) -> Result<Option<String>, String> {
    let guard = lock_mutex("decrypt_cache.files", &cache.files)?;
    Ok(guard.get(path).cloned())
}

fn insert_cached_content(
    cache: &DecryptCache,
    path: &Path,
    content: String,
) -> Result<(), String> {
    let mut guard = lock_mutex("decrypt_cache.files", &cache.files)?;
    guard.insert(path.to_path_buf(), content);
    Ok(())
}

fn acquire_inflight_entry(cache: &DecryptCache, path: &PathBuf) -> Result<InflightRole, String> {
    let mut guard = lock_mutex("decrypt_cache.inflight", &cache.inflight)?;
    if let Some(entry) = guard.get(path).cloned() {
        return Ok(InflightRole::Follower(entry));
    }

    let entry = Arc::new(InFlightDecrypt::default());
    guard.insert(path.clone(), entry.clone());
    Ok(InflightRole::Leader(entry))
}

fn wait_for_inflight_result(path: &Path, entry: &Arc<InFlightDecrypt>) -> Result<String, String> {
    let name = format!("decrypt_cache.inflight_entry path={}", path.display());
    log_lock_wait(&name);
    let mut guard = entry
        .state
        .lock()
        .map_err(|_| format!("{} lock poisoned", name))?;
    log_lock_acquired(&name);

    loop {
        match &*guard {
            InFlightState::Ready(result) => {
                let result = result.clone();
                log_lock_released(&name);
                drop(guard);
                return result;
            }
            InFlightState::Pending => {
                let wait_result = entry
                    .condvar
                    .wait_timeout(guard, Duration::from_secs(INFLIGHT_WAIT_TIMEOUT_SECS))
                    .map_err(|_| format!("{} wait poisoned", name))?;
                guard = wait_result.0;
                if wait_result.1.timed_out() {
                    log_lock_released(&name);
                    drop(guard);
                    return Err(format!(
                        "decrypt_if_needed wait timed out after {}s for {}",
                        INFLIGHT_WAIT_TIMEOUT_SECS,
                        path.display()
                    ));
                }
            }
        }
    }
}

fn complete_inflight(
    cache: &DecryptCache,
    path: &Path,
    entry: &Arc<InFlightDecrypt>,
    result: Result<String, String>,
) -> Result<(), String> {
    {
        let name = format!("decrypt_cache.inflight_entry path={}", path.display());
        log_lock_wait(&name);
        let mut state = entry
            .state
            .lock()
            .map_err(|_| format!("{} lock poisoned", name))?;
        log_lock_acquired(&name);
        *state = InFlightState::Ready(result);
        entry.condvar.notify_all();
        log_lock_released(&name);
    }

    let mut inflight = lock_mutex("decrypt_cache.inflight", &cache.inflight)?;
    inflight.remove(path);
    Ok(())
}

pub fn modify_block(
    path: &Path,
    block_name: &str,
    updater: impl Fn(&str) -> String,
) -> Result<(), String> {
    backup_file(path)?;
    let content = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let re = regex::Regex::new(&format!(
        r"{}s*:\s*[A-Za-z0-9._]+\s*\{{([\s\S]*?)\}}",
        block_name
    ))
    .map_err(|e| e.to_string())?;
    let new_content = re
        .replace(&content, |caps: &regex::Captures| updater(&caps[1]))
        .to_string();

    let tmp_path = path.with_extension("tmp");
    fs::write(&tmp_path, &new_content).map_err(|e| e.to_string())?;
    fs::rename(tmp_path, path).map_err(|e| e.to_string())?;

    dev_log!(
        "Block '{}' erfolgreich modifiziert: {}",
        block_name,
        path.display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{decode_text_bytes, detect_signature};

    #[test]
    fn detect_signature_for_plain_and_encrypted_fixtures() {
        let plain = include_bytes!("../../test-fixtures/decrypt/plain_game.sii");
        let encrypted = include_bytes!("../../test-fixtures/decrypt/encrypted_game.sii");
        assert_eq!(detect_signature(plain).signature, "plain_text_sii");
        assert_eq!(detect_signature(encrypted).signature, "aes_text");
    }

    #[test]
    fn decode_pipeline_returns_deterministic_text_for_encrypted_fixture() {
        let encrypted = include_bytes!("../../test-fixtures/decrypt/encrypted_game.sii");
        let decoded = decode_text_bytes(encrypted, "encrypted_fixture", &[]).unwrap();
        assert!(decoded.starts_with("SiiNunit"));
        assert!(decoded.contains("company"));
    }
}
