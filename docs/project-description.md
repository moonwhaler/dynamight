# Dynamight - Project Architecture Documentation

> A web-based multi-destination backup manager with Rust backend and Svelte frontend, supporting local rsync, cloud storage (S3, Google Drive, OneDrive), SFTP, and WebDAV destinations.

## Table of Contents

1. [Overview](#overview)
2. [Technology Stack](#technology-stack)
3. [Architecture Diagram](#architecture-diagram)
4. [Directory Structure](#directory-structure)
5. [Backend Architecture](#backend-architecture)
6. [Frontend Architecture](#frontend-architecture)
7. [Provider System](#provider-system)
8. [Data Flow](#data-flow)
9. [Security Model](#security-model)
10. [Docker Deployment](#docker-deployment)
11. [Native Linux Deployment](#native-linux-deployment)
12. [Configuration Reference](#configuration-reference)
13. [File Reference](#file-reference)
14. [Directory Compression](#directory-compression)
15. [Job List Column Preferences](#job-list-column-preferences)

---

## Overview

Dynamight is a self-hosted backup management application that provides a web UI for configuring and executing backups to multiple destinations. It supports local rsync-based backups with USB mount automation, as well as cloud storage providers like AWS S3, Google Drive, OneDrive, SFTP servers, and WebDAV endpoints.

### Core Capabilities

- **Multi-Provider Support**: Backup to local drives, S3, Google Drive, OneDrive, SFTP, or WebDAV
- **Job Management**: Create, edit, delete, clone, and execute backup jobs
- **USB Mount Support**: Auto-detect USB drives by UUID, mount/unmount automatically
- **Scheduling**: Timezone-aware cron-based job scheduling (daily, weekly, monthly, custom)
- **Directory Compression**: Archive source directories (tar.gz or zip) before transfer, with optional encryption, versioned retention, and excluded path support
- **Real-time Logs**: WebSocket streaming of backup progress
- **Credential Management**: Encrypted storage for cloud provider credentials
- **Two-Factor Authentication**: TOTP-based 2FA with recovery codes
- **Rate Limiting**: Protection against brute-force attacks
- **Customizable Job Table**: Persistent per-browser column visibility, order, and resize

### Supported Providers

| Provider | Description | Authentication |
|----------|-------------|----------------|
| Local/USB | Rsync to local or mounted drives | None (filesystem) |
| AWS S3 | S3 and S3-compatible (MinIO, Backblaze B2, DigitalOcean Spaces) | Access Key + Secret |
| Google Drive | Google Drive folders and shared drives | OAuth2 |
| OneDrive | Microsoft OneDrive personal and business | OAuth2 |
| SFTP | SSH/SFTP servers | Password or SSH Key |
| WebDAV | Nextcloud, ownCloud, generic WebDAV | Username + Password |

---

## Technology Stack

| Layer | Technology | Purpose |
|-------|------------|---------|
| Backend Framework | Axum 0.7 | Async web framework with tower middleware |
| Runtime | Tokio | Async runtime for Rust |
| Database | SQLite + sqlx | Embedded database with compile-time checked queries |
| Authentication | Argon2 + JWT | Password hashing and stateless sessions |
| 2FA | totp-rs | TOTP authentication with QR code generation |
| Credential Encryption | AES-256-GCM | Secure storage for provider credentials |
| Cloud Storage | aws-sdk-s3 | S3 and S3-compatible storage |
| SFTP | russh, russh-sftp | SSH/SFTP client |
| HTTP Client | reqwest | WebDAV, Google Drive, OneDrive APIs |
| Frontend Framework | Svelte 5 | Reactive UI with runes ($state, $derived, $effect) |
| Build Tool | Vite 6 | Fast frontend bundling |
| Styling | Tailwind CSS 4 | Utility-first CSS |
| Containerization | Docker + Alpine | Minimal runtime image |

---

## Architecture Diagram

```
+-------------------------------------------------------------------------+
|                          Docker Container / Host                          |
|  +-------------------------------------------------------------------+  |
|  |                       Dynamight Binary                             |  |
|  |  +----------------+  +------------------+  +-------------------+   |  |
|  |  |   Axum Router  |  |    Services      |  |    Scheduler      |   |  |
|  |  |                |  |                  |  |   (Background)    |   |  |
|  |  |  /api/*        |  |  - Auth          |  |                   |   |  |
|  |  |  /ws/*         |  |  - Backup        |  |  Checks cron      |   |  |
|  |  |  /* (static)   |  |  - Compress      |  |  every 60s        |   |  |
|  |  |                |  |  - Credential    |  |  Timezone-aware   |   |  |
|  |  |                |  |  - Mount         |  |                   |   |  |
|  |  |                |  |  - RateLimit     |  |                   |   |  |
|  |  |                |  |  - TOTP          |  |                   |   |  |
|  |  +----------------+  +------------------+  +-------------------+   |  |
|  |         |                    |                     |               |  |
|  |         v                    v                     v               |  |
|  |  +-----------------------------------------------------------+    |  |
|  |  |                    Provider Layer                          |    |  |
|  |  |  +--------+ +------+ +-------+ +------+ +------+ +------+  |    |  |
|  |  |  | Rsync  | |  S3  | | GDrive| |OneDrv| | SFTP | |WebDAV|  |    |  |
|  |  |  +--------+ +------+ +-------+ +------+ +------+ +------+  |    |  |
|  |  +-----------------------------------------------------------+    |  |
|  |         |                    |                     |               |  |
|  |         v                    v                     v               |  |
|  |  +-----------------------------------------------------------+    |  |
|  |  |              SQLite Databases (WAL mode)                   |    |  |
|  |  |  Main: users | jobs | schedules | credentials | settings   |    |  |
|  |  |  Logs: job_runs | log_entries                              |    |  |
|  |  +-----------------------------------------------------------+    |  |
|  +-------------------------------------------------------------------+  |
|                              |                                          |
+------------------------------|-----------------------------------------+
                               v
+-------------------------------------------------------------------------+
|                         External Services                                 |
|  Local: rsync, mount/umount, blkid, lsblk                               |
|  Cloud: S3 API, Google Drive API, Microsoft Graph API                   |
|  Remote: SFTP servers, WebDAV servers                                   |
+-------------------------------------------------------------------------+
```

---

## Directory Structure

```
dynamight-web/
├── Cargo.toml                 # Rust workspace definition
├── Dockerfile                 # Multi-stage build (node -> rust -> alpine)
├── docker-compose.yml         # Container orchestration
├── .env.example               # Environment variable template
├── project-description.md     # This file
│
├── scripts/                   # Build and deployment scripts
│   ├── dev.sh                 # Development server (backend + frontend)
│   ├── build.sh               # Production build and packaging
│   └── install.sh             # System service installation
│
├── migrations/
│   ├── 001_initial.sql        # Base schema (users, jobs, schedules)
│   ├── 002_logs.sql           # Logs database schema
│   └── 003_providers.sql      # Provider and credential support
│
├── backend/                   # Rust backend application
│   ├── Cargo.toml             # Backend dependencies
│   └── src/
│       ├── main.rs            # Entry point, server setup, router
│       ├── config.rs          # Environment configuration
│       ├── errors.rs          # Error types
│       ├── extractors.rs      # Axum extractors
│       ├── middleware.rs      # Auth middleware
│       │
│       ├── db/
│       │   └── mod.rs         # Database initialization, migrations
│       │
│       ├── models/            # Data structures
│       │   ├── mod.rs
│       │   ├── user.rs        # User, LoginRequest
│       │   ├── job.rs         # Job, CreateJobRequest
│       │   ├── schedule.rs    # Schedule, CreateScheduleRequest
│       │   ├── log_entry.rs   # LogEntry, LogMessage, JobRun
│       │   ├── credential.rs  # Credential types for providers
│       │   └── destination.rs # DestinationConfig variants
│       │
│       ├── handlers/          # HTTP request handlers
│       │   ├── mod.rs
│       │   ├── auth.rs        # Login, logout, setup, password
│       │   ├── totp.rs        # 2FA setup, validate, recovery
│       │   ├── jobs.rs        # CRUD + run, cancel, clone
│       │   ├── schedules.rs   # Schedule CRUD
│       │   ├── logs.rs        # Run history and logs
│       │   ├── system.rs      # Drives, mounts, browse, health
│       │   ├── settings.rs    # App settings
│       │   ├── credentials.rs # Credential CRUD
│       │   ├── providers.rs   # Provider info, test connection
│       │   └── websocket.rs   # WebSocket handlers
│       │
│       └── services/          # Business logic layer
│           ├── mod.rs
│           ├── auth_service.rs      # Password hashing, JWT
│           ├── totp_service.rs      # TOTP generation/validation
│           ├── backup_service.rs    # Job execution orchestration
│           ├── compress_service.rs  # Directory compression and retention
│           ├── mount_service.rs     # USB mount/unmount
│           ├── scheduler_service.rs # Cron job runner (timezone-aware)
│           ├── credential_service.rs # Credential encryption
│           ├── rate_limit_service.rs # Rate limiting
│           │
│           └── providers/           # Sync provider implementations
│               ├── mod.rs           # SyncProvider trait, factory
│               ├── rsync.rs         # Local/USB rsync
│               ├── s3.rs            # AWS S3 and compatible
│               ├── googledrive.rs   # Google Drive API
│               ├── onedrive.rs      # Microsoft OneDrive
│               ├── sftp.rs          # SFTP/SSH
│               └── webdav.rs        # WebDAV
│
└── frontend/                  # Svelte SPA
    ├── package.json
    ├── vite.config.ts
    ├── svelte.config.js
    ├── tailwind.config.js
    │
    └── src/
        ├── main.ts
        ├── app.css
        ├── App.svelte
        │
        ├── lib/
        │   ├── types.ts             # TypeScript interfaces
        │   ├── api.ts               # API client
        │   │
        │   └── stores/
        │       ├── auth.ts          # Auth state
        │       ├── jobs.ts          # Jobs state
        │       ├── logs.ts          # WebSocket status
        │       ├── theme.ts         # Dark/light theme
        │       ├── tablePreferences.ts  # Job list column visibility/order/widths
        │       ├── viewPreferences.ts
        │       ├── preferences.ts
        │       ├── fileBrowser.ts   # Directory browser state
        │       └── language.ts      # UI language selection
        │
        ├── components/
        │   ├── layout/
        │   │   ├── Layout.svelte
        │   │   ├── Navbar.svelte
        │   │   └── Sidebar.svelte
        │   │
        │   ├── jobs/
        │   │   ├── JobCard.svelte
        │   │   ├── JobListRow.svelte          # Dynamic row renderer
        │   │   ├── ColumnSelector.svelte      # Column visibility/order popover
        │   │   ├── ProviderSelector.svelte    # Provider selection UI
        │   │   ├── CredentialSelector.svelte  # Credential picker (used inside providers/)
        │   │   ├── TestConnection.svelte      # Connection tester (used inside providers/)
        │   │   ├── CompressOptions.svelte     # Directory compression settings
        │   │   ├── SyncOptions.svelte         # Unified sync options
        │   │   ├── RsyncOptions.svelte        # Rsync-specific options
        │   │   ├── SchedulePicker.svelte
        │   │   ├── PathSelector.svelte
        │   │   ├── SinglePathSelector.svelte
        │   │   │
        │   │   └── providers/                 # Provider-specific forms
        │   │       ├── LocalDestination.svelte
        │   │       ├── S3Destination.svelte
        │   │       ├── GoogleDriveDestination.svelte
        │   │       ├── OneDriveDestination.svelte
        │   │       ├── SftpDestination.svelte
        │   │       └── WebDavDestination.svelte
        │   │
        │   ├── settings/
        │   │   ├── CredentialsManager.svelte  # Credential management
        │   │   └── CredentialEditModal.svelte
        │   │
        │   ├── logs/
        │   │   ├── LogViewer.svelte
        │   │   └── RunLogModal.svelte
        │   │
        │   ├── ui/
        │   │   ├── ConfirmDialog.svelte
        │   │   ├── Toast.svelte
        │   │   └── HelpTooltip.svelte
        │   │
        │   ├── SettingsModal.svelte
        │   ├── TotpSetup.svelte
        │   ├── TotpVerification.svelte
        │   └── PasswordStrength.svelte
        │
        └── routes/
            ├── Login.svelte
            ├── Setup.svelte
            ├── Dashboard.svelte
            ├── Jobs.svelte
            ├── JobDetail.svelte
            └── History.svelte
```

---

## Backend Architecture

### Entry Point (`main.rs`)

Application bootstrap order:

1. Initialize tracing (logging)
2. Load configuration from environment
3. Connect to SQLite databases (main + logs)
4. Run migrations
5. Initialize services (Auth, Backup, Mount, Credential, RateLimit, TOTP)
6. Start SchedulerService in background task
7. Build Axum router with routes
8. Start HTTP server

### Service Layer

#### AuthService (`services/auth_service.rs`)

- `hash_password(password)` - Argon2 hash
- `verify_password(password, hash)` - Verification
- `generate_token(user_id)` - JWT (24h expiry)
- `validate_token(token)` - JWT validation

#### TOTPService (`services/totp_service.rs`)

- `generate_secret()` - New TOTP secret
- `generate_qr_code(secret, username)` - QR code for authenticator apps
- `validate_code(secret, code)` - Verify TOTP code
- `generate_recovery_codes()` - Generate backup codes

#### BackupService (`services/backup_service.rs`)

Orchestrates backup execution:

1. Load job configuration from database
2. Resolve provider based on destination type
3. Retrieve and decrypt credentials if needed
4. **Compression phase** (if `compress_dirs` enabled): compress each source directory into an archive in the staging path, then use the staging path as the effective source for the sync. Retention cleanup runs after each archive is created. Dry-run mode logs what would be compressed without writing files.
5. **Space pre-flight check** (Local provider only, skipped when compression is active)
6. Create `SyncContext` with effective source directories
7. Call provider's `sync()` method
8. Stream logs via WebSocket broadcast
9. Store logs in database
10. Handle cancellation (checked before each compression step and throughout provider sync)

#### CredentialService (`services/credential_service.rs`)

- **Encryption**: AES-256-GCM
- **Key Derivation**: SHA-256(JWT_SECRET + salt)
- `create()` - Encrypt and store
- `get_decrypted()` - Retrieve and decrypt
- `update()` / `delete()` - Manage credentials

#### MountService (`services/mount_service.rs`)

- `get_device_by_uuid(uuid)` - Device lookup via `blkid`
- `mount_by_uuid(uuid, mount_point)` - Mount USB
- `unmount(mount_point)` - Sync and unmount
- `list_usb_drives()` - Parse `lsblk -J`
- `browse_path(path)` - Directory listing

#### CompressService (`services/compress_service.rs`)

Handles per-directory archiving before provider sync:

- `compress_directory(source_dir, job_id, opts, exclude_paths, log_fn, is_cancelled)` — archives a single source directory into the staging path, honouring cancellation and excluded sub-paths
- `cleanup_old_archives(staging_path, source_dir, job_id, max_count)` — deletes oldest archives beyond the retention limit, matching by sanitised dir name + job ID
- **Archive naming**: `[<timestamp>_][<custom_name>_]<sanitised_dir_name>_<job_id>.<ext>`
  - Job ID is embedded in every filename so multiple jobs can safely share the same staging directory
  - Timestamp format: `YYYY-MM-DDTHH-MM-SS`
- **Supported formats**:
  - `tar_gz`: gzip-compressed tar (`.tar.gz`); or uncompressed with `store_only` (`.tar`)
  - `zip`: ZIP archive (`.zip`)
- **Encryption**:
  - `tar_gz` + password: two-step process — create tar, then pipe through `openssl enc -aes-256-cbc -pbkdf2`; output has `.enc` suffix (e.g. `.tar.gz.enc`). Password is passed via stdin, not on the command line.
  - `zip` + password: built-in zip `-P` flag; no additional suffix. Note: zip encryption is weaker than AES-256-CBC.
- **Excluded directories**: `archive_relative_excludes()` converts absolute `exclude_dirs` to archive-relative paths so they are honoured during compression, not just during provider sync
- **GNU tar required**: Alpine's default BusyBox tar lacks `--numeric-owner`; the Dockerfile installs the GNU `tar` package

#### RateLimitService (`services/rate_limit_service.rs`)

- Track failed auth attempts per IP
- Exponential backoff lockouts
- Automatic cleanup of old records

#### SchedulerService (`services/scheduler_service.rs`)

- Background tokio task, checks schedules every 60 seconds
- Timezone-aware cron parsing — timezone configured via `server.timezone` in `dynamight.toml` or the `TZ` environment variable
- Calculates next run times using 5-field cron expressions with full timezone support
- Skips execution if the same job is already running
- Updates `last_run_at` and `next_run_at` after each invocation
- Preserves "cancelled" status (does not overwrite with success/failure)

### Handler Layer

| Endpoint Group | Path | Purpose |
|----------------|------|---------|
| Auth | `/api/auth/*` | Login, logout, setup, password change |
| TOTP | `/api/auth/totp/*` | 2FA setup, validation, recovery |
| Jobs | `/api/jobs/*` | Job CRUD, run, cancel, clone |
| Schedules | `/api/jobs/:id/schedules/*` | Schedule management |
| Logs | `/api/runs/*` | Run history and logs |
| System | `/api/system/*` | Drives, mounts, browse, health |
| Settings | `/api/settings` | App configuration |
| Credentials | `/api/credentials/*` | Credential CRUD |
| Providers | `/api/providers/*` | Provider info, test connection |
| WebSocket | `/api/ws/*` | Real-time log streaming |

### Database Schema

**Main Database:**
```sql
users           -- User accounts (id, username, password_hash, totp_secret)
jobs            -- Backup jobs with destination_type, destination_config, sync_options
schedules       -- Cron schedules linked to jobs
credentials     -- Encrypted provider credentials
app_settings    -- Application configuration
sessions        -- JWT token tracking
```

**Logs Database:**
```sql
job_runs        -- Execution history (status, timestamps, stats)
log_entries     -- Individual log lines from sync output
```

---

## Frontend Architecture

### State Management

Uses Svelte 5 runes for local state and custom stores for global state.

#### Stores

- **authStore**: User session, login/logout
- **jobsStore**: Cached job list with CRUD
- **statusStore**: WebSocket for global job updates
- **themeStore**: Light/dark theme
- **viewPreferencesStore**: Grid/list view preference
- **preferencesStore**: User preferences

### Routing

Uses `svelte-spa-router` with hash-based routing:

- `/` - Dashboard
- `/jobs` - Job list
- `/jobs/new` - Create job
- `/jobs/:id` - Edit job
- `/history` - Run history

### Component Hierarchy

```
App.svelte
├── (unauthenticated) Login.svelte
│   └── TotpVerification.svelte (if 2FA enabled)
├── (first-time) Setup.svelte
│   └── TotpSetup.svelte (optional)
└── (authenticated) Layout.svelte
    ├── Navbar.svelte
    │   └── SettingsModal.svelte
    │       └── CredentialsManager.svelte
    ├── Sidebar.svelte
    └── Router → [Dashboard|Jobs|JobDetail|History]
        ├── Jobs.svelte
        │   ├── ColumnSelector.svelte    # Column visibility/order popover
        │   ├── JobListRow.svelte        # One per job; dynamic column rendering
        │   └── RunLogModal.svelte       # Live log viewer (lifted state from rows)
        └── JobDetail.svelte
            ├── ProviderSelector.svelte
            ├── [Provider]Destination.svelte  # Includes CredentialSelector + TestConnection
            ├── PathSelector.svelte           # Multi-source directory picker
            ├── CompressOptions.svelte        # Directory compression settings
            ├── SyncOptions.svelte
            ├── SchedulePicker.svelte
            └── RunLogModal.svelte            # Live log viewer (also used here)
```

---

## Provider System

### SyncProvider Trait

All providers implement:

```rust
pub trait SyncProvider: Send + Sync {
    fn provider_type(&self) -> &'static str;
    fn capabilities(&self) -> ProviderCapabilities;
    fn validate_config(&self, dest: &DestinationConfig, cred: Option<&CredentialData>) -> Result<()>;
    async fn sync(&self, ctx: Arc<SyncContext>) -> Result<SyncResult>;
    async fn test_connection(&self, dest: &DestinationConfig, cred: Option<&CredentialData>) -> Result<TestConnectionResult>;
}
```

### Provider Capabilities

| Capability | Local | S3 | GDrive | OneDrive | SFTP | WebDAV |
|------------|-------|-----|--------|----------|------|--------|
| Delete extraneous | Yes | Yes | Yes | Yes | Yes | Yes |
| Compression | Yes | No | No | No | Yes | No |
| Checksum | Yes | Yes | No | No | No | No |
| Bandwidth limit | Yes | No | No | No | Yes | No |
| Exclude patterns | Yes | Yes | Yes | Yes | Yes | Yes |
| Incremental | Yes | Yes | Yes | Yes | Yes | Yes |
| Dry run | Yes | Yes | Yes | Yes | Yes | Yes |
| Requires credentials | No | Yes | Yes | Yes | Yes | Yes |

### Destination Configuration

```rust
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
        drive_id: Option<String>,
    },
    S3 {
        bucket: String,
        prefix: String,
        region: String,
        endpoint: Option<String>,
        storage_class: Option<String>,
    },
    Sftp {
        host: String,
        port: u16,
        username: String,
        remote_path: String,
        key_based_auth: bool,
        host_key_fingerprint: Option<String>, // TOFU-based MITM protection ("SHA256:...")
    },
    WebDav {
        url: String,
        remote_path: String,
    },
}
```

---

## Data Flow

### Job Execution Flow

```
User clicks "Run" button
        |
        v
POST /api/jobs/:id/run
        |
        v
jobs::run_job handler
├── Creates job_run record (status: pending)
├── Spawns async task
│   ├── Updates status to "running"
│   ├── Loads job with destination config
│   ├── Decrypts credentials if needed
│   ├── [If compress_dirs enabled]
│   │   ├── For each source_dir:
│   │   │   ├── compress_service::compress_directory()
│   │   │   │   ├── Honours exclude_dirs (archive-relative paths)
│   │   │   │   └── Streams progress logs
│   │   │   └── cleanup_old_archives() (if max_archives_per_dir set)
│   │   └── staging_path becomes effective source for sync
│   ├── Creates provider via factory
│   ├── Calls provider.sync(context)
│   │   ├── Provider-specific sync logic
│   │   └── Streams logs -> broadcast channel
│   └── Updates job_run with results
└── Returns { runId } immediately
        |
        v
Frontend connects to WS /api/ws/logs/:runId
        |
        v
LogViewer displays streaming logs
```

### Credential Storage Flow

```
User adds credential (UI)
        |
        v
POST /api/credentials
        |
        v
CredentialService.create()
├── Validate credential data
├── Generate random nonce (12 bytes)
├── Derive key: SHA256(JWT_SECRET + salt)
├── Encrypt: AES-256-GCM(key, nonce, data)
├── Store: nonce || ciphertext
└── Return credential ID
```

### 2FA Authentication Flow

```
User enters username + password
        |
        v
POST /api/auth/login
        |
        v
If TOTP enabled:
├── Return { totp_required: true, totp_token }
│           |
│           v
│   User enters 6-digit code
│           |
│           v
│   POST /api/auth/totp/validate { totp_token, code }
│           |
│           v
│   Return { token } (JWT cookie set)
│
If TOTP not enabled:
└── Return { token } (JWT cookie set)
```

---

## Security Model

### Authentication

- **Password Storage**: Argon2id (memory-hard)
- **Session Tokens**: JWT with 24-hour expiry
- **Cookie Flags**: `HttpOnly`, `SameSite=Strict`, optionally `Secure`
- **2FA**: TOTP (RFC 6238) with SHA-1, 6-digit codes

### Rate Limiting

- Track failed attempts per IP address
- Default: 5 attempts per 60 seconds
- Lockout: 60 seconds initial, exponential backoff
- Maximum lockout: 1 hour
- Applied to: login, TOTP validation, recovery codes

### Credential Encryption

- **Algorithm**: AES-256-GCM
- **Key**: SHA-256(JWT_SECRET + "credential_encryption_salt")
- **Nonce**: 12-byte random per encryption
- **Storage**: Nonce prepended to ciphertext

### Container Security

- Runs as root (required for mount operations)
- `SYS_ADMIN` capability only (not full privileged)
- `no-new-privileges` security option
- Read-only source directory mounts recommended

### Input Validation

- Path traversal prevented via canonicalization
- SQL injection prevented via sqlx parameterized queries
- Mount operations validate UUID format
- URL validation for WebDAV

---

## Docker Deployment

### Build Stages

1. **frontend-builder** (node:20-alpine)
   - Installs npm dependencies
   - Runs `vite build` -> outputs to `dist/`

2. **backend-builder** (rust:1.83-alpine)
   - Builds release binary with musl (static linking)
   - Includes migrations

3. **runtime** (alpine:3.20)
   - Installs rsync, mount utilities, NTFS/exFAT support
   - Copies binary and static files
   - Exposes port 8080

### Docker Compose

```yaml
services:
  dynamight:
    build: .
    container_name: dynamight
    restart: unless-stopped
    cap_add:
      - SYS_ADMIN
    devices:
      - /dev:/dev
    ports:
      - "8080:8080"
    volumes:
      - dynamight-data:/app/data
      - dynamight-logs:/app/logs
      - /mnt:/mnt:rshared
      # Add source directories:
      # - /home:/source/home:ro
    environment:
      - TZ=${TZ:-UTC}
      - RUST_LOG=${RUST_LOG:-info,dynamight=debug}
      - JWT_SECRET=${JWT_SECRET:?JWT_SECRET is required}
      - ALLOWED_BROWSE_PATHS=${ALLOWED_BROWSE_PATHS:-/mnt,/home,/media}
    security_opt:
      - no-new-privileges:true
```

### Required Environment Variables

| Variable | Required | Description |
|----------|----------|-------------|
| `JWT_SECRET` | Yes | Secret for JWT signing and credential encryption |
| `TZ` | No | Timezone (default: UTC) |
| `RUST_LOG` | No | Log level (default: info,dynamight=debug) |
| `ALLOWED_BROWSE_PATHS` | No | Comma-separated paths for browsing |

---

## Native Linux Deployment

### Development

```bash
./scripts/dev.sh
```

Starts:
- Backend on http://localhost:8080
- Frontend dev server on http://localhost:5173 (hot-reload)

### Production Build

```bash
./scripts/build.sh
```

Creates `dist/dynamight-<timestamp>.tar.gz` containing:
- Compiled release binary
- Frontend static files
- Database migrations
- Installation scripts

### System Service Installation

```bash
# On target server
tar -xzf dynamight-*.tar.gz
cd dynamight-*
sudo ./scripts/install.sh
```

Installation layout:
- `/opt/dynamight/` - Binary and static files
- `/etc/dynamight/.env` - Configuration
- `/var/lib/dynamight/` - Database and data
- Logs via journald

Service management:
```bash
sudo systemctl enable dynamight    # Auto-start on boot
sudo systemctl start dynamight     # Start now
sudo systemctl status dynamight    # Check status
journalctl -u dynamight -f         # View logs
```

### Systemd Security

The service includes:
- Dedicated `dynamight` system user
- `NoNewPrivileges=true`
- `ProtectSystem=strict`
- `ProtectHome=read-only`
- `PrivateTmp=true`
- `CAP_SYS_ADMIN` capability for mount operations

---

## Configuration Reference

Configuration is primarily via `dynamight.toml` (copy from `dynamight.toml.example`). Any setting can be overridden by an environment variable using SCREAMING_SNAKE_CASE. Environment variables take highest priority.

### Configuration Search Paths

1. `DYNAMIGHT_CONFIG` environment variable (if set)
2. `./dynamight.toml` (current directory)
3. `/etc/dynamight/dynamight.toml` (system-wide)

### Required Settings

| TOML key | Env var | Description |
|----------|---------|-------------|
| `security.jwt_secret` | `JWT_SECRET` | Secret for JWT signing and credential encryption. Generate with `openssl rand -base64 32` |

### Database Settings

| TOML key | Env var | Default | Description |
|----------|---------|---------|-------------|
| `database.url` | `DATABASE_URL` | `sqlite:data/dynamight.db` | Main database location |
| — | `LOGS_DATABASE_URL` | (derived from DATABASE_URL) | Logs database location |

### Server Settings

| TOML key | Env var | Default | Description |
|----------|---------|---------|-------------|
| `server.host` | `HOST` | `0.0.0.0` | Network interface to bind |
| `server.port` | `PORT` | `8080` | Port to listen on |
| `server.timezone` | `TZ` | `UTC` | Timezone for scheduled jobs (e.g. `Europe/Berlin`) |
| — | `STATIC_FILES_DIR` | `static` | Frontend files directory |

### Logging

| Env var | Default | Description |
|---------|---------|-------------|
| `RUST_LOG` | `info,dynamight=debug` | Log level configuration |

### Security Settings

| TOML key | Env var | Default | Description |
|----------|---------|---------|-------------|
| `security.allowed_browse_paths` | `ALLOWED_BROWSE_PATHS` | `["/mnt","/home","/media"]` | Paths users can browse (comma-separated in env) |
| — | `CORS_ORIGINS` | (same-origin) | Allowed CORS origins |
| `security.secure_cookies` | `SECURE_COOKIES` | `true` | Require HTTPS for cookies |

### Rate Limiting

| Variable | Default | Description |
|----------|---------|-------------|
| `RATE_LIMIT_MAX_ATTEMPTS` | `5` | Max failed attempts before lockout |
| `RATE_LIMIT_WINDOW_SECS` | `60` | Time window for tracking attempts |
| `RATE_LIMIT_LOCKOUT_SECS` | `60` | Initial lockout duration |
| `RATE_LIMIT_MAX_LOCKOUT_SECS` | `3600` | Maximum lockout duration |

---

## File Reference

### Backend Files

| File | Purpose |
|------|---------|
| `main.rs` | Application entry, router setup |
| `config.rs` | Environment variable parsing |
| `middleware.rs` | Authentication middleware |
| `services/auth_service.rs` | Argon2 hashing, JWT |
| `services/totp_service.rs` | 2FA implementation |
| `services/backup_service.rs` | Job execution orchestration |
| `services/credential_service.rs` | AES-256-GCM encryption |
| `services/compress_service.rs` | Directory archiving and retention |
| `services/mount_service.rs` | USB device operations |
| `services/scheduler_service.rs` | Timezone-aware cron scheduler |
| `services/rate_limit_service.rs` | Auth rate limiting |
| `services/providers/mod.rs` | Provider trait and factory |
| `services/providers/rsync.rs` | Local rsync provider |
| `services/providers/s3.rs` | AWS S3 provider |
| `services/providers/googledrive.rs` | Google Drive provider |
| `services/providers/onedrive.rs` | OneDrive provider |
| `services/providers/sftp.rs` | SFTP provider |
| `services/providers/webdav.rs` | WebDAV provider |
| `handlers/auth.rs` | Auth endpoints |
| `handlers/totp.rs` | 2FA endpoints |
| `handlers/jobs.rs` | Job CRUD endpoints |
| `handlers/credentials.rs` | Credential endpoints |
| `handlers/providers.rs` | Provider info endpoints |

### Frontend Files

| File | Purpose |
|------|---------|
| `lib/types.ts` | TypeScript interfaces |
| `lib/api.ts` | API client with all endpoints |
| `lib/stores/tablePreferences.ts` | Job list column visibility, order, and widths |
| `components/jobs/ProviderSelector.svelte` | Provider selection tiles |
| `components/jobs/CredentialSelector.svelte` | Credential picker (embedded in provider forms) |
| `components/jobs/TestConnection.svelte` | Connection tester (embedded in provider forms) |
| `components/jobs/CompressOptions.svelte` | Directory compression settings UI |
| `components/jobs/ColumnSelector.svelte` | Column visibility/order popover |
| `components/jobs/JobListRow.svelte` | Dynamic table row with column-aware rendering |
| `components/jobs/SyncOptions.svelte` | Sync configuration (exclude dirs grouped by source) |
| `components/jobs/providers/*.svelte` | Provider-specific forms (include credential selector) |
| `components/settings/CredentialsManager.svelte` | Credential management |
| `components/logs/RunLogModal.svelte` | Live log viewer (used in Jobs and JobDetail) |
| `components/TotpSetup.svelte` | 2FA setup wizard |
| `components/TotpVerification.svelte` | 2FA login prompt |
| `routes/Jobs.svelte` | Job list with column management, drag/resize |
| `routes/JobDetail.svelte` | Job create/edit with provider selection |

---

## Directory Compression

When `sync_options.compress_dirs.enabled` is true, each source directory is archived before being transferred to the destination.

### Workflow

1. For each source directory, `CompressService.compress_directory()` creates an archive in `staging_path/`
2. Excluded directories (`exclude_dirs`) are honoured during archiving — they are converted to archive-relative paths so they are excluded from the tar/zip contents
3. If `max_archives_per_dir` is set, `cleanup_old_archives()` removes the oldest archives after each run
4. The provider then syncs the staging directory instead of the original source directories
5. Space pre-flight checks are skipped when compression is active (unpredictable ratio)

### Archive Naming

```
[<YYYY-MM-DDTHH-MM-SS>_][<custom_name>_]<sanitised_dir_name>_<job_id>.<ext>
```

Examples:
- `2026-03-01T14-30-00_photos_42.tar.gz` (timestamp + default name)
- `backup_home_christian_42.zip` (custom name "backup", no timestamp)
- `documents_42.tar.gz.enc` (tar.gz encrypted)

The job ID embedded in every filename ensures multiple jobs can share the same staging directory without collisions.

### Formats and Encryption

| Format | store_only | Password | Extension |
|--------|-----------|----------|-----------|
| tar.gz | No | No | `.tar.gz` |
| tar.gz | No | Yes | `.tar.gz.enc` (openssl AES-256-CBC, password via stdin) |
| tar.gz | Yes | No | `.tar` |
| tar.gz | Yes | Yes | `.tar.enc` |
| zip | — | No | `.zip` |
| zip | — | Yes | `.zip` (built-in zip `-P` flag) |

Note: tar encryption uses `openssl enc -aes-256-cbc -pbkdf2`; the password is passed via stdin (not command-line) to avoid exposure in the process list. Zip uses built-in encryption which is weaker than AES-256-CBC.

---

## Job List Column Preferences

The job list table supports persistent per-browser column configuration.

### Columns

| Column | Fixed | Default visible | Description |
|--------|-------|----------------|-------------|
| `job` | Yes (first) | Yes | Job name + status indicator |
| `status` | No | Yes | Enabled/disabled badge |
| `sources` | No | Yes | Source directory count |
| `destination` | No | Yes | Destination summary |
| `last_run` | No | Yes | Relative time of last run |
| `schedule` | No | No | Human-readable cron + next run |
| `options` | No | Yes | Active option badges |
| `actions` | Yes (last) | Yes | Run/Stop/Logs buttons |

### Behaviour

- **Visibility**: Toggle columns via the Columns popover (fixed columns cannot be hidden)
- **Reordering**: Drag column headers to reorder; fixed columns (`job`, `actions`) always stay at the ends
- **Resizing**: Drag column borders to set widths (minimum 60px); excess space is distributed proportionally
- **Persistence**: All preferences are stored in `localStorage` under key `dynamight-job-table-prefs`
- **Schedules**: Displayed as human-readable text (e.g. "Daily at 14:30", "Weekly on Monday at 09:00"); orphaned or disabled schedules are styled accordingly

### Exclude Directories UI

In `SyncOptions.svelte`, excluded directories are **grouped by their parent source directory** in a card-per-source layout. Paths that no longer match any current source directory are highlighted as orphaned entries (amber warning) and can be cleaned up.

---

## Key Design Decisions

1. **SQLite over PostgreSQL**: Simplicity for single-user/self-hosted deployments
2. **Separate logs database**: Better performance and easier cleanup
3. **Argon2 over bcrypt**: More resistant to modern attack vectors
4. **AES-256-GCM for credentials**: Authenticated encryption
5. **Trait-based providers**: Extensible architecture for adding new providers
6. **Provider factory pattern**: Clean instantiation and configuration
7. **WebSocket for logs**: Real-time feedback without polling
8. **UUID-based mounts**: More reliable than device paths
9. **Svelte 5 runes**: Modern reactive primitives
10. **Alpine base image**: Minimal attack surface

---

## Extension Points

To extend this application:

- **Add new provider**: Implement `SyncProvider` trait in `services/providers/`
- **Add OAuth flow**: Implement authorization endpoints for OAuth providers
- **Add email notifications**: Create notification service, call from scheduler
- **Add backup verification**: Add post-sync checksum verification
- **Add retention policies**: Implement cleanup based on age/count
- **Add multi-user**: Extend user model with roles, add authorization
