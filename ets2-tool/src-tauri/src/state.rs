use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

use crate::models::global_config_info::BaseGameConfig;
use crate::models::quicksave_game_info::GameDataQuicksave;
use crate::models::save_game_config::SaveGameConfig;
use crate::models::save_game_data::SaveGameData;
use crate::models::trailers::ParsedTrailer;
use crate::models::trucks::ParsedTruck;

#[derive(Default)]
pub struct DecryptCache {
    pub files: Mutex<HashMap<PathBuf, String>>,
}

#[derive(Default)]
struct CachedProfileData {
    profile_path: Option<String>,
    save_path: Option<String>,
    base_config: Option<BaseGameConfig>,
    save_config: Option<SaveGameConfig>,
    save_game_data: Option<SaveGameData>,
    quicksave_data: Option<GameDataQuicksave>,
    all_trucks: Option<Vec<ParsedTruck>>,
    player_truck: Option<ParsedTruck>,
    all_trailers: Option<Vec<ParsedTrailer>>,
    player_trailer: Option<Option<ParsedTrailer>>,
}

pub struct ProfileCache {
    data: Mutex<CachedProfileData>,
}

impl Default for ProfileCache {
    fn default() -> Self {
        Self {
            data: Mutex::new(CachedProfileData::default()),
        }
    }
}

impl ProfileCache {
    fn matches_save_path(data: &CachedProfileData, path: &str) -> bool {
        data.save_path.as_deref() == Some(path)
    }

    pub fn reset_profile(&self, profile_path: Option<String>) {
        let mut guard = self.data.lock().unwrap();
        guard.profile_path = profile_path;
        guard.save_path = None;
        guard.base_config = None;
        guard.save_config = None;
        guard.save_game_data = None;
        guard.quicksave_data = None;
        guard.all_trucks = None;
        guard.player_truck = None;
        guard.all_trailers = None;
        guard.player_trailer = None;
    }

    pub fn set_save_path(&self, save_path: Option<String>) {
        let mut guard = self.data.lock().unwrap();
        guard.save_path = save_path;
        guard.save_game_data = None;
        guard.quicksave_data = None;
        guard.all_trucks = None;
        guard.player_truck = None;
        guard.all_trailers = None;
        guard.player_trailer = None;
    }

    pub fn cache_save_game_data(&self, path: String, value: SaveGameData) {
        let mut guard = self.data.lock().unwrap();
        guard.save_path = Some(path);
        guard.save_game_data = Some(value);
    }

    pub fn get_save_game_data(&self, path: &str) -> Option<SaveGameData> {
        let guard = self.data.lock().unwrap();
        if Self::matches_save_path(&guard, path) {
            guard.save_game_data.clone()
        } else {
            None
        }
    }

    pub fn cache_quicksave_data(&self, path: String, value: GameDataQuicksave) {
        let mut guard = self.data.lock().unwrap();
        guard.save_path = Some(path);
        guard.quicksave_data = Some(value);
    }

    pub fn get_quicksave_data(&self, path: &str) -> Option<GameDataQuicksave> {
        let guard = self.data.lock().unwrap();
        if Self::matches_save_path(&guard, path) {
            guard.quicksave_data.clone()
        } else {
            None
        }
    }

    pub fn cache_base_config(&self, value: BaseGameConfig) {
        let mut guard = self.data.lock().unwrap();
        guard.base_config = Some(value);
    }

    pub fn get_base_config(&self) -> Option<BaseGameConfig> {
        self.data.lock().unwrap().base_config.clone()
    }

    pub fn invalidate_base_config(&self) {
        self.data.lock().unwrap().base_config = None;
    }

    pub fn cache_save_config(&self, value: SaveGameConfig) {
        let mut guard = self.data.lock().unwrap();
        guard.save_config = Some(value);
    }

    pub fn get_save_config(&self) -> Option<SaveGameConfig> {
        self.data.lock().unwrap().save_config.clone()
    }

    pub fn invalidate_save_config(&self) {
        self.data.lock().unwrap().save_config = None;
    }

    pub fn cache_trucks(
        &self,
        path: String,
        trucks: Vec<ParsedTruck>,
        player_truck: Option<ParsedTruck>,
    ) {
        let mut guard = self.data.lock().unwrap();
        guard.save_path = Some(path);
        guard.all_trucks = Some(trucks);
        guard.player_truck = player_truck;
    }

    pub fn get_cached_trucks(&self, path: &str) -> Option<Vec<ParsedTruck>> {
        let guard = self.data.lock().unwrap();
        if Self::matches_save_path(&guard, path) {
            guard.all_trucks.clone()
        } else {
            None
        }
    }

    pub fn get_cached_player_truck(&self, path: &str) -> Option<ParsedTruck> {
        let guard = self.data.lock().unwrap();
        if Self::matches_save_path(&guard, path) {
            guard.player_truck.clone()
        } else {
            None
        }
    }

    pub fn cache_trailers(
        &self,
        path: String,
        trailers: Vec<ParsedTrailer>,
        player_trailer: Option<ParsedTrailer>,
    ) {
        let mut guard = self.data.lock().unwrap();
        guard.save_path = Some(path);
        guard.all_trailers = Some(trailers);
        guard.player_trailer = Some(player_trailer);
    }

    pub fn get_cached_trailers(&self, path: &str) -> Option<Vec<ParsedTrailer>> {
        let guard = self.data.lock().unwrap();
        if Self::matches_save_path(&guard, path) {
            guard.all_trailers.clone()
        } else {
            None
        }
    }

    pub fn get_cached_player_trailer(&self, path: &str) -> Option<Option<ParsedTrailer>> {
        let guard = self.data.lock().unwrap();
        if Self::matches_save_path(&guard, path) {
            guard.player_trailer.clone()
        } else {
            None
        }
    }

    pub fn invalidate_vehicle_data(&self) {
        let mut guard = self.data.lock().unwrap();
        guard.all_trucks = None;
        guard.player_truck = None;
        guard.all_trailers = None;
        guard.player_trailer = None;
    }

    pub fn invalidate_save_data(&self) {
        let mut guard = self.data.lock().unwrap();
        guard.save_game_data = None;
        guard.quicksave_data = None;
    }
}

pub struct AppProfileState {
    pub current_profile: Mutex<Option<String>>,
    pub current_save: Mutex<Option<String>>,
    pub selected_game: Mutex<String>,
}

impl Default for AppProfileState {
    fn default() -> Self {
        Self {
            current_profile: Mutex::new(None),
            current_save: Mutex::new(None),
            selected_game: Mutex::new("ets2".to_string()),
        }
    }
}
