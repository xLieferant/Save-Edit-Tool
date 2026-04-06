use std::collections::HashSet;

use chrono::Utc;
use rusqlite::{params, Connection};

use crate::features::career::dispatcher::models::{
    DISPATCHER_DEFAULT_INTERVAL_MINUTES, DISPATCHER_DEFAULT_MAX_OPEN_JOBS,
};
use crate::shared::sqlite_schema::{create_indexes, ensure_columns};

pub(super) fn ensure_dispatcher_tables(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS dispatcher_jobs (
            id TEXT PRIMARY KEY,
            source_type TEXT NOT NULL DEFAULT 'generated',
            company_id TEXT NOT NULL,
            company_name TEXT NOT NULL,
            job_type TEXT NOT NULL,
            cargo_type TEXT NOT NULL,
            origin_city TEXT NOT NULL,
            origin_country TEXT NOT NULL,
            destination_city TEXT NOT NULL,
            destination_country TEXT NOT NULL,
            distance_km REAL NOT NULL,
            cargo_mass_kg REAL NOT NULL,
            urgency_level TEXT NOT NULL,
            difficulty_level TEXT NOT NULL,
            equipment_type_required TEXT NOT NULL,
            trailer_type_required TEXT,
            base_rate_per_km REAL NOT NULL,
            calculated_rate_per_km REAL NOT NULL,
            total_reward INTEGER NOT NULL,
            estimated_duration_minutes INTEGER NOT NULL DEFAULT 0,
            payment_tier_snapshot TEXT NOT NULL,
            payment_multiplier_snapshot REAL NOT NULL,
            country_multiplier_snapshot REAL NOT NULL,
            reputation_multiplier_snapshot REAL NOT NULL,
            cargo_multiplier_snapshot REAL NOT NULL,
            urgency_multiplier_snapshot REAL NOT NULL,
            equipment_multiplier_snapshot REAL NOT NULL,
            market_variation_snapshot REAL NOT NULL,
            customer_multiplier_snapshot REAL NOT NULL,
            company_reputation INTEGER NOT NULL,
            fuel_cost_estimate INTEGER NOT NULL DEFAULT 0,
            profit_estimate INTEGER NOT NULL DEFAULT 0,
            risk_note TEXT,
            bonus_note TEXT,
            expires_at_utc TEXT,
            status TEXT NOT NULL,
            progress_km REAL NOT NULL DEFAULT 0,
            profile_reference TEXT,
            save_reference TEXT,
            quicksave_reference TEXT,
            save_session_id TEXT,
            route_reference TEXT,
            ets2_job_link_status TEXT DEFAULT 'pending_route',
            accepted_at_utc TEXT,
            completed_at_utc TEXT,
            created_at_utc TEXT NOT NULL,
            updated_at_utc TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS dispatcher_offers (
            id TEXT PRIMARY KEY,
            company_id TEXT NOT NULL,
            company_name TEXT NOT NULL,
            user_id TEXT NOT NULL,
            offer_type TEXT NOT NULL,
            requested_job_type TEXT NOT NULL,
            requested_cargo_type TEXT,
            requested_region TEXT,
            proposed_rate_per_km REAL,
            note TEXT,
            equipment_type TEXT,
            contract_scope TEXT,
            status TEXT NOT NULL,
            counter_rate_per_km REAL,
            final_rate_per_km REAL,
            response_reason TEXT,
            linked_job_id TEXT,
            created_at_utc TEXT NOT NULL,
            updated_at_utc TEXT NOT NULL,
            expires_at_utc TEXT
        );

        CREATE TABLE IF NOT EXISTS dispatcher_contracts (
            id TEXT PRIMARY KEY,
            company_id TEXT NOT NULL,
            user_id TEXT NOT NULL,
            contract_type TEXT NOT NULL,
            agreed_rate_modifier REAL NOT NULL,
            preferred_cargo_type TEXT,
            region_scope TEXT,
            active_from_utc TEXT NOT NULL,
            active_until_utc TEXT,
            status TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS dispatcher_generation_config (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            interval_minutes INTEGER NOT NULL DEFAULT 10,
            max_open_jobs INTEGER NOT NULL DEFAULT 24,
            last_generated_at_utc TEXT,
            last_cleanup_at_utc TEXT
        );
        "#,
    )
    .map_err(|e| e.to_string())?;

    ensure_dispatcher_job_columns(conn)?;
    ensure_dispatcher_offer_columns(conn)?;
    ensure_dispatcher_contract_columns(conn)?;
    ensure_dispatcher_generation_config_columns(conn)?;
    ensure_dispatcher_generation_config(conn)?;
    ensure_dispatcher_indexes(conn)?;

    Ok(())
}

fn ensure_dispatcher_job_columns(conn: &Connection) -> Result<(), String> {
    let columns = ensure_columns(
        conn,
        "dispatcher_jobs",
        &[
            ("source_type", "TEXT NOT NULL DEFAULT 'generated'"),
            ("company_id", "TEXT NOT NULL DEFAULT ''"),
            ("company_name", "TEXT NOT NULL DEFAULT ''"),
            ("job_type", "TEXT NOT NULL DEFAULT 'freight_market'"),
            ("cargo_type", "TEXT NOT NULL DEFAULT 'standard'"),
            ("origin_city", "TEXT NOT NULL DEFAULT ''"),
            ("origin_country", "TEXT NOT NULL DEFAULT ''"),
            ("destination_city", "TEXT NOT NULL DEFAULT ''"),
            ("destination_country", "TEXT NOT NULL DEFAULT ''"),
            ("distance_km", "REAL NOT NULL DEFAULT 0"),
            ("cargo_mass_kg", "REAL NOT NULL DEFAULT 0"),
            ("urgency_level", "TEXT NOT NULL DEFAULT 'normal'"),
            ("difficulty_level", "TEXT NOT NULL DEFAULT 'normal'"),
            ("equipment_type_required", "TEXT NOT NULL DEFAULT 'own_truck'"),
            ("trailer_type_required", "TEXT"),
            ("base_rate_per_km", "REAL NOT NULL DEFAULT 0"),
            ("calculated_rate_per_km", "REAL NOT NULL DEFAULT 0"),
            ("total_reward", "INTEGER NOT NULL DEFAULT 0"),
            ("estimated_duration_minutes", "INTEGER NOT NULL DEFAULT 0"),
            ("payment_tier_snapshot", "TEXT NOT NULL DEFAULT 'standard'"),
            ("payment_multiplier_snapshot", "REAL NOT NULL DEFAULT 1"),
            ("country_multiplier_snapshot", "REAL NOT NULL DEFAULT 1"),
            ("reputation_multiplier_snapshot", "REAL NOT NULL DEFAULT 1"),
            ("cargo_multiplier_snapshot", "REAL NOT NULL DEFAULT 1"),
            ("urgency_multiplier_snapshot", "REAL NOT NULL DEFAULT 1"),
            ("equipment_multiplier_snapshot", "REAL NOT NULL DEFAULT 1"),
            ("market_variation_snapshot", "REAL NOT NULL DEFAULT 1"),
            ("customer_multiplier_snapshot", "REAL NOT NULL DEFAULT 1"),
            ("company_reputation", "INTEGER NOT NULL DEFAULT 0"),
            ("fuel_cost_estimate", "INTEGER NOT NULL DEFAULT 0"),
            ("profit_estimate", "INTEGER NOT NULL DEFAULT 0"),
            ("risk_note", "TEXT"),
            ("bonus_note", "TEXT"),
            ("expires_at_utc", "TEXT"),
            ("status", "TEXT NOT NULL DEFAULT 'open'"),
            ("progress_km", "REAL NOT NULL DEFAULT 0"),
            ("profile_reference", "TEXT"),
            ("save_reference", "TEXT"),
            ("quicksave_reference", "TEXT"),
            ("save_session_id", "TEXT"),
            ("route_reference", "TEXT"),
            ("ets2_job_link_status", "TEXT DEFAULT 'pending_route'"),
            ("accepted_at_utc", "TEXT"),
            ("completed_at_utc", "TEXT"),
            ("created_at_utc", "TEXT NOT NULL DEFAULT ''"),
            ("updated_at_utc", "TEXT NOT NULL DEFAULT ''"),
        ],
    )?;

    ensure_dispatcher_primary_column("dispatcher_jobs", &columns, "id")?;
    backfill_dispatcher_text_column(
        conn,
        "dispatcher_jobs",
        &columns,
        "created_at_utc",
        &["created_at", "accepted_at_utc", "completed_at_utc"],
        &Utc::now().to_rfc3339(),
    )?;
    backfill_dispatcher_text_column(
        conn,
        "dispatcher_jobs",
        &columns,
        "updated_at_utc",
        &["updated_at", "created_at_utc", "accepted_at_utc", "completed_at_utc"],
        &Utc::now().to_rfc3339(),
    )?;
    conn.execute(
        "UPDATE dispatcher_jobs
         SET source_type = 'generated'
         WHERE source_type IS NULL OR trim(source_type) = ''",
        [],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE dispatcher_jobs
         SET status = 'open'
         WHERE status IS NULL OR trim(status) = ''",
        [],
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}

fn ensure_dispatcher_offer_columns(conn: &Connection) -> Result<(), String> {
    let columns = ensure_columns(
        conn,
        "dispatcher_offers",
        &[
            ("company_id", "TEXT NOT NULL DEFAULT ''"),
            ("company_name", "TEXT NOT NULL DEFAULT ''"),
            ("user_id", "TEXT NOT NULL DEFAULT 'local-player'"),
            ("offer_type", "TEXT NOT NULL DEFAULT 'quote_request'"),
            ("requested_job_type", "TEXT NOT NULL DEFAULT 'freight_market'"),
            ("requested_cargo_type", "TEXT"),
            ("requested_region", "TEXT"),
            ("proposed_rate_per_km", "REAL"),
            ("note", "TEXT"),
            ("equipment_type", "TEXT"),
            ("contract_scope", "TEXT"),
            ("status", "TEXT NOT NULL DEFAULT 'draft'"),
            ("counter_rate_per_km", "REAL"),
            ("final_rate_per_km", "REAL"),
            ("response_reason", "TEXT"),
            ("linked_job_id", "TEXT"),
            ("created_at_utc", "TEXT NOT NULL DEFAULT ''"),
            ("updated_at_utc", "TEXT NOT NULL DEFAULT ''"),
            ("expires_at_utc", "TEXT"),
        ],
    )?;

    ensure_dispatcher_primary_column("dispatcher_offers", &columns, "id")?;
    backfill_dispatcher_text_column(
        conn,
        "dispatcher_offers",
        &columns,
        "created_at_utc",
        &["created_at"],
        &Utc::now().to_rfc3339(),
    )?;
    backfill_dispatcher_text_column(
        conn,
        "dispatcher_offers",
        &columns,
        "updated_at_utc",
        &["updated_at", "created_at_utc"],
        &Utc::now().to_rfc3339(),
    )?;
    conn.execute(
        "UPDATE dispatcher_offers
         SET status = 'draft'
         WHERE status IS NULL OR trim(status) = ''",
        [],
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}

fn ensure_dispatcher_contract_columns(conn: &Connection) -> Result<(), String> {
    let columns = ensure_columns(
        conn,
        "dispatcher_contracts",
        &[
            ("company_id", "TEXT NOT NULL DEFAULT ''"),
            ("user_id", "TEXT NOT NULL DEFAULT 'local-player'"),
            ("contract_type", "TEXT NOT NULL DEFAULT 'contract'"),
            ("agreed_rate_modifier", "REAL NOT NULL DEFAULT 1"),
            ("preferred_cargo_type", "TEXT"),
            ("region_scope", "TEXT"),
            ("active_from_utc", "TEXT NOT NULL DEFAULT ''"),
            ("active_until_utc", "TEXT"),
            ("status", "TEXT NOT NULL DEFAULT 'active'"),
        ],
    )?;

    ensure_dispatcher_primary_column("dispatcher_contracts", &columns, "id")?;
    backfill_dispatcher_text_column(
        conn,
        "dispatcher_contracts",
        &columns,
        "active_from_utc",
        &["active_from", "created_at", "updated_at"],
        &Utc::now().to_rfc3339(),
    )?;
    conn.execute(
        "UPDATE dispatcher_contracts
         SET status = 'active'
         WHERE status IS NULL OR trim(status) = ''",
        [],
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}

fn ensure_dispatcher_generation_config_columns(conn: &Connection) -> Result<(), String> {
    let columns = ensure_columns(
        conn,
        "dispatcher_generation_config",
        &[
            (
                "interval_minutes",
                "INTEGER NOT NULL DEFAULT 10",
            ),
            ("max_open_jobs", "INTEGER NOT NULL DEFAULT 24"),
            ("last_generated_at_utc", "TEXT"),
            ("last_cleanup_at_utc", "TEXT"),
        ],
    )?;

    ensure_dispatcher_primary_column("dispatcher_generation_config", &columns, "id")?;
    Ok(())
}

fn ensure_dispatcher_indexes(conn: &Connection) -> Result<(), String> {
    create_indexes(
        conn,
        &[
            "CREATE INDEX IF NOT EXISTS idx_dispatcher_jobs_status ON dispatcher_jobs(status, created_at_utc DESC)",
            "CREATE INDEX IF NOT EXISTS idx_dispatcher_jobs_company ON dispatcher_jobs(company_id, status)",
            "CREATE INDEX IF NOT EXISTS idx_dispatcher_jobs_context ON dispatcher_jobs(profile_reference, save_reference, status)",
            "CREATE INDEX IF NOT EXISTS idx_dispatcher_jobs_source ON dispatcher_jobs(source_type, status, created_at_utc DESC)",
            "CREATE INDEX IF NOT EXISTS idx_dispatcher_offers_status ON dispatcher_offers(status, created_at_utc DESC)",
        ],
    )?;
    Ok(())
}

fn ensure_dispatcher_primary_column(
    table: &str,
    columns: &HashSet<String>,
    primary_key: &str,
) -> Result<(), String> {
    if columns.contains(primary_key) {
        return Ok(());
    }

    Err(format!(
        "{table} schema is invalid: missing required primary key column {primary_key}"
    ))
}

fn backfill_dispatcher_text_column(
    conn: &Connection,
    table: &str,
    columns: &HashSet<String>,
    target: &str,
    legacy_sources: &[&str],
    fallback: &str,
) -> Result<(), String> {
    let mut expressions = vec![format!("NULLIF({target}, '')")];
    for source in legacy_sources {
        if columns.contains(*source) {
            expressions.push(format!("NULLIF({source}, '')"));
        }
    }

    let sql = format!(
        "UPDATE {table}
         SET {target} = COALESCE({}, ?1)
         WHERE {target} IS NULL OR trim({target}) = ''",
        expressions.join(", ")
    );
    conn.execute(&sql, [fallback]).map_err(|e| e.to_string())?;

    Ok(())
}

pub(super) fn ensure_dispatcher_generation_config(conn: &Connection) -> Result<(), String> {
    ensure_dispatcher_generation_config_columns(conn)?;
    conn.execute(
        r#"
        INSERT OR IGNORE INTO dispatcher_generation_config (
            id,
            interval_minutes,
            max_open_jobs,
            last_generated_at_utc,
            last_cleanup_at_utc
        )
        VALUES (1, ?1, ?2, NULL, NULL)
        "#,
        params![
            DISPATCHER_DEFAULT_INTERVAL_MINUTES,
            DISPATCHER_DEFAULT_MAX_OPEN_JOBS
        ],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE dispatcher_generation_config
         SET interval_minutes = COALESCE(interval_minutes, ?1),
             max_open_jobs = COALESCE(max_open_jobs, ?2)
         WHERE id = 1",
        params![
            DISPATCHER_DEFAULT_INTERVAL_MINUTES,
            DISPATCHER_DEFAULT_MAX_OPEN_JOBS
        ],
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use super::ensure_dispatcher_tables;
    use crate::shared::sqlite_schema::existing_columns;

    #[test]
    fn dispatcher_legacy_jobs_schema_gets_columns_before_indexes() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            r#"
            CREATE TABLE dispatcher_jobs (
                id TEXT PRIMARY KEY,
                company_id TEXT NOT NULL,
                status TEXT NOT NULL
            );
            CREATE TABLE dispatcher_offers (id TEXT PRIMARY KEY);
            CREATE TABLE dispatcher_contracts (id TEXT PRIMARY KEY);
            CREATE TABLE dispatcher_generation_config (id INTEGER PRIMARY KEY CHECK (id = 1));
            "#,
        )
        .unwrap();

        ensure_dispatcher_tables(&conn).unwrap();

        let columns = existing_columns(&conn, "dispatcher_jobs").unwrap();
        assert!(columns.contains("profile_reference"));
        assert!(columns.contains("save_reference"));
        assert!(columns.contains("source_type"));
        assert!(columns.contains("created_at_utc"));

        let index_exists: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = 'idx_dispatcher_jobs_context'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(index_exists, 1);
    }

    #[test]
    fn dispatcher_aux_tables_get_required_columns() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            r#"
            CREATE TABLE dispatcher_jobs (id TEXT PRIMARY KEY);
            CREATE TABLE dispatcher_offers (id TEXT PRIMARY KEY);
            CREATE TABLE dispatcher_contracts (id TEXT PRIMARY KEY);
            CREATE TABLE dispatcher_generation_config (id INTEGER PRIMARY KEY CHECK (id = 1));
            "#,
        )
        .unwrap();

        ensure_dispatcher_tables(&conn).unwrap();

        let offer_columns = existing_columns(&conn, "dispatcher_offers").unwrap();
        assert!(offer_columns.contains("created_at_utc"));
        assert!(offer_columns.contains("updated_at_utc"));
        assert!(offer_columns.contains("status"));

        let contract_columns = existing_columns(&conn, "dispatcher_contracts").unwrap();
        assert!(contract_columns.contains("active_from_utc"));
        assert!(contract_columns.contains("status"));

        let generation_columns = existing_columns(&conn, "dispatcher_generation_config").unwrap();
        assert!(generation_columns.contains("interval_minutes"));
        assert!(generation_columns.contains("max_open_jobs"));

        let generation_row_exists: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM dispatcher_generation_config WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(generation_row_exists, 1);
    }
}
