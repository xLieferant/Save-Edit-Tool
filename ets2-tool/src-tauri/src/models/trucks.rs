use serde::Serialize;

#[derive(Debug, Serialize, Clone)]
pub struct ParsedTruck {
    pub truck_id: String,       // z. B. "_nameless.1234.abcd"
    pub brand: String,          // z. B. "volvo"
    pub model: String,          // z. B. "fh16_2012"
    pub odometer: Option<f32>,  // km-Stand
    pub mileage: Option<f32>,   // alternative Werte
    pub assigned_garage: Option<String>,
}
