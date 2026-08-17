use std::sync::{Arc, Mutex};

use super::models::AiDriverPoolSnapshot;

#[derive(Debug, Clone)]
struct CachedAiDriverPool {
    profile_id: String,
    save_id: String,
    game_sii_path: String,
    modified_token: u128,
    snapshot: AiDriverPoolSnapshot,
}

#[derive(Clone, Default)]
pub struct AiDriverPoolCache {
    current: Arc<Mutex<Option<CachedAiDriverPool>>>,
}

impl AiDriverPoolCache {
    pub fn get(
        &self,
        profile_id: &str,
        save_id: &str,
        game_sii_path: &str,
        modified_token: u128,
    ) -> Option<AiDriverPoolSnapshot> {
        let guard = self.current.lock().ok()?;
        let cached = guard.as_ref()?;
        if cached.profile_id == profile_id
            && cached.save_id == save_id
            && cached.game_sii_path == game_sii_path
            && cached.modified_token == modified_token
        {
            Some(cached.snapshot.clone())
        } else {
            None
        }
    }

    pub fn store(
        &self,
        profile_id: String,
        save_id: String,
        game_sii_path: String,
        modified_token: u128,
        snapshot: AiDriverPoolSnapshot,
    ) {
        if let Ok(mut guard) = self.current.lock() {
            *guard = Some(CachedAiDriverPool {
                profile_id,
                save_id,
                game_sii_path,
                modified_token,
                snapshot,
            });
        }
    }

    pub fn invalidate_all(&self) {
        if let Ok(mut guard) = self.current.lock() {
            *guard = None;
        }
    }

    pub fn invalidate_profile(&self, profile_id: &str) {
        if let Ok(mut guard) = self.current.lock() {
            if guard
                .as_ref()
                .is_some_and(|cached| cached.profile_id == profile_id)
            {
                *guard = None;
            }
        }
    }

    pub fn invalidate_save(&self, profile_id: &str, save_id: &str) {
        if let Ok(mut guard) = self.current.lock() {
            if guard
                .as_ref()
                .is_some_and(|cached| cached.profile_id == profile_id && cached.save_id == save_id)
            {
                *guard = None;
            }
        }
    }
}
