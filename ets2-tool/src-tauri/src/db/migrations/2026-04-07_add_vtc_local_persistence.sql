CREATE TABLE IF NOT EXISTS vtc_companies (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    sync_state TEXT NOT NULL DEFAULT 'local_only',
    remote_id TEXT
);

CREATE TABLE IF NOT EXISTS vtc_company_members (
    company_id INTEGER NOT NULL,
    user_id INTEGER NOT NULL,
    role TEXT NOT NULL,
    PRIMARY KEY (company_id, user_id)
);

CREATE TABLE IF NOT EXISTS vtc_local_context (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    active_user_id INTEGER,
    active_company_id INTEGER,
    updated_at TEXT NOT NULL
);

INSERT OR IGNORE INTO vtc_local_context (
    id,
    active_user_id,
    active_company_id,
    updated_at
) VALUES (1, NULL, NULL, CURRENT_TIMESTAMP);

CREATE INDEX IF NOT EXISTS idx_vtc_companies_sync_state
    ON vtc_companies(sync_state);

CREATE INDEX IF NOT EXISTS idx_vtc_company_members_user
    ON vtc_company_members(user_id);
