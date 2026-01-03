-- Log entries (separate database for performance)
CREATE TABLE IF NOT EXISTS log_entries (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    job_run_id INTEGER NOT NULL,
    timestamp DATETIME DEFAULT CURRENT_TIMESTAMP,
    level TEXT NOT NULL,
    message TEXT NOT NULL,
    source TEXT
);

-- Indexes for performance
CREATE INDEX IF NOT EXISTS idx_log_entries_job_run_id ON log_entries(job_run_id);
CREATE INDEX IF NOT EXISTS idx_log_entries_timestamp ON log_entries(timestamp);
CREATE INDEX IF NOT EXISTS idx_log_entries_level ON log_entries(level);
