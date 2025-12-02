use crate::log; // This import is now used
use crate::models::save_game_data::SaveGameData;
use crate::utils::decrypt::decrypt_if_needed;
use crate::utils::paths::autosave_path;
use regex::Regex;
use std::env;
use tauri::command;

