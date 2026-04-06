use rusqlite::{Connection, OptionalExtension, params};

use crate::features::auth::repo as auth_repo;
use crate::features::companies::repo as companies_repo;
use crate::features::vtc::models::{
    CareerSettings, CompanyMember, CompanyOverview, CompanyRoleOption, CompanySettings,
    CreateCompanyInput, UpdateCareerSettingsInput, UpdateCompanyProfileInput,
    UpdateCompanySettingsInput, UpdateUserSettingsInput, UserProfile, UserSettings,
};

pub fn is_valid_role(conn: &Connection, role_key: &str) -> Result<bool, String> {
    let exists: Option<i64> = conn
        .query_row(
            "SELECT 1 FROM company_roles WHERE role_key = ?1 LIMIT 1",
            params![role_key],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    Ok(exists.is_some())
}

pub fn list_roles(conn: &Connection) -> Result<Vec<CompanyRoleOption>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT role_key, role_label, sort_order FROM company_roles ORDER BY sort_order ASC, role_key ASC",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map([], |row| {
            Ok(CompanyRoleOption {
                role_key: row.get(0)?,
                role_label: row.get(1)?,
                sort_order: row.get(2)?,
            })
        })
        .map_err(|e| e.to_string())?;

    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

pub fn ensure_user_settings_row(conn: &Connection, user_id: i64, now: &str) -> Result<(), String> {
    conn.execute(
        r#"
        INSERT OR IGNORE INTO user_settings (
            user_id,
            language,
            preferred_game,
            profile_visibility,
            username_last_changed_at,
            theme_preference,
            notifications_enabled,
            avatar_path,
            bio,
            created_at,
            updated_at
        ) VALUES (?1, 'en', 'ETS2', 'private', NULL, NULL, 1, NULL, NULL, ?2, ?3)
        "#,
        params![user_id, now, now],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn load_user_settings(conn: &Connection, user_id: i64) -> Result<UserSettings, String> {
    conn.query_row(
        r#"
        SELECT
            user_id,
            language,
            preferred_game,
            profile_visibility,
            username_last_changed_at,
            theme_preference,
            notifications_enabled,
            avatar_path,
            bio,
            created_at,
            updated_at
        FROM user_settings
        WHERE user_id = ?1
        "#,
        params![user_id],
        |row| {
            Ok(UserSettings {
                user_id: row.get(0)?,
                language: row.get(1)?,
                preferred_game: row.get(2)?,
                profile_visibility: row.get(3)?,
                username_last_changed_at: row.get(4)?,
                theme_preference: row.get(5)?,
                notifications_enabled: row.get::<_, i64>(6)? != 0,
                avatar_path: row.get(7)?,
                bio: row.get(8)?,
                created_at: row.get(9)?,
                updated_at: row.get(10)?,
            })
        },
    )
    .map_err(|e| e.to_string())
}

pub fn update_user_settings(
    conn: &Connection,
    user_id: i64,
    input: &UpdateUserSettingsInput,
    now: &str,
) -> Result<(), String> {
    let current = load_user_settings(conn, user_id)?;

    let language = input
        .language
        .clone()
        .unwrap_or(current.language)
        .trim()
        .to_string();
    let preferred_game = input
        .preferred_game
        .clone()
        .unwrap_or(current.preferred_game)
        .trim()
        .to_uppercase();
    let profile_visibility = input
        .profile_visibility
        .clone()
        .unwrap_or(current.profile_visibility)
        .trim()
        .to_string();
    let theme_preference = input.theme_preference.clone().or(current.theme_preference);
    let notifications_enabled = input
        .notifications_enabled
        .unwrap_or(current.notifications_enabled);

    conn.execute(
        r#"
        UPDATE user_settings
        SET
            language = ?1,
            preferred_game = ?2,
            profile_visibility = ?3,
            theme_preference = ?4,
            notifications_enabled = ?5,
            updated_at = ?6
        WHERE user_id = ?7
        "#,
        params![
            language,
            preferred_game,
            profile_visibility,
            theme_preference,
            if notifications_enabled { 1 } else { 0 },
            now,
            user_id,
        ],
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}

pub fn update_user_profile_meta(
    conn: &Connection,
    user_id: i64,
    avatar_path: Option<String>,
    bio: Option<String>,
    profile_visibility: Option<String>,
    now: &str,
) -> Result<(), String> {
    let current = load_user_settings(conn, user_id)?;

    let visibility = profile_visibility
        .unwrap_or(current.profile_visibility)
        .trim()
        .to_string();

    conn.execute(
        r#"
        UPDATE user_settings
        SET
            avatar_path = ?1,
            bio = ?2,
            profile_visibility = ?3,
            updated_at = ?4
        WHERE user_id = ?5
        "#,
        params![avatar_path, bio, visibility, now, user_id],
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}

pub fn set_username_last_changed_at(
    conn: &Connection,
    user_id: i64,
    now: &str,
) -> Result<(), String> {
    conn.execute(
        "UPDATE user_settings SET username_last_changed_at = ?1, updated_at = ?2 WHERE user_id = ?3",
        params![now, now, user_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn find_user_by_username_case_insensitive(
    conn: &Connection,
    username: &str,
) -> Result<Option<(i64, String)>, String> {
    conn.query_row(
        r#"
        SELECT id, username
        FROM users
        WHERE LOWER(username) = LOWER(?1)
        LIMIT 1
        "#,
        params![username],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )
    .optional()
    .map_err(|e| e.to_string())
}

pub fn update_username(
    conn: &Connection,
    user_id: i64,
    username: &str,
    now: &str,
) -> Result<(), String> {
    conn.execute(
        "UPDATE users SET username = ?1, updated_at = ?2 WHERE id = ?3",
        params![username, now, user_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn load_company_role_for_user(
    conn: &Connection,
    user_id: i64,
) -> Result<Option<(i64, String)>, String> {
    conn.query_row(
        r#"
        SELECT company_id, member_role
        FROM company_members
        WHERE user_id = ?1 AND is_active = 1
        ORDER BY id DESC
        LIMIT 1
        "#,
        params![user_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )
    .optional()
    .map_err(|e| e.to_string())
}

pub fn load_user_profile(conn: &Connection, user_id: i64) -> Result<Option<UserProfile>, String> {
    let user = auth_repo::load_user_by_id(conn, user_id)?;
    let Some(user) = user else {
        return Ok(None);
    };

    let settings = load_user_settings(conn, user_id)?;
    let role_info = load_company_role_for_user(conn, user_id)?;
    let (role_company_id, company_role) = match role_info {
        Some((company_id, role)) => (Some(company_id), Some(role)),
        None => (None, None),
    };

    Ok(Some(UserProfile {
        user_id: user.id,
        username: user.username,
        language: settings.language,
        avatar_path: settings.avatar_path,
        bio: settings.bio,
        in_company: user.company_id.is_some(),
        company_id: user.company_id.or(role_company_id),
        company_role,
        username_last_changed_at: settings.username_last_changed_at,
        username_next_change_at: None,
        created_at: user.created_at,
        updated_at: user.updated_at,
    }))
}

pub fn find_company_id_by_name_case_insensitive(
    conn: &Connection,
    name: &str,
) -> Result<Option<i64>, String> {
    conn.query_row(
        r#"
        SELECT id
        FROM companies
        WHERE LOWER(name) = LOWER(?1)
          AND is_active = 1
        LIMIT 1
        "#,
        params![name],
        |row| row.get(0),
    )
    .optional()
    .map_err(|e| e.to_string())
}

pub fn create_company(
    conn: &Connection,
    owner_user_id: i64,
    input: &CreateCompanyInput,
    now: &str,
) -> Result<i64, String> {
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
            is_active,
            slogan,
            accent_color,
            public_visibility
        ) VALUES (?1, ?2, ?3, NULL, NULL, ?4, NULL, NULL, ?5, 0, ?6, ?7, ?8, 'vtc', ?9, ?10, 1, ?11, ?12, ?13)
        "#,
        params![
            owner_user_id,
            input.name.trim(),
            input.logo_path,
            input.header_path,
            input.description,
            input.location.trim(),
            input.language.trim(),
            input.game.trim().to_uppercase(),
            now,
            now,
            input.slogan,
            input.accent_color,
            if input.public_visibility.unwrap_or(true) { 1 } else { 0 },
        ],
    )
    .map_err(|e| e.to_string())?;

    Ok(conn.last_insert_rowid())
}

pub fn ensure_company_settings_row(
    conn: &Connection,
    company_id: i64,
    language: &str,
    game: &str,
    now: &str,
) -> Result<(), String> {
    conn.execute(
        r#"
        INSERT OR IGNORE INTO company_settings (
            company_id,
            company_language,
            company_game,
            allow_public_join_requests,
            show_company_publicly,
            default_member_role,
            dispatcher_can_manage_jobs,
            trainee_visible_in_roster,
            allow_member_custom_profiles,
            created_at,
            updated_at
        ) VALUES (?1, ?2, ?3, 0, 1, 'driver', 1, 1, 1, ?4, ?5)
        "#,
        params![company_id, language, game, now, now],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn load_company_overview(
    conn: &Connection,
    company_id: i64,
) -> Result<Option<CompanyOverview>, String> {
    conn.query_row(
        r#"
        SELECT
            c.id,
            c.name,
            c.location,
            c.language,
            c.game,
            c.description,
            c.logo_path,
            c.header_path,
            c.slogan,
            c.accent_color,
            COALESCE(c.public_visibility, 1),
            c.owner_user_id,
            c.created_at,
            c.updated_at,
            (
                SELECT COUNT(1)
                FROM company_members m
                WHERE m.company_id = c.id AND m.is_active = 1
            ) AS members_count
        FROM companies c
        WHERE c.id = ?1 AND c.is_active = 1
        "#,
        params![company_id],
        |row| {
            Ok(CompanyOverview {
                id: row.get(0)?,
                name: row.get(1)?,
                location: row.get(2)?,
                language: row.get(3)?,
                game: row.get(4)?,
                description: row.get(5)?,
                logo_path: row.get(6)?,
                header_path: row.get(7)?,
                slogan: row.get(8)?,
                accent_color: row.get(9)?,
                public_visibility: row.get::<_, i64>(10)? != 0,
                owner_user_id: row.get(11)?,
                created_at: row.get(12)?,
                updated_at: row.get(13)?,
                members_count: row.get(14)?,
            })
        },
    )
    .optional()
    .map_err(|e| e.to_string())
}

pub fn update_company_profile(
    conn: &Connection,
    company_id: i64,
    input: &UpdateCompanyProfileInput,
    now: &str,
) -> Result<(), String> {
    let current =
        load_company_overview(conn, company_id)?.ok_or_else(|| "company_not_found".to_string())?;

    let name = input
        .name
        .clone()
        .unwrap_or(current.name)
        .trim()
        .to_string();
    let location = input
        .location
        .clone()
        .unwrap_or(current.location)
        .trim()
        .to_string();
    let language = input.language.clone().or(current.language);
    let game = input
        .game
        .clone()
        .or(current.game)
        .map(|value| value.to_uppercase());
    let description = input.description.clone().or(current.description);
    let logo_path = input.logo_path.clone().or(current.logo_path);
    let header_path = input.header_path.clone().or(current.header_path);
    let slogan = input.slogan.clone().or(current.slogan);
    let accent_color = input.accent_color.clone().or(current.accent_color);
    let public_visibility = input.public_visibility.unwrap_or(current.public_visibility);

    conn.execute(
        r#"
        UPDATE companies
        SET
            name = ?1,
            location = ?2,
            language = ?3,
            game = ?4,
            description = ?5,
            logo_path = ?6,
            header_path = ?7,
            slogan = ?8,
            accent_color = ?9,
            public_visibility = ?10,
            updated_at = ?11
        WHERE id = ?12
        "#,
        params![
            name,
            location,
            language,
            game,
            description,
            logo_path,
            header_path,
            slogan,
            accent_color,
            if public_visibility { 1 } else { 0 },
            now,
            company_id,
        ],
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}

pub fn load_company_members(
    conn: &Connection,
    company_id: i64,
) -> Result<Vec<CompanyMember>, String> {
    let mut stmt = conn
        .prepare(
            r#"
            SELECT
                m.id,
                m.company_id,
                m.user_id,
                u.username,
                m.member_role,
                m.joined_at,
                m.promoted_at,
                m.invited_by,
                m.notes
            FROM company_members m
            INNER JOIN users u ON u.id = m.user_id
            WHERE m.company_id = ?1 AND m.is_active = 1
            ORDER BY
                CASE m.member_role
                    WHEN 'owner' THEN 0
                    WHEN 'ceo' THEN 1
                    WHEN 'manager' THEN 2
                    WHEN 'dispatcher' THEN 3
                    WHEN 'driver' THEN 4
                    WHEN 'trainee' THEN 5
                    ELSE 6
                END,
                m.id ASC
            "#,
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map(params![company_id], |row| {
            Ok(CompanyMember {
                id: row.get(0)?,
                company_id: row.get(1)?,
                user_id: row.get(2)?,
                username: row.get(3)?,
                role_key: row.get(4)?,
                joined_at: row.get(5)?,
                promoted_at: row.get(6)?,
                invited_by: row.get(7)?,
                notes: row.get(8)?,
            })
        })
        .map_err(|e| e.to_string())?;

    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

pub fn load_company_member_by_id(
    conn: &Connection,
    company_id: i64,
    member_id: i64,
) -> Result<Option<CompanyMember>, String> {
    conn.query_row(
        r#"
        SELECT
            m.id,
            m.company_id,
            m.user_id,
            u.username,
            m.member_role,
            m.joined_at,
            m.promoted_at,
            m.invited_by,
            m.notes
        FROM company_members m
        INNER JOIN users u ON u.id = m.user_id
        WHERE m.company_id = ?1 AND m.id = ?2 AND m.is_active = 1
        LIMIT 1
        "#,
        params![company_id, member_id],
        |row| {
            Ok(CompanyMember {
                id: row.get(0)?,
                company_id: row.get(1)?,
                user_id: row.get(2)?,
                username: row.get(3)?,
                role_key: row.get(4)?,
                joined_at: row.get(5)?,
                promoted_at: row.get(6)?,
                invited_by: row.get(7)?,
                notes: row.get(8)?,
            })
        },
    )
    .optional()
    .map_err(|e| e.to_string())
}

pub fn load_company_member_by_user(
    conn: &Connection,
    company_id: i64,
    user_id: i64,
) -> Result<Option<CompanyMember>, String> {
    conn.query_row(
        r#"
        SELECT
            m.id,
            m.company_id,
            m.user_id,
            u.username,
            m.member_role,
            m.joined_at,
            m.promoted_at,
            m.invited_by,
            m.notes
        FROM company_members m
        INNER JOIN users u ON u.id = m.user_id
        WHERE m.company_id = ?1 AND m.user_id = ?2 AND m.is_active = 1
        LIMIT 1
        "#,
        params![company_id, user_id],
        |row| {
            Ok(CompanyMember {
                id: row.get(0)?,
                company_id: row.get(1)?,
                user_id: row.get(2)?,
                username: row.get(3)?,
                role_key: row.get(4)?,
                joined_at: row.get(5)?,
                promoted_at: row.get(6)?,
                invited_by: row.get(7)?,
                notes: row.get(8)?,
            })
        },
    )
    .optional()
    .map_err(|e| e.to_string())
}

pub fn assign_member_role(
    conn: &Connection,
    company_id: i64,
    user_id: i64,
    role_key: &str,
    invited_by: Option<i64>,
    now: &str,
) -> Result<(), String> {
    let existing = load_company_member_by_user(conn, company_id, user_id)?;
    if let Some(member) = existing {
        conn.execute(
            r#"
            UPDATE company_members
            SET member_role = ?1, promoted_at = ?2, updated_at = ?3
            WHERE id = ?4
            "#,
            params![role_key, now, now, member.id],
        )
        .map_err(|e| e.to_string())?;
        return Ok(());
    }

    companies_repo::insert_member(conn, company_id, user_id, role_key, now)?;
    conn.execute(
        "UPDATE company_members SET invited_by = ?1, updated_at = ?2 WHERE company_id = ?3 AND user_id = ?4",
        params![invited_by, now, company_id, user_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn change_member_role(
    conn: &Connection,
    company_id: i64,
    member_id: i64,
    role_key: &str,
    now: &str,
) -> Result<(), String> {
    let changed = conn
        .execute(
            r#"
            UPDATE company_members
            SET member_role = ?1, promoted_at = ?2, updated_at = ?3
            WHERE id = ?4 AND company_id = ?5 AND is_active = 1
            "#,
            params![role_key, now, now, member_id, company_id],
        )
        .map_err(|e| e.to_string())?;

    if changed == 0 {
        return Err("member_not_found".to_string());
    }

    Ok(())
}

pub fn load_company_settings(
    conn: &Connection,
    company_id: i64,
) -> Result<CompanySettings, String> {
    conn.query_row(
        r#"
        SELECT
            company_id,
            company_language,
            company_game,
            allow_public_join_requests,
            show_company_publicly,
            default_member_role,
            dispatcher_can_manage_jobs,
            trainee_visible_in_roster,
            allow_member_custom_profiles,
            created_at,
            updated_at
        FROM company_settings
        WHERE company_id = ?1
        "#,
        params![company_id],
        |row| {
            Ok(CompanySettings {
                company_id: row.get(0)?,
                company_language: row.get(1)?,
                company_game: row.get(2)?,
                allow_public_join_requests: row.get::<_, i64>(3)? != 0,
                show_company_publicly: row.get::<_, i64>(4)? != 0,
                default_member_role: row.get(5)?,
                dispatcher_can_manage_jobs: row.get::<_, i64>(6)? != 0,
                trainee_visible_in_roster: row.get::<_, i64>(7)? != 0,
                allow_member_custom_profiles: row.get::<_, i64>(8)? != 0,
                created_at: row.get(9)?,
                updated_at: row.get(10)?,
            })
        },
    )
    .map_err(|e| e.to_string())
}

pub fn update_company_settings(
    conn: &Connection,
    company_id: i64,
    input: &UpdateCompanySettingsInput,
    now: &str,
) -> Result<(), String> {
    let current = load_company_settings(conn, company_id)?;

    let company_language = input
        .company_language
        .clone()
        .unwrap_or(current.company_language)
        .trim()
        .to_string();
    let company_game = input
        .company_game
        .clone()
        .unwrap_or(current.company_game)
        .trim()
        .to_uppercase();
    let allow_public_join_requests = input
        .allow_public_join_requests
        .unwrap_or(current.allow_public_join_requests);
    let show_company_publicly = input
        .show_company_publicly
        .unwrap_or(current.show_company_publicly);
    let default_member_role = input
        .default_member_role
        .clone()
        .unwrap_or(current.default_member_role)
        .trim()
        .to_string();
    let dispatcher_can_manage_jobs = input
        .dispatcher_can_manage_jobs
        .unwrap_or(current.dispatcher_can_manage_jobs);
    let trainee_visible_in_roster = input
        .trainee_visible_in_roster
        .unwrap_or(current.trainee_visible_in_roster);
    let allow_member_custom_profiles = input
        .allow_member_custom_profiles
        .unwrap_or(current.allow_member_custom_profiles);

    conn.execute(
        r#"
        UPDATE company_settings
        SET
            company_language = ?1,
            company_game = ?2,
            allow_public_join_requests = ?3,
            show_company_publicly = ?4,
            default_member_role = ?5,
            dispatcher_can_manage_jobs = ?6,
            trainee_visible_in_roster = ?7,
            allow_member_custom_profiles = ?8,
            updated_at = ?9
        WHERE company_id = ?10
        "#,
        params![
            company_language,
            company_game,
            if allow_public_join_requests { 1 } else { 0 },
            if show_company_publicly { 1 } else { 0 },
            default_member_role,
            if dispatcher_can_manage_jobs { 1 } else { 0 },
            if trainee_visible_in_roster { 1 } else { 0 },
            if allow_member_custom_profiles { 1 } else { 0 },
            now,
            company_id,
        ],
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}

pub fn load_career_settings(conn: &Connection) -> Result<CareerSettings, String> {
    conn.query_row(
        r#"
        SELECT
            telemetry_enabled,
            local_stats_tracking_enabled,
            auto_job_logging_enabled,
            auto_finance_tracking_enabled,
            use_metric_units,
            use_24h_time,
            autosave_career_data,
            created_at,
            updated_at
        FROM career_settings
        WHERE id = 1
        "#,
        [],
        |row| {
            Ok(CareerSettings {
                telemetry_enabled: row.get::<_, i64>(0)? != 0,
                local_stats_tracking_enabled: row.get::<_, i64>(1)? != 0,
                auto_job_logging_enabled: row.get::<_, i64>(2)? != 0,
                auto_finance_tracking_enabled: row.get::<_, i64>(3)? != 0,
                use_metric_units: row.get::<_, i64>(4)? != 0,
                use_24h_time: row.get::<_, i64>(5)? != 0,
                autosave_career_data: row.get::<_, i64>(6)? != 0,
                created_at: row.get(7)?,
                updated_at: row.get(8)?,
            })
        },
    )
    .map_err(|e| e.to_string())
}

pub fn update_career_settings(
    conn: &Connection,
    input: &UpdateCareerSettingsInput,
    now: &str,
) -> Result<(), String> {
    let current = load_career_settings(conn)?;

    conn.execute(
        r#"
        UPDATE career_settings
        SET
            telemetry_enabled = ?1,
            local_stats_tracking_enabled = ?2,
            auto_job_logging_enabled = ?3,
            auto_finance_tracking_enabled = ?4,
            use_metric_units = ?5,
            use_24h_time = ?6,
            autosave_career_data = ?7,
            updated_at = ?8
        WHERE id = 1
        "#,
        params![
            if input.telemetry_enabled.unwrap_or(current.telemetry_enabled) {
                1
            } else {
                0
            },
            if input
                .local_stats_tracking_enabled
                .unwrap_or(current.local_stats_tracking_enabled)
            {
                1
            } else {
                0
            },
            if input
                .auto_job_logging_enabled
                .unwrap_or(current.auto_job_logging_enabled)
            {
                1
            } else {
                0
            },
            if input
                .auto_finance_tracking_enabled
                .unwrap_or(current.auto_finance_tracking_enabled)
            {
                1
            } else {
                0
            },
            if input.use_metric_units.unwrap_or(current.use_metric_units) {
                1
            } else {
                0
            },
            if input.use_24h_time.unwrap_or(current.use_24h_time) {
                1
            } else {
                0
            },
            if input
                .autosave_career_data
                .unwrap_or(current.autosave_career_data)
            {
                1
            } else {
                0
            },
            now,
        ],
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}

pub fn set_user_company(conn: &Connection, user_id: i64, company_id: i64) -> Result<(), String> {
    auth_repo::update_user_company(conn, user_id, company_id)
}
