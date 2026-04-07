use std::path::Path;

use crate::features::ets2save::errors::{AppError, AppErrorCode};
use crate::features::ets2save::models::{PostWriteOfferSlotScan, PostWriteValidationResult};
use crate::features::ets2save::parser::{
    UnitRange, extract_field_value, find_job_offer_data_block, list_job_offer_pointers, sii_token,
};
use crate::features::ets2save::sii_codec::decode_sii_lines;

const ROOT_CAUSE_ETS2_LOAD_OR_CACHE: &str = "ets2_load_or_cache";
const ROOT_CAUSE_WRONG_DEPOT: &str = "wrong_depot";
const ROOT_CAUSE_WRONG_SLOT: &str = "wrong_slot";
const ROOT_CAUSE_WRITE_CORRUPT: &str = "write_corrupt";
const ROOT_CAUSE_CARGO_MISMATCH: &str = "cargo_mismatch";
const ROOT_CAUSE_TARGET_MISMATCH: &str = "target_mismatch";
const ROOT_CAUSE_SHORTEST_DISTANCE_MISSING: &str = "shortest_distance_missing";
const ROOT_CAUSE_EXPIRATION_TIME_MISSING: &str = "expiration_time_missing";
const VALIDATION_ERROR_COMPANY_BLOCK_MISSING: &str = "company_block_missing";
const VALIDATION_ERROR_OFFER_POINTER_MISSING: &str = "offer_pointer_missing";
const VALIDATION_ERROR_OFFER_DATA_MISSING: &str = "offer_data_missing";
const VALIDATION_ERROR_CARGO_MISMATCH: &str = "cargo_mismatch";
const VALIDATION_ERROR_TARGET_MISMATCH: &str = "target_mismatch";
const VALIDATION_ERROR_SHORTEST_DISTANCE_MISSING: &str = "shortest_distance_missing";
const VALIDATION_ERROR_EXPIRATION_TIME_MISSING: &str = "expiration_time_missing";

pub fn validate_written_job(
    save_path: &Path,
    expected_company: &str,
    expected_offer_pointer: &str,
    expected_cargo: &str,
    expected_target: &str,
) -> Result<PostWriteValidationResult, AppError> {
    let lines = decode_sii_lines(save_path)?;
    Ok(validate_written_job_lines(
        &lines,
        expected_company,
        expected_offer_pointer,
        expected_cargo,
        expected_target,
    ))
}

pub fn validate_written_job_lines(
    lines: &[String],
    expected_company: &str,
    expected_offer_pointer: &str,
    expected_cargo: &str,
    expected_target: &str,
) -> PostWriteValidationResult {
    let normalized_company = normalize_company_unit(expected_company);
    let mut result = PostWriteValidationResult {
        expected_company: normalized_company.clone(),
        expected_offer_pointer: expected_offer_pointer.trim().to_string(),
        expected_cargo: expected_cargo.trim().to_string(),
        expected_target: expected_target.trim().to_string(),
        root_cause: ROOT_CAUSE_ETS2_LOAD_OR_CACHE.to_string(),
        ..PostWriteValidationResult::default()
    };

    let company_block = match find_company_block_by_unit(lines, &normalized_company) {
        Some(range) => {
            result.company_block_found = true;
            range
        }
        None => {
            result.root_cause = ROOT_CAUSE_WRONG_DEPOT.to_string();
            result.validation_error_code = Some(VALIDATION_ERROR_COMPANY_BLOCK_MISSING.to_string());
            result.validation_error = Some(format!(
                "Company block not found after write: {}",
                normalized_company
            ));
            return result;
        }
    };

    let mut slots = scan_company_offer_slots(lines, company_block, expected_offer_pointer);
    if let Some(slot) = slots
        .iter_mut()
        .find(|slot| slot.pointer == expected_offer_pointer.trim())
    {
        slot.selected = true;
        result.offer_pointer_found = true;
        result.selected_offer_slot_index = Some(slot.index);
        result.selected_offer_slot_pointer = Some(slot.pointer.clone());
    } else {
        result.offer_slots = slots;
        result.root_cause = ROOT_CAUSE_WRONG_SLOT.to_string();
        result.validation_error_code = Some(VALIDATION_ERROR_OFFER_POINTER_MISSING.to_string());
        result.validation_error = Some(format!(
            "Expected offer pointer missing from company block: {}",
            expected_offer_pointer.trim()
        ));
        return result;
    }
    result.offer_slots = slots;

    let offer_range = match find_job_offer_data_block(lines, expected_offer_pointer.trim()) {
        Ok(range) => {
            result.offer_data_found = true;
            range
        }
        Err(_) => {
            result.root_cause = ROOT_CAUSE_WRITE_CORRUPT.to_string();
            result.validation_error_code = Some(VALIDATION_ERROR_OFFER_DATA_MISSING.to_string());
            result.validation_error = Some(format!(
                "job_offer_data block missing for pointer {}",
                expected_offer_pointer.trim()
            ));
            return result;
        }
    };

    result.written_cargo = extract_field_value(lines, offer_range, "cargo");
    result.written_target = extract_field_value(lines, offer_range, "target");
    result.written_shortest_distance_km = parse_i64_field(extract_field_value(
        lines,
        offer_range,
        "shortest_distance_km",
    ));
    result.written_expiration_time =
        parse_i64_field(extract_field_value(lines, offer_range, "expiration_time"));
    result.cargo_matches =
        values_match(result.written_cargo.as_deref(), Some(expected_cargo.trim()));
    result.target_matches = values_match(
        result.written_target.as_deref(),
        Some(expected_target.trim()),
    );
    result.shortest_distance_present = result.written_shortest_distance_km.is_some();
    result.expiration_time_present = result.written_expiration_time.is_some();

    if !result.cargo_matches {
        result.root_cause = ROOT_CAUSE_CARGO_MISMATCH.to_string();
        result.validation_error_code = Some(VALIDATION_ERROR_CARGO_MISMATCH.to_string());
        result.validation_error = Some(format!(
            "Cargo mismatch after write: expected={} actual={}",
            expected_cargo.trim(),
            result.written_cargo.as_deref().unwrap_or("-")
        ));
        return result;
    }

    if !result.target_matches {
        result.root_cause = ROOT_CAUSE_TARGET_MISMATCH.to_string();
        result.validation_error_code = Some(VALIDATION_ERROR_TARGET_MISMATCH.to_string());
        result.validation_error = Some(format!(
            "Target mismatch after write: expected={} actual={}",
            expected_target.trim(),
            result.written_target.as_deref().unwrap_or("-")
        ));
        return result;
    }

    if !result.shortest_distance_present {
        result.root_cause = ROOT_CAUSE_SHORTEST_DISTANCE_MISSING.to_string();
        result.validation_error_code = Some(VALIDATION_ERROR_SHORTEST_DISTANCE_MISSING.to_string());
        result.validation_error =
            Some("shortest_distance_km missing in written job_offer_data".to_string());
        return result;
    }

    if !result.expiration_time_present {
        result.root_cause = ROOT_CAUSE_EXPIRATION_TIME_MISSING.to_string();
        result.validation_error_code = Some(VALIDATION_ERROR_EXPIRATION_TIME_MISSING.to_string());
        result.validation_error =
            Some("expiration_time missing in written job_offer_data".to_string());
        return result;
    }

    result.valid = true;
    result
}

pub fn select_offer_slot(
    lines: &[String],
    company_block: UnitRange,
) -> Result<(usize, String, Vec<PostWriteOfferSlotScan>), AppError> {
    let mut slots = scan_company_offer_slots(lines, company_block, "");
    if slots.is_empty() {
        return Err(AppError::new(
            AppErrorCode::CompanyHasNoJobOffers,
            "Company block has no job_offer entries",
        ));
    }

    if let Some(selected_index) = slots.iter().position(|slot| slot.offer_data_found) {
        slots[selected_index].selected = true;
        return Ok((
            slots[selected_index].index as usize,
            slots[selected_index].pointer.clone(),
            slots,
        ));
    }

    slots[0].selected = true;
    Ok((slots[0].index as usize, slots[0].pointer.clone(), slots))
}

fn scan_company_offer_slots(
    lines: &[String],
    company_block: UnitRange,
    expected_offer_pointer: &str,
) -> Vec<PostWriteOfferSlotScan> {
    list_job_offer_pointers(lines, company_block)
        .into_iter()
        .map(|pointer| PostWriteOfferSlotScan {
            index: pointer.index as i64,
            offer_data_found: find_job_offer_data_block(lines, &pointer.pointer).is_ok(),
            matches_expected_pointer: !expected_offer_pointer.trim().is_empty()
                && pointer.pointer == expected_offer_pointer.trim(),
            pointer: pointer.pointer,
            selected: false,
        })
        .collect()
}

fn parse_i64_field(value: Option<String>) -> Option<i64> {
    value.and_then(|raw| raw.trim().parse::<i64>().ok())
}

fn values_match(actual: Option<&str>, expected: Option<&str>) -> bool {
    match (actual, expected) {
        (Some(actual), Some(expected)) => actual.trim() == expected.trim(),
        _ => false,
    }
}

fn normalize_company_unit(value: &str) -> String {
    let trimmed = value.trim().trim_end_matches('{').trim();
    if let Some(raw) = trimmed.strip_prefix("company.volatile.") {
        let mut parts = raw.splitn(2, '.');
        let company = sii_token(parts.next().unwrap_or_default());
        let city = sii_token(parts.next().unwrap_or_default());
        if !company.is_empty() && !city.is_empty() {
            return format!("company.volatile.{}.{}", company, city);
        }
    }

    let mut parts = trimmed.splitn(2, '.');
    let company = sii_token(parts.next().unwrap_or_default());
    let city = sii_token(parts.next().unwrap_or_default());
    if !company.is_empty() && !city.is_empty() {
        return format!("company.volatile.{}.{}", company, city);
    }

    trimmed.to_string()
}

fn find_company_block_by_unit(lines: &[String], company_unit: &str) -> Option<UnitRange> {
    let token = format!("company : {}", company_unit.trim());
    find_unit_range(lines, &token)
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

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{select_offer_slot, validate_written_job, validate_written_job_lines};
    use crate::features::ets2save::parser::find_company_block;

    fn valid_fixture() -> &'static str {
        "SiiNunit\n{\ncompany : company.volatile.tradeaux.berlin {\n job_offer: 2\n job_offer[0]: _nameless.offer.000\n job_offer[1]: _nameless.offer.001\n}\njob_offer_data : _nameless.offer.001 {\n target: eurogoodies.hamburg\n expiration_time: 6120\n shortest_distance_km: 520\n cargo: cargo.trucks\n}\n}\n"
    }

    #[test]
    fn validator_accepts_valid_pointer_chain() {
        let lines = valid_fixture()
            .lines()
            .map(|line| line.to_string())
            .collect::<Vec<_>>();
        let validation = validate_written_job_lines(
            &lines,
            "company.volatile.tradeaux.berlin",
            "_nameless.offer.001",
            "cargo.trucks",
            "eurogoodies.hamburg",
        );

        assert!(validation.valid);
        assert_eq!(validation.root_cause, "ets2_load_or_cache");
        assert!(validation.company_block_found);
        assert!(validation.offer_pointer_found);
        assert!(validation.offer_data_found);
        assert!(validation.cargo_matches);
        assert!(validation.target_matches);
    }

    #[test]
    fn validator_flags_cargo_mismatch() {
        let lines = valid_fixture()
            .lines()
            .map(|line| line.to_string())
            .collect::<Vec<_>>();
        let validation = validate_written_job_lines(
            &lines,
            "company.volatile.tradeaux.berlin",
            "_nameless.offer.001",
            "cargo.cars",
            "eurogoodies.hamburg",
        );

        assert!(!validation.valid);
        assert_eq!(validation.root_cause, "cargo_mismatch");
        assert_eq!(
            validation.validation_error_code.as_deref(),
            Some("cargo_mismatch")
        );
    }

    #[test]
    fn slot_selector_prefers_pointer_with_offer_data() {
        let lines = valid_fixture()
            .lines()
            .map(|line| line.to_string())
            .collect::<Vec<_>>();
        let block = find_company_block(&lines, "tradeaux", "berlin").unwrap();
        let (index, pointer, slots) = select_offer_slot(&lines, block).unwrap();

        assert_eq!(index, 1);
        assert_eq!(pointer, "_nameless.offer.001");
        assert!(
            slots
                .iter()
                .any(|slot| slot.selected && slot.pointer == pointer)
        );
    }

    #[test]
    fn validator_reads_save_from_disk() {
        let temp_dir = std::env::temp_dir().join("ets2_tool_post_write_validator_test");
        let _ = fs::create_dir_all(&temp_dir);
        let save_path = temp_dir.join("game.sii");
        fs::write(&save_path, valid_fixture()).unwrap();

        let validation = validate_written_job(
            &save_path,
            "company.volatile.tradeaux.berlin",
            "_nameless.offer.001",
            "cargo.trucks",
            "eurogoodies.hamburg",
        )
        .unwrap();

        assert!(validation.valid);
    }
}
