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

#[repr(C)]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
pub struct TelemetryData {
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
    pub reserved: [u8; 3],
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

    fn read_snapshot(&self) -> Result<Option<TelemetryData>, String> {
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

        if header.payload_size as usize != std::mem::size_of::<TelemetryData>() {
            return Err("Shared memory payload size mismatch".to_string());
        }

        let payload_ptr =
            unsafe { (self.view.Value as *const u8).add(std::mem::size_of::<BridgeHeader>()) }
                as *const TelemetryData;

        let seq1 = unsafe { std::ptr::read_volatile(std::ptr::addr_of!(header.sequence)) };
        if seq1 & 1 == 1 {
            return Ok(None);
        }

        let snapshot = unsafe { std::ptr::read_volatile(payload_ptr) };

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

        while !runtime.stop_all.load(Ordering::Relaxed)
            && !runtime.telemetry_stop.load(Ordering::Relaxed)
        {
            if bridge.is_none() {
                match SharedBridge::connect() {
                    Ok(client) => {
                        bridge = Some(client);
                        runtime.bridge_connected.store(true, Ordering::Relaxed);
                        crate::dev_log!(
                            "[career] connected shared memory: {}",
                            SHARED_MEMORY_NAME
                        );
                    }
                    Err(_) => {
                        runtime.bridge_connected.store(false, Ordering::Relaxed);
                        std::thread::sleep(Duration::from_millis(500));
                        continue;
                    }
                }
            }

            match bridge.as_ref().unwrap().read_snapshot() {
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
