#[cfg(not(target_os = "windows"))]
use sqlx::SqlitePool;
#[cfg(not(target_os = "windows"))]
use tauri::{AppHandle, Emitter};

#[cfg(not(target_os = "windows"))]
use crate::events::EVT_SYSTEM_STATUS;
#[cfg(not(target_os = "windows"))]
use crate::features::telemetry::events::SystemStatusPayload;

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
    use crate::features::telemetry::simnexus_protocol::{
        self, LEGACY_SHARED_MEMORY_NAME, SHARED_MEMORY_NAME,
    };

    struct SharedBridge {
        handle: HANDLE,
        view: MEMORY_MAPPED_VIEW_ADDRESS,
    }

    impl SharedBridge {
        fn connect() -> Result<Self, String> {
            let segment_name = wide_null(SHARED_MEMORY_NAME);
            let handle = unsafe { OpenFileMappingW(FILE_MAP_READ, 0, segment_name.as_ptr()) };
            if handle.is_null() {
                if mapping_exists(LEGACY_SHARED_MEMORY_NAME) {
                    return Err(
                        "Legacy SimNexus mapping found: DLL protocol 1, app requires protocol 3"
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
                return Err("Failed to map SimNexus shared memory".to_string());
            }

            Ok(Self { handle, view })
        }

        fn read_event(&self) -> Result<Option<(TelemetryJobEventPayload, u64)>, String> {
            let Some(snapshot) =
                (unsafe { simnexus_protocol::read_consistent(self.view.Value.cast::<u8>())? })
            else {
                return Ok(None);
            };
            simnexus_protocol::validate_liveness(&snapshot.payload)?;
            let payload = snapshot.payload;

            let cargo_id = simnexus_protocol::bytes_to_string(&payload.cargo_id);
            let cargo = simnexus_protocol::bytes_to_string(&payload.cargo);
            let source_city_id = simnexus_protocol::bytes_to_string(&payload.source_city_id);
            let source_city = simnexus_protocol::bytes_to_string(&payload.source_city);
            let destination_city_id =
                simnexus_protocol::bytes_to_string(&payload.destination_city_id);
            let destination_city = simnexus_protocol::bytes_to_string(&payload.destination_city);
            let source_company_id = simnexus_protocol::bytes_to_string(&payload.source_company_id);
            let source_company = simnexus_protocol::bytes_to_string(&payload.source_company);
            let destination_company_id =
                simnexus_protocol::bytes_to_string(&payload.destination_company_id);
            let destination_company =
                simnexus_protocol::bytes_to_string(&payload.destination_company);
            let job_finished = payload.job_event != 0;

            Ok(Some((
                TelemetryJobEventPayload {
                    sdk_active: true,
                    paused: payload.game_paused != 0,
                    on_job: payload.job_active != 0,
                    job_finished,
                    job_delivered: payload.job_event == 1,
                    job_cancelled: payload.job_event == 2,
                    job_result: match payload.job_event {
                        1 => Some("completed".to_string()),
                        2 => Some("cancelled".to_string()),
                        value if value != 0 => Some("finished".to_string()),
                        _ => None,
                    },
                    cargo_id: optional(cargo_id),
                    cargo: optional(cargo),
                    city_src_id: optional(source_city_id),
                    city_src: optional(source_city),
                    comp_src_id: optional(source_company_id),
                    comp_src: optional(source_company),
                    city_dst_id: optional(destination_city_id),
                    city_dst: optional(destination_city),
                    comp_dst_id: optional(destination_company_id),
                    comp_dst: optional(destination_company),
                    planned_distance_km: payload.job_planned_distance_km,
                    route_distance: payload.job_planned_distance_km,
                    route_time: payload.job_delivery_time_min as i64,
                    job_income: payload.job_income,
                    job_delivered_revenue: payload.job_income,
                },
                payload.job_event_sequence,
            )))
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

    fn optional(value: String) -> Option<String> {
        if value.is_empty() { None } else { Some(value) }
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
            let mut last_job_event_sequence = 0u64;

            while !stop.load(Ordering::Relaxed) {
                if bridge.is_none() {
                    match SharedBridge::connect() {
                        Ok(client) => bridge = Some(client),
                        Err(error) => {
                            emit_status(
                                &app_for_thread,
                                &mut last_status,
                                SystemStatusPayload {
                                    sdk_active: false,
                                    telemetry_available: false,
                                    message: Some(error),
                                },
                            );
                            std::thread::sleep(Duration::from_millis(750));
                            continue;
                        }
                    }
                }

                match bridge.as_ref().unwrap().read_event() {
                    Ok(Some((event, job_event_sequence))) => {
                        emit_status(
                            &app_for_thread,
                            &mut last_status,
                            SystemStatusPayload {
                                sdk_active: true,
                                telemetry_available: true,
                                message: None,
                            },
                        );
                        if last_event.as_ref() != Some(&event)
                            || last_job_event_sequence != job_event_sequence
                        {
                            last_event = Some(event.clone());
                            last_job_event_sequence = job_event_sequence;
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
                        emit_status(
                            &app_for_thread,
                            &mut last_status,
                            SystemStatusPayload {
                                sdk_active: false,
                                telemetry_available: false,
                                message: Some(error),
                            },
                        );
                    }
                }

                std::thread::sleep(Duration::from_millis(250));
            }
        });
        crate::dev_log!("[trace] END telemetry_shared_mem_startup duration_ms=0");
    }

    fn emit_status(
        app: &AppHandle,
        previous: &mut Option<SystemStatusPayload>,
        status: SystemStatusPayload,
    ) {
        if previous.as_ref() == Some(&status) {
            return;
        }
        *previous = Some(status.clone());
        let _ = app.emit(EVT_SYSTEM_STATUS, status);
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
