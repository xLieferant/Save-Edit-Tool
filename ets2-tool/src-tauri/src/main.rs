#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod logs;
mod models;
mod utils;

use tauri::generate_handler;

fn main() {
    tauri::Builder::default()
        .invoke_handler(generate_handler![
            // Profile Commands
            commands::profiles::find_ets2_profiles,
            commands::profiles::load_profile,

            // SaveReader
            commands::save_reader::read_money,
            commands::save_reader::read_xp,
            commands::save_reader::read_all_save_data,

            // SaveEditor
            commands::save_editor::edit_money,
            commands::save_editor::edit_xp,
            commands::save_editor::edit_level,

            // Config
            commands::save_config::read_save_config,
            commands::global_config::read_base_config,

            // Quicksave
            commands::quicksave_game::quicksave_game_info,

            // Trucks
            commands::trucks::get_all_trucks,
            commands::trucks::get_player_truck,

            //Trailer 
            commands::trailers::get_all_trailers,
            commands::trailers::get_player_trailer,
        ])
        .run(tauri::generate_context!())
        .expect("Error while running Tauri app");
}
