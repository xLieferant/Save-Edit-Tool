#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

mod commands;
mod logs;
mod models;
mod utils;

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            // apply_setting.rs
            commands::apply_setting::apply_setting,

            // global_config.rs
            commands::global_config::read_base_config,

            // profiles.rs
            commands::profiles::find_ets2_profiles,
            commands::profiles::load_profile,

            // quicksave_game.rs
            commands::quicksave_game::quicksave_game_info,

            // save_config.rs
            commands::save_config::read_save_config,

            // save_editor.rs
            commands::save_editor::edit_money,
            commands::save_editor::edit_xp,
            commands::save_editor::edit_level,
            commands::save_editor::edit_truck_odometer,
            commands::save_editor::edit_truck_license_plate,
            commands::save_editor::edit_config_value,
            commands::save_editor::edit_save_config_value,
            commands::save_editor::edit_traffic_value,
            commands::save_editor::edit_parking_doubles_value,
            commands::save_editor::edit_developer_value,
            commands::save_editor::edit_console_value,
            commands::save_editor::edit_convoy_value,
            commands::save_editor::edit_player_money,
            commands::save_editor::edit_player_experience,
            commands::save_editor::edit_skill_value,
            commands::save_editor::edit_truck_license_plate,
            


            // save_reader.rs
            commands::save_reader::read_money,
            commands::save_reader::read_xp,
            commands::save_reader::read_all_save_data,

            // trucks.rs
            commands::trucks::get_all_trucks,
            commands::trucks::get_player_truck,

            // trailers.rs
            commands::trailers::get_player_trailer,
            commands::trailers::get_all_trailers
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}