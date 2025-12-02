use serde::Serialize;

 #[derive(Serialize)]
 pub struct GameDataQuicksave {
     pub adr: Option<i64>,
     pub long_dist: Option<i64>,
     pub heavy: Option<i64>,
     pub fragile: Option<i64>,
     pub urgent: Option<i64>,
     pub mechanical: Option<i64>
 }