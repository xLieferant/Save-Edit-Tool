use serde::Serialize;

#[derive(Serialize)]
pub struct GameDataQuicksave {
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
}
