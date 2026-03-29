use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Manager};

use crate::features::career::{db, overlay, telemetry};
use crate::features::hub::events::CareerStatus;
use crate::state::CareerRuntime;

pub fn start_background(app: AppHandle, runtime: Arc<CareerRuntime>) {
    std::thread::spawn(move || {
        let db_path = db::default_db_path();
        if let Err(e) = db::init_logbook(&db_path) {
            crate::dev_log!("[career] db init failed: {}", e);
        } else {
            crate::dev_log!("[career] db ready: {}", db_path.display());
        }
        *runtime.db_path.lock().unwrap() = Some(db_path);

        let mut last_status: Option<CareerStatus> = None;

        loop {
            if runtime.stop_all.load(Ordering::Relaxed) {
                break;
            }

            let ets2_running = is_process_running("eurotrucks2.exe");
            let ats_running = is_process_running("amtrucks.exe");

            runtime.ets2_running.store(ets2_running, Ordering::Relaxed);
            runtime.ats_running.store(ats_running, Ordering::Relaxed);

            let game_running = ets2_running || ats_running;

            if game_running {
                telemetry::ensure_running(app.clone(), runtime.clone());
                let _ = overlay::ensure_overlay(&app);
            } else {
                runtime.telemetry_stop.store(true, Ordering::Relaxed);
                let _ = overlay::hide_overlay(&app);
            }

            let status = CareerStatus {
                ets2_running,
                ats_running,
                telemetry_running: runtime.telemetry_running.load(Ordering::Relaxed),
            };

            if last_status.as_ref() != Some(&status) {
                last_status = Some(status.clone());
                let _ = app.emit_all("career://status", status);
            }

            std::thread::sleep(Duration::from_millis(1000));
        }
    });
}

#[cfg(target_os = "windows")]
fn is_process_running(exe_name: &str) -> bool {
    use std::process::Command;

    let filter = format!("IMAGENAME eq {}", exe_name);
    let output = Command::new("tasklist")
        .arg("/FI")
        .arg(filter)
        .arg("/NH")
        .output();

    let Ok(output) = output else {
        return false;
    };

    let stdout = String::from_utf8_lossy(&output.stdout).to_lowercase();
    stdout.contains(&exe_name.to_lowercase())
}

#[cfg(not(target_os = "windows"))]
fn is_process_running(_exe_name: &str) -> bool {
    false
}
