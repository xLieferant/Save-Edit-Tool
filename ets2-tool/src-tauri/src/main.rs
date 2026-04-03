#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use crate::state::{AppProfileState, DecryptCache, ProfileCache};
use crate::state::{CareerState, HubState};
use crate::state::AuthState;
use tauri::Manager;

mod models;
mod state;
mod shared;   // ehemals utils
mod features; // ehemals commands (aufgeteilt)

fn main() {
    std::panic::set_hook(Box::new(|info| {
        crate::shared::logs::write_log(format!("[panic] {}", info));
    }));
    crate::dev_log!("[app] starting");

    features::career::scs_sdk_telemetry::start_terminal_telemetry_loop();
    features::career::telemetry_debug::start_telemetry_debug_thread();

    tauri::Builder::default()
        .plugin(tauri_plugin_fs::init())
        .manage(DecryptCache::default())
        .manage(AppProfileState::default())
        .manage(ProfileCache::default())
        .manage(HubState::default())
        .manage(CareerState::default())
        .manage(AuthState::default())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let handle = app.handle().clone();
            let career = app.state::<CareerState>();
            let runtime = career.runtime.clone();
            let auth = app.state::<AuthState>();
            crate::dev_log!("[app] setup begin");

            let db_path = features::career::db::default_db_path();
            crate::dev_log!("[app] setup init db: {}", db_path.display());
            match features::career::db::init_logbook(&db_path) {
                Ok(()) => {
                    crate::dev_log!("[career] setup db ready: {}", db_path.display());
                    if let Ok(mut guard) = runtime.db_path.lock() {
                        *guard = Some(db_path);
                    }

                    if let Ok(conn) = rusqlite::Connection::open(features::auth::db::default_db_path()) {
                        if let Err(error) = features::auth::db::ensure_tables(&conn) {
                            crate::dev_log!("[auth] ensure tables failed: {}", error);
                        } else if let Err(error) = features::auth::service::seed_default_admin(&conn) {
                            crate::dev_log!("[auth] seed admin failed: {}", error);
                        } else if let Err(error) =
                            features::auth::service::restore_persisted_session(&conn, auth.inner())
                        {
                            crate::dev_log!("[auth] restore session failed: {}", error);
                        } else {
                            crate::dev_log!("[auth] restore session ok");
                        }
                    }
                }
                Err(error) => {
                    crate::dev_log!("[career] setup db init failed: {}", error);
                }
            }

            crate::dev_log!("[app] setup load hub mode");
            match features::hub::config::load_mode() {
                Ok(mode) => {
                    let hub = app.state::<HubState>();
                    if let Ok(mut guard) = hub.mode.write() {
                        *guard = mode;
                    }
                }
                Err(error) => {
                    crate::dev_log!("[hub] config load failed: {}", error);
                }
            }
            crate::dev_log!("[app] setup ensure plugin files");
            for game in [
                features::career::plugin_installer::ScsGame::Ets2,
                features::career::plugin_installer::ScsGame::Ats,
            ] {
                match features::career::plugin_installer::ensure_plugin_files(&handle, game) {
                    Ok(path) => crate::dev_log!(
                        "[career] plugin installed for {:?}: {}",
                        game,
                        path.display()
                    ),
                    Err(error) => crate::dev_log!(
                        "[career] plugin install skipped for {:?}: {}",
                        game,
                        error
                    ),
                }
            }
            crate::dev_log!("[app] setup start telemetry bridge + background threads");
            features::career::scs_sdk_telemetry::start_frontend_telemetry_bridge(
                handle.clone(),
                runtime.clone(),
            );
            features::career::service::start_background(handle, runtime);
            crate::dev_log!("[app] setup complete");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Apply Settings
            features::settings::apply_settings::apply_setting,
            // Read Base and Save Config.cfg
            features::settings::game_config::read_base_config,
            features::settings::game_config::read_save_config,
            // Profile Manager
            features::profile_manager::commands::find_ets2_profiles,
            features::profile_manager::commands::load_profile,
            features::profile_manager::commands::save_profiles_cache,
            features::profile_manager::commands::read_profiles_cache,
            features::profile_manager::commands::save_last_profile,
            features::profile_manager::commands::read_last_profile,
            features::profile_manager::commands::find_profile_saves,
            features::profile_manager::commands::switch_profile,
            features::profile_manager::commands::set_active_profile,
            features::profile_manager::commands::set_current_save,
            features::profile_manager::commands::set_selected_game,
            features::profile_manager::commands::get_selected_game,
            // Profile Editing
            features::save_editor::commands::edit_money,
            features::save_editor::commands::edit_xp,
            features::save_editor::commands::edit_level,
            features::save_editor::commands::edit_config_value,
            features::save_editor::commands::edit_save_config_value,
            features::save_editor::commands::edit_traffic_value,
            features::save_editor::commands::edit_parking_doubles_value,
            features::save_editor::commands::edit_developer_value,
            features::save_editor::commands::edit_console_value,
            features::save_editor::commands::edit_convoy_value,
            features::save_editor::commands::edit_player_money,
            features::save_editor::commands::edit_player_experience,
            features::save_editor::commands::edit_skill_value,
            
            // Save Analysis+
            features::save_analysis::reader::read_all_save_data,
            // features::save_analysis::reader::read_money,
            // features::save_analysis::reader::read_xp,
            features::save_analysis::reader::read_traffic_value,
            features::save_analysis::quicksave::quicksave_game_info,
            // Vehicles and trailers
            features::vehicles::trucks::get_all_trucks,
            features::vehicles::trucks::get_player_truck,
            features::vehicles::trailers::get_all_trailers,
            features::vehicles::trailers::get_player_trailer,
            features::vehicles::editor::set_player_truck_license_plate,
            features::vehicles::editor::set_player_trailer_license_plate,
            features::vehicles::editor::repair_player_truck,
            features::vehicles::editor::refuel_player_truck,
            features::vehicles::editor::set_player_truck_fuel,
            features::vehicles::editor::set_player_truck_wear,
            features::vehicles::editor::repair_player_trailer,
            features::vehicles::editor::set_player_trailer_cargo_mass,
            features::vehicles::editor::edit_truck_odometer,
            
            // FEATURE: PROFILE CLONE + Rename
            features::profile_clone::commands::clone_profile_command,
            features::profile_clone::commands::validate_clone_target,
            features::profile_rename::commands::profile_rename,
            features::profile_move_mods::commands::copy_mods_to_profile,

            // Language Management
            features::language::commands::get_available_languages_command,
            features::language::commands::get_current_language_command,
            features::language::commands::set_language_command,
            features::language::commands::translate_command,
            // User action logging
            features::logging::commands::log_user_action,
                
                //Feature: Profile Controls move around
                features::profile_controls::commands::copy_profile_controls,

            // Hub (UI navigation)
            features::hub::commands::hub_get_mode,
            features::hub::commands::hub_set_mode,

            // Career (background + logbook)
            features::career::commands::career_get_status,
            features::career::commands::career_get_overview,
            features::career::commands::career_get_active_job,
            features::career::commands::career_get_job_log,
            features::career::commands::career_get_job_stats,
            features::career::commands::career_list_trips,
            features::career::commands::career_generate_jobs,
            features::career::commands::career_accept_job,
            features::career::commands::career_complete_job,
            features::career::commands::get_plugin_status,

            // Auth
            features::auth::commands::auth_seed_default_admin,
            features::auth::commands::auth_register,
            features::auth::commands::auth_login,
            features::auth::commands::auth_logout,
            features::auth::commands::auth_get_current_user,
            features::auth::commands::auth_restore_session,
            features::auth::commands::auth_get_account_overview,
            features::auth::commands::auth_generate_recovery_codes,
            features::auth::commands::auth_reset_password_with_recovery_code,
            features::auth::commands::auth_admin_get_db_overview,

            // Companies
            features::companies::commands::company_create,
            features::companies::commands::company_create_onboarding,
            features::companies::commands::company_list,
            features::companies::commands::company_join,
            features::companies::commands::company_get_current,
            features::companies::commands::company_get_for_user,

            // Onboarding
            features::career_onboarding::commands::career_get_onboarding_state,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
