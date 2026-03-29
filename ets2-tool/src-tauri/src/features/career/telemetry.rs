use serde::Serialize;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Manager};

use crate::state::CareerRuntime;

#[derive(Debug, Clone, Serialize)]
pub struct TelemetryTick {
    pub speed_kph: f32,
    pub gear: i32,
}

pub fn ensure_running(app: AppHandle, runtime: Arc<CareerRuntime>) {
    runtime.telemetry_stop.store(false, Ordering::Relaxed);

    if runtime
        .telemetry_running
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return;
    }

    crate::dev_log!("[career] telemetry placeholder thread started");

    std::thread::spawn(move || {
        while !runtime.stop_all.load(Ordering::Relaxed) && !runtime.telemetry_stop.load(Ordering::Relaxed) {
            let _ = app.emit_all(
                "career://telemetry_tick",
                TelemetryTick {
                    speed_kph: 0.0,
                    gear: 0,
                },
            );
            std::thread::sleep(Duration::from_millis(250));
        }

        runtime.telemetry_running.store(false, Ordering::Relaxed);
        crate::dev_log!("[career] telemetry placeholder thread stopped");
    });
}
