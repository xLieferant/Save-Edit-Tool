CREATE TABLE IF NOT EXISTS ets_profiles (
    profile_id TEXT PRIMARY KEY,
    profile_path TEXT NOT NULL,
    game TEXT NOT NULL DEFAULT 'ets2',
    steam_cloud_enabled INTEGER NOT NULL DEFAULT 0,
    created_at_utc TEXT NOT NULL,
    updated_at_utc TEXT NOT NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_ets_profiles_path
    ON ets_profiles(profile_path);
