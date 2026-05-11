use super::models::{
    AnalysisSources, AnalyzedError, AnalyzerLogPaths, AnalyzerOverview, CrashSummary,
    DiagnosticsContext, MissingReference, ModConflictAnalysisReport, SuspectedMod,
};
use crate::shared::current_profile::snapshot_resolved_save_context;
use crate::shared::decrypt::decrypt_if_needed;
use crate::shared::paths::{game_crash_path, game_log_path, game_sii_from_save, get_base_path, mod_directory_path};
use crate::shared::{logs, user_log};
use crate::state::{AppProfileState, DecryptCache};
use chrono::Local;
use regex::Regex;
use std::any::Any;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::fs;
use std::fs::File;
use std::io::Read;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;
use zip::ZipArchive;

const LAST_CONTEXT_COUNT: usize = 10;
const MAX_RELEVANT_LOG_LINES: usize = 160;
const MAX_RELEVANT_CRASH_LINES: usize = 80;
const MAX_SUSPECTED_MODS: usize = 12;

const COMMON_TOKENS: &[&str] = &[
    "accessory",
    "addon",
    "bank",
    "cargo",
    "data",
    "dds",
    "def",
    "definition",
    "ets2",
    "file",
    "game",
    "log",
    "map",
    "material",
    "mod",
    "mods",
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
    "apemods",
    "jazzycat",
    "local_mods",
    "promods",
    "reforma",
    "rusmap",
    "schumi",
    "sierranevada",
    "soundfixes",
];

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
    file_path: String,
    readable: bool,
    manifest_present: bool,
    category_hints: BTreeSet<String>,
    label_hints: BTreeSet<String>,
    indexed_paths: Vec<String>,
    path_set: HashSet<String>,
    file_names: HashSet<String>,
    tokens: HashSet<String>,
    active_state: String,
}

#[derive(Debug, Clone, Default)]
struct ManifestSummary {
    display_name: Option<String>,
    package_name: Option<String>,
    compatible_versions: Vec<String>,
}

#[derive(Debug, Clone, Default)]
struct MatchSignals {
    exact_path_match: bool,
    partial_path_match: bool,
    category_match: bool,
    active_match: bool,
    crash_context_match: bool,
    label_hint_match: bool,
}

#[derive(Debug, Clone, Default)]
struct CandidateScore {
    score: i32,
    exact_path_match: bool,
    matched_paths: BTreeSet<String>,
    reasons: Vec<String>,
}

#[derive(Debug, Clone)]
struct OptionalText {
    content: Option<String>,
    found: bool,
    path: Option<String>,
}

impl OptionalText {
    fn missing(path: Option<&Path>) -> Self {
        Self {
            content: None,
            found: false,
            path: path.map(path_to_string),
        }
    }
}

fn compile_regex(pattern: &str, context: &str) -> Option<Regex> {
    match Regex::new(pattern) {
        Ok(regex) => Some(regex),
        Err(error) => {
            crate::dev_log!("[diagnostics] regex compile failed in {}: {}", context, error);
            None
        }
    }
}

fn panic_message(payload: Box<dyn Any + Send>) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        return (*message).to_string();
    }
    if let Some(message) = payload.downcast_ref::<String>() {
        return message.clone();
    }
    "unknown panic payload".to_string()
}

fn log_user_issue(message: &str) {
    if let Err(error) = user_log::write_user_log(&format!("mod_conflict_analyzer | {}", message), "error") {
        crate::dev_log!("[diagnostics] user log write failed: {}", error);
    }
}

fn record_limitation(limitations: &mut Vec<String>, message: impl Into<String>, user_visible: bool) {
    let message = message.into();
    crate::dev_log!("[diagnostics] limitation: {}", message);
    if user_visible {
        log_user_issue(&message);
    }
    limitations.push(message);
}

fn resolve_profile_sii_path(profile_path: Option<&str>, limitations: &mut Vec<String>) -> Option<PathBuf> {
    let Some(profile_path) = profile_path else {
        record_limitation(
            limitations,
            "No active profile path is available for the Mod Conflict Analyzer.",
            false,
        );
        return None;
    };

    let candidate = Path::new(profile_path).join("profile.sii");
    if candidate.exists() {
        return Some(candidate);
    }

    record_limitation(
        limitations,
        format!("Profile path is invalid or missing `profile.sii`: {}", profile_path),
        false,
    );
    None
}

fn resolve_save_sii_path(save_path: Option<&str>, limitations: &mut Vec<String>) -> Option<PathBuf> {
    let Some(save_path) = save_path else {
        record_limitation(
            limitations,
            "No active save is available for the Mod Conflict Analyzer.",
            false,
        );
        return None;
    };

    let candidate = game_sii_from_save(Path::new(save_path));
    if candidate.exists() {
        return Some(candidate);
    }

    record_limitation(
        limitations,
        format!(
            "Analyzer could not find `game.sii` for the active save at {}",
            candidate.display()
        ),
        false,
    );
    None
}

fn read_plain_text_lossy(label: &str, path: &Path) -> Result<String, String> {
    let bytes = fs::read(path).map_err(|error| {
        format!(
            "plain_text_read_failed | source={} | label={} | reason={}",
            path.display(),
            label,
            error
        )
    })?;

    match String::from_utf8(bytes) {
        Ok(content) => Ok(content),
        Err(error) => {
            crate::dev_log!(
                "[diagnostics] {} contains invalid UTF-8. Falling back to lossy decoding for {}",
                label,
                path.display()
            );
            Ok(String::from_utf8_lossy(&error.into_bytes()).into_owned())
        }
    }
}

pub fn analyze_mod_conflict_diagnostics(
    profile_state: &AppProfileState,
    decrypt_cache: &DecryptCache,
) -> Result<ModConflictAnalysisReport, String> {
    crate::dev_log!("[diagnostics] analysis start");
    if let Err(error) = user_log::write_user_log("mod_conflict_analyzer opened", "start") {
        crate::dev_log!("[diagnostics] user log write failed: {}", error);
    }

    let generated_at = Local::now().to_rfc3339();
    let selected_game = profile_state
        .selected_game
        .lock()
        .map_err(|_| "selected_game lock poisoned".to_string())?
        .clone();
    let resolved_context = snapshot_resolved_save_context(profile_state)
        .map_err(|error| format!("Failed to resolve active save context: {}", error))?;

    let mut limitations = Vec::new();
    let base_path = get_base_path(&selected_game);
    let log_path = game_log_path(&selected_game);
    let crash_path = game_crash_path(&selected_game);
    let mod_path = mod_directory_path(&selected_game);
    let profile_sii_path = resolve_profile_sii_path(
        resolved_context.context.profile_reference.as_deref(),
        &mut limitations,
    );
    let game_sii_path = resolve_save_sii_path(
        resolved_context.context.save_reference.as_deref(),
        &mut limitations,
    );

    crate::dev_log!(
        "[diagnostics] selected_game={} base={:?} game_log={:?} game_crash={:?} mod_dir={:?}",
        selected_game,
        base_path,
        log_path,
        crash_path,
        mod_path
    );

    let game_log = read_optional_text(
        "game.log.txt",
        log_path.as_deref(),
        decrypt_cache,
        &mut limitations,
        false,
    );
    let game_crash = read_optional_text(
        "game.crash.txt",
        crash_path.as_deref(),
        decrypt_cache,
        &mut limitations,
        false,
    );
    let profile_sii = read_optional_text(
        "profile.sii",
        profile_sii_path.as_deref(),
        decrypt_cache,
        &mut limitations,
        false,
    );
    let game_sii = read_optional_text(
        "game.sii",
        game_sii_path.as_deref(),
        decrypt_cache,
        &mut limitations,
        false,
    );

    let active_mods = profile_sii
        .content
        .as_deref()
        .map(parse_active_mods)
        .unwrap_or_default();
    let active_mods_reliably_known = profile_sii.content.is_some();

    let mut indexed_mods = mod_path
        .as_deref()
        .map(|path| scan_installed_mods(path, &mut limitations))
        .unwrap_or_default();
    let mod_folder_found = mod_path.as_deref().map(Path::exists).unwrap_or(false);
    apply_active_states(&mut indexed_mods, &active_mods, active_mods_reliably_known);

    crate::dev_log!(
        "[diagnostics] sources loaded active_mods={} indexed_mods={} readable_mods={} unreadable_mods={}",
        active_mods.len(),
        indexed_mods.len(),
        indexed_mods.iter().filter(|item| item.readable).count(),
        indexed_mods.iter().filter(|item| !item.readable).count()
    );

    let mut log_errors = game_log
        .content
        .as_deref()
        .map(|content| extract_log_errors("game.log.txt", content))
        .unwrap_or_default();
    let mut crash_errors = game_crash
        .content
        .as_deref()
        .map(|content| extract_crash_errors("game.crash.txt", content))
        .unwrap_or_default();

    mark_last_context(&mut log_errors, LAST_CONTEXT_COUNT);
    mark_last_context(&mut crash_errors, LAST_CONTEXT_COUNT);

    let mut errors = Vec::new();
    errors.extend(log_errors.iter().cloned());
    errors.extend(crash_errors.iter().cloned());

    let mut missing_references = build_missing_active_mod_references(&active_mods, &indexed_mods);
    let mut save_state_errors = Vec::new();
    if let Some(content) = game_sii.content.as_deref() {
        let custom_save_refs = extract_custom_save_references(content, &active_mods);
        let (save_missing_refs, save_errors) =
            build_save_missing_references(&custom_save_refs, &indexed_mods);
        missing_references.extend(save_missing_refs);
        save_state_errors.extend(save_errors);
    }

    let (log_missing_refs, log_missing_errors) =
        build_unmatched_path_references(&errors, &indexed_mods);
    missing_references.extend(log_missing_refs);
    save_state_errors.extend(log_missing_errors);

    errors.extend(save_state_errors);
    deduplicate_missing_references(&mut missing_references);
    sort_errors(&mut errors);

    let suspected_mods = rank_suspected_mods(
        &indexed_mods,
        &errors,
        active_mods_reliably_known,
    );

    let removed_mod_suspected = suspected_mods.is_empty()
        && missing_references
            .iter()
            .any(|item| item.category != "ActiveModList");
    let removed_mod_reason = if removed_mod_suspected {
        Some(
            "The log references assets that are not provided by any indexed local mod. This can happen when a mod was removed but the save still references truck, trailer, accessory or map content."
                .to_string(),
        )
    } else {
        None
    };

    let sources = AnalysisSources {
        game_log_found: game_log.found,
        game_log_path: game_log.path,
        game_crash_found: game_crash.found,
        game_crash_path: game_crash.path,
        mod_folder_found,
        mod_folder_path: mod_path.as_deref().map(path_to_string),
        indexed_mods_count: indexed_mods.len(),
        readable_mods_count: indexed_mods.iter().filter(|item| item.readable).count(),
        unreadable_mods_count: indexed_mods.iter().filter(|item| !item.readable).count(),
        active_mods_count: active_mods.len(),
        active_mods_reliably_known,
        extracted_errors_count: errors.len(),
    };

    let crash_summary = build_crash_summary(&errors, game_crash.found);
    let overview = build_overview(
        &sources,
        &crash_summary,
        &suspected_mods,
        &missing_references,
        removed_mod_suspected,
        &limitations,
    );

    let raw_relevant_log_lines = log_errors
        .iter()
        .map(render_raw_line)
        .collect::<Vec<_>>();
    let raw_relevant_crash_lines = crash_errors
        .iter()
        .map(render_raw_line)
        .collect::<Vec<_>>();
    let active_mod_names = active_mods
        .iter()
        .map(|item| item.display_name.clone())
        .collect::<Vec<_>>();
    let logs = AnalyzerLogPaths {
        technical_log_path: Some(path_to_string(&logs::technical_log_path())),
        user_log_path: Some(path_to_string(&user_log::user_log_path())),
        log_directory_path: logs::log_directory_path().map(|path| path_to_string(&path)),
    };

    crate::dev_log!(
        "[diagnostics] analysis complete status={} errors={} missing_refs={} suspects={}",
        overview.status_badge,
        errors.len(),
        missing_references.len(),
        suspected_mods.len()
    );
    if let Err(error) = user_log::write_user_log(
        &format!(
            "mod_conflict_analyzer success | indexed_mods={} errors={} suspected_mods={}",
            indexed_mods.len(),
            errors.len(),
            suspected_mods.len()
        ),
        "success",
    ) {
        crate::dev_log!("[diagnostics] user log write failed: {}", error);
    }

    Ok(ModConflictAnalysisReport {
        generated_at,
        report_version: "mod-conflict-analyzer.mvp.v1".to_string(),
        context: DiagnosticsContext {
            selected_game,
            base_path: base_path.as_deref().map(path_to_string),
            profile_path: resolved_context.context.profile_reference,
            profile_inferred: resolved_context.profile_inferred,
            save_path: resolved_context.context.save_reference,
            save_inferred: resolved_context.save_inferred,
        },
        sources,
        overview,
        crash_summary,
        active_mods: active_mod_names,
        suspected_mods,
        missing_references,
        errors,
        removed_mod_suspected,
        removed_mod_reason,
        logs,
        raw_relevant_log_lines,
        raw_relevant_crash_lines,
        limitations,
    })
}

fn read_optional_text(
    label: &str,
    path: Option<&Path>,
    decrypt_cache: &DecryptCache,
    limitations: &mut Vec<String>,
    user_visible: bool,
) -> OptionalText {
    let Some(path) = path else {
        crate::dev_log!("[diagnostics] {} path unavailable", label);
        return OptionalText::missing(None);
    };

    crate::dev_log!("[diagnostics] reading {} from {}", label, path.display());
    if !path.exists() {
        let message = match label {
            "game.log.txt" => format!("No game.log.txt found at {}", path.display()),
            "game.crash.txt" => format!("No game.crash.txt found at {}", path.display()),
            "profile.sii" => format!("No profile.sii found at {}", path.display()),
            "game.sii" => format!("No game.sii found at {}", path.display()),
            _ => format!("Required file is missing: {}", path.display()),
        };
        record_limitation(limitations, message, user_visible);
        return OptionalText::missing(Some(path));
    }

    if let Ok(cache) = decrypt_cache.files.lock() {
        if let Some(content) = cache.get(path).cloned() {
            crate::dev_log!("[diagnostics] {} cache hit ({} chars)", label, content.len());
            return OptionalText {
                content: Some(content),
                found: true,
                path: Some(path_to_string(path)),
            };
        }
    }

    let result = match label {
        "game.log.txt" | "game.crash.txt" => read_plain_text_lossy(label, path),
        _ => decrypt_if_needed(path),
    };

    match result {
        Ok(content) => {
            if let Ok(mut cache) = decrypt_cache.files.lock() {
                cache.insert(path.to_path_buf(), content.clone());
            }
            crate::dev_log!("[diagnostics] {} loaded ({} chars)", label, content.len());
            OptionalText {
                content: Some(content),
                found: true,
                path: Some(path_to_string(path)),
            }
        }
        Err(error) => {
            let message = match label {
                "game.log.txt" => format!("Could not read game.log.txt: {}", error),
                "game.crash.txt" => format!("Could not read game.crash.txt: {}", error),
                "profile.sii" => format!("Could not decode profile.sii: {}", error),
                "game.sii" => format!("Could not decode game.sii: {}", error),
                _ => format!("Could not decode {}: {}", path.display(), error),
            };
            record_limitation(limitations, message, user_visible);
            OptionalText::missing(Some(path))
        }
    }
}

fn extract_log_errors(source: &str, content: &str) -> Vec<AnalyzedError> {
    let mut errors = content
        .lines()
        .enumerate()
        .filter_map(|(index, line)| build_error_from_line(source, index + 1, line, false))
        .collect::<Vec<_>>();
    if errors.len() > MAX_RELEVANT_LOG_LINES {
        let keep_from = errors.len().saturating_sub(MAX_RELEVANT_LOG_LINES);
        errors = errors.split_off(keep_from);
    }
    errors
}

fn extract_crash_errors(source: &str, content: &str) -> Vec<AnalyzedError> {
    let mut entries = content
        .lines()
        .enumerate()
        .filter_map(|(index, line)| build_error_from_line(source, index + 1, line, true))
        .collect::<Vec<_>>();

    if entries.is_empty() {
        let mut fallback = content
            .lines()
            .enumerate()
            .filter_map(|(index, line)| {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    return None;
                }
                Some(AnalyzedError {
                    source: source.to_string(),
                    severity: "Critical".to_string(),
                    category: "CrashContext".to_string(),
                    line_number: Some(index + 1),
                    raw_line: trimmed.to_string(),
                    extracted_path: extract_path_from_line(trimmed),
                    explanation: "This line comes directly from game.crash.txt and provides crash context, but not proof of a single culprit mod."
                        .to_string(),
                    in_last_context: false,
                })
            })
            .collect::<Vec<_>>();
        if fallback.len() > MAX_RELEVANT_CRASH_LINES {
            let keep_from = fallback.len().saturating_sub(MAX_RELEVANT_CRASH_LINES);
            fallback = fallback.split_off(keep_from);
        }
        return fallback;
    }

    if entries.len() > MAX_RELEVANT_CRASH_LINES {
        let keep_from = entries.len().saturating_sub(MAX_RELEVANT_CRASH_LINES);
        entries = entries.split_off(keep_from);
    }
    entries
}

fn build_error_from_line(
    source: &str,
    line_number: usize,
    line: &str,
    crash_mode: bool,
) -> Option<AnalyzedError> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }

    if crash_mode {
        if !is_relevant_crash_line(trimmed) && extract_path_from_line(trimmed).is_none() {
            return None;
        }
    } else if !is_relevant_log_line(trimmed) {
        return None;
    }

    let extracted_path = extract_path_from_line(trimmed);
    let category = classify_error_category(trimmed, extracted_path.as_deref(), crash_mode);
    let severity = classify_error_severity(trimmed, crash_mode);
    let explanation = explanation_for_category(&category, extracted_path.as_deref(), crash_mode);

    Some(AnalyzedError {
        source: source.to_string(),
        severity,
        category,
        line_number: Some(line_number),
        raw_line: trimmed.to_string(),
        extracted_path,
        explanation,
        in_last_context: false,
    })
}

fn mark_last_context(errors: &mut [AnalyzedError], count: usize) {
    let start = errors.len().saturating_sub(count);
    for item in errors.iter_mut().skip(start) {
        item.in_last_context = true;
    }
}

fn build_missing_active_mod_references(
    active_mods: &[ActiveModEntry],
    indexed_mods: &[IndexedMod],
) -> Vec<MissingReference> {
    let mut references = Vec::new();
    for active_mod in active_mods {
        let matched = indexed_mods
            .iter()
            .any(|indexed_mod| active_mod_matches_indexed_mod(active_mod, indexed_mod));
        if matched {
            continue;
        }

        references.push(MissingReference {
            path: active_mod.display_name.clone(),
            category: "ActiveModList".to_string(),
            source: "profile.sii".to_string(),
            reason: "The active profile still references this mod, but no indexed local mod entry matched it.".to_string(),
        });
    }
    references
}

fn build_save_missing_references(
    custom_save_refs: &[String],
    indexed_mods: &[IndexedMod],
) -> (Vec<MissingReference>, Vec<AnalyzedError>) {
    let mut refs = Vec::new();
    let mut errors = Vec::new();

    for asset_path in custom_save_refs {
        if path_matches_any_mod(asset_path, indexed_mods) {
            continue;
        }

        let category = classify_path_category(asset_path, None, false);
        let reason = format!(
            "The active save still references `{}` but no indexed local mod provides that path.",
            asset_path
        );
        refs.push(MissingReference {
            path: asset_path.clone(),
            category: category.clone(),
            source: "game.sii".to_string(),
            reason: reason.clone(),
        });
        errors.push(AnalyzedError {
            source: "game.sii".to_string(),
            severity: "Warning".to_string(),
            category,
            line_number: None,
            raw_line: asset_path.clone(),
            extracted_path: Some(asset_path.clone()),
            explanation: reason,
            in_last_context: false,
        });
    }

    (refs, errors)
}

fn build_unmatched_path_references(
    errors: &[AnalyzedError],
    indexed_mods: &[IndexedMod],
) -> (Vec<MissingReference>, Vec<AnalyzedError>) {
    let mut refs = Vec::new();
    let mut synthetic_errors = Vec::new();

    for item in errors {
        let Some(path) = item.extracted_path.as_deref() else {
            continue;
        };
        if path_matches_any_mod(path, indexed_mods) {
            continue;
        }
        if item.source != "game.log.txt" && item.source != "game.crash.txt" {
            continue;
        }

        let reason = "The log references assets that are not provided by any indexed local mod. This can happen when a mod was removed but the save still references truck, trailer, accessory or map content."
            .to_string();
        refs.push(MissingReference {
            path: path.to_string(),
            category: item.category.clone(),
            source: item.source.clone(),
            reason: reason.clone(),
        });

        if item.in_last_context {
            synthetic_errors.push(AnalyzedError {
                source: item.source.clone(),
                severity: item.severity.clone(),
                category: "UnknownReference".to_string(),
                line_number: item.line_number,
                raw_line: item.raw_line.clone(),
                extracted_path: Some(path.to_string()),
                explanation: reason,
                in_last_context: true,
            });
        }
    }

    (refs, synthetic_errors)
}

fn deduplicate_missing_references(references: &mut Vec<MissingReference>) {
    let mut seen = HashSet::new();
    references.retain(|item| {
        let key = format!("{}|{}|{}", item.source, item.category, item.path);
        seen.insert(key)
    });
}

fn sort_errors(errors: &mut [AnalyzedError]) {
    errors.sort_by(|left, right| {
        severity_sort_key(&right.severity)
            .cmp(&severity_sort_key(&left.severity))
            .then_with(|| left.source.cmp(&right.source))
            .then_with(|| left.line_number.cmp(&right.line_number))
    });
}

fn rank_suspected_mods(
    indexed_mods: &[IndexedMod],
    errors: &[AnalyzedError],
    active_mods_reliably_known: bool,
) -> Vec<SuspectedMod> {
    let mut suspects = Vec::new();

    for indexed_mod in indexed_mods {
        let signals = collect_match_signals(indexed_mod, errors);
        let candidate = score_candidate(indexed_mod, &signals, active_mods_reliably_known);
        if candidate.score <= 0 {
            continue;
        }

        let score = candidate.score.clamp(0, 100) as u8;
        suspects.push(SuspectedMod {
            name: indexed_mod.name.clone(),
            package_name: indexed_mod.package_name.clone(),
            file_path: indexed_mod.file_path.clone(),
            score,
            confidence: confidence_from_score(score),
            reasons: candidate.reasons,
            matched_paths: candidate.matched_paths.into_iter().collect(),
            readable: indexed_mod.readable,
            active_state: indexed_mod.active_state.clone(),
            manifest_present: indexed_mod.manifest_present,
            category_hints: indexed_mod.category_hints.iter().cloned().collect(),
        });
    }

    suspects.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| right.confidence.cmp(&left.confidence))
            .then_with(|| left.name.cmp(&right.name))
    });
    suspects.truncate(MAX_SUSPECTED_MODS);
    suspects
}

fn collect_match_signals(indexed_mod: &IndexedMod, errors: &[AnalyzedError]) -> MatchSignals {
    let mut signals = MatchSignals::default();

    for error in errors {
        if let Some(path) = error.extracted_path.as_deref() {
            if indexed_mod.path_set.contains(path) {
                signals.exact_path_match = true;
                if error.in_last_context {
                    signals.crash_context_match = true;
                }
            } else if find_partial_path_match(indexed_mod, path).is_some() {
                signals.partial_path_match = true;
                if error.in_last_context {
                    signals.crash_context_match = true;
                }
            }
        }

        if indexed_mod.category_hints.contains(&error.category) {
            signals.category_match = true;
        }
        if indexed_mod.label_hints.contains(&error.category) {
            signals.label_hint_match = true;
        }
    }

    signals.active_match = indexed_mod.active_state == "Active";
    signals
}

fn score_candidate(
    indexed_mod: &IndexedMod,
    signals: &MatchSignals,
    active_mods_reliably_known: bool,
) -> CandidateScore {
    let mut score = 0i32;
    let mut reasons = Vec::new();
    let mut matched_paths = BTreeSet::new();

    if signals.exact_path_match {
        score += 60;
        reasons.push("Exact log path match found inside the indexed local mod.".to_string());
    } else if signals.partial_path_match {
        score += 35;
        reasons.push("A path suffix or filename from the log matched a file inside the indexed local mod.".to_string());
    }

    if signals.category_match {
        score += 25;
        reasons.push("The mod category inferred from local files matches the extracted error category.".to_string());
    }

    if signals.active_match && (signals.exact_path_match || signals.partial_path_match || signals.category_match) {
        score += 20;
        reasons.push("The mod appears to be active in the current profile.".to_string());
    }

    if signals.crash_context_match && (signals.exact_path_match || signals.partial_path_match) {
        score += 15;
        reasons.push("The matched path appeared in the last relevant crash context.".to_string());
    }

    if signals.label_hint_match && signals.category_match {
        score += 10;
        reasons.push("The manifest or file name also hints at the same problem category.".to_string());
    }

    if !indexed_mod.readable && indexed_mod.active_state == "Active" {
        score += 10;
        reasons.push("The mod could not be indexed cleanly and also appears to be active.".to_string());
    }

    if active_mods_reliably_known
        && indexed_mod.active_state == "Not active"
        && (signals.exact_path_match || signals.partial_path_match || signals.category_match)
    {
        score -= 15;
        reasons.push("The mod matched locally, but it does not appear in the current active mod list.".to_string());
    }

    if signals.exact_path_match || signals.partial_path_match {
        for path in &indexed_mod.indexed_paths {
            matched_paths.insert(path.clone());
            if matched_paths.len() >= 5 {
                break;
            }
        }
    }

    CandidateScore {
        score,
        exact_path_match: signals.exact_path_match,
        matched_paths,
        reasons,
    }
}

fn build_crash_summary(errors: &[AnalyzedError], crash_log_found: bool) -> CrashSummary {
    let mut category_counts = HashMap::<String, usize>::new();
    let mut error_count = 0usize;
    let mut warning_count = 0usize;
    let mut last_relevant_context = Vec::new();

    for error in errors {
        if error.source == "game.log.txt" || error.source == "game.crash.txt" {
            *category_counts.entry(error.category.clone()).or_default() += 1;
        }
        match error.severity.as_str() {
            "Critical" | "Error" => error_count += 1,
            "Warning" => warning_count += 1,
            _ => {}
        }
        if error.in_last_context {
            last_relevant_context.push(render_raw_line(error));
        }
    }

    let primary_category = category_counts
        .into_iter()
        .max_by(|left, right| left.1.cmp(&right.1).then_with(|| left.0.cmp(&right.0)))
        .map(|item| item.0);

    let headline = if crash_log_found {
        "game.crash.txt was found and correlated with the most recent relevant error chain.".to_string()
    } else if error_count > 0 || warning_count > 0 {
        "Relevant issues were extracted from game.log.txt, but no game.crash.txt was found.".to_string()
    } else {
        "No relevant crash pattern could be extracted from the available logs.".to_string()
    };

    let summary = if let Some(last_line) = last_relevant_context.last() {
        format!(
            "The analyzer uses the last relevant error context instead of trusting a single final line. Latest context item: {}",
            last_line
        )
    } else if crash_log_found {
        "A crash log exists, but it did not contain enough structured hints to assign a strong error category.".to_string()
    } else {
        "Not enough log evidence was available to build a detailed crash context.".to_string()
    };

    CrashSummary {
        crash_detected: crash_log_found,
        primary_category,
        headline,
        summary,
        error_count,
        warning_count,
        last_relevant_context,
    }
}

fn build_overview(
    sources: &AnalysisSources,
    crash_summary: &CrashSummary,
    suspected_mods: &[SuspectedMod],
    missing_references: &[MissingReference],
    removed_mod_suspected: bool,
    limitations: &[String],
) -> AnalyzerOverview {
    let has_issue = !suspected_mods.is_empty()
        || !missing_references.is_empty()
        || crash_summary.error_count > 0
        || crash_summary.crash_detected;
    let has_warning = crash_summary.warning_count > 0
        || sources.unreadable_mods_count > 0
        || !limitations.is_empty();

    let status_badge = if !sources.game_log_found && !sources.game_crash_found {
        "Not enough data"
    } else if has_issue {
        "Issues found"
    } else if has_warning {
        "Warnings"
    } else {
        "Clean"
    }
    .to_string();

    let summary = if let Some(top_mod) = suspected_mods.first() {
        format!(
            "Top local suspect: {} ({}, {}/100). Review the matched paths and raw log lines before disabling anything.",
            top_mod.name, top_mod.confidence, top_mod.score
        )
    } else if removed_mod_suspected {
        "No local mod could be matched with confidence. A removed mod or stale save reference is more likely than a currently installed culprit.".to_string()
    } else if !sources.game_log_found && !sources.game_crash_found {
        "No crash logs were found. The analyzer needs at least game.log.txt or game.crash.txt for a useful result.".to_string()
    } else {
        "No suspicious local mod could be assigned from the current evidence.".to_string()
    };

    let confidence_note = if let Some(top_mod) = suspected_mods.first() {
        format!(
            "{} confidence means heuristic suspicion, not proof. Exact path matches are stronger than category-only matches.",
            top_mod.confidence
        )
    } else if removed_mod_suspected {
        "Not enough data to assign a local mod. Unknown / Removed Mod Suspected is based on unmatched asset references.".to_string()
    } else {
        "Not enough data to assign a mod.".to_string()
    };

    let disclaimer = "The analyzer only scores indexed local mods. It does not prove a single culprit and may miss workshop items or references from mods that are no longer installed."
        .to_string();

    AnalyzerOverview {
        status_badge,
        summary,
        confidence_note,
        disclaimer,
    }
}

fn parse_active_mods(profile_content: &str) -> Vec<ActiveModEntry> {
    let Some(re) = compile_regex(
        r#"active_mods\[\d+\]:\s*"([^"]+)""#,
        "parse_active_mods",
    ) else {
        return Vec::new();
    };

    re.captures_iter(profile_content)
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

fn scan_installed_mods(mod_dir: &Path, limitations: &mut Vec<String>) -> Vec<IndexedMod> {
    if !mod_dir.exists() {
        record_limitation(
            limitations,
            format!("The mod folder does not exist: {}", mod_dir.display()),
            false,
        );
        return Vec::new();
    }

    let Ok(entries) = fs::read_dir(mod_dir) else {
        record_limitation(
            limitations,
            format!("The mod folder could not be read: {}", mod_dir.display()),
            false,
        );
        return Vec::new();
    };

    let mut mods = Vec::new();
    let entries = entries.flatten().collect::<Vec<_>>();
    crate::dev_log!(
        "[diagnostics] scanning mod folder {} entries={}",
        mod_dir.display(),
        entries.len()
    );

    for entry in entries {
        let path = entry.path();
        let is_archive = path
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| matches!(value.to_ascii_lowercase().as_str(), "scs" | "zip"))
            .unwrap_or(false);
        if !path.is_dir() && !is_archive {
            continue;
        }

        let inspected = catch_unwind(AssertUnwindSafe(|| inspect_installed_mod_entry(&path, is_archive)));
        match inspected {
            Ok(Ok(indexed_mod)) => mods.push(indexed_mod),
            Ok(Err(error)) => {
                crate::dev_log!(
                    "[diagnostics] mod entry inspection failed safely: {} | {}",
                    path.display(),
                    error
                );
                record_limitation(
                    limitations,
                    format!("Could not fully index mod entry `{}`: {}", path.display(), error),
                    false,
                );
                mods.push(fallback_indexed_mod(&path, false));
            }
            Err(payload) => {
                let message = panic_message(payload);
                crate::dev_log!(
                    "[diagnostics] mod entry panic avoided: {} | {}",
                    path.display(),
                    message
                );
                record_limitation(
                    limitations,
                    format!("A mod entry was skipped after an internal analyzer failure: {}", path.display()),
                    false,
                );
                mods.push(fallback_indexed_mod(&path, false));
            }
        }
    }

    mods
}

fn inspect_installed_mod_entry(path: &Path, is_archive: bool) -> Result<IndexedMod, String> {
    if is_archive {
        inspect_archive_mod_entry(path)
    } else {
        inspect_folder_mod_entry(path)
    }
}

fn inspect_folder_mod_entry(path: &Path) -> Result<IndexedMod, String> {
    let mut manifest_summary = ManifestSummary::default();
    let mut manifest_present = false;
    let mut indexed_paths = Vec::new();
    let mut readable = true;

    for entry in WalkDir::new(path) {
        let entry = match entry {
            Ok(value) => value,
            Err(error) => {
                readable = false;
                crate::dev_log!("[diagnostics] walkdir failed for {}: {}", path.display(), error);
                continue;
            }
        };
        if !entry.file_type().is_file() {
            continue;
        }

        let relative = entry
            .path()
            .strip_prefix(path)
            .map_err(|error| error.to_string())?
            .to_string_lossy()
            .replace('\\', "/");
        let normalized = normalize_indexed_path(&relative);
        if normalized.ends_with("/manifest.sii") || normalized == "/manifest.sii" {
            manifest_present = true;
            if let Ok(content) = read_plain_text_lossy("manifest.sii", entry.path()) {
                manifest_summary = parse_manifest_text(&content);
            }
        }
        if is_relevant_indexed_path(&normalized) {
            indexed_paths.push(normalized);
        }
    }

    Ok(build_indexed_mod(
        path,
        true,
        readable,
        manifest_present,
        manifest_summary,
        indexed_paths,
    ))
}

fn inspect_archive_mod_entry(path: &Path) -> Result<IndexedMod, String> {
    let file = File::open(path).map_err(|error| error.to_string())?;
    let mut archive = ZipArchive::new(file).map_err(|error| error.to_string())?;

    let mut manifest_summary = ManifestSummary::default();
    let mut manifest_present = false;
    let mut indexed_paths = Vec::new();

    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(|error| error.to_string())?;
        if !entry.is_file() {
            continue;
        }

        let normalized = normalize_indexed_path(&entry.name().replace('\\', "/"));
        if normalized.ends_with("/manifest.sii") || normalized == "/manifest.sii" {
            manifest_present = true;
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes).map_err(|error| error.to_string())?;
            manifest_summary = parse_manifest_text(&String::from_utf8_lossy(&bytes));
        }
        if is_relevant_indexed_path(&normalized) {
            indexed_paths.push(normalized);
        }
    }

    Ok(build_indexed_mod(
        path,
        true,
        true,
        manifest_present,
        manifest_summary,
        indexed_paths,
    ))
}

fn fallback_indexed_mod(path: &Path, readable: bool) -> IndexedMod {
    build_indexed_mod(
        path,
        path.is_dir(),
        readable,
        false,
        ManifestSummary::default(),
        Vec::new(),
    )
}

fn build_indexed_mod(
    path: &Path,
    is_folder: bool,
    readable: bool,
    manifest_present: bool,
    manifest_summary: ManifestSummary,
    indexed_paths: Vec<String>,
) -> IndexedMod {
    let file_stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .or_else(|| path.file_name().and_then(|value| value.to_str()))
        .unwrap_or("unknown_mod")
        .to_string();

    let name = manifest_summary
        .display_name
        .clone()
        .unwrap_or_else(|| prettify_token(&file_stem));
    let package_name = manifest_summary.package_name.clone();

    let mut path_set = HashSet::new();
    let mut file_names = HashSet::new();
    let mut category_hints = BTreeSet::new();
    for indexed_path in indexed_paths.iter().cloned() {
        file_names.extend(file_name_token(&indexed_path));
        category_hints.extend(categories_from_path(&indexed_path));
        path_set.insert(indexed_path);
    }

    let mut tokens = tokenize_to_set(&name);
    tokens.extend(tokenize(&file_stem));
    if let Some(package_name) = &package_name {
        tokens.extend(tokenize(package_name));
    }

    let mut label_hints = classify_label_hints(&format!(
        "{} {} {}",
        name,
        file_stem,
        manifest_summary.compatible_versions.join(" ")
    ));
    if label_hints.is_empty() {
        label_hints.insert("UnknownReference".to_string());
    }
    if category_hints.is_empty() {
        category_hints.extend(label_hints.iter().cloned());
    }

    IndexedMod {
        name,
        package_name,
        file_path: path_to_string(path),
        readable,
        manifest_present,
        category_hints,
        label_hints,
        indexed_paths,
        path_set,
        file_names,
        tokens,
        active_state: if is_folder {
            "Unknown".to_string()
        } else {
            "Unknown".to_string()
        },
    }
}

fn apply_active_states(
    indexed_mods: &mut [IndexedMod],
    active_mods: &[ActiveModEntry],
    active_mods_reliably_known: bool,
) {
    for indexed_mod in indexed_mods.iter_mut() {
        if !active_mods_reliably_known {
            indexed_mod.active_state = "Unknown".to_string();
            continue;
        }

        let is_active = active_mods
            .iter()
            .any(|active_mod| active_mod_matches_indexed_mod(active_mod, indexed_mod));
        indexed_mod.active_state = if is_active {
            "Active".to_string()
        } else {
            "Not active".to_string()
        };
    }
}

fn active_mod_matches_indexed_mod(active_mod: &ActiveModEntry, indexed_mod: &IndexedMod) -> bool {
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
        normalize_alias(
            Path::new(&indexed_mod.file_path)
                .file_stem()
                .and_then(|value| value.to_str())
                .unwrap_or_default(),
        ),
    ];

    for active_alias in &active_aliases {
        if active_alias.is_empty() {
            continue;
        }
        for indexed_alias in &indexed_aliases {
            if indexed_alias.is_empty() {
                continue;
            }
            if active_alias == indexed_alias
                || active_alias.contains(indexed_alias.as_str())
                || indexed_alias.contains(active_alias.as_str())
            {
                return true;
            }
        }
    }

    false
}

fn parse_manifest_text(text: &str) -> ManifestSummary {
    let display_name = capture_manifest_value(text, "display_name")
        .or_else(|| capture_manifest_value(text, "name"))
        .or_else(|| capture_manifest_value(text, "package_name"));
    let package_name = capture_manifest_value(text, "package_name")
        .or_else(|| capture_manifest_value(text, "name"));

    let Some(compat_re) = compile_regex(
        r#"compatible_versions\[\d+\]:\s*"([^"]+)""#,
        "parse_manifest_text.compatible_versions",
    ) else {
        return ManifestSummary {
            display_name,
            package_name,
            compatible_versions: Vec::new(),
        };
    };

    let compatible_versions = compat_re
        .captures_iter(text)
        .filter_map(|capture| capture.get(1).map(|value| value.as_str().to_string()))
        .collect::<Vec<_>>();

    ManifestSummary {
        display_name,
        package_name,
        compatible_versions,
    }
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

fn extract_custom_save_references(save_content: &str, active_mods: &[ActiveModEntry]) -> Vec<String> {
    let mut references = BTreeSet::new();
    let Some(data_path_re) = compile_regex(
        r#"data_path:\s*"([^"]+)""#,
        "extract_custom_save_references.data_path",
    ) else {
        return Vec::new();
    };
    let Some(asset_re) = compile_regex(
        r#"([A-Za-z0-9_/\.-]+\.(?:sii|sui|pmd|pmg|mat|tobj|dds|ogg|bank|unit))"#,
        "extract_custom_save_references.asset_re",
    ) else {
        return Vec::new();
    };

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

fn is_relevant_log_line(line: &str) -> bool {
    let normalized = line.to_ascii_lowercase();
    let has_failure = contains_any(
        &normalized,
        &[
            "error",
            "warning",
            "missing",
            "failed to open",
            "unable to find",
            "invalid",
            "incorrect",
            "cannot load",
            "can't load",
            "unknown unit",
            "parse",
        ],
    );
    let has_domain_hint = contains_any(
        &normalized,
        &[
            "accessory",
            "cargo",
            "map",
            "material",
            "prefab",
            "sound",
            "texture",
            "trailer",
            "truck",
            "ui",
        ],
    );
    has_failure || (has_domain_hint && extract_path_from_line(line).is_some())
}

fn is_relevant_crash_line(line: &str) -> bool {
    let normalized = line.to_ascii_lowercase();
    contains_any(
        &normalized,
        &[
            "access violation",
            "assert",
            "backtrace",
            "call stack",
            "crash",
            "exception",
            "fault",
            "module",
            "stack",
        ],
    )
}

fn extract_path_from_line(line: &str) -> Option<String> {
    let patterns = [
        compile_regex(
            r#"([A-Za-z]:\\[^"\r\n]+)"#,
            "extract_path_from_line.windows_path",
        ),
        compile_regex(
            r#"(/(?:def|vehicle|model|material|map|ui|sound|prefab)[^"\s]*)"#,
            "extract_path_from_line.asset_prefix",
        ),
        compile_regex(
            r#"([A-Za-z0-9_/\.-]+\.(?:sii|sui|pmd|pmg|mat|tobj|dds|ogg|bank|unit))"#,
            "extract_path_from_line.asset_ext",
        ),
    ];

    for pattern in patterns {
        let Some(pattern) = pattern else {
            continue;
        };
        if let Some(value) = pattern
            .captures(line)
            .and_then(|capture| capture.get(1))
            .map(|value| value.as_str())
        {
            return Some(normalize_indexed_path(value));
        }
    }

    None
}

fn classify_error_severity(line: &str, crash_mode: bool) -> String {
    if crash_mode {
        return "Critical".to_string();
    }

    let normalized = line.to_ascii_lowercase();
    if contains_any(&normalized, &["<error", " error", "failed to open", "cannot load", "unknown unit", "unable to find"]) {
        return "Error".to_string();
    }
    if contains_any(&normalized, &["<warning", " warning", "invalid", "incorrect", "missing"]) {
        return "Warning".to_string();
    }
    "Info".to_string()
}

fn classify_error_category(line: &str, extracted_path: Option<&str>, crash_mode: bool) -> String {
    let category = classify_path_category(
        extracted_path.unwrap_or_default(),
        Some(line),
        crash_mode,
    );
    if category == "UnknownReference" && crash_mode {
        "CrashContext".to_string()
    } else {
        category
    }
}

fn classify_path_category(path: &str, line: Option<&str>, crash_mode: bool) -> String {
    let normalized_path = path.to_ascii_lowercase();
    let normalized_line = line.unwrap_or_default().to_ascii_lowercase();

    if normalized_path.contains("/def/vehicle/truck")
        || normalized_line.contains("truck")
    {
        return "TruckReference".to_string();
    }
    if normalized_path.contains("/def/vehicle/trailer")
        || normalized_line.contains("trailer")
    {
        return "TrailerReference".to_string();
    }
    if normalized_path.contains("accessory") || normalized_line.contains("accessory") {
        return "AccessoryReference".to_string();
    }
    if normalized_path.contains("/def/cargo") || normalized_line.contains("cargo") {
        return "CargoReference".to_string();
    }
    if normalized_path.contains("/prefab/") || normalized_line.contains("prefab") {
        return "PrefabReference".to_string();
    }
    if normalized_path.contains("/map/") || normalized_line.contains(" map") {
        return "MapReference".to_string();
    }
    if normalized_path.ends_with(".mat") || normalized_path.contains("/material/") || normalized_line.contains("material") {
        return "MaterialReference".to_string();
    }
    if normalized_path.ends_with(".dds")
        || normalized_path.ends_with(".tobj")
        || normalized_line.contains("texture")
    {
        return "TextureReference".to_string();
    }
    if normalized_path.ends_with(".ogg")
        || normalized_path.ends_with(".bank")
        || normalized_path.contains("/sound/")
        || normalized_line.contains("sound")
    {
        return "SoundReference".to_string();
    }
    if normalized_path.ends_with(".sui")
        || normalized_path.contains("/ui/")
        || normalized_line.contains("route advisor")
        || normalized_line.contains("hud")
        || normalized_line.contains(" ui")
    {
        return "UiReference".to_string();
    }
    if normalized_path.ends_with(".sii")
        || normalized_path.ends_with(".unit")
        || normalized_path.contains("/def/")
        || normalized_line.contains("unknown unit")
    {
        return "DefinitionReference".to_string();
    }
    if contains_any(
        &normalized_line,
        &["failed to open", "unable to find", "cannot load", "can't load"],
    ) {
        return "FileOpenError".to_string();
    }
    if contains_any(&normalized_line, &["invalid", "incorrect", "parse"]) {
        return "ParseError".to_string();
    }
    if crash_mode {
        return "CrashContext".to_string();
    }
    "UnknownReference".to_string()
}

fn explanation_for_category(category: &str, path: Option<&str>, crash_mode: bool) -> String {
    let subject = path.unwrap_or("the referenced asset");
    match category {
        "TruckReference" => format!("The logs point at truck-related content. Review truck definitions, owned truck mods and any truck upgrades that reference {}.", subject),
        "TrailerReference" => format!("The logs point at trailer-related content. Review trailer definitions, cargo packs and trailer accessories linked to {}.", subject),
        "AccessoryReference" => format!("The logs point at accessory content. Removed or outdated tuning and accessory mods often leave references like {} behind.", subject),
        "CargoReference" => format!("The logs point at cargo-related content. Cargo packs and trailer economy mods should be checked for {}.", subject),
        "MapReference" => format!("The logs point at map content. Broken load order or removed map packages can leave missing references such as {}.", subject),
        "PrefabReference" => format!("The logs point at a prefab reference. Prefab issues are commonly tied to map mods or removed map dependencies such as {}.", subject),
        "MaterialReference" => format!("The logs point at a material file. Visual, vehicle or map mods may be missing material resources like {}.", subject),
        "TextureReference" => format!("The logs point at a texture resource. Texture, UI or visual mods should be checked for {}.", subject),
        "UiReference" => format!("The logs point at UI content. Route advisor, HUD and UI mods should be checked for {}.", subject),
        "SoundReference" => format!("The logs point at sound content. Sound packs and audio-related mods should be checked for {}.", subject),
        "DefinitionReference" => format!("The logs point at a definition or unit file. Missing definitions such as {} often come from removed or incompatible mods.", subject),
        "FileOpenError" => format!("The game could not open {}. That usually means the referenced asset is missing or unreadable.", subject),
        "ParseError" => format!("The game reported invalid or incorrect data around {}. That often points to malformed definitions or incompatible mod data.", subject),
        "CrashContext" => {
            if crash_mode {
                "This line comes directly from game.crash.txt and provides crash context, but not proof of a single culprit mod.".to_string()
            } else {
                "This line contributes crash context but does not identify a single culprit on its own.".to_string()
            }
        }
        _ => format!("The analyzer captured {} as a relevant reference, but could not classify it more precisely.", subject),
    }
}

fn render_raw_line(item: &AnalyzedError) -> String {
    match item.line_number {
        Some(line_number) => format!(
            "{}:{} [{}] {}",
            item.source, line_number, item.severity, item.raw_line
        ),
        None => format!("{} [{}] {}", item.source, item.severity, item.raw_line),
    }
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

fn categories_from_path(path: &str) -> BTreeSet<String> {
    let mut categories = BTreeSet::new();
    categories.insert(classify_path_category(path, None, false));
    categories.retain(|item| item != "UnknownReference");
    categories
}

fn classify_label_hints(label: &str) -> BTreeSet<String> {
    let normalized = label.to_ascii_lowercase();
    let mut hints = BTreeSet::new();
    if contains_any(&normalized, &["sound", "audio"]) {
        hints.insert("SoundReference".to_string());
    }
    if contains_any(&normalized, &["ui", "hud", "route advisor", "gps"]) {
        hints.insert("UiReference".to_string());
    }
    if contains_any(&normalized, &["map", "promods", "reforma", "road"]) {
        hints.insert("MapReference".to_string());
        hints.insert("PrefabReference".to_string());
    }
    if contains_any(&normalized, &["cargo", "economy", "freight"]) {
        hints.insert("CargoReference".to_string());
    }
    if contains_any(&normalized, &["trailer", "krone", "schmitz"]) {
        hints.insert("TrailerReference".to_string());
    }
    if contains_any(&normalized, &["truck", "scania", "volvo", "daf", "man", "kenworth", "peterbilt"]) {
        hints.insert("TruckReference".to_string());
    }
    if contains_any(&normalized, &["accessory", "tuning", "wheel", "interior", "paint"]) {
        hints.insert("AccessoryReference".to_string());
    }
    hints
}

fn path_matches_any_mod(path: &str, indexed_mods: &[IndexedMod]) -> bool {
    indexed_mods.iter().any(|indexed_mod| {
        indexed_mod.path_set.contains(path) || find_partial_path_match(indexed_mod, path).is_some()
    })
}

fn find_partial_path_match(indexed_mod: &IndexedMod, path: &str) -> Option<String> {
    let mut best_match: Option<(usize, String)> = None;
    let target_file_name = file_name_from_path(path);

    for indexed_path in &indexed_mod.indexed_paths {
        let overlap = trailing_segment_overlap(indexed_path, path);
        let same_file_name = match (file_name_from_path(indexed_path), target_file_name.clone()) {
            (Some(left), Some(right)) => left == right,
            _ => false,
        };

        if overlap >= 2 || (overlap == 1 && same_file_name) {
            match best_match.as_ref() {
                Some((current_overlap, _)) if *current_overlap >= overlap => {}
                _ => best_match = Some((overlap, indexed_path.clone())),
            }
        }
    }

    best_match.map(|(_, path)| path)
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

fn file_name_from_path(path: &str) -> Option<String> {
    path.rsplit('/')
        .find(|segment| !segment.is_empty())
        .map(|segment| segment.to_string())
}

fn file_name_token(path: &str) -> Option<String> {
    file_name_from_path(path)
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
                overlap += 1;
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

fn confidence_from_score(score: u8) -> String {
    match score {
        90..=100 => "High".to_string(),
        70..=89 => "Likely".to_string(),
        40..=69 => "Possible".to_string(),
        _ => "Low".to_string(),
    }
}

fn severity_sort_key(severity: &str) -> u8 {
    match severity {
        "Critical" => 4,
        "Error" => 3,
        "Warning" => 2,
        _ => 1,
    }
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

fn path_to_string(path: &Path) -> String {
    path.display().to_string()
}

fn contains_any(value: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| value.contains(needle))
}
