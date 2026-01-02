use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct ParsedTrailer {
    pub trailer_id: String,
    pub trailer_definition: String, // _nameless...
    pub cargo_mass: f32,
    pub cargo_damage: f32,

    pub body_wear: f32,
    pub body_wear_unfixable: f32,
    pub chassis_wear: f32,
    pub chassis_wear_unfixable: f32,

    pub wheels_wear: Vec<f32>,
    pub wheels_wear_unfixable: Vec<f32>,

    pub odometer: f32,
    pub integrity_odometer: f32,

    pub accessories: Vec<String>,
    pub license_plate: Option<String>,
    
    // Definition Data
    pub gross_trailer_weight_limit: f32,
    pub chassis_mass: f32,
    pub body_mass: f32,
    pub body_type: Option<String>,
    pub chain_type: Option<String>,
    pub length: f32,
}

/// Typ für Trailer-Daten (Zwischenformat beim Parsen)
#[derive(Debug, Clone)]
pub struct TrailerData {
    pub trailer_id: String,
    pub trailer_definition: String,
    pub brand: Option<String>,
    pub model: Option<String>,
    pub license_plate: Option<String>,
    pub odometer: f32,
    pub odometer_float: Option<f32>,
    pub wear_float: Option<f32>,
    pub wheels_float: Option<Vec<f32>>,
    pub assigned_garage: Option<String>,
    
    // Raw fields for merging
    pub cargo_mass: f32,
    pub cargo_damage: f32,
    pub body_wear_unfixable: f32,
    pub chassis_wear: f32,
    pub chassis_wear_unfixable: f32,
    pub wheels_wear_unfixable: Vec<f32>,
    pub integrity_odometer: f32,
    pub accessories: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct TrailerDefData {
    pub id: String,
    pub gross_trailer_weight_limit: f32,
    pub chassis_mass: f32,
    pub body_mass: f32,
    pub length: f32,
    pub body_type: Option<String>,
    pub chain_type: Option<String>,
    pub source_name: Option<String>,
}
