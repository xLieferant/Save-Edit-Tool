use rusqlite::{params, Connection, OptionalExtension};

use crate::features::companies::models::{Company, CompanyListItem, NewCompany};

pub fn insert_company(conn: &Connection, company: &NewCompany) -> Result<i64, String> {
    conn.execute(
        r#"
        INSERT INTO companies (
            owner_user_id,
            name,
            logo_path,
            logo_blob,
            logo_mime,
            header_path,
            header_blob,
            header_mime,
            description,
            salary_base,
            location,
            language,
            game,
            job_type,
            created_at,
            updated_at,
            is_active
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)
        "#,
        params![
            company.owner_user_id,
            company.name,
            company.logo_path,
            company.logo_blob,
            company.logo_mime,
            company.header_path,
            company.header_blob,
            company.header_mime,
            company.description,
            company.salary_base,
            company.location,
            company.language,
            company.game,
            company.job_type,
            company.created_at,
            company.updated_at,
            if company.is_active { 1 } else { 0 }
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(conn.last_insert_rowid())
}

pub fn load_company_by_id(conn: &Connection, company_id: i64) -> Result<Option<Company>, String> {
    conn.query_row(
        r#"
        SELECT
            id,
            owner_user_id,
            name,
            logo_path,
            header_path,
            language,
            game,
            description,
            salary_base,
            location,
            job_type,
            created_at,
            updated_at,
            is_active
        FROM companies
        WHERE id = ?1
        "#,
        params![company_id],
        |row| {
            Ok(Company {
                id: row.get(0)?,
                owner_user_id: row.get(1)?,
                name: row.get(2)?,
                logo_path: row.get(3)?,
                header_path: row.get(4)?,
                language: row.get(5)?,
                game: row.get(6)?,
                description: row.get(7)?,
                salary_base: row.get(8)?,
                location: row.get(9)?,
                job_type: row.get(10)?,
                created_at: row.get(11)?,
                updated_at: row.get(12)?,
                is_active: row.get::<_, i64>(13)? != 0,
            })
        },
    )
    .optional()
    .map_err(|e| e.to_string())
}

pub fn find_company_id_by_name(conn: &Connection, name: &str) -> Result<Option<i64>, String> {
    let id: Option<i64> = conn
        .query_row(
            r#"
            SELECT id
            FROM companies
            WHERE LOWER(name) = LOWER(?1) AND is_active = 1
            LIMIT 1
            "#,
            params![name],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    Ok(id)
}

pub fn list_companies(conn: &Connection, limit: i64) -> Result<Vec<CompanyListItem>, String> {
    let mut stmt = conn
        .prepare(
            r#"
            SELECT
                c.id,
                c.name,
                c.logo_path,
                c.description,
                c.location,
                c.job_type,
                c.language,
                c.game,
                (
                    SELECT COUNT(1)
                    FROM company_members m
                    WHERE m.company_id = c.id AND m.is_active = 1
                ) AS members_count
            FROM companies c
            WHERE c.is_active = 1
            ORDER BY c.id DESC
            LIMIT ?1
            "#,
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map(params![limit], |row| {
            Ok(CompanyListItem {
                id: row.get(0)?,
                name: row.get(1)?,
                logo_path: row.get(2)?,
                description: row.get(3)?,
                location: row.get(4)?,
                job_type: row.get(5)?,
                language: row.get(6)?,
                game: row.get(7)?,
                members_count: row.get(8)?,
            })
        })
        .map_err(|e| e.to_string())?;

    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

pub fn insert_member(
    conn: &Connection,
    company_id: i64,
    user_id: i64,
    member_role: &str,
    joined_at: &str,
) -> Result<i64, String> {
    conn.execute(
        r#"
        INSERT INTO company_members (
            company_id,
            user_id,
            member_role,
            joined_at,
            salary_override,
            is_active
        ) VALUES (?1, ?2, ?3, ?4, NULL, 1)
        "#,
        params![company_id, user_id, member_role, joined_at],
    )
    .map_err(|e| e.to_string())?;
    Ok(conn.last_insert_rowid())
}

pub fn is_user_member_of_company(
    conn: &Connection,
    company_id: i64,
    user_id: i64,
) -> Result<bool, String> {
    let exists: Option<i64> = conn
        .query_row(
            r#"
            SELECT 1
            FROM company_members
            WHERE company_id = ?1 AND user_id = ?2 AND is_active = 1
            LIMIT 1
            "#,
            params![company_id, user_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    Ok(exists.is_some())
}
