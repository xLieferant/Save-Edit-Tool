use serde::{Deserialize, Serialize};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter};
#[cfg(target_os = "windows")]
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
#[cfg(target_os = "windows")]
use windows_sys::Win32::System::Memory::{
    FILE_MAP_READ, MEMORY_MAPPED_VIEW_ADDRESS, MapViewOfFile, OpenFileMappingW, UnmapViewOfFile,
};

use crate::state::CareerRuntime;

const SHARED_MEMORY_NAME: &str = "Local\\SimNexusTelemetry";
const BRIDGE_MAGIC: [u8; 8] = *b"SNXTLM01";
const BRIDGE_ABI: u32 = 1;
const PAYLOAD_V1_SIZE: usize = 56;
const JOB_DEBUG_LOG_INTERVAL_MS: i64 = 10_000;

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

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct BridgeHeader {
    magic: [u8; 8],
    abi_version: u32,
    payload_size: u32,
    sequence: i64,
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
    pub job: Option<TelemetryJob>,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct TelemetryDataV1 {
    frame_id: u64,
    simulation_timestamp: u64,
    speed_kph: f64,
    engine_rpm: f64,
    odometer_km: f64,
    fuel_liters: f32,
    fuel_capacity_liters: f32,
    map_scale: f32,
    gear: i32,
    paused: u8,
    reserved: [u8; 3],
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct TelemetryDataV2 {
    frame_id: u64,
    simulation_timestamp: u64,
    payload_revision: u32,
    payload_reserved: u32,
    speed_kph: f64,
    engine_rpm: f64,
    odometer_km: f64,
    fuel_liters: f32,
    fuel_capacity_liters: f32,
    map_scale: f32,
    gear: i32,
    paused: u8,
    job_active: u8,
    job_special: u8,
    job_cargo_loaded: u8,
    job_event: u8,
    job_income: i64,
    job_delivery_time_min: u32,
    game_time_min: u32,
    job_planned_distance_km: f64,
    job_cargo_damage: f64,
    job_id: [u8; 64],
    source_city: [u8; 64],
    destination_city: [u8; 64],
    source_company: [u8; 64],
    destination_company: [u8; 64],
    cargo: [u8; 64],
    job_market: [u8; 32],
    reserved: [u8; 8],
}

fn bytes_to_string(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).trim().to_string()
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
            return Err(format!(
                "Shared memory segment not available: {}",
                SHARED_MEMORY_NAME
            ));
        }

        let view = unsafe { MapViewOfFile(handle, FILE_MAP_READ, 0, 0, 0) };
        if view.Value.is_null() {
            unsafe {
                CloseHandle(handle);
            }
            return Err("Failed to map shared memory view".to_string());
        }

        Ok(Self { handle, view })
    }

    fn read_snapshot(&self) -> Result<Option<TelemetrySnapshot>, String> {
        let header_ptr = self.view.Value as *const BridgeHeader;
        let header = unsafe { &*header_ptr };

        if header.magic != BRIDGE_MAGIC {
            return Err("Shared memory magic mismatch".to_string());
        }

        if header.abi_version != BRIDGE_ABI {
            return Err(format!(
                "Unsupported shared memory ABI: {}",
                header.abi_version
            ));
        }

        let payload_size = header.payload_size as usize;
        let payload_ptr =
            unsafe { (self.view.Value as *const u8).add(std::mem::size_of::<BridgeHeader>()) };

        let seq1 = unsafe { std::ptr::read_volatile(std::ptr::addr_of!(header.sequence)) };
        if seq1 & 1 == 1 {
            return Ok(None);
        }

        let snapshot = if payload_size == std::mem::size_of::<TelemetryDataV1>()
            || payload_size == PAYLOAD_V1_SIZE
        {
            let v1 = unsafe { std::ptr::read_volatile(payload_ptr as *const TelemetryDataV1) };
            TelemetrySnapshot {
                frame_id: v1.frame_id,
                simulation_timestamp: v1.simulation_timestamp,
                speed_kph: v1.speed_kph,
                engine_rpm: v1.engine_rpm,
                odometer_km: v1.odometer_km,
                fuel_liters: v1.fuel_liters,
                fuel_capacity_liters: v1.fuel_capacity_liters,
                map_scale: v1.map_scale,
                gear: v1.gear,
                paused: v1.paused,
                job: None,
            }
        } else if payload_size == std::mem::size_of::<TelemetryDataV2>() {
            let v2 = unsafe { std::ptr::read_volatile(payload_ptr as *const TelemetryDataV2) };

            let job_id = bytes_to_string(&v2.job_id);
            let source_city = bytes_to_string(&v2.source_city);
            let destination_city = bytes_to_string(&v2.destination_city);
            let source_company = bytes_to_string(&v2.source_company);
            let destination_company = bytes_to_string(&v2.destination_company);
            let cargo = bytes_to_string(&v2.cargo);
            let job_market = bytes_to_string(&v2.job_market);

            let has_route = !source_city.is_empty() && !destination_city.is_empty();
            let has_cargo = !cargo.is_empty();
            let has_income = v2.job_income > 0;
            let active_detected = v2.job_active != 0 || has_route || (has_cargo && has_income);

            let job = if active_detected {
                Some(TelemetryJob {
                    job_id,
                    source_city,
                    destination_city,
                    source_company,
                    destination_company,
                    cargo,
                    income: v2.job_income,
                    delivery_time_min: v2.job_delivery_time_min,
                    game_time_min: v2.game_time_min,
                    planned_distance_km: v2.job_planned_distance_km,
                    cargo_damage: v2.job_cargo_damage,
                    job_market,
                    special_job: v2.job_special != 0,
                    cargo_loaded: v2.job_cargo_loaded != 0,
                    event: match v2.job_event {
                        1 => Some(JobEvent::Delivered),
                        2 => Some(JobEvent::Cancelled),
                        _ => None,
                    },
                })
            } else {
                None
            };

            TelemetrySnapshot {
                frame_id: v2.frame_id,
                simulation_timestamp: v2.simulation_timestamp,
                speed_kph: v2.speed_kph,
                engine_rpm: v2.engine_rpm,
                odometer_km: v2.odometer_km,
                fuel_liters: v2.fuel_liters,
                fuel_capacity_liters: v2.fuel_capacity_liters,
                map_scale: v2.map_scale,
                gear: v2.gear,
                paused: v2.paused,
                job,
            }
        } else {
            return Err("Shared memory payload size mismatch".to_string());
        };

        let seq2 = unsafe { std::ptr::read_volatile(std::ptr::addr_of!(header.sequence)) };
        if seq1 != seq2 || seq2 & 1 == 1 {
            return Ok(None);
        }

        Ok(Some(snapshot))
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

#[cfg(not(target_os = "windows"))]
struct SharedBridge;

#[cfg(not(target_os = "windows"))]
impl SharedBridge {
    fn connect() -> Result<Self, String> {
        Err("Shared memory bridge is only implemented on Windows".to_string())
    }

    fn read_snapshot(&self) -> Result<Option<TelemetryData>, String> {
        Err("Shared memory bridge is only implemented on Windows".to_string())
    }
}

#[cfg(target_os = "windows")]
fn wide_null(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
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

    crate::dev_log!("[career] telemetry reader thread started");

    std::thread::spawn(move || {
        let mut bridge: Option<SharedBridge> = None;
        let mut last_job_missing_log_ms: i64 = 0;
        let mut last_payload_debug_ms: i64 = 0;
        let mut last_job_debug_fingerprint: Option<String> = None;
        let mut last_frontend_tick_emit_ms: i64 = 0;

        while !runtime.stop_all.load(Ordering::Relaxed)
            && !runtime.telemetry_stop.load(Ordering::Relaxed)
        {
            if bridge.is_none() {
                match SharedBridge::connect() {
                    Ok(client) => {
                        bridge = Some(client);
                        crate::dev_log!(
                            "[career] connected shared memory: {}",
                            SHARED_MEMORY_NAME
                        );

                        // Log the expected V2 layout details once per connection.
                        let base = std::mem::MaybeUninit::<TelemetryDataV2>::uninit();
                        let base_ptr = base.as_ptr() as usize;
                        let off_rev = unsafe {
                            std::ptr::addr_of!((*base.as_ptr()).payload_revision) as usize - base_ptr
                        };
                        let off_job_active = unsafe {
                            std::ptr::addr_of!((*base.as_ptr()).job_active) as usize - base_ptr
                        };
                        let off_job_id = unsafe {
                            std::ptr::addr_of!((*base.as_ptr()).job_id) as usize - base_ptr
                        };
                        crate::dev_log!(
                            "[career] SimNexus payload expected: v1_size={}, v2_size={}, off(rev)={}, off(job_active)={}, off(job_id)={}",
                            PAYLOAD_V1_SIZE,
                            std::mem::size_of::<TelemetryDataV2>(),
                            off_rev,
                            off_job_active,
                            off_job_id
                        );
                    }
                    Err(_) => {
                        std::thread::sleep(Duration::from_millis(500));
                        continue;
                    }
                }
            }

            match bridge.as_ref().unwrap().read_snapshot() {
                Ok(Some(snapshot)) => {
                    let now = chrono::Utc::now().timestamp_millis();
                    if now - last_payload_debug_ms > JOB_DEBUG_LOG_INTERVAL_MS {
                        // If we are stuck on V1 payloads, jobs can never be detected.
                        if snapshot.job.is_none() && snapshot.frame_id != 0 {
                            crate::dev_log!(
                                "[career] job debug: snapshot has no job (frame_id={}, speed_kph={:.1})",
                                snapshot.frame_id,
                                snapshot.speed_kph
                            );
                        }
                        last_payload_debug_ms = now;
                    }

                    if let Some(job) = snapshot.job.as_ref() {
                        // Avoid spamming: log only when key job fields change.
                        let fingerprint = format!(
                            "{}|{}|{}|{}|{}|{}|{}|{}|{}",
                            job.job_id,
                            job.source_city,
                            job.destination_city,
                            job.source_company,
                            job.destination_company,
                            job.cargo,
                            job.income,
                            job.planned_distance_km,
                            job.delivery_time_min
                        );
                        if last_job_debug_fingerprint.as_ref() != Some(&fingerprint)
                            && now - last_job_missing_log_ms > 500
                        {
                            crate::dev_log!(
                                "[career] job debug: active=1 job_id='{}' source='{}' dest='{}' cargo='{}' income={} planned_km={:.1} delivery_min={} damage={:.3} market='{}' special={}",
                                job.job_id,
                                job.source_city,
                                job.destination_city,
                                job.cargo,
                                job.income,
                                job.planned_distance_km,
                                job.delivery_time_min,
                                job.cargo_damage,
                                job.job_market,
                                job.special_job
                            );
                            last_job_debug_fingerprint = Some(fingerprint);
                        }
                    }

                    if snapshot.job.is_none() {
                        // Avoid spamming: log at most every 10 seconds.
                        if now - last_job_missing_log_ms > 10_000 {
                            crate::dev_log!(
                                "[career] telemetry active, but no job detected (possible causes: no job in-game, DLL not updated, payload layout mismatch)"
                            );
                            last_job_missing_log_ms = now;
                        }
                    }
                    if let Err(error) =
                        crate::features::career::job_tracking::process_snapshot(runtime.as_ref(), &snapshot)
                    {
                        crate::dev_log!("[career] job tracking failed: {}", error);
                    }
                    if now - last_frontend_tick_emit_ms >= 250 {
                        last_frontend_tick_emit_ms = now;
                        let _ = app.emit("career://telemetry_tick", snapshot);
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }
                Ok(None) => {
                    std::thread::sleep(Duration::from_millis(16));
                }
                Err(error) => {
                    crate::dev_log!("[career] shared memory read failed: {}", error);
                    bridge = None;
                    std::thread::sleep(Duration::from_millis(500));
                }
            }
        }

        runtime.telemetry_running.store(false, Ordering::Relaxed);
        crate::dev_log!("[career] telemetry reader thread stopped");
    });
}
