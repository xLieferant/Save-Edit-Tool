use super::models::ModConflictAnalysisReport;
use std::fs;
use std::path::PathBuf;
use tauri::AppHandle;
use tauri_plugin_dialog::DialogExt;

pub fn export_report(
    app: &AppHandle,
    report: &ModConflictAnalysisReport,
    formatted: bool,
) -> Result<Option<String>, String> {
    let default_file_name = if formatted {
        "crash-report.txt"
    } else {
        "errors.txt"
    };
    let title = if formatted {
        "Export crash-report.txt"
    } else {
        "Export errors.txt"
    };

    let file_path = app
        .dialog()
        .file()
        .add_filter("Text file", &["txt"])
        .set_title(title)
        .set_file_name(default_file_name)
        .blocking_save_file();

    let Some(file_path) = file_path else {
        return Ok(None);
    };

    let path = file_path_to_path_buf(file_path)?;
    let body = if formatted {
        build_pretty_report(report)
    } else {
        build_errors_report(report)
    };

    fs::write(&path, body).map_err(|error| {
        format!(
            "The diagnostic report could not be written to {}: {}",
            path.display(),
            error
        )
    })?;

    Ok(Some(path.display().to_string()))
}

pub fn build_errors_report(report: &ModConflictAnalysisReport) -> String {
    let mut out = Vec::new();
    out.push(format!(
        "Mod Conflict Analyzer export | generated_at={}",
        report.generated_at
    ));
    out.push(format!("report_version={}", report.report_version));
    out.push(String::new());

    out.push("== OVERVIEW ==".to_string());
    out.push(format!("status: {}", report.overview.status_badge));
    out.push(format!("summary: {}", report.overview.summary));
    out.push(format!("confidence_note: {}", report.overview.confidence_note));
    out.push(format!("disclaimer: {}", report.overview.disclaimer));
    out.push(String::new());

    out.push("== DATA SOURCES ==".to_string());
    out.push(format!(
        "game.log.txt: {} | {}",
        found_label(report.sources.game_log_found),
        report.sources.game_log_path.as_deref().unwrap_or("-")
    ));
    out.push(format!(
        "game.crash.txt: {} | {}",
        found_label(report.sources.game_crash_found),
        report.sources.game_crash_path.as_deref().unwrap_or("-")
    ));
    out.push(format!(
        "mod folder: {} | {}",
        found_label(report.sources.mod_folder_found),
        report.sources.mod_folder_path.as_deref().unwrap_or("-")
    ));
    out.push(format!(
        "indexed_mods_count: {} (readable={} unreadable={})",
        report.sources.indexed_mods_count,
        report.sources.readable_mods_count,
        report.sources.unreadable_mods_count
    ));
    out.push(format!(
        "active_mods_count: {} | reliably_known={}",
        report.sources.active_mods_count, report.sources.active_mods_reliably_known
    ));
    out.push(format!(
        "extracted_errors_count: {}",
        report.sources.extracted_errors_count
    ));
    out.push(String::new());

    out.push("== CONTEXT ==".to_string());
    out.push(format!("game: {}", report.context.selected_game));
    out.push(format!(
        "base_path: {}",
        report.context.base_path.as_deref().unwrap_or("-")
    ));
    out.push(format!(
        "profile_path: {}",
        report.context.profile_path.as_deref().unwrap_or("-")
    ));
    out.push(format!(
        "profile_inferred: {}",
        report.context.profile_inferred
    ));
    out.push(format!(
        "save_path: {}",
        report.context.save_path.as_deref().unwrap_or("-")
    ));
    out.push(format!("save_inferred: {}", report.context.save_inferred));
    out.push(String::new());

    out.push("== CRASH SUMMARY ==".to_string());
    out.push(format!("crash_detected: {}", report.crash_summary.crash_detected));
    out.push(format!(
        "primary_category: {}",
        report
            .crash_summary
            .primary_category
            .as_deref()
            .unwrap_or("-")
    ));
    out.push(format!("headline: {}", report.crash_summary.headline));
    out.push(format!("summary: {}", report.crash_summary.summary));
    out.push(format!(
        "error_count: {} | warning_count: {}",
        report.crash_summary.error_count, report.crash_summary.warning_count
    ));
    if report.crash_summary.last_relevant_context.is_empty() {
        out.push("No last relevant crash context captured.".to_string());
    } else {
        out.push("Last relevant context:".to_string());
        for line in &report.crash_summary.last_relevant_context {
            out.push(format!("- {}", line));
        }
    }
    out.push(String::new());

    out.push("== ACTIVE MODS ==".to_string());
    if report.active_mods.is_empty() {
        out.push("No active mods could be identified.".to_string());
    } else {
        for active_mod in &report.active_mods {
            out.push(format!("- {}", active_mod));
        }
    }
    out.push(String::new());

    out.push("== SUSPECTED MODS ==".to_string());
    if report.suspected_mods.is_empty() {
        out.push("No suspicious local mods detected.".to_string());
    } else {
        for (index, item) in report.suspected_mods.iter().enumerate() {
            out.push(format!(
                "{}. {} | score={} | confidence={} | active={} | readable={} | manifest_present={}",
                index + 1,
                item.name,
                item.score,
                item.confidence,
                item.active_state,
                item.readable,
                item.manifest_present
            ));
            out.push(format!("   file: {}", item.file_path));
            if let Some(package_name) = &item.package_name {
                out.push(format!("   package: {}", package_name));
            }
            if !item.category_hints.is_empty() {
                out.push(format!(
                    "   categories: {}",
                    item.category_hints.join(", ")
                ));
            }
            for reason in &item.reasons {
                out.push(format!("   - {}", reason));
            }
            for matched_path in &item.matched_paths {
                out.push(format!("   matched_path: {}", matched_path));
            }
        }
    }
    out.push(String::new());

    out.push("== MISSING / REMOVED REFERENCES ==".to_string());
    if report.missing_references.is_empty() {
        out.push("No missing references were detected.".to_string());
    } else {
        for item in &report.missing_references {
            out.push(format!(
                "- [{}] {} | source={}",
                item.category, item.path, item.source
            ));
            out.push(format!("  {}", item.reason));
        }
    }
    if report.removed_mod_suspected {
        out.push(String::new());
        out.push("Unknown / Removed Mod Suspected".to_string());
        if let Some(reason) = &report.removed_mod_reason {
            out.push(reason.clone());
        }
    }
    out.push(String::new());

    out.push("== ANALYZED ERRORS ==".to_string());
    if report.errors.is_empty() {
        out.push("No relevant errors were extracted.".to_string());
    } else {
        for item in &report.errors {
            let line_ref = item
                .line_number
                .map(|line| line.to_string())
                .unwrap_or_else(|| "-".to_string());
            out.push(format!(
                "- [{} / {}] {}:{}",
                item.severity, item.category, item.source, line_ref
            ));
            if let Some(path) = &item.extracted_path {
                out.push(format!("  path: {}", path));
            }
            if item.in_last_context {
                out.push("  in_last_context: true".to_string());
            }
            out.push(format!("  explanation: {}", item.explanation));
            out.push(format!("  raw: {}", item.raw_line));
        }
    }
    out.push(String::new());

    out.push("== LOG PATHS ==".to_string());
    out.push(format!(
        "technical_log: {}",
        report.logs.technical_log_path.as_deref().unwrap_or("-")
    ));
    out.push(format!(
        "user_log: {}",
        report.logs.user_log_path.as_deref().unwrap_or("-")
    ));
    out.push(format!(
        "log_directory: {}",
        report.logs.log_directory_path.as_deref().unwrap_or("-")
    ));
    out.push(String::new());

    out.push("== RAW RELEVANT game.log.txt LINES ==".to_string());
    if report.raw_relevant_log_lines.is_empty() {
        out.push("No relevant game.log.txt lines captured.".to_string());
    } else {
        out.extend(report.raw_relevant_log_lines.iter().cloned());
    }
    out.push(String::new());

    out.push("== RAW RELEVANT game.crash.txt LINES ==".to_string());
    if report.raw_relevant_crash_lines.is_empty() {
        out.push("No relevant game.crash.txt lines captured.".to_string());
    } else {
        out.extend(report.raw_relevant_crash_lines.iter().cloned());
    }
    out.push(String::new());

    out.push("== LIMITATIONS ==".to_string());
    if report.limitations.is_empty() {
        out.push("No additional limitations recorded.".to_string());
    } else {
        for limitation in &report.limitations {
            out.push(format!("- {}", limitation));
        }
    }

    out.join("\r\n")
}

pub fn build_pretty_report(report: &ModConflictAnalysisReport) -> String {
    let mut out = Vec::new();
    out.push("Mod Conflict Analyzer Report".to_string());
    out.push("============================".to_string());
    out.push(format!("Generated at: {}", report.generated_at));
    out.push(format!("Game: {}", report.context.selected_game.to_uppercase()));
    out.push(String::new());

    out.push(format!("Status: {}", report.overview.status_badge));
    out.push(report.overview.summary.clone());
    out.push(report.overview.confidence_note.clone());
    out.push(report.overview.disclaimer.clone());
    out.push(String::new());

    out.push("Data Sources".to_string());
    out.push(format!(
        "- game.log.txt: {}",
        if report.sources.game_log_found {
            report.sources.game_log_path.as_deref().unwrap_or("found")
        } else {
            "No game.log.txt found"
        }
    ));
    out.push(format!(
        "- game.crash.txt: {}",
        if report.sources.game_crash_found {
            report.sources.game_crash_path.as_deref().unwrap_or("found")
        } else {
            "No game.crash.txt found"
        }
    ));
    out.push(format!(
        "- mod folder: {}",
        if report.sources.mod_folder_found {
            report.sources.mod_folder_path.as_deref().unwrap_or("found")
        } else {
            "No mod folder found"
        }
    ));
    out.push(format!(
        "- indexed mods: {} (unreadable: {})",
        report.sources.indexed_mods_count, report.sources.unreadable_mods_count
    ));
    out.push(String::new());

    out.push("Crash Summary".to_string());
    out.push(format!(
        "- primary category: {}",
        report
            .crash_summary
            .primary_category
            .as_deref()
            .unwrap_or("No category")
    ));
    out.push(format!(
        "- errors: {} | warnings: {}",
        report.crash_summary.error_count, report.crash_summary.warning_count
    ));
    out.push(format!("- {}", report.crash_summary.headline));
    out.push(format!("- {}", report.crash_summary.summary));
    for line in report.crash_summary.last_relevant_context.iter().take(6) {
        out.push(format!("  {}", line));
    }
    out.push(String::new());

    out.push("Suspected Mods".to_string());
    if report.suspected_mods.is_empty() {
        out.push("- No suspicious local mods detected.".to_string());
    } else {
        for item in report.suspected_mods.iter().take(8) {
            out.push(format!(
                "- {} | {}/100 | {} | {}",
                item.name, item.score, item.confidence, item.active_state
            ));
            for reason in item.reasons.iter().take(3) {
                out.push(format!("  {}", reason));
            }
            for matched_path in item.matched_paths.iter().take(3) {
                out.push(format!("  matched: {}", matched_path));
            }
        }
    }
    out.push(String::new());

    out.push("Missing / Removed References".to_string());
    if report.missing_references.is_empty() {
        out.push("- No missing references detected.".to_string());
    } else {
        for item in report.missing_references.iter().take(12) {
            out.push(format!("- [{}] {}", item.category, item.path));
            out.push(format!("  {}", item.reason));
        }
    }
    if report.removed_mod_suspected {
        out.push("- Unknown / Removed Mod Suspected".to_string());
        if let Some(reason) = &report.removed_mod_reason {
            out.push(format!("  {}", reason));
        }
    }
    out.push(String::new());

    out.push("Relevant Errors".to_string());
    if report.errors.is_empty() {
        out.push("- No relevant errors extracted.".to_string());
    } else {
        for item in report.errors.iter().take(12) {
            out.push(format!(
                "- [{} / {}] {}",
                item.severity, item.category, item.raw_line
            ));
        }
    }
    out.push(String::new());

    out.push("Log Files".to_string());
    out.push(format!(
        "- technical log: {}",
        report.logs.technical_log_path.as_deref().unwrap_or("-")
    ));
    out.push(format!(
        "- user log: {}",
        report.logs.user_log_path.as_deref().unwrap_or("-")
    ));

    out.join("\r\n")
}

fn file_path_to_path_buf(path: tauri_plugin_dialog::FilePath) -> Result<PathBuf, String> {
    path.into_path()
        .map_err(|_| "The selected export path could not be resolved.".to_string())
}

fn found_label(found: bool) -> &'static str {
    if found {
        "found"
    } else {
        "missing"
    }
}
