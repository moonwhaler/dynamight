-- Migration: Add multi-provider sync support
-- This migration adds support for multiple sync destinations (S3, SFTP, WebDAV, etc.)

-- Credentials table for storing encrypted authentication data
CREATE TABLE IF NOT EXISTS credentials (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    provider_type TEXT NOT NULL,  -- 'google_drive', 'onedrive', 's3', 'sftp', 'webdav'
    encrypted_data BLOB NOT NULL,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Add provider columns to jobs table
-- destination_type: 'local', 'google_drive', 'onedrive', 's3', 'sftp', 'webdav'
-- destination_config: JSON object with provider-specific config
-- sync_options: JSON object with unified sync options
-- credential_id: Reference to credentials table for cloud providers

ALTER TABLE jobs ADD COLUMN destination_type TEXT DEFAULT 'local';
ALTER TABLE jobs ADD COLUMN destination_config TEXT DEFAULT NULL;
ALTER TABLE jobs ADD COLUMN sync_options TEXT DEFAULT NULL;
ALTER TABLE jobs ADD COLUMN credential_id INTEGER REFERENCES credentials(id) ON DELETE SET NULL;

-- Index for credential lookups
CREATE INDEX IF NOT EXISTS idx_credentials_provider_type ON credentials(provider_type);
CREATE INDEX IF NOT EXISTS idx_jobs_credential_id ON jobs(credential_id);
CREATE INDEX IF NOT EXISTS idx_jobs_destination_type ON jobs(destination_type);
