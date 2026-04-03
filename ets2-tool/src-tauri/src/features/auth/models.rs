use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicUser {
    pub id: i64,
    pub username: String,
    pub email: String,
    pub role: String,
    pub company_id: Option<i64>,
    pub created_at: String,
    pub updated_at: String,
    pub consent_at: String,
    pub is_active: bool,
    pub is_seed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthLoginResult {
    pub user: PublicUser,
    pub remember_me: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthRegisterResult {
    pub user: PublicUser,
    pub remember_me: bool,
}

#[derive(Debug, Clone)]
pub struct NewUser {
    pub username: String,
    pub email: String,
    pub password_hash: String,
    pub role: String,
    pub company_id: Option<i64>,
    pub created_at: String,
    pub updated_at: String,
    pub consent_at: String,
    pub is_active: bool,
    pub is_seed: bool,
}

#[derive(Debug, Clone)]
pub struct UserRecord {
    pub id: i64,
    pub username: String,
    pub email: String,
    pub password_hash: String,
    pub role: String,
    pub company_id: Option<i64>,
    pub created_at: String,
    pub updated_at: String,
    pub consent_at: String,
    pub is_active: bool,
    pub is_seed: bool,
}

impl From<UserRecord> for PublicUser {
    fn from(value: UserRecord) -> Self {
        Self {
            id: value.id,
            username: value.username,
            email: value.email,
            role: value.role,
            company_id: value.company_id,
            created_at: value.created_at,
            updated_at: value.updated_at,
            consent_at: value.consent_at,
            is_active: value.is_active,
            is_seed: value.is_seed,
        }
    }
}

#[derive(Debug, Clone)]
pub struct NewSession {
    pub user_id: i64,
    pub token: String,
    pub created_at: String,
    pub expires_at: String,
    pub last_used_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthSessionOverview {
    pub id: i64,
    pub created_at: String,
    pub expires_at: Option<String>,
    pub last_used_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthMauSnapshot {
    pub year_month: String,
    pub installation_active: bool,
    pub active_accounts: u32,
    pub current_account_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthAccountOverview {
    pub user: Option<PublicUser>,
    pub sessions: Vec<AuthSessionOverview>,
    pub current_session_id: Option<i64>,
    pub unused_recovery_codes: u32,
    pub mau: AuthMauSnapshot,
}
