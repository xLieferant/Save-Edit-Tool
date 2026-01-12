use serde::Serialize;

#[derive(Serialize, Clone)]
pub struct GameDataQuicksave {
    pub player_id: Option<String>,
    pub bank_id: Option<String>,
    pub player_xp: Option<i64>,
    pub player_my_truck: Option<String>,
    pub player_my_trailer: Option<String>,
    pub adr: Option<i64>,
    pub long_dist: Option<i64>,
    pub heavy: Option<i64>,
    pub fragile: Option<i64>,
    pub urgent: Option<i64>,
    pub mechanical: Option<i64>,

    pub vehicle_id: Option<String>,
    pub brand_path: Option<String>,
    pub license_plate: Option<String>,
    pub odometer: Option<i64>,
    pub trip_fuel_l: Option<i64>,
    pub truck_brand: Option<String>,
    pub truck_model: Option<String>,

    // Trailer-Felder
    pub trailer_brand: Option<String>,
    pub trailer_model: Option<String>,
    pub trailer_license_plate: Option<String>,
    pub trailer_odometer: Option<Vec<f32>>,
    pub trailer_odometer_float: Option<Vec<f32>>,
    pub trailer_wear_float: Option<Vec<f32>>,
    pub trailer_wheels_float: Option<Vec<String>>,
}
