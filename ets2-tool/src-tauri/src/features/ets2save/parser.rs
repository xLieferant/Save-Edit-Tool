use std::collections::{HashMap, HashSet};

use crate::features::ets2save::errors::{AppError, AppErrorCode};
use crate::features::ets2save::models::{
    EtsJobOfferPatch, SaveDepotBlock, SaveJobInfoSnapshot, SaveJobOfferData, SaveJobOfferPointer,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnitRange {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveDepotIndex {
    pub depots_by_city: HashMap<String, Vec<String>>,
    pub all_depots: HashSet<(String, String)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CityTokenResolution {
    pub token: String,
    pub mode: String,
    pub candidates: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SaveTemplateScan {
    pub depots: Vec<SaveDepotBlock>,
    pub depot_index: SaveDepotIndex,
    pub job_offer_data: HashMap<String, SaveJobOfferData>,
    pub job_info_units: Vec<SaveJobInfoSnapshot>,
    pub companies_index: Vec<String>,
    pub visited_cities: Vec<String>,
    pub transported_cargo_tokens: Vec<String>,
    pub selected_job_pointer: Option<String>,
}

pub fn sii_token(value: &str) -> String {
    let mut token = String::new();
    for character in value.trim().chars() {
        let mapped = match character {
            'a'..='z' | '0'..='9' => character,
            'A'..='Z' => character.to_ascii_lowercase(),
            'ä' | 'Ä' => 'a',
            'ö' | 'Ö' => 'o',
            'ü' | 'Ü' => 'u',
            'ß' => 's',
            _ => '_',
        };
        token.push(mapped);
    }

    while token.contains("__") {
        token = token.replace("__", "_");
    }
    token.trim_matches('_').to_string()
}

pub fn build_save_depot_index(lines: &[String]) -> SaveDepotIndex {
    let mut depots_by_city: HashMap<String, Vec<String>> = HashMap::new();
    let mut all_depots: HashSet<(String, String)> = HashSet::new();

    for line in lines {
        let trimmed = line.trim();
        let Some(without_prefix) = trimmed.strip_prefix("company : company.volatile.") else {
            continue;
        };
        let unit = without_prefix
            .split_whitespace()
            .next()
            .unwrap_or_default()
            .trim_end_matches('{')
            .trim();
        if unit.is_empty() {
            continue;
        }
        let mut parts = unit.splitn(2, '.');
        let company = sii_token(parts.next().unwrap_or_default());
        let city = sii_token(parts.next().unwrap_or_default());
        if company.is_empty() || city.is_empty() {
            continue;
        }
        all_depots.insert((company.clone(), city.clone()));
        depots_by_city.entry(city).or_default().push(company);
    }

    for companies in depots_by_city.values_mut() {
        companies.sort();
        companies.dedup();
    }

    SaveDepotIndex {
        depots_by_city,
        all_depots,
    }
}

pub fn resolve_city_token(req_city: &str, index: &SaveDepotIndex) -> Option<CityTokenResolution> {
    if index.depots_by_city.is_empty() {
        return None;
    }
    let req = sii_token(req_city);
    if req.is_empty() {
        return None;
    }

    if index.depots_by_city.contains_key(&req) {
        return Some(CityTokenResolution {
            token: req,
            mode: "exact".to_string(),
            candidates: vec![],
        });
    }

    let req_compact = req.replace('_', "").replace('-', "");
    let mut alias_hits = index
        .depots_by_city
        .keys()
        .filter(|key| key.replace('_', "").replace('-', "") == req_compact)
        .cloned()
        .collect::<Vec<_>>();
    alias_hits.sort();
    alias_hits.dedup();
    if let Some(city) = alias_hits.first() {
        return Some(CityTokenResolution {
            token: city.clone(),
            mode: "city_alias".to_string(),
            candidates: alias_hits,
        });
    }

    let mut scored = index
        .depots_by_city
        .keys()
        .map(|city| (city.clone(), dice_similarity(&req, city)))
        .collect::<Vec<_>>();
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    if let Some((best_city, best_score)) = scored.first() {
        if *best_score >= 0.92 {
            let top = scored
                .iter()
                .take(5)
                .map(|(city, _)| city.clone())
                .collect::<Vec<_>>();
            return Some(CityTokenResolution {
                token: best_city.clone(),
                mode: "fuzzy".to_string(),
                candidates: top,
            });
        }
    }

    None
}

pub fn fallback_company_in_city(index: &SaveDepotIndex, city_token: &str) -> Option<String> {
    let companies = index.depots_by_city.get(city_token)?;
    companies.first().cloned()
}

pub fn fallback_company_in_city_with_offers(
    depots: &[SaveDepotBlock],
    city_token: &str,
) -> Option<String> {
    depots
        .iter()
        .filter(|depot| depot.city_token == city_token && depot.job_offer_count > 0)
        .map(|depot| depot.company_token.clone())
        .min()
}

pub fn scan_save_templates(lines: &[String]) -> SaveTemplateScan {
    let depots = parse_save_depots(lines);
    let depot_index = build_save_depot_index(lines);
    let job_offer_data = parse_job_offer_data_units(lines);
    let job_info_units = parse_job_info_units(lines);
    let companies_index = parse_array_values(lines, "companies[");
    let visited_cities = parse_array_values(lines, "visited_cities[");
    let transported_cargo_tokens = parse_array_values(lines, "transported_cargo_types[")
        .into_iter()
        .map(|value| sii_token(value.trim_start_matches("cargo.")))
        .collect::<Vec<_>>();
    let selected_job_pointer = lines.iter().map(|line| line.trim()).find_map(|trimmed| {
        if !trimmed.starts_with("selected_job:") {
            return None;
        }
        trimmed
            .split_once(':')
            .map(|(_, value)| value.trim().to_string())
    });

    SaveTemplateScan {
        depots,
        depot_index,
        job_offer_data,
        job_info_units,
        companies_index,
        visited_cities,
        transported_cargo_tokens,
        selected_job_pointer,
    }
}

fn parse_save_depots(lines: &[String]) -> Vec<SaveDepotBlock> {
    let mut depots = Vec::new();
    let mut index = 0usize;
    while index < lines.len() {
        let line = lines[index].trim();
        let Some(rest) = line.strip_prefix("company : company.volatile.") else {
            index += 1;
            continue;
        };
        let unit = rest
            .split_whitespace()
            .next()
            .unwrap_or_default()
            .trim_end_matches('{')
            .trim();
        let mut parts = unit.splitn(2, '.');
        let company_token = sii_token(parts.next().unwrap_or_default());
        let city_token = sii_token(parts.next().unwrap_or_default());
        if company_token.is_empty() || city_token.is_empty() {
            index += 1;
            continue;
        }

        let mut depth = line.matches('{').count() as i32 - line.matches('}').count() as i32;
        let start = index;
        let mut end = index;
        while end + 1 < lines.len() && depth > 0 {
            end += 1;
            depth += lines[end].matches('{').count() as i32;
            depth -= lines[end].matches('}').count() as i32;
        }

        let mut permanent_data = None;
        let mut job_offer_count = 0usize;
        let mut job_offers = Vec::new();
        for block_line in &lines[start..=end] {
            let trimmed = block_line.trim();
            if let Some((_, value)) = trimmed.split_once(':') {
                if trimmed.starts_with("permanent_data:") {
                    permanent_data = Some(value.trim().trim_matches('"').to_string());
                } else if trimmed.starts_with("job_offer:") {
                    job_offer_count = value.trim().parse::<usize>().unwrap_or(0);
                } else if trimmed.starts_with("job_offer[") {
                    let idx = trimmed
                        .split(']')
                        .next()
                        .and_then(|left| left.strip_prefix("job_offer["))
                        .and_then(|raw| raw.parse::<usize>().ok())
                        .unwrap_or(0);
                    job_offers.push(SaveJobOfferPointer {
                        index: idx,
                        pointer: value.trim().to_string(),
                    });
                }
            }
        }
        depots.push(SaveDepotBlock {
            unit_token: format!("company.volatile.{}.{}", company_token, city_token),
            company_token,
            city_token,
            permanent_data,
            job_offer_count,
            job_offers,
        });
        index = end + 1;
    }
    depots
}

fn parse_job_offer_data_units(lines: &[String]) -> HashMap<String, SaveJobOfferData> {
    let mut out = HashMap::new();
    let mut index = 0usize;
    while index < lines.len() {
        let line = lines[index].trim();
        let Some(rest) = line.strip_prefix("job_offer_data : ") else {
            index += 1;
            continue;
        };
        let pointer = rest
            .split_whitespace()
            .next()
            .unwrap_or_default()
            .trim_end_matches('{')
            .to_string();
        if pointer.is_empty() {
            index += 1;
            continue;
        }
        let mut depth = line.matches('{').count() as i32 - line.matches('}').count() as i32;
        let start = index;
        let mut end = index;
        while end + 1 < lines.len() && depth > 0 {
            end += 1;
            depth += lines[end].matches('{').count() as i32;
            depth -= lines[end].matches('}').count() as i32;
        }

        let field = |name: &str| extract_field_value(lines, UnitRange { start, end }, name);
        let parse_i64 = |name: &str| field(name).and_then(|value| value.parse::<i64>().ok());
        out.insert(
            pointer.clone(),
            SaveJobOfferData {
                pointer,
                target: field("target"),
                expiration_time: parse_i64("expiration_time"),
                urgency: parse_i64("urgency"),
                shortest_distance_km: parse_i64("shortest_distance_km"),
                ferry_time: parse_i64("ferry_time"),
                ferry_price: parse_i64("ferry_price"),
                cargo: field("cargo"),
                company_truck: field("company_truck"),
                trailer_variant: field("trailer_variant"),
                trailer_definition: field("trailer_definition"),
                units_count: parse_i64("units_count"),
                fill_ratio: parse_i64("fill_ratio"),
                trailer_place: parse_i64("trailer_place"),
            },
        );
        index = end + 1;
    }
    out
}

fn parse_job_info_units(lines: &[String]) -> Vec<SaveJobInfoSnapshot> {
    let mut out = Vec::new();
    let mut index = 0usize;
    while index < lines.len() {
        let line = lines[index].trim();
        let Some(rest) = line.strip_prefix("job_info : ") else {
            index += 1;
            continue;
        };
        let pointer = rest
            .split_whitespace()
            .next()
            .unwrap_or_default()
            .trim_end_matches('{')
            .to_string();
        if pointer.is_empty() {
            index += 1;
            continue;
        }
        let mut depth = line.matches('{').count() as i32 - line.matches('}').count() as i32;
        let start = index;
        let mut end = index;
        while end + 1 < lines.len() && depth > 0 {
            end += 1;
            depth += lines[end].matches('{').count() as i32;
            depth -= lines[end].matches('}').count() as i32;
        }
        let field = |name: &str| extract_field_value(lines, UnitRange { start, end }, name);
        let parse_i64 = |name: &str| field(name).and_then(|value| value.parse::<i64>().ok());
        out.push(SaveJobInfoSnapshot {
            pointer,
            cargo: field("cargo"),
            source_company: field("source_company"),
            target_company: field("target_company"),
            planned_distance_km: parse_i64("planned_distance_km"),
            ferry_time: parse_i64("ferry_time"),
            ferry_price: parse_i64("ferry_price"),
            urgency: field("urgency"),
            units_count: parse_i64("units_count"),
            fill_ratio: parse_i64("fill_ratio"),
        });
        index = end + 1;
    }
    out
}

fn parse_array_values(lines: &[String], prefix: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in lines {
        let trimmed = line.trim();
        if !trimmed.starts_with(prefix) {
            continue;
        }
        if let Some((_, value)) = trimmed.split_once(':') {
            out.push(value.trim().trim_matches('"').to_string());
        }
    }
    out
}

fn dice_similarity(a: &str, b: &str) -> f64 {
    if a == b {
        return 1.0;
    }
    let a_chars = a.chars().collect::<Vec<_>>();
    let b_chars = b.chars().collect::<Vec<_>>();
    if a_chars.len() < 2 || b_chars.len() < 2 {
        return 0.0;
    }

    let mut a_bigrams: HashMap<(char, char), usize> = HashMap::new();
    for window in a_chars.windows(2) {
        let key = (window[0], window[1]);
        *a_bigrams.entry(key).or_insert(0) += 1;
    }
    let mut matches = 0usize;
    for window in b_chars.windows(2) {
        let key = (window[0], window[1]);
        if let Some(count) = a_bigrams.get_mut(&key) {
            if *count > 0 {
                *count -= 1;
                matches += 1;
            }
        }
    }
    (2.0 * matches as f64) / ((a_chars.len() - 1 + b_chars.len() - 1) as f64)
}

pub fn find_company_block(
    lines: &[String],
    src_company: &str,
    src_city: &str,
) -> Result<UnitRange, AppError> {
    let token = format!(
        "company : company.volatile.{}.{}",
        sii_token(src_company),
        sii_token(src_city)
    );
    find_unit_range(lines, &token).ok_or_else(|| {
        AppError::new(
            AppErrorCode::CompanyNotFoundInSave,
            format!("Company block not found: {}", token),
        )
    })
}

pub fn extract_job_offer_pointer(
    lines: &[String],
    company_block: UnitRange,
) -> Result<String, AppError> {
    let offer_count = extract_job_offer_count(lines, company_block)?;
    if offer_count == 0 {
        return Err(AppError::new(
            AppErrorCode::CompanyHasNoJobOffers,
            "Company block has no job_offer entries",
        ));
    }

    for line in &lines[company_block.start..=company_block.end] {
        let trimmed = line.trim();
        if trimmed.starts_with("job_offer[0]:") {
            return trimmed
                .split_once(':')
                .map(|(_, value)| value.trim().to_string())
                .ok_or_else(|| {
                    AppError::new(
                        AppErrorCode::InvalidToken,
                        "Could not parse job_offer[0] pointer",
                    )
                });
        }
    }

    Err(AppError::new(
        AppErrorCode::InvalidToken,
        "job_offer[0] pointer missing",
    ))
}

pub fn list_job_offer_pointers(
    lines: &[String],
    company_block: UnitRange,
) -> Vec<SaveJobOfferPointer> {
    let mut out = Vec::new();
    for line in &lines[company_block.start..=company_block.end] {
        let trimmed = line.trim();
        if !trimmed.starts_with("job_offer[") {
            continue;
        }
        let Some((_, value)) = trimmed.split_once(':') else {
            continue;
        };
        let idx = trimmed
            .split(']')
            .next()
            .and_then(|left| left.strip_prefix("job_offer["))
            .and_then(|raw| raw.parse::<usize>().ok())
            .unwrap_or(0);
        let pointer = value.trim().to_string();
        if !pointer.is_empty() {
            out.push(SaveJobOfferPointer {
                index: idx,
                pointer,
            });
        }
    }
    out.sort_by_key(|item| item.index);
    out
}

pub fn find_job_offer_data_block(lines: &[String], pointer: &str) -> Result<UnitRange, AppError> {
    let token = format!("job_offer_data : {}", pointer);
    find_unit_range(lines, &token).ok_or_else(|| {
        AppError::new(
            AppErrorCode::InvalidToken,
            format!("job_offer_data block not found for {}", pointer),
        )
    })
}

pub fn find_job_info_block(lines: &[String], pointer: &str) -> Result<UnitRange, AppError> {
    let token = format!("job_info : {}", pointer);
    find_unit_range(lines, &token).ok_or_else(|| {
        AppError::new(
            AppErrorCode::InvalidToken,
            format!("job_info block not found for {}", pointer),
        )
    })
}

pub fn extract_field_value(lines: &[String], range: UnitRange, field: &str) -> Option<String> {
    for line in &lines[range.start..=range.end] {
        let trimmed = line.trim();
        if trimmed.starts_with(&format!("{}:", field)) {
            return trimmed
                .split_once(':')
                .map(|(_, value)| value.trim().trim_matches('"').to_string());
        }
    }
    None
}

pub fn extract_in_game_time(lines: &[String]) -> i64 {
    for line in lines {
        let trimmed = line.trim();
        if trimmed.starts_with("game_time:") {
            if let Some((_, value)) = trimmed.split_once(':') {
                if let Ok(parsed) = value.trim().parse::<i64>() {
                    return parsed;
                }
            }
        }
    }
    0
}

pub fn patch_job_offer_data(lines: &mut Vec<String>, range: UnitRange, patch: &EtsJobOfferPatch) {
    set_or_insert_field(lines, range, "target", &patch.target);
    set_or_insert_field(
        lines,
        range,
        "expiration_time",
        &patch.expiration_time.to_string(),
    );
    set_or_insert_field(lines, range, "urgency", &patch.urgency.to_string());
    set_or_insert_field(
        lines,
        range,
        "shortest_distance_km",
        &patch.shortest_distance_km.to_string(),
    );
    set_or_insert_field(lines, range, "ferry_time", &patch.ferry_time.to_string());
    set_or_insert_field(lines, range, "ferry_price", &patch.ferry_price.to_string());
    set_or_insert_field(lines, range, "cargo", &patch.cargo);
    set_or_insert_field(
        lines,
        range,
        "company_truck",
        if patch.company_truck { "true" } else { "false" },
    );
    if let Some(trailer_variant) = patch.trailer_variant.as_deref() {
        set_or_insert_field(lines, range, "trailer_variant", trailer_variant);
    }
    if let Some(trailer_definition) = patch.trailer_definition.as_deref() {
        set_or_insert_field(lines, range, "trailer_definition", trailer_definition);
    }
    set_or_insert_field(lines, range, "units_count", &patch.units_count.to_string());
    set_or_insert_field(lines, range, "fill_ratio", &patch.fill_ratio.to_string());
    set_or_insert_field(
        lines,
        range,
        "trailer_place",
        &patch.trailer_place.to_string(),
    );
}

fn extract_job_offer_count(lines: &[String], company_block: UnitRange) -> Result<i64, AppError> {
    for line in &lines[company_block.start..=company_block.end] {
        let trimmed = line.trim();
        if trimmed.starts_with("job_offer:") {
            return trimmed
                .split_once(':')
                .map(|(_, value)| value.trim().parse::<i64>())
                .transpose()
                .map_err(|error| {
                    AppError::new(
                        AppErrorCode::InvalidToken,
                        format!("Invalid job_offer count: {}", error),
                    )
                })?
                .ok_or_else(|| {
                    AppError::new(AppErrorCode::InvalidToken, "Missing job_offer count value")
                });
        }
    }

    Err(AppError::new(
        AppErrorCode::InvalidToken,
        "job_offer count missing from company block",
    ))
}

fn find_unit_range(lines: &[String], token: &str) -> Option<UnitRange> {
    let mut start_index = None;
    let mut depth = 0_i32;

    for (index, line) in lines.iter().enumerate() {
        if start_index.is_none() && line.trim().starts_with(token) {
            start_index = Some(index);
        }

        if let Some(start) = start_index {
            depth += line.matches('{').count() as i32;
            depth -= line.matches('}').count() as i32;

            if index > start && depth <= 0 {
                return Some(UnitRange { start, end: index });
            }
        }
    }

    None
}

fn set_or_insert_field(lines: &mut Vec<String>, range: UnitRange, field: &str, value: &str) {
    let replacement = format!(" {}: {}", field, value);
    for index in range.start..=range.end {
        let trimmed = lines[index].trim();
        if trimmed.starts_with(&format!("{}:", field)) {
            let indent = lines[index]
                .chars()
                .take_while(|character| character.is_whitespace())
                .collect::<String>();
            lines[index] = format!("{}{}: {}", indent, field, value);
            return;
        }
    }

    lines.insert(range.end, replacement);
}

#[cfg(test)]
mod tests {
    use super::{
        build_save_depot_index, extract_job_offer_pointer, fallback_company_in_city,
        fallback_company_in_city_with_offers, find_company_block, find_job_offer_data_block,
        patch_job_offer_data, resolve_city_token, scan_save_templates,
    };
    use crate::features::ets2save::models::EtsJobOfferPatch;

    fn fixture_lines() -> Vec<String> {
        [
            "SiiNunit",
            "{",
            "company : company.volatile.test_company.berlin {",
            " job_offer: 1",
            " job_offer[0]: _nameless.offer.001",
            "}",
            "job_offer_data : _nameless.offer.001 {",
            " target: test_company.munich",
            " expiration_time: 100",
            " cargo: cargo.old",
            " company_truck: false",
            " units_count: 1",
            " fill_ratio: 1",
            " trailer_place: 0",
            "}",
            "}",
        ]
        .into_iter()
        .map(|line| line.to_string())
        .collect()
    }

    #[test]
    fn parser_finds_company_block() {
        let lines = fixture_lines();
        let range = find_company_block(&lines, "test_company", "berlin").unwrap();
        assert_eq!(range.start, 2);
        assert_eq!(range.end, 5);
    }

    #[test]
    fn parser_extracts_offer_pointer() {
        let lines = fixture_lines();
        let company = find_company_block(&lines, "test_company", "berlin").unwrap();
        let pointer = extract_job_offer_pointer(&lines, company).unwrap();
        assert_eq!(pointer, "_nameless.offer.001");
    }

    #[test]
    fn parser_patches_job_offer_data() {
        let mut lines = fixture_lines();
        let pointer = "_nameless.offer.001";
        let range = find_job_offer_data_block(&lines, pointer).unwrap();
        patch_job_offer_data(
            &mut lines,
            range,
            &EtsJobOfferPatch {
                target: "test_company.hamburg".to_string(),
                expiration_time: 6120,
                urgency: 0,
                shortest_distance_km: 520,
                ferry_time: 0,
                ferry_price: 0,
                cargo: "cargo.trucks".to_string(),
                company_truck: true,
                trailer_variant: Some("variant.a".to_string()),
                trailer_definition: Some("trailer.def".to_string()),
                units_count: 8,
                fill_ratio: 1,
                trailer_place: 0,
                job_info_unit: None,
                selected_job_unit: None,
            },
        );

        let joined = lines.join("\n");
        assert!(joined.contains("target: test_company.hamburg"));
        assert!(joined.contains("cargo: cargo.trucks"));
        assert!(joined.contains("company_truck: true"));
        assert!(joined.contains("trailer_definition: trailer.def"));
    }

    #[test]
    fn resolve_city_token_aliases_le_havre_to_lehavre() {
        let lines = [
            "SiiNunit",
            "{",
            "company : company.volatile.voitureux.lehavre {",
            " job_offer: 1",
            " job_offer[0]: _nameless.offer.001",
            "}",
            "}",
        ]
        .into_iter()
        .map(|line| line.to_string())
        .collect::<Vec<_>>();
        let index = build_save_depot_index(&lines);
        let resolved = resolve_city_token("le_havre", &index).unwrap();
        assert_eq!(resolved.token, "lehavre");
        assert_eq!(resolved.mode, "city_alias");
    }

    #[test]
    fn fallback_host_selects_first_company_in_city() {
        let lines = [
            "SiiNunit",
            "{",
            "company : company.volatile.voitureux.lehavre {",
            " job_offer: 1",
            " job_offer[0]: _nameless.offer.001",
            "}",
            "company : company.volatile.tradeaux.lehavre {",
            " job_offer: 1",
            " job_offer[0]: _nameless.offer.002",
            "}",
            "}",
        ]
        .into_iter()
        .map(|line| line.to_string())
        .collect::<Vec<_>>();
        let index = build_save_depot_index(&lines);
        let selected = fallback_company_in_city(&index, "lehavre").unwrap();
        assert_eq!(selected, "tradeaux");
    }

    #[test]
    fn fallback_host_with_offers_skips_offerless_companies() {
        let lines = [
            "SiiNunit",
            "{",
            "company : company.volatile.tradeaux.lehavre {",
            " job_offer: 0",
            "}",
            "company : company.volatile.voitureux.lehavre {",
            " job_offer: 2",
            " job_offer[0]: _nameless.offer.002",
            " job_offer[1]: _nameless.offer.003",
            "}",
            "}",
        ]
        .into_iter()
        .map(|line| line.to_string())
        .collect::<Vec<_>>();
        let scan = scan_save_templates(&lines);
        let selected = fallback_company_in_city_with_offers(&scan.depots, "lehavre").unwrap();
        assert_eq!(selected, "voitureux");
    }

    #[test]
    fn scan_templates_defaults_missing_job_offer_count_to_zero() {
        let lines = [
            "SiiNunit",
            "{",
            "company : company.volatile.tradeaux.berlin {",
            " permanent_data: company.permanent.tradeaux",
            "}",
            "}",
        ]
        .into_iter()
        .map(|line| line.to_string())
        .collect::<Vec<_>>();
        let scan = scan_save_templates(&lines);
        assert_eq!(scan.depots.len(), 1);
        assert_eq!(scan.depots[0].job_offer_count, 0);
    }

    #[test]
    fn scan_templates_extracts_company_offer_data_and_job_info() {
        let lines = [
            "SiiNunit",
            "{",
            "company : company.volatile.eurogoodies.dusseldorf {",
            " permanent_data: company.permanent.eurogoodies",
            " job_offer: 1",
            " job_offer[0]: _nameless.offer.1000",
            "}",
            "job_offer_data : _nameless.offer.1000 {",
            " target: suprema.panevezys",
            " expiration_time: 1100860",
            " urgency: 1",
            " shortest_distance_km: 1545",
            " cargo: cargo.vinegar_c",
            " company_truck: scania_streamline_6x2_norm_440",
            " trailer_definition: trailer_def.kogel.port.ch_3_tri_20.container",
            " units_count: 15",
            " fill_ratio: 1",
            " trailer_place: 0",
            "}",
            "companies[0]: company.volatile.eurogoodies.dusseldorf",
            "visited_cities[0]: dusseldorf",
            "job_info : _nameless.jobinfo.1 {",
            " planned_distance_km: 1545",
            " cargo: cargo.vinegar_c",
            " source_company: eurogoodies.dusseldorf",
            " target_company: suprema.panevezys",
            "}",
            "}",
        ]
        .into_iter()
        .map(|line| line.to_string())
        .collect::<Vec<_>>();
        let scan = scan_save_templates(&lines);
        assert_eq!(scan.depots.len(), 1);
        assert_eq!(scan.job_offer_data.len(), 1);
        assert_eq!(scan.job_info_units.len(), 1);
        assert_eq!(scan.companies_index.len(), 1);
        assert_eq!(scan.visited_cities.len(), 1);
        assert_eq!(scan.depots[0].company_token, "eurogoodies");
        assert_eq!(scan.depots[0].city_token, "dusseldorf");
    }
}
