use serde::Serialize;

#[derive(Debug, Serialize, Clone)]
pub struct ParsedTruck {
    pub truck_id: String,       // z. B. "_nameless.1234.abcd"
    pub brand: String,          // z. B. "volvo"
    pub model: String,          // z. B. "fh16_2012"
    pub odometer: Option<i64>,       // in i64, passend zu GameDataQuicksave
    pub mileage: Option<f32>,        // optional
    pub trip_fuel_l: Option<i64>,    // hinzugefügt
    pub license_plate: Option<String>, // hinzugefügt
    pub assigned_garage: Option<String>,
}
