use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use std::collections::HashMap;

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
    pub resolved_source_company_token: Option<String>,
    pub resolved_source_city_token: Option<String>,
    pub resolved_target_company_token: Option<String>,
    pub resolved_target_city_token: Option<String>,
    pub requested_cargo_token: Option<String>,
    pub resolved_cargo_token: Option<String>,
    pub cargo_resolution_mode: Option<String>,
    pub cargo_validation_source: Option<String>,
    pub cargo_valid_for_snapshot: Option<bool>,
    pub cargo_id: String,
    pub distance_km: f64,
    pub planned_reward: i64,
    pub patch: EtsJobOfferPatch,
    pub save_offer_template: Option<DispatcherSaveOfferTemplate>,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SaveJobOfferPointer {
    pub index: usize,
    pub pointer: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SaveDepotBlock {
    pub unit_token: String,
    pub company_token: String,
    pub city_token: String,
    pub permanent_data: Option<String>,
    pub job_offer_count: usize,
    pub job_offers: Vec<SaveJobOfferPointer>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SaveJobOfferData {
    pub pointer: String,
    pub target: Option<String>,
    pub expiration_time: Option<i64>,
    pub urgency: Option<i64>,
    pub shortest_distance_km: Option<i64>,
    pub ferry_time: Option<i64>,
    pub ferry_price: Option<i64>,
    pub cargo: Option<String>,
    pub company_truck: Option<String>,
    pub trailer_variant: Option<String>,
    pub trailer_definition: Option<String>,
    pub units_count: Option<i64>,
    pub fill_ratio: Option<i64>,
    pub trailer_place: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SaveJobInfoSnapshot {
    pub pointer: String,
    pub cargo: Option<String>,
    pub source_company: Option<String>,
    pub target_company: Option<String>,
    pub planned_distance_km: Option<i64>,
    pub ferry_time: Option<i64>,
    pub ferry_price: Option<i64>,
    pub urgency: Option<String>,
    pub units_count: Option<i64>,
    pub fill_ratio: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DispatcherResolvedSaveLink {
    pub resolution_mode: String,
    pub requested_source_company_token: String,
    pub requested_source_city_token: String,
    pub requested_target_city_token: String,
    pub resolved_source_depot_block: String,
    pub resolved_source_company_token: String,
    pub resolved_source_city_token: String,
    pub resolved_target_company_token: String,
    pub resolved_target_city_token: String,
    pub resolved_offer_pointer: String,
    pub resolved_job_offer_data_pointer: String,
    pub resolved_job_info_pointer: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DispatcherSaveOfferTemplate {
    pub dispatcher_job_id: String,
    pub source_type: String,
    pub job_type: String,
    pub company_id: String,
    pub company_token: String,
    pub company_name: String,
    pub source_city_token: String,
    pub source_city_name: String,
    pub source_country_token: Option<String>,
    pub target_city_token: String,
    pub target_city_name: String,
    pub target_country_token: Option<String>,
    pub target_company_token: String,
    pub target_company_name: Option<String>,
    pub cargo_token: String,
    pub requested_cargo_token: Option<String>,
    pub resolved_cargo_token: Option<String>,
    pub cargo_resolution_mode: Option<String>,
    pub cargo_validation_source: Option<String>,
    pub cargo_valid_for_snapshot: Option<bool>,
    pub cargo_name: Option<String>,
    pub trailer_variant_token: Option<String>,
    pub trailer_definition_token: Option<String>,
    pub company_truck_token: Option<String>,
    pub company_truck: bool,
    pub shortest_distance_km: i64,
    pub urgency: i64,
    pub ferry_time: i64,
    pub ferry_price: i64,
    pub units_count: i64,
    pub fill_ratio: i64,
    pub trailer_place: i64,
    pub expiration_time: i64,
    pub planned_distance_km: i64,
    pub save_reference: Option<String>,
    pub quicksave_reference: Option<String>,
    pub save_session_id: Option<String>,
    pub ets2_job_link_status: String,
    pub dispatcher_status: String,
    pub last_error_code: Option<String>,
    pub last_error_message: Option<String>,
    pub resolved: DispatcherResolvedSaveLink,
    pub job_info: Option<SaveJobInfoSnapshot>,
    pub companies_index: Vec<String>,
    pub visited_cities: Vec<String>,
    pub source_city_visited: bool,
    pub target_city_visited: bool,
    pub depots_by_city: HashMap<String, Vec<String>>,
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
    pub save_session_id: Option<String>,
    pub route_reference: Option<String>,
    pub dispatcher_status: Option<String>,
    pub last_error_code: Option<String>,
    pub last_error_message: Option<String>,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct PostWriteOfferSlotScan {
    pub index: i64,
    pub pointer: String,
    pub offer_data_found: bool,
    pub selected: bool,
    pub matches_expected_pointer: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct PostWriteValidationResult {
    pub valid: bool,
    pub company_block_found: bool,
    pub offer_pointer_found: bool,
    pub offer_data_found: bool,
    pub cargo_matches: bool,
    pub target_matches: bool,
    pub shortest_distance_present: bool,
    pub expiration_time_present: bool,
    pub expected_company: String,
    pub expected_offer_pointer: String,
    pub expected_cargo: String,
    pub expected_target: String,
    pub written_cargo: Option<String>,
    pub written_target: Option<String>,
    pub written_shortest_distance_km: Option<i64>,
    pub written_expiration_time: Option<i64>,
    pub selected_offer_slot_index: Option<i64>,
    pub selected_offer_slot_pointer: Option<String>,
    pub root_cause: String,
    pub validation_error_code: Option<String>,
    pub validation_error: Option<String>,
    pub offer_slots: Vec<PostWriteOfferSlotScan>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EtsJobWriteResult {
    pub link: EtsJobLink,
    pub save_path: String,
    pub backup_path: Option<String>,
    pub before_sha256: String,
    pub after_sha256: String,
    pub write_mode: String,
    pub job_info_updated: bool,
    pub post_write_valid: bool,
    pub validation: PostWriteValidationResult,
    pub post_write_validated: bool,
    pub company_block_found_after_write: bool,
    pub offer_pointer_found_after_write: bool,
    pub job_offer_data_found_after_write: bool,
    pub cargo_written_token: String,
    pub target_written_token: String,
    pub shortest_distance_written: Option<i64>,
    pub expiration_time_written: Option<i64>,
    pub job_info_status: String,
    pub validation_error_code: Option<String>,
    pub validation_error_message: Option<String>,
    pub offer_slot_index: Option<i64>,
    pub offer_slot_pointer: Option<String>,
    pub expected_load_path: Option<String>,
    pub load_path_warning: Option<String>,
}
