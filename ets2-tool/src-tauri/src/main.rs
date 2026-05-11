#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use crate::state::AuthState;
use crate::state::{AppProfileState, AppState, DecryptCache, EtsDbState, ProfileCache};
use crate::state::{CareerState, HubState};
use tauri::Manager;

mod commands;
mod db;
mod events;
mod features;
mod models;
mod shared; // ehemals utils
mod state; // ehemals commands (aufgeteilt)
mod xp;

fn main() {
    std::panic::set_hook(Box::new(|info| {
        crate::shared::logs::write_log(format!("[panic] {}", info));
        let _ = crate::shared::user_log::user_log_error("App", format!("Application panic: {}", info));
    }));
    crate::dev_log!("[app] starting");
    let _ = crate::shared::user_log::user_log_info("App", "Application start");

    features::career::scs_sdk_telemetry::start_terminal_telemetry_loop();
    features::career::telemetry_debug::start_telemetry_debug_thread();
    let sqlite_pool = tauri::async_runtime::block_on(db::sqlite::init_sqlite())
        .expect("failed to initialize central sqlite pool");
    let sqlite_path = db::sqlite::app_db_path();
    if let Err(error) = db::sqlite::validate_sqlite_extension(&sqlite_path) {
        panic!("{}", error);
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_fs::init())
        .manage(DecryptCache::default())
        .manage(AppProfileState::default())
        .manage(ProfileCache::default())
        .manage(HubState::default())
        .manage(CareerState::default())
        .manage(AuthState::default())
        .manage(EtsDbState {
            pool: sqlite_pool.clone(),
        })
        .manage(AppState {
            sqlite: sqlite_pool.clone(),
            sqlite_path: sqlite_path.clone(),
        })
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let handle = app.handle().clone();
            let career = app.state::<CareerState>();
            let runtime = career.runtime.clone();
            let auth = app.state::<AuthState>();
            crate::dev_log!("[app] setup begin");
            let _ = crate::shared::user_log::user_log_info("App", "App setup started");

            let db_path = db::sqlite::app_db_path();
            crate::dev_log!("[app] setup init db: {}", db_path.display());
            match features::career::db::init_logbook(&db_path) {
                Ok(()) => {
                    crate::dev_log!("[career] setup db ready: {}", db_path.display());
                    if let Ok(mut guard) = runtime.db_path.lock() {
                        *guard = Some(db_path);
                    }

                    if let Ok(mut conn) = rusqlite::Connection::open(db::sqlite::app_db_path()) {
                        if let Err(error) = features::auth::db::ensure_tables(&conn) {
                            crate::dev_log!("[auth] ensure tables failed: {}", error);
                        } else if let Err(error) = features::vtc::db::ensure_tables(&conn) {
                            crate::dev_log!("[vtc] ensure tables failed: {}", error);
                        } else if let Err(error) =
                            features::auth::service::seed_default_admin(&conn)
                        {
                            crate::dev_log!("[auth] seed admin failed: {}", error);
                        } else if let Err(error) =
                            features::auth::service::restore_persisted_session(&conn, auth.inner())
                        {
                            crate::dev_log!("[auth] restore session failed: {}", error);
                        } else {
                            crate::dev_log!("[auth] restore session ok");
                            if let Err(error) =
                                features::vtc::service::ensure_local_company_bootstrap(
                                    &conn,
                                    auth.inner(),
                                )
                            {
                                crate::dev_log!("[vtc] bootstrap failed: {}", error);
                            }
                        }

                        if let Err(error) = shared::ets2data::import::ensure_tables(&conn) {
                            crate::dev_log!("[ets2data] ensure tables failed: {}", error);
                        } else {
                            let company_count: i64 = conn
                                .query_row("SELECT COUNT(*) FROM ets2_companies", [], |row| {
                                    row.get(0)
                                })
                                .unwrap_or(0);
                            if company_count == 0 {
                                if let Err(error) = shared::ets2data::import::import_datasets(
                                    None,
                                    &mut conn,
                                    &shared::ets2data::default_repo_root(),
                                    false,
                                ) {
                                    crate::dev_log!(
                                        "[ets2data] auto import skipped/failed: {}",
                                        error
                                    );
                                } else {
                                    crate::dev_log!("[ets2data] auto import completed");
                                }
                            }
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
                    Err(error) => {
                        crate::dev_log!("[career] plugin install skipped for {:?}: {}", game, error)
                    }
                }
            }
            crate::dev_log!("[app] setup start telemetry bridge + background threads");
            crate::dev_log!("[trace] START telemetry_bridge_startup");
            features::career::scs_sdk_telemetry::start_frontend_telemetry_bridge(
                handle.clone(),
                runtime.clone(),
            );
            features::career::service::start_background(handle, runtime);
            let ets_db = app.state::<EtsDbState>();
            features::telemetry::scs_shared_mem::start(app.handle().clone(), ets_db.pool.clone());
            crate::dev_log!("[trace] END telemetry_bridge_startup duration_ms=0");
            crate::dev_log!("[app] setup complete");
            let _ = crate::shared::user_log::user_log_info("App", "App setup completed");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::ets_get_last_quicksave,
            commands::ets_prepare_job_link,
            commands::ets_write_job_to_quicksave,
            commands::ets_get_job_link_status,
            commands::ets_snapshot_refresh_active_save,
            commands::ets_snapshot_get_active,
            commands::ets_snapshot_list_depots,
            commands::ets_snapshot_get_active_diagnostics,
            commands::get_sqlite_info,
            commands::get_sqlite_table_counts,
            commands::data_import_ets2_datasets,
            commands::ets2data_get_city,
            commands::ets2data_get_company,
            commands::ets2data_list_cities,
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
            features::save_editor::commands::apply_custom_reset_values,
            features::save_editor::commands::undo_last_save_change,
            features::save_editor::commands::get_undo_status,
            // Save safety
            features::backup::commands::list_active_save_backups,
            features::backup::commands::preview_backup_restore,
            features::backup::commands::restore_backup,
            features::health_monitor::commands::get_active_save_health,
            features::health_monitor::commands::apply_save_health_fix,
            // Save Analysis+
            features::save_analysis::reader::read_all_save_data,
            // features::save_analysis::reader::read_money,
            // features::save_analysis::reader::read_xp,
            features::save_analysis::reader::read_traffic_value,
            features::save_analysis::quicksave::quicksave_game_info,
            features::save_analysis::quicksave::get_current_truck_summary,
            features::save_analysis::commands::analyze_mod_conflict_diagnostics,
            features::save_analysis::commands::export_mod_conflict_diagnostics_report,
            features::mod_profile_manager::commands::load_mod_profile_manager_state,
            features::mod_profile_manager::commands::scan_mods,
            features::mod_profile_manager::commands::list_mod_presets,
            features::mod_profile_manager::commands::create_mod_preset,
            features::mod_profile_manager::commands::compare_mod_preset,
            features::mod_profile_manager::commands::export_mod_preset,
            features::mod_profile_manager::commands::import_mod_preset,
            features::mod_profile_manager::commands::delete_mod_preset,
            features::mod_profile_manager::commands::select_manual_workshop_directory,
            features::mod_profile_manager::commands::clear_manual_workshop_directory,
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
            features::profile_sharing::commands::get_profile_share_context,
            features::profile_sharing::commands::pick_shared_profile_import_archive,
            features::profile_sharing::commands::pick_shared_profile_export_directory,
            features::profile_sharing::commands::export_shared_profile,
            features::profile_sharing::commands::inspect_shared_profile_archive,
            features::profile_sharing::commands::import_shared_profile,
            // Language Management
            features::language::commands::get_available_languages_command,
            features::language::commands::get_current_language_command,
            features::language::commands::set_language_command,
            features::language::commands::translate_command,
            // User action logging
            features::logging::commands::log_user_action,
            features::logging::commands::log_diagnostics_event,
            features::logging::commands::get_user_logs,
            features::logging::commands::export_user_logs,
            features::logging::commands::clear_user_logs,
            features::logging::commands::get_log_status,
            features::logging::commands::build_support_report,
            features::logging::commands::export_logs_bundle,
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
            features::career::commands::dispatcher_generate_jobs,
            features::career::commands::dispatcher_get_market_jobs,
            features::career::commands::dispatcher_get_open_jobs,
            features::career::commands::dispatcher_get_job_details,
            features::career::commands::dispatcher_get_job_by_id,
            features::career::commands::dispatcher_accept_job,
            features::career::commands::dispatcher_get_active_jobs,
            features::career::commands::dispatcher_cancel_job,
            features::career::commands::dispatcher_get_job_history,
            features::career::commands::dispatcher_get_company_contacts,
            features::career::commands::dispatcher_create_offer,
            features::career::commands::dispatcher_get_offers,
            features::career::commands::dispatcher_cancel_offer,
            features::career::commands::dispatcher_respond_to_counter,
            features::career::commands::dispatcher_get_dispatcher_overview,
            features::career::commands::dispatcher_generate_universal_jobs,
            features::career::commands::dispatcher_get_generation_status,
            features::career::commands::dispatcher_cleanup_expired_jobs,
            features::career::commands::dispatcher_restore_jobs_for_last_quicksave,
            features::career::commands::dispatcher_link_job_to_save_context,
            features::career::commands::dispatcher_assign_job_to_active_save,
            features::career::commands::dispatcher_assign_and_prepare_ets_link,
            features::career::commands::dispatcher_assign_and_prepare_and_write,
            features::career::commands::dispatcher_accept_generated_job,
            features::career::commands::dispatcher_mark_job_synced_to_ets2,
            features::career::commands::dispatcher_get_jobs_by_save_context,
            features::career::commands::dispatcher_get_jobs_for_active_save,
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
            // VTC / Career Management
            features::vtc::commands::get_current_user_profile,
            features::vtc::commands::get_vtc_runtime_context,
            features::vtc::commands::update_user_language,
            features::vtc::commands::update_username,
            features::vtc::commands::check_username_availability,
            features::vtc::commands::update_user_profile_meta,
            features::vtc::commands::create_company,
            features::vtc::commands::get_company_overview,
            features::vtc::commands::update_company_profile,
            features::vtc::commands::get_company_members,
            features::vtc::commands::update_company_settings,
            features::vtc::commands::assign_member_role,
            features::vtc::commands::change_member_role,
            features::vtc::commands::get_available_roles,
            features::vtc::commands::get_user_settings,
            features::vtc::commands::update_user_settings,
            features::vtc::commands::get_company_settings,
            features::vtc::commands::get_career_settings,
            features::vtc::commands::update_career_settings,
            // Onboarding
            features::career_onboarding::commands::career_get_onboarding_state,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
