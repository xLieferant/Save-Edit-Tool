use std::fs;
use std::path::Path;

use chrono::Utc;
use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::events::{EVT_DATA_IMPORT_DONE, EVT_DATA_IMPORT_ERROR, EVT_DATA_IMPORT_PROGRESS};
use crate::shared::ets2data::fuzzy::{FuzzyDisposition, fuzzy_disposition, levenshtein_similarity};
use crate::shared::ets2data::models::{
    CityQueryFilter, CityRecord, CompanyOfficeRecord, CompanyRecord, CountryRecord, DatasetFile,
    Ets2DataImportSummary,
};
use crate::shared::ets2data::validate::{validate_cities, validate_companies, validate_countries};

const DATASET_MIGRATION_SQL: &str =
    include_str!("../../db/migrations/2026-04-06_create_ets2_datasets.sql");
const EMBEDDED_COUNTRIES_DATASET_JSON: &str =
    include_str!("../../../../data/ets2/countries.json");
const EMBEDDED_CITIES_DATASET_JSON: &str =
    include_str!("../../../../data/ets2/cities.json");
const EMBEDDED_COMPANIES_DATASET_JSON: &str =
    include_str!("../../../../data/ets2/companies.json");

pub fn ensure_tables(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(DATASET_MIGRATION_SQL)
        .map_err(|error| error.to_string())
}

pub fn import_datasets(
    app: Option<&AppHandle>,
    conn: &mut Connection,
    repo_root: &Path,
    force: bool,
) -> Result<Ets2DataImportSummary, String> {
    ensure_tables(conn)?;

    emit_progress(app, "load_countries", 1, 6);
    let countries: DatasetFile<CountryRecord> = load_dataset_with_fallback(
        &repo_root.join("data/ets2/countries.json"),
        "<embedded>/data/ets2/countries.json",
        EMBEDDED_COUNTRIES_DATASET_JSON,
    )?;
    emit_progress(app, "load_cities", 2, 6);
    let cities: DatasetFile<CityRecord> = load_dataset_with_fallback(
        &repo_root.join("data/ets2/cities.json"),
        "<embedded>/data/ets2/cities.json",
        EMBEDDED_CITIES_DATASET_JSON,
    )?;
    emit_progress(app, "load_companies", 3, 6);
    let companies: DatasetFile<CompanyRecord> = load_dataset_with_fallback(
        &repo_root.join("data/ets2/companies.json"),
        "<embedded>/data/ets2/companies.json",
        EMBEDDED_COMPANIES_DATASET_JSON,
    )?;

    validate_countries(&countries.records)?;
    validate_cities(&cities.records, &countries.records)?;
    validate_companies(&companies.records)?;

    let tx = conn.transaction().map_err(|error| error.to_string())?;

    emit_progress(app, "import_countries", 4, 6);
    for record in &countries.records {
        upsert_country(&tx, record, &countries.meta.dataset_version, force)?;
    }

    emit_progress(app, "import_cities", 5, 6);
    for record in &cities.records {
        upsert_city(&tx, record, &cities.meta.dataset_version, force)?;
    }

    emit_progress(app, "import_companies", 6, 6);
    let mut office_count = 0usize;
    for record in &companies.records {
        upsert_company(&tx, record, &companies.meta.dataset_version, force)?;
        for office in &record.offices {
            office_count += 1;
            upsert_company_office(
                &tx,
                &record.id,
                office,
                &companies.meta.dataset_version,
                force,
            )?;
        }
    }

    tx.commit().map_err(|error| error.to_string())?;

    let summary = Ets2DataImportSummary {
        dataset_version: countries.meta.dataset_version.clone(),
        country_count: countries.records.len(),
        city_count: cities.records.len(),
        company_count: companies.records.len(),
        office_count,
        warnings: [
            countries.meta.warnings.clone(),
            cities.meta.warnings.clone(),
            companies.meta.warnings.clone(),
        ]
        .concat(),
        countries_checksum: countries.meta.file_sha256.clone(),
        cities_checksum: cities.meta.file_sha256.clone(),
        companies_checksum: companies.meta.file_sha256.clone(),
        force,
    };

    if let Some(app) = app {
        let _ = app.emit(EVT_DATA_IMPORT_DONE, &summary);
    }

    Ok(summary)
}

pub fn import_datasets_with_error_event(
    app: &AppHandle,
    conn: &mut Connection,
    repo_root: &Path,
    force: bool,
) -> Result<Ets2DataImportSummary, String> {
    match import_datasets(Some(app), conn, repo_root, force) {
        Ok(summary) => Ok(summary),
        Err(error) => {
            let _ = app.emit(
                EVT_DATA_IMPORT_ERROR,
                serde_json::json!({
                    "errorCode": "data_import_failed",
                    "message": error,
                }),
            );
            Err(error)
        }
    }
}

pub fn get_city(conn: &Connection, city_id: &str) -> Result<Option<CityRecord>, String> {
    ensure_tables(conn)?;
    conn.query_row(
        r#"
        SELECT
            id, namespace, game_token, country_id, country_iso2, name_en, name_local,
            aliases_json, population, coords_json, replaces_city_id, source, source_version,
            checksum, warnings_json
        FROM ets2_cities
        WHERE id = ?1
        "#,
        [city_id],
        map_city_row,
    )
    .optional()
    .map_err(|error| error.to_string())
}

pub fn get_company(conn: &Connection, company_id: &str) -> Result<Option<CompanyRecord>, String> {
    ensure_tables(conn)?;
    let company = conn
        .query_row(
            r#"
            SELECT
                id, namespace, game_token, name_en, name_local, aliases_json,
                payment_tier, payment_multiplier, preferred_cargo_types_json,
                notes_json, source, source_version, checksum, warnings_json
            FROM ets2_companies
            WHERE id = ?1
            "#,
            [company_id],
            |row| {
                Ok(CompanyRecord {
                    id: row.get(0)?,
                    namespace: row.get(1)?,
                    game_token: row.get(2)?,
                    name_en: row.get(3)?,
                    name_local: row.get(4)?,
                    aliases: from_json_for_row(row.get::<_, String>(5)?)?,
                    payment_tier: row.get(6)?,
                    payment_multiplier: row.get(7)?,
                    preferred_cargo_types: from_json_for_row(row.get::<_, String>(8)?)?,
                    offices: Vec::new(),
                    notes: from_json_for_row(row.get::<_, String>(9)?)?,
                    source: row.get(10)?,
                    source_version: row.get(11)?,
                    checksum: row.get(12)?,
                    warnings: from_json_for_row(row.get::<_, String>(13)?)?,
                })
            },
        )
        .optional()
        .map_err(|error| error.to_string())?;

    let Some(mut company) = company else {
        return Ok(None);
    };

    let mut statement = conn
        .prepare(
            r#"
            SELECT id, city_id, city_game_token, prefab_token, source, source_version, checksum, warnings_json
            FROM ets2_company_offices
            WHERE company_id = ?1
            ORDER BY city_game_token ASC, id ASC
            "#,
        )
        .map_err(|error| error.to_string())?;
    let offices = statement
        .query_map([company_id], |row| {
            Ok(CompanyOfficeRecord {
                id: row.get(0)?,
                city_id: row.get(1)?,
                city_game_token: row.get(2)?,
                prefab_token: row.get(3)?,
                source: row.get(4)?,
                source_version: row.get(5)?,
                checksum: row.get(6)?,
                warnings: from_json_for_row(row.get::<_, String>(7)?)?,
            })
        })
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    company.offices = offices;

    Ok(Some(company))
}

pub fn list_cities(
    conn: &Connection,
    filter: Option<CityQueryFilter>,
) -> Result<Vec<CityRecord>, String> {
    ensure_tables(conn)?;
    let filter = filter.unwrap_or_default();
    let mut statement = conn
        .prepare(
            r#"
            SELECT
                id, namespace, game_token, country_id, country_iso2, name_en, name_local,
                aliases_json, population, coords_json, replaces_city_id, source, source_version,
                checksum, warnings_json
            FROM ets2_cities
            ORDER BY country_iso2 ASC, name_en ASC, id ASC
            "#,
        )
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([], map_city_row)
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;

    let search = filter
        .search
        .as_ref()
        .map(|value| value.to_ascii_lowercase());
    let mut filtered = rows
        .into_iter()
        .filter(|record| {
            if let Some(country_iso2) = filter.country_iso2.as_deref() {
                if !record.country_iso2.eq_ignore_ascii_case(country_iso2) {
                    return false;
                }
            }
            if let Some(namespace) = filter.namespace.as_deref() {
                if !record.namespace.eq_ignore_ascii_case(namespace) {
                    return false;
                }
            }
            if let Some(search) = search.as_deref() {
                let aliases = record.aliases.join(" ").to_ascii_lowercase();
                let haystack = format!(
                    "{} {} {} {}",
                    record.name_en, record.name_local, record.game_token, aliases
                )
                .to_ascii_lowercase();
                if !haystack.contains(search) {
                    return false;
                }
            }
            true
        })
        .collect::<Vec<_>>();

    if let Some(limit) = filter.limit {
        filtered.truncate(limit);
    }
    Ok(filtered)
}

pub fn find_city_with_fallback(
    conn: &Connection,
    name: &str,
    country_iso2: &str,
) -> Result<Option<(CityRecord, f64, bool)>, String> {
    let cities = list_cities(
        conn,
        Some(CityQueryFilter {
            search: None,
            country_iso2: Some(country_iso2.to_string()),
            namespace: None,
            limit: None,
        }),
    )?;
    let needle = name.trim();
    let mut best: Option<(CityRecord, f64, bool)> = None;

    for city in cities {
        let candidates = std::iter::once(city.name_en.as_str())
            .chain(std::iter::once(city.name_local.as_str()))
            .chain(city.aliases.iter().map(String::as_str));
        for candidate in candidates {
            let similarity = levenshtein_similarity(candidate, needle);
            match fuzzy_disposition(similarity) {
                FuzzyDisposition::Merge | FuzzyDisposition::AutoMerge => {
                    let replace = match best.as_ref() {
                        Some((_, current_similarity, _)) => similarity > *current_similarity,
                        None => true,
                    };
                    if replace {
                        best = Some((city.clone(), similarity, true));
                    }
                }
                FuzzyDisposition::ManualReview => {
                    let replace = match best.as_ref() {
                        Some((_, current_similarity, _)) => similarity > *current_similarity,
                        None => true,
                    };
                    if replace {
                        best = Some((city.clone(), similarity, false));
                    }
                }
                FuzzyDisposition::KeepSeparate => {}
            }
        }
    }

    Ok(best)
}

fn load_dataset_with_fallback<T: for<'de> serde::Deserialize<'de>>(
    path: &Path,
    fallback_label: &str,
    fallback_content: &str,
) -> Result<DatasetFile<T>, String> {
    match fs::read_to_string(path) {
        Ok(content) => parse_dataset_content(&content, &path.display().to_string()),
        Err(error) => {
            crate::dev_log!(
                "[ets2data] dataset read failed for {}: {}. Falling back to embedded dataset.",
                path.display(),
                error
            );
            parse_dataset_content(fallback_content, fallback_label)
        }
    }
}

fn parse_dataset_content<T: for<'de> serde::Deserialize<'de>>(
    content: &str,
    source: &str,
) -> Result<DatasetFile<T>, String> {
    let normalized = content.trim_start_matches('\u{feff}');
    serde_json::from_str(normalized)
        .map_err(|error| format!("failed to parse {}: {}", source, error))
}

fn upsert_country(
    conn: &Connection,
    record: &CountryRecord,
    dataset_version: &str,
    _force: bool,
) -> Result<(), String> {
    conn.execute(
        r#"
        INSERT INTO ets2_countries (
            id, namespace, game_token, country_code, iso_country_code, country_iso2,
            name_en, name_local, aliases_json, coords_json, payment_multiplier, notes_json,
            source, source_version, checksum, warnings_json, dataset_version, imported_at_utc
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)
        ON CONFLICT(id) DO UPDATE SET
            namespace = excluded.namespace,
            game_token = excluded.game_token,
            country_code = excluded.country_code,
            iso_country_code = excluded.iso_country_code,
            country_iso2 = excluded.country_iso2,
            name_en = excluded.name_en,
            name_local = excluded.name_local,
            aliases_json = excluded.aliases_json,
            coords_json = excluded.coords_json,
            payment_multiplier = excluded.payment_multiplier,
            notes_json = excluded.notes_json,
            source = excluded.source,
            source_version = excluded.source_version,
            checksum = excluded.checksum,
            warnings_json = excluded.warnings_json,
            dataset_version = excluded.dataset_version,
            imported_at_utc = excluded.imported_at_utc
        "#,
        params![
            record.id,
            record.namespace,
            record.game_token,
            record.country_code,
            record.iso_country_code,
            record.country_iso2,
            record.name_en,
            record.name_local,
            to_json_column(&record.aliases)?,
            to_json_column(&record.coords)?,
            record.payment_multiplier,
            to_json_column(&record.notes)?,
            record.source,
            record.source_version,
            record.checksum,
            to_json_column(&record.warnings)?,
            dataset_version,
            Utc::now().to_rfc3339(),
        ],
    )
    .map(|_| ())
    .map_err(|error| error.to_string())
}

fn upsert_city(
    conn: &Connection,
    record: &CityRecord,
    dataset_version: &str,
    _force: bool,
) -> Result<(), String> {
    conn.execute(
        r#"
        INSERT INTO ets2_cities (
            id, namespace, game_token, country_id, country_iso2, name_en, name_local,
            aliases_json, population, coords_json, replaces_city_id, source, source_version,
            checksum, warnings_json, dataset_version, imported_at_utc
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)
        ON CONFLICT(id) DO UPDATE SET
            namespace = excluded.namespace,
            game_token = excluded.game_token,
            country_id = excluded.country_id,
            country_iso2 = excluded.country_iso2,
            name_en = excluded.name_en,
            name_local = excluded.name_local,
            aliases_json = excluded.aliases_json,
            population = excluded.population,
            coords_json = excluded.coords_json,
            replaces_city_id = excluded.replaces_city_id,
            source = excluded.source,
            source_version = excluded.source_version,
            checksum = excluded.checksum,
            warnings_json = excluded.warnings_json,
            dataset_version = excluded.dataset_version,
            imported_at_utc = excluded.imported_at_utc
        "#,
        params![
            record.id,
            record.namespace,
            record.game_token,
            record.country_id,
            record.country_iso2,
            record.name_en,
            record.name_local,
            to_json_column(&record.aliases)?,
            record.population,
            to_json_column(&record.coords)?,
            record.replaces_city_id,
            record.source,
            record.source_version,
            record.checksum,
            to_json_column(&record.warnings)?,
            dataset_version,
            Utc::now().to_rfc3339(),
        ],
    )
    .map(|_| ())
    .map_err(|error| error.to_string())
}

fn upsert_company(
    conn: &Connection,
    record: &CompanyRecord,
    dataset_version: &str,
    _force: bool,
) -> Result<(), String> {
    conn.execute(
        r#"
        INSERT INTO ets2_companies (
            id, namespace, game_token, name_en, name_local, aliases_json,
            payment_tier, payment_multiplier, preferred_cargo_types_json, notes_json,
            source, source_version, checksum, warnings_json, dataset_version, imported_at_utc
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)
        ON CONFLICT(id) DO UPDATE SET
            namespace = excluded.namespace,
            game_token = excluded.game_token,
            name_en = excluded.name_en,
            name_local = excluded.name_local,
            aliases_json = excluded.aliases_json,
            payment_tier = excluded.payment_tier,
            payment_multiplier = excluded.payment_multiplier,
            preferred_cargo_types_json = excluded.preferred_cargo_types_json,
            notes_json = excluded.notes_json,
            source = excluded.source,
            source_version = excluded.source_version,
            checksum = excluded.checksum,
            warnings_json = excluded.warnings_json,
            dataset_version = excluded.dataset_version,
            imported_at_utc = excluded.imported_at_utc
        "#,
        params![
            record.id,
            record.namespace,
            record.game_token,
            record.name_en,
            record.name_local,
            to_json_column(&record.aliases)?,
            record.payment_tier,
            record.payment_multiplier,
            to_json_column(&record.preferred_cargo_types)?,
            to_json_column(&record.notes)?,
            record.source,
            record.source_version,
            record.checksum,
            to_json_column(&record.warnings)?,
            dataset_version,
            Utc::now().to_rfc3339(),
        ],
    )
    .map(|_| ())
    .map_err(|error| error.to_string())
}

fn upsert_company_office(
    conn: &Connection,
    company_id: &str,
    record: &CompanyOfficeRecord,
    dataset_version: &str,
    _force: bool,
) -> Result<(), String> {
    conn.execute(
        r#"
        INSERT INTO ets2_company_offices (
            id, company_id, city_id, city_game_token, prefab_token, source,
            source_version, checksum, warnings_json, dataset_version, imported_at_utc
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
        ON CONFLICT(id) DO UPDATE SET
            company_id = excluded.company_id,
            city_id = excluded.city_id,
            city_game_token = excluded.city_game_token,
            prefab_token = excluded.prefab_token,
            source = excluded.source,
            source_version = excluded.source_version,
            checksum = excluded.checksum,
            warnings_json = excluded.warnings_json,
            dataset_version = excluded.dataset_version,
            imported_at_utc = excluded.imported_at_utc
        "#,
        params![
            record.id,
            company_id,
            record.city_id,
            record.city_game_token,
            record.prefab_token,
            record.source,
            record.source_version,
            record.checksum,
            to_json_column(&record.warnings)?,
            dataset_version,
            Utc::now().to_rfc3339(),
        ],
    )
    .map(|_| ())
    .map_err(|error| error.to_string())
}

fn map_city_row(row: &rusqlite::Row<'_>) -> Result<CityRecord, rusqlite::Error> {
    Ok(CityRecord {
        id: row.get(0)?,
        namespace: row.get(1)?,
        game_token: row.get(2)?,
        country_id: row.get(3)?,
        country_iso2: row.get(4)?,
        name_en: row.get(5)?,
        name_local: row.get(6)?,
        aliases: from_json_for_row(row.get::<_, String>(7)?)?,
        population: row.get(8)?,
        coords: from_json_for_row(row.get::<_, String>(9)?)?,
        replaces_city_id: row.get(10)?,
        source: row.get(11)?,
        source_version: row.get(12)?,
        checksum: row.get(13)?,
        warnings: from_json_for_row(row.get::<_, String>(14)?)?,
    })
}

fn to_json_column<T: Serialize>(value: &T) -> Result<String, String> {
    serde_json::to_string(value).map_err(|error| error.to_string())
}

fn from_json_for_row<T: serde::de::DeserializeOwned>(value: String) -> Result<T, rusqlite::Error> {
    serde_json::from_str(&value)
        .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))
}

fn emit_progress(app: Option<&AppHandle>, stage: &str, current: usize, total: usize) {
    let Some(app) = app else {
        return;
    };
    let _ = app.emit(
        EVT_DATA_IMPORT_PROGRESS,
        serde_json::json!({
            "stage": stage,
            "current": current,
            "total": total,
        }),
    );
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use crate::shared::ets2data::models::{
        CityRecord, CompanyOfficeRecord, CompanyRecord, CountryRecord,
        DEFAULT_COUNTRY_PAYMENT_MULTIPLIER, DEFAULT_PAYMENT_MULTIPLIER,
    };
    use crate::shared::ets2data::validate::{
        checksum_city_record, checksum_company_record, checksum_country_record,
    };

    use super::{
        ensure_tables, get_city, get_company, list_cities, upsert_city, upsert_company,
        upsert_company_office, upsert_country,
    };

    #[test]
    fn sqlite_import_smoke() {
        let conn = Connection::open_in_memory().unwrap();
        ensure_tables(&conn).unwrap();

        let mut country = CountryRecord {
            id: "promods:morocco".to_string(),
            namespace: "promods".to_string(),
            game_token: "morocco".to_string(),
            country_code: Some("MA".to_string()),
            iso_country_code: Some("mar".to_string()),
            country_iso2: "MA".to_string(),
            name_en: "Morocco".to_string(),
            name_local: "Morocco".to_string(),
            aliases: vec![],
            coords: None,
            payment_multiplier: DEFAULT_COUNTRY_PAYMENT_MULTIPLIER,
            notes: vec![],
            source: "local:test".to_string(),
            source_version: "unknown".to_string(),
            checksum: String::new(),
            warnings: vec![],
        };
        country.checksum = checksum_country_record(&country).unwrap();
        upsert_country(&conn, &country, "1.0.0", true).unwrap();

        let mut city = CityRecord {
            id: "promods:larache".to_string(),
            namespace: "promods".to_string(),
            game_token: "larache".to_string(),
            country_id: country.id.clone(),
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
        city.checksum = checksum_city_record(&city).unwrap();
        upsert_city(&conn, &city, "1.0.0", true).unwrap();

        let office = CompanyOfficeRecord {
            id: "promods:adm:larache:musor".to_string(),
            city_id: Some(city.id.clone()),
            city_game_token: city.game_token.clone(),
            prefab_token: Some("musor".to_string()),
            source: "local:test".to_string(),
            source_version: "unknown".to_string(),
            checksum: "office-checksum".to_string(),
            warnings: vec![],
        };
        let mut company = CompanyRecord {
            id: "promods:adm".to_string(),
            namespace: "promods".to_string(),
            game_token: "adm".to_string(),
            name_en: "Adm".to_string(),
            name_local: "Adm".to_string(),
            aliases: vec![],
            payment_tier: "standard".to_string(),
            payment_multiplier: DEFAULT_PAYMENT_MULTIPLIER,
            preferred_cargo_types: vec!["aircond".to_string()],
            offices: vec![office.clone()],
            notes: vec![],
            source: "local:test".to_string(),
            source_version: "unknown".to_string(),
            checksum: String::new(),
            warnings: vec![],
        };
        company.checksum = checksum_company_record(&company).unwrap();
        upsert_company(&conn, &company, "1.0.0", true).unwrap();
        upsert_company_office(&conn, &company.id, &office, "1.0.0", true).unwrap();

        assert!(get_city(&conn, &city.id).unwrap().is_some());
        assert_eq!(list_cities(&conn, None).unwrap().len(), 1);
        let loaded_company = get_company(&conn, &company.id).unwrap().unwrap();
        assert_eq!(loaded_company.offices.len(), 1);
    }
}
