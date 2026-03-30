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

const BRIDGE_MAGIC: [u8; 8] = *b"SNXTLM01";
const BRIDGE_ABI: u32 = 1;

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

    pub fn segment_name(self) -> &'static str {
        match self {
            Self::Ets2 => "Local\\SimNexusTelemetry_EUT2",
            Self::Ats => "Local\\SimNexusTelemetry_ATS",
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

#[repr(C, align(64))]
struct BridgeHeader {
    magic: [u8; 8],
    abi_version: u32,
    header_size: u32,
    payload_size: u32,
    sequence: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct TelemetryDataWire {
    frame_id: u64,
    paused: u32,
    render_time_us: u64,
    simulation_time_us: u64,
    game_time_minutes: i32,
    speed_mps: f32,
    engine_rpm: f32,
    fuel_liters: f32,
    fuel_capacity_liters: f32,
    gear: i32,
    odometer_km: f64,
    map_scale: f32,
    cargo_mass_kg: f32,
    cargo_id: [u8; 64],
    cargo_name: [u8; 64],
}

impl Default for TelemetryDataWire {
    fn default() -> Self {
        Self {
            frame_id: 0,
            paused: 0,
            render_time_us: 0,
            simulation_time_us: 0,
            game_time_minutes: 0,
            speed_mps: 0.0,
            engine_rpm: 0.0,
            fuel_liters: 0.0,
            fuel_capacity_liters: 0.0,
            gear: 0,
            odometer_km: 0.0,
            map_scale: 0.0,
            cargo_mass_kg: 0.0,
            cargo_id: [0; 64],
            cargo_name: [0; 64],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TelemetryData {
    pub game: String,
    pub frame_id: u64,
    pub paused: bool,
    pub render_time_us: u64,
    pub simulation_time_us: u64,
    pub game_time_minutes: i32,
    pub speed_mps: f32,
    pub speed_kph: f32,
    pub engine_rpm: f32,
    pub fuel_liters: f32,
    pub fuel_capacity_liters: f32,
    pub fuel_pct: f32,
    pub gear: i32,
    pub odometer_km: f64,
    pub map_scale: f32,
    pub cargo_mass_kg: Option<f32>,
    pub cargo_id: Option<String>,
    pub cargo_name: Option<String>,
}

impl TelemetryData {
    fn from_wire(game: GameId, wire: TelemetryDataWire) -> Self {
        let fuel_pct = if wire.fuel_capacity_liters > 0.0 {
            wire.fuel_liters / wire.fuel_capacity_liters
        } else {
            0.0
        };

        Self {
            game: game.as_str().to_string(),
            frame_id: wire.frame_id,
            paused: wire.paused != 0,
            render_time_us: wire.render_time_us,
            simulation_time_us: wire.simulation_time_us,
            game_time_minutes: wire.game_time_minutes,
            speed_mps: wire.speed_mps,
            speed_kph: wire.speed_mps * 3.6,
            engine_rpm: wire.engine_rpm,
            fuel_liters: wire.fuel_liters,
            fuel_capacity_liters: wire.fuel_capacity_liters,
            fuel_pct,
            gear: wire.gear,
            odometer_km: wire.odometer_km,
            map_scale: wire.map_scale,
            cargo_mass_kg: (wire.cargo_mass_kg > 0.0).then_some(wire.cargo_mass_kg),
            cargo_id: decode_string(&wire.cargo_id),
            cargo_name: decode_string(&wire.cargo_name),
        }
    }
}

#[cfg(target_os = "windows")]
struct SharedBridge {
    handle: HANDLE,
    view: MEMORY_MAPPED_VIEW_ADDRESS,
}

#[cfg(target_os = "windows")]
impl SharedBridge {
    fn connect(game: GameId) -> Result<Self, String> {
        let segment_name = wide_null(game.segment_name());
        let handle = unsafe { OpenFileMappingW(FILE_MAP_READ, 0, segment_name.as_ptr()) };
        if handle.is_null() {
            return Err(format!(
                "Shared memory segment not available: {}",
                game.segment_name()
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

    fn read_snapshot(&self, game: GameId) -> Result<Option<TelemetryData>, String> {
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

        if header.header_size as usize != std::mem::size_of::<BridgeHeader>() {
            return Err("Shared memory header size mismatch".to_string());
        }

        if header.payload_size as usize != std::mem::size_of::<TelemetryDataWire>() {
            return Err("Shared memory payload size mismatch".to_string());
        }

        let payload_ptr = unsafe {
            (self.view.Value as *const u8).add(std::mem::size_of::<BridgeHeader>())
        } as *const TelemetryDataWire;

        let seq1 = unsafe { std::ptr::read_volatile(std::ptr::addr_of!(header.sequence)) };
        if seq1 & 1 == 1 {
            return Ok(None);
        }

        let wire = unsafe { std::ptr::read_volatile(payload_ptr) };

        let seq2 = unsafe { std::ptr::read_volatile(std::ptr::addr_of!(header.sequence)) };
        if seq1 != seq2 || seq2 & 1 == 1 {
            return Ok(None);
        }

        Ok(Some(TelemetryData::from_wire(game, wire)))
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
    fn connect(_game: GameId) -> Result<Self, String> {
        Err("Shared memory bridge is only implemented on Windows".to_string())
    }

    fn read_snapshot(&self, _game: GameId) -> Result<Option<TelemetryData>, String> {
        Err("Shared memory bridge is only implemented on Windows".to_string())
    }
}

fn decode_string(bytes: &[u8]) -> Option<String> {
    let end = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len());
    let text = String::from_utf8_lossy(&bytes[..end]).trim().to_string();
    (!text.is_empty()).then_some(text)
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
        let mut attached_game: Option<GameId> = None;

        while !runtime.stop_all.load(Ordering::Relaxed) && !runtime.telemetry_stop.load(Ordering::Relaxed) {
            let current_game = runtime
                .active_game
                .lock()
                .unwrap()
                .as_deref()
                .and_then(|value| GameId::try_from(value).ok());

            let Some(current_game) = current_game else {
                runtime.bridge_connected.store(false, Ordering::Relaxed);
                bridge = None;
                attached_game = None;
                std::thread::sleep(Duration::from_millis(250));
                continue;
            };

            if attached_game != Some(current_game) {
                bridge = None;
                attached_game = Some(current_game);
                runtime.bridge_connected.store(false, Ordering::Relaxed);
            }

            if bridge.is_none() {
                match SharedBridge::connect(current_game) {
                    Ok(client) => {
                        bridge = Some(client);
                        runtime.bridge_connected.store(true, Ordering::Relaxed);
                        crate::dev_log!(
                            "[career] connected shared memory: {}",
                            current_game.segment_name()
                        );
                    }
                    Err(_) => {
                        runtime.bridge_connected.store(false, Ordering::Relaxed);
                        std::thread::sleep(Duration::from_millis(500));
                        continue;
                    }
                }
            }

            match bridge.as_ref().unwrap().read_snapshot(current_game) {
                Ok(Some(snapshot)) => {
                    runtime.bridge_connected.store(true, Ordering::Relaxed);
                    let _ = app.emit("career://telemetry_tick", snapshot);
                    std::thread::sleep(Duration::from_millis(100));
                }
                Ok(None) => {
                    std::thread::sleep(Duration::from_millis(16));
                }
                Err(error) => {
                    crate::dev_log!("[career] shared memory read failed: {}", error);
                    runtime.bridge_connected.store(false, Ordering::Relaxed);
                    bridge = None;
                    std::thread::sleep(Duration::from_millis(500));
                }
            }
        }

        runtime.bridge_connected.store(false, Ordering::Relaxed);
        runtime.telemetry_running.store(false, Ordering::Relaxed);
        crate::dev_log!("[career] telemetry reader thread stopped");
    });
}
