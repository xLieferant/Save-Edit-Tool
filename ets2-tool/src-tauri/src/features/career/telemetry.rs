use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;
use tauri::{AppHandle, Emitter};
#[cfg(target_os = "windows")]
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
#[cfg(target_os = "windows")]
use windows_sys::Win32::System::Memory::{
    FILE_MAP_READ, MEMORY_MAPPED_VIEW_ADDRESS, MapViewOfFile, OpenFileMappingW, UnmapViewOfFile,
};

use crate::features::career::logbook::{self, TelemetrySample};
use crate::features::telemetry::simnexus_protocol::{
    self, BRIDGE_PROTOCOL_VERSION, LEGACY_SHARED_MEMORY_NAME, SHARED_MEMORY_NAME, TelemetryDataV3,
};
use crate::state::CareerRuntime;

const STATUS_LOG_INTERVAL_MS: i64 = 10_000;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum GameId {
    Ets2,
    Ats,
}

impl GameId {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ets2 => "ets2",
            Self::Ats => "ats",
        }
    }
}

impl TryFrom<&str> for GameId {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "ets2" => Ok(Self::Ets2),
            "ats" => Ok(Self::Ats),
            _ => Err(format!("Unknown game id: {value}")),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum JobEvent {
    Delivered,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct TelemetryJob {
    pub job_id: String,
    pub source_city: String,
    pub destination_city: String,
    pub source_company: String,
    pub destination_company: String,
    pub cargo: String,
    pub income: i64,
    pub delivery_time_min: u32,
    pub game_time_min: u32,
    pub planned_distance_km: f64,
    pub cargo_damage: f64,
    pub job_market: String,
    pub special_job: bool,
    pub cargo_loaded: bool,
    pub event: Option<JobEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TelemetrySnapshot {
    pub protocol_version: u32,
    pub payload_size: u32,
    pub sequence: i64,
    pub heartbeat_timestamp_ms: u64,
    pub heartbeat_age_ms: u64,
    pub frame_id: u64,
    pub simulation_timestamp: u64,
    pub speed_kph: f64,
    pub engine_rpm: f64,
    pub odometer_km: f64,
    pub fuel_liters: f32,
    pub fuel_capacity_liters: f32,
    pub map_scale: f32,
    pub gear: i32,
    pub paused: u8,
    pub engine_enabled: bool,
    pub plugin_initialized: bool,
    pub sdk_connected: bool,
    pub telemetry_active: bool,
    pub telemetry_callback_seen: bool,
    pub job_config_seen: bool,
    pub dll_build_id: String,
    pub dll_version: String,
    pub dll_path: String,
    pub job_event_sequence: u64,
    pub job_event: Option<JobEvent>,
    pub job: Option<TelemetryJob>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct FrontendTelemetryPayload {
    speed: f32,
    rpm: f32,
    gear: String,
    fuel: f32,
    fuel_capacity: f32,
    engine_on: bool,
    timestamp: u64,
    paused: bool,
    plugin_installed: bool,
    sdk_connected: bool,
}

fn payload_has_active_job(payload: &TelemetryDataV3) -> bool {
    payload.job_active != 0
}

fn job_event(value: u8) -> Option<JobEvent> {
    match value {
        1 => Some(JobEvent::Delivered),
        2 => Some(JobEvent::Cancelled),
        _ => None,
    }
}

fn format_gear(gear: i32) -> String {
    match gear {
        value if value < 0 => format!("R{}", value.abs()),
        0 => "N".to_string(),
        value => value.to_string(),
    }
}

#[cfg(target_os = "windows")]
struct SharedBridge {
    handle: HANDLE,
    view: MEMORY_MAPPED_VIEW_ADDRESS,
}

#[cfg(target_os = "windows")]
impl SharedBridge {
    fn connect() -> Result<Self, String> {
        let segment_name = wide_null(SHARED_MEMORY_NAME);
        let handle = unsafe { OpenFileMappingW(FILE_MAP_READ, 0, segment_name.as_ptr()) };
        if handle.is_null() {
            if mapping_exists(LEGACY_SHARED_MEMORY_NAME) {
                return Err(
                    "Legacy SimNexus DLL detected: protocol 1 mapping exists, protocol 3 required"
                        .to_string(),
                );
            }
            return Err(format!("Shared memory unavailable: {SHARED_MEMORY_NAME}"));
        }
        let view = unsafe { MapViewOfFile(handle, FILE_MAP_READ, 0, 0, 0) };
        if view.Value.is_null() {
            unsafe {
                CloseHandle(handle);
            }
            return Err("Failed to map SimNexus shared memory view".to_string());
        }
        Ok(Self { handle, view })
    }

    fn read_snapshot(&self) -> Result<Option<TelemetrySnapshot>, String> {
        let Some(bridge) =
            (unsafe { simnexus_protocol::read_consistent(self.view.Value.cast::<u8>())? })
        else {
            return Ok(None);
        };
        let heartbeat_age_ms = simnexus_protocol::validate_liveness(&bridge.payload)?;
        let payload = bridge.payload;
        let event = job_event(payload.job_event);

        let job = if payload_has_active_job(&payload) {
            Some(TelemetryJob {
                job_id: simnexus_protocol::bytes_to_string(&payload.job_id),
                source_city: simnexus_protocol::bytes_to_string(&payload.source_city),
                destination_city: simnexus_protocol::bytes_to_string(&payload.destination_city),
                source_company: simnexus_protocol::bytes_to_string(&payload.source_company),
                destination_company: simnexus_protocol::bytes_to_string(
                    &payload.destination_company,
                ),
                cargo: simnexus_protocol::bytes_to_string(&payload.cargo),
                income: payload.job_income,
                delivery_time_min: payload.job_delivery_time_min,
                game_time_min: payload.game_time_min,
                planned_distance_km: payload.job_planned_distance_km,
                cargo_damage: payload.job_cargo_damage,
                job_market: simnexus_protocol::bytes_to_string(&payload.job_market),
                special_job: payload.job_special != 0,
                cargo_loaded: payload.job_cargo_loaded != 0,
                event,
            })
        } else {
            None
        };

        Ok(Some(TelemetrySnapshot {
            protocol_version: BRIDGE_PROTOCOL_VERSION,
            payload_size: std::mem::size_of::<TelemetryDataV3>() as u32,
            sequence: bridge.sequence,
            heartbeat_timestamp_ms: payload.heartbeat_timestamp_ms,
            heartbeat_age_ms,
            frame_id: payload.frame_id,
            simulation_timestamp: payload.telemetry_timestamp_ms,
            speed_kph: payload.speed_kph,
            engine_rpm: payload.engine_rpm,
            odometer_km: payload.odometer_km,
            fuel_liters: payload.fuel_liters,
            fuel_capacity_liters: payload.fuel_capacity_liters,
            map_scale: payload.map_scale,
            gear: payload.gear,
            paused: payload.game_paused,
            engine_enabled: payload.engine_enabled != 0,
            plugin_initialized: payload.plugin_initialized != 0,
            sdk_connected: payload.sdk_connected != 0,
            telemetry_active: payload.telemetry_active != 0,
            telemetry_callback_seen: payload.telemetry_callback_seen != 0,
            job_config_seen: payload.job_config_seen != 0,
            dll_build_id: simnexus_protocol::bytes_to_string(&payload.build_id),
            dll_version: simnexus_protocol::bytes_to_string(&payload.dll_version),
            dll_path: simnexus_protocol::bytes_to_string(&payload.dll_path),
            job_event_sequence: payload.job_event_sequence,
            job_event: event,
            job,
        }))
    }
}

#[cfg(target_os = "windows")]
impl Drop for SharedBridge {
    fn drop(&mut self) {
        unsafe {
            if !self.view.Value.is_null() {
                UnmapViewOfFile(self.view);
            }
            if !self.handle.is_null() {
                CloseHandle(self.handle);
            }
        }
    }
}

#[cfg(target_os = "windows")]
fn mapping_exists(name: &str) -> bool {
    let name = wide_null(name);
    let handle = unsafe { OpenFileMappingW(FILE_MAP_READ, 0, name.as_ptr()) };
    if handle.is_null() {
        return false;
    }
    unsafe {
        CloseHandle(handle);
    }
    true
}

#[cfg(target_os = "windows")]
fn wide_null(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(not(target_os = "windows"))]
struct SharedBridge;

#[cfg(not(target_os = "windows"))]
impl SharedBridge {
    fn connect() -> Result<Self, String> {
        Err("Shared memory bridge is only implemented on Windows".to_string())
    }

    fn read_snapshot(&self) -> Result<Option<TelemetrySnapshot>, String> {
        Err("Shared memory bridge is only implemented on Windows".to_string())
    }
}

pub fn ensure_running(app: AppHandle, runtime: Arc<CareerRuntime>, game: GameId) {
    *runtime.active_game.lock().unwrap() = Some(game.as_str().to_string());
    runtime.telemetry_stop.store(false, Ordering::Relaxed);
    if runtime
        .telemetry_running
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return;
    }

    crate::dev_log!("[career] SimNexus telemetry reader started");
    std::thread::spawn(move || {
        let mut bridge: Option<SharedBridge> = None;
        let mut last_status_fingerprint: Option<String> = None;
        let mut last_status_log_ms = 0i64;
        let mut last_error: Option<String> = None;
        let mut last_error_log_ms = 0i64;
        let mut last_frontend_tick_emit_ms = 0i64;
        let mut last_frontend_payload: Option<FrontendTelemetryPayload> = None;

        while !runtime.stop_all.load(Ordering::Relaxed)
            && !runtime.telemetry_stop.load(Ordering::Relaxed)
        {
            if bridge.is_none() {
                match SharedBridge::connect() {
                    Ok(client) => {
                        bridge = Some(client);
                        runtime.plugin_installed.store(true, Ordering::Relaxed);
                        crate::dev_log!(
                            "[career] connected shared memory: {} ({})",
                            SHARED_MEMORY_NAME,
                            simnexus_protocol::layout_diagnostic()
                        );
                    }
                    Err(error) => {
                        runtime.bridge_connected.store(false, Ordering::Relaxed);
                        log_error_rate_limited(&error, &mut last_error, &mut last_error_log_ms);
                        std::thread::sleep(Duration::from_millis(500));
                        continue;
                    }
                }
            }

            match bridge.as_ref().unwrap().read_snapshot() {
                Ok(Some(snapshot)) => {
                    let now = chrono::Utc::now().timestamp_millis();
                    runtime.plugin_installed.store(true, Ordering::Relaxed);
                    runtime.bridge_connected.store(true, Ordering::Relaxed);
                    last_error = None;

                    let fingerprint = format!(
                        "{}|{}|{}|{}|{}|{}|{}|{}",
                        snapshot.dll_build_id,
                        snapshot.plugin_initialized,
                        snapshot.sdk_connected,
                        snapshot.telemetry_active,
                        snapshot.telemetry_callback_seen,
                        snapshot.paused,
                        snapshot.job.is_some(),
                        snapshot.job_config_seen
                    );
                    if last_status_fingerprint.as_ref() != Some(&fingerprint)
                        || now - last_status_log_ms >= STATUS_LOG_INTERVAL_MS
                    {
                        log_snapshot_status(&snapshot);
                        last_status_fingerprint = Some(fingerprint);
                        last_status_log_ms = now;
                    }

                    if let Err(error) = crate::features::career::job_tracking::process_snapshot(
                        runtime.as_ref(),
                        &snapshot,
                    ) {
                        crate::dev_log!("[career] job tracking failed: {}", error);
                    }
                    if let Err(error) = logbook::process_snapshot(
                        runtime.as_ref(),
                        TelemetrySample {
                            timestamp: snapshot.simulation_timestamp,
                            speed_kph: snapshot.speed_kph as f32,
                            rpm: snapshot.engine_rpm as f32,
                            gear: snapshot.gear,
                            fuel_liters: snapshot.fuel_liters,
                            fuel_capacity_liters: snapshot.fuel_capacity_liters,
                            engine_enabled: snapshot.engine_enabled,
                            paused: snapshot.paused != 0,
                        },
                    ) {
                        crate::dev_log!("[career] telemetry logbook sync failed: {}", error);
                    }

                    let frontend_payload = FrontendTelemetryPayload {
                        speed: snapshot.speed_kph as f32,
                        rpm: snapshot.engine_rpm as f32,
                        gear: format_gear(snapshot.gear),
                        fuel: snapshot.fuel_liters,
                        fuel_capacity: snapshot.fuel_capacity_liters,
                        engine_on: snapshot.engine_enabled,
                        timestamp: snapshot.simulation_timestamp,
                        paused: snapshot.paused != 0,
                        plugin_installed: true,
                        sdk_connected: true,
                    };
                    if last_frontend_payload.as_ref() != Some(&frontend_payload) {
                        last_frontend_payload = Some(frontend_payload.clone());
                        let _ = app.emit("telemetry:update", frontend_payload);
                    }
                    if now - last_frontend_tick_emit_ms >= 250 {
                        last_frontend_tick_emit_ms = now;
                        let _ = app.emit("career://telemetry_tick", snapshot);
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }
                Ok(None) => std::thread::sleep(Duration::from_millis(16)),
                Err(error) => {
                    runtime.bridge_connected.store(false, Ordering::Relaxed);
                    log_error_rate_limited(&error, &mut last_error, &mut last_error_log_ms);
                    bridge = None;
                    std::thread::sleep(Duration::from_millis(500));
                }
            }
        }

        runtime.telemetry_running.store(false, Ordering::Relaxed);
        runtime.plugin_installed.store(false, Ordering::Relaxed);
        runtime.bridge_connected.store(false, Ordering::Relaxed);
        crate::dev_log!("[career] SimNexus telemetry reader stopped");
    });
}

fn log_snapshot_status(snapshot: &TelemetrySnapshot) {
    let job = snapshot.job.as_ref();
    crate::dev_log!(
        "[career] telemetry status: shared_memory=connected magic=valid protocol={} payload_size={} dll_version='{}' build='{}' dll='{}' sequence={} heartbeat_age_ms={} plugin_initialized={} sdk_connected={} telemetry_updates={} callback_seen={} paused={} frame_id={} timestamp={} job_active={} job_config_seen={} source='{}' destination='{}' cargo='{}'",
        snapshot.protocol_version,
        snapshot.payload_size,
        snapshot.dll_version,
        snapshot.dll_build_id,
        snapshot.dll_path,
        snapshot.sequence,
        snapshot.heartbeat_age_ms,
        snapshot.plugin_initialized,
        snapshot.sdk_connected,
        snapshot.telemetry_active,
        snapshot.telemetry_callback_seen,
        snapshot.paused != 0,
        snapshot.frame_id,
        snapshot.simulation_timestamp,
        job.is_some(),
        snapshot.job_config_seen,
        job.map(|value| value.source_city.as_str()).unwrap_or(""),
        job.map(|value| value.destination_city.as_str())
            .unwrap_or(""),
        job.map(|value| value.cargo.as_str()).unwrap_or("")
    );
}

fn log_error_rate_limited(error: &str, previous: &mut Option<String>, previous_log_ms: &mut i64) {
    let now = chrono::Utc::now().timestamp_millis();
    if previous.as_deref() != Some(error) || now - *previous_log_ms >= STATUS_LOG_INTERVAL_MS {
        crate::dev_log!("[career] SimNexus telemetry unavailable: {}", error);
        *previous = Some(error.to_string());
        *previous_log_ms = now;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn global_game_time_does_not_create_a_job() {
        let payload = TelemetryDataV3 {
            game_time_min: 42_000,
            job_active: 0,
            ..Default::default()
        };
        assert!(!payload_has_active_job(&payload));
    }

    #[test]
    fn explicit_job_flag_creates_a_job() {
        let payload = TelemetryDataV3 {
            job_active: 1,
            ..Default::default()
        };
        assert!(payload_has_active_job(&payload));
    }
}
