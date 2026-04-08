CREATE TABLE IF NOT EXISTS vtc_job_ledger (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    link_id TEXT NOT NULL,
    vtc_job_id TEXT NOT NULL,
    event_type TEXT NOT NULL,
    revenue INTEGER,
    payload_json TEXT,
    created_at_utc TEXT NOT NULL,
    FOREIGN KEY (link_id) REFERENCES ets_job_links(link_id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_vtc_job_ledger_link
    ON vtc_job_ledger(link_id, created_at_utc DESC);
