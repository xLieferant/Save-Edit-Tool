use std::collections::HashSet;

use rusqlite::Connection;

pub fn existing_columns(conn: &Connection, table: &str) -> Result<HashSet<String>, String> {
    let mut stmt = conn
        .prepare(&format!("PRAGMA table_info({table})"))
        .map_err(|error| error.to_string())?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|error| error.to_string())?;

    let mut columns = HashSet::new();
    for row in rows {
        columns.insert(row.map_err(|error| error.to_string())?);
    }

    Ok(columns)
}

pub fn ensure_columns(
    conn: &Connection,
    table: &str,
    required: &[(&str, &str)],
) -> Result<HashSet<String>, String> {
    let mut columns = existing_columns(conn, table)?;

    for (column, definition) in required {
        if columns.contains(*column) {
            continue;
        }

        conn.execute(
            &format!("ALTER TABLE {table} ADD COLUMN {column} {definition}"),
            [],
        )
        .map_err(|error| error.to_string())?;
        columns.insert((*column).to_string());
    }

    Ok(columns)
}

pub fn create_indexes(conn: &Connection, statements: &[&str]) -> Result<(), String> {
    for statement in statements {
        conn.execute(statement, []).map_err(|error| error.to_string())?;
    }

    Ok(())
}
