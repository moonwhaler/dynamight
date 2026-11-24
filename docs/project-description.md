# Dynamight - Project Architecture

> A self-hosted backup manager: a Rust (Axum) server with a Svelte web UI that backs up folders to local and USB drives (rsync), S3, Google Drive, OneDrive, SFTP and WebDAV.

This document describes how Dynamight is built, as of version 2.1. For installation and day-to-day use see the [README](../README.md); for every endpoint see the [API reference](API.md).

## Table of Contents

1. [Overview](#overview)
2. [Technology Stack](#technology-stack)
3. [Architecture](#architecture)
4. [Directory Structure](#directory-structure)
5. [Backend](#backend)
6. [Running a Backup](#running-a-backup)
7. [Provider System](#provider-system)
8. [Frontend](#frontend)
9. [Security Model](#security-model)
10. [Deployment](#deployment)
11. [Configuration Reference](#configuration-reference)
12. [Directory Compression](#directory-compression)
13. [Testing](#testing)
14. [Key Design Decisions](#key-design-decisions)
15. [Extension Points](#extension-points)

---

## Overview

Dynamight runs on the machine that holds the data (typically a home server or NAS) and is operated by one administrator through the browser. A *job* describes what to back up (one or more source folders), where to (one destination) and when (any number of cron schedules). Runs happen on schedule or on demand, one at a time, and every run keeps its statistics and full log.

### Core Capabilities

- **Destinations**: local folders and USB drives (rsync, with automatic mounting by UUID), S3 and S3-compatible storage, Google Drive, OneDrive, SFTP and WebDAV
- **Scheduling**: timezone-aware cron schedules; missed schedules run once after downtime
- **Run queue**: one backup at a time; further runs wait in line
- **Monitoring**: per-job health (failed, overdue, never backed up), activity history, live logs over WebSocket, notifications via ntfy and webhooks
- **Directory compression**: optional tar.gz or zip archives of each source folder before the transfer, optionally encrypted
- **Retention**: runs per job, and days to keep full logs
- **File manager**: browse, search, preview and play, download, upload (resumable), rename, copy and move (background tasks), trash with restore, within allowed paths
- **Configuration backup**: encrypted export and import of jobs, credentials, schedules and settings
- **Security**: Argon2 passwords, TOTP two-factor authentication with recovery codes, encrypted credential storage, rate limiting

### Supported Providers

| Provider | Description | Authentication |
|----------|-------------|----------------|
| Local/USB | rsync to a local folder or a mounted drive | None (filesystem) |
| S3 | AWS S3 and S3-compatible services (MinIO, Backblaze B2, SeaweedFS, ...) | Access key + secret |
| Google Drive | My Drive folders and shared drives | OAuth2 (own app registration, refresh token) |
| OneDrive | Personal and business OneDrive | OAuth2 (own app registration, refresh token) |
| SFTP | SSH servers | Password or SSH key, host key pinned on first use |
| WebDAV | Nextcloud, ownCloud, generic WebDAV | Username + password |

---

## Technology Stack

| Layer | Technology | Purpose |
|-------|------------|---------|
| Web framework | Axum 0.7, tower-http | HTTP, WebSockets, static files, compression, security headers |
| Runtime | Tokio | Async runtime, process spawning, timers |
| Database | SQLite via sqlx 0.8 (WAL mode) | Main and logs databases |
| Authentication | argon2, jsonwebtoken | Password hashing, session tokens |
| 2FA | totp-rs, qrcode | TOTP codes and QR codes |
| Credential encryption | aes-gcm, argon2 | AES-256-GCM with Argon2id key derivation |
| Config backup encryption | aes + cbc, pbkdf2 | OpenSSL-compatible AES-256-CBC |
| S3 | aws-sdk-s3 | S3 and compatible storage |
| SFTP | russh, russh-sftp | SSH/SFTP client |
| HTTP client | reqwest (rustls) | WebDAV, Google Drive, OneDrive, notifications |
| Scheduling | cron, chrono-tz | Cron parsing in the configured timezone |
| External tools | rsync, mount/umount, findmnt, lsblk, tar, zip, openssl, pigz (optional) | Local backups, drives, archives |
| Frontend | Svelte 5 (runes), TypeScript | Single-page application |
| Build | Vite 5 | Bundling (hashed asset names) |
| Styling | Tailwind CSS 3 with CSS variables | Theming (3 base tones × 6 accents, light/dark) |
| Routing | svelte-spa-router | Hash-based routes |
| Translations | Paraglide JS 2 | English and German |
| Tests | cargo test, axum-test, Vitest | Unit and API tests |
| Container | Alpine 3.22 | Runtime image |

---

## Architecture

```
┌──────────────────────┐   REST + WebSocket   ┌─────────────────────────────────────────────┐
│ Browser (Svelte SPA) │◀────────────────────▶│ Axum server                                 │
└──────────────────────┘                      │  /api/*   handlers (auth middleware)        │
                                              │  /api/ws/* live logs, run events            │
                                              │  /*        static files (cached, compressed)│
                                              └───────┬─────────────────────────────────────┘
                                                      │
       ┌──────────────────────┬───────────────────────┼─────────────────────┬────────────────┐
       ▼                      ▼                       ▼                     ▼                ▼
┌──────────────┐   ┌────────────────────┐   ┌──────────────────┐  ┌─────────────────┐ ┌──────────────┐
│ Scheduler    │──▶│ BackupService      │──▶│ Provider layer   │  │ Notifier        │ │ Auth, TOTP,  │
│ (each minute)│   │  run queue (1 slot)│   │ rsync, S3, Drive,│  │ ntfy / webhook  │ │ credentials, │
└──────────────┘   │  cancel tokens     │   │ OneDrive, SFTP,  │  └─────────────────┘ │ rate limits  │
                   │  log writer, events│   │ WebDAV           │                      └──────────────┘
                   │  log retention     │   └──────────────────┘
                   └─────────┬──────────┘
                             ▼
              ┌──────────────────────────────┐   ┌──────────────────────────┐
              │ dynamight.db (main)          │   │ logs.db                  │
              │ users, jobs, schedules,      │   │ log_entries              │
              │ job_runs, credentials, ...   │   │ (one row per log line)   │
              └──────────────────────────────┘   └──────────────────────────┘
```

Everything runs in one process. Run log lines go to their own database, so a verbose backup writing thousands of lines does not slow down the main database.

---

## Directory Structure

```
dynamight-web/
├── Cargo.toml                  # Workspace
├── Dockerfile                  # node → rust (musl) → alpine
├── docker-compose.yml
├── dynamight.toml.example      # Configuration template
├── migrations/                 # Base SQL schema (later changes live in db/mod.rs)
├── docs/                       # API reference, this document, screenshots
├── scripts/
│   ├── dev.sh                  # Backend + Vite dev server
│   ├── build.sh                # Release package (and Docker image)
│   ├── install.sh              # Install / uninstall as a systemd service
│   ├── update.sh               # Update with backup and automatic rollback
│   ├── version.sh              # Set the version everywhere
│   └── dynamight.service       # systemd unit
│
├── backend/
│   ├── build.rs                # Embeds git hash and build date
│   ├── tests/                  # API tests (axum-test) and live provider tests
│   └── src/
│       ├── main.rs             # Startup, background tasks, outer layers, shutdown
│       ├── lib.rs              # AppState and the API router (shared with tests)
│       ├── config.rs           # dynamight.toml + environment overrides
│       ├── errors.rs           # ApiError and error codes
│       ├── extractors.rs       # AuthClaims, token from cookie/header
│       ├── middleware.rs       # require_auth, session checks
│       ├── db/mod.rs           # Migrations for both databases
│       ├── models/             # job, schedule, log_entry (runs, log lines), destination, credential, user
│       ├── handlers/           # auth, totp, jobs, schedules, logs (runs), system, settings,
│       │                       # notifications, config_backup, credentials, providers, websocket,
│       │                       # files (places, raw, trash, sizes), file_actions (tasks, rename,
│       │                       # archive), file_preview (text, thumbnails), uploads
│       └── services/
│           ├── backup_service.rs        # Run queue, execution, finalizing, retention
│           ├── scheduler_service.rs     # Cron schedules
│           ├── log_writer.rs            # Batched log line storage
│           ├── notification_service.rs  # ntfy / webhook notifications
│           ├── compress_service.rs      # Archives before the transfer
│           ├── mount_service.rs         # Drives, mounts, directory listing and sizes, free space
│           ├── path_guard.rs            # Allowed-path checks, no-replace rename, internal names
│           ├── trash.rs                 # Trash folders per drive: move, list, restore, purge
│           ├── file_ops.rs              # Copy/move tasks with conflicts and progress
│           ├── credential_service.rs    # Credential encryption
│           ├── oauth_service.rs         # Access token refresh (Drive, OneDrive)
│           ├── config_backup_service.rs # Encrypted export / import
│           ├── auth_service.rs          # Passwords, JWT
│           ├── totp_service.rs          # 2FA
│           ├── rate_limit_service.rs    # Login rate limiting
│           └── providers/
│               ├── mod.rs               # SyncProvider trait, SyncContext, factory
│               ├── common.rs            # Shared helpers for remote providers
│               ├── rsync.rs, s3.rs, googledrive.rs, onedrive.rs, sftp.rs, webdav.rs
│
└── frontend/
    ├── messages/               # en.json, de.json (Paraglide)
    └── src/
        ├── App.svelte          # Auth gate and routes
        ├── main.ts             # Fonts, leave guard, mount
        ├── app.css             # Theme variables and component classes
        ├── routes/             # Overview, Jobs, JobView, JobNew, Activity, FileBrowser, Settings,
        │                       # Login, Setup, Redirect
        ├── components/
        │   ├── shell/          # AppShell, Sidebar (rail on tablets), BottomTabs (phones)
        │   ├── jobs/           # JobTable, JobRow, JobTile, editor/ (JobEditor, JobWizard, ...),
        │   │                   # providers/ (destination forms), SchedulePicker, SyncOptions, ...
        │   ├── logs/           # RunLogModal, RunLogHost
        │   ├── settings/       # One component per settings section
        │   ├── filebrowser/    # PlacesPanel, PathBar, FileList (windowed), FilePreview, MusicPlayer,
        │   │                   # TrashView, FileTasks (transfers), conflict and delete dialogs
        │   └── ui/             # Dialog, Menu, Toggle, Tabs, StatusChip, RunStrip, Toast, ...
        └── lib/
            ├── api.ts          # API client
            ├── types.ts        # Shared types
            ├── jobDraft.svelte.ts  # Editable job state (editor and wizard)
            ├── jobActions.ts   # Run, stop, enable, clone, delete
            ├── leaveGuard.ts   # Unsaved-changes guard
            ├── stores/         # auth, jobs (live), logs (status socket), runLog, preferences, theme, ...
            ├── utils/          # jobStatus, backupHealth, cron, format, path, ...
            └── i18n/           # Schedule text, job summary sentence, relative time
```

---

## Backend

### Startup (`main.rs`)

1. Answer `--version` / `--help` and exit, if given
2. Set up logging (`RUST_LOG` wins over `[logging] level`)
3. Load the configuration
4. Open both SQLite databases (WAL, `synchronous=NORMAL`, 30 s busy timeout) and run migrations
5. Read the stored settings (run retention, log retention, search timeout, directory sizes)
6. Create the services; upgrade credentials still in the legacy encryption format
7. Mark runs left `pending` or `running` by a crash as failed ("Interrupted by server restart")
8. Start background tasks:
   - hourly cleanup of expired 2FA sessions and revoked tokens; log retention every 6 hours (also right at startup)
   - the scheduler
9. Build the app: the API under `/api`, static files, response compression, tracing, CORS, security headers
10. Serve until `SIGTERM`/`SIGINT`, then shut down (see [Shutdown](#shutdown))

### Services

#### BackupService (`services/backup_service.rs`)

The center of the application. It owns:

- **The run queue**: a semaphore with one slot (`MAX_PARALLEL_RUNS = 1`), FIFO. Each job has at most one running and one waiting run.
- **Cancellation**: one `CancellationToken` per active run. Providers check it while working; waits (`select!`) end as soon as it fires.
- **Run guards**: a `RunGuard` holds a run's place (its waiting/running flag, its token, its slot) and releases everything when dropped, on every path including errors and panics.
- **Log writing**: lines go to the WebSocket broadcast and to the `LogWriter`, which stores them in batches.
- **Run events**: a small broadcast channel (`Queued`, `Started`, `Completed`) for the status WebSocket, separate from the log lines.
- **Finalizing**: final status, statistics, failure summary, last good copy time, storage info, notifications, run retention.
- **Overdue checks**: jobs without a recent good copy are marked overdue and notified once.
- **Log retention**: see [Retention](#retention).

#### SchedulerService (`services/scheduler_service.rs`)

- Ticks one second after every full minute (cron has minute resolution); the first check runs at startup.
- Due schedules (`next_run_at <= now`) are advanced first, then the job is handed to `start_run`. Schedules of disabled jobs are advanced without running.
- At startup, `next_run_at` is recalculated (the timezone may have changed), except for schedules that came due while the server was down: those keep their past time and run once.
- Runs the overdue check every 10 minutes.
- Times use the configured timezone (`server.timezone` or `TZ`).

#### LogWriter (`services/log_writer.rs`)

A single task that stores log lines in batches (up to 500 lines or 100 ms) inside one transaction. Writers wait only when its queue (8192 lines) is full. `flush()` waits until everything queued so far is stored; `finalize_run` uses it so the "Run finished" line and the stored log are complete when clients reload.

#### Notifier (`services/notification_service.rs`)

Sends ntfy and webhook notifications for failed, overdue, recovered and (optionally) every successful run. Settings are one JSON value in `app_settings`, so they travel with the configuration backup. Secrets are never sent back to the browser.

#### CompressService (`services/compress_service.rs`)

Archives source folders before the transfer. See [Directory Compression](#directory-compression).

#### MountService (`services/mount_service.rs`)

- Lists USB drives (`lsblk -J`) and mounts (`findmnt -J`)
- Mounts by UUID and unmounts (UI actions)
- Lists, searches and sizes directories for the file manager (search stops at its deadline; sizes stay on one filesystem and stop when the browser leaves)
- `fs_space(path)`: free and total bytes via `statvfs`, like `df`

#### File manager (`path_guard.rs`, `trash.rs`, `file_ops.rs`, `handlers/files*.rs`, `handlers/uploads.rs`)

- Every client path is resolved and checked against `allowed_browse_paths` before and after following links; an entry that is a link is acted on itself, never its target.
- **Trash**: an item moves (rename) to `.dynamight-trash/<id>/` at the nearest allowed root, mount point or filesystem boundary, with a sidecar JSON holding its path relative to that folder (a remounted drive still restores right). Trash folders are recorded in `trash_roots`; the hourly cleanup deletes items older than `trash_retention_days`.
- **Copy and move** run as tasks, one at a time, on a blocking thread. Files are written as `.<name>.dynamight-part` and renamed without replacing (`renameat2(RENAME_NOREPLACE)`). A taken name pauses the task until the user answers (replace moves the old item to the trash). Moves to another drive copy, then delete each file. Progress goes out on its own broadcast channel and `/api/ws/status`.
- **Uploads**: chunks are appended to `.<name>.dynamight-upload-<id>`; its length is the offset, so an upload resumes after any interruption. The `uploads` table remembers them across restarts; stale ones are removed after 24 hours.
- **Downloads** use Range requests (`ServeFile`); inline previews carry a sandbox CSP. Several items download as a streamed `tar`. Thumbnails (images with the `image` crate, videos with `ffmpeg` when present) are cached next to the database.
- Backups and archives exclude the trash and the part files.

#### CredentialService and OAuth (`credential_service.rs`, `oauth_service.rs`)

- Encrypts credentials with AES-256-GCM; the key is derived from `jwt_secret` with Argon2id and a random salt per credential (format `DM1\0 + salt + nonce + ciphertext`). Key derivation runs on a blocking thread.
- Older credentials (SHA-256 derived key) are upgraded at startup.
- `get_fresh` renews an expired OAuth access token before a run; `OAuthTokenSource` renews it during long runs.

#### ConfigBackupService (`config_backup_service.rs`)

Exports jobs, schedules, credentials and settings as YAML, encrypted with AES-256-CBC and PBKDF2 (600,000 iterations), compatible with the `openssl enc` command. Import supports *merge* and *replace all*, with a preview.

#### AuthService, TOTP, RateLimitService

- Argon2 password hashing; JWT session tokens (24 hours, with a unique `jti`)
- TOTP secrets, QR codes, recovery codes; a used time step cannot be replayed
- Failed logins tracked per client IP with exponential lockouts

### Handlers

| Group | Paths | Purpose |
|-------|-------|---------|
| Auth | `/api/auth/*` | Setup, login, logout, password change, current user |
| 2FA | `/api/auth/totp/*` | Setup, enable, disable, validate, recovery |
| Jobs | `/api/jobs/*` | CRUD, run, cancel, clone, space check |
| Schedules | `/api/jobs/:id/schedules`, `/api/schedules/*` | Schedule CRUD, cron validation |
| Runs | `/api/runs/*`, `/api/jobs/:id/runs` | Run history, run details, logs (filter, search), deletion |
| System | `/api/system/*` | Health, version, drives, mounts, browse, search, download, delete |
| Files | `/api/files/*` | Places, raw files (Range), folder sizes (NDJSON), trash, copy/move tasks, rename, archive, text preview, thumbnails, uploads |
| Settings | `/api/settings*` | App settings, notification settings |
| Config | `/api/config/*` | Configuration export, import, import preview |
| Credentials | `/api/credentials/*` | Credential CRUD and usage |
| Providers | `/api/providers/*` | Provider list, capabilities, connection test |
| WebSocket | `/api/ws/logs/:runId`, `/api/ws/status` | Live log lines, run events, file task progress |

Handlers report database errors as errors (HTTP 500), never as empty data.

### Database

**Main database (`dynamight.db`)**

| Table | Contents |
|-------|----------|
| `users` | The admin account, password hash, 2FA secret and state, `password_changed_at` |
| `jobs` | Job settings (`destination_type`, `destination_config` and `sync_options` as JSON), `last_success_at`, `alert_state`, destination free space |
| `schedules` | Cron expression, enabled, `last_run_at`, `next_run_at` |
| `job_runs` | Status, times, files and bytes transferred, error count, `summary` (failure reason), `log_pruned` |
| `credentials` | Encrypted provider credentials |
| `app_settings` | Key/value settings (retention, notifications, file browser) |
| `trash_roots` | Trash folders the file manager has used |
| `uploads` | Uploads in progress (target, size, fingerprint, conflict choice) |
| `pending_totp_sessions`, `recovery_codes`, `revoked_tokens` | 2FA login, recovery codes, logged-out tokens |

**Logs database (`logs.db`)**

| Table | Contents |
|-------|----------|
| `log_entries` | One row per log line: run ID, time, level, message, source |

The base schema is in `migrations/`; later columns and tables are added by `db/mod.rs` at startup (idempotent).

### Retention

- **Runs**: `max_runs_per_job` (default 5, or unlimited). After each run, older runs of that job and their log lines are deleted.
- **Logs**: `log_retention_days` (default 90, or forever). Every 6 hours, the log lines of runs that finished longer ago are deleted and the run is marked `log_pruned`; each job's newest finished run and newest failed run keep their log. When at least a quarter of `logs.db` is then free pages, it is compacted (`VACUUM`), but only while no backup runs (it holds the run slot) and with enough free disk space.

### Shutdown

On `SIGTERM`/`SIGINT` no new runs start; active and waiting runs are stored as cancelled ("server shutting down") and their tokens fire. Runs get 20 seconds to stop (rsync is killed, drives are unmounted), the log writer is flushed, then open HTTP connections get 5 more seconds. systemd's `TimeoutStopSec` and the compose file's `stop_grace_period` are 30 seconds.

---

## Running a Backup

```
POST /api/jobs/:id/run            Scheduler (due schedule)
          │                                │
          └──────────────┬─────────────────┘
                         ▼
          BackupService::start_run(job, schedule_id)
          ├── reserve the job (refused: job already active; a schedule may queue one follow-up)
          ├── INSERT job_runs (status = pending), create the cancel token
          ├── slot free?  no → log "Waiting for another backup to finish", event job_queued
          └── spawn the run task, return { runId, queued }
                         │
                         ▼  (waits for the slot, or for cancellation)
          status = running, event job_started, reload the job (edits made while waiting apply)
                         │
                         ▼
          do_execute
          ├── load and refresh credentials
          ├── validate the destination
          ├── [archives] compress each source folder into the staging folder
          ├── last completed run's transfer size (quick space check)
          └── provider.sync(SyncContext)  ── log lines ─▶ WebSocket + LogWriter
                         │
                         ▼
          finalize_run
          ├── final status (cancelled stays cancelled), statistics
          ├── failed: store the last error line as summary
          ├── completed (not dry run): last_success_at
          ├── destination free space
          ├── "Run finished: <status>" (after the log is flushed), event job_completed
          ├── notification (failed / recovered / success)
          └── run retention
```

The browser follows along through two WebSockets: `/api/ws/status` (run events, so the job list refreshes, and file manager task progress) and `/api/ws/logs/:runId` (log lines of the open run log). Both authenticate with the session cookie.

---

## Provider System

### SyncProvider Trait

```rust
#[async_trait]
pub trait SyncProvider: Send + Sync {
    fn provider_type(&self) -> &'static str;
    fn capabilities(&self) -> ProviderCapabilities;
    fn validate_config(&self, destination: &DestinationConfig, credential: Option<&CredentialData>) -> Result<(), ProviderError>;
    async fn sync(&self, ctx: Arc<SyncContext>) -> Result<SyncResult, ProviderError>;
    async fn test_connection(&self, destination: &DestinationConfig, credential: Option<&CredentialData>) -> Result<TestConnectionResult, ProviderError>;
    async fn get_storage_info(&self, destination: &DestinationConfig, credential: Option<&CredentialData>) -> Result<StorageInfo, ProviderError>;
}
```

`SyncContext` carries the run ID, the source folders, the destination, the sync options, the credential, the log channels, the run's cancellation token (and a `check_cancelled()` shortcut), the OAuth token source and the last run's transfer size.

### Capabilities

| Capability | Local | S3 | Google Drive | OneDrive | SFTP | WebDAV |
|------------|:-----:|:--:|:------------:|:--------:|:----:|:------:|
| Delete extraneous files | Yes | Yes | Yes (trash) | Yes (recycle bin) | Yes | Yes |
| Compression during transfer | Yes | No | No | No | No | No |
| Checksum comparison | Yes | No | No | No | No | No |
| Bandwidth limit | Yes | No | No | No | No | No |
| Exclude patterns | Yes | Yes | Yes | Yes | Yes | Yes |
| Incremental | Yes | Yes | Yes | Yes | Yes | Yes |
| Dry run | Yes | Yes | Yes | Yes | Yes | Yes |
| Requires credentials | No | Yes | Yes | Yes | Yes | Yes |

### Destination Configuration

```rust
pub enum DestinationConfig {
    Local { mount_point: String, backup_subdir: String, usb_uuid: Option<String>, auto_mount: bool, auto_unmount: bool },
    GoogleDrive { folder_id: String, shared_drive_id: Option<String> },
    OneDrive { folder_path: String, drive_id: Option<String> },
    S3 { bucket: String, prefix: String, region: String, endpoint: Option<String>, storage_class: Option<String> },
    Sftp { host: String, port: u16, username: String, remote_path: String, key_based_auth: bool, host_key_fingerprint: Option<String> },
    WebDav { url: String, remote_path: String },
}
```

### Local and USB (`rsync.rs`)

- **Mounting**: with a USB drive and auto-mount, `findmnt` decides what is mounted at the mount point (not `mount`'s translated messages). An empty mount point is mounted with `mount -U`; a different device there is refused; the expected drive already there is used and left mounted afterwards. Before copying, the drive is verified again, so rsync never writes into the empty mount-point directory.
- **Space check** (mode `warn`, `fail` or `none`): first the quick check, which compares free space with the last completed run's transfer size plus 5 %. Only when that does not clearly fit, or there is no estimate, one rsync dry run (`--out-format=%l --stats`) measures the transfer and the source size. The **Check space** button always measures; it is refused while the job runs.
- **rsync**: one process per source folder in its own process group, with arguments for the filesystem (`-a`, or `-rltD` on exFAT/NTFS/FAT), `--delete`, `--checksum`, `-z`, `--ignore-times`, `--bwlimit`, excludes. Numbers are printed in the C locale so `--stats` can be parsed. stdout and stderr are read concurrently; progress redraws (`\r`) are reduced to their final state. Exit code 24 (files vanished) is a warning.
- **Cancel**: the run's token kills the process group at once.
- **Unmount**: `umount` (it flushes the drive), retried once after 2 s if busy.

### Remote Providers (`common.rs`)

- **Local tree**: `walk_source` walks a source folder on a blocking thread without following symlinks. `ExcludeMatcher` compiles the exclude patterns, excluded folders and (in compress mode) the include-only list once per run.
- **Change detection**: `needs_upload` uploads a file when the destination has none, a different size, or an older modification/upload time (the rule `aws s3 sync` uses).
- **Listing**: `walk_remote` lists the destination tree with up to 4 folders in flight (SFTP, WebDAV, Google Drive, OneDrive).
- **Uploading**: `upload_parallel` uploads up to 4 files at a time (3 for Google Drive) and stops at once on cancel. Files are streamed; S3 uses multipart upload above 64 MiB, Google Drive and OneDrive use resumable chunked uploads.
- **Retries**: `send_with_retry` retries throttled (429) and temporary server errors, honoring `Retry-After`. `send_create_with_retry` avoids duplicates when a create request timed out but succeeded.
- **Deletes**: with *delete extraneous*, remote files missing locally are deleted (Drive: trash, OneDrive: recycle bin); excluded paths are never deleted.

---

## Frontend

### Shell and Routes

`App.svelte` shows Setup (first start), Login (with the 2FA step) or the app shell. `AppShell` has a sidebar on desktops (an icon rail on tablets) and a bottom tab bar on phones.

| Route | Page |
|-------|------|
| `/` | Overview: jobs needing attention, all jobs with recent runs, upcoming runs, recent activity |
| `/jobs` | Job list (table or cards), filters by state, sorting |
| `/jobs/new` | Guided setup (wizard) or the full form, depending on the preference; `?clone=` copies a job |
| `/jobs/:id`, `/jobs/:id/runs`, `/jobs/:id/settings` | Job page with Overview, Runs and Settings (editor) tabs |
| `/activity` | All runs, grouped by day, with filters |
| `/files` | File manager (`?path=` the open folder, `?view=trash` the trash) |
| `/settings`, `/settings/:section` | Settings: general, appearance, notifications, credentials, account, security, data, about |
| `/history`, `/about` | Redirects to `/activity` and `/settings/about` |

### State and Data

- **`jobsStore`** (`stores/jobs.ts`): jobs and the recent runs. While a page shows live data it refreshes on every status event (debounced) and every 30 s as a fallback, and when the tab becomes visible again.
- **`statusStore`** (`stores/logs.ts`): the `/api/ws/status` connection, reconnecting after drops.
- **`runLog`**: opens the run log dialog from anywhere (`RunLogHost` is mounted in the shell).
- **Preferences, theme, language, view and table preferences**: in `localStorage`.
- **`jobStatus.ts`**: one state per job (running, queued, failed, overdue, never, ok, disabled) for chips, filters and the overview.
- **`jobDraft.svelte.ts`**: the editable job shared by the editor and the wizard: change tracking, validation per section, request building, schedule changes.
- **`leaveGuard.ts`**: asks before leaving a page with unsaved changes (in-app navigation and reloads).

### UI Building Blocks

`components/ui/` holds the shared parts: `Dialog` (native `<dialog>`, focus trap, Escape), `Menu` (keyboard navigation), `Toggle`, `Tabs`, `PageHeader`, `StatusChip`, `RunStrip`, `Toast` (shown above open dialogs), `ConfirmDialog` (optionally with typed confirmation), `EmptyState`, `PathText`.

Styling uses Tailwind with CSS variables for the gray and primary scales, so the three base tones and six accents switch without rebuilding. Fonts (Archivo, JetBrains Mono) are bundled; nothing loads from other servers.

### Translations

Texts are in `frontend/messages/en.json` and `de.json` and compiled by Paraglide into `src/lib/paraglide/`. Unit tests use the mock in `src/__mocks__/paraglide-messages.ts`, which needs an entry for every key.

---

## Security Model

### Sessions

- Login sets an `HttpOnly`, `SameSite=Strict` cookie (`Secure` unless `secure_cookies = false`) with a JWT valid for 24 hours.
- Every request checks the token, whether it was revoked by logout (`revoked_tokens`), and whether it was issued before the last password change.
- WebSockets authenticate with the same cookie during the handshake.
- With 2FA, login returns a short-lived `pending_session_id`; the session cookie is set only after a valid TOTP or recovery code.

### Rate Limiting

Failed logins, TOTP and recovery attempts are counted per client IP (default 5 per 60 s), with lockouts starting at 60 s and doubling up to 1 hour. `X-Forwarded-For` is only trusted from `trusted_proxies`.

### Stored Secrets

- Passwords: Argon2
- Provider credentials: AES-256-GCM, key from `jwt_secret` via Argon2id with a per-credential salt. Changing `jwt_secret` makes stored credentials unreadable.
- Configuration backups: AES-256-CBC with PBKDF2, protected by a password chosen at export
- Archive passwords: stored in the job's options; passed to openssl through its environment, never on a command line

### HTTP Hardening

Content-Security-Policy (`default-src 'self'`, no external scripts or fonts, `frame-ancestors 'none'`), `X-Content-Type-Options`, `X-Frame-Options`, `Referrer-Policy`, `Permissions-Policy`, HSTS with secure cookies, a request body limit, and same-origin CORS unless `cors_origins` is set. API responses are `no-store`.

### File System

- Every file manager action is limited to `allowed_browse_paths`, checked before and after resolving symlinks; paths with `..` are refused.
- Deleting moves items to the trash; deleting permanently (and emptying the trash) needs a recent password confirmation. Allowed roots and anything that is or contains a mount point cannot be deleted, moved or copied.
- Renames, finished copies and uploads never replace an existing file unless the user chose Replace, and then the old file goes to the trash.
- Files shown inline get `Content-Security-Policy: sandbox`; HTML and scripts are sent as plain text.
- Exclude patterns are validated; rsync, tar and zip get arguments, never a shell.

### Privileges

The service runs as root, because mounting drives needs it. The container needs `CAP_SYS_ADMIN` and the devices; source folders are best mounted read-only.

---

## Deployment

### Docker

Build stages: `node:22-alpine` builds the frontend, `rust:1-alpine` builds a static musl binary, and `alpine:3.22` is the runtime with rsync, util-linux, GNU tar, zip, openssl, filesystem tools (ext4, NTFS, exFAT), CA certificates and time zones. The image serves on port 8080.

`docker-compose.yml` mounts `dynamight.toml` read-only, keeps the databases in a volume, shares `/mnt` with `rshared` propagation (so mounts are visible on the host), adds `SYS_ADMIN` and `/dev`, and gives runs 30 s to stop.

### Native Installation

```bash
tar -xzf dynamight-<version>.tar.gz
cd dynamight-<version>
sudo ./scripts/install.sh
```

| Path | Contents |
|------|----------|
| `/opt/dynamight/` | Binary, static files, scripts |
| `/etc/dynamight/dynamight.toml` | Configuration |
| `/var/lib/dynamight/` | Databases |
| journald (`journalctl -u dynamight`) | Server log |

`scripts/update.sh` checks the new binary, stops the service, backs up the program and both databases to `/opt/dynamight-backups/<timestamp>` (keeping 3), installs, waits for `/api/system/health` to report the new version, and rolls back automatically if it does not. `update.sh rollback` returns to the previous version.

### Development

`./scripts/dev.sh` starts the backend and the Vite dev server (hot reload, API proxied to the backend).

---

## Configuration Reference

Settings come from `dynamight.toml` (see `dynamight.toml.example`), found via `DYNAMIGHT_CONFIG`, `./dynamight.toml` or `/etc/dynamight/dynamight.toml`. Environment variables override the file.

| TOML key | Environment | Default | Description |
|----------|-------------|---------|-------------|
| `security.jwt_secret` | `JWT_SECRET` | (required) | Signs sessions and derives the credential key; at least 32 characters |
| `security.secure_cookies` | `SECURE_COOKIES` | `true` | `Secure` cookie flag (needs HTTPS) |
| `security.allowed_browse_paths` | `ALLOWED_BROWSE_PATHS` | `["/mnt", "/home", "/media"]` | Paths the file manager may show and change |
| `security.cors_origins` | `CORS_ORIGINS` | `[]` | Extra allowed origins (same-origin only when empty) |
| `security.trusted_proxies` | `TRUSTED_PROXIES` | `[]` | Proxies whose `X-Forwarded-For` is trusted |
| `server.host` | `HOST` | `0.0.0.0` | Interface to bind |
| `server.port` | `PORT` | `8080` | Port |
| `server.static_files_dir` | `STATIC_FILES_DIR` | `static` | Frontend files |
| `server.timezone` | `TZ` | `UTC` | Timezone for schedules |
| `database.url` | `DATABASE_URL` | `sqlite:data/dynamight.db` | Main database |
| `database.logs_url` | `LOGS_DATABASE_URL` | `logs.db` next to the main database | Logs database |
| `logging.level` | `RUST_LOG` | `info,dynamight=debug` | Log filter |
| `rate_limit.max_attempts` | `RATE_LIMIT_MAX_ATTEMPTS` | `5` | Failed attempts before a lockout |
| `rate_limit.window_secs` | `RATE_LIMIT_WINDOW_SECS` | `60` | Counting window |
| `rate_limit.lockout_secs` | `RATE_LIMIT_LOCKOUT_SECS` | `60` | First lockout |
| `rate_limit.max_lockout_secs` | `RATE_LIMIT_MAX_LOCKOUT_SECS` | `3600` | Longest lockout |
| `limits.max_download_size` | `MAX_DOWNLOAD_SIZE` | 1 TiB | Largest single file download (previews are not limited) |
| `limits.max_request_body_size` | `MAX_REQUEST_BODY_SIZE` | 10 MiB | Request body limit |

Settings changed in the UI (run, log and trash retention, file browser options, notifications) are stored in the database, not in this file.

---

## Directory Compression

When `sync_options.compress_dirs.enabled` is set, each source folder is archived into the staging folder before the transfer, and the provider transfers the staging folder instead.

1. For each source folder, `compress_directory` writes `<name>.<run>.tmp` and renames it when complete. Excluded folders are converted to archive-relative paths.
2. With a timestamp and `max_archives_per_dir`, older archives of that folder and job are removed (any format, so archives from before a format or password change rotate out too).
3. The provider then transfers only this job's archives (`include_only` globs `*_<job id>.<ext>`), so several jobs can share one staging folder; other files there are never transferred or deleted.
4. The space check is skipped (the compression ratio is unknown); dry runs only log what would be archived.

**Archive names**: `[<YYYY-MM-DDTHH-MM-SS>_][<custom_name>_]<folder>_<job id>.<ext>`, for example `2026-03-01T14-30-00_photos_42.tar.gz`.

| Format | Store only | Password | File |
|--------|:----------:|:--------:|------|
| tar.gz | No | No | `.tar.gz` (pigz on all cores when installed, else gzip) |
| tar.gz | No | Yes | `.tar.gz.enc` |
| tar.gz | Yes | No | `.tar` |
| tar.gz | Yes | Yes | `.tar.enc` |
| zip | – | No | `.zip` |
| zip | – | Yes | `.zip.enc` |

With a password, tar or zip writes to a pipe into `openssl enc -aes-256-cbc -pbkdf2`, so no unencrypted copy is written to disk. Decrypt with `openssl enc -d -aes-256-cbc -pbkdf2 -in <file>.enc -out <file>`. GNU tar is required (BusyBox tar lacks `--numeric-owner`). tar exit code 1 and zip exit code 18 (files changed or unreadable) are warnings.

---

## Testing

| Command | Covers |
|---------|--------|
| `cargo test` (in `backend/`) | Unit tests next to the code; API tests in `tests/` against the real router with in-memory databases |
| `cargo test --test providers_live -- --ignored --test-threads=1` | Real WebDAV, SFTP and S3 servers (see the file header for podman commands) |
| `cargo clippy --all-targets` | Lints (kept at zero warnings) |
| `npm test` (in `frontend/`) | Vitest unit tests (job state, draft, schedule text, summaries, ...) |
| `npm run check` | svelte-check type and accessibility checks |

---

## Key Design Decisions

1. **SQLite, two databases**: no database server to run; log lines in their own file keep the main database small and fast.
2. **One backup at a time**: backups compete for the same disks and network; a queue keeps them predictable and avoids two jobs mounting the same drive.
3. **Cancellation tokens and RAII guards**: cancel reacts immediately, and a run's place is released on every path.
4. **Typed run events**: start and finish events have their own channel, so they are never lost among log lines.
5. **Incremental sync by size and time**: works with every provider, including those that only report the upload time.
6. **External tools for local work**: rsync, tar, zip and openssl are proven and fast; Dynamight orchestrates them and never passes data through a shell.
7. **UUID-based mounts with verification**: backups never land in an empty mount-point directory on the system disk.
8. **Trait-based providers**: one interface for all destinations, shared helpers for the remote ones.
9. **Hash routes and a static SPA**: the server only serves files and JSON; hashed assets are cached for good.

---

## Extension Points

- **New provider**: add a `DestinationConfig` variant, implement `SyncProvider` (reuse `common.rs` for walking, change detection, parallel uploads and retries), register it in the factory, add a destination form in `frontend/src/components/jobs/providers/`.
- **New notification channel**: extend `notification_service.rs` and the notification settings component.
- **More parallel runs**: `MAX_PARALLEL_RUNS` in `backup_service.rs`; a job's follow-up run would then need its own guard against running beside itself.
- **Restore from the UI**: not built yet; backups are restored with the destination's own tools (rsync, openssl, provider clients).
- **Multiple users**: the user model and sessions are single-admin today; roles would need authorization checks in the handlers.
