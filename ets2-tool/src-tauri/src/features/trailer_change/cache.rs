use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use chrono::Utc;

use super::models::{OwnedTrailerDiagnostics, TrailerChangeSession, TrailerInventoryItem};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TrailerChangeSessionCacheKey {
    pub profile_id: String,
    pub save_path: PathBuf,
    pub save_hash: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CurrentTrailerCacheEntry {
    pub profile_id: String,
    pub save_path: PathBuf,
    pub save_hash: String,
    pub trailer_id: String,
    pub trailer: TrailerInventoryItem,
    pub owned_trailers: Vec<TrailerInventoryItem>,
    pub diagnostics: Option<OwnedTrailerDiagnostics>,
    pub loaded_at: String,
}

impl CurrentTrailerCacheEntry {
    pub fn from_session(
        profile_id: String,
        save_path: PathBuf,
        session: &TrailerChangeSession,
    ) -> Self {
        Self {
            profile_id,
            save_path,
            save_hash: session.save_hash.clone(),
            trailer_id: session.current_trailer.trailer_id.clone(),
            trailer: session.current_trailer.clone(),
            owned_trailers: session.owned_trailers.clone(),
            diagnostics: session.diagnostics.clone(),
            loaded_at: Utc::now().to_rfc3339(),
        }
    }
}

#[derive(Default)]
pub struct TrailerChangeSessionCache {
    entries: Mutex<HashMap<TrailerChangeSessionCacheKey, CurrentTrailerCacheEntry>>,
}

impl TrailerChangeSessionCache {
    pub fn get(
        &self,
        profile_id: &str,
        save_path: &Path,
        save_hash: &str,
    ) -> Option<CurrentTrailerCacheEntry> {
        let key = TrailerChangeSessionCacheKey {
            profile_id: profile_id.to_string(),
            save_path: save_path.to_path_buf(),
            save_hash: save_hash.to_string(),
        };
        self.entries.lock().unwrap().get(&key).cloned()
    }

    pub fn store(&self, entry: CurrentTrailerCacheEntry) {
        let key = TrailerChangeSessionCacheKey {
            profile_id: entry.profile_id.clone(),
            save_path: entry.save_path.clone(),
            save_hash: entry.save_hash.clone(),
        };
        self.entries.lock().unwrap().insert(key, entry);
    }

    pub fn invalidate_save(&self, profile_id: &str, save_path: &Path) {
        self.entries
            .lock()
            .unwrap()
            .retain(|key, _| key.profile_id != profile_id || key.save_path != save_path);
    }
}
