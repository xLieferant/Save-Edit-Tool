use rusqlite::{Connection, params};
use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BankState {
    pub cash_balance: i64,
    pub debt_balance: i64,
    pub interest_rate: f64,
    pub installment: i64,
}

pub fn ensure_tables(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS bank_state (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            cash_balance INTEGER NOT NULL,
            debt_balance INTEGER NOT NULL,
            interest_rate REAL NOT NULL,
            installment INTEGER NOT NULL
        );
        "#,
    )
    .map_err(|e| e.to_string())?;

    conn.execute(
        r#"
        INSERT OR IGNORE INTO bank_state (id, cash_balance, debt_balance, interest_rate, installment)
        VALUES (1, 145000, 38000, 0.0475, 1800)
        "#,
        [],
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}

pub fn load_state(conn: &Connection) -> Result<BankState, String> {
    conn.query_row(
        r#"
        SELECT cash_balance, debt_balance, interest_rate, installment
        FROM bank_state
        WHERE id = 1
        "#,
        [],
        |row| {
            Ok(BankState {
                cash_balance: row.get(0)?,
                debt_balance: row.get(1)?,
                interest_rate: row.get(2)?,
                installment: row.get(3)?,
            })
        },
    )
    .map_err(|e| e.to_string())
}

pub fn apply_trip_result(conn: &Connection, net_income: i64) -> Result<BankState, String> {
    let current = load_state(conn)?;
    let interest_charge = if current.debt_balance > 0 {
        ((current.debt_balance as f64) * current.interest_rate / 240.0).round() as i64
    } else {
        0
    };
    let principal_payment = if current.debt_balance > 0 {
        current.installment.min(current.debt_balance)
    } else {
        0
    };

    let next_cash = current.cash_balance + net_income - interest_charge - principal_payment;
    let next_debt = (current.debt_balance - principal_payment).max(0);

    conn.execute(
        r#"
        UPDATE bank_state
        SET cash_balance = ?1, debt_balance = ?2
        WHERE id = 1
        "#,
        params![next_cash, next_debt],
    )
    .map_err(|e| e.to_string())?;

    load_state(conn)
}
