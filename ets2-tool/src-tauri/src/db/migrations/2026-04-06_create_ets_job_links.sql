CREATE TABLE IF NOT EXISTS ets_job_links (
    link_id TEXT PRIMARY KEY,
    profile_id TEXT NOT NULL,
    save_id TEXT NOT NULL,
    vtc_job_id TEXT NOT NULL,
    offer_pointer TEXT,
    job_offer_data_pointer TEXT,
    src_company TEXT NOT NULL,
    src_city TEXT NOT NULL,
    dst_company TEXT NOT NULL,
    dst_city TEXT NOT NULL,
    cargo_id TEXT NOT NULL,
    distance_km REAL NOT NULL DEFAULT 0,
    planned_reward INTEGER NOT NULL DEFAULT 0,
    patch_json TEXT NOT NULL,
    status TEXT NOT NULL,
    error_code TEXT,
    error_message TEXT,
    created_at_utc TEXT NOT NULL,
    updated_at_utc TEXT NOT NULL,
    written_at_utc TEXT,
    requires_load_at_utc TEXT,
    synced_at_utc TEXT,
    completed_at_utc TEXT,
    UNIQUE(vtc_job_id),
    FOREIGN KEY (profile_id) REFERENCES ets_profiles(profile_id) ON DELETE CASCADE,
    FOREIGN KEY (save_id) REFERENCES ets_saves(save_id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_ets_job_links_status
    ON ets_job_links(status, updated_at_utc DESC);
