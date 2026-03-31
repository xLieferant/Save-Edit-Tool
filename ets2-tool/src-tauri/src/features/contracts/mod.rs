use chrono::Utc;
use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ContractSummary {
    pub contract_id: String,
    pub company_name: String,
    pub origin: String,
    pub destination: String,
    pub cargo: String,
    pub bonus_payout: i64,
    pub active: bool,
    pub completion_count: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DispatchAssignment {
    pub job_id: String,
    pub contract_id: Option<String>,
    pub company_name: String,
    pub origin: String,
    pub destination: String,
    pub cargo: String,
    pub bonus_payout: i64,
}

pub fn ensure_tables(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS contracts (
            contract_id TEXT PRIMARY KEY,
            company_name TEXT NOT NULL,
            origin TEXT NOT NULL,
            destination TEXT NOT NULL,
            cargo TEXT NOT NULL,
            bonus_payout INTEGER NOT NULL,
            active INTEGER NOT NULL DEFAULT 1,
            cancelled INTEGER NOT NULL DEFAULT 0,
            completion_count INTEGER NOT NULL DEFAULT 0
        );
        "#,
    )
    .map_err(|e| e.to_string())?;

    let contracts = [
        (
            "contract-north-axis",
            "North Axis Pharma",
            "Berlin",
            "Prague",
            "Medical supplies",
            2800_i64,
            1_i64,
        ),
        (
            "contract-alpine-steel",
            "Alpine Steelworks",
            "Hamburg",
            "Lyon",
            "Industrial steel",
            4200_i64,
            1_i64,
        ),
        (
            "contract-fresh-link",
            "FreshLink Foods",
            "Warsaw",
            "Vienna",
            "Fresh produce",
            1600_i64,
            0_i64,
        ),
    ];

    for (contract_id, company_name, origin, destination, cargo, bonus_payout, active) in contracts {
        conn.execute(
            r#"
            INSERT OR IGNORE INTO contracts (
                contract_id,
                company_name,
                origin,
                destination,
                cargo,
                bonus_payout,
                active
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
            params![
                contract_id,
                company_name,
                origin,
                destination,
                cargo,
                bonus_payout,
                active
            ],
        )
        .map_err(|e| e.to_string())?;
    }

    Ok(())
}

pub fn load_active_contracts(
    conn: &Connection,
    limit: usize,
) -> Result<Vec<ContractSummary>, String> {
    let mut stmt = conn
        .prepare(
            r#"
            SELECT contract_id, company_name, origin, destination, cargo, bonus_payout, active, completion_count
            FROM contracts
            WHERE cancelled = 0
            ORDER BY active DESC, bonus_payout DESC
            LIMIT ?1
            "#,
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map([limit as i64], |row| {
            Ok(ContractSummary {
                contract_id: row.get(0)?,
                company_name: row.get(1)?,
                origin: row.get(2)?,
                destination: row.get(3)?,
                cargo: row.get(4)?,
                bonus_payout: row.get(5)?,
                active: row.get::<_, i64>(6)? != 0,
                completion_count: row.get(7)?,
            })
        })
        .map_err(|e| e.to_string())?;

    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

pub fn select_dispatch_assignment(conn: &Connection) -> Result<DispatchAssignment, String> {
    let contract = conn
        .query_row(
            r#"
            SELECT contract_id, company_name, origin, destination, cargo, bonus_payout
            FROM contracts
            WHERE active = 1 AND cancelled = 0
            ORDER BY bonus_payout DESC, completion_count ASC
            LIMIT 1
            "#,
            [],
            |row| {
                Ok(DispatchAssignment {
                    job_id: format!("job-{}", Utc::now().timestamp()),
                    contract_id: Some(row.get(0)?),
                    company_name: row.get(1)?,
                    origin: row.get(2)?,
                    destination: row.get(3)?,
                    cargo: row.get(4)?,
                    bonus_payout: row.get(5)?,
                })
            },
        )
        .optional()
        .map_err(|e| e.to_string())?;

    Ok(contract.unwrap_or_else(|| DispatchAssignment {
        job_id: format!("job-{}", Utc::now().timestamp()),
        contract_id: None,
        company_name: "Open Market".to_string(),
        origin: "Telemetry Start".to_string(),
        destination: "Telemetry Destination".to_string(),
        cargo: "General cargo".to_string(),
        bonus_payout: 0,
    }))
}

pub fn record_completion(conn: &Connection, contract_id: Option<&str>) -> Result<(), String> {
    let Some(contract_id) = contract_id else {
        return Ok(());
    };

    conn.execute(
        r#"
        UPDATE contracts
        SET completion_count = completion_count + 1
        WHERE contract_id = ?1
        "#,
        [contract_id],
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}
