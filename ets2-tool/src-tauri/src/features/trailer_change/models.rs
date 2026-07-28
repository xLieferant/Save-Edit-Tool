use serde::{Deserialize, Serialize};
use serde_json::Value;

pub type NormalizedSiiId = String;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TrailerSwitchMode {
    FreeTrailer,
    SlotSwap,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CurrentTrailerPointerKind {
    PlayerAssignedVehicles,
    PlayerAssignedTrailer,
    PlayerMyTrailer,
    FallbackPlayerVehicles,
    FallbackFirstOwnedTrailer,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CurrentTrailerPointer {
    pub kind: CurrentTrailerPointerKind,
    pub trailer_id: String,
    pub owner_unit_id: String,
    pub field_name: String,
    pub referenced_player_vehicle_unit_id: Option<String>,
    pub source: String,
    pub confidence: String,
    pub writable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct CurrentTrailerPointerDiagnostics {
    pub player_found: bool,
    pub my_trailer_raw: Option<String>,
    pub my_trailer_block_found: bool,
    pub assigned_vehicles_raw: Option<String>,
    pub assigned_vehicles_unit_found: bool,
    pub assigned_vehicles_trailer_raw: Option<String>,
    pub assigned_vehicles_trailer_block_found: bool,
    pub assigned_trailer_raw: Option<String>,
    pub assigned_trailer_block_found: bool,
    pub current_trailer_pointer_kind: Option<CurrentTrailerPointerKind>,
    pub current_trailer_id: Option<String>,
    pub current_trailer_source: Option<String>,
    pub current_trailer_confidence: Option<String>,
    pub fallback_player_vehicle_unit_id: Option<String>,
    pub fallback_player_vehicle_trailer_raw: Option<String>,
    pub fallback_first_owned_trailer_raw: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PlayerTrailerSlotAssignment {
    pub slot_id: String,
    pub slot_id_normalized: NormalizedSiiId,
    pub slot_index: Option<usize>,
    pub trailer_id: Option<String>,
    pub trailer_id_normalized: Option<NormalizedSiiId>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TrailerInventoryItem {
    pub id: String,
    pub trailer_id: String,
    pub unit_id: String,
    pub nameless_id: String,
    pub display_index: usize,
    pub display_name: String,
    pub brand: Option<String>,
    pub model: Option<String>,
    pub raw_license_plate: Option<String>,
    pub display_license_plate: Option<String>,
    pub license_plate: Option<String>,
    pub garage_city: Option<String>,
    pub garage_country: Option<String>,
    pub garage_id: Option<String>,
    pub garage_display_name: Option<String>,
    pub assigned_garage: Option<String>,
    pub driver_label: Option<String>,
    pub owner_label: Option<String>,
    pub assignment_label: Option<String>,
    pub is_active: bool,
    pub is_available: bool,
    pub is_switchable: bool,
    pub availability_reason: Option<String>,
    pub assigned_driver_id: Option<String>,
    pub assigned_storage_id: Option<String>,
    pub source: String,
    pub accessory_count: usize,
    pub cargo_mass: Option<f32>,
    pub wear: Option<f32>,
    pub player_vehicle_slot_id: Option<String>,
    pub player_vehicle_slot_index: Option<usize>,
    pub technical_details: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct OwnedTrailerDiagnostics {
    pub total_trailer_blocks: usize,
    pub owned_trailers: usize,
    pub player_trailers_array_count: usize,
    pub player_trailer_refs_with_blocks: usize,
    pub player_trailer_reference_missing_blocks: Vec<String>,
    pub current_trailer_pointer_kind: Option<CurrentTrailerPointerKind>,
    pub current_trailer_id: Option<String>,
    pub assigned_vehicles_unit_id: Option<String>,
    pub current_trailer_pointer: CurrentTrailerPointerDiagnostics,
    pub current_trailer_source: Option<String>,
    pub current_trailer_confidence: Option<String>,
    pub excluded_job_trailers: usize,
    pub excluded_duplicates: usize,
    pub excluded_invalid: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TrailerSwapPreviewDetails {
    pub current_trailer_id: Option<String>,
    pub target_trailer_id: String,
    pub target_location: Option<String>,
    pub old_trailer_destination: Option<String>,
    pub target_is_free: bool,
    pub target_player_vehicle_slot_id: Option<String>,
    pub target_player_vehicle_slot_index: Option<usize>,
    pub write_case: Option<String>,
    pub can_write_safely: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TrailerChangePreview {
    pub mode: TrailerSwitchMode,
    pub current_trailer: TrailerInventoryItem,
    pub target_trailer: TrailerInventoryItem,
    pub selected_trailer: TrailerInventoryItem,
    pub warnings: Vec<String>,
    pub error_code: Option<String>,
    pub diagnostics: Option<OwnedTrailerDiagnostics>,
    pub swap_plan: Option<TrailerSwapPreviewDetails>,
    pub expected_file_hash: String,
    pub safe_to_write: bool,
    pub can_apply: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TrailerWriteValidation {
    pub success: bool,
    pub expected_trailer_id: String,
    pub actual_trailer_id: Option<String>,
    pub dangling_references: Vec<String>,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TrailerSwitchList {
    pub save_path: String,
    pub file_hash: String,
    pub active_trailer_id: Option<String>,
    pub trailers: Vec<TrailerInventoryItem>,
    pub diagnostics: OwnedTrailerDiagnostics,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TrailerChangeSession {
    pub save_path: String,
    pub save_hash: String,
    pub current_trailer: TrailerInventoryItem,
    pub owned_trailers: Vec<TrailerInventoryItem>,
    pub diagnostics: Option<OwnedTrailerDiagnostics>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ApplyTrailerChangeResult {
    pub success: bool,
    pub backup_id: Option<String>,
    pub persistent_backup_created: bool,
    pub temporary_rollback_used: bool,
    pub temporary_rollback_cleaned: bool,
    pub previous_trailer_id: String,
    pub active_trailer_id: String,
    pub file_hash_before: String,
    pub file_hash_after: String,
    pub validation: TrailerWriteValidation,
    pub refreshed_session: TrailerChangeSession,
}
