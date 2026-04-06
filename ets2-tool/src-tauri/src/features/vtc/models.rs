use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsernameAvailability {
    pub available: bool,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserSettings {
    pub user_id: i64,
    pub language: String,
    pub preferred_game: String,
    pub profile_visibility: String,
    pub username_last_changed_at: Option<String>,
    pub theme_preference: Option<String>,
    pub notifications_enabled: bool,
    pub avatar_path: Option<String>,
    pub bio: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserProfile {
    pub user_id: i64,
    pub username: String,
    pub language: String,
    pub avatar_path: Option<String>,
    pub bio: Option<String>,
    pub in_company: bool,
    pub company_id: Option<i64>,
    pub company_role: Option<String>,
    pub username_last_changed_at: Option<String>,
    pub username_next_change_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompanyOverview {
    pub id: i64,
    pub name: String,
    pub location: String,
    pub language: Option<String>,
    pub game: Option<String>,
    pub description: Option<String>,
    pub logo_path: Option<String>,
    pub header_path: Option<String>,
    pub slogan: Option<String>,
    pub accent_color: Option<String>,
    pub public_visibility: bool,
    pub owner_user_id: i64,
    pub created_at: String,
    pub updated_at: String,
    pub members_count: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompanyMember {
    pub id: i64,
    pub company_id: i64,
    pub user_id: i64,
    pub username: String,
    pub role_key: String,
    pub joined_at: String,
    pub promoted_at: Option<String>,
    pub invited_by: Option<i64>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompanySettings {
    pub company_id: i64,
    pub company_language: String,
    pub company_game: String,
    pub allow_public_join_requests: bool,
    pub show_company_publicly: bool,
    pub default_member_role: String,
    pub dispatcher_can_manage_jobs: bool,
    pub trainee_visible_in_roster: bool,
    pub allow_member_custom_profiles: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CareerSettings {
    pub telemetry_enabled: bool,
    pub local_stats_tracking_enabled: bool,
    pub auto_job_logging_enabled: bool,
    pub auto_finance_tracking_enabled: bool,
    pub use_metric_units: bool,
    pub use_24h_time: bool,
    pub autosave_career_data: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompanyRoleOption {
    pub role_key: String,
    pub role_label: String,
    pub sort_order: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VtcRuntimeContext {
    pub user_id: i64,
    pub username: String,
    pub profile_reference: Option<String>,
    pub save_reference: Option<String>,
    pub save_session_id: Option<String>,
    pub has_active_profile: bool,
    pub has_active_save: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateUserSettingsInput {
    pub language: Option<String>,
    pub preferred_game: Option<String>,
    pub profile_visibility: Option<String>,
    pub theme_preference: Option<String>,
    pub notifications_enabled: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCompanySettingsInput {
    pub company_language: Option<String>,
    pub company_game: Option<String>,
    pub allow_public_join_requests: Option<bool>,
    pub show_company_publicly: Option<bool>,
    pub default_member_role: Option<String>,
    pub dispatcher_can_manage_jobs: Option<bool>,
    pub trainee_visible_in_roster: Option<bool>,
    pub allow_member_custom_profiles: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCareerSettingsInput {
    pub telemetry_enabled: Option<bool>,
    pub local_stats_tracking_enabled: Option<bool>,
    pub auto_job_logging_enabled: Option<bool>,
    pub auto_finance_tracking_enabled: Option<bool>,
    pub use_metric_units: Option<bool>,
    pub use_24h_time: Option<bool>,
    pub autosave_career_data: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCompanyProfileInput {
    pub name: Option<String>,
    pub location: Option<String>,
    pub language: Option<String>,
    pub game: Option<String>,
    pub description: Option<String>,
    pub logo_path: Option<String>,
    pub header_path: Option<String>,
    pub slogan: Option<String>,
    pub accent_color: Option<String>,
    pub public_visibility: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateUserProfileMetaInput {
    pub avatar_path: Option<String>,
    pub bio: Option<String>,
    pub profile_visibility: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateCompanyInput {
    pub name: String,
    pub location: String,
    pub language: String,
    pub game: String,
    pub description: Option<String>,
    pub logo_path: Option<String>,
    pub header_path: Option<String>,
    pub slogan: Option<String>,
    pub accent_color: Option<String>,
    pub public_visibility: Option<bool>,
}
