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
    pub display_license_plate: Option<String>,

    // Definition Data
    pub gross_trailer_weight_limit: f32,
    pub chassis_mass: f32,
    pub body_mass: f32,
    pub body_type: Option<String>,
    pub chain_type: Option<String>,
    pub length: f32,
}

#[derive(Debug, Clone, Serialize)]
pub struct PlayerTrailerResult {
    pub success: bool,
    pub has_trailer: bool,
    pub has_active_job: bool,
    pub active_job_cargo_mass: Option<f32>,
    pub trailer: Option<ParsedTrailer>,
    pub message: String,
}

impl PlayerTrailerResult {
    pub fn some(trailer: ParsedTrailer) -> Self {
        Self {
            success: true,
            has_trailer: true,
            has_active_job: false,
            active_job_cargo_mass: None,
            trailer: Some(trailer),
            message: "Player trailer found.".to_string(),
        }
    }

    pub fn none() -> Self {
        Self {
            success: true,
            has_trailer: false,
            has_active_job: false,
            active_job_cargo_mass: None,
            trailer: None,
            message: "No player trailer found in this save.".to_string(),
        }
    }

    pub fn with_active_job(mut self, has_active_job: bool) -> Self {
        self.has_active_job = has_active_job;
        self
    }

    pub fn with_active_job_cargo_mass(mut self, active_job_cargo_mass: Option<f32>) -> Self {
        self.active_job_cargo_mass = active_job_cargo_mass;
        self
    }
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
