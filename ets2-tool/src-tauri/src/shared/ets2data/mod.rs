pub mod fuzzy;
pub mod import;
pub mod models;
pub mod validate;

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use chrono::Utc;
use regex::Regex;
use serde::de::DeserializeOwned;
use zip::ZipArchive;

use crate::shared::ets2data::fuzzy::{FuzzyDisposition, fuzzy_disposition, levenshtein_similarity};
use crate::shared::ets2data::models::{
    CityRecord, CompanyOfficeRecord, CompanyOverride, CompanyRecord, CountryOverride,
    CountryRecord, DATASET_VERSION, DEFAULT_COUNTRY_PAYMENT_MULTIPLIER,
    DEFAULT_PAYMENT_MULTIPLIER, DEFAULT_PAYMENT_TIER, DatasetBuildSummary, DatasetFile,
    DatasetInput, ManualReviewItem,
};
use crate::shared::ets2data::validate::{
    checksum_city_record, checksum_company_record, checksum_country_record, finalize_dataset_meta,
    sha256_hex_bytes, validate_cities, validate_companies, validate_countries,
};
use crate::shared::paths::ets2_base_path;

#[derive(Debug, Clone)]
struct SourceInput {
    id: String,
    kind: String,
    namespace: String,
    priority: u16,
    path: PathBuf,
    source_version: String,
    available: bool,
    notes: Vec<String>,
}

#[derive(Debug, Clone)]
struct SiiUnit {
    class_name: String,
    unit_name: String,
    fields: HashMap<String, Vec<String>>,
}

#[derive(Debug, Clone)]
struct CountryDraft {
    record: CountryRecord,
    priority: u16,
}

#[derive(Debug, Clone)]
struct CityDraft {
    record: CityRecord,
    country_token: String,
    priority: u16,
}

#[derive(Debug, Clone)]
struct CompanyDraft {
    record: CompanyRecord,
    priority: u16,
}

pub fn default_repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")))
        .to_path_buf()
}

pub fn build_datasets(repo_root: &Path) -> Result<DatasetBuildSummary, String> {
    let generated_at_utc = Utc::now().to_rfc3339();
    let output_dir = repo_root.join("data/ets2");
    fs::create_dir_all(output_dir.join("overrides")).map_err(|error| error.to_string())?;

    let country_overrides: HashMap<String, CountryOverride> = load_optional_json(
        &output_dir.join("overrides/country_overrides.json"),
        HashMap::new(),
    )?;
    let company_overrides: HashMap<String, CompanyOverride> = load_optional_json(
        &output_dir.join("overrides/company_overrides.json"),
        HashMap::new(),
    )?;

    let sources = discover_sources()?;
    let mut inputs = Vec::new();
    let mut warnings = Vec::new();
    let mut review_items = Vec::new();
    let mut countries: BTreeMap<String, CountryDraft> = BTreeMap::new();
    let mut cities: BTreeMap<String, CityDraft> = BTreeMap::new();
    let mut companies: BTreeMap<String, CompanyDraft> = BTreeMap::new();

    for source in &sources {
        let _ = &source.id;
        let sha256 = if source.path.exists() {
            match fs::read(&source.path) {
                Ok(bytes) => sha256_hex_bytes(&bytes),
                Err(_) => String::new(),
            }
        } else {
            String::new()
        };
        inputs.push(DatasetInput {
            kind: source.kind.clone(),
            path: normalize_path(&source.path),
            sha256,
            available: source.available,
            source_version: source.source_version.clone(),
            notes: source.notes.clone(),
        });

        if !source.available {
            warnings.push(format!(
                "input_unavailable:{}:{}",
                source.namespace,
                normalize_path(&source.path)
            ));
            continue;
        }

        let files = match load_relevant_text_files(source) {
            Ok(files) => files,
            Err(error) => {
                warnings.push(format!(
                    "input_parse_failed:{}:{}",
                    normalize_path(&source.path),
                    error
                ));
                continue;
            }
        };

        let localization = build_localization_map(&files);

        for (relative_path, content) in &files {
            let units = parse_sii_units(content);
            if relative_path.starts_with("def/country/") {
                merge_country_units(
                    source,
                    relative_path,
                    &units,
                    &localization,
                    &country_overrides,
                    &mut countries,
                    &mut warnings,
                    &mut review_items,
                )?;
            } else if relative_path.starts_with("def/city/") {
                merge_city_units(
                    source,
                    relative_path,
                    &units,
                    &localization,
                    &mut cities,
                    &mut warnings,
                    &mut review_items,
                )?;
            } else if relative_path.starts_with("def/company/") {
                merge_company_units(
                    source,
                    relative_path,
                    &units,
                    &localization,
                    &company_overrides,
                    &mut companies,
                    &mut warnings,
                    &mut review_items,
                )?;
            }
        }
    }

    let country_records = finalize_countries(countries, &mut warnings)?;
    let city_records = finalize_cities(cities, &country_records, &mut warnings)?;
    let company_records = finalize_companies(companies, &city_records, &mut warnings)?;

    review_items.extend(collect_city_review_items(&city_records));

    validate_countries(&country_records)?;
    validate_cities(&city_records, &country_records)?;
    validate_companies(&company_records)?;

    let countries_meta = finalize_dataset_meta(
        DATASET_VERSION,
        &generated_at_utc,
        inputs.clone(),
        warnings.clone(),
        review_items.clone(),
        &country_records,
    )?;
    let countries_dataset = DatasetFile {
        meta: countries_meta.clone(),
        records: country_records.clone(),
    };

    let cities_meta = finalize_dataset_meta(
        DATASET_VERSION,
        &generated_at_utc,
        inputs.clone(),
        warnings.clone(),
        review_items.clone(),
        &city_records,
    )?;
    let cities_dataset = DatasetFile {
        meta: cities_meta.clone(),
        records: city_records.clone(),
    };

    let companies_meta = finalize_dataset_meta(
        DATASET_VERSION,
        &generated_at_utc,
        inputs,
        warnings.clone(),
        review_items,
        &company_records,
    )?;
    let companies_dataset = DatasetFile {
        meta: companies_meta.clone(),
        records: company_records.clone(),
    };

    write_dataset(&output_dir.join("countries.json"), &countries_dataset)?;
    write_dataset(&output_dir.join("cities.json"), &cities_dataset)?;
    write_dataset(&output_dir.join("companies.json"), &companies_dataset)?;

    Ok(DatasetBuildSummary {
        dataset_version: DATASET_VERSION.to_string(),
        generated_at_utc,
        country_count: country_records.len(),
        city_count: city_records.len(),
        company_count: company_records.len(),
        office_count: company_records.iter().map(|record| record.offices.len()).sum(),
        warnings,
        countries_checksum: countries_meta.file_sha256,
        cities_checksum: cities_meta.file_sha256,
        companies_checksum: companies_meta.file_sha256,
    })
}

fn discover_sources() -> Result<Vec<SourceInput>, String> {
    let mut sources = Vec::new();

    if let Some(game_dir) = discover_game_dir() {
        sources.push(build_source(
            "base_def",
            "base_archive",
            "scs",
            200,
            game_dir.join("def.scs"),
            Vec::new(),
        ));
        sources.push(build_source(
            "base_locale",
            "base_archive",
            "scs",
            200,
            game_dir.join("locale.scs"),
            Vec::new(),
        ));

        let dlc_sources = fs::read_dir(&game_dir)
            .map_err(|error| error.to_string())?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name()
                    .and_then(|value| value.to_str())
                    .map(is_relevant_map_dlc_archive)
                    .unwrap_or(false)
            })
            .collect::<Vec<_>>();
        for path in dlc_sources {
            let id = canonical_id_component(
                path.file_stem()
                    .and_then(|value| value.to_str())
                    .unwrap_or("dlc"),
            );
            sources.push(build_source(&id, "dlc_archive", "scs", 210, path, Vec::new()));
        }
    } else {
        sources.push(SourceInput {
            id: "missing_game_dir".to_string(),
            kind: "base_archive".to_string(),
            namespace: "scs".to_string(),
            priority: 200,
            path: default_game_dir_hint(),
            source_version: "unknown".to_string(),
            available: false,
            notes: vec!["game_directory_not_found".to_string()],
        });
    }

    let mod_dir = ets2_base_path()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("mod");
    if mod_dir.exists() {
        let mod_paths = fs::read_dir(&mod_dir)
            .map_err(|error| error.to_string())?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name()
                    .and_then(|value| value.to_str())
                    .map(is_relevant_promods_archive)
                    .unwrap_or(false)
            })
            .collect::<Vec<_>>();
        if mod_paths.is_empty() {
            sources.push(SourceInput {
                id: "missing_promods".to_string(),
                kind: "mod_archive".to_string(),
                namespace: "promods".to_string(),
                priority: 300,
                path: mod_dir,
                source_version: "unknown".to_string(),
                available: false,
                notes: vec!["promods_missing".to_string()],
            });
        } else {
            for path in mod_paths {
                let id = canonical_id_component(
                    path.file_stem()
                        .and_then(|value| value.to_str())
                        .unwrap_or("promods"),
                );
                sources.push(build_source(&id, "mod_archive", "promods", 300, path, Vec::new()));
            }
        }
    }

    sources.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(sources)
}

fn build_source(
    id: &str,
    kind: &str,
    namespace: &str,
    priority: u16,
    path: PathBuf,
    notes: Vec<String>,
) -> SourceInput {
    let source_version = detect_source_version(&path);
    let available = path.exists() && archive_is_zip_readable(&path);
    let mut notes = notes;
    if !path.exists() {
        notes.push("missing_path".to_string());
    } else if !available {
        notes.push("archive_read_unsupported".to_string());
    }
    SourceInput {
        id: id.to_string(),
        kind: kind.to_string(),
        namespace: namespace.to_string(),
        priority,
        path,
        source_version,
        available,
        notes,
    }
}

fn discover_game_dir() -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(path) = read_steam_library_game_dir() {
        candidates.push(path);
    }
    candidates.extend([
        PathBuf::from(r"A:\SteamLibrary\steamapps\common\Euro Truck Simulator 2"),
        PathBuf::from(r"C:\Program Files (x86)\Steam\steamapps\common\Euro Truck Simulator 2"),
        PathBuf::from(r"C:\Program Files\Steam\steamapps\common\Euro Truck Simulator 2"),
        PathBuf::from(r"D:\SteamLibrary\steamapps\common\Euro Truck Simulator 2"),
        PathBuf::from(r"F:\SteamLibrary\steamapps\common\Euro Truck Simulator 2"),
        PathBuf::from(r"G:\SteamLibrary\steamapps\common\Euro Truck Simulator 2"),
    ]);
    candidates.into_iter().find(|path| path.exists())
}

fn read_steam_library_game_dir() -> Option<PathBuf> {
    let libraryfolders = [
        PathBuf::from(r"C:\Program Files (x86)\Steam\steamapps\libraryfolders.vdf"),
        PathBuf::from(r"C:\Program Files\Steam\steamapps\libraryfolders.vdf"),
    ];
    let path_re = Regex::new(r#""path"\s+"([^"]+)""#).ok()?;
    for libraryfile in libraryfolders {
        let content = fs::read_to_string(&libraryfile).ok()?;
        for captures in path_re.captures_iter(&content) {
            let steam_library = captures.get(1)?.as_str().replace(r#"\\"#, "\\");
            let candidate = PathBuf::from(steam_library)
                .join("steamapps")
                .join("common")
                .join("Euro Truck Simulator 2");
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }
    None
}

fn default_game_dir_hint() -> PathBuf {
    PathBuf::from(r"steamapps/common/Euro Truck Simulator 2")
}

fn archive_is_zip_readable(path: &Path) -> bool {
    let file = match fs::File::open(path) {
        Ok(file) => file,
        Err(_) => return false,
    };
    ZipArchive::new(file).is_ok()
}

fn detect_source_version(path: &Path) -> String {
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if let Some(captures) = Regex::new(r"v(\d+(?:\.\d+)*)")
        .ok()
        .and_then(|regex| regex.captures(&file_name))
    {
        return captures
            .get(1)
            .map(|value| format!("v{}", value.as_str()))
            .unwrap_or_else(|| "unknown".to_string());
    }
    "unknown".to_string()
}

fn is_relevant_map_dlc_archive(file_name: &str) -> bool {
    let lower = file_name.to_ascii_lowercase();
    matches!(
        lower.as_str(),
        "dlc_east.scs"
            | "dlc_north.scs"
            | "dlc_fr.scs"
            | "dlc_it.scs"
            | "dlc_balt.scs"
            | "dlc_iberia.scs"
            | "dlc_balkan_e.scs"
            | "dlc_balkan_w.scs"
            | "dlc_greece.scs"
    )
}

fn is_relevant_promods_archive(file_name: &str) -> bool {
    let lower = file_name.to_ascii_lowercase();
    lower.starts_with("promods")
        && lower.ends_with(".scs")
        && (lower.contains("def") || lower.contains("defmap"))
}

fn load_relevant_text_files(source: &SourceInput) -> Result<Vec<(String, String)>, String> {
    let file = fs::File::open(&source.path).map_err(|error| error.to_string())?;
    let mut archive = ZipArchive::new(file).map_err(|error| error.to_string())?;
    let mut files = Vec::new();

    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(|error| error.to_string())?;
        if !entry.is_file() {
            continue;
        }
        let name = entry.name().replace('\\', "/");
        if !is_relevant_archive_path(&name) {
            continue;
        }
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).map_err(|error| error.to_string())?;
        let content = String::from_utf8_lossy(&bytes).to_string();
        files.push((name, content));
    }

    files.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(files)
}

fn is_relevant_archive_path(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    ((lower.starts_with("def/country/") || lower.starts_with("def/city/"))
        && (lower.ends_with(".sii") || lower.ends_with(".sui")))
        || (lower.starts_with("def/company/")
            && (lower.ends_with(".sii") || lower.ends_with(".sui")))
        || lower.starts_with("locale/en_gb/")
        || lower.starts_with("locale/de_de/")
        || lower.contains("local_module")
}

fn build_localization_map(files: &[(String, String)]) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for (_, content) in files
        .iter()
        .filter(|(path, _)| path.contains("locale/") || path.contains("local_module"))
    {
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with("//") {
                continue;
            }
            if let Some((key, value)) = parse_localization_line(trimmed) {
                map.insert(key.clone(), value.clone());
                if !key.starts_with("@@") {
                    map.insert(format!("@@{}@@", key), value.clone());
                }
            }
        }
    }
    map
}

fn parse_localization_line(line: &str) -> Option<(String, String)> {
    let quoted_pair = Regex::new(r#"^"([^"]+)"\s+"([^"]+)"$"#).ok()?;
    if let Some(captures) = quoted_pair.captures(line) {
        return Some((captures.get(1)?.as_str().to_string(), captures.get(2)?.as_str().to_string()));
    }
    let colon_pair = Regex::new(r#"^([@A-Za-z0-9_.-]+)\s*:\s*"([^"]+)"$"#).ok()?;
    if let Some(captures) = colon_pair.captures(line) {
        return Some((captures.get(1)?.as_str().to_string(), captures.get(2)?.as_str().to_string()));
    }
    None
}
fn parse_sii_units(content: &str) -> Vec<SiiUnit> {
    let cleaned = strip_comments(content);
    let start_re = Regex::new(r"^([A-Za-z0-9_]+)\s*:\s*([^\{]+)\{\s*$").unwrap();
    let pending_re = Regex::new(r"^([A-Za-z0-9_]+)\s*:\s*(.+)$").unwrap();
    let mut units = Vec::new();
    let mut current_class = String::new();
    let mut current_name = String::new();
    let mut current_fields: HashMap<String, Vec<String>> = HashMap::new();
    let mut in_unit = false;
    let mut brace_depth = 0i32;
    let mut pending_class: Option<String> = None;
    let mut pending_name: Option<String> = None;

    for raw_line in cleaned.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line == "SiiNunit" || line == "{" || (line == "}" && !in_unit) {
            if line == "{" && !in_unit {
                if let (Some(class_name), Some(unit_name)) =
                    (pending_class.take(), pending_name.take())
                {
                    current_class = class_name;
                    current_name = unit_name;
                    current_fields.clear();
                    in_unit = true;
                    brace_depth = 1;
                }
            }
            continue;
        }

        if !in_unit {
            if let Some(captures) = start_re.captures(line) {
                current_class = captures.get(1).unwrap().as_str().trim().to_string();
                current_name = captures.get(2).unwrap().as_str().trim().to_string();
                current_fields.clear();
                in_unit = true;
                brace_depth = 1;
                pending_class = None;
                pending_name = None;
            } else if let Some(captures) = pending_re.captures(line) {
                pending_class = Some(captures.get(1).unwrap().as_str().trim().to_string());
                pending_name = Some(captures.get(2).unwrap().as_str().trim().to_string());
            }
            continue;
        }

        if line.contains('{') {
            brace_depth += line.matches('{').count() as i32;
        }
        if line.contains('}') {
            brace_depth -= line.matches('}').count() as i32;
            if brace_depth <= 0 {
                units.push(SiiUnit {
                    class_name: current_class.clone(),
                    unit_name: current_name.clone(),
                    fields: current_fields.clone(),
                });
                in_unit = false;
                current_class.clear();
                current_name.clear();
                current_fields.clear();
                brace_depth = 0;
                continue;
            }
        }

        if let Some((key, value)) = parse_field_line(line) {
            current_fields.entry(key).or_default().push(value);
        }
    }

    units
}

fn strip_comments(content: &str) -> String {
    let mut result = String::new();
    let mut chars = content.chars().peekable();
    let mut in_block_comment = false;
    let mut in_quote = false;

    while let Some(ch) = chars.next() {
        if in_block_comment {
            if ch == '*' && chars.peek() == Some(&'/') {
                chars.next();
                in_block_comment = false;
            }
            continue;
        }
        if ch == '"' {
            in_quote = !in_quote;
            result.push(ch);
            continue;
        }
        if !in_quote && ch == '/' && chars.peek() == Some(&'*') {
            chars.next();
            in_block_comment = true;
            continue;
        }
        if !in_quote && ch == '/' && chars.peek() == Some(&'/') {
            for next in chars.by_ref() {
                if next == '\n' {
                    result.push('\n');
                    break;
                }
            }
            continue;
        }
        if !in_quote && ch == '#' {
            for next in chars.by_ref() {
                if next == '\n' {
                    result.push('\n');
                    break;
                }
            }
            continue;
        }
        result.push(ch);
    }
    result
}

fn parse_field_line(line: &str) -> Option<(String, String)> {
    let position = line.find(':')?;
    let key = line[..position].trim();
    let value = line[position + 1..].trim();
    if key.is_empty() || value.is_empty() {
        return None;
    }
    let normalized_key = key.split('[').next().unwrap_or(key).trim().to_string();
    Some((normalized_key, unquote(value)))
}

fn unquote(value: &str) -> String {
    let trimmed = value.trim().trim_end_matches(',').trim();
    if trimmed.starts_with('"') && trimmed.ends_with('"') && trimmed.len() >= 2 {
        trimmed[1..trimmed.len() - 1].to_string()
    } else {
        trimmed.to_string()
    }
}

fn merge_country_units(
    source: &SourceInput,
    relative_path: &str,
    units: &[SiiUnit],
    localization: &HashMap<String, String>,
    overrides: &HashMap<String, CountryOverride>,
    countries: &mut BTreeMap<String, CountryDraft>,
    _warnings: &mut Vec<String>,
    review_items: &mut Vec<ManualReviewItem>,
) -> Result<(), String> {
    for unit in units {
        if unit.class_name != "country_data" {
            continue;
        }
        let game_token = unit
            .unit_name
            .split('.')
            .next_back()
            .unwrap_or(unit.unit_name.as_str())
            .trim_matches('.')
            .to_string();
        let id = format!("{}:{}", source.namespace, canonical_id_component(&game_token));
        let country_code = first_field(unit, "country_code");
        let iso_country_code = first_field(unit, "iso_country_code");
        let country_iso2 = derive_iso2(iso_country_code.as_deref(), country_code.as_deref(), &game_token);
        let name_base = first_field(unit, "name").unwrap_or_else(|| title_case(&game_token));
        let name_localized = first_field(unit, "name_localized");
        let name_local = resolve_localized(name_localized.as_deref(), &name_base, localization);
        let name_en = resolve_localized(name_localized.as_deref(), &name_base, localization);
        let override_value = [id.as_str(), game_token.as_str(), country_iso2.as_str()]
            .iter()
            .find_map(|key| overrides.get(*key));
        let payment_multiplier = override_value
            .and_then(|value| value.payment_multiplier)
            .unwrap_or(DEFAULT_COUNTRY_PAYMENT_MULTIPLIER);
        let mut notes = vec!["VTC balancing default country payment multiplier".to_string()];
        if let Some(extra_notes) = override_value.and_then(|value| value.notes.clone()) {
            notes.extend(extra_notes);
        }

        let mut record = CountryRecord {
            id: id.clone(),
            namespace: source.namespace.clone(),
            game_token: game_token.clone(),
            country_code,
            iso_country_code,
            country_iso2,
            name_en,
            name_local,
            aliases: vec![],
            coords: None,
            payment_multiplier,
            notes,
            source: format!("local:{}!{}", normalize_path(&source.path), relative_path),
            source_version: source.source_version.clone(),
            checksum: String::new(),
            warnings: vec![],
        };
        merge_record_name(
            &id,
            relative_path,
            &mut record.name_en,
            &mut record.name_local,
            &mut record.aliases,
            countries.get(&id).map(|draft| &draft.record),
            review_items,
        );

        match countries.get_mut(&id) {
            Some(existing) if existing.priority > source.priority => {
                merge_aliases(&mut existing.record.aliases, &record.aliases);
            }
            Some(existing) => {
                merge_aliases(&mut record.aliases, &existing.record.aliases);
                *existing = CountryDraft { record, priority: source.priority };
            }
            None => {
                countries.insert(id, CountryDraft { record, priority: source.priority });
            }
        }
    }
    Ok(())
}

fn merge_city_units(
    source: &SourceInput,
    relative_path: &str,
    units: &[SiiUnit],
    localization: &HashMap<String, String>,
    cities: &mut BTreeMap<String, CityDraft>,
    _warnings: &mut Vec<String>,
    review_items: &mut Vec<ManualReviewItem>,
) -> Result<(), String> {
    for unit in units {
        if unit.class_name != "city_data" {
            continue;
        }
        let game_token = unit
            .unit_name
            .split('.')
            .next_back()
            .unwrap_or(unit.unit_name.as_str())
            .trim_matches('.')
            .to_string();
        let id = format!("{}:{}", source.namespace, canonical_id_component(&game_token));
        let country_token = first_field(unit, "country").unwrap_or_else(|| "unknown".to_string());
        let country_id = format!("{}:{}", source.namespace, canonical_id_component(&country_token));
        let name_base = first_field(unit, "city_name").unwrap_or_else(|| title_case(&game_token));
        let name_localized = first_field(unit, "city_name_localized")
            .or_else(|| first_field(unit, "short_city_name_localized"));
        let name_local = resolve_localized(name_localized.as_deref(), &name_base, localization);
        let name_en = resolve_localized(name_localized.as_deref(), &name_base, localization);
        let population = first_field(unit, "population").and_then(|value| value.parse::<i64>().ok());
        let mut record = CityRecord {
            id: id.clone(),
            namespace: source.namespace.clone(),
            game_token: game_token.clone(),
            country_id,
            country_iso2: String::new(),
            name_en,
            name_local,
            aliases: vec![],
            population,
            coords: None,
            replaces_city_id: None,
            source: format!("local:{}!{}", normalize_path(&source.path), relative_path),
            source_version: source.source_version.clone(),
            checksum: String::new(),
            warnings: vec![],
        };
        merge_record_name(
            &id,
            relative_path,
            &mut record.name_en,
            &mut record.name_local,
            &mut record.aliases,
            cities.get(&id).map(|draft| &draft.record),
            review_items,
        );

        match cities.get_mut(&id) {
            Some(existing) if existing.priority > source.priority => {
                merge_aliases(&mut existing.record.aliases, &record.aliases);
            }
            Some(existing) => {
                merge_aliases(&mut record.aliases, &existing.record.aliases);
                *existing = CityDraft { record, country_token, priority: source.priority };
            }
            None => {
                cities.insert(id, CityDraft { record, country_token, priority: source.priority });
            }
        }
    }
    Ok(())
}

fn merge_company_units(
    source: &SourceInput,
    relative_path: &str,
    units: &[SiiUnit],
    localization: &HashMap<String, String>,
    overrides: &HashMap<String, CompanyOverride>,
    companies: &mut BTreeMap<String, CompanyDraft>,
    _warnings: &mut Vec<String>,
    review_items: &mut Vec<ManualReviewItem>,
) -> Result<(), String> {
    let path_segments = relative_path.split('/').collect::<Vec<_>>();
    let company_token_from_path = path_segments.get(2).copied().unwrap_or_default();

    for unit in units {
        let class_name = unit.class_name.as_str();
        if class_name != "company_permanent" && class_name != "company_def" && class_name != "cargo_def" {
            continue;
        }
        let game_token = if class_name == "company_permanent" {
            unit.unit_name
                .split('.')
                .next_back()
                .unwrap_or(unit.unit_name.as_str())
                .trim_matches('.')
                .to_string()
        } else {
            company_token_from_path.to_string()
        };
        if game_token.is_empty() {
            continue;
        }
        let id = format!("{}:{}", source.namespace, canonical_id_component(&game_token));
        let override_value = overrides.get(&id).or_else(|| overrides.get(&game_token));
        let entry = companies.entry(id.clone()).or_insert_with(|| CompanyDraft {
            record: CompanyRecord {
                id: id.clone(),
                namespace: source.namespace.clone(),
                game_token: game_token.clone(),
                name_en: title_case(&game_token),
                name_local: title_case(&game_token),
                aliases: vec![],
                payment_tier: override_value.and_then(|value| value.payment_tier.clone()).unwrap_or_else(|| DEFAULT_PAYMENT_TIER.to_string()),
                payment_multiplier: override_value.and_then(|value| value.payment_multiplier).unwrap_or(DEFAULT_PAYMENT_MULTIPLIER),
                preferred_cargo_types: vec![],
                offices: vec![],
                notes: override_value.and_then(|value| value.notes.clone()).unwrap_or_else(|| vec!["VTC balancing default company payment tier and multiplier".to_string()]),
                source: format!("local:{}!{}", normalize_path(&source.path), relative_path),
                source_version: source.source_version.clone(),
                checksum: String::new(),
                warnings: vec![],
            },
            priority: source.priority,
        });
        if source.priority >= entry.priority {
            entry.priority = source.priority;
            entry.record.source = format!("local:{}!{}", normalize_path(&source.path), relative_path);
            entry.record.source_version = source.source_version.clone();
        }

        match class_name {
            "company_permanent" => {
                let base_name = first_field(unit, "name")
                    .or_else(|| first_field(unit, "sort_name"))
                    .unwrap_or_else(|| title_case(&game_token));
                let mut candidate_name_en = resolve_localized(None, &base_name, localization);
                let mut candidate_name_local = candidate_name_en.clone();
                let existing_snapshot = entry.record.clone();
                merge_record_name(
                    &id,
                    relative_path,
                    &mut candidate_name_en,
                    &mut candidate_name_local,
                    &mut entry.record.aliases,
                    Some(&existing_snapshot),
                    review_items,
                );
                if source.priority >= entry.priority {
                    entry.record.name_en = candidate_name_en;
                    entry.record.name_local = candidate_name_local;
                }
            }
            "company_def" => {
                let city_token = first_field(unit, "city").unwrap_or_else(|| "unknown".to_string());
                let prefab_token = first_field(unit, "prefab");
                let office_id = format!(
                    "{}:{}:{}:{}",
                    source.namespace,
                    canonical_id_component(&game_token),
                    canonical_id_component(&city_token),
                    canonical_id_component(prefab_token.as_deref().unwrap_or("unknown"))
                );
                if entry.record.offices.iter().any(|office| office.id == office_id) {
                    continue;
                }
                entry.record.offices.push(CompanyOfficeRecord {
                    id: office_id,
                    city_id: Some(format!(
                        "{}:{}",
                        source.namespace,
                        canonical_id_component(&city_token)
                    )),
                    city_game_token: canonical_id_component(&city_token),
                    prefab_token: prefab_token.map(|value| canonical_id_component(&value)),
                    source: format!("local:{}!{}", normalize_path(&source.path), relative_path),
                    source_version: source.source_version.clone(),
                    checksum: String::new(),
                    warnings: vec![],
                });
            }
            "cargo_def" => {
                if let Some(cargo) = first_field(unit, "cargo") {
                    let cargo_token = cargo.strip_prefix("cargo.").unwrap_or(cargo.as_str()).to_string();
                    if !entry.record.preferred_cargo_types.contains(&cargo_token) {
                        entry.record.preferred_cargo_types.push(cargo_token);
                    }
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn finalize_countries(
    countries: BTreeMap<String, CountryDraft>,
    warnings: &mut Vec<String>,
) -> Result<Vec<CountryRecord>, String> {
    let mut records = countries.into_values().map(|draft| draft.record).collect::<Vec<_>>();
    records.sort_by(|left, right| left.id.cmp(&right.id));
    if records.is_empty() {
        warnings.push("countries_dataset_empty".to_string());
    }
    for record in &mut records {
        record.aliases.sort();
        record.aliases.dedup();
        record.checksum = checksum_country_record(record)?;
    }
    Ok(records)
}

fn finalize_cities(
    cities: BTreeMap<String, CityDraft>,
    countries: &[CountryRecord],
    warnings: &mut Vec<String>,
) -> Result<Vec<CityRecord>, String> {
    let country_map = countries
        .iter()
        .map(|country| (country.game_token.clone(), country))
        .collect::<HashMap<_, _>>();
    let mut records = Vec::new();
    for draft in cities.into_values() {
        let mut record = draft.record;
        if let Some(country) = country_map.get(&draft.country_token) {
            record.country_id = country.id.clone();
            record.country_iso2 = country.country_iso2.clone();
        } else {
            record.country_iso2 = "UN".to_string();
            record.warnings.push(format!("country_missing:{}", draft.country_token));
            warnings.push(format!("city_country_missing:{}:{}", record.id, draft.country_token));
        }
        record.aliases.sort();
        record.aliases.dedup();
        record.checksum = checksum_city_record(&record)?;
        records.push(record);
    }
    records.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(records)
}

fn finalize_companies(
    companies: BTreeMap<String, CompanyDraft>,
    cities: &[CityRecord],
    warnings: &mut Vec<String>,
) -> Result<Vec<CompanyRecord>, String> {
    let city_map = cities
        .iter()
        .map(|city| (city.game_token.clone(), city.id.clone()))
        .collect::<HashMap<_, _>>();
    let mut records = Vec::new();

    for draft in companies.into_values() {
        let mut record = draft.record;
        record.preferred_cargo_types.sort();
        record.preferred_cargo_types.dedup();
        record.aliases.sort();
        record.aliases.dedup();
        record.offices.sort_by(|left, right| left.id.cmp(&right.id));
        for office in &mut record.offices {
            office.city_id = city_map.get(&office.city_game_token).cloned();
            if office.city_id.is_none() {
                office.warnings.push(format!("office_city_missing:{}", office.city_game_token));
                warnings.push(format!("company_office_city_missing:{}:{}", record.id, office.city_game_token));
            }
            office.checksum = sha256_hex_bytes(serde_json::to_string(&office).map_err(|error| error.to_string())?.as_bytes());
        }
        record.checksum = checksum_company_record(&record)?;
        records.push(record);
    }

    records.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(records)
}

fn collect_city_review_items(cities: &[CityRecord]) -> Vec<ManualReviewItem> {
    let mut items = Vec::new();
    for (index, left) in cities.iter().enumerate() {
        for right in cities.iter().skip(index + 1) {
            if left.country_iso2 != right.country_iso2 || left.id == right.id {
                continue;
            }
            let similarity = levenshtein_similarity(&left.name_en, &right.name_en);
            if matches!(fuzzy_disposition(similarity), FuzzyDisposition::ManualReview) {
                items.push(ManualReviewItem {
                    left_id: left.id.clone(),
                    right_id: right.id.clone(),
                    similarity,
                    reason: format!("city_name_near_match_same_country:{}", left.country_iso2),
                });
            }
        }
    }
    items
}

fn merge_record_name<T>(
    record_id: &str,
    relative_path: &str,
    name_en: &mut String,
    name_local: &mut String,
    aliases: &mut Vec<String>,
    existing: Option<&T>,
    review_items: &mut Vec<ManualReviewItem>,
) where
    T: NameCarrier,
{
    let Some(existing) = existing else {
        return;
    };
    for candidate in [name_en.as_str(), name_local.as_str()] {
        let similarity = levenshtein_similarity(existing.primary_name(), candidate);
        match fuzzy_disposition(similarity) {
            FuzzyDisposition::Merge | FuzzyDisposition::AutoMerge => {
                if candidate != existing.primary_name() {
                    aliases.push(candidate.to_string());
                }
            }
            FuzzyDisposition::ManualReview => review_items.push(ManualReviewItem {
                left_id: record_id.to_string(),
                right_id: record_id.to_string(),
                similarity,
                reason: format!("same_id_name_review:{}", relative_path),
            }),
            FuzzyDisposition::KeepSeparate => aliases.push(candidate.to_string()),
        }
    }
}

trait NameCarrier {
    fn primary_name(&self) -> &str;
}

impl NameCarrier for CountryRecord {
    fn primary_name(&self) -> &str {
        &self.name_en
    }
}

impl NameCarrier for CityRecord {
    fn primary_name(&self) -> &str {
        &self.name_en
    }
}

impl NameCarrier for CompanyRecord {
    fn primary_name(&self) -> &str {
        &self.name_en
    }
}

fn merge_aliases(target: &mut Vec<String>, source: &[String]) {
    target.extend(source.iter().cloned());
    target.sort();
    target.dedup();
}

fn first_field(unit: &SiiUnit, key: &str) -> Option<String> {
    unit.fields.get(key).and_then(|values| values.first()).cloned()
}

fn resolve_localized(
    candidate: Option<&str>,
    fallback: &str,
    localization: &HashMap<String, String>,
) -> String {
    let Some(candidate) = candidate else {
        return fallback.to_string();
    };
    if candidate.starts_with("@@") && candidate.ends_with("@@") {
        localization
            .get(candidate)
            .cloned()
            .or_else(|| localization.get(candidate.trim_matches('@')).cloned())
            .unwrap_or_else(|| fallback.to_string())
    } else if candidate.trim().is_empty() {
        fallback.to_string()
    } else {
        candidate.to_string()
    }
}
fn derive_iso2(
    iso_country_code: Option<&str>,
    country_code: Option<&str>,
    game_token: &str,
) -> String {
    if let Some(iso3) = iso_country_code {
        let lower = iso3.trim().to_ascii_lowercase();
        if let Some(mapped) = iso3_to_iso2(&lower) {
            return mapped.to_string();
        }
        if lower.len() == 2 {
            return lower.to_ascii_uppercase();
        }
    }
    if let Some(code) = country_code {
        let trimmed = code.trim().to_ascii_uppercase();
        if trimmed.len() == 2 {
            return trimmed;
        }
    }
    canonical_id_component(game_token)
        .chars()
        .take(2)
        .collect::<String>()
        .to_ascii_uppercase()
}

fn iso3_to_iso2(value: &str) -> Option<&'static str> {
    match value {
        "aut" => Some("AT"),
        "bel" => Some("BE"),
        "bgr" => Some("BG"),
        "che" => Some("CH"),
        "cyp" => Some("CY"),
        "cze" => Some("CZ"),
        "deu" => Some("DE"),
        "dnk" => Some("DK"),
        "dza" => Some("DZ"),
        "egy" => Some("EG"),
        "esp" => Some("ES"),
        "est" => Some("EE"),
        "fin" => Some("FI"),
        "fra" => Some("FR"),
        "gbr" => Some("GB"),
        "geo" => Some("GE"),
        "grc" => Some("GR"),
        "hrv" => Some("HR"),
        "hun" => Some("HU"),
        "irl" => Some("IE"),
        "isl" => Some("IS"),
        "ita" => Some("IT"),
        "jor" => Some("JO"),
        "kaz" => Some("KZ"),
        "lbn" => Some("LB"),
        "ltu" => Some("LT"),
        "lux" => Some("LU"),
        "lva" => Some("LV"),
        "lby" => Some("LY"),
        "mar" => Some("MA"),
        "mlt" => Some("MT"),
        "mne" => Some("ME"),
        "nld" => Some("NL"),
        "nor" => Some("NO"),
        "pol" => Some("PL"),
        "prt" => Some("PT"),
        "rou" => Some("RO"),
        "rus" => Some("RU"),
        "sau" => Some("SA"),
        "srb" => Some("RS"),
        "svk" => Some("SK"),
        "svn" => Some("SI"),
        "swe" => Some("SE"),
        "syr" => Some("SY"),
        "tun" => Some("TN"),
        "tur" => Some("TR"),
        "ukr" => Some("UA"),
        _ => None,
    }
}

fn canonical_id_component(value: &str) -> String {
    let mut result = String::new();
    let lowered = value.trim().to_ascii_lowercase();
    for ch in lowered.chars() {
        let mapped = match ch {
            'ä' => "ae",
            'ö' => "oe",
            'ü' => "ue",
            'ß' => "ss",
            'à' | 'á' | 'â' | 'ã' | 'å' => "a",
            'ç' => "c",
            'è' | 'é' | 'ê' | 'ë' => "e",
            'ì' | 'í' | 'î' | 'ï' => "i",
            'ñ' => "n",
            'ò' | 'ó' | 'ô' | 'õ' => "o",
            'ù' | 'ú' | 'û' => "u",
            _ => "",
        };
        if !mapped.is_empty() {
            result.push_str(mapped);
            continue;
        }
        if ch.is_ascii_alphanumeric() {
            result.push(ch);
        } else if !result.ends_with('_') {
            result.push('_');
        }
    }
    result.trim_matches('_').to_string()
}

fn title_case(value: &str) -> String {
    canonical_id_component(value)
        .split('_')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => format!("{}{}", first.to_ascii_uppercase(), chars.as_str()),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn normalize_path(path: &Path) -> String {
    path.display().to_string().replace('\\', "/")
}

fn load_optional_json<T: DeserializeOwned>(path: &Path, default: T) -> Result<T, String> {
    if !path.exists() {
        return Ok(default);
    }
    let content = fs::read_to_string(path)
        .map_err(|error| error.to_string())?
        .trim_start_matches('\u{feff}')
        .to_string();
    serde_json::from_str(&content).map_err(|error| error.to_string())
}

fn write_dataset<T: serde::Serialize>(path: &Path, dataset: &DatasetFile<T>) -> Result<(), String> {
    let content = serde_json::to_string_pretty(dataset).map_err(|error| error.to_string())?;
    fs::write(path, format!("{}\n", content)).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::{canonical_id_component, parse_sii_units, strip_comments};

    #[test]
    fn parse_simple_units() {
        let units = parse_sii_units(
            r#"SiiNunit
            {
            city_data : city.berlin {
                city_name: \"Berlin\"
                country: germany
            }
            }
            "#,
        );
        assert_eq!(units.len(), 1);
        assert_eq!(units[0].class_name, "city_data");
        assert_eq!(units[0].unit_name, "city.berlin");
    }

    #[test]
    fn strip_comments_handles_inline_and_block_comments() {
        let cleaned = strip_comments("test // hidden\nvalue /* hidden */ keep");
        assert!(cleaned.contains("test"));
        assert!(cleaned.contains("keep"));
        assert!(!cleaned.contains("hidden"));
    }

    #[test]
    fn canonical_id_component_normalizes_tokens() {
        assert_eq!(canonical_id_component("Trade Aux"), "trade_aux");
    }
}

