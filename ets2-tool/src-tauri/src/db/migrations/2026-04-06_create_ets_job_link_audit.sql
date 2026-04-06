CREATE TABLE IF NOT EXISTS ets_job_link_audit (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    link_id TEXT NOT NULL,
    from_status TEXT,
    to_status TEXT NOT NULL,
    payload_json TEXT,
    created_at_utc TEXT NOT NULL,
    FOREIGN KEY (link_id) REFERENCES ets_job_links(link_id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_ets_job_link_audit_link
    ON ets_job_link_audit(link_id, created_at_utc DESC);
