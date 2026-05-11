CREATE TABLE IF NOT EXISTS ets_save_backups (
    backup_id TEXT PRIMARY KEY,
    save_session_id TEXT,
    profile_reference TEXT,
    save_reference TEXT,
    profile_name TEXT,
    save_name TEXT,
    action_reason TEXT NOT NULL,
    created_at_utc TEXT NOT NULL,
    storage_dir TEXT NOT NULL,
    files_json TEXT NOT NULL,
    file_count INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_ets_save_backups_session_created
    ON ets_save_backups (save_session_id, created_at_utc DESC);

CREATE INDEX IF NOT EXISTS idx_ets_save_backups_profile_created
    ON ets_save_backups (profile_reference, created_at_utc DESC);
