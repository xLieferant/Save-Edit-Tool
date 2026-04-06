use serde::{Deserialize, Serialize};

pub type DispatcherSaveContext = crate::shared::models::save_context::SaveContext;

pub(super) const DISPATCHER_DEFAULT_INTERVAL_MINUTES: i64 = 10;
pub(super) const DISPATCHER_DEFAULT_MAX_OPEN_JOBS: i64 = 24;
pub(super) const DISPATCHER_MAX_GENERATION_BATCH: usize = 4;
pub(super) const DISPATCHER_OPEN_JOB_STATUSES: &[&str] = &["open"];
pub(super) const DISPATCHER_ACTIVE_JOB_STATUSES: &[&str] =
    &["planned", "accepted", "in_transit", "delayed", "problematic"];
pub(super) const DISPATCHER_BUSY_JOB_STATUSES: &[&str] =
    &["accepted", "in_transit", "delayed"];
pub(super) const DISPATCHER_HISTORY_JOB_STATUSES: &[&str] =
    &["completed", "problematic", "cancelled", "rejected", "expired"];
pub(super) const DISPATCHER_ALL_JOB_STATUSES: &[&str] = &[
    "open",
    "planned",
    "accepted",
    "in_transit",
    "delayed",
    "problematic",
    "completed",
    "cancelled",
    "rejected",
    "expired",
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DispatcherJobFilter {
    pub search: Option<String>,
    pub job_type: Option<String>,
    pub company_id: Option<String>,
    pub country: Option<String>,
    pub cargo_type: Option<String>,
    pub urgency: Option<String>,
    pub equipment_type: Option<String>,
    pub payment_tier: Option<String>,
    pub status: Option<String>,
    pub min_distance_km: Option<f64>,
    pub max_distance_km: Option<f64>,
    pub min_rate_per_km: Option<f64>,
    pub max_rate_per_km: Option<f64>,
    pub min_total_reward: Option<i64>,
    pub max_total_reward: Option<i64>,
    pub sort_by: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DispatcherMarketJob {
    pub id: String,
    pub source_type: String,
    pub company_id: String,
    pub company_name: String,
    pub job_type: String,
    pub cargo_type: String,
    pub origin_city: String,
    pub origin_country: String,
    pub destination_city: String,
    pub destination_country: String,
    pub distance_km: f64,
    pub cargo_mass_kg: f64,
    pub urgency_level: String,
    pub difficulty_level: String,
    pub equipment_type_required: String,
    pub trailer_type_required: Option<String>,
    pub base_rate_per_km: f64,
    pub calculated_rate_per_km: f64,
    pub total_reward: i64,
    pub estimated_duration_minutes: i64,
    pub payment_tier_snapshot: String,
    pub payment_tier: String,
    pub company_multiplier_snapshot: f64,
    pub company_reputation: u16,
    pub country_multiplier_snapshot: f64,
    pub reputation_multiplier_snapshot: f64,
    pub cargo_multiplier_snapshot: f64,
    pub urgency_multiplier_snapshot: f64,
    pub equipment_multiplier_snapshot: f64,
    pub market_variation_snapshot: f64,
    pub customer_multiplier_snapshot: f64,
    pub fuel_cost_estimate: i64,
    pub profit_estimate: i64,
    pub risk_note: Option<String>,
    pub bonus_note: Option<String>,
    pub expires_at_utc: Option<String>,
    pub status: String,
    pub dispatcher_status: String,
    pub progress_km: f64,
    pub route_distance_km: f64,
    pub profile_reference: Option<String>,
    pub save_reference: Option<String>,
    pub quicksave_reference: Option<String>,
    pub save_session_id: Option<String>,
    pub route_reference: Option<String>,
    pub ets2_job_link_status: Option<String>,
    pub accepted_at_utc: Option<String>,
    pub completed_at_utc: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DispatcherJobDetails {
    pub job: DispatcherMarketJob,
    pub payout_drivers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DispatcherHistorySummary {
    pub total_completed: i64,
    pub total_failed: i64,
    pub total_rejected: i64,
    pub revenue: i64,
    pub avg_rate_per_km: f64,
    pub avg_distance_km: f64,
    pub punctuality: f64,
    pub quality: f64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DispatcherHistoryResponse {
    pub summary: DispatcherHistorySummary,
    pub items: Vec<DispatcherMarketJob>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DispatcherCompanyContact {
    pub company_id: String,
    pub company_name: String,
    pub payment_tier: String,
    pub payment_multiplier: f64,
    pub customer_multiplier: f64,
    pub reputation: u16,
    pub reputation_multiplier: f64,
    pub home_country_code: Option<String>,
    pub country_multiplier: f64,
    pub cargo_focus: Option<String>,
    pub completed_jobs: i64,
    pub failed_jobs: i64,
    pub accepted_offers: i64,
    pub rejected_offers: i64,
    pub success_rate: f64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DispatcherOffer {
    pub id: String,
    pub company_id: String,
    pub company_name: String,
    pub user_id: String,
    pub offer_type: String,
    pub requested_job_type: String,
    pub requested_cargo_type: Option<String>,
    pub requested_region: Option<String>,
    pub proposed_rate_per_km: Option<f64>,
    pub note: Option<String>,
    pub equipment_type: Option<String>,
    pub contract_scope: Option<String>,
    pub status: String,
    pub counter_rate_per_km: Option<f64>,
    pub final_rate_per_km: Option<f64>,
    pub response_reason: Option<String>,
    pub linked_job_id: Option<String>,
    pub expires_at_utc: Option<String>,
    pub created_at_utc: String,
    pub updated_at_utc: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DispatcherOverview {
    pub open_market_jobs: i64,
    pub active_jobs: i64,
    pub open_offers: i64,
    pub accepted_contracts: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DispatcherGenerationStatus {
    pub interval_minutes: i64,
    pub max_open_jobs: i64,
    pub open_generated_jobs: i64,
    pub open_total_jobs: i64,
    pub last_generated_at_utc: Option<String>,
    pub last_cleanup_at_utc: Option<String>,
    pub next_generation_at_utc: Option<String>,
    pub current_context: DispatcherSaveContext,
    pub save_link_active: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DispatcherJobsBySaveContextResponse {
    pub context: DispatcherSaveContext,
    pub jobs: Vec<DispatcherMarketJob>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DispatcherCreateOfferInput {
    pub company_id: String,
    pub user_id: Option<String>,
    pub offer_type: String,
    pub requested_job_type: String,
    pub requested_cargo_type: Option<String>,
    pub requested_region: Option<String>,
    pub proposed_rate_per_km: Option<f64>,
    pub note: Option<String>,
    pub equipment_type: Option<String>,
    pub contract_scope: Option<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DispatcherRespondToCounterInput {
    pub offer_id: String,
    pub accept_counter: bool,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DispatcherGenerationConfigInput {
    pub interval_minutes: Option<i64>,
    pub max_open_jobs: Option<i64>,
}

#[derive(Debug, Clone)]
pub(super) struct DispatcherJobRow {
    pub id: String,
    pub source_type: String,
    pub company_id: String,
    pub company_name: String,
    pub job_type: String,
    pub cargo_type: String,
    pub origin_city: String,
    pub origin_country: String,
    pub destination_city: String,
    pub destination_country: String,
    pub distance_km: f64,
    pub cargo_mass_kg: f64,
    pub urgency_level: String,
    pub difficulty_level: String,
    pub equipment_type_required: String,
    pub trailer_type_required: Option<String>,
    pub base_rate_per_km: f64,
    pub calculated_rate_per_km: f64,
    pub total_reward: i64,
    pub estimated_duration_minutes: i64,
    pub payment_tier_snapshot: String,
    pub payment_multiplier_snapshot: f64,
    pub country_multiplier_snapshot: f64,
    pub reputation_multiplier_snapshot: f64,
    pub cargo_multiplier_snapshot: f64,
    pub urgency_multiplier_snapshot: f64,
    pub equipment_multiplier_snapshot: f64,
    pub market_variation_snapshot: f64,
    pub customer_multiplier_snapshot: f64,
    pub company_reputation: i64,
    pub fuel_cost_estimate: i64,
    pub profit_estimate: i64,
    pub risk_note: Option<String>,
    pub bonus_note: Option<String>,
    pub expires_at_utc: Option<String>,
    pub status: String,
    pub progress_km: f64,
    pub profile_reference: Option<String>,
    pub save_reference: Option<String>,
    pub quicksave_reference: Option<String>,
    pub save_session_id: Option<String>,
    pub route_reference: Option<String>,
    pub ets2_job_link_status: Option<String>,
    pub accepted_at_utc: Option<String>,
    pub completed_at_utc: Option<String>,
    pub created_at_utc: String,
    pub updated_at_utc: String,
}

#[derive(Debug, Clone)]
pub(super) struct DispatcherGenerationConfigRow {
    pub interval_minutes: i64,
    pub max_open_jobs: i64,
    pub last_generated_at_utc: Option<String>,
    pub last_cleanup_at_utc: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DispatcherGenerationRunResult {
    pub status: DispatcherGenerationStatus,
    pub market_changed: bool,
}
