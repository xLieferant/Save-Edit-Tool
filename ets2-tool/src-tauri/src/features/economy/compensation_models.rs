use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum BaseRateType {
    QuickJob,
    OwnTruck,
    OwnTruckOwnTrailer,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum EquipmentType {
    QuickJob,
    OwnTruck,
    OwnTruckOwnTrailer,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum CargoType {
    Standard,
    Fragile,
    Refrigerated,
    Valuable,
    Hazardous,
    Oversize,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Urgency {
    Normal,
    Priority,
    Express,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum CompanyPaymentTier {
    Budget,
    Standard,
    Good,
    Premium,
    Elite,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct JobCompensationInput {
    pub company_id: String,
    pub company_name: Option<String>,
    pub distance_km: f64,
    pub base_rate_type: BaseRateType,
    pub equipment_type: EquipmentType,
    pub cargo_type: CargoType,
    pub urgency: Urgency,
    pub origin_country_code: String,
    pub destination_country_code: String,
    pub market_seed: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CompanyPaymentProfile {
    pub company_id: String,
    pub company_name: Option<String>,
    pub payment_tier: CompanyPaymentTier,
    pub payment_multiplier: f64,
    pub home_country_code: Option<String>,
    pub cargo_focus: Option<String>,
    pub updated_at_utc: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UpsertCompanyPaymentProfileInput {
    pub company_id: String,
    pub company_name: Option<String>,
    pub payment_tier: CompanyPaymentTier,
    pub payment_multiplier: f64,
    pub home_country_code: Option<String>,
    pub cargo_focus: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CountryPaymentLevel {
    pub country_code: String,
    pub country_name: String,
    pub payment_multiplier: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CompanyCompensationCondition {
    pub company_id: String,
    pub company_name: String,
    pub payment_tier: CompanyPaymentTier,
    pub payment_multiplier: f64,
    pub customer_multiplier: f64,
    pub reputation: u16,
    pub reputation_multiplier: f64,
    pub home_country_code: Option<String>,
    pub home_country_multiplier: f64,
    pub cargo_focus: Option<String>,
    pub effective_multiplier: f64,
    pub updated_at_utc: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CompanyReputationState {
    pub company_id: String,
    pub reputation: u16,
    pub reliability_streak: u16,
    pub completed_jobs: u32,
    pub late_jobs: u32,
    pub damage_incidents: u32,
    pub canceled_jobs: u32,
    pub updated_at_utc: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CompanyReputationOutcome {
    pub completed: bool,
    pub on_time: bool,
    pub damage_percent: f64,
    pub canceled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct JobCompensationResult {
    pub distance_km: f64,
    pub company_id: String,
    pub origin_country_code: String,
    pub destination_country_code: String,
    pub company_reputation: u16,
    pub company_payment_tier: CompanyPaymentTier,
    pub company_payment_multiplier: f64,
    pub base_rate_per_km: f64,
    pub customer_multiplier: f64,
    pub country_multiplier: f64,
    pub equipment_multiplier: f64,
    pub company_reputation_multiplier: f64,
    pub cargo_multiplier: f64,
    pub urgency_multiplier: f64,
    pub market_variation: f64,
    pub final_rate_per_km: f64,
    pub final_price: i64,
}
