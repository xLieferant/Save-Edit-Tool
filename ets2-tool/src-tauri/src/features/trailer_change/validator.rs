use super::graph::trailer_dangling_accessories;
use super::models::TrailerWriteValidation;
use super::parser::{
    find_trailer_block_by_id, parse_trailer_save, resolve_current_trailer_pointer,
};
use crate::features::truck_change::parser::normalize_sii_unit_id;

pub fn validate_trailer_switch_content(
    content: &str,
    expected_trailer_id: &str,
) -> TrailerWriteValidation {
    let parsed = parse_trailer_save(content);
    let actual_trailer_id = resolve_current_trailer_pointer(&parsed)
        .ok()
        .map(|pointer| pointer.trailer_id);
    let mut dangling_references = Vec::new();
    let mut errors = Vec::new();

    match actual_trailer_id.as_deref() {
        Some(actual)
            if normalize_sii_unit_id(actual) == normalize_sii_unit_id(expected_trailer_id) => {}
        Some(_) => errors.push("active_trailer_mismatch".to_string()),
        None => errors.push("active_trailer_not_found".to_string()),
    }

    if !player_trailers_contains(&parsed, expected_trailer_id) {
        errors.push("expected_trailer_missing_from_player_trailers".to_string());
    }

    match find_trailer_block_by_id(&parsed.trailer_blocks, expected_trailer_id) {
        Some(block) => {
            dangling_references.extend(trailer_dangling_accessories(block, &parsed.unit_ids));
            if !dangling_references.is_empty() {
                errors.push("dangling_trailer_references".to_string());
            }
        }
        None => errors.push("target_trailer_not_found".to_string()),
    }

    dangling_references.sort();
    dangling_references.dedup();
    errors.sort();
    errors.dedup();

    TrailerWriteValidation {
        success: errors.is_empty(),
        expected_trailer_id: expected_trailer_id.to_string(),
        actual_trailer_id,
        dangling_references,
        errors,
    }
}

pub fn player_trailers_contains(
    parsed: &super::parser::ParsedTrailerSave,
    trailer_id: &str,
) -> bool {
    let normalized = normalize_sii_unit_id(trailer_id);
    parsed
        .trailer_order
        .iter()
        .any(|candidate| normalize_sii_unit_id(candidate) == normalized)
        || parsed
            .active_trailer_id
            .as_deref()
            .map(|active| normalize_sii_unit_id(active) == normalized)
            .unwrap_or(false)
}
