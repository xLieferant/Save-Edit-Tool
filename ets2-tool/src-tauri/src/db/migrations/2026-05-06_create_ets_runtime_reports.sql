CREATE TABLE IF NOT EXISTS ets_runtime_reports (
    report_id TEXT PRIMARY KEY,
    created_at_utc TEXT NOT NULL,
    level TEXT NOT NULL,
    action TEXT NOT NULL,
    profile_name TEXT,
    save_name TEXT,
    error_code TEXT,
    user_message TEXT NOT NULL,
    technical_details TEXT,
    context_json TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_ets_runtime_reports_created
    ON ets_runtime_reports (created_at_utc DESC);

CREATE INDEX IF NOT EXISTS idx_ets_runtime_reports_action_created
    ON ets_runtime_reports (action, created_at_utc DESC);
