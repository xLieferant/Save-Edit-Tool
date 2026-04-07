use serde::{Deserialize, Serialize};

pub const DATASET_VERSION: &str = "1.0.0";
pub const DEFAULT_PAYMENT_TIER: &str = "standard";
pub const DEFAULT_PAYMENT_MULTIPLIER: f64 = 1.0;
pub const DEFAULT_COUNTRY_PAYMENT_MULTIPLIER: f64 = 1.0;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DatasetInput {
    pub kind: String,
    pub path: String,
    pub sha256: String,
    pub available: bool,
    pub source_version: String,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ManualReviewItem {
    pub left_id: String,
    pub right_id: String,
    pub similarity: f64,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DatasetMeta {
    pub dataset_version: String,
    pub generated_at_utc: String,
    pub inputs: Vec<DatasetInput>,
    pub file_sha256: String,
    pub warnings: Vec<String>,
    pub manual_review: Vec<ManualReviewItem>,
    pub record_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DatasetFile<T> {
    pub meta: DatasetMeta,
    pub records: Vec<T>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MapCoords {
    pub map_x: f64,
    pub map_y: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CountryRecord {
    pub id: String,
    pub namespace: String,
    pub game_token: String,
    pub country_code: Option<String>,
    pub iso_country_code: Option<String>,
    pub country_iso2: String,
    pub name_en: String,
    pub name_local: String,
    pub aliases: Vec<String>,
    pub coords: Option<MapCoords>,
    pub payment_multiplier: f64,
    pub notes: Vec<String>,
    pub source: String,
    pub source_version: String,
    pub checksum: String,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CityRecord {
    pub id: String,
    pub namespace: String,
    pub game_token: String,
    pub country_id: String,
    pub country_iso2: String,
    pub name_en: String,
    pub name_local: String,
    pub aliases: Vec<String>,
    pub population: Option<i64>,
    pub coords: Option<MapCoords>,
    pub replaces_city_id: Option<String>,
    pub source: String,
    pub source_version: String,
    pub checksum: String,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CompanyOfficeRecord {
    pub id: String,
    pub city_id: Option<String>,
    pub city_game_token: String,
    pub prefab_token: Option<String>,
    pub source: String,
    pub source_version: String,
    pub checksum: String,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CompanyRecord {
    pub id: String,
    pub namespace: String,
    pub game_token: String,
    pub name_en: String,
    pub name_local: String,
    pub aliases: Vec<String>,
    pub payment_tier: String,
    pub payment_multiplier: f64,
    pub preferred_cargo_types: Vec<String>,
    pub offices: Vec<CompanyOfficeRecord>,
    pub notes: Vec<String>,
    pub source: String,
    pub source_version: String,
    pub checksum: String,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CountryOverride {
    pub payment_multiplier: Option<f64>,
    pub notes: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CompanyOverride {
    pub payment_tier: Option<String>,
    pub payment_multiplier: Option<f64>,
    pub notes: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DatasetBuildSummary {
    pub dataset_version: String,
    pub generated_at_utc: String,
    pub country_count: usize,
    pub city_count: usize,
    pub company_count: usize,
    pub office_count: usize,
    pub warnings: Vec<String>,
    pub countries_checksum: String,
    pub cities_checksum: String,
    pub companies_checksum: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Ets2DataImportSummary {
    pub dataset_version: String,
    pub country_count: usize,
    pub city_count: usize,
    pub company_count: usize,
    pub office_count: usize,
    pub warnings: Vec<String>,
    pub countries_checksum: String,
    pub cities_checksum: String,
    pub companies_checksum: String,
    pub force: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct CityQueryFilter {
    pub search: Option<String>,
    pub country_iso2: Option<String>,
    pub namespace: Option<String>,
    pub limit: Option<usize>,
}
