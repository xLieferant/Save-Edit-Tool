use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EtsProfile {
    pub profile_id: String,
    pub profile_path: String,
    pub game: String,
    pub steam_cloud_enabled: bool,
    pub created_at_utc: String,
    pub updated_at_utc: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EtsSaveSlot {
    pub save_id: String,
    pub profile_id: String,
    pub slot_name: String,
    pub save_path: String,
    pub game_sii_path: String,
    pub is_quicksave: bool,
    pub modified_at_utc: String,
    pub created_at_utc: String,
    pub updated_at_utc: String,
    pub last_loaded_at_utc: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "TEXT", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum EtsJobLinkStatus {
    Pending,
    Prepared,
    Written,
    RequiresLoad,
    Synced,
    Completed,
    Error,
}

impl EtsJobLinkStatus {
    pub fn as_db(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Prepared => "prepared",
            Self::Written => "written",
            Self::RequiresLoad => "requires_load",
            Self::Synced => "synced",
            Self::Completed => "completed",
            Self::Error => "error",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EtsJobLink {
    pub link_id: String,
    pub profile_id: String,
    pub save_id: String,
    pub vtc_job_id: String,
    pub offer_pointer: Option<String>,
    pub job_offer_data_pointer: Option<String>,
    pub src_company: String,
    pub src_city: String,
    pub dst_company: String,
    pub dst_city: String,
    pub cargo_id: String,
    pub distance_km: f64,
    pub planned_reward: i64,
    pub patch: EtsJobOfferPatch,
    pub status: EtsJobLinkStatus,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub created_at_utc: String,
    pub updated_at_utc: String,
    pub written_at_utc: Option<String>,
    pub requires_load_at_utc: Option<String>,
    pub synced_at_utc: Option<String>,
    pub completed_at_utc: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct VtcDispatcherJob {
    pub vtc_job_id: String,
    pub source_type: String,
    pub company_id: String,
    pub company_name: String,
    pub payment_tier: Option<String>,
    pub job_type: String,
    pub cargo_type: String,
    pub cargo_mass_kg: f64,
    pub urgency_level: String,
    pub difficulty_level: String,
    pub equipment_type_required: String,
    pub trailer_type_required: Option<String>,
    pub origin_city: String,
    pub origin_country: String,
    pub destination_city: String,
    pub destination_country: String,
    pub route_distance_km: f64,
    pub estimated_duration_minutes: i64,
    pub base_rate_per_km: f64,
    pub calculated_rate_per_km: f64,
    pub total_reward: i64,
    pub profile_reference: Option<String>,
    pub quicksave_reference: Option<String>,
    pub save_reference: Option<String>,
    pub route_reference: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EtsJobOfferPatch {
    pub target: String,
    pub expiration_time: i64,
    pub urgency: i64,
    pub shortest_distance_km: i64,
    pub ferry_time: i64,
    pub ferry_price: i64,
    pub cargo: String,
    pub company_truck: bool,
    pub trailer_variant: Option<String>,
    pub trailer_definition: Option<String>,
    pub units_count: i64,
    pub fill_ratio: i64,
    pub trailer_place: i64,
    pub job_info_unit: Option<String>,
    pub selected_job_unit: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EtsJobWriteResult {
    pub link: EtsJobLink,
    pub backup_path: String,
}
