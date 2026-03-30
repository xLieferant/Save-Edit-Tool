use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter};

use crate::features::career::telemetry::GameId;
use crate::features::career::{db, overlay, telemetry};
use crate::features::hub::events::CareerStatus;
use crate::state::CareerRuntime;

fn choose_game(ets2_running: bool, ats_running: bool, previous: Option<&str>) -> Option<GameId> {
    match (ets2_running, ats_running, previous) {
        (true, true, Some("ats")) => Some(GameId::Ats),
        (true, _, _) => Some(GameId::Ets2),
        (false, true, _) => Some(GameId::Ats),
        _ => None,
    }
}

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

            let previous = runtime.active_game.lock().unwrap().clone();
            let active_game = choose_game(ets2_running, ats_running, previous.as_deref());

            *runtime.active_game.lock().unwrap() =
                active_game.map(|game| game.as_str().to_string());

            if let Some(game) = active_game {
                telemetry::ensure_running(app.clone(), runtime.clone(), game);
                let _ = overlay::ensure_overlay(&app);
            } else {
                runtime.telemetry_stop.store(true, Ordering::Relaxed);
                runtime.bridge_connected.store(false, Ordering::Relaxed);
                let _ = overlay::hide_overlay(&app);
            }

            let status = CareerStatus {
                ets2_running,
                ats_running,
                telemetry_running: runtime.telemetry_running.load(Ordering::Relaxed),
                bridge_connected: runtime.bridge_connected.load(Ordering::Relaxed),
                active_game: runtime.active_game.lock().unwrap().clone(),
            };

            if last_status.as_ref() != Some(&status) {
                last_status = Some(status.clone());
                let _ = app.emit("career://status", status);
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
