use super::models::ModCategory;
use std::collections::BTreeSet;

pub fn detect_categories(paths: &[String], manifest_categories: &[String], label_hints: &[String]) -> Vec<ModCategory> {
    let mut categories = BTreeSet::new();

    for value in manifest_categories {
        for category in categories_from_text(value) {
            categories.insert(category);
        }
    }

    for value in label_hints {
        for category in categories_from_text(value) {
            categories.insert(category);
        }
    }

    for path in paths {
        for category in categories_from_path(path) {
            categories.insert(category);
        }
    }

    if categories.is_empty() {
        categories.insert(ModCategory::Unknown);
    }

    categories.into_iter().collect()
}

pub fn categories_from_text(value: &str) -> Vec<ModCategory> {
    let normalized = normalize(value);
    let mut categories = BTreeSet::new();

    if contains_any(&normalized, &["truck", "interior", "dashboard"]) {
        categories.insert(ModCategory::Truck);
    }
    if contains_any(&normalized, &["trailer"]) {
        categories.insert(ModCategory::Trailer);
    }
    if contains_any(&normalized, &["map", "road connection", "road_connection", "city addon", "ferry"]) {
        categories.insert(ModCategory::Map);
    }
    if contains_any(&normalized, &["cargo", "freight"]) {
        categories.insert(ModCategory::Cargo);
    }
    if contains_any(&normalized, &["traffic", "ai traffic"]) {
        categories.insert(ModCategory::Traffic);
    }
    if contains_any(&normalized, &["economy", "damage", "fuel", "payout", "police", "fine"]) {
        categories.insert(ModCategory::Economy);
    }
    if contains_any(&normalized, &["sound", "audio", "engine sound"]) {
        categories.insert(ModCategory::Sound);
    }
    if contains_any(&normalized, &["ui", "hud", "advisor", "route advisor", "interface"]) {
        categories.insert(ModCategory::Ui);
    }
    if contains_any(&normalized, &["graphic", "graphics", "weather", "texture", "visual"]) {
        categories.insert(ModCategory::Graphics);
    }
    if contains_any(&normalized, &["tuning", "accessory", "parts"]) {
        categories.insert(ModCategory::Tuning);
    }
    if contains_any(&normalized, &["skin", "paint", "livery"]) {
        categories.insert(ModCategory::Skin);
    }

    if categories.is_empty() && !normalized.is_empty() {
        categories.insert(ModCategory::Other);
    }

    categories.into_iter().collect()
}

pub fn categories_from_path(path: &str) -> Vec<ModCategory> {
    let normalized = normalize(path);
    let mut categories = BTreeSet::new();

    if normalized.contains("/def/vehicle/truck") || normalized.contains("/vehicle/truck") {
        categories.insert(ModCategory::Truck);
    }
    if normalized.contains("/def/vehicle/trailer") || normalized.contains("/vehicle/trailer") {
        categories.insert(ModCategory::Trailer);
    }
    if normalized.contains("/def/cargo") {
        categories.insert(ModCategory::Cargo);
    }
    if normalized.contains("/map") || normalized.contains("/prefab") {
        categories.insert(ModCategory::Map);
    }
    if normalized.contains("/def/world/traffic") || normalized.contains("/traffic") {
        categories.insert(ModCategory::Traffic);
    }
    if normalized.contains("/def/economy") {
        categories.insert(ModCategory::Economy);
    }
    if normalized.contains("/sound") || normalized.ends_with(".ogg") || normalized.ends_with(".bank") {
        categories.insert(ModCategory::Sound);
    }
    if normalized.contains("/ui") || normalized.ends_with(".sui") {
        categories.insert(ModCategory::Ui);
    }
    if normalized.contains("/material/ui")
        || normalized.ends_with(".dds")
        || normalized.ends_with(".tobj")
        || normalized.ends_with(".mat")
    {
        categories.insert(ModCategory::Graphics);
    }
    if normalized.contains("/def/vehicle/truck/accessory")
        || normalized.contains("/def/vehicle/trailer/accessory")
        || normalized.contains("/accessory")
    {
        categories.insert(ModCategory::Tuning);
    }
    if normalized.contains("/def/vehicle/truck/paint_job")
        || normalized.contains("/def/vehicle/trailer/paint_job")
        || normalized.contains("/skin")
    {
        categories.insert(ModCategory::Skin);
    }

    if categories.is_empty() {
        categories.insert(ModCategory::Unknown);
    }

    categories.into_iter().collect()
}

fn contains_any(value: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| value.contains(needle))
}

fn normalize(value: &str) -> String {
    value
        .trim()
        .replace('\\', "/")
        .to_ascii_lowercase()
}
