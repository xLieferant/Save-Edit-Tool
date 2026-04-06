use crate::features::ets2save::errors::{AppError, AppErrorCode};
use crate::features::ets2save::models::EtsJobOfferPatch;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnitRange {
    pub start: usize,
    pub end: usize,
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

pub fn find_job_offer_data_block(
    lines: &[String],
    pointer: &str,
) -> Result<UnitRange, AppError> {
    let token = format!("job_offer_data : {}", pointer);
    find_unit_range(lines, &token).ok_or_else(|| {
        AppError::new(
            AppErrorCode::InvalidToken,
            format!("job_offer_data block not found for {}", pointer),
        )
    })
}

pub fn extract_field_value(
    lines: &[String],
    range: UnitRange,
    field: &str,
) -> Option<String> {
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

pub fn patch_job_offer_data(
    lines: &mut Vec<String>,
    range: UnitRange,
    patch: &EtsJobOfferPatch,
) {
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
                    AppError::new(
                        AppErrorCode::InvalidToken,
                        "Missing job_offer count value",
                    )
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

fn set_or_insert_field(
    lines: &mut Vec<String>,
    range: UnitRange,
    field: &str,
    value: &str,
) {
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
        extract_job_offer_pointer, find_company_block, find_job_offer_data_block, patch_job_offer_data,
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
}
