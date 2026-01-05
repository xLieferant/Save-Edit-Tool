#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use crate::state::{AppProfileState, DecryptCache};

mod models;
mod state;
mod shared;   // ehemals utils
mod features; // ehemals commands (aufgeteilt)

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_fs::init())
        .manage(DecryptCache::default())
        .manage(AppProfileState::default())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_opener::init())
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
                
                //Feature: Profile Controls move around
                features::profile_controls::commands::copy_profile_controls,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
