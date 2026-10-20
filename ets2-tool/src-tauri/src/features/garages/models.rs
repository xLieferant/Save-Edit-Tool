use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GarageSize {
    Unowned,
    Small,
    Large,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GarageOwnership {
    Owned,
    NotOwned,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GarageSlotInfo {
    pub index: usize,
    pub truck_id: Option<String>,
    pub driver_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GarageInfo {
    pub garage_id: String,
    pub city_token: Option<String>,
    pub city_name: Option<String>,
    pub country_code: Option<String>,
    pub status: Option<i32>,
    pub size: GarageSize,
    pub ownership: GarageOwnership,
    pub vehicle_slot_count: usize,
    pub driver_slot_count: usize,
    pub trailer_slot_count: usize,
    pub maximum_slot_count: usize,
    pub occupied_slots: usize,
    pub available_slots: usize,
    pub assigned_driver_count: usize,
    pub assigned_truck_count: usize,
    pub assigned_trailer_count: usize,
    pub slots: Vec<GarageSlotInfo>,
    pub trailer_ids: Vec<String>,
    pub is_headquarters: bool,
    pub capacity_consistent: bool,
    pub profit_log_id: Option<String>,
    pub productivity: Option<f32>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GarageParseDiagnostics {
    pub declared_garage_count: usize,
    pub referenced_garage_count: usize,
    pub parsed_garage_count: usize,
    pub owned_garage_count: usize,
    pub not_owned_garage_count: usize,
    pub unknown_status_count: usize,
    pub unreferenced_garage_count: usize,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GarageListResult {
    pub game: String,
    pub save_hash: String,
    pub headquarters_garage_id: Option<String>,
    pub garages: Vec<GarageInfo>,
    pub diagnostics: GarageParseDiagnostics,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GarageOperation {
    Purchase,
    Relinquish,
    Upgrade,
    Update,
    AssignResources,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GarageBulkOperation {
    PurchaseAll,
    RelinquishEmpty,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GarageMutationRequest {
    pub garage_id: String,
    pub expected_save_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GarageResourceAssignmentRequest {
    pub garage_id: String,
    pub expected_save_hash: String,
    #[serde(default)]
    pub assign_random_driver: bool,
    #[serde(default)]
    pub assign_random_truck: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GarageBuyAllRequest {
    pub expected_save_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GarageRelinquishEmptyRequest {
    pub expected_save_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GarageUpdateRequest {
    pub garage_id: String,
    pub expected_save_hash: String,
    pub target_size: Option<GarageSize>,
    #[serde(default)]
    pub set_as_headquarters: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GarageMutationResult {
    pub garage_id: String,
    pub operation: GarageOperation,
    pub previous_state: GarageInfo,
    pub updated_state: GarageInfo,
    pub backup_id: String,
    pub backup_created: bool,
    pub verified: bool,
    pub financial_transaction_applied: bool,
    pub save_hash: String,
    pub assigned_driver_id: Option<String>,
    pub assigned_truck_id: Option<String>,
    pub assigned_driver_slot_index: Option<usize>,
    pub assigned_truck_slot_index: Option<usize>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GarageBuyAllResult {
    pub operation: GarageBulkOperation,
    pub purchased_garage_ids: Vec<String>,
    pub purchased_count: usize,
    pub backup_id: Option<String>,
    pub backup_created: bool,
    pub verified: bool,
    pub financial_transaction_applied: bool,
    pub save_hash: String,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GarageRelinquishEmptyResult {
    pub operation: GarageBulkOperation,
    pub relinquished_garage_ids: Vec<String>,
    pub relinquished_count: usize,
    pub backup_id: Option<String>,
    pub backup_created: bool,
    pub verified: bool,
    pub financial_transaction_applied: bool,
    pub save_hash: String,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AiDriverSkills {
    pub adr: u32,
    pub long_dist: u32,
    pub heavy: u32,
    pub fragile: u32,
    pub urgent: u32,
    pub mechanical: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AiDriverInfo {
    pub driver_id: String,
    pub display_name: Option<String>,
    pub experience_points: Option<u64>,
    pub level: Option<u32>,
    pub skills: Option<AiDriverSkills>,
    pub state: Option<i32>,
    pub training_policy: Option<i32>,
    pub garage_id: String,
    pub garage_name: Option<String>,
    pub garage_slot_index: usize,
    pub truck_id: Option<String>,
    pub active_job: bool,
    pub removable: bool,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AiDriverTruckInfo {
    pub truck_id: String,
    pub garage_id: String,
    pub garage_name: Option<String>,
    pub garage_slot_index: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AiDriverManagementResult {
    pub game: String,
    pub save_hash: String,
    pub drivers: Vec<AiDriverInfo>,
    pub available_driver_count: usize,
    pub available_trucks: Vec<AiDriverTruckInfo>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AiDriverGenerateRequest {
    pub garage_id: String,
    pub expected_save_hash: String,
    pub count: usize,
    #[serde(default)]
    pub assign_trucks: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AiDriverTruckAssignmentRequest {
    pub driver_id: String,
    pub expected_save_hash: String,
    pub truck_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AiDriverRemoveRequest {
    pub driver_ids: Vec<String>,
    pub expected_save_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AiDriverMutationOperation {
    Generate,
    AssignTruck,
    UnassignTruck,
    Remove,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AiDriverMutationResult {
    pub operation: AiDriverMutationOperation,
    pub affected_driver_ids: Vec<String>,
    pub backup_id: String,
    pub backup_created: bool,
    pub verified: bool,
    pub save_hash: String,
    pub management: AiDriverManagementResult,
    pub warnings: Vec<String>,
}
