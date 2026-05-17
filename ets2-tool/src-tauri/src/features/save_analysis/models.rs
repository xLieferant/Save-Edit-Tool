use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DiagnosticsContext {
    pub selected_game: String,
    pub base_path: Option<String>,
    pub profile_path: Option<String>,
    pub profile_inferred: bool,
    pub save_path: Option<String>,
    pub save_inferred: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AnalyzerOverview {
    pub status_badge: String,
    pub summary: String,
    pub confidence_note: String,
    pub disclaimer: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AnalysisSources {
    pub analysis_mode: String,
    pub analysis_timed_out: bool,
    pub game_log_found: bool,
    pub game_log_path: Option<String>,
    pub game_crash_found: bool,
    pub game_crash_path: Option<String>,
    pub mod_folder_found: bool,
    pub mod_folder_path: Option<String>,
    pub indexed_mods_count: usize,
    pub readable_mods_count: usize,
    pub unreadable_mods_count: usize,
    pub active_mods_count: usize,
    pub active_mods_reliably_known: bool,
    pub extracted_errors_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AnalyzedError {
    pub source: String,
    pub severity: String,
    pub category: String,
    pub line_number: Option<usize>,
    pub raw_line: String,
    pub extracted_path: Option<String>,
    pub explanation: String,
    pub in_last_context: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CrashSummary {
    pub crash_detected: bool,
    pub primary_category: Option<String>,
    pub headline: String,
    pub summary: String,
    pub error_count: usize,
    pub warning_count: usize,
    pub last_relevant_context: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MissingReference {
    pub path: String,
    pub category: String,
    pub source: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SuspectedMod {
    pub name: String,
    pub package_name: Option<String>,
    pub file_path: String,
    pub score: u8,
    pub confidence: String,
    pub reasons: Vec<String>,
    pub matched_paths: Vec<String>,
    pub readable: bool,
    pub active_state: String,
    pub manifest_present: bool,
    pub category_hints: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AnalyzerLogPaths {
    pub technical_log_path: Option<String>,
    pub user_log_path: Option<String>,
    pub log_directory_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModConflictAnalysisReport {
    pub generated_at: String,
    pub report_version: String,
    pub context: DiagnosticsContext,
    pub sources: AnalysisSources,
    pub overview: AnalyzerOverview,
    pub crash_summary: CrashSummary,
    pub active_mods: Vec<String>,
    pub suspected_mods: Vec<SuspectedMod>,
    pub missing_references: Vec<MissingReference>,
    pub errors: Vec<AnalyzedError>,
    pub unreadable_mods: Vec<String>,
    pub removed_mod_suspected: bool,
    pub removed_mod_reason: Option<String>,
    pub logs: AnalyzerLogPaths,
    pub raw_relevant_log_lines: Vec<String>,
    pub raw_relevant_crash_lines: Vec<String>,
    pub limitations: Vec<String>,
}
