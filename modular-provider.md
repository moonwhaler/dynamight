# Modular Sync Provider Architecture

> A plan to restructure Dynamight for multi-provider sync support (rsync, Google Drive, OneDrive, S3, SFTP, WebDAV).

---

## Current State Assessment

The codebase is **tightly coupled to rsync**:

- `BackupService` directly spawns rsync processes
- Job model has rsync-specific fields (`rsync_excludes`, `checksum_mode`, `compress`)
- Command building is hardcoded (`Command::new("rsync")`)
- Output parsing is rsync-specific

**Verdict**: Cannot cleanly support multiple providers without restructuring. However, the existing **service layer pattern** and **separation of concerns** provide a solid foundation.

---

## Phase 1: Backend Provider Abstraction

### 1.1 Provider Trait System

**New file**: `backend/src/services/providers/mod.rs`

```rust
use async_trait::async_trait;

#[async_trait]
pub trait SyncProvider: Send + Sync {
    /// Execute sync operation
    async fn sync(&self, ctx: SyncContext) -> Result<SyncResult, SyncError>;

    /// Validate configuration before execution
    fn validate_config(&self, config: &ProviderConfig) -> Result<(), ValidationError>;

    /// Get provider capabilities (supports delete, compression, etc.)
    fn capabilities(&self) -> ProviderCapabilities;

    /// Parse provider-specific output for progress/stats
    fn parse_output(&self, line: &str) -> Option<SyncProgress>;
}

pub struct ProviderCapabilities {
    pub supports_delete: bool,
    pub supports_compression: bool,
    pub supports_checksum: bool,
    pub supports_bandwidth_limit: bool,
    pub supports_exclude_patterns: bool,
    pub supports_incremental: bool,
}

pub struct SyncContext {
    pub run_id: i64,
    pub source_dirs: Vec<String>,
    pub destination: DestinationConfig,
    pub options: SyncOptions,
    pub log_sender: broadcast::Sender<LogMessage>,
}

pub struct SyncResult {
    pub success: bool,
    pub files_transferred: i64,
    pub bytes_transferred: i64,
    pub files_deleted: i64,
    pub error_message: Option<String>,
}

pub struct SyncProgress {
    pub current_file: Option<String>,
    pub bytes_transferred: i64,
    pub percentage: Option<f32>,
}
```

### 1.2 Provider Directory Structure

```
backend/src/services/providers/
├── mod.rs              # Trait definitions + factory
├── rsync.rs            # Existing rsync logic (extracted from backup_service.rs)
├── google_drive.rs     # Google Drive API (OAuth2 + REST)
├── onedrive.rs         # OneDrive via Microsoft Graph API
├── s3.rs               # AWS S3 compatible (MinIO, Backblaze B2)
├── sftp.rs             # SSH/SFTP via russh
└── webdav.rs           # WebDAV protocol (Nextcloud, ownCloud)
```

### 1.3 Provider Factory

```rust
// backend/src/services/providers/mod.rs

pub fn create_provider(destination: &DestinationConfig) -> Box<dyn SyncProvider> {
    match destination {
        DestinationConfig::Local { .. } => Box::new(RsyncProvider::new()),
        DestinationConfig::GoogleDrive { .. } => Box::new(GoogleDriveProvider::new()),
        DestinationConfig::OneDrive { .. } => Box::new(OneDriveProvider::new()),
        DestinationConfig::S3 { .. } => Box::new(S3Provider::new()),
        DestinationConfig::Sftp { .. } => Box::new(SftpProvider::new()),
        DestinationConfig::WebDav { .. } => Box::new(WebDavProvider::new()),
    }
}
```

### 1.4 Unified Configuration Model

**Refactor** `backend/src/models/job.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DestinationConfig {
    Local {
        mount_point: String,
        backup_subdir: String,
        usb_uuid: Option<String>,
        auto_mount: bool,
        auto_unmount: bool,
    },
    GoogleDrive {
        folder_id: String,
        shared_drive_id: Option<String>,
    },
    OneDrive {
        folder_path: String,
        drive_id: Option<String>,  // For SharePoint/shared drives
    },
    S3 {
        bucket: String,
        prefix: String,
        region: String,
        endpoint: Option<String>,  // Custom endpoint for MinIO/Backblaze
        storage_class: Option<String>,
    },
    Sftp {
        host: String,
        port: u16,
        username: String,
        remote_path: String,
        key_based_auth: bool,
    },
    WebDav {
        url: String,
        remote_path: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncOptions {
    pub delete_extraneous: bool,       // Mirror mode (was: sync_deletes)
    pub exclude_patterns: Vec<String>, // (was: rsync_excludes JSON)
    pub bandwidth_limit_kbps: Option<i32>,
    pub dry_run: bool,
    pub verbosity: Verbosity,

    // Provider-specific options (checksum for rsync, etc.)
    #[serde(default)]
    pub provider_options: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Verbosity {
    Quiet,
    #[default]
    Normal,
    Verbose,
}

// Updated Job struct
pub struct Job {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub enabled: bool,

    // Source configuration
    pub source_dirs: Vec<String>,

    // Destination (polymorphic)
    pub destination: DestinationConfig,

    // Sync options (unified)
    pub sync_options: SyncOptions,

    // Credentials reference (for cloud providers)
    pub credential_id: Option<i64>,

    // Timestamps
    pub created_at: String,
    pub updated_at: String,
}
```

### 1.5 Credentials Management

**New file**: `backend/src/models/credential.rs`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Credential {
    pub id: i64,
    pub name: String,
    pub provider_type: ProviderType,
    pub created_at: String,
    pub updated_at: String,
    // encrypted_data stored separately, never serialized to API
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderType {
    GoogleDrive,
    OneDrive,
    S3,
    Sftp,
    WebDav,
}

// Credential data variants (stored encrypted)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CredentialData {
    OAuth {
        access_token: String,
        refresh_token: String,
        expires_at: i64,
    },
    S3 {
        access_key_id: String,
        secret_access_key: String,
    },
    Sftp {
        password: Option<String>,
        private_key: Option<String>,
        passphrase: Option<String>,
    },
    WebDav {
        username: String,
        password: String,
    },
}
```

**New file**: `backend/src/services/credential_service.rs`

```rust
pub struct CredentialService {
    encryption_key: [u8; 32],  // Derived from JWT_SECRET or separate key
}

impl CredentialService {
    pub fn encrypt(&self, data: &CredentialData) -> Result<Vec<u8>>;
    pub fn decrypt(&self, encrypted: &[u8]) -> Result<CredentialData>;
    pub async fn refresh_oauth_token(&self, credential: &Credential) -> Result<CredentialData>;
}
```

### 1.6 Database Migration

**New file**: `migrations/003_providers.sql`

```sql
-- Credentials table for storing encrypted authentication data
CREATE TABLE credentials (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    provider_type TEXT NOT NULL,  -- 'google_drive', 'onedrive', 's3', 'sftp', 'webdav'
    encrypted_data BLOB NOT NULL,
    created_at TEXT DEFAULT (datetime('now')),
    updated_at TEXT DEFAULT (datetime('now'))
);

-- Add provider columns to jobs table
ALTER TABLE jobs ADD COLUMN destination_type TEXT DEFAULT 'local';
ALTER TABLE jobs ADD COLUMN destination_config TEXT DEFAULT '{}';  -- JSON
ALTER TABLE jobs ADD COLUMN sync_options TEXT DEFAULT '{}';        -- JSON
ALTER TABLE jobs ADD COLUMN credential_id INTEGER REFERENCES credentials(id);

-- Migration: Convert existing rsync-specific columns to new format
-- (Run as data migration script, not DDL)

-- Index for credential lookups
CREATE INDEX idx_credentials_provider_type ON credentials(provider_type);
CREATE INDEX idx_jobs_credential_id ON jobs(credential_id);
```

### 1.7 Refactored BackupService

```rust
// backend/src/services/backup_service.rs

impl BackupService {
    pub async fn execute_job(
        &self,
        job: &Job,
        run_id: i64,
        credential: Option<&CredentialData>,
    ) -> Result<SyncResult> {
        // Get the appropriate provider
        let provider = providers::create_provider(&job.destination);

        // Validate configuration
        provider.validate_config(&job.destination)?;

        // Build context
        let ctx = SyncContext {
            run_id,
            source_dirs: job.source_dirs.clone(),
            destination: job.destination.clone(),
            options: job.sync_options.clone(),
            credential: credential.cloned(),
            log_sender: self.log_tx.clone(),
        };

        // Execute sync
        let result = provider.sync(ctx).await?;

        Ok(result)
    }
}
```

---

## Phase 2: Frontend Restructuring

### 2.1 Provider Selection Component

**New file**: `frontend/src/components/jobs/ProviderSelector.svelte`

```svelte
<script lang="ts">
    import { type DestinationType } from '$lib/types';

    interface Props {
        selected: DestinationType;
        onSelect: (type: DestinationType) => void;
    }

    let { selected, onSelect }: Props = $props();

    const providers = [
        { type: 'local', name: 'Local / USB', icon: 'hard-drive', description: 'Rsync to local or mounted drives' },
        { type: 'google_drive', name: 'Google Drive', icon: 'cloud', description: 'Sync to Google Drive folder' },
        { type: 'onedrive', name: 'OneDrive', icon: 'cloud', description: 'Sync to Microsoft OneDrive' },
        { type: 's3', name: 'S3 / Compatible', icon: 'database', description: 'AWS S3, MinIO, Backblaze B2' },
        { type: 'sftp', name: 'SFTP', icon: 'server', description: 'Sync via SSH/SFTP' },
        { type: 'webdav', name: 'WebDAV', icon: 'globe', description: 'Nextcloud, ownCloud, etc.' },
    ] as const;
</script>

<div class="grid grid-cols-2 md:grid-cols-3 gap-3">
    {#each providers as provider}
        <button
            type="button"
            class="provider-card"
            class:selected={selected === provider.type}
            onclick={() => onSelect(provider.type)}
        >
            <Icon name={provider.icon} />
            <span class="font-medium">{provider.name}</span>
            <span class="text-xs text-muted">{provider.description}</span>
        </button>
    {/each}
</div>
```

### 2.2 Provider Configuration Components

```
frontend/src/components/jobs/providers/
├── index.ts                      # Re-exports all providers
├── LocalDestination.svelte       # USB/mount config (extracted from JobDetail)
├── GoogleDriveDestination.svelte # Google Drive folder picker
├── OneDriveDestination.svelte    # OneDrive folder picker
├── S3Destination.svelte          # Bucket, region, endpoint config
├── SftpDestination.svelte        # Host, port, path config
└── WebDavDestination.svelte      # URL, path config
```

**Example**: `frontend/src/components/jobs/providers/S3Destination.svelte`

```svelte
<script lang="ts">
    import type { S3DestinationConfig } from '$lib/types';

    interface Props {
        config: S3DestinationConfig;
        credentialId: number | null;
        onUpdate: (config: S3DestinationConfig) => void;
    }

    let { config, credentialId, onUpdate }: Props = $props();

    const regions = [
        'us-east-1', 'us-west-2', 'eu-west-1', 'eu-central-1',
        'ap-southeast-1', 'ap-northeast-1', 'custom'
    ];
</script>

<div class="space-y-4">
    <CredentialSelector
        providerType="s3"
        selected={credentialId}
        onSelect={(id) => /* update credential */}
    />

    <div class="form-group">
        <label for="bucket">Bucket Name</label>
        <input
            id="bucket"
            type="text"
            bind:value={config.bucket}
            placeholder="my-backup-bucket"
        />
    </div>

    <div class="form-group">
        <label for="region">Region</label>
        <select id="region" bind:value={config.region}>
            {#each regions as region}
                <option value={region}>{region}</option>
            {/each}
        </select>
    </div>

    {#if config.region === 'custom'}
        <div class="form-group">
            <label for="endpoint">Custom Endpoint</label>
            <input
                id="endpoint"
                type="url"
                bind:value={config.endpoint}
                placeholder="https://s3.example.com"
            />
            <p class="hint">For MinIO, Backblaze B2, or other S3-compatible services</p>
        </div>
    {/if}

    <div class="form-group">
        <label for="prefix">Path Prefix</label>
        <input
            id="prefix"
            type="text"
            bind:value={config.prefix}
            placeholder="backups/server1/"
        />
    </div>

    <div class="form-group">
        <label for="storage-class">Storage Class</label>
        <select id="storage-class" bind:value={config.storage_class}>
            <option value="STANDARD">Standard</option>
            <option value="STANDARD_IA">Standard-IA (Infrequent Access)</option>
            <option value="GLACIER">Glacier</option>
            <option value="DEEP_ARCHIVE">Glacier Deep Archive</option>
        </select>
    </div>
</div>
```

### 2.3 Unified Sync Options Component

**Refactor**: `RsyncOptions.svelte` → `SyncOptions.svelte`

```svelte
<script lang="ts">
    import type { SyncOptions, ProviderCapabilities } from '$lib/types';

    interface Props {
        options: SyncOptions;
        capabilities: ProviderCapabilities;
        onUpdate: (options: SyncOptions) => void;
    }

    let { options, capabilities, onUpdate }: Props = $props();
</script>

<div class="space-y-4">
    <!-- Common options (shown if supported) -->
    {#if capabilities.supports_delete}
        <label class="toggle-option">
            <input type="checkbox" bind:checked={options.delete_extraneous} />
            <div>
                <span class="font-medium">Mirror Mode</span>
                <p class="text-sm text-muted">Delete files at destination that don't exist in source</p>
            </div>
        </label>
    {/if}

    {#if capabilities.supports_exclude_patterns}
        <div class="form-group">
            <label>Exclude Patterns</label>
            <ExcludePatternEditor bind:patterns={options.exclude_patterns} />
        </div>
    {/if}

    {#if capabilities.supports_bandwidth_limit}
        <div class="form-group">
            <label>Bandwidth Limit (KB/s)</label>
            <input type="number" bind:value={options.bandwidth_limit_kbps} min="0" />
            <p class="hint">0 = unlimited</p>
        </div>
    {/if}

    <label class="toggle-option">
        <input type="checkbox" bind:checked={options.dry_run} />
        <div>
            <span class="font-medium">Dry Run</span>
            <p class="text-sm text-muted">Simulate sync without making changes</p>
        </div>
    </label>

    <!-- Provider-specific options slot -->
    <slot name="provider-options" />
</div>
```

### 2.4 OAuth Flow Component

**New file**: `frontend/src/components/credentials/OAuthConnect.svelte`

```svelte
<script lang="ts">
    import { api } from '$lib/api';

    interface Props {
        provider: 'google_drive' | 'onedrive';
        onConnected: (credentialId: number) => void;
    }

    let { provider, onConnected }: Props = $props();
    let connecting = $state(false);

    async function startOAuth() {
        connecting = true;

        // Get OAuth URL from backend
        const { auth_url, state } = await api.credentials.getOAuthUrl(provider);

        // Open popup
        const popup = window.open(auth_url, 'oauth', 'width=500,height=600');

        // Listen for callback
        window.addEventListener('message', async (event) => {
            if (event.data.type === 'oauth_callback' && event.data.state === state) {
                const credential = await api.credentials.completeOAuth(provider, event.data.code);
                onConnected(credential.id);
                connecting = false;
            }
        });
    }
</script>

<button
    type="button"
    class="btn btn-outline"
    onclick={startOAuth}
    disabled={connecting}
>
    {#if connecting}
        <Spinner size="sm" />
        Connecting...
    {:else}
        <Icon name={provider === 'google_drive' ? 'google' : 'microsoft'} />
        Connect {provider === 'google_drive' ? 'Google Drive' : 'OneDrive'}
    {/if}
</button>
```

### 2.5 Credential Manager

**New file**: `frontend/src/components/credentials/CredentialSelector.svelte`

```svelte
<script lang="ts">
    import { api } from '$lib/api';
    import type { Credential, ProviderType } from '$lib/types';

    interface Props {
        providerType: ProviderType;
        selected: number | null;
        onSelect: (id: number | null) => void;
    }

    let { providerType, selected, onSelect }: Props = $props();

    let credentials = $state<Credential[]>([]);
    let showAddModal = $state(false);

    $effect(() => {
        api.credentials.list(providerType).then(c => credentials = c);
    });
</script>

<div class="credential-selector">
    <label>Credentials</label>

    {#if credentials.length === 0}
        <p class="text-muted">No credentials configured</p>
    {:else}
        <select value={selected} onchange={(e) => onSelect(Number(e.target.value) || null)}>
            <option value="">Select credentials...</option>
            {#each credentials as cred}
                <option value={cred.id}>{cred.name}</option>
            {/each}
        </select>
    {/if}

    <button type="button" class="btn btn-sm" onclick={() => showAddModal = true}>
        + Add Credentials
    </button>
</div>

{#if showAddModal}
    <CredentialModal {providerType} onClose={() => showAddModal = false} />
{/if}
```

### 2.6 Updated TypeScript Types

**Update**: `frontend/src/lib/types.ts`

```typescript
// Destination types
export type DestinationType = 'local' | 'google_drive' | 'onedrive' | 's3' | 'sftp' | 'webdav';

export interface LocalDestinationConfig {
    type: 'local';
    mount_point: string;
    backup_subdir: string;
    usb_uuid?: string;
    auto_mount: boolean;
    auto_unmount: boolean;
}

export interface GoogleDriveDestinationConfig {
    type: 'google_drive';
    folder_id: string;
    shared_drive_id?: string;
}

export interface OneDriveDestinationConfig {
    type: 'onedrive';
    folder_path: string;
    drive_id?: string;
}

export interface S3DestinationConfig {
    type: 's3';
    bucket: string;
    prefix: string;
    region: string;
    endpoint?: string;
    storage_class?: string;
}

export interface SftpDestinationConfig {
    type: 'sftp';
    host: string;
    port: number;
    username: string;
    remote_path: string;
    key_based_auth: boolean;
}

export interface WebDavDestinationConfig {
    type: 'webdav';
    url: string;
    remote_path: string;
}

export type DestinationConfig =
    | LocalDestinationConfig
    | GoogleDriveDestinationConfig
    | OneDriveDestinationConfig
    | S3DestinationConfig
    | SftpDestinationConfig
    | WebDavDestinationConfig;

// Sync options
export interface SyncOptions {
    delete_extraneous: boolean;
    exclude_patterns: string[];
    bandwidth_limit_kbps?: number;
    dry_run: boolean;
    verbosity: 'quiet' | 'normal' | 'verbose';
    provider_options?: Record<string, unknown>;
}

// Provider capabilities
export interface ProviderCapabilities {
    supports_delete: boolean;
    supports_compression: boolean;
    supports_checksum: boolean;
    supports_bandwidth_limit: boolean;
    supports_exclude_patterns: boolean;
    supports_incremental: boolean;
}

// Credentials
export type ProviderType = 'google_drive' | 'onedrive' | 's3' | 'sftp' | 'webdav';

export interface Credential {
    id: number;
    name: string;
    provider_type: ProviderType;
    created_at: string;
    updated_at: string;
}

// Updated Job interface
export interface Job {
    id: number;
    name: string;
    description?: string;
    enabled: boolean;
    source_dirs: string[];
    destination: DestinationConfig;
    sync_options: SyncOptions;
    credential_id?: number;
    created_at: string;
    updated_at: string;
}
```

---

## Phase 3: Settings Dialog Reference

### Local / rsync Destination

| Setting | Type | Description |
|---------|------|-------------|
| Mount Point | Path picker | Local path for backup destination |
| USB UUID | Dropdown | Auto-detect USB drive by UUID |
| Auto Mount | Toggle | Mount drive automatically before backup |
| Auto Unmount | Toggle | Unmount drive after backup completes |
| Backup Subdirectory | Text | Folder name within mount point |

**Provider-specific options:**
| Setting | Type | Description |
|---------|------|-------------|
| Checksum Mode | Toggle | Compare files by hash instead of mtime/size |
| Compression | Toggle | Compress data during transfer |

### Google Drive

| Setting | Type | Description |
|---------|------|-------------|
| Account | OAuth button | Connect Google account |
| Destination Folder | Folder picker | Browse and select Drive folder |
| Use Shared Drives | Toggle | Access shared/team drives |

### OneDrive

| Setting | Type | Description |
|---------|------|-------------|
| Account | OAuth button | Connect Microsoft account |
| Destination Folder | Path input | Path within OneDrive (e.g., `/Backups/Server`) |
| SharePoint Site | Optional | For SharePoint document libraries |

### S3 / Compatible

| Setting | Type | Description |
|---------|------|-------------|
| Credentials | Selector | Select saved Access Key + Secret |
| Bucket | Text | S3 bucket name |
| Region | Dropdown | AWS region or "custom" |
| Custom Endpoint | URL | For MinIO, Backblaze B2, Wasabi, etc. |
| Path Prefix | Text | Object key prefix (e.g., `backups/daily/`) |
| Storage Class | Dropdown | STANDARD, STANDARD_IA, GLACIER, etc. |

### SFTP

| Setting | Type | Description |
|---------|------|-------------|
| Credentials | Selector | Select saved SSH credentials |
| Host | Text | Server hostname or IP |
| Port | Number | SSH port (default: 22) |
| Username | Text | SSH username |
| Remote Path | Text | Destination directory on server |
| Authentication | Radio | Password or SSH Key |

### WebDAV

| Setting | Type | Description |
|---------|------|-------------|
| Credentials | Selector | Select saved WebDAV credentials |
| Server URL | URL | WebDAV endpoint (e.g., `https://nextcloud.example.com/remote.php/dav`) |
| Remote Path | Text | Destination folder path |

### Universal Sync Options (All Providers)

| Setting | Type | Description |
|---------|------|-------------|
| Mirror Mode | Toggle | Delete destination files not in source |
| Exclude Patterns | Tag input | Glob patterns to skip (*.tmp, node_modules, .git) |
| Bandwidth Limit | Number | Throttle transfer speed (KB/s), 0 = unlimited |
| Dry Run | Toggle | Simulate sync without making changes |
| Verbosity | Dropdown | quiet / normal / verbose |

---

## Phase 4: API Endpoints

### New Credential Endpoints

```
POST   /api/credentials                    # Create credential
GET    /api/credentials                    # List all credentials
GET    /api/credentials?provider=s3        # List by provider type
GET    /api/credentials/:id                # Get credential (without secrets)
PUT    /api/credentials/:id                # Update credential
DELETE /api/credentials/:id                # Delete credential

# OAuth flow
GET    /api/credentials/oauth/:provider/url    # Get OAuth authorization URL
POST   /api/credentials/oauth/:provider/callback  # Complete OAuth flow
POST   /api/credentials/oauth/:provider/refresh   # Refresh OAuth token
```

### New Provider Endpoints

```
GET    /api/providers                      # List available providers
GET    /api/providers/:type/capabilities   # Get provider capabilities

# Cloud folder browsing
GET    /api/providers/google_drive/browse?folder_id=...&credential_id=...
GET    /api/providers/onedrive/browse?path=...&credential_id=...
GET    /api/providers/s3/buckets?credential_id=...
GET    /api/providers/sftp/browse?path=...&credential_id=...
GET    /api/providers/webdav/browse?path=...&credential_id=...
```

### Updated Job Endpoints

```
# Existing endpoints work with new Job structure
POST   /api/jobs        # Create job with destination config
PUT    /api/jobs/:id    # Update job with destination config
```

---

## Phase 5: Recommended Rust Dependencies

Add to `backend/Cargo.toml`:

```toml
[dependencies]
# Existing dependencies...

# Provider implementations
aws-sdk-s3 = "1.0"              # S3 and compatible services
google-drive3 = "5.0"           # Google Drive API
russh = "0.44"                  # Async SSH client
russh-sftp = "2.0"              # SFTP over russh
reqwest = { version = "0.12", features = ["json", "stream"] }  # HTTP for WebDAV/OneDrive

# OAuth2 flows
oauth2 = "4.4"

# Credential encryption
aes-gcm = "0.10"                # AES-256-GCM encryption
rand = "0.8"                    # Secure random for IVs

# Async utilities
futures = "0.3"                 # Stream utilities for chunked uploads
tokio-util = { version = "0.7", features = ["io"] }
```

---

## Phase 6: Implementation Order

### Stage 1: Foundation (Backend)
1. Create `providers/mod.rs` with trait definitions
2. Extract rsync logic into `providers/rsync.rs`
3. Create credential model and migration
4. Implement `CredentialService` with encryption
5. Refactor `BackupService` to use provider factory

### Stage 2: Foundation (Frontend)
1. Update TypeScript types
2. Create `ProviderSelector` component
3. Extract `LocalDestination` from `JobDetail`
4. Create `SyncOptions` component (refactor from `RsyncOptions`)
5. Update `JobDetail` to use new components

### Stage 3: Cloud Providers
1. Implement S3 provider (simplest cloud provider)
2. Add S3 credential UI and destination component
3. Implement SFTP provider
4. Add SFTP credential UI and destination component
5. Implement WebDAV provider
6. Add WebDAV UI components

### Stage 4: OAuth Providers
1. Implement OAuth2 flow infrastructure
2. Implement Google Drive provider
3. Add Google Drive UI with folder browser
4. Implement OneDrive provider (Microsoft Graph)
5. Add OneDrive UI with folder browser

### Stage 5: Polish
1. Add credential management page
2. Implement provider capability checks in UI
3. Add migration tool for existing jobs
4. Update documentation
5. Add provider-specific progress parsing

---

## Architecture Diagram

```
┌─────────────────────────────────────────────────────────────────────────┐
│                              Frontend                                    │
│  ┌──────────────┐  ┌────────────────────┐  ┌─────────────────────────┐ │
│  │   Provider   │  │  Provider Config   │  │     SyncOptions         │ │
│  │   Selector   │→ │  Components        │→ │  (capability-aware)     │ │
│  │              │  │  - Local           │  │                         │ │
│  │  ○ Local     │  │  - GoogleDrive     │  │  ☑ Mirror Mode          │ │
│  │  ○ GDrive    │  │  - OneDrive        │  │  ☑ Exclude Patterns     │ │
│  │  ● S3        │  │  - S3              │  │  ☐ Bandwidth Limit      │ │
│  │  ○ SFTP      │  │  - SFTP            │  │  ☐ Dry Run              │ │
│  │  ○ WebDAV    │  │  - WebDAV          │  │                         │ │
│  └──────────────┘  └────────────────────┘  └─────────────────────────┘ │
└─────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                           Backend API                                    │
│  POST /api/jobs { destination: { type: "s3", bucket: "...", ... } }     │
│  POST /api/credentials { provider: "s3", data: { access_key: ... } }    │
│  GET  /api/providers/s3/capabilities                                     │
└─────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                         Provider Factory                                 │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │  fn create_provider(dest: &DestinationConfig) -> Box<dyn Sync>  │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│         │              │              │              │              │   │
│         ▼              ▼              ▼              ▼              ▼   │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐ │
│  │  Rsync   │  │  Google  │  │ OneDrive │  │    S3    │  │   SFTP   │ │
│  │ Provider │  │  Drive   │  │ Provider │  │ Provider │  │ Provider │ │
│  │          │  │ Provider │  │          │  │          │  │          │ │
│  │ Command  │  │ REST API │  │  Graph   │  │ AWS SDK  │  │  russh   │ │
│  │ spawning │  │ + OAuth  │  │   API    │  │          │  │          │ │
│  └──────────┘  └──────────┘  └──────────┘  └──────────┘  └──────────┘ │
└─────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                        Unified Logging                                   │
│  All providers emit SyncProgress → broadcast channel → WebSocket → UI   │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## Summary

| Aspect | Current | After Restructure |
|--------|---------|-------------------|
| Provider support | rsync only | Pluggable via trait |
| Destination config | Fixed DB columns | Polymorphic JSON |
| Credentials | N/A | Encrypted store with OAuth |
| UI | rsync-specific | Dynamic per-provider |
| Adding new provider | N/A | ~1 backend + 1 frontend file |
| Capabilities | Hardcoded | Per-provider declaration |

This architecture enables:
- Adding new providers by implementing `SyncProvider` trait
- Provider-specific UI through conditional component rendering
- Secure credential storage with encryption at rest
- OAuth flows for cloud services
- Unified progress/logging across all providers
- Backward compatibility with existing rsync jobs
