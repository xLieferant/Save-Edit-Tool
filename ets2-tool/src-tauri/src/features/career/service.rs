use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

use crate::features::career::plugin_installer::{self, ScsGame};
use crate::features::career::telemetry::GameId;
use crate::features::career::{db, overlay, overview, telemetry};
use crate::features::hub::events::CareerStatus;
use crate::state::{AppProfileState, CareerRuntime};

fn choose_game(ets2_running: bool, ats_running: bool, previous: Option<&str>) -> Option<GameId> {
    match (ets2_running, ats_running, previous) {
        (true, true, Some("ats")) => Some(GameId::Ats),
        (true, _, _) => Some(GameId::Ets2),
        (false, true, _) => Some(GameId::Ats),
        _ => None,
    }
}

fn selected_game(app: &AppHandle) -> Option<GameId> {
    let profile_state = app.state::<AppProfileState>();
    let selected = profile_state.selected_game.lock().ok()?.clone();
    GameId::try_from(selected.as_str()).ok()
}

fn to_scs_game(game: GameId) -> ScsGame {
    match game {
        GameId::Ets2 => ScsGame::Ets2,
        GameId::Ats => ScsGame::Ats,
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
        let mut last_overview: Option<overview::CareerOverview> = None;
        let mut last_game_running: Option<bool> = None;
        let mut last_plugin_installed: Option<bool> = None;
        let mut last_bridge_connected: Option<bool> = None;
        let mut ets2_install_attempted = false;
        let mut ats_install_attempted = false;

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

            let status_game = active_game.or_else(|| selected_game(&app));
            let _plugin_files_ready = status_game
                .map(|game| {
                    let scs_game = to_scs_game(game);
                    let installed_now =
                        plugin_installer::plugin_file_installed(scs_game).unwrap_or(false);

                    match scs_game {
                        ScsGame::Ets2 => {
                            if installed_now {
                                ets2_install_attempted = false;
                                true
                            } else {
                                if !ets2_install_attempted {
                                    ets2_install_attempted = true;
                                    if let Err(error) =
                                        plugin_installer::ensure_plugin_files(&app, scs_game)
                                    {
                                        crate::dev_log!(
                                            "[career] plugin auto-install failed for ETS2: {}",
                                            error
                                        );
                                    }
                                }
                                plugin_installer::plugin_file_installed(scs_game).unwrap_or(false)
                            }
                        }
                        ScsGame::Ats => {
                            if installed_now {
                                ats_install_attempted = false;
                                true
                            } else {
                                if !ats_install_attempted {
                                    ats_install_attempted = true;
                                    if let Err(error) =
                                        plugin_installer::ensure_plugin_files(&app, scs_game)
                                    {
                                        crate::dev_log!(
                                            "[career] plugin auto-install failed for ATS: {}",
                                            error
                                        );
                                    }
                                }
                                plugin_installer::plugin_file_installed(scs_game).unwrap_or(false)
                            }
                        }
                    }
                })
                .unwrap_or(false);

            if let Some(game) = active_game {
                telemetry::ensure_running(app.clone(), runtime.clone(), game);
                let _ = overlay::ensure_overlay(&app);
            } else {
                runtime.telemetry_stop.store(true, Ordering::Relaxed);
                runtime.plugin_installed.store(false, Ordering::Relaxed);
                runtime.bridge_connected.store(false, Ordering::Relaxed);
                let _ = overlay::hide_overlay(&app);
            }

            let game_running = ets2_running || ats_running;
            let plugin_installed = runtime.plugin_installed.load(Ordering::Relaxed);
            let bridge_connected = runtime.bridge_connected.load(Ordering::Relaxed);

            if last_game_running != Some(game_running) {
                last_game_running = Some(game_running);
                let _ = app.emit("career://game_running", game_running);
            }

            if last_plugin_installed != Some(plugin_installed) {
                last_plugin_installed = Some(plugin_installed);
                let _ = app.emit("career://plugin_installed", plugin_installed);
            }

            if last_bridge_connected != Some(bridge_connected) {
                last_bridge_connected = Some(bridge_connected);
                let _ = app.emit("career://bridge_connected", bridge_connected);
            }

            let status = CareerStatus {
                ets2_running,
                ats_running,
                telemetry_running: runtime.telemetry_running.load(Ordering::Relaxed),
                plugin_installed,
                bridge_connected,
                active_game: runtime.active_game.lock().unwrap().clone(),
            };

            if last_status.as_ref() != Some(&status) {
                last_status = Some(status.clone());
                let _ = app.emit("career://status", status);
            }

            let should_emit_overview =
                runtime.overview_dirty.swap(false, Ordering::Relaxed) || last_overview.is_none();

            if should_emit_overview {
                match overview::load_overview(runtime.as_ref()) {
                    Ok(next_overview) => {
                        if last_overview.as_ref() != Some(&next_overview) {
                            last_overview = Some(next_overview.clone());
                            let _ = app.emit("career://overview", next_overview);
                        }
                    }
                    Err(error) => {
                        crate::dev_log!("[career] overview refresh failed: {}", error);
                    }
                }
            }

            std::thread::sleep(Duration::from_millis(500));
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
