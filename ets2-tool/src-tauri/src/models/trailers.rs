use serde::Serialize;

#[derive(Debug, Serialize, Clone)]
pub struct ParsedTrailer {
    pub trailer_id: String,
    pub brand: Option<String>,
    pub model: Option<String>,
    pub odometer: Option<i64>,
    pub odometer_float: Option<f32>,
    pub wear_float: Option<f32>,
    pub wheels_float: Option<f32>,
    pub license_plate: Option<String>,
    pub assigned_garage: Option<String>,
}
