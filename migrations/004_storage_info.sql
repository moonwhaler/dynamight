-- Migration: Add destination storage info to jobs
-- Storage info is updated after each successful job run to display on dashboard

-- Free space at destination (bytes)
ALTER TABLE jobs ADD COLUMN dest_storage_free INTEGER DEFAULT NULL;

-- Total space at destination (bytes)
ALTER TABLE jobs ADD COLUMN dest_storage_total INTEGER DEFAULT NULL;

-- When storage info was last updated
ALTER TABLE jobs ADD COLUMN dest_storage_updated_at DATETIME DEFAULT NULL;
