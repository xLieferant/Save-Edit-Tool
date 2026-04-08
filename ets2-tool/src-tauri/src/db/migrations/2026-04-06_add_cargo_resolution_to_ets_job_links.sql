ALTER TABLE ets_job_links ADD COLUMN requested_cargo_token TEXT;
ALTER TABLE ets_job_links ADD COLUMN resolved_cargo_token TEXT;
ALTER TABLE ets_job_links ADD COLUMN cargo_resolution_mode TEXT;
ALTER TABLE ets_job_links ADD COLUMN cargo_validation_source TEXT;
ALTER TABLE ets_job_links ADD COLUMN cargo_valid_for_snapshot INTEGER NOT NULL DEFAULT 0;
