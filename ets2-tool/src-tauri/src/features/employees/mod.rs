use rusqlite::{Connection, params};
use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EmployeeSummary {
    pub employee_id: String,
    pub name: String,
    pub role: String,
    pub status: String,
    pub salary: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EmployeeOverview {
    pub total: i64,
    pub on_duty: i64,
    pub resting: i64,
    pub dispatchers: i64,
}

pub fn ensure_tables(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS employees (
            employee_id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            role TEXT NOT NULL,
            status TEXT NOT NULL,
            salary INTEGER NOT NULL,
            active INTEGER NOT NULL DEFAULT 1
        );
        "#,
    )
    .map_err(|e| e.to_string())?;

    let employees = [
        ("emp-chief", "Alex Mercer", "Chief", "on_duty", 5200_i64),
        ("emp-dispatch", "Mara Stein", "Dispatcher", "on_duty", 3400_i64),
        ("emp-driver", "Lena Kovac", "Driver", "resting", 2900_i64),
        ("emp-apprentice", "Noah Weiss", "Azubi", "training", 1600_i64),
    ];

    for (employee_id, name, role, status, salary) in employees {
        conn.execute(
            r#"
            INSERT OR IGNORE INTO employees (
                employee_id,
                name,
                role,
                status,
                salary
            )
            VALUES (?1, ?2, ?3, ?4, ?5)
            "#,
            params![employee_id, name, role, status, salary],
        )
        .map_err(|e| e.to_string())?;
    }

    Ok(())
}

pub fn load_staff(conn: &Connection, limit: usize) -> Result<Vec<EmployeeSummary>, String> {
    let mut stmt = conn
        .prepare(
            r#"
            SELECT employee_id, name, role, status, salary
            FROM employees
            WHERE active = 1
            ORDER BY
                CASE role
                    WHEN 'Chief' THEN 0
                    WHEN 'Dispatcher' THEN 1
                    WHEN 'Driver' THEN 2
                    ELSE 3
                END,
                name ASC
            LIMIT ?1
            "#,
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map([limit as i64], |row| {
            Ok(EmployeeSummary {
                employee_id: row.get(0)?,
                name: row.get(1)?,
                role: row.get(2)?,
                status: row.get(3)?,
                salary: row.get(4)?,
            })
        })
        .map_err(|e| e.to_string())?;

    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

pub fn load_overview(conn: &Connection) -> Result<EmployeeOverview, String> {
    conn.query_row(
        r#"
        SELECT
            COUNT(*) AS total,
            SUM(CASE WHEN status = 'on_duty' THEN 1 ELSE 0 END) AS on_duty,
            SUM(CASE WHEN status = 'resting' THEN 1 ELSE 0 END) AS resting,
            SUM(CASE WHEN role = 'Dispatcher' THEN 1 ELSE 0 END) AS dispatchers
        FROM employees
        WHERE active = 1
        "#,
        [],
        |row| {
            Ok(EmployeeOverview {
                total: row.get(0)?,
                on_duty: row.get::<_, Option<i64>>(1)?.unwrap_or(0),
                resting: row.get::<_, Option<i64>>(2)?.unwrap_or(0),
                dispatchers: row.get::<_, Option<i64>>(3)?.unwrap_or(0),
            })
        },
    )
    .map_err(|e| e.to_string())
}

pub fn mark_driver_status(conn: &Connection, status: &str) -> Result<(), String> {
    conn.execute(
        r#"
        UPDATE employees
        SET status = ?1
        WHERE employee_id = 'emp-driver'
        "#,
        [status],
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}
