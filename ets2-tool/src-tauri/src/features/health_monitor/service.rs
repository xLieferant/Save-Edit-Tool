use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::{Duration, UNIX_EPOCH};

use chrono::Utc;
use regex::Regex;
use serde::Deserialize;
use walkdir::WalkDir;

use crate::features::backup::service as backup_service;
use crate::features::logging::models::LogContext;
use crate::features::logging::service as logging_service;
use crate::shared::current_profile::{ResolvedSaveContext, snapshot_resolved_save_context};
use crate::shared::decrypt::{decrypt_cached_with_cache, decrypt_if_needed};
use crate::shared::paths::{game_sii_from_save, mod_directory_path};
use crate::shared::sii_parser::{
    get_player_id, get_vehicle_ids, parse_trailer_defs_from_sii, parse_trailers_from_sii,
    parse_trucks_from_sii,
};
use crate::shared::trace::{TraceScope, lock_mutex};
use crate::shared::user_log;
use crate::state::{AppProfileState, DecryptCache, ProfileCache};

use super::models::{SaveHealthFixResultDto, SaveHealthProblemDto, SaveHealthReportDto};

const FIX_SYNC_PLAYER_XP_LEVEL: &str = "sync_player_xp_level";
const COMMON_TOKENS: &[&str] = &[
    "accessory",
    "addon",
    "cargo",
    "data",
    "dds",
    "def",
    "ets2",
    "file",
    "game",
    "map",
    "material",
    "mod",
    "model",
    "prefab",
    "profile",
    "save",
    "sound",
    "texture",
    "trailer",
    "truck",
    "ui",
    "unit",
    "vehicle",
];
const KNOWN_CUSTOM_TOKENS: &[&str] = &[
    "jazzycat",
    "promods",
    "reforma",
    "rusmap",
    "soundfixes",
    "sierranevada",
    "schumi",
];
const MOD_SCAN_TIMEOUT_SECS: u64 = 8;
const MOD_SCAN_RETRY_AFTER_SECS: u64 = 30;
const MOD_SCAN_MAX_DEPTH: usize = 4;
const MOD_SCAN_MAX_ARCHIVE_SIZE_BYTES: u64 = 1024 * 1024 * 1024;

#[derive(Debug, Clone, Default)]
struct ActiveModEntry {
    raw: String,
    display_name: String,
    identifier: String,
    tokens: HashSet<String>,
}

#[derive(Debug, Clone, Default)]
struct IndexedMod {
    name: String,
    package_name: Option<String>,
    indexed_paths: Vec<String>,
    path_set: HashSet<String>,
    tokens: HashSet<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct LevelEntry {
    level: i64,
    total_xp: i64,
}

#[derive(Clone, Default)]
struct ModScanCacheEntry {
    fingerprint: Vec<String>,
    mods: Vec<IndexedMod>,
    path_index_complete: bool,
    state: ModScanState,
    message: Option<String>,
    updated_at_unix_ms: u128,
}

static MOD_SCAN_CACHE: OnceLock<Mutex<BTreeMap<PathBuf, ModScanCacheEntry>>> = OnceLock::new();
static MOD_SCAN_INFLIGHT: OnceLock<Mutex<BTreeSet<PathBuf>>> = OnceLock::new();

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum ModScanState {
    #[default]
    Ready,
    InProgress,
    TimedOut,
    Failed,
}

#[derive(Debug, Clone, Default)]
struct ModScanCollected {
    mods: Vec<IndexedMod>,
    entries_count: usize,
    path_index_complete: bool,
}

#[derive(Debug, Clone)]
struct ModScanOutcome {
    mods: Vec<IndexedMod>,
    state: ModScanState,
    path_index_complete: bool,
    message: Option<String>,
    pending: bool,
}

pub fn analyze_active_save_health(
    profile_state: &AppProfileState,
    decrypt_cache: &DecryptCache,
) -> Result<SaveHealthReportDto, String> {
    let context = logging_service::resolve_active_context(profile_state);
    let selected_game = context
        .selected_game
        .clone()
        .unwrap_or_else(|| "ets2".to_string());
    let resolved = snapshot_resolved_save_context(profile_state)
        .map_err(|error| format!("Failed to resolve active save context: {}", error))?;
    analyze_resolved_save_health(context, resolved, selected_game, decrypt_cache)
}

pub fn analyze_resolved_save_health(
    context: LogContext,
    resolved: ResolvedSaveContext,
    selected_game: String,
    decrypt_cache: &DecryptCache,
) -> Result<SaveHealthReportDto, String> {
    let generated_at_utc = Utc::now().to_rfc3339();
    let mut trace = TraceScope::with_fields(
        "health_monitor_init",
        &[(
            "save",
            resolved
                .context
                .save_reference
                .clone()
                .unwrap_or_else(|| "none".to_string()),
        )],
    );
    let mut problems = Vec::new();
    let Some(save_reference) = resolved.context.save_reference.as_deref() else {
        problems.push(problem(
            "no_active_save",
            "critical",
            "structure",
            "No active save is selected",
            "The health monitor cannot inspect save integrity until a save is selected.",
            "Load a profile and select a save before running repairs or restore operations.",
            false,
            None,
            Vec::new(),
        ));
        trace.finish_ok();
        return Ok(finalize_report(
            generated_at_utc,
            context.profile_name,
            context.save_name,
            problems,
            false,
            None,
        ));
    };

    let save_path = game_sii_from_save(Path::new(save_reference));
    let mut save_scope = TraceScope::with_fields(
        "health_monitor.read_game_sii",
        &[("path", save_path.display().to_string())],
    );
    let save_content = match decrypt_cached_with_cache(&save_path, decrypt_cache) {
        Ok(content) => content,
        Err(error) => {
            save_scope.finish_error(&error);
            problems.push(problem(
                "save_decode_failed",
                "critical",
                "structure",
                "The active save could not be decoded",
                    "The health monitor could not read `game.sii` for the active save.",
                "Restore a known-good backup or open the save in game and create a fresh quicksave.",
                false,
                None,
                vec![error],
            ));
            trace.finish_ok();
            return Ok(finalize_report(
                generated_at_utc,
                context.profile_name,
                context.save_name,
                problems,
                false,
                None,
            ));
        }
    };
    save_scope.finish_ok();

    if !save_content.starts_with("SiiNunit") {
        problems.push(problem(
            "save_header_invalid",
            "critical",
            "structure",
            "The active save does not look like a valid SII save",
            "The decoded file does not start with the expected `SiiNunit` header.",
            "Restore a known-good backup before editing the save again.",
            false,
            None,
            Vec::new(),
        ));
    }

    if !save_content.contains("economy :") {
        problems.push(problem(
            "economy_block_missing",
            "critical",
            "structure",
            "The economy block is missing",
            "The save is missing the central economy block that links player state and world data.",
            "Restore a backup or verify that the save was fully written by the game.",
            false,
            None,
            Vec::new(),
        ));
    }

    let player_id = get_player_id(&save_content);
    if player_id.is_none() {
        problems.push(problem(
            "player_block_missing",
            "critical",
            "structure",
            "The player reference is missing",
            "The health monitor could not find the active player reference in the save.",
            "Restore a backup before applying further edits.",
            false,
            None,
            Vec::new(),
        ));
    }

    if let Some(player_id) = player_id.as_deref() {
        let (player_truck_id, player_trailer_id) = get_vehicle_ids(&save_content, player_id);
        let mut truck_parse_scope = TraceScope::new("health_monitor.parse_trucks_from_sii");
        let trucks = parse_trucks_from_sii(&save_content);
        truck_parse_scope.finish_ok();
        let mut trailer_parse_scope = TraceScope::new("health_monitor.parse_trailers_from_sii");
        let trailers = parse_trailers_from_sii(&save_content);
        trailer_parse_scope.finish_ok();
        let mut trailer_def_scope = TraceScope::new("health_monitor.parse_trailer_defs_from_sii");
        let trailer_defs = parse_trailer_defs_from_sii(&save_content);
        trailer_def_scope.finish_ok();
        let vehicle_accessories = collect_block_ids(&save_content, "vehicle_accessory");

        if let Some(player_truck_id) = player_truck_id {
            let normalized_id = player_truck_id.trim().to_ascii_lowercase();
            if let Some(player_truck) = trucks
                .iter()
                .find(|truck| truck.truck_id.to_ascii_lowercase() == normalized_id)
            {
                let missing_accessories = collect_missing_accessories(
                    extract_array_values(&save_content, "accessories", "vehicle", &normalized_id),
                    &vehicle_accessories,
                );
                if !missing_accessories.is_empty() {
                    problems.push(problem(
                        "truck_accessories_missing",
                        "warning",
                        "accessories",
                        "The active truck references missing accessories",
                        "At least one accessory referenced by the active truck is not present in the save.",
                        "Restore a backup from before the accessory or truck mod change, or remove the broken truck setup in game.",
                        false,
                        None,
                        missing_accessories,
                    ));
                }

                if player_truck.brand.is_empty() && player_truck.model.is_empty() {
                    problems.push(problem(
                        "truck_definition_incomplete",
                        "warning",
                        "truck",
                        "The active truck definition could not be resolved cleanly",
                        "The truck exists in the save but its brand/model references are incomplete.",
                        "Review recent truck mod changes or restore a backup taken before the vehicle edit.",
                        false,
                        None,
                        vec![player_truck.truck_id.clone()],
                    ));
                }
            } else {
                problems.push(problem(
                    "player_truck_missing",
                    "critical",
                    "truck",
                    "The player truck reference is broken",
                    "The active player points to a truck unit that does not exist in the save.",
                    "Restore a backup from before the vehicle change or load a clean in-game save.",
                    false,
                    None,
                    vec![player_truck_id],
                ));
            }
        } else {
            problems.push(problem(
                "player_truck_unassigned",
                "warning",
                "truck",
                "The player truck reference is empty",
                "The save did not expose an active truck for the player.",
                "Open the save in game and assign a valid truck before editing vehicle data again.",
                false,
                None,
                Vec::new(),
            ));
        }

        if let Some(player_trailer_id) = player_trailer_id {
            let normalized_id = player_trailer_id.trim().to_ascii_lowercase();
            if let Some(trailer) = trailers
                .iter()
                .find(|item| item.trailer_id.to_ascii_lowercase() == normalized_id)
            {
                if !trailer_defs.contains_key(&trailer.trailer_definition) {
                    problems.push(problem(
                        "trailer_definition_missing",
                        "warning",
                        "trailer",
                        "The active trailer definition is missing",
                        "The player trailer exists, but its trailer definition could not be resolved inside the save.",
                        "Restore a backup from before the trailer or cargo mod change.",
                        false,
                        None,
                        vec![trailer.trailer_definition.clone()],
                    ));
                }
            } else {
                problems.push(problem(
                    "player_trailer_missing",
                    "critical",
                    "trailer",
                    "The player trailer reference is broken",
                    "The active player points to a trailer unit that does not exist in the save.",
                    "Restore a backup from before the trailer edit or detach the trailer in a clean in-game save.",
                    false,
                    None,
                    vec![player_trailer_id],
                ));
            }
        }
    }

    let profile_path = resolved
        .context
        .profile_reference
        .as_deref()
        .map(PathBuf::from);
    let profile_content = profile_path.as_ref().and_then(|path| {
        let profile_sii = path.join("profile.sii");
        let mut profile_scope = TraceScope::with_fields(
            "health_monitor.read_profile_sii",
            &[("path", profile_sii.display().to_string())],
        );
        let result = decrypt_cached_with_cache(&profile_sii, decrypt_cache).ok();
        if result.is_some() {
            profile_scope.finish_ok();
        } else {
            profile_scope.finish_error("profile.sii decode failed");
        }
        result
    });
    let active_mods = profile_content.as_deref().map_or_else(Vec::new, |content| {
        let mut parse_mods_scope = TraceScope::new("health_monitor.parse_active_mods");
        let mods = parse_active_mods(content);
        parse_mods_scope.finish_ok();
        mods
    });

    let mod_scan_outcome = if active_mods.is_empty() {
        ModScanOutcome {
            mods: Vec::new(),
            state: ModScanState::Ready,
            path_index_complete: false,
            message: None,
            pending: false,
        }
    } else {
        mod_directory_path(&selected_game)
            .as_deref()
            .map(|path| resolve_mod_scan_outcome(path))
            .unwrap_or(ModScanOutcome {
                mods: Vec::new(),
                state: ModScanState::Failed,
                path_index_complete: false,
                message: Some(
                    "The local mod directory could not be resolved for the active game."
                        .to_string(),
                ),
                pending: false,
            })
    };

    match mod_scan_outcome.state {
        ModScanState::Ready => {
            let missing_mods = active_mods
                .iter()
                .filter(|active_mod| {
                    !mod_scan_outcome
                        .mods
                        .iter()
                        .any(|item| active_mod_matches(active_mod, item))
                })
                .map(|item| item.display_name.clone())
                .collect::<Vec<_>>();
            if !missing_mods.is_empty() {
                problems.push(problem(
                    "missing_mods",
                    "warning",
                    "mods",
                    "The active profile references mods that are not installed locally",
                    "The save still depends on one or more active mods that the local mod folder does not provide.",
                    "Reinstall the missing mods or restore a backup from before the mod set changed.",
                    false,
                    None,
                    missing_mods,
                ));
            }

            if mod_scan_outcome.path_index_complete {
                let mut custom_ref_scope =
                    TraceScope::new("health_monitor.extract_custom_save_references");
                let save_custom_refs = extract_custom_save_references(&save_content, &active_mods);
                custom_ref_scope.finish_ok();
                let broken_asset_refs = save_custom_refs
                    .into_iter()
                    .filter(|path| !path_matches_any_mod(path, &mod_scan_outcome.mods))
                    .take(10)
                    .collect::<Vec<_>>();
                if !broken_asset_refs.is_empty() {
                    problems.push(problem(
                        "broken_mod_asset_references",
                        "warning",
                        "references",
                        "The save references mod assets that are no longer available",
                        "One or more custom truck, trailer, accessory or map assets referenced by the save were not found in the indexed local mods.",
                        "Restore a backup from before the mod change or reinstall the content that provided these assets.",
                        false,
                        None,
                        broken_asset_refs,
                    ));
                }
            }
        }
        ModScanState::InProgress => {
            problems.push(problem(
                "mod_scan_in_progress",
                "warning",
                "mods",
                "Mod scan is still running",
                "The health monitor started a background scan of the local mod folder and returned before it finished.",
                "Wait a moment and refresh the health monitor again to include local mod checks.",
                false,
                None,
                mod_scan_outcome
                    .message
                    .clone()
                    .into_iter()
                    .collect::<Vec<_>>(),
            ));
        }
        ModScanState::TimedOut => {
            problems.push(problem(
                "mod_scan_timed_out",
                "warning",
                "mods",
                "Mod scan timed out",
                "The health monitor skipped deep mod inspection because the local mod folder took too long to scan.",
                "Try a manual rescan after the UI loads. Very large or problematic archives should be reduced or removed.",
                false,
                None,
                mod_scan_outcome
                    .message
                    .clone()
                    .into_iter()
                    .collect::<Vec<_>>(),
            ));
        }
        ModScanState::Failed => {
            problems.push(problem(
                "mod_scan_failed",
                "warning",
                "mods",
                "Mod scan could not be completed",
                "The health monitor could not inspect the local mod directory for this save.",
                "Check the mod folder path and try a manual rescan after the UI loads.",
                false,
                None,
                mod_scan_outcome
                    .message
                    .clone()
                    .into_iter()
                    .collect::<Vec<_>>(),
            ));
        }
    }

    let xp_main = capture_number(&save_content, r"(?m)^\s*experience_points:\s*(\d+)");
    let xp_info = capture_number(&save_content, r"info_players_experience:\s*(\d+)");
    let level_info = capture_number(&save_content, r"info_player_level:\s*(\d+)");
    let derived_level = xp_main.or(xp_info).map(level_from_xp);

    if xp_main.is_some() && xp_info.is_some() && xp_main != xp_info {
        problems.push(problem(
            "xp_metadata_mismatch",
            "warning",
            "progression",
            "XP metadata is inconsistent",
            "The player XP stored in the main save block does not match the info summary value.",
            "Apply the safe fix to synchronize the info summary with the main player XP value.",
            true,
            Some(FIX_SYNC_PLAYER_XP_LEVEL.to_string()),
            vec![
                format!("experience_points={}", xp_main.unwrap_or_default()),
                format!("info_players_experience={}", xp_info.unwrap_or_default()),
            ],
        ));
    }

    if level_info.is_some() && derived_level.is_some() && level_info != derived_level {
        problems.push(problem(
            "level_metadata_mismatch",
            "warning",
            "progression",
            "Level metadata is inconsistent",
            "The info-level stored in the save does not match the level derived from current XP.",
            "Apply the safe fix to resynchronize XP and level metadata in the active save.",
            true,
            Some(FIX_SYNC_PLAYER_XP_LEVEL.to_string()),
            vec![
                format!("info_player_level={}", level_info.unwrap_or_default()),
                format!("derived_level={}", derived_level.unwrap_or_default()),
            ],
        ));
    }

    if let Some(report) = Some(finalize_report(
        generated_at_utc,
        context.profile_name.clone(),
        context.save_name.clone(),
        problems,
        mod_scan_outcome.pending,
        mod_scan_outcome.message.clone(),
    )) {
        if report.status != "Clean" {
            let mut log_context = context;
            log_context
                .extra
                .insert("status".to_string(), report.status.clone());
            log_context
                .extra
                .insert("problemCount".to_string(), report.problem_count.to_string());
            let _ = logging_service::record_warning(
                "save_health_check",
                Some("save_health_attention"),
                &report.summary,
                None,
                &log_context,
            );
        }
        trace.finish_ok();
        return Ok(report);
    }

    unreachable!()
}

pub fn apply_safe_fix(
    fix_id: &str,
    confirmed: bool,
    profile_state: &AppProfileState,
    profile_cache: &ProfileCache,
    decrypt_cache: &DecryptCache,
) -> Result<SaveHealthFixResultDto, String> {
    if !confirmed {
        return Err("Safe fixes require explicit confirmation.".to_string());
    }

    match fix_id {
        FIX_SYNC_PLAYER_XP_LEVEL => {
            apply_sync_player_xp_level_fix(profile_state, profile_cache, decrypt_cache)
        }
        _ => Err(format!("Unknown health fix `{}`.", fix_id)),
    }
}

fn apply_sync_player_xp_level_fix(
    profile_state: &AppProfileState,
    profile_cache: &ProfileCache,
    decrypt_cache: &DecryptCache,
) -> Result<SaveHealthFixResultDto, String> {
    let resolved = snapshot_resolved_save_context(profile_state)
        .map_err(|error| format!("Failed to resolve active save context: {}", error))?;
    let save_reference = resolved
        .context
        .save_reference
        .ok_or_else(|| "No active save was available for the safe fix.".to_string())?;
    let save_path = game_sii_from_save(Path::new(&save_reference));
    let content = decrypt_if_needed(&save_path)?;
    let xp_main = capture_number(&content, r"(?m)^\s*experience_points:\s*(\d+)")
        .or_else(|| capture_number(&content, r"info_players_experience:\s*(\d+)"))
        .ok_or_else(|| "The player XP fields could not be found in the active save.".to_string())?;
    let derived_level = level_from_xp(xp_main);

    let backup_targets = backup_service::recommended_targets(&save_path);
    backup_service::create_backup_for_targets(
        profile_state,
        "before health fix sync player xp/level",
        &backup_targets,
    )?;

    let xp_re = Regex::new(r"info_players_experience:\s*\d+").map_err(|error| error.to_string())?;
    let level_re = Regex::new(r"info_player_level:\s*\d+").map_err(|error| error.to_string())?;
    let new_content = level_re
        .replace(
            &xp_re.replace(&content, format!("info_players_experience: {}", xp_main)),
            format!("info_player_level: {}", derived_level),
        )
        .to_string();

    fs::write(&save_path, new_content.as_bytes()).map_err(|error| error.to_string())?;
    decrypt_cache.invalidate_path(&save_path);
    profile_cache.invalidate_save_data();
    profile_cache.invalidate_vehicle_data();

    let mut context = logging_service::resolve_active_context(profile_state);
    context
        .extra
        .insert("fixId".to_string(), FIX_SYNC_PLAYER_XP_LEVEL.to_string());
    context.extra.insert("xp".to_string(), xp_main.to_string());
    context
        .extra
        .insert("level".to_string(), derived_level.to_string());
    let _ = logging_service::record_info(
        "save_health_fix",
        "XP and level metadata were synchronized for the active save.",
        &context,
    );

    Ok(SaveHealthFixResultDto {
        fix_id: FIX_SYNC_PLAYER_XP_LEVEL.to_string(),
        applied: true,
        message: "XP and level metadata were synchronized for the active save.".to_string(),
    })
}

fn finalize_report(
    generated_at_utc: String,
    profile_name: Option<String>,
    save_name: Option<String>,
    problems: Vec<SaveHealthProblemDto>,
    mod_scan_pending: bool,
    mod_scan_message: Option<String>,
) -> SaveHealthReportDto {
    let fixable_count = problems
        .iter()
        .filter(|problem| problem.auto_fix_available)
        .count();
    let status = if problems
        .iter()
        .any(|problem| problem.severity == "critical")
    {
        "Broken"
    } else if problems.is_empty() {
        "Clean"
    } else {
        "Risky"
    }
    .to_string();

    let default_summary = match status.as_str() {
        "Clean" => "The active save passed the current structural, progression and mod reference checks.".to_string(),
        "Broken" => "The active save contains structural or reference errors that can break loading or vehicle ownership.".to_string(),
        _ => "The active save is still usable, but one or more risky inconsistencies were detected.".to_string(),
    };
    let summary = if mod_scan_pending {
        "Lightweight checks completed. The local mod scan is still running in the background."
            .to_string()
    } else if matches!(status.as_str(), "Clean" | "Risky") {
        mod_scan_message.clone().unwrap_or(default_summary)
    } else {
        default_summary
    };

    SaveHealthReportDto {
        generated_at_utc,
        status,
        profile_name,
        save_name,
        summary,
        problem_count: problems.len(),
        fixable_count,
        problems,
        mod_scan_pending,
        mod_scan_message,
    }
}

fn problem(
    id: &str,
    severity: &str,
    category: &str,
    title: &str,
    description: &str,
    suggestion: &str,
    auto_fix_available: bool,
    fix_id: Option<String>,
    evidence: Vec<String>,
) -> SaveHealthProblemDto {
    SaveHealthProblemDto {
        id: id.to_string(),
        severity: severity.to_string(),
        category: category.to_string(),
        title: title.to_string(),
        description: description.to_string(),
        suggestion: suggestion.to_string(),
        auto_fix_available,
        fix_id,
        evidence,
    }
}

fn capture_number(content: &str, pattern: &str) -> Option<i64> {
    let regex = Regex::new(pattern).ok()?;
    regex
        .captures(content)
        .and_then(|captures| captures.get(1))
        .and_then(|value| value.as_str().parse::<i64>().ok())
}

fn level_from_xp(xp: i64) -> i64 {
    let table: Vec<LevelEntry> =
        serde_json::from_str(include_str!("../../../../src/data/level-table.json"))
            .unwrap_or_default();
    let mut level = 0i64;
    for entry in table {
        if xp >= entry.total_xp {
            level = entry.level;
        } else {
            break;
        }
    }
    level
}

fn parse_active_mods(profile_content: &str) -> Vec<ActiveModEntry> {
    let regex = match Regex::new(r#"active_mods\[\d+\]:\s*"([^"]+)""#) {
        Ok(regex) => regex,
        Err(_) => return Vec::new(),
    };

    regex
        .captures_iter(profile_content)
        .filter_map(|capture| capture.get(1).map(|value| value.as_str().to_string()))
        .map(|raw| {
            let parts = raw
                .split('|')
                .map(|part| part.trim())
                .filter(|part| !part.is_empty())
                .collect::<Vec<_>>();
            let display_name = parts
                .last()
                .map(|value| (*value).to_string())
                .unwrap_or_else(|| raw.clone());
            let identifier = parts
                .first()
                .map(|value| (*value).to_string())
                .unwrap_or_else(|| raw.clone());

            let mut tokens = tokenize_to_set(&raw);
            tokens.extend(tokenize(&display_name));
            tokens.extend(tokenize(&identifier));

            ActiveModEntry {
                raw,
                display_name,
                identifier,
                tokens,
            }
        })
        .collect()
}

fn resolve_mod_scan_outcome(mod_dir: &Path) -> ModScanOutcome {
    let fingerprint = build_mod_dir_fingerprint(mod_dir);
    if let Some(cached) = cached_mod_scan_outcome(mod_dir, &fingerprint) {
        crate::dev_log!("[trace] MOD_SCAN cache_hit=true");
        return cached;
    }

    crate::dev_log!("[trace] MOD_SCAN cache_hit=false");

    let is_running = match lock_mutex("health_monitor.mod_scan_inflight", mod_scan_inflight()) {
        Ok(inflight) => inflight.contains(mod_dir),
        Err(error) => {
            return ModScanOutcome {
                mods: Vec::new(),
                state: ModScanState::Failed,
                path_index_complete: false,
                message: Some(error),
                pending: false,
            };
        }
    };

    if is_running {
        let _ = user_log::user_log_warn("HealthMonitor", "Mod scan already running.");
        return ModScanOutcome {
            mods: Vec::new(),
            state: ModScanState::InProgress,
            path_index_complete: false,
            message: Some("Mod scan already running".to_string()),
            pending: true,
        };
    }

    if let Err(error) = start_background_mod_scan(mod_dir.to_path_buf(), fingerprint) {
        return ModScanOutcome {
            mods: Vec::new(),
            state: ModScanState::Failed,
            path_index_complete: false,
            message: Some(error),
            pending: false,
        };
    }

    ModScanOutcome {
        mods: Vec::new(),
        state: ModScanState::InProgress,
        path_index_complete: false,
        message: Some("Mod scan started in background".to_string()),
        pending: true,
    }
}

fn cached_mod_scan_outcome(mod_dir: &Path, fingerprint: &[String]) -> Option<ModScanOutcome> {
    let now = unix_timestamp_ms();
    let cache = lock_mutex("health_monitor.mod_scan_cache", mod_scan_cache()).ok()?;
    let entry = cache.get(mod_dir)?;
    if entry.fingerprint != fingerprint {
        return None;
    }

    let recent_retry_window_ms = (MOD_SCAN_RETRY_AFTER_SECS as u128) * 1000;
    let age_ms = now.saturating_sub(entry.updated_at_unix_ms);
    let should_reuse = match entry.state {
        ModScanState::Ready => true,
        ModScanState::TimedOut | ModScanState::Failed => age_ms < recent_retry_window_ms,
        ModScanState::InProgress => false,
    };
    if !should_reuse {
        return None;
    }

    Some(ModScanOutcome {
        mods: entry.mods.clone(),
        state: entry.state,
        path_index_complete: entry.path_index_complete,
        message: entry.message.clone(),
        pending: false,
    })
}

fn start_background_mod_scan(mod_dir: PathBuf, fingerprint: Vec<String>) -> Result<(), String> {
    {
        let mut inflight = lock_mutex("health_monitor.mod_scan_inflight", mod_scan_inflight())?;
        if !inflight.insert(mod_dir.clone()) {
            return Ok(());
        }
    }

    thread::spawn(move || {
        let mut trace = TraceScope::with_fields(
            "health_monitor.scan_installed_mods",
            &[("path", mod_dir.display().to_string())],
        );
        let (sender, receiver) = mpsc::channel();
        let scan_path = mod_dir.clone();

        thread::spawn(move || {
            let result = scan_installed_mods(&scan_path);
            let _ = sender.send(result);
        });

        match receiver.recv_timeout(Duration::from_secs(MOD_SCAN_TIMEOUT_SECS)) {
            Ok(Ok(result)) => {
                crate::dev_log!("[trace] MOD_SCAN entries_count={}", result.entries_count);
                complete_mod_scan(&mod_dir, fingerprint, result);
                trace.finish_ok();
            }
            Ok(Err(error)) => {
                mark_mod_scan_failure(&mod_dir, fingerprint, error.clone());
                trace.finish_error(error);
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                crate::dev_log!(
                    "[trace] MOD_SCAN timeout after_ms={}",
                    Duration::from_secs(MOD_SCAN_TIMEOUT_SECS).as_millis()
                );
                mark_mod_scan_timeout(&mod_dir, fingerprint);
                trace.finish_ok();
            }
            Err(error) => {
                let message = format!("mod scan channel failed: {}", error);
                mark_mod_scan_failure(&mod_dir, fingerprint, message.clone());
                trace.finish_error(message);
            }
        }

        if let Ok(mut inflight) =
            lock_mutex("health_monitor.mod_scan_inflight", mod_scan_inflight())
        {
            inflight.remove(&mod_dir);
        }
    });

    Ok(())
}

fn complete_mod_scan(mod_dir: &Path, fingerprint: Vec<String>, result: ModScanCollected) {
    if let Ok(mut cache) = lock_mutex("health_monitor.mod_scan_cache", mod_scan_cache()) {
        cache.insert(
            mod_dir.to_path_buf(),
            ModScanCacheEntry {
                fingerprint,
                mods: result.mods,
                path_index_complete: result.path_index_complete,
                state: ModScanState::Ready,
                message: None,
                updated_at_unix_ms: unix_timestamp_ms(),
            },
        );
    }
}

fn mark_mod_scan_timeout(mod_dir: &Path, fingerprint: Vec<String>) {
    let _ = user_log::user_log_warn(
        "HealthMonitor",
        format!(
            "Mod scan timed out after {}ms.",
            Duration::from_secs(MOD_SCAN_TIMEOUT_SECS).as_millis()
        ),
    );
    if let Ok(mut cache) = lock_mutex("health_monitor.mod_scan_cache", mod_scan_cache()) {
        cache.insert(
            mod_dir.to_path_buf(),
            ModScanCacheEntry {
                fingerprint,
                mods: Vec::new(),
                path_index_complete: false,
                state: ModScanState::TimedOut,
                message: Some("Mod scan timed out. Try manual rescan.".to_string()),
                updated_at_unix_ms: unix_timestamp_ms(),
            },
        );
    }
}

fn mark_mod_scan_failure(mod_dir: &Path, fingerprint: Vec<String>, error: String) {
    let _ = user_log::user_log_error("HealthMonitor", format!("Mod scan failed: {}", error));
    if let Ok(mut cache) = lock_mutex("health_monitor.mod_scan_cache", mod_scan_cache()) {
        cache.insert(
            mod_dir.to_path_buf(),
            ModScanCacheEntry {
                fingerprint,
                mods: Vec::new(),
                path_index_complete: false,
                state: ModScanState::Failed,
                message: Some(error),
                updated_at_unix_ms: unix_timestamp_ms(),
            },
        );
    }
}

fn scan_installed_mods(mod_dir: &Path) -> Result<ModScanCollected, String> {
    if !mod_dir.exists() {
        return Ok(ModScanCollected::default());
    }

    let entries = fs::read_dir(mod_dir).map_err(|error| {
        format!(
            "failed to read mod directory `{}`: {}",
            mod_dir.display(),
            error
        )
    })?;

    let mut collected = ModScanCollected {
        mods: Vec::new(),
        entries_count: 0,
        path_index_complete: false,
    };

    for entry_result in entries {
        collected.entries_count += 1;
        let entry = match entry_result {
            Ok(entry) => entry,
            Err(error) => {
                crate::dev_log!(
                    "[trace] ERROR health_monitor.scan_installed_mods: {}",
                    error
                );
                continue;
            }
        };

        let path = entry.path();
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) => {
                crate::dev_log!(
                    "[trace] ERROR health_monitor.scan_installed_mods: {} ({})",
                    error,
                    path.display()
                );
                continue;
            }
        };

        if is_symlink_or_reparse(&metadata) {
            crate::dev_log!(
                "[trace] MOD_SCAN skipped_reparse_path path={}",
                path.display()
            );
            continue;
        }

        let is_archive = path
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| {
                matches!(
                    value.to_ascii_lowercase().as_str(),
                    "scs" | "zip" | "rar" | "7z"
                )
            })
            .unwrap_or(false);

        if !metadata.is_dir() && !is_archive {
            continue;
        }

        if is_archive {
            if metadata.len() > MOD_SCAN_MAX_ARCHIVE_SIZE_BYTES {
                crate::dev_log!(
                    "[trace] MOD_SCAN skipped_large_file path={} size_mb={}",
                    path.display(),
                    metadata.len() / (1024 * 1024)
                );
            }
            crate::dev_log!(
                "[trace] MOD_SCAN skipped_archive_deep_scan path={}",
                path.display()
            );
        }

        if let Some(indexed_mod) = inspect_mod_entry(&path, is_archive) {
            collected.mods.push(indexed_mod);
        }
    }

    Ok(collected)
}

fn inspect_mod_entry(path: &Path, is_archive: bool) -> Option<IndexedMod> {
    if is_archive {
        inspect_archive_mod_entry(path)
    } else {
        inspect_folder_mod_entry(path)
    }
}

fn inspect_folder_mod_entry(path: &Path) -> Option<IndexedMod> {
    let mut display_name = None;
    let mut package_name = None;
    let mut indexed_paths = Vec::new();

    for entry_result in WalkDir::new(path)
        .follow_links(false)
        .max_depth(MOD_SCAN_MAX_DEPTH)
        .into_iter()
    {
        let entry = match entry_result {
            Ok(entry) => entry,
            Err(error) => {
                crate::dev_log!(
                    "[trace] ERROR health_monitor.scan_installed_mods: {}",
                    error
                );
                continue;
            }
        };

        if entry.file_type().is_symlink() || !entry.file_type().is_file() {
            continue;
        }
        let relative = entry
            .path()
            .strip_prefix(path)
            .ok()?
            .to_string_lossy()
            .replace('\\', "/");
        let normalized = normalize_indexed_path(&relative);
        if normalized.ends_with("/manifest.sii") || normalized == "/manifest.sii" {
            if let Ok(content) = fs::read_to_string(entry.path()) {
                display_name = capture_manifest_value(&content, "display_name")
                    .or_else(|| capture_manifest_value(&content, "name"));
                package_name = capture_manifest_value(&content, "package_name")
                    .or_else(|| capture_manifest_value(&content, "name"));
            }
        }
        if is_relevant_indexed_path(&normalized) {
            indexed_paths.push(normalized);
        }
    }

    Some(build_indexed_mod(
        path,
        display_name,
        package_name,
        indexed_paths,
    ))
}

fn inspect_archive_mod_entry(path: &Path) -> Option<IndexedMod> {
    Some(build_indexed_mod(path, None, None, Vec::new()))
}

fn build_indexed_mod(
    path: &Path,
    display_name: Option<String>,
    package_name: Option<String>,
    indexed_paths: Vec<String>,
) -> IndexedMod {
    let file_stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .or_else(|| path.file_name().and_then(|value| value.to_str()))
        .unwrap_or("unknown_mod")
        .to_string();
    let name = display_name.unwrap_or_else(|| prettify_token(&file_stem));
    let mut tokens = tokenize_to_set(&name);
    tokens.extend(tokenize(&file_stem));
    if let Some(package_name) = package_name.as_deref() {
        tokens.extend(tokenize(package_name));
    }

    let mut path_set = HashSet::new();
    for item in &indexed_paths {
        path_set.insert(item.clone());
    }

    IndexedMod {
        name,
        package_name,
        indexed_paths,
        path_set,
        tokens,
    }
}

fn active_mod_matches(active_mod: &ActiveModEntry, indexed_mod: &IndexedMod) -> bool {
    if !active_mod.tokens.is_disjoint(&indexed_mod.tokens) {
        return true;
    }

    let active_aliases = [
        normalize_alias(&active_mod.display_name),
        normalize_alias(&active_mod.identifier),
        normalize_alias(&active_mod.raw),
    ];
    let indexed_aliases = [
        normalize_alias(&indexed_mod.name),
        normalize_alias(indexed_mod.package_name.as_deref().unwrap_or_default()),
    ];

    active_aliases.iter().any(|active| {
        !active.is_empty()
            && indexed_aliases.iter().any(|indexed| {
                !indexed.is_empty()
                    && (active == indexed
                        || active.contains(indexed.as_str())
                        || indexed.contains(active.as_str()))
            })
    })
}

fn extract_custom_save_references(
    save_content: &str,
    active_mods: &[ActiveModEntry],
) -> Vec<String> {
    let data_path_re = match Regex::new(r#"data_path:\s*"([^"]+)""#) {
        Ok(regex) => regex,
        Err(_) => return Vec::new(),
    };
    let asset_re = match Regex::new(
        r#"([A-Za-z0-9_/\.-]+\.(?:sii|sui|pmd|pmg|mat|tobj|dds|ogg|bank|unit))"#,
    ) {
        Ok(regex) => regex,
        Err(_) => return Vec::new(),
    };

    let mut references = BTreeSet::new();
    for capture in data_path_re.captures_iter(save_content) {
        if let Some(value) = capture.get(1) {
            let normalized = normalize_indexed_path(value.as_str());
            if looks_like_custom_reference(&normalized, active_mods) {
                references.insert(normalized);
            }
        }
    }
    for capture in asset_re.captures_iter(save_content) {
        if let Some(value) = capture.get(1) {
            let normalized = normalize_indexed_path(value.as_str());
            if looks_like_custom_reference(&normalized, active_mods) {
                references.insert(normalized);
            }
        }
    }

    references.into_iter().collect()
}

fn looks_like_custom_reference(path: &str, active_mods: &[ActiveModEntry]) -> bool {
    let tokens = tokenize_to_set(path);
    if tokens
        .iter()
        .any(|token| KNOWN_CUSTOM_TOKENS.contains(&token.as_str()))
    {
        return true;
    }

    active_mods
        .iter()
        .any(|active_mod| !active_mod.tokens.is_disjoint(&tokens))
}

fn path_matches_any_mod(path: &str, indexed_mods: &[IndexedMod]) -> bool {
    indexed_mods.iter().any(|indexed_mod| {
        indexed_mod.path_set.contains(path)
            || indexed_mod
                .indexed_paths
                .iter()
                .any(|candidate| trailing_segment_overlap(candidate, path) >= 2)
    })
}

fn collect_block_ids(content: &str, block_type: &str) -> HashSet<String> {
    let pattern = format!(
        r"{}\s*:\s*([A-Za-z0-9._-]+)\s*\{{",
        regex::escape(block_type)
    );
    let regex = match Regex::new(&pattern) {
        Ok(regex) => regex,
        Err(_) => return HashSet::new(),
    };
    regex
        .captures_iter(content)
        .filter_map(|capture| {
            capture
                .get(1)
                .map(|value| value.as_str().to_ascii_lowercase())
        })
        .collect()
}

fn extract_array_values(
    content: &str,
    field_name: &str,
    block_type: &str,
    block_id: &str,
) -> Vec<String> {
    let block_pattern = format!(
        r"{}\s*:\s*{}\s*\{{(?s)(.*?)\}}",
        regex::escape(block_type),
        regex::escape(block_id)
    );
    let block_re = match Regex::new(&block_pattern) {
        Ok(regex) => regex,
        Err(_) => return Vec::new(),
    };
    let field_re = match Regex::new(&format!(
        r#"{}\[\d+\]:\s*([^\s]+)"#,
        regex::escape(field_name)
    )) {
        Ok(regex) => regex,
        Err(_) => return Vec::new(),
    };

    let Some(captures) = block_re.captures(content) else {
        return Vec::new();
    };
    let Some(body) = captures.get(1) else {
        return Vec::new();
    };

    field_re
        .captures_iter(body.as_str())
        .filter_map(|capture| {
            capture
                .get(1)
                .map(|value| value.as_str().trim().to_ascii_lowercase())
        })
        .collect()
}

fn collect_missing_accessories(
    accessories: Vec<String>,
    available_accessories: &HashSet<String>,
) -> Vec<String> {
    accessories
        .into_iter()
        .filter(|item| !available_accessories.contains(item))
        .collect()
}

fn capture_manifest_value(text: &str, key: &str) -> Option<String> {
    let quoted = Regex::new(&format!(r#"{key}\s*:\s*"([^"]+)""#)).ok()?;
    if let Some(value) = quoted.captures(text).and_then(|capture| capture.get(1)) {
        return Some(value.as_str().to_string());
    }
    let bare = Regex::new(&format!(r#"{key}\s*:\s*([^\s]+)"#)).ok()?;
    bare.captures(text)
        .and_then(|capture| capture.get(1))
        .map(|value| value.as_str().trim_matches('"').to_string())
}

fn is_relevant_indexed_path(path: &str) -> bool {
    let normalized = path.to_ascii_lowercase();
    normalized.starts_with("/def/")
        || normalized.starts_with("/vehicle/")
        || normalized.starts_with("/model/")
        || normalized.starts_with("/material/")
        || normalized.starts_with("/map/")
        || normalized.starts_with("/ui/")
        || normalized.starts_with("/sound/")
        || normalized.starts_with("/prefab/")
        || normalized.ends_with(".sii")
        || normalized.ends_with(".sui")
        || normalized.ends_with(".pmd")
        || normalized.ends_with(".pmg")
        || normalized.ends_with(".mat")
        || normalized.ends_with(".tobj")
        || normalized.ends_with(".dds")
        || normalized.ends_with(".ogg")
        || normalized.ends_with(".bank")
        || normalized.ends_with(".unit")
}

fn normalize_indexed_path(path: &str) -> String {
    let mut normalized = path.trim().replace('\\', "/");
    while normalized.contains("//") {
        normalized = normalized.replace("//", "/");
    }
    normalized = normalized.trim_matches('"').to_string();
    if normalized.starts_with("./") {
        normalized = normalized.trim_start_matches("./").to_string();
    }
    if !normalized.starts_with('/') && !normalized.contains(':') {
        normalized = format!("/{}", normalized);
    }
    normalized.to_ascii_lowercase()
}

fn normalize_alias(value: &str) -> String {
    value
        .trim()
        .to_ascii_lowercase()
        .replace(['\\', '/', '-', '_', '|'], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn trailing_segment_overlap(left: &str, right: &str) -> usize {
    let left_segments = left
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();
    let right_segments = right
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();

    let mut overlap = 0usize;
    let mut left_iter = left_segments.iter().rev();
    let mut right_iter = right_segments.iter().rev();
    loop {
        match (left_iter.next(), right_iter.next()) {
            (Some(left_segment), Some(right_segment)) if left_segment == right_segment => {
                overlap += 1
            }
            _ => break,
        }
    }
    overlap
}

fn prettify_token(value: &str) -> String {
    value
        .replace(['_', '-'], " ")
        .split_whitespace()
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

fn tokenize_to_set(value: &str) -> HashSet<String> {
    tokenize(value).into_iter().collect()
}

fn tokenize(value: &str) -> Vec<String> {
    value
        .split(|character: char| !character.is_ascii_alphanumeric())
        .map(|part| part.trim().to_ascii_lowercase())
        .filter(|part| part.len() >= 3)
        .filter(|part| !COMMON_TOKENS.contains(&part.as_str()))
        .collect()
}

fn mod_scan_cache() -> &'static Mutex<BTreeMap<PathBuf, ModScanCacheEntry>> {
    MOD_SCAN_CACHE.get_or_init(|| Mutex::new(BTreeMap::new()))
}

fn mod_scan_inflight() -> &'static Mutex<BTreeSet<PathBuf>> {
    MOD_SCAN_INFLIGHT.get_or_init(|| Mutex::new(BTreeSet::new()))
}

fn build_mod_dir_fingerprint(mod_dir: &Path) -> Vec<String> {
    if !mod_dir.exists() {
        return Vec::new();
    }

    let Ok(entries) = fs::read_dir(mod_dir) else {
        return Vec::new();
    };

    let mut fingerprint = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let metadata = entry.metadata().ok();
        let modified = metadata
            .as_ref()
            .and_then(|item| item.modified().ok())
            .and_then(|value| value.duration_since(UNIX_EPOCH).ok())
            .map(|value| value.as_millis())
            .unwrap_or(0);
        let size = metadata.as_ref().map(|item| item.len()).unwrap_or(0);
        let kind = if path.is_dir() { "dir" } else { "file" };
        let name = path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        fingerprint.push(format!("{}|{}|{}|{}", name, kind, size, modified));
    }
    fingerprint.sort();
    fingerprint
}

fn unix_timestamp_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_millis())
        .unwrap_or(0)
}

#[cfg(windows)]
fn is_symlink_or_reparse(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;

    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
    metadata.file_type().is_symlink()
        || (metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT) != 0
}

#[cfg(not(windows))]
fn is_symlink_or_reparse(metadata: &fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}
