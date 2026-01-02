use serde::Serialize;

#[derive(Debug, Serialize, Clone)]
pub struct ParsedTruck {
    pub truck_id: String,              // z. B. "_nameless.1234.abcd"
    pub brand: String,                 // z. B. "volvo"
    pub model: String,                 // z. B. "fh16_2012"
    
    pub odometer: f32,
    pub integrity_odometer: f32,
    
    pub fuel_relative: f32,
    pub trip_fuel_l: f32,
    pub trip_distance_km: f32,
    pub trip_time_min: f32,

    pub engine_wear: f32,
    pub transmission_wear: f32,
    pub cabin_wear: f32,
    pub chassis_wear: f32,
    pub wheels_wear: Vec<f32>,

    pub engine_wear_unfixable: f32,
    pub transmission_wear_unfixable: f32,
    pub cabin_wear_unfixable: f32,
    pub chassis_wear_unfixable: f32,
    pub wheels_wear_unfixable: Vec<f32>,

    pub license_plate: Option<String>,
    pub assigned_garage: Option<String>,
}
