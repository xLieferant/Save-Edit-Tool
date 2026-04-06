use std::fs;
use std::hash::{Hash, Hasher};
use std::path::Path;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct SaveContext {
    pub profile_reference: Option<String>,
    pub save_reference: Option<String>,
    pub quicksave_reference: Option<String>,
    pub save_session_id: Option<String>,
}

impl SaveContext {
    pub fn is_ready(&self) -> bool {
        self.profile_reference.is_some() && self.save_reference.is_some()
    }

    pub fn from_paths(profile_reference: Option<String>, save_reference: Option<String>) -> Self {
        let normalized_profile = profile_reference.map(|value| normalize_reference(&value));
        let normalized_save = save_reference.map(|value| normalize_reference(&value));
        let quicksave_reference = normalized_save
            .as_deref()
            .and_then(detect_quicksave_reference);
        let save_session_id =
            build_save_session_id(normalized_profile.as_deref(), normalized_save.as_deref());

        Self {
            profile_reference: normalized_profile,
            save_reference: normalized_save,
            quicksave_reference,
            save_session_id,
        }
    }
}

pub fn normalize_reference(value: &str) -> String {
    value.trim().replace('\\', "/")
}

pub fn detect_quicksave_reference(save_reference: &str) -> Option<String> {
    let normalized = normalize_reference(save_reference);
    if normalized.to_ascii_lowercase().contains("quicksave") {
        Some(normalized)
    } else {
        None
    }
}

pub fn build_save_session_id(
    profile_reference: Option<&str>,
    save_reference: Option<&str>,
) -> Option<String> {
    let profile_reference = profile_reference?;
    let save_reference = save_reference?;
    let modified_token = fs::metadata(Path::new(save_reference))
        .ok()
        .and_then(|metadata| metadata.modified().ok())
        .and_then(|modified| modified.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs().to_string())
        .unwrap_or_else(|| "0".to_string());

    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    normalize_reference(profile_reference).hash(&mut hasher);
    normalize_reference(save_reference).hash(&mut hasher);
    modified_token.hash(&mut hasher);

    Some(format!("savectx-{:016x}", hasher.finish()))
}
