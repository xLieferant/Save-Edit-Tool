use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, RwLock};

use serde::{Deserialize, Serialize};

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

impl DecryptCache {
    pub fn invalidate_path(&self, path: &Path) {
        self.files.lock().unwrap().remove(path);
    }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AppMode {
    #[serde(rename = "editor", alias = "utility")]
    Utility,
    #[serde(rename = "career")]
    Career,
}

pub struct HubState {
    pub mode: RwLock<AppMode>,
}

impl Default for HubState {
    fn default() -> Self {
        Self {
            mode: RwLock::new(AppMode::Utility),
        }
    }
}

pub struct CareerRuntime {
    pub stop_all: AtomicBool,
    pub telemetry_stop: AtomicBool,
    pub telemetry_running: AtomicBool,
    pub trip_start_blocked: AtomicBool,
    pub ets2_running: AtomicBool,
    pub ats_running: AtomicBool,
    pub plugin_installed: AtomicBool,
    pub bridge_connected: AtomicBool,
    pub overview_dirty: AtomicBool,
    pub active_game: Mutex<Option<String>>,
    pub last_telemetry: Mutex<Option<LiveTelemetryState>>,
    pub active_job: Mutex<Option<ActiveJobState>>,
    pub active_trip: Mutex<Option<ActiveTripState>>,
    pub db_path: Mutex<Option<PathBuf>>,
}

impl Default for CareerRuntime {
    fn default() -> Self {
        Self {
            stop_all: AtomicBool::new(false),
            telemetry_stop: AtomicBool::new(false),
            telemetry_running: AtomicBool::new(false),
            trip_start_blocked: AtomicBool::new(false),
            ets2_running: AtomicBool::new(false),
            ats_running: AtomicBool::new(false),
            plugin_installed: AtomicBool::new(false),
            bridge_connected: AtomicBool::new(false),
            overview_dirty: AtomicBool::new(true),
            active_game: Mutex::new(None),
            last_telemetry: Mutex::new(None),
            active_job: Mutex::new(None),
            active_trip: Mutex::new(None),
            db_path: Mutex::new(None),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct LiveTelemetryState {
    pub speed_kph: f32,
    pub rpm: f32,
    pub gear: String,
    pub fuel_liters: f32,
    pub fuel_capacity_liters: f32,
    pub engine_on: bool,
    pub timestamp: u64,
    pub paused: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ActiveJobState {
    pub job_id: String,
    pub started_at_utc: String,
    pub last_seen_at_utc: String,
    pub origin_city: String,
    pub destination_city: String,
    pub source_company: String,
    pub destination_company: String,
    pub cargo: String,
    pub planned_distance_km: f64,
    pub income: i64,
    pub delivery_time_min: u32,
    pub game_time_min: u32,
    pub cargo_damage: f32,
    pub job_market: String,
    pub special_job: bool,
    pub last_event: Option<ActiveJobEvent>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveJobEvent {
    Delivered,
    Cancelled,
}

#[derive(Debug, Clone)]
pub struct ActiveTripState {
    pub trip_id: i64,
    pub job_id: String,
    pub contract_id: Option<String>,
    pub job_progress_base_km: f64,
    pub job_target_distance_km: Option<f64>,
    pub job_price_per_km: Option<f64>,
    pub started_at_utc_ms: i64,
    pub last_update_utc_ms: i64,
    pub origin: String,
    pub destination: String,
    pub cargo: String,
    pub bonus_payout: i64,
    pub distance_km: f64,
    pub duration_seconds: i64,
    pub max_speed_kph: f32,
    pub speed_sum_kph: f64,
    pub speed_samples: u32,
    pub speeding_events: i64,
    pub was_speeding: bool,
    pub fuel_used_liters: f64,
    pub last_fuel_liters: f32,
    pub last_speed_kph: f32,
}

pub struct CareerState {
    pub runtime: Arc<CareerRuntime>,
}

impl Default for CareerState {
    fn default() -> Self {
        Self {
            runtime: Arc::new(CareerRuntime::default()),
        }
    }
}
