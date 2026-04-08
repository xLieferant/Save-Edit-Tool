use crate::dev_log;
use crate::state::DecryptCache;
use decrypt_truck::decrypt_bin_file;
use std::fs;
use std::path::Path;
use tauri::State;

const SIG_PLAINTEXT_SII: u32 = 1315531091;
const SIG_ENCRYPTED_AES: u32 = 1131635539;
const SIG_BSII: u32 = 1229542210;
const SIG_3NK: u32 = 21720627;

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
    dev_log!("decrypt_if_needed: {}", path.display());
    let bytes = fs::read(path).map_err(|error| {
        format!(
            "decode_failed | source={} | reason=read_failed: {}",
            path.display(),
            error
        )
    })?;
    decode_text_bytes(&bytes, &path.display().to_string(), &[])
}

pub fn decrypt_cached(path: &Path, cache: &State<DecryptCache>) -> Result<String, String> {
    if let Some(value) = cache.files.lock().unwrap().get(path).cloned() {
        return Ok(value);
    }

    let content = decrypt_if_needed(path)?;
    cache
        .files
        .lock()
        .unwrap()
        .insert(path.to_path_buf(), content.clone());
    Ok(content)
}

pub fn backup_file(path: &Path) -> Result<(), String> {
    let backup_path = path.with_extension("bak");
    fs::copy(path, &backup_path).map_err(|e| e.to_string())?;
    dev_log!("Backup erstellt: {}", backup_path.display());
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
