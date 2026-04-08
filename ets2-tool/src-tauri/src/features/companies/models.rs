use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Company {
    pub id: i64,
    pub owner_user_id: i64,
    pub name: String,
    pub logo_path: Option<String>,
    pub header_path: Option<String>,
    pub language: Option<String>,
    pub game: Option<String>,
    pub description: Option<String>,
    pub salary_base: i64,
    pub location: String,
    pub job_type: String,
    pub created_at: String,
    pub updated_at: String,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompanyListItem {
    pub id: i64,
    pub name: String,
    pub logo_path: Option<String>,
    pub description: Option<String>,
    pub location: String,
    pub job_type: String,
    pub language: Option<String>,
    pub game: Option<String>,
    pub members_count: i64,
}

#[derive(Debug, Clone)]
pub struct NewCompany {
    pub owner_user_id: i64,
    pub name: String,
    pub logo_path: Option<String>,
    pub logo_blob: Option<Vec<u8>>,
    pub logo_mime: Option<String>,
    pub header_path: Option<String>,
    pub header_blob: Option<Vec<u8>>,
    pub header_mime: Option<String>,
    pub language: Option<String>,
    pub game: Option<String>,
    pub description: Option<String>,
    pub salary_base: i64,
    pub location: String,
    pub job_type: String,
    pub created_at: String,
    pub updated_at: String,
    pub is_active: bool,
}
