use super::discovery::scan_inventory;
use super::models::{
    ChangedPathEntry, CompareSummary, DiscoveredMod, DuplicateModEntry, ExtraModEntry, GameType,
    LoadOrderDifference, MissingModEntry, PresetCompareResult, PresetModEntry,
};
use super::presets;
use std::collections::{BTreeSet, HashMap, HashSet};
use tauri::AppHandle;

pub fn compare_preset(
    app: &AppHandle,
    profile_state: &crate::state::AppProfileState,
    preset_id: &str,
    game: GameType,
) -> Result<PresetCompareResult, String> {
    let preset = presets::find_preset(app, preset_id)?;
    if preset.game != game {
        return Err("The selected preset belongs to a different game.".to_string());
    }

    let inventory = scan_inventory(app, profile_state, Some(game.as_str()))?;
    let mut warnings = inventory.warnings.clone();
    let mut matched_current_indexes = HashSet::new();
    let mut missing_mods = Vec::new();
    let mut changed_paths = Vec::new();
    let mut load_order_differences = Vec::new();

    for preset_mod in &preset.mods {
        let matches = find_current_matches(preset_mod, &inventory.mods);
        if matches.is_empty() {
            missing_mods.push(MissingModEntry {
                preset_mod: preset_mod.clone(),
                reason: if preset_mod.workshop_id.is_some() {
                    "The workshop item is not present in the current local scan.".to_string()
                } else {
                    "The preset entry was not found in the current local mod scan.".to_string()
                },
                workshop_url: preset_mod
                    .workshop_id
                    .as_ref()
                    .map(|value| format!("https://steamcommunity.com/sharedfiles/filedetails/?id={}", value)),
            });
            continue;
        }

        let Some(current_index) = matches.first().copied() else {
            continue;
        };
        matched_current_indexes.insert(current_index);
        let current_mod = &inventory.mods[current_index];

        if normalize_path(&preset_mod.file_path) != normalize_path(&current_mod.file_path) {
            changed_paths.push(ChangedPathEntry {
                preset_mod: preset_mod.clone(),
                current_path: current_mod.file_path.clone(),
            });
        }

        if inventory.summary.active_mods_reliably_known
            && current_mod.load_order_index != Some(preset_mod.load_order_index)
        {
            load_order_differences.push(LoadOrderDifference {
                preset_mod: preset_mod.clone(),
                current_index: current_mod.load_order_index,
            });
        }
    }

    let comparison_basis = if inventory.summary.active_mods_reliably_known {
        inventory
            .mods
            .iter()
            .enumerate()
            .filter(|(_, item)| item.enabled == Some(true))
            .map(|(index, _)| index)
            .collect::<Vec<_>>()
    } else {
        warnings.push(
            "Current active mod list is not available. Extra mods are based on the readable installed inventory.".to_string(),
        );
        inventory
            .mods
            .iter()
            .enumerate()
            .filter(|(_, item)| item.readable && item.status != "invalid_workshop_item")
            .map(|(index, _)| index)
            .collect::<Vec<_>>()
    };

    let extra_mods = comparison_basis
        .into_iter()
        .filter(|index| !matched_current_indexes.contains(index))
        .map(|index| ExtraModEntry {
            current_mod: inventory.mods[index].clone(),
            reason: if inventory.summary.active_mods_reliably_known {
                "The mod is active now but not part of the preset.".to_string()
            } else {
                "The mod is installed locally but not part of the preset.".to_string()
            },
        })
        .collect::<Vec<_>>();

    let unreadable_mods = inventory
        .mods
        .iter()
        .filter(|item| !item.readable)
        .cloned()
        .collect::<Vec<_>>();

    let duplicate_mods = duplicate_mod_entries(&inventory.mods);
    let workshop_links = missing_mods
        .iter()
        .filter_map(|item| item.workshop_url.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();

    let summary = CompareSummary {
        missing_mods_count: missing_mods.len(),
        extra_mods_count: extra_mods.len(),
        changed_paths_count: changed_paths.len(),
        load_order_differences_count: load_order_differences.len(),
        unreadable_mods_count: unreadable_mods.len(),
        duplicate_mods_count: duplicate_mods.len(),
        workshop_links_count: workshop_links.len(),
        active_mods_reliably_known: inventory.summary.active_mods_reliably_known,
    };

    Ok(PresetCompareResult {
        preset,
        game,
        generated_at: chrono::Local::now().to_rfc3339(),
        missing_mods,
        extra_mods,
        changed_paths,
        load_order_differences,
        unreadable_mods,
        duplicate_mods,
        workshop_links,
        summary,
        warnings,
        load_order_source: inventory.summary.load_order_source,
    })
}

fn find_current_matches(preset_mod: &PresetModEntry, current_mods: &[DiscoveredMod]) -> Vec<usize> {
    let mut scored = current_mods
        .iter()
        .enumerate()
        .map(|(index, current_mod)| (index, preset_match_score(preset_mod, current_mod)))
        .filter(|(_, score)| *score > 0)
        .collect::<Vec<_>>();

    scored.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    scored.into_iter().map(|(index, _)| index).collect()
}

fn preset_match_score(preset_mod: &PresetModEntry, current_mod: &DiscoveredMod) -> i32 {
    let mut score = 0;

    if preset_mod.mod_id == current_mod.id {
        score += 120;
    }
    if preset_mod.workshop_id.is_some() && preset_mod.workshop_id == current_mod.workshop_id {
        score += 90;
    }
    if preset_mod.app_id.is_some() && preset_mod.app_id == current_mod.app_id {
        score += 10;
    }
    if normalize_token(&preset_mod.name) == normalize_token(&current_mod.name) {
        score += 40;
    }
    if normalize_path(&preset_mod.file_path) == normalize_path(&current_mod.file_path) {
        score += 60;
    }
    if normalize_file_name(&preset_mod.file_path) == normalize_file_name(&current_mod.file_path) {
        score += 35;
    }
    if normalize_token(&preset_mod.mod_id) == normalize_token(&current_mod.duplicate_key) {
        score += 55;
    }
    if preset_mod.source == current_mod.source {
        score += 10;
    }

    score
}

fn duplicate_mod_entries(current_mods: &[DiscoveredMod]) -> Vec<DuplicateModEntry> {
    let mut grouped: HashMap<String, Vec<&DiscoveredMod>> = HashMap::new();
    for item in current_mods {
        grouped.entry(item.duplicate_key.clone()).or_default().push(item);
    }

    grouped
        .into_iter()
        .filter_map(|(key, entries)| {
            if entries.len() < 2 {
                return None;
            }

            Some(DuplicateModEntry {
                mod_id: key,
                name: entries
                    .first()
                    .map(|item| item.name.clone())
                    .unwrap_or_else(|| "Unknown".to_string()),
                file_paths: entries.iter().map(|item| item.file_path.clone()).collect(),
            })
        })
        .collect()
}

fn normalize_path(value: &str) -> String {
    value.trim().replace('\\', "/").to_ascii_lowercase()
}

fn normalize_file_name(value: &str) -> String {
    value
        .rsplit(['\\', '/'])
        .next()
        .unwrap_or(value)
        .to_ascii_lowercase()
}

fn normalize_token(value: &str) -> String {
    value
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("_")
        .to_ascii_lowercase()
}
