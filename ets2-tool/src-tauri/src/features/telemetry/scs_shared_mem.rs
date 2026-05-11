use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use sqlx::SqlitePool;
use tauri::{AppHandle, Emitter};

use crate::events::{EVT_SYSTEM_STATUS, EVT_TELEMETRY_JOB_EVENT};
use crate::features::ets2save::link_service;
use crate::features::telemetry::events::{SystemStatusPayload, TelemetryJobEventPayload};

#[cfg(target_os = "windows")]
mod platform {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Duration;

    use sqlx::SqlitePool;
    use tauri::{AppHandle, Emitter};
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
    use windows_sys::Win32::System::Memory::{
        FILE_MAP_READ, MEMORY_MAPPED_VIEW_ADDRESS, MapViewOfFile, OpenFileMappingW, UnmapViewOfFile,
    };

    use crate::events::{EVT_SYSTEM_STATUS, EVT_TELEMETRY_JOB_EVENT};
    use crate::features::ets2save::link_service;
    use crate::features::telemetry::events::{SystemStatusPayload, TelemetryJobEventPayload};

    const SHARED_MEMORY_NAME: &str = "Local\\SimNexusTelemetry";
    const BRIDGE_MAGIC: [u8; 8] = *b"SNXTLM01";
    const BRIDGE_ABI: u32 = 1;

    #[repr(C)]
    #[derive(Debug, Clone, Copy)]
    struct BridgeHeader {
        magic: [u8; 8],
        abi_version: u32,
        payload_size: u32,
        sequence: i64,
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

    struct SharedBridge {
        handle: HANDLE,
        view: MEMORY_MAPPED_VIEW_ADDRESS,
    }

    impl SharedBridge {
        fn connect() -> Result<Self, String> {
            let segment_name = wide_null(SHARED_MEMORY_NAME);
            let handle = unsafe { OpenFileMappingW(FILE_MAP_READ, 0, segment_name.as_ptr()) };
            if handle.is_null() {
                return Err("telemetry bridge unavailable".to_string());
            }

            let view = unsafe { MapViewOfFile(handle, FILE_MAP_READ, 0, 0, 0) };
            if view.Value.is_null() {
                unsafe {
                    CloseHandle(handle);
                }
                return Err("telemetry bridge map failed".to_string());
            }

            Ok(Self { handle, view })
        }

        fn read_event(&self) -> Result<Option<TelemetryJobEventPayload>, String> {
            let header_ptr = self.view.Value as *const BridgeHeader;
            let header = unsafe { &*header_ptr };
            if header.magic != BRIDGE_MAGIC || header.abi_version != BRIDGE_ABI {
                return Err("telemetry bridge header mismatch".to_string());
            }

            let seq_before =
                unsafe { std::ptr::read_volatile(std::ptr::addr_of!(header.sequence)) };
            if seq_before & 1 == 1 {
                return Ok(None);
            }

            let payload_ptr =
                unsafe { (self.view.Value as *const u8).add(std::mem::size_of::<BridgeHeader>()) };
            let payload = unsafe { std::ptr::read_volatile(payload_ptr as *const TelemetryDataV2) };

            let seq_after = unsafe { std::ptr::read_volatile(std::ptr::addr_of!(header.sequence)) };
            if seq_before != seq_after || seq_after & 1 == 1 {
                return Ok(None);
            }

            let cargo = bytes_to_string(&payload.cargo);
            let src_city = bytes_to_string(&payload.source_city);
            let dst_city = bytes_to_string(&payload.destination_city);
            let src_company = bytes_to_string(&payload.source_company);
            let dst_company = bytes_to_string(&payload.destination_company);
            let on_job = payload.job_active != 0
                || (!cargo.is_empty() && !src_city.is_empty() && !dst_city.is_empty());
            let job_delivered = payload.job_event == 1;
            let job_cancelled = payload.job_event == 2;
            let job_finished = payload.job_event != 0;
            let job_result = match payload.job_event {
                1 => Some("completed".to_string()),
                2 => Some("cancelled".to_string()),
                value if value != 0 => Some("finished".to_string()),
                _ => None,
            };

            Ok(Some(TelemetryJobEventPayload {
                sdk_active: true,
                paused: payload.paused != 0,
                on_job,
                job_finished,
                job_delivered,
                job_cancelled,
                job_result,
                cargo_id: if cargo.is_empty() {
                    None
                } else {
                    Some(cargo.clone())
                },
                cargo: if cargo.is_empty() { None } else { Some(cargo) },
                city_src_id: if src_city.is_empty() {
                    None
                } else {
                    Some(src_city.clone())
                },
                city_src: if src_city.is_empty() {
                    None
                } else {
                    Some(src_city)
                },
                comp_src_id: if src_company.is_empty() {
                    None
                } else {
                    Some(src_company.clone())
                },
                comp_src: if src_company.is_empty() {
                    None
                } else {
                    Some(src_company)
                },
                city_dst_id: if dst_city.is_empty() {
                    None
                } else {
                    Some(dst_city.clone())
                },
                city_dst: if dst_city.is_empty() {
                    None
                } else {
                    Some(dst_city)
                },
                comp_dst_id: if dst_company.is_empty() {
                    None
                } else {
                    Some(dst_company.clone())
                },
                comp_dst: if dst_company.is_empty() {
                    None
                } else {
                    Some(dst_company)
                },
                planned_distance_km: payload.job_planned_distance_km,
                route_distance: payload.job_planned_distance_km,
                route_time: payload.job_delivery_time_min as i64,
                job_income: payload.job_income,
                job_delivered_revenue: payload.job_income,
            }))
        }
    }

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

    fn bytes_to_string(bytes: &[u8]) -> String {
        let end = bytes
            .iter()
            .position(|byte| *byte == 0)
            .unwrap_or(bytes.len());
        String::from_utf8_lossy(&bytes[..end]).trim().to_string()
    }

    fn wide_null(value: &str) -> Vec<u16> {
        value.encode_utf16().chain(std::iter::once(0)).collect()
    }

    pub fn start(app: AppHandle, pool: SqlitePool) {
        crate::dev_log!("[trace] START telemetry_shared_mem_startup");
        let stop = Arc::new(AtomicBool::new(false));
        let app_for_thread = app.clone();
        std::thread::spawn(move || {
            let mut bridge: Option<SharedBridge> = None;
            let mut last_status: Option<SystemStatusPayload> = None;
            let mut last_event: Option<TelemetryJobEventPayload> = None;

            while !stop.load(Ordering::Relaxed) {
                if bridge.is_none() {
                    match SharedBridge::connect() {
                        Ok(client) => {
                            bridge = Some(client);
                            let status = SystemStatusPayload {
                                sdk_active: true,
                                telemetry_available: true,
                                message: None,
                            };
                            if last_status.as_ref() != Some(&status) {
                                last_status = Some(status.clone());
                                let _ = app_for_thread.emit(EVT_SYSTEM_STATUS, status);
                            }
                        }
                        Err(error) => {
                            let status = SystemStatusPayload {
                                sdk_active: false,
                                telemetry_available: false,
                                message: Some(error),
                            };
                            if last_status.as_ref() != Some(&status) {
                                last_status = Some(status.clone());
                                let _ = app_for_thread.emit(EVT_SYSTEM_STATUS, status);
                            }
                            std::thread::sleep(Duration::from_millis(750));
                            continue;
                        }
                    }
                }

                match bridge.as_ref().unwrap().read_event() {
                    Ok(Some(event)) => {
                        if last_event.as_ref() != Some(&event) {
                            last_event = Some(event.clone());
                            let _ = app_for_thread.emit(EVT_TELEMETRY_JOB_EVENT, &event);
                            let app_clone = app_for_thread.clone();
                            let pool_clone = pool.clone();
                            tauri::async_runtime::spawn(async move {
                                let _ = link_service::handle_telemetry_job_event(
                                    &app_clone,
                                    &pool_clone,
                                    &event,
                                )
                                .await;
                            });
                        }
                    }
                    Ok(None) => {}
                    Err(error) => {
                        bridge = None;
                        let status = SystemStatusPayload {
                            sdk_active: false,
                            telemetry_available: false,
                            message: Some(error),
                        };
                        if last_status.as_ref() != Some(&status) {
                            last_status = Some(status.clone());
                            let _ = app_for_thread.emit(EVT_SYSTEM_STATUS, status);
                        }
                    }
                }

                std::thread::sleep(Duration::from_millis(250));
            }
        });
        crate::dev_log!("[trace] END telemetry_shared_mem_startup duration_ms=0");
    }
}

#[cfg(target_os = "windows")]
pub use platform::start;

#[cfg(not(target_os = "windows"))]
pub fn start(app: AppHandle, _pool: SqlitePool) {
    let _ = app.emit(
        EVT_SYSTEM_STATUS,
        SystemStatusPayload {
            sdk_active: false,
            telemetry_available: false,
            message: Some("Shared memory telemetry is only available on Windows".to_string()),
        },
    );
}
