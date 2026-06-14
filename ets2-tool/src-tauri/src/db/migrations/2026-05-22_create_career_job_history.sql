CREATE TABLE IF NOT EXISTS career_job_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    game TEXT,
    profile_id TEXT,
    profile_name TEXT,
    save_path TEXT,
    source TEXT NOT NULL DEFAULT 'unknown',
    source_save_name TEXT,
    detected_at TEXT NOT NULL,
    started_at TEXT,
    completed_at TEXT,
    job_uid TEXT NOT NULL UNIQUE,
    status TEXT NOT NULL DEFAULT 'unknown',
    cargo_name TEXT,
    source_city TEXT,
    destination_city TEXT,
    source_company TEXT,
    destination_company TEXT,
    distance_km REAL,
    revenue INTEGER,
    costs INTEGER,
    penalties INTEGER,
    damage_percent REAL,
    profit INTEGER,
    xp INTEGER,
    level_after INTEGER,
    truck_name TEXT,
    trailer_name TEXT,
    driven_with_truck INTEGER,
    data_origin_note TEXT,
    raw_data_json TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_career_job_history_uid
    ON career_job_history(job_uid);

CREATE INDEX IF NOT EXISTS idx_career_job_history_profile
    ON career_job_history(profile_id, detected_at DESC);

CREATE INDEX IF NOT EXISTS idx_career_job_history_status
    ON career_job_history(status, detected_at DESC);

CREATE INDEX IF NOT EXISTS idx_career_job_history_source
    ON career_job_history(source, detected_at DESC);
