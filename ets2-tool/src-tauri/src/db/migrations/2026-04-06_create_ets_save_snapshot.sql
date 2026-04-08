CREATE TABLE IF NOT EXISTS ets_save_snapshot (
    save_session_id TEXT PRIMARY KEY,
    profile_reference TEXT,
    save_reference TEXT,
    quicksave_reference TEXT,
    captured_at_utc TEXT NOT NULL,
    checksum TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS ets_save_depots (
    save_session_id TEXT NOT NULL,
    company_token TEXT NOT NULL,
    city_token TEXT NOT NULL,
    depot_key TEXT NOT NULL,
    discovered INTEGER NOT NULL DEFAULT 1,
    job_offer_count INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (save_session_id, depot_key),
    FOREIGN KEY (save_session_id) REFERENCES ets_save_snapshot(save_session_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS ets_save_visited_cities (
    save_session_id TEXT NOT NULL,
    city_token TEXT NOT NULL,
    PRIMARY KEY (save_session_id, city_token),
    FOREIGN KEY (save_session_id) REFERENCES ets_save_snapshot(save_session_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS ets_save_transport_cargo (
    save_session_id TEXT NOT NULL,
    cargo_token TEXT NOT NULL,
    PRIMARY KEY (save_session_id, cargo_token),
    FOREIGN KEY (save_session_id) REFERENCES ets_save_snapshot(save_session_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS ets_save_snapshot_meta (
    save_session_id TEXT NOT NULL,
    key TEXT NOT NULL,
    value TEXT,
    PRIMARY KEY (save_session_id, key),
    FOREIGN KEY (save_session_id) REFERENCES ets_save_snapshot(save_session_id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_ets_save_depots_city
    ON ets_save_depots(save_session_id, city_token);
