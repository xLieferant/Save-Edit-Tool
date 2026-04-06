use rusqlite::Connection;

use super::models::{
    DispatcherGenerationConfigInput, DispatcherGenerationRunResult, DispatcherGenerationStatus,
    DispatcherSaveContext,
};
use super::{
    apply_dispatcher_generation_config, build_dispatcher_generation_status,
    ensure_dispatcher_market_jobs, expire_dispatcher_market_jobs, prepare_dispatcher_system,
};

pub(super) fn dispatcher_get_generation_status(
    conn: &Connection,
    save_context: &DispatcherSaveContext,
) -> Result<DispatcherGenerationStatus, String> {
    prepare_dispatcher_system(conn)?;
    build_dispatcher_generation_status(conn, save_context)
}

pub(super) fn dispatcher_generate_jobs(
    conn: &Connection,
    save_context: &DispatcherSaveContext,
) -> Result<DispatcherGenerationStatus, String> {
    prepare_dispatcher_system(conn)?;
    Ok(ensure_dispatcher_market_jobs(conn, save_context, true)?.status)
}

pub(super) fn dispatcher_generate_universal_jobs(
    conn: &Connection,
    save_context: &DispatcherSaveContext,
    force: bool,
    config: Option<DispatcherGenerationConfigInput>,
) -> Result<DispatcherGenerationStatus, String> {
    prepare_dispatcher_system(conn)?;
    apply_dispatcher_generation_config(conn, config)?;
    Ok(ensure_dispatcher_market_jobs(conn, save_context, force)?.status)
}

pub(super) fn dispatcher_cleanup_expired_jobs(
    conn: &Connection,
    save_context: &DispatcherSaveContext,
) -> Result<DispatcherGenerationStatus, String> {
    prepare_dispatcher_system(conn)?;
    expire_dispatcher_market_jobs(conn)?;
    build_dispatcher_generation_status(conn, save_context)
}

pub(super) fn dispatcher_restore_jobs_for_last_quicksave(
    conn: &Connection,
    save_context: &DispatcherSaveContext,
) -> Result<DispatcherGenerationStatus, String> {
    prepare_dispatcher_system(conn)?;
    Ok(ensure_dispatcher_market_jobs(conn, save_context, false)?.status)
}

pub(super) fn dispatcher_background_tick(
    conn: &Connection,
    save_context: &DispatcherSaveContext,
) -> Result<DispatcherGenerationRunResult, String> {
    prepare_dispatcher_system(conn)?;
    ensure_dispatcher_market_jobs(conn, save_context, false)
}
