use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TruckInventoryItem {
    pub truck_id: String,
    pub display_index: usize,
    pub brand: Option<String>,
    pub model: Option<String>,
    pub raw_license_plate: Option<String>,
    pub display_license_plate: Option<String>,
    pub license_plate: Option<String>,
    pub garage_id: Option<String>,
    pub garage_display_name: Option<String>,
    pub assigned_garage: Option<String>,
    pub assigned_driver_id: Option<String>,
    pub driver_display_name: Option<String>,
    pub country_code: Option<String>,
    pub country_display_name: Option<String>,
    pub is_active: bool,
    pub is_switchable: bool,
    pub blocked_reason: Option<String>,
    pub requires_driver_swap: bool,
    pub engine_data_path: Option<String>,
    pub transmission_data_path: Option<String>,
    pub accessory_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DriverDisplayInfo {
    pub driver_id: String,
    pub display_name: Option<String>,
    pub current_truck_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TruckAssignment {
    pub truck_id: String,
    pub driver_id: Option<String>,
    pub driver_name: Option<String>,
    pub garage_id: Option<String>,
    pub garage_name: Option<String>,
    pub is_player_truck: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct VehicleAccessoryNode {
    pub id: String,
    pub unit_type: String,
    pub data_path: Option<String>,
    pub raw_block: String,
    pub references: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TruckGraph {
    pub vehicle_id: String,
    pub vehicle_block: String,
    pub accessory_ids: Vec<String>,
    pub accessories: Vec<VehicleAccessoryNode>,
    pub referenced_unit_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TruckChangePreview {
    pub current_truck: TruckInventoryItem,
    pub target_truck: TruckInventoryItem,
    pub current_player_truck: TruckInventoryItem,
    pub selected_truck: TruckInventoryItem,
    pub affected_driver: Option<DriverDisplayInfo>,
    pub driver_receives_truck: Option<TruckInventoryItem>,
    pub warnings: Vec<String>,
    pub expected_file_hash: String,
    pub can_apply: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TruckWriteValidation {
    pub success: bool,
    pub expected_truck_id: String,
    pub actual_truck_id: Option<String>,
    pub dangling_references: Vec<String>,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ApplyTruckChangeResult {
    pub success: bool,
    pub backup_id: Option<String>,
    pub persistent_backup_created: bool,
    pub temporary_rollback_used: bool,
    pub temporary_rollback_cleaned: bool,
    pub previous_truck_id: String,
    pub active_truck_id: String,
    pub affected_driver_id: Option<String>,
    pub driver_received_truck_id: Option<String>,
    pub file_hash_before: String,
    pub file_hash_after: String,
    pub validation: TruckWriteValidation,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TruckSwitchList {
    pub save_path: String,
    pub file_hash: String,
    pub active_truck_id: Option<String>,
    pub trucks: Vec<TruckInventoryItem>,
    pub diagnostics: OwnedTruckDiagnostics,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GarageSlotAssignment {
    pub garage_id: String,
    pub garage_display_name: Option<String>,
    pub country_code: Option<String>,
    pub country_display_name: Option<String>,
    pub slot_index: usize,
    pub truck_id: String,
    pub driver_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GarageCapacity {
    pub garage_id: String,
    pub garage_display_name: Option<String>,
    pub total_truck_slots: usize,
    pub occupied_truck_slots: usize,
    pub free_truck_slots: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct OwnedTruckDiagnostics {
    pub total_vehicle_blocks: usize,
    pub candidate_trucks: usize,
    pub owned_trucks: usize,
    pub excluded_trailers: usize,
    pub excluded_unreferenced: usize,
    pub excluded_job_vehicles: usize,
    pub excluded_duplicates: usize,
    pub excluded_invalid: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PowertrainCatalog {
    pub schema_version: u32,
    pub game: String,
    pub game_version: String,
    pub generated_at: String,
    pub sources: Vec<String>,
    pub engines: Vec<PowertrainEngine>,
    pub transmissions: Vec<PowertrainTransmission>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PowertrainEngine {
    pub id: String,
    pub data_path: String,
    pub brand: String,
    pub truck_model: String,
    pub name: String,
    #[serde(rename = "type")]
    pub engine_type: String,
    pub torque_nm: Option<f64>,
    pub power: Option<f64>,
    pub rpm_idle: Option<f64>,
    pub rpm_limit: Option<f64>,
    pub official: bool,
    pub source_archive: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PowertrainTransmission {
    pub id: String,
    pub data_path: String,
    pub brand: String,
    pub truck_model: String,
    pub name: String,
    pub gears_forward: Option<u32>,
    pub ratios_forward: Vec<f64>,
    pub ratios_reverse: Vec<f64>,
    pub differential_ratio: Option<f64>,
    pub retarder_steps: Option<u32>,
    pub official: bool,
    pub source_archive: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PowertrainComponentPreview {
    pub current_data_path: Option<String>,
    pub selected_data_path: Option<String>,
    pub selected_name: Option<String>,
    pub selected_family: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TruckPowertrainPreview {
    pub truck_id: String,
    pub truck_family: Option<String>,
    pub current_engine: PowertrainComponentPreview,
    pub new_engine: Option<PowertrainComponentPreview>,
    pub current_transmission: PowertrainComponentPreview,
    pub new_transmission: Option<PowertrainComponentPreview>,
    pub selected_differential_ratio: Option<f64>,
    pub engine_same_family: Option<bool>,
    pub transmission_same_family: Option<bool>,
    pub experimental_cross_brand: bool,
    pub warnings: Vec<String>,
    pub can_apply_later: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TruckTransferSelection {
    pub truck_id: String,
    pub target_garage_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TruckTransferPreview {
    pub selected_truck_count: usize,
    pub free_truck_slots: usize,
    pub can_apply: bool,
    pub error: Option<String>,
    pub source_graphs: Vec<TruckGraph>,
    pub id_remap: HashMap<String, String>,
    pub target_garages: Vec<GarageCapacity>,
    pub warnings: Vec<String>,
}
