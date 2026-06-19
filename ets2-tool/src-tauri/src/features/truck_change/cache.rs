use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use chrono::Utc;

use super::models::{OwnedTruckDiagnostics, TruckChangeSession, TruckInventoryItem};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TruckChangeSessionCacheKey {
    pub profile_id: String,
    pub save_path: PathBuf,
    pub save_hash: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CurrentTruckCacheEntry {
    pub profile_id: String,
    pub save_path: PathBuf,
    pub save_hash: String,
    pub truck_id: String,
    pub truck: TruckInventoryItem,
    pub owned_trucks: Vec<TruckInventoryItem>,
    pub diagnostics: Option<OwnedTruckDiagnostics>,
    pub loaded_at: String,
}

impl CurrentTruckCacheEntry {
    pub fn from_session(
        profile_id: String,
        save_path: PathBuf,
        session: &TruckChangeSession,
    ) -> Self {
        Self {
            profile_id,
            save_path,
            save_hash: session.save_hash.clone(),
            truck_id: session.current_truck.truck_id.clone(),
            truck: session.current_truck.clone(),
            owned_trucks: session.owned_trucks.clone(),
            diagnostics: session.diagnostics.clone(),
            loaded_at: Utc::now().to_rfc3339(),
        }
    }
}

#[derive(Default)]
pub struct TruckChangeSessionCache {
    entries: Mutex<HashMap<TruckChangeSessionCacheKey, CurrentTruckCacheEntry>>,
}

impl TruckChangeSessionCache {
    pub fn get(
        &self,
        profile_id: &str,
        save_path: &Path,
        save_hash: &str,
    ) -> Option<CurrentTruckCacheEntry> {
        let key = TruckChangeSessionCacheKey {
            profile_id: profile_id.to_string(),
            save_path: save_path.to_path_buf(),
            save_hash: save_hash.to_string(),
        };
        self.entries.lock().unwrap().get(&key).cloned()
    }

    pub fn store(&self, entry: CurrentTruckCacheEntry) {
        let key = TruckChangeSessionCacheKey {
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

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{CurrentTruckCacheEntry, TruckChangeSessionCache};
    use crate::features::truck_change::models::TruckInventoryItem;

    fn item(id: &str) -> TruckInventoryItem {
        TruckInventoryItem {
            truck_id: id.to_string(),
            display_index: 1,
            brand: None,
            model: None,
            raw_license_plate: None,
            display_license_plate: None,
            license_plate: None,
            garage_id: None,
            garage_display_name: None,
            assigned_garage: None,
            assigned_driver_id: None,
            driver_display_name: None,
            country_code: None,
            country_display_name: None,
            is_active: true,
            is_switchable: true,
            blocked_reason: None,
            requires_driver_swap: false,
            engine_data_path: None,
            transmission_data_path: None,
            accessory_count: 0,
        }
    }

    fn entry(profile_id: &str, save_path: &str, save_hash: &str) -> CurrentTruckCacheEntry {
        let truck = item("_nameless.truck.active");
        CurrentTruckCacheEntry {
            profile_id: profile_id.to_string(),
            save_path: Path::new(save_path).to_path_buf(),
            save_hash: save_hash.to_string(),
            truck_id: truck.truck_id.clone(),
            truck: truck.clone(),
            owned_trucks: vec![truck],
            diagnostics: None,
            loaded_at: "2026-06-18T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn cache_key_uses_profile_save_path_and_save_hash() {
        let cache = TruckChangeSessionCache::default();
        cache.store(entry("profile-a", "game.sii", "hash-a"));

        assert!(
            cache
                .get("profile-a", Path::new("game.sii"), "hash-a")
                .is_some()
        );
        assert!(
            cache
                .get("profile-b", Path::new("game.sii"), "hash-a")
                .is_none()
        );
        assert!(
            cache
                .get("profile-a", Path::new("other_game.sii"), "hash-a")
                .is_none()
        );
        assert!(
            cache
                .get("profile-a", Path::new("game.sii"), "hash-b")
                .is_none()
        );
    }

    #[test]
    fn changed_hash_discards_stale_cache_entry() {
        let cache = TruckChangeSessionCache::default();
        cache.store(entry("profile-a", "game.sii", "hash-a"));

        assert!(
            cache
                .get("profile-a", Path::new("game.sii"), "hash-b")
                .is_none()
        );
    }
}
