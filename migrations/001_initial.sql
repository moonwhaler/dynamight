-- Users table (single admin user)
CREATE TABLE IF NOT EXISTS users (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    username TEXT NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Backup jobs table
CREATE TABLE IF NOT EXISTS jobs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    description TEXT,
    enabled INTEGER DEFAULT 1,

    -- Mount configuration
    usb_uuid TEXT,
    mount_point TEXT NOT NULL,
    auto_mount INTEGER DEFAULT 1,
    auto_unmount INTEGER DEFAULT 1,

    -- Source and destination
    source_dirs TEXT NOT NULL,
    backup_subdir TEXT DEFAULT 'backups',

    -- Rsync options
    sync_deletes INTEGER DEFAULT 0,
    rsync_excludes TEXT,
    checksum_mode INTEGER DEFAULT 0,
    compress INTEGER DEFAULT 0,
    dry_run INTEGER DEFAULT 0,
    bandwidth_limit INTEGER,
    verbosity TEXT DEFAULT 'normal',

    -- Metadata
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Schedules table
CREATE TABLE IF NOT EXISTS schedules (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    job_id INTEGER NOT NULL,
    enabled INTEGER DEFAULT 1,

    -- Cron-like schedule
    cron_expression TEXT NOT NULL,

    -- Human-readable alternative
    schedule_type TEXT,
    time_of_day TEXT,
    day_of_week INTEGER,
    day_of_month INTEGER,

    -- Metadata
    last_run_at DATETIME,
    next_run_at DATETIME,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,

    FOREIGN KEY (job_id) REFERENCES jobs(id) ON DELETE CASCADE
);

-- Job executions (history)
CREATE TABLE IF NOT EXISTS job_runs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    job_id INTEGER NOT NULL,
    schedule_id INTEGER,

    -- Execution details
    status TEXT NOT NULL,
    started_at DATETIME,
    completed_at DATETIME,

    -- Results
    exit_code INTEGER,
    files_transferred INTEGER,
    bytes_transferred INTEGER,
    total_size INTEGER,
    error_count INTEGER DEFAULT 0,

    -- Summary
    summary TEXT,

    FOREIGN KEY (job_id) REFERENCES jobs(id) ON DELETE CASCADE,
    FOREIGN KEY (schedule_id) REFERENCES schedules(id) ON DELETE SET NULL
);

-- Note: log_entries table is now in a separate database (logs.db) for performance

-- Sessions table for JWT token management
CREATE TABLE IF NOT EXISTS sessions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL,
    token_hash TEXT NOT NULL,
    expires_at DATETIME NOT NULL,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,

    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

-- Indexes for performance
CREATE INDEX IF NOT EXISTS idx_jobs_enabled ON jobs(enabled);
CREATE INDEX IF NOT EXISTS idx_schedules_job_id ON schedules(job_id);
CREATE INDEX IF NOT EXISTS idx_schedules_next_run ON schedules(next_run_at);
CREATE INDEX IF NOT EXISTS idx_job_runs_job_id ON job_runs(job_id);
CREATE INDEX IF NOT EXISTS idx_job_runs_status ON job_runs(status);
CREATE INDEX IF NOT EXISTS idx_sessions_token ON sessions(token_hash);
CREATE INDEX IF NOT EXISTS idx_sessions_expires ON sessions(expires_at);
