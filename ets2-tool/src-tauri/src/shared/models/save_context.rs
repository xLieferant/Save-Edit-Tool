use std::hash::{Hash, Hasher};

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
    let mut normalized = value.trim().replace('\\', "/");
    while normalized.ends_with('/') {
        let lower = normalized.to_ascii_lowercase();
        if lower.ends_with(":/") || normalized == "/" {
            break;
        }
        normalized.pop();
    }
    normalized
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

    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    normalize_reference(profile_reference).hash(&mut hasher);
    normalize_reference(save_reference).hash(&mut hasher);

    Some(format!("savectx-{:016x}", hasher.finish()))
}
