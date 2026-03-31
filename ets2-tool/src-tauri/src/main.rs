#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use crate::state::{AppProfileState, DecryptCache, ProfileCache};
use crate::state::{CareerState, HubState};
use tauri::Manager;

mod models;
mod state;
mod shared;   // ehemals utils
mod features; // ehemals commands (aufgeteilt)

fn main() {
    features::career::scs_sdk_telemetry::start_terminal_telemetry_loop();
    features::career::telemetry_debug::start_telemetry_debug_thread();

    tauri::Builder::default()
        .plugin(tauri_plugin_fs::init())
        .manage(DecryptCache::default())
        .manage(AppProfileState::default())
        .manage(ProfileCache::default())
        .manage(HubState::default())
        .manage(CareerState::default())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let handle = app.handle().clone();
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
            let runtime = app.state::<CareerState>().runtime.clone();
            features::career::scs_sdk_telemetry::start_frontend_telemetry_bridge(
                handle.clone(),
                runtime.clone(),
            );
            features::career::service::start_background(handle, runtime);
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
            features::career::commands::career_list_trips,
            features::career::commands::get_plugin_status,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
