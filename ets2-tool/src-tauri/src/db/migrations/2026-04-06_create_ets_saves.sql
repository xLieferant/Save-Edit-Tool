CREATE TABLE IF NOT EXISTS ets_saves (
    save_id TEXT PRIMARY KEY,
    profile_id TEXT NOT NULL,
    slot_name TEXT NOT NULL,
    save_path TEXT NOT NULL,
    game_sii_path TEXT NOT NULL,
    is_quicksave INTEGER NOT NULL DEFAULT 0,
    modified_at_utc TEXT NOT NULL,
    created_at_utc TEXT NOT NULL,
    updated_at_utc TEXT NOT NULL,
    last_loaded_at_utc TEXT,
    UNIQUE(profile_id, save_path),
    FOREIGN KEY (profile_id) REFERENCES ets_profiles(profile_id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_ets_saves_profile
    ON ets_saves(profile_id, is_quicksave, modified_at_utc DESC);
