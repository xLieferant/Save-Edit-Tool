use serde::Serialize;
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use crate::shared::ets2data::models::{
    CompanyRecord, CountryRecord, DatasetFile, DatasetMeta, MapCoords, ManualReviewItem,
};
use crate::shared::ets2data::models::{CityRecord, DEFAULT_PAYMENT_TIER};

pub fn canonical_json<T: Serialize>(value: &T) -> Result<String, String> {
    let value = serde_json::to_value(value).map_err(|error| error.to_string())?;
    let sorted = sort_value(value);
    serde_json::to_string(&sorted).map_err(|error| error.to_string())
}

fn sort_value(value: Value) -> Value {
    match value {
        Value::Object(object) => {
            let mut sorted = Map::new();
            let mut entries: Vec<(String, Value)> = object.into_iter().collect();
            entries.sort_by(|left, right| left.0.cmp(&right.0));
            for (key, value) in entries {
                sorted.insert(key, sort_value(value));
            }
            Value::Object(sorted)
        }
        Value::Array(items) => Value::Array(items.into_iter().map(sort_value).collect()),
        other => other,
    }
}

pub fn sha256_hex_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

pub fn sha256_hex<T: Serialize>(value: &T) -> Result<String, String> {
    Ok(sha256_hex_bytes(canonical_json(value)?.as_bytes()))
}

pub fn checksum_country_record(record: &CountryRecord) -> Result<String, String> {
    let mut value = serde_json::to_value(record).map_err(|error| error.to_string())?;
    if let Value::Object(object) = &mut value {
        object.remove("checksum");
    }
    Ok(sha256_hex_bytes(canonical_json(&value)?.as_bytes()))
}

pub fn checksum_city_record(record: &CityRecord) -> Result<String, String> {
    let mut value = serde_json::to_value(record).map_err(|error| error.to_string())?;
    if let Value::Object(object) = &mut value {
        object.remove("checksum");
    }
    Ok(sha256_hex_bytes(canonical_json(&value)?.as_bytes()))
}

pub fn checksum_company_record(record: &CompanyRecord) -> Result<String, String> {
    let mut value = serde_json::to_value(record).map_err(|error| error.to_string())?;
    if let Value::Object(object) = &mut value {
        object.remove("checksum");
    }
    Ok(sha256_hex_bytes(canonical_json(&value)?.as_bytes()))
}

pub fn checksum_dataset<T: Serialize + Clone>(dataset: &DatasetFile<T>) -> Result<String, String> {
    let mut value = serde_json::to_value(dataset).map_err(|error| error.to_string())?;
    if let Value::Object(object) = &mut value {
        if let Some(Value::Object(meta)) = object.get_mut("meta") {
            meta.remove("fileSha256");
        }
    }
    Ok(sha256_hex_bytes(canonical_json(&value)?.as_bytes()))
}

pub fn validate_countries(records: &[CountryRecord]) -> Result<(), String> {
    let mut ids = std::collections::HashSet::new();
    let mut iso2_codes = std::collections::HashSet::new();
    for record in records {
        if record.id.trim().is_empty() {
            return Err("country.id missing".to_string());
        }
        if record.game_token.trim().is_empty() {
            return Err(format!("country.game_token missing for {}", record.id));
        }
        if record.country_iso2.trim().is_empty() {
            return Err(format!("country.country_iso2 missing for {}", record.id));
        }
        if !ids.insert(record.id.clone()) {
            return Err(format!("duplicate country id: {}", record.id));
        }
        iso2_codes.insert(record.country_iso2.clone());
        if record.checksum != checksum_country_record(record)? {
            return Err(format!("country checksum mismatch: {}", record.id));
        }
        validate_coords(record.coords.as_ref(), &record.id)?;
    }
    if iso2_codes.is_empty() {
        return Err("countries dataset is empty".to_string());
    }
    Ok(())
}

pub fn validate_cities(records: &[CityRecord], countries: &[CountryRecord]) -> Result<(), String> {
    let mut ids = std::collections::HashSet::new();
    let valid_country_iso2: std::collections::HashSet<String> = countries
        .iter()
        .map(|country| country.country_iso2.clone())
        .collect();
    for record in records {
        if record.id.trim().is_empty() {
            return Err("city.id missing".to_string());
        }
        if record.name_en.trim().is_empty() {
            return Err(format!("city.name_en missing for {}", record.id));
        }
        if record.country_iso2.trim().is_empty() {
            return Err(format!("city.country_iso2 missing for {}", record.id));
        }
        if !valid_country_iso2.contains(&record.country_iso2) {
            return Err(format!(
                "city.country_iso2 does not exist in countries for {} -> {}",
                record.id, record.country_iso2
            ));
        }
        if !ids.insert(record.id.clone()) {
            return Err(format!("duplicate city id: {}", record.id));
        }
        if record.checksum != checksum_city_record(record)? {
            return Err(format!("city checksum mismatch: {}", record.id));
        }
        validate_coords(record.coords.as_ref(), &record.id)?;
    }
    Ok(())
}

pub fn validate_companies(records: &[CompanyRecord]) -> Result<(), String> {
    let mut ids = std::collections::HashSet::new();
    for record in records {
        if record.id.trim().is_empty() {
            return Err("company.id missing".to_string());
        }
        if record.game_token.trim().is_empty() {
            return Err(format!("company.game_token missing for {}", record.id));
        }
        if !ids.insert(record.id.clone()) {
            return Err(format!("duplicate company id: {}", record.id));
        }
        if !matches!(
            record.payment_tier.as_str(),
            "budget" | "standard" | "good" | "premium" | "elite"
        ) {
            return Err(format!(
                "invalid payment_tier for {}: {}",
                record.id, record.payment_tier
            ));
        }
        if record.checksum != checksum_company_record(record)? {
            return Err(format!("company checksum mismatch: {}", record.id));
        }
    }
    Ok(())
}

pub fn finalize_dataset_meta<T: Serialize + Clone>(
    dataset_version: &str,
    generated_at_utc: &str,
    inputs: Vec<crate::shared::ets2data::models::DatasetInput>,
    warnings: Vec<String>,
    manual_review: Vec<ManualReviewItem>,
    records: &[T],
) -> Result<DatasetMeta, String> {
    let mut meta = DatasetMeta {
        dataset_version: dataset_version.to_string(),
        generated_at_utc: generated_at_utc.to_string(),
        inputs,
        file_sha256: String::new(),
        warnings,
        manual_review,
        record_count: records.len(),
    };
    let dataset = DatasetFile {
        meta: meta.clone(),
        records: records.to_vec(),
    };
    meta.file_sha256 = checksum_dataset(&dataset)?;
    Ok(meta)
}

pub fn default_payment_tier_if_empty(value: &str) -> String {
    if value.trim().is_empty() {
        DEFAULT_PAYMENT_TIER.to_string()
    } else {
        value.to_string()
    }
}

fn validate_coords(coords: Option<&MapCoords>, record_id: &str) -> Result<(), String> {
    let Some(coords) = coords else {
        return Ok(());
    };

    if coords.map_x.abs() > 100_000.0 || coords.map_y.abs() > 100_000.0 {
        return Err(format!("coords out of range for {}", record_id));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::shared::ets2data::models::{
        CompanyRecord, CountryRecord, MapCoords, DEFAULT_PAYMENT_MULTIPLIER,
    };
    use crate::shared::ets2data::models::{CityRecord, DEFAULT_COUNTRY_PAYMENT_MULTIPLIER};

    use super::{
        checksum_city_record, checksum_country_record, sha256_hex_bytes, validate_cities,
        validate_companies, validate_countries,
    };

    fn sample_country() -> CountryRecord {
        let mut record = CountryRecord {
            id: "promods:morocco".to_string(),
            namespace: "promods".to_string(),
            game_token: "morocco".to_string(),
            country_code: Some("MA".to_string()),
            iso_country_code: Some("mar".to_string()),
            country_iso2: "MA".to_string(),
            name_en: "Morocco".to_string(),
            name_local: "Morocco".to_string(),
            aliases: vec![],
            coords: Some(MapCoords {
                map_x: 10.0,
                map_y: 20.0,
            }),
            payment_multiplier: DEFAULT_COUNTRY_PAYMENT_MULTIPLIER,
            notes: vec![],
            source: "local:test".to_string(),
            source_version: "unknown".to_string(),
            checksum: String::new(),
            warnings: vec![],
        };
        record.checksum = checksum_country_record(&record).unwrap();
        record
    }

    fn sample_city() -> CityRecord {
        let mut record = CityRecord {
            id: "promods:larache".to_string(),
            namespace: "promods".to_string(),
            game_token: "larache".to_string(),
            country_id: "promods:morocco".to_string(),
            country_iso2: "MA".to_string(),
            name_en: "Larache".to_string(),
            name_local: "Larache".to_string(),
            aliases: vec![],
            population: None,
            coords: None,
            replaces_city_id: None,
            source: "local:test".to_string(),
            source_version: "unknown".to_string(),
            checksum: String::new(),
            warnings: vec![],
        };
        record.checksum = checksum_city_record(&record).unwrap();
        record
    }

    fn sample_company() -> CompanyRecord {
        let mut record = CompanyRecord {
            id: "promods:adm".to_string(),
            namespace: "promods".to_string(),
            game_token: "adm".to_string(),
            name_en: "Adm".to_string(),
            name_local: "Adm".to_string(),
            aliases: vec![],
            payment_tier: "standard".to_string(),
            payment_multiplier: DEFAULT_PAYMENT_MULTIPLIER,
            preferred_cargo_types: vec![],
            offices: vec![],
            notes: vec![],
            source: "local:test".to_string(),
            source_version: "unknown".to_string(),
            checksum: String::new(),
            warnings: vec![],
        };
        record.checksum = super::checksum_company_record(&record).unwrap();
        record
    }

    #[test]
    fn checksum_roundtrip() {
        let first = sha256_hex_bytes(br#"{"id":1}"#);
        let second = sha256_hex_bytes(br#"{"id":1}"#);
        assert_eq!(first, second);
    }

    #[test]
    fn validation_required_fields() {
        let country = sample_country();
        let city = sample_city();
        let company = sample_company();

        validate_countries(&[country.clone()]).unwrap();
        validate_cities(&[city.clone()], &[country]).unwrap();
        validate_companies(&[company]).unwrap();

        let mut broken = city;
        broken.country_iso2.clear();
        assert!(validate_cities(&[broken], &[sample_country()]).is_err());
    }
}

