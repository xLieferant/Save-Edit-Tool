use std::collections::HashSet;

use rusqlite::Connection;

pub fn ensure_tables(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS companies (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            owner_user_id INTEGER NOT NULL,
            name TEXT NOT NULL,
            logo_path TEXT,
            logo_blob BLOB,
            logo_mime TEXT,
            header_path TEXT,
            header_blob BLOB,
            header_mime TEXT,
            description TEXT,
            salary_base INTEGER NOT NULL DEFAULT 0,
            location TEXT NOT NULL,
            language TEXT,
            game TEXT,
            job_type TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            is_active INTEGER NOT NULL DEFAULT 1
        );

        CREATE INDEX IF NOT EXISTS idx_companies_owner_user_id ON companies(owner_user_id);
        CREATE INDEX IF NOT EXISTS idx_companies_active ON companies(is_active);

        CREATE TABLE IF NOT EXISTS company_members (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            company_id INTEGER NOT NULL,
            user_id INTEGER NOT NULL,
            member_role TEXT NOT NULL,
            joined_at TEXT NOT NULL,
            salary_override INTEGER,
            is_active INTEGER NOT NULL DEFAULT 1
        );

        CREATE UNIQUE INDEX IF NOT EXISTS idx_company_members_unique ON company_members(company_id, user_id);
        CREATE INDEX IF NOT EXISTS idx_company_members_company_id ON company_members(company_id);
        CREATE INDEX IF NOT EXISTS idx_company_members_user_id ON company_members(user_id);
        "#,
    )
    .map_err(|e| e.to_string())?;

    ensure_company_columns(conn)?;

    // Optional: enforce uniqueness for company names (may fail if legacy data contains duplicates).
    if let Err(error) = conn.execute(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_companies_name_unique ON companies(name)",
        [],
    ) {
        // Keep the database usable even if the unique index cannot be created.
        crate::dev_log!("[companies] unique name index skipped: {}", error);
    }
    Ok(())
}

fn ensure_company_columns(conn: &Connection) -> Result<(), String> {
    let existing = existing_columns(conn, "companies")?;
    let required = [
        ("logo_blob", "BLOB"),
        ("logo_mime", "TEXT"),
        ("header_path", "TEXT"),
        ("header_blob", "BLOB"),
        ("header_mime", "TEXT"),
        ("language", "TEXT"),
        ("game", "TEXT"),
    ];

    for (column, definition) in required {
        if !existing.contains(column) {
            conn.execute(&format!("ALTER TABLE companies ADD COLUMN {column} {definition}"), [])
                .map_err(|e| e.to_string())?;
        }
    }

    Ok(())
}

fn existing_columns(conn: &Connection, table: &str) -> Result<HashSet<String>, String> {
    let mut stmt = conn
        .prepare(&format!("PRAGMA table_info({table})"))
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|e| e.to_string())?;

    let mut columns = HashSet::new();
    for row in rows {
        columns.insert(row.map_err(|e| e.to_string())?);
    }

    Ok(columns)
}
