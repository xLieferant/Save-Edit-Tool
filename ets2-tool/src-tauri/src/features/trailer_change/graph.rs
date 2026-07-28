use std::collections::HashSet;

use crate::features::truck_change::parser::{
    UnitBlock, extract_array_values, normalize_sii_unit_id,
};

pub fn trailer_dangling_accessories(block: &UnitBlock, unit_ids: &HashSet<String>) -> Vec<String> {
    let mut missing = extract_array_values(&block.raw_block, "accessories")
        .into_iter()
        .filter(|value| value.starts_with("_nameless."))
        .filter(|value| {
            let normalized = normalize_sii_unit_id(value);
            !unit_ids
                .iter()
                .any(|unit_id| normalize_sii_unit_id(unit_id) == normalized)
        })
        .collect::<Vec<_>>();
    missing.sort();
    missing.dedup();
    missing
}
