#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod utils;
mod models;
mod logs;

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            // Profiles
            commands::profiles::find_ets2_profiles,
            commands::profiles::load_profile,
            // Readers
            commands::save_reader::read_money,
            commands::save_reader::read_xp,
            commands::save_reader::read_all_save_data,
            // Editors
            commands::save_editor::edit_money,
            commands::save_editor::edit_xp,
            commands::save_editor::edit_level,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri app");
}
