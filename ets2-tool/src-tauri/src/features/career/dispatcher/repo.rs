use chrono::Utc;
use rusqlite::{Connection, params};

use super::models::{
    DISPATCHER_ACTIVE_JOB_STATUSES, DISPATCHER_ALL_JOB_STATUSES, DISPATCHER_BUSY_JOB_STATUSES,
    DISPATCHER_HISTORY_JOB_STATUSES, DISPATCHER_OPEN_JOB_STATUSES, DispatcherHistoryResponse,
    DispatcherHistorySummary, DispatcherJobDetails, DispatcherJobFilter,
    DispatcherJobsBySaveContextResponse, DispatcherMarketJob, DispatcherSaveContext,
};
use super::{
    build_dispatcher_route_reference, count_dispatcher_jobs_by_status, dispatcher_equipment_ok,
    dispatcher_reputation_requirement_for, expire_dispatcher_market_jobs,
    list_dispatcher_jobs_by_status, load_dispatcher_job_by_id, load_dispatcher_job_by_id_any,
    matches_filter_text, prepare_dispatcher_system, to_dispatcher_market_job,
};

pub(super) fn dispatcher_get_jobs_by_save_context(
    conn: &Connection,
    save_context: &DispatcherSaveContext,
    status: Option<String>,
) -> Result<DispatcherJobsBySaveContextResponse, String> {
    prepare_dispatcher_system(conn)?;
    let jobs = if let Some(status) = status
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        list_dispatcher_jobs_by_status(conn, &[status], 240, save_context)?
    } else {
        list_dispatcher_jobs_by_status(conn, DISPATCHER_ALL_JOB_STATUSES, 240, save_context)?
    };

    Ok(DispatcherJobsBySaveContextResponse {
        context: save_context.clone(),
        jobs: jobs.into_iter().map(to_dispatcher_market_job).collect(),
    })
}

pub(super) fn dispatcher_get_jobs_for_active_save(
    conn: &Connection,
    save_context: &DispatcherSaveContext,
) -> Result<DispatcherJobsBySaveContextResponse, String> {
    dispatcher_get_jobs_by_save_context(conn, save_context, None)
}

pub(super) fn dispatcher_assign_job_to_active_save(
    conn: &Connection,
    job_id: &str,
    save_context: &DispatcherSaveContext,
) -> Result<DispatcherJobDetails, String> {
    prepare_dispatcher_system(conn)?;
    if !save_context.is_ready() {
        return Err("no_active_save".to_string());
    }

    let row = load_dispatcher_job_by_id_any(conn, job_id)?
        .ok_or_else(|| "dispatcher_job_not_found".to_string())?;

    match row.status.as_str() {
        "open" | "accepted" | "failed" => {}
        "assigned_to_save" | "prepared" | "injected" => {
            return Err("job_already_assigned".to_string());
        }
        _ => return Err("invalid_job_status".to_string()),
    }

    let route_reference = row.route_reference.unwrap_or_else(|| {
        build_dispatcher_route_reference(
            &row.company_id,
            &row.origin_country,
            &row.origin_city,
            &row.destination_country,
            &row.destination_city,
            &row.job_type,
        )
    });
    let now = Utc::now().to_rfc3339();

    conn.execute(
        r#"
        UPDATE dispatcher_jobs
        SET profile_reference = ?2,
            save_reference = ?3,
            quicksave_reference = ?4,
            save_session_id = ?5,
            route_reference = ?6,
            status = 'assigned_to_save',
            ets2_job_link_status = 'pending',
            last_error_code = NULL,
            last_error_message = NULL,
            updated_at_utc = ?7
        WHERE id = ?1
        "#,
        params![
            job_id,
            save_context.profile_reference.as_deref(),
            save_context.save_reference.as_deref(),
            save_context.quicksave_reference.as_deref(),
            save_context.save_session_id.as_deref(),
            route_reference,
            now,
        ],
    )
    .map_err(|e| e.to_string())?;

    dispatcher_get_job_details(conn, job_id, save_context)
}

pub(super) fn dispatcher_get_market_jobs(
    conn: &Connection,
    filter: Option<DispatcherJobFilter>,
    save_context: &DispatcherSaveContext,
) -> Result<Vec<DispatcherMarketJob>, String> {
    prepare_dispatcher_system(conn)?;
    super::ensure_dispatcher_market_jobs(conn, save_context, false)?;
    let mut rows =
        list_dispatcher_jobs_by_status(conn, DISPATCHER_OPEN_JOB_STATUSES, 80, save_context)?;

    if let Some(filter) = filter {
        rows = rows
            .into_iter()
            .filter(|row| {
                if let Some(search) = filter.search.as_deref() {
                    let haystack = format!(
                        "{} {} {} {} {}",
                        row.company_name,
                        row.origin_city,
                        row.destination_city,
                        row.job_type,
                        row.cargo_type
                    );
                    if !matches_filter_text(&haystack, search) {
                        return false;
                    }
                }
                if let Some(job_type) = filter.job_type.as_deref() {
                    if !row.job_type.eq_ignore_ascii_case(job_type) {
                        return false;
                    }
                }
                if let Some(company_id) = filter.company_id.as_deref() {
                    if !row.company_id.eq_ignore_ascii_case(company_id) {
                        return false;
                    }
                }
                if let Some(country) = filter.country.as_deref() {
                    if !row.origin_country.eq_ignore_ascii_case(country)
                        && !row.destination_country.eq_ignore_ascii_case(country)
                    {
                        return false;
                    }
                }
                if let Some(cargo_type) = filter.cargo_type.as_deref() {
                    if !row.cargo_type.eq_ignore_ascii_case(cargo_type) {
                        return false;
                    }
                }
                if let Some(urgency) = filter.urgency.as_deref() {
                    if !row.urgency_level.eq_ignore_ascii_case(urgency) {
                        return false;
                    }
                }
                if let Some(eq) = filter.equipment_type.as_deref() {
                    if !row.equipment_type_required.eq_ignore_ascii_case(eq) {
                        return false;
                    }
                }
                if let Some(tier) = filter.payment_tier.as_deref() {
                    if !row.payment_tier_snapshot.eq_ignore_ascii_case(tier) {
                        return false;
                    }
                }
                if let Some(min_distance) = filter.min_distance_km {
                    if row.distance_km < min_distance {
                        return false;
                    }
                }
                if let Some(max_distance) = filter.max_distance_km {
                    if row.distance_km > max_distance {
                        return false;
                    }
                }
                if let Some(min_rate) = filter.min_rate_per_km {
                    if row.calculated_rate_per_km < min_rate {
                        return false;
                    }
                }
                if let Some(max_rate) = filter.max_rate_per_km {
                    if row.calculated_rate_per_km > max_rate {
                        return false;
                    }
                }
                if let Some(min_total) = filter.min_total_reward {
                    if row.total_reward < min_total {
                        return false;
                    }
                }
                if let Some(max_total) = filter.max_total_reward {
                    if row.total_reward > max_total {
                        return false;
                    }
                }
                true
            })
            .collect::<Vec<_>>();

        match filter
            .sort_by
            .unwrap_or_else(|| "newest".to_string())
            .as_str()
        {
            "best_reward" => rows.sort_by(|a, b| b.total_reward.cmp(&a.total_reward)),
            "best_rate" => rows.sort_by(|a, b| {
                b.calculated_rate_per_km
                    .partial_cmp(&a.calculated_rate_per_km)
                    .unwrap_or(std::cmp::Ordering::Equal)
            }),
            "shortest_distance" => rows.sort_by(|a, b| {
                a.distance_km
                    .partial_cmp(&b.distance_km)
                    .unwrap_or(std::cmp::Ordering::Equal)
            }),
            "highest_urgency" => rows.sort_by(|a, b| b.urgency_level.cmp(&a.urgency_level)),
            _ => rows.sort_by(|a, b| b.created_at_utc.cmp(&a.created_at_utc)),
        }
    }

    Ok(rows.into_iter().map(to_dispatcher_market_job).collect())
}

pub(super) fn dispatcher_get_job_details(
    conn: &Connection,
    job_id: &str,
    save_context: &DispatcherSaveContext,
) -> Result<DispatcherJobDetails, String> {
    prepare_dispatcher_system(conn)?;
    let row = load_dispatcher_job_by_id(conn, job_id, save_context)?
        .ok_or_else(|| format!("dispatcher_job_not_found:{job_id}"))?;
    let drivers = vec![
        format!("payment_tier={}", row.payment_tier_snapshot),
        format!(
            "country_profile={} -> {}",
            row.origin_country, row.destination_country
        ),
        format!("reputation={}", row.company_reputation),
        format!("equipment={}", row.equipment_type_required),
    ];
    Ok(DispatcherJobDetails {
        job: to_dispatcher_market_job(row),
        payout_drivers: drivers,
    })
}

pub(super) fn dispatcher_get_job_by_id(
    conn: &Connection,
    job_id: &str,
    save_context: &DispatcherSaveContext,
) -> Result<DispatcherJobDetails, String> {
    dispatcher_get_job_details(conn, job_id, save_context)
}

pub(super) fn dispatcher_accept_job(
    conn: &Connection,
    job_id: &str,
    save_context: &DispatcherSaveContext,
) -> Result<DispatcherJobDetails, String> {
    prepare_dispatcher_system(conn)?;
    expire_dispatcher_market_jobs(conn)?;
    let row = load_dispatcher_job_by_id(conn, job_id, save_context)?
        .ok_or_else(|| format!("dispatcher_job_not_found:{job_id}"))?;

    if row.status != "open" {
        return Err("dispatcher_job_not_open".to_string());
    }
    if row
        .expires_at_utc
        .as_deref()
        .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())
        .map(|value| value.with_timezone(&Utc) < Utc::now())
        .unwrap_or(false)
    {
        return Err("dispatcher_job_expired".to_string());
    }

    let active_jobs =
        count_dispatcher_jobs_by_status(conn, DISPATCHER_BUSY_JOB_STATUSES, save_context, None)?;
    if active_jobs > 0 {
        return Err("dispatcher_active_job_exists".to_string());
    }

    let equipment_ok = dispatcher_equipment_ok(conn, &row.equipment_type_required)?;
    if !equipment_ok {
        return Err("dispatcher_equipment_requirement_not_met".to_string());
    }
    if (row.company_reputation as u16)
        < dispatcher_reputation_requirement_for(&row.difficulty_level)
    {
        return Err("dispatcher_reputation_requirement_not_met".to_string());
    }

    let now = Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE dispatcher_jobs SET status = 'accepted', accepted_at_utc = ?2, updated_at_utc = ?2 WHERE id = ?1",
        params![job_id, now],
    )
    .map_err(|e| e.to_string())?;
    dispatcher_get_job_details(conn, job_id, save_context)
}

pub(super) fn dispatcher_get_active_jobs(
    conn: &Connection,
    save_context: &DispatcherSaveContext,
) -> Result<Vec<DispatcherMarketJob>, String> {
    prepare_dispatcher_system(conn)?;
    let rows =
        list_dispatcher_jobs_by_status(conn, DISPATCHER_ACTIVE_JOB_STATUSES, 60, save_context)?;
    Ok(rows.into_iter().map(to_dispatcher_market_job).collect())
}

pub(super) fn dispatcher_get_job_history(
    conn: &Connection,
    save_context: &DispatcherSaveContext,
) -> Result<DispatcherHistoryResponse, String> {
    prepare_dispatcher_system(conn)?;
    let rows =
        list_dispatcher_jobs_by_status(conn, DISPATCHER_HISTORY_JOB_STATUSES, 120, save_context)?;
    let items = rows
        .iter()
        .cloned()
        .map(to_dispatcher_market_job)
        .collect::<Vec<_>>();
    let completed = rows.iter().filter(|row| row.status == "completed").count() as i64;
    let failed = rows
        .iter()
        .filter(|row| {
            row.status == "problematic" || row.status == "cancelled" || row.status == "failed"
        })
        .count() as i64;
    let rejected = rows
        .iter()
        .filter(|row| row.status == "rejected" || row.status == "expired")
        .count() as i64;
    let revenue = rows
        .iter()
        .filter(|row| row.status == "completed")
        .map(|row| row.total_reward)
        .sum::<i64>();
    let rate_values = rows
        .iter()
        .map(|row| row.calculated_rate_per_km)
        .collect::<Vec<_>>();
    let dist_values = rows.iter().map(|row| row.distance_km).collect::<Vec<_>>();
    let avg_rate_per_km = if rate_values.is_empty() {
        0.0
    } else {
        rate_values.iter().sum::<f64>() / rate_values.len() as f64
    };
    let avg_distance_km = if dist_values.is_empty() {
        0.0
    } else {
        dist_values.iter().sum::<f64>() / dist_values.len() as f64
    };
    let base = (completed + failed + rejected).max(1) as f64;
    let punctuality = (completed as f64 / base).clamp(0.0, 1.0);
    let quality =
        ((completed as f64 - failed as f64 * 0.3 - rejected as f64 * 0.2) / base).clamp(0.0, 1.0);

    Ok(DispatcherHistoryResponse {
        summary: DispatcherHistorySummary {
            total_completed: completed,
            total_failed: failed,
            total_rejected: rejected,
            revenue,
            avg_rate_per_km,
            avg_distance_km,
            punctuality,
            quality,
        },
        items,
    })
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use rusqlite::{Connection, params};

    use super::dispatcher_assign_job_to_active_save;
    use crate::features::career::dispatcher::{DispatcherSaveContext, prepare_dispatcher_system};

    #[test]
    fn assign_job_to_active_save_persists_context_and_status() {
        let conn = Connection::open_in_memory().unwrap();
        crate::features::economy::ensure_tables(&conn).unwrap();
        prepare_dispatcher_system(&conn).unwrap();

        let now = Utc::now().to_rfc3339();
        conn.execute(
            r#"
            INSERT INTO dispatcher_jobs (
                id, source_type, company_id, company_name, job_type, cargo_type,
                origin_city, origin_country, destination_city, destination_country,
                distance_km, cargo_mass_kg, urgency_level, difficulty_level,
                equipment_type_required, trailer_type_required, base_rate_per_km,
                calculated_rate_per_km, total_reward, estimated_duration_minutes,
                payment_tier_snapshot, payment_multiplier_snapshot, country_multiplier_snapshot,
                reputation_multiplier_snapshot, cargo_multiplier_snapshot,
                urgency_multiplier_snapshot, equipment_multiplier_snapshot,
                market_variation_snapshot, customer_multiplier_snapshot, company_reputation,
                fuel_cost_estimate, profit_estimate, risk_note, bonus_note,
                expires_at_utc, status, progress_km, profile_reference, save_reference,
                quicksave_reference, save_session_id, route_reference, ets2_job_link_status,
                accepted_at_utc, completed_at_utc, created_at_utc, updated_at_utc
            )
            VALUES (
                ?1, 'generated', 'north-axis-logistics', 'North Axis Logistics', 'quick_job', 'standard',
                'Hamburg', 'DE', 'Prague', 'CZ',
                642.0, 12000.0, 'normal', 'normal',
                'quick_job', NULL, 1.12,
                1.18, 758, 620,
                'standard', 1.0, 1.02,
                1.01, 1.0,
                1.0, 1.0,
                1.0, 1.0, 320,
                120, 480, NULL, NULL,
                NULL, 'open', 0, NULL, NULL,
                NULL, NULL, NULL, 'pending_route',
                NULL, NULL, ?2, ?2
            )
            "#,
            params!["dispatcher-test-1", now],
        )
        .unwrap();

        let save_context = DispatcherSaveContext {
            profile_reference: Some("profiles/main".to_string()),
            save_reference: Some("profiles/main/save/quicksave".to_string()),
            quicksave_reference: Some("profiles/main/save/quicksave".to_string()),
            save_session_id: Some("savectx-test".to_string()),
        };

        let result =
            dispatcher_assign_job_to_active_save(&conn, "dispatcher-test-1", &save_context)
                .unwrap();

        assert_eq!(result.job.status, "assigned_to_save");
        assert_eq!(
            result.job.profile_reference.as_deref(),
            Some("profiles/main")
        );
        assert_eq!(
            result.job.save_reference.as_deref(),
            Some("profiles/main/save/quicksave")
        );
        assert_eq!(result.job.save_session_id.as_deref(), Some("savectx-test"));
        assert_eq!(result.job.ets2_job_link_status.as_deref(), Some("pending"));
        assert!(result.job.route_reference.is_some());
        assert!(result.job.linked_to_active_save);
    }

    #[test]
    fn assign_job_to_active_save_allows_accepted_status() {
        let conn = Connection::open_in_memory().unwrap();
        crate::features::economy::ensure_tables(&conn).unwrap();
        prepare_dispatcher_system(&conn).unwrap();

        let now = Utc::now().to_rfc3339();
        conn.execute(
            r#"
            INSERT INTO dispatcher_jobs (
                id, source_type, company_id, company_name, job_type, cargo_type,
                origin_city, origin_country, destination_city, destination_country,
                distance_km, cargo_mass_kg, urgency_level, difficulty_level,
                equipment_type_required, trailer_type_required, base_rate_per_km,
                calculated_rate_per_km, total_reward, estimated_duration_minutes,
                payment_tier_snapshot, payment_multiplier_snapshot, country_multiplier_snapshot,
                reputation_multiplier_snapshot, cargo_multiplier_snapshot,
                urgency_multiplier_snapshot, equipment_multiplier_snapshot,
                market_variation_snapshot, customer_multiplier_snapshot, company_reputation,
                fuel_cost_estimate, profit_estimate, risk_note, bonus_note,
                expires_at_utc, status, progress_km, profile_reference, save_reference,
                quicksave_reference, save_session_id, route_reference, ets2_job_link_status,
                accepted_at_utc, completed_at_utc, created_at_utc, updated_at_utc
            )
            VALUES (
                ?1, 'generated', 'north-axis-logistics', 'North Axis Logistics', 'quick_job', 'standard',
                'Hamburg', 'DE', 'Prague', 'CZ',
                642.0, 12000.0, 'normal', 'normal',
                'quick_job', NULL, 1.12,
                1.18, 758, 620,
                'standard', 1.0, 1.02,
                1.01, 1.0,
                1.0, 1.0,
                1.0, 1.0, 320,
                120, 480, NULL, NULL,
                NULL, 'accepted', 0, NULL, NULL,
                NULL, NULL, NULL, 'pending_route',
                ?2, NULL, ?2, ?2
            )
            "#,
            params!["dispatcher-test-accepted-1", now],
        )
        .unwrap();

        let save_context = DispatcherSaveContext {
            profile_reference: Some("profiles/main".to_string()),
            save_reference: Some("profiles/main/save/quicksave".to_string()),
            quicksave_reference: Some("profiles/main/save/quicksave".to_string()),
            save_session_id: Some("savectx-test".to_string()),
        };

        let result = dispatcher_assign_job_to_active_save(
            &conn,
            "dispatcher-test-accepted-1",
            &save_context,
        )
        .unwrap();

        assert_eq!(result.job.status, "assigned_to_save");
        assert_eq!(
            result.job.profile_reference.as_deref(),
            Some("profiles/main")
        );
    }
}
