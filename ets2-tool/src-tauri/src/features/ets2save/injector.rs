use std::path::{Path, PathBuf};

use crate::features::ets2save::errors::AppError;
use crate::features::ets2save::models::{
    EtsJobOfferPatch, PostWriteOfferSlotScan, PostWriteValidationResult, VtcDispatcherJob,
};
use crate::features::ets2save::parser::{
    UnitRange, extract_field_value, extract_in_game_time, find_company_block,
    find_job_offer_data_block, patch_job_offer_data, sii_token,
};
use crate::features::ets2save::post_write_validator::{select_offer_slot, validate_written_job};
use crate::features::ets2save::sii_codec::{decode_sii_lines, write_lines_atomic};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InjectionPointers {
    pub offer_pointer: String,
    pub job_offer_data_pointer: String,
    pub backup_path: Option<PathBuf>,
    pub job_info_updated: bool,
    pub offer_slot_index: usize,
    pub offer_slots: Vec<PostWriteOfferSlotScan>,
    pub validation: PostWriteValidationResult,
}

pub fn build_offer_patch(
    job: &VtcDispatcherJob,
    lines: &[String],
    offer_data_range: UnitRange,
    dst_company: &str,
    link_seed: &str,
) -> EtsJobOfferPatch {
    let in_game_time = extract_in_game_time(lines);
    let trailer_variant = extract_field_value(lines, offer_data_range, "trailer_variant");
    let trailer_definition = extract_field_value(lines, offer_data_range, "trailer_definition");
    let units_count = ((job.cargo_mass_kg / 4000.0).round() as i64).clamp(1, 12);
    let job_info_unit = format!("vtc.nameless.job.info.{}", link_seed.replace('-', "_"));

    EtsJobOfferPatch {
        target: format!(
            "{}.{}",
            sii_token(dst_company),
            sii_token(&job.destination_city)
        ),
        expiration_time: in_game_time + 6120,
        urgency: 0,
        shortest_distance_km: job.route_distance_km.round() as i64,
        ferry_time: 0,
        ferry_price: 0,
        cargo: format!("cargo.{}", sii_token(&job.cargo_type)),
        company_truck: job.job_type.eq_ignore_ascii_case("quick_job"),
        trailer_variant,
        trailer_definition,
        units_count,
        fill_ratio: 1,
        trailer_place: 0,
        job_info_unit: Some(job_info_unit.clone()),
        selected_job_unit: None,
    }
}

pub fn preview_offer_pointers(
    save_path: &Path,
    src_company: &str,
    src_city: &str,
) -> Result<(Vec<String>, usize, String, UnitRange), AppError> {
    let lines = decode_sii_lines(save_path)?;
    let company_block = find_company_block(&lines, src_company, src_city)?;
    let (offer_index, offer_pointer, _) = select_offer_slot(&lines, company_block)?;
    let offer_data_range = find_job_offer_data_block(&lines, &offer_pointer)?;
    Ok((lines, offer_index, offer_pointer, offer_data_range))
}

pub fn write_job_offer_patch(
    save_path: &Path,
    src_company: &str,
    src_city: &str,
    patch: &EtsJobOfferPatch,
) -> Result<InjectionPointers, AppError> {
    let mut lines = decode_sii_lines(save_path)?;
    let company_block = find_company_block(&lines, src_company, src_city)?;
    let (offer_slot_index, offer_pointer, offer_slots) = select_offer_slot(&lines, company_block)?;
    let offer_data_range = find_job_offer_data_block(&lines, &offer_pointer)?;

    patch_job_offer_data(&mut lines, offer_data_range, patch);
    upsert_job_info_unit(&mut lines, patch, &offer_pointer);
    upsert_selected_job(&mut lines, patch);
    ensure_selected_job_nil(&mut lines);
    let player_job_pointer = build_player_job_pointer();
    insert_player_job_state(
        &mut lines,
        &player_job_pointer,
        patch,
        src_company,
        src_city,
    );

    write_lines_atomic(save_path, &lines)?;
    let expected_company = format!(
        "company.volatile.{}.{}",
        sii_token(src_company),
        sii_token(src_city)
    );
    let mut validation = match validate_written_job(
        save_path,
        &expected_company,
        &offer_pointer,
        &patch.cargo,
        &patch.target,
    ) {
        Ok(validation) => validation,
        Err(error) => PostWriteValidationResult {
            expected_company,
            expected_offer_pointer: offer_pointer.clone(),
            expected_cargo: patch.cargo.clone(),
            expected_target: patch.target.clone(),
            selected_offer_slot_index: Some(offer_slot_index as i64),
            selected_offer_slot_pointer: Some(offer_pointer.clone()),
            root_cause: "write_corrupt".to_string(),
            validation_error_code: Some(error.code.as_key().to_string()),
            validation_error: Some(error.message),
            offer_slots: offer_slots.clone(),
            ..PostWriteValidationResult::default()
        },
    };
    if validation.offer_slots.is_empty() {
        validation.offer_slots = offer_slots.clone();
    }
    if validation.selected_offer_slot_index.is_none() {
        validation.selected_offer_slot_index = Some(offer_slot_index as i64);
    }
    if validation.selected_offer_slot_pointer.is_none() {
        validation.selected_offer_slot_pointer = Some(offer_pointer.clone());
    }

    Ok(InjectionPointers {
        offer_pointer: offer_pointer.clone(),
        job_offer_data_pointer: offer_pointer,
        backup_path: None,
        job_info_updated: patch.job_info_unit.is_some(),
        offer_slot_index,
        offer_slots,
        validation,
    })
}

fn upsert_job_info_unit(lines: &mut Vec<String>, patch: &EtsJobOfferPatch, offer_pointer: &str) {
    let Some(job_info_unit) = patch.job_info_unit.as_deref() else {
        return;
    };

    let header = format!("job_info : {} {{", job_info_unit);
    if let Some(start) = lines.iter().position(|line| line.trim() == header) {
        let range = find_inline_unit(lines, start);
        set_job_info_fields(lines, range, patch, offer_pointer);
        return;
    }

    let insert_at = lines.len().saturating_sub(1);
    let block = vec![
        header,
        format!(" job_offer_data: {}", offer_pointer),
        format!(" cargo: {}", patch.cargo),
        format!(" target: {}", patch.target),
        format!(" urgency: {}", patch.urgency),
        "}".to_string(),
    ];
    lines.splice(insert_at..insert_at, block);
}

fn upsert_selected_job(lines: &mut Vec<String>, patch: &EtsJobOfferPatch) {
    let Some(selected_job_unit) = patch.selected_job_unit.as_deref() else {
        return;
    };

    for line in lines.iter_mut() {
        if line.trim().starts_with("selected_job:") {
            *line = format!(" selected_job: {}", selected_job_unit);
            return;
        }
    }
}

fn set_job_info_fields(
    lines: &mut Vec<String>,
    range: UnitRange,
    patch: &EtsJobOfferPatch,
    offer_pointer: &str,
) {
    set_or_insert(lines, range, "job_offer_data", offer_pointer);
    set_or_insert(lines, range, "cargo", &patch.cargo);
    set_or_insert(lines, range, "target", &patch.target);
    set_or_insert(lines, range, "urgency", &patch.urgency.to_string());
}

fn set_or_insert(lines: &mut Vec<String>, range: UnitRange, field: &str, value: &str) {
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

    lines.insert(range.end, format!(" {}: {}", field, value));
}

fn ensure_selected_job_nil(lines: &mut Vec<String>) {
    for line in lines.iter_mut() {
        if line.trim().starts_with("selected_job:") {
            *line = " selected_job: nil".to_string();
            return;
        }
    }
    lines.push(" selected_job: nil".to_string());
}

fn build_player_job_pointer() -> String {
    let uuid = Uuid::new_v4().as_u128();
    let high = (uuid >> 64) as u64;
    let low = uuid as u64;
    format!("_nameless.{}.{}", high, low)
}

fn insert_player_job_state(
    lines: &mut Vec<String>,
    job_pointer: &str,
    patch: &EtsJobOfferPatch,
    src_company: &str,
    src_city: &str,
) {
    if let Some(range) = find_unit_range_with_prefix(lines, "player_job :") {
        lines.drain(range.start..=range.end);
    }

    let player_range = match find_player_block_range(lines) {
        Some(range) => range,
        None => return,
    };

    let truck_pointer = resolve_player_truck(lines, player_range);
    let trailer_pointer = resolve_player_trailer(lines, player_range);
    let trailer_ref = trailer_pointer.as_deref().filter(|value| *value != "null");
    let truck_ref = truck_pointer.as_deref().filter(|value| *value != "null");
    let truck_value = truck_ref.unwrap_or("null");
    let trailer_value = trailer_ref.unwrap_or("null");

    update_player_block(lines, player_range, job_pointer, trailer_value);

    let player_range = match find_player_block_range(lines) {
        Some(range) => range,
        None => return,
    };
    let insert_pos = player_range.end + 1;
    let block_lines = build_player_job_block_lines(
        job_pointer,
        truck_value,
        trailer_value,
        patch,
        src_company,
        src_city,
    );
    lines.splice(insert_pos..insert_pos, block_lines);
}

fn find_player_block_range(lines: &[String]) -> Option<UnitRange> {
    find_unit_range_with_prefix(lines, "player :")
}

fn find_unit_range_with_prefix(lines: &[String], prefix: &str) -> Option<UnitRange> {
    let mut start_index = None;
    let mut depth = 0_i32;
    for (index, line) in lines.iter().enumerate() {
        if start_index.is_none() && line.trim().starts_with(prefix) {
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

fn resolve_player_truck(lines: &[String], range: UnitRange) -> Option<String> {
    extract_player_field(lines, range, "assigned_truck")
        .or_else(|| extract_player_field(lines, range, "my_truck"))
        .or_else(|| extract_first_array_value(lines, range, "trucks["))
}

fn resolve_player_trailer(lines: &[String], range: UnitRange) -> Option<String> {
    extract_player_field(lines, range, "assigned_trailer")
        .or_else(|| extract_player_field(lines, range, "my_trailer"))
        .or_else(|| extract_first_array_value(lines, range, "trailers["))
}

fn extract_player_field(lines: &[String], range: UnitRange, field: &str) -> Option<String> {
    extract_field_value(lines, range, field).filter(|value| !value.eq_ignore_ascii_case("null"))
}

fn extract_first_array_value(lines: &[String], range: UnitRange, prefix: &str) -> Option<String> {
    for index in range.start..=range.end {
        let trimmed = lines[index].trim();
        if trimmed.starts_with(prefix) {
            if let Some((_, value)) = trimmed.split_once(':') {
                let trimmed_value = value.trim();
                if !trimmed_value.is_empty() && !trimmed_value.eq_ignore_ascii_case("nil") {
                    return Some(trimmed_value.to_string());
                }
            }
        }
    }
    None
}

fn update_player_block(
    lines: &mut Vec<String>,
    range: UnitRange,
    job_pointer: &str,
    trailer: &str,
) {
    set_or_insert(lines, range, "current_job", job_pointer);
    set_or_insert(lines, range, "assigned_trailer", trailer);
    set_or_insert(lines, range, "assigned_trailer_connected", "true");
    set_or_insert(lines, range, "my_trailer_attached", "true");
    set_or_insert(lines, range, "my_trailer", trailer);
}

fn build_player_job_block_lines(
    pointer: &str,
    truck: &str,
    trailer: &str,
    patch: &EtsJobOfferPatch,
    src_company: &str,
    src_city: &str,
) -> Vec<String> {
    let source_company = format!(
        "company.volatile.{}.{}",
        sii_token(src_company),
        sii_token(src_city)
    );
    vec![
        format!("player_job : {} {{", pointer),
        format!(" company_truck: {}", truck),
        format!(" company_trailer: {}", trailer),
        format!(" cargo: {}", patch.cargo),
        format!(" source_company: {}", source_company),
        format!(" target_company: {}", patch.target),
        format!(" planned_distance_km: {}", patch.shortest_distance_km),
        format!(" urgency: {}", patch.urgency),
        " is_cargo_market_job: false".to_string(),
        " is_trailer_loaded: true".to_string(),
        " autoload_used: true".to_string(),
        "}".to_string(),
    ]
}

fn find_inline_unit(lines: &[String], start: usize) -> UnitRange {
    let mut depth = 0_i32;
    for (offset, line) in lines[start..].iter().enumerate() {
        depth += line.matches('{').count() as i32;
        depth -= line.matches('}').count() as i32;
        if offset > 0 && depth <= 0 {
            return UnitRange {
                start,
                end: start + offset,
            };
        }
    }

    UnitRange {
        start,
        end: lines.len().saturating_sub(1),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{build_offer_patch, write_job_offer_patch};
    use crate::features::ets2save::models::VtcDispatcherJob;

    fn fixture_text() -> &'static str {
        "SiiNunit\n{\ncompany : company.volatile.test_company.berlin {\n job_offer: 1\n job_offer[0]: _nameless.offer.001\n}\njob_offer_data : _nameless.offer.001 {\n target: test_company.munich\n expiration_time: 100\n urgency: 1\n shortest_distance_km: 120\n cargo: cargo.old\n company_truck: false\n trailer_variant: original.variant\n trailer_definition: original.trailer\n units_count: 1\n fill_ratio: 1\n trailer_place: 0\n}\n selected_job: old.job.info\nplayer : _nameless.player {\n assigned_truck: _nameless.truck.1111.1111\n my_truck: _nameless.truck.1111.1111\n assigned_trailer: _nameless.trailer.2222.2222\n my_trailer: _nameless.trailer.2222.2222\n trailers: 1\n trailers[0]: _nameless.trailer.2222.2222\n}\n}\n"
    }

    #[test]
    fn injector_keeps_offer_count_and_replaces_fields() {
        let temp_dir = std::env::temp_dir().join("ets2_tool_injector_test");
        let _ = fs::create_dir_all(&temp_dir);
        let save_path = temp_dir.join("game.sii");
        fs::write(&save_path, fixture_text()).unwrap();

        let lines = crate::features::ets2save::sii_codec::split_lines(fixture_text());
        let job = VtcDispatcherJob {
            vtc_job_id: "job-1".to_string(),
            source_type: "generated".to_string(),
            company_id: "test_company".to_string(),
            company_name: "Test Company".to_string(),
            payment_tier: Some("standard".to_string()),
            job_type: "freight_market".to_string(),
            cargo_type: "trucks".to_string(),
            cargo_mass_kg: 12000.0,
            urgency_level: "normal".to_string(),
            difficulty_level: "normal".to_string(),
            equipment_type_required: "own_truck".to_string(),
            trailer_type_required: None,
            origin_city: "berlin".to_string(),
            origin_country: "de".to_string(),
            destination_city: "hamburg".to_string(),
            destination_country: "de".to_string(),
            route_distance_km: 520.0,
            estimated_duration_minutes: 360,
            base_rate_per_km: 1.0,
            calculated_rate_per_km: 1.4,
            total_reward: 728,
            profile_reference: None,
            quicksave_reference: None,
            save_reference: None,
            save_session_id: None,
            route_reference: None,
            dispatcher_status: None,
            last_error_code: None,
            last_error_message: None,
        };
        let offer_range = crate::features::ets2save::parser::find_job_offer_data_block(
            &lines,
            "_nameless.offer.001",
        )
        .unwrap();
        let patch = build_offer_patch(&job, &lines, offer_range, "test_company", "link-1");
        let _result = write_job_offer_patch(&save_path, "test_company", "berlin", &patch).unwrap();

        let written = fs::read_to_string(&save_path).unwrap();
        assert!(written.contains("job_offer: 1"));
        assert!(written.contains("target: test_company.hamburg"));
        assert!(written.contains("cargo: cargo.trucks"));
        assert!(written.contains("selected_job: nil"));
        assert!(written.contains("player_job :"));
        assert!(written.contains("company_truck: _nameless.truck.1111.1111"));
        assert!(written.contains("company_trailer: _nameless.trailer.2222.2222"));
    }
}
