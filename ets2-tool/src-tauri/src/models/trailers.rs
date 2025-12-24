use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct ParsedTrailer {
    pub trailer_id: String,
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
}
