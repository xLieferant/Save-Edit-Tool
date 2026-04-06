use std::path::{Path, PathBuf};

use crate::features::ets2save::errors::AppError;
use crate::features::ets2save::models::{EtsJobOfferPatch, VtcDispatcherJob};
use crate::features::ets2save::parser::{
    UnitRange, extract_field_value, extract_in_game_time, extract_job_offer_pointer,
    find_company_block, find_job_offer_data_block, patch_job_offer_data, sii_token,
};
use crate::features::ets2save::sii_codec::{decode_sii_lines, write_lines_atomic};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InjectionPointers {
    pub offer_pointer: String,
    pub job_offer_data_pointer: String,
    pub backup_path: PathBuf,
    pub job_info_updated: bool,
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
        selected_job_unit: Some(job_info_unit),
    }
}

pub fn preview_offer_pointers(
    save_path: &Path,
    src_company: &str,
    src_city: &str,
) -> Result<(Vec<String>, String, UnitRange), AppError> {
    let lines = decode_sii_lines(save_path)?;
    let company_block = find_company_block(&lines, src_company, src_city)?;
    let offer_pointer = extract_job_offer_pointer(&lines, company_block)?;
    let offer_data_range = find_job_offer_data_block(&lines, &offer_pointer)?;
    Ok((lines, offer_pointer, offer_data_range))
}

pub fn write_job_offer_patch(
    save_path: &Path,
    src_company: &str,
    src_city: &str,
    patch: &EtsJobOfferPatch,
) -> Result<InjectionPointers, AppError> {
    let mut lines = decode_sii_lines(save_path)?;
    let company_block = find_company_block(&lines, src_company, src_city)?;
    let offer_pointer = extract_job_offer_pointer(&lines, company_block)?;
    let offer_data_range = find_job_offer_data_block(&lines, &offer_pointer)?;

    patch_job_offer_data(&mut lines, offer_data_range, patch);
    upsert_job_info_unit(&mut lines, patch, &offer_pointer);
    upsert_selected_job(&mut lines, patch);

    let backup_path = write_lines_atomic(save_path, &lines)?;

    Ok(InjectionPointers {
        offer_pointer: offer_pointer.clone(),
        job_offer_data_pointer: offer_pointer,
        backup_path,
        job_info_updated: patch.job_info_unit.is_some(),
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
        "SiiNunit\n{\ncompany : company.volatile.test_company.berlin {\n job_offer: 1\n job_offer[0]: _nameless.offer.001\n}\njob_offer_data : _nameless.offer.001 {\n target: test_company.munich\n expiration_time: 100\n urgency: 1\n shortest_distance_km: 120\n cargo: cargo.old\n company_truck: false\n trailer_variant: original.variant\n trailer_definition: original.trailer\n units_count: 1\n fill_ratio: 1\n trailer_place: 0\n}\n selected_job: old.job.info\n}\n"
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
        assert!(written.contains("selected_job: vtc.nameless.job.info.link_1"));
    }
}
