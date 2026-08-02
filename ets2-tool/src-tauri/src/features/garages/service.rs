use super::models::GarageActionResult;

fn placeholder(action: &str) -> GarageActionResult {
    GarageActionResult {
        action: action.to_string(),
        implemented: false,
    }
}

pub fn buy_garage() -> GarageActionResult {
    placeholder("buy_garage")
}

pub fn upgrade_garage() -> GarageActionResult {
    placeholder("upgrade_garage")
}

pub fn buy_all_garages() -> GarageActionResult {
    placeholder("buy_all_garages")
}

pub fn relinquish_garage_ownership() -> GarageActionResult {
    placeholder("relinquish_garage_ownership")
}
