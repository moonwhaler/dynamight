# Dynamight — Developer Quick Reference

Self-hosted backup manager. Rust (Axum) backend + Svelte 5 frontend. Single binary + SQLite. Docker or systemd deployment.

---

## Stack

| Layer | Technology |
|-------|-----------|
| Backend | Rust, Axum 0.7, Tokio, SQLite via sqlx |
| Frontend | Svelte 5 (runes), TypeScript, Tailwind CSS 4, Vite 6 |
| i18n | Paraglide — `frontend/messages/en.json` + `de.json` |
| Auth | Argon2id passwords, JWT (24h), TOTP 2FA |
| Encryption | AES-256-GCM (credentials), AES-256-CBC via openssl (archives) |
| Runtime | Alpine Docker, or native Linux via systemd |

---

## Key File Locations

```
backend/src/
  main.rs                    # Entry point, router, service wiring
  config.rs                  # Config loading (TOML + env overrides)
  errors.rs                  # ErrorCode enum + ApiError + IntoResponse
  models/
    destination.rs           # DestinationConfig, SyncOptions, CompressFormat, CompressDirsOptions
    job.rs                   # Job, JobResponse, CreateJobRequest, UpdateJobRequest
    schedule.rs              # Schedule
    credential.rs            # CredentialData variants
  handlers/
    jobs.rs                  # Job CRUD + validate_compress_dirs_options(), validate_exclude_dirs()
    auth.rs / totp.rs        # Auth endpoints
    schedules.rs             # Schedule CRUD
    system.rs                # Browse, drives, mounts
  services/
    backup_service.rs        # Job execution orchestrator (do_execute)
    compress_service.rs      # Archive creation + retention cleanup
    scheduler_service.rs     # Timezone-aware cron runner
    credential_service.rs    # AES-256-GCM encrypt/decrypt
    mount_service.rs         # USB blkid/mount/umount
    providers/
      mod.rs                 # SyncProvider trait + factory function
      rsync.rs / s3.rs / googledrive.rs / onedrive.rs / sftp.rs / webdav.rs

frontend/src/
  lib/types.ts               # All TS interfaces — must mirror Rust models
  lib/api.ts                 # Typed API client
  lib/stores/
    auth.ts                  # User session
    jobs.ts                  # Cached job list with CRUD helpers
    tablePreferences.ts      # Job list column config (localStorage-persisted)
    theme.ts / viewPreferences.ts / preferences.ts / language.ts
  components/jobs/
    JobListRow.svelte         # Dynamic table row (iterates visible columns)
    ColumnSelector.svelte     # Column visibility/order popover
    CompressOptions.svelte    # Compression settings UI
    SyncOptions.svelte        # Sync settings (exclude dirs grouped by source)
    ProviderSelector.svelte   # Provider type selection tiles
    providers/                # Provider-specific forms (each embeds CredentialSelector + TestConnection)
  components/logs/
    RunLogModal.svelte        # Live log viewer — imported in both Jobs.svelte AND JobDetail.svelte
  routes/
    Jobs.svelte               # Job list: column management, drag/resize, polling
    JobDetail.svelte          # Job create/edit form
```

---

## Core Data Models

### `DestinationConfig` (`models/destination.rs`)

Tagged enum — serialised as JSON blob in the `jobs` table:
```rust
Local     { mount_point, backup_subdir, usb_uuid, auto_mount, auto_unmount }
S3        { bucket, prefix, region, endpoint, storage_class }
GoogleDrive { folder_id, shared_drive_id }
OneDrive  { folder_path, drive_id }
Sftp      { host, port, username, remote_path, key_based_auth, host_key_fingerprint }
WebDav    { url, remote_path }
```

### `SyncOptions` (`models/destination.rs`)

Also stored as a JSON blob. **New fields must have `#[serde(default)]`** — no migration required.
```rust
delete_extraneous: bool
exclude_patterns:  Vec<String>   // glob patterns
exclude_dirs:      Vec<String>   // absolute paths; must be children of a source_dir
bandwidth_limit_kbps: Option<u32>
dry_run:           bool
verbosity:         String        // "quiet" | "normal" | "verbose"
provider_options:  Option<Value> // rsync: checksum_mode, compress, ignore_times
space_check:       String        // "fail" | "warn" | "none"
compress_dirs:     Option<CompressDirsOptions>
```

### `CompressDirsOptions` (`models/destination.rs`)

```rust
enabled:              bool
format:               CompressFormat   // TarGz | Zip
store_only:           bool             // archive without compression
add_timestamp:        bool             // enables versioned archives
custom_name:          Option<String>   // ^[a-zA-Z0-9_-]{1,64}$
max_archives_per_dir: Option<u32>      // only meaningful when add_timestamp = true; >= 1
staging_path:         String           // flat dir; job ID embedded in every filename
password:             Option<String>   // tar: openssl AES-256-CBC via stdin; zip: built-in -P
```

Archive naming: `[<YYYY-MM-DDTHH-MM-SS>_][<custom_name>_]<sanitised_dir_name>_<job_id>.<ext>`

Encryption extensions: `.tar.gz.enc` / `.tar.enc` for tar+password; `.zip` unchanged (zip handles it internally).

### `Job` (`models/job.rs`)

Key methods:
- `source_dirs_vec()` — parses JSON array
- `get_destination_config()` — deserialises `destination_config` JSON (or constructs from legacy fields)
- `get_sync_options()` — deserialises `sync_options` JSON

`JobResponse` adds `last_run_status`, `last_run_at`, `schedules: Vec<Schedule>`.

---

## Service Layer

### BackupService — Execution Order (`do_execute`)

1. Load job config, decrypt credentials
2. **Compression phase** (if `compress_dirs.enabled`):
   - For each source dir: `compress_service::compress_directory()` → archive in `staging_path/`
   - Excluded dirs honoured via `archive_relative_excludes()` (converted to archive-relative paths)
   - `cleanup_old_archives()` runs after each archive if `max_archives_per_dir` is set
   - Staging path becomes `effective_source_dirs` for the sync
   - Dry-run: logs what would happen, skips actual archiving
3. **Space pre-flight check** (Local provider only; skipped when compression is active)
4. Call `provider.sync(SyncContext { effective_source_dirs, ... })`
5. Persist results, update `last_run_at`, run retention cleanup for job runs

### SyncProvider Trait (`services/providers/mod.rs`)

```rust
pub trait SyncProvider: Send + Sync {
    fn provider_type(&self) -> &'static str;
    fn capabilities(&self) -> ProviderCapabilities;
    fn validate_config(&self, dest, cred) -> Result<()>;
    async fn sync(&self, ctx: Arc<SyncContext>) -> Result<SyncResult>;
    async fn test_connection(&self, dest, cred) -> Result<TestConnectionResult>;
}
```

`SyncContext` contains `source_dirs` (the effective directories after potential compression), `dest`, `sync_options`, `log_fn`, `is_cancelled`.

### SchedulerService (`scheduler_service.rs`)

- Polls every 60 seconds; skips if a job is already running
- Timezone from `server.timezone` (TOML) or `TZ` (env); affects `calculate_next_run()`
- 5-field cron expressions (seconds prepended automatically)
- Preserves "cancelled" status — does not overwrite it with success/failure

---

## Error Handling (`errors.rs`)

**Adding a new error code requires updating three places:**
1. `ErrorCode` enum
2. Helper constructor method on `ApiError`
3. HTTP status match arm in `IntoResponse` — missing arm silently falls through to 500

---

## Handler Conventions (`handlers/jobs.rs`)

Key validation helpers (call before any DB write):
- `validate_compress_dirs_options()` — staging path must be within `allowed_browse_paths` and must not overlap any source_dir (bidirectional)
- `validate_exclude_dirs()` — each exclude_dir must be a strict child of at least one source_dir
- `validate_exclude_pattern()` — rejects shell injection chars, control chars, patterns starting with `-`
- `validate_display_field()` — max length + no control chars

---

## Frontend Patterns

### Svelte 5 Runes

- State: `$state`, `$derived`, `$bindable`, `$effect`
- For `$bindable` props with nested objects: **direct property mutation** (`opts.enabled = true`) — not spread (`opts = { ...opts, enabled: true }`)
- Stores in templates: `$tablePreferencesStore.visibleColumns`

### i18n (Paraglide)

- Import: `import * as m from '$lib/paraglide/messages.js'`
- Use: `m.compress_dirs_enabled()`, `m.job_name()` etc.
- Add keys to **both** `frontend/messages/en.json` and `de.json`
- Compression keys are prefixed `compress_dirs_*`
- **Manual message files** (with parameters) need JSDoc `@param {{...}} inputs` + `@param {{ locale?: "en" | "de" }} options` + `@returns {LocalizedString}` — `svelte-check` fails without these

### TypeScript Interfaces (`lib/types.ts`)

Must stay in sync with Rust models. Key interfaces: `Job`, `Schedule`, `SyncOptions`, `CompressDirsOptions`, `DestinationConfig` (tagged union), `Credential`.

Helper functions in `types.ts`: `createDefaultDestination(type)`, `createDefaultSyncOptions()`.

### Job List Column System

Columns: `job` (fixed first) | `status` | `sources` | `destination` | `last_run` | `schedule` | `options` | `actions` (fixed last).

`tablePreferencesStore` (localStorage key: `dynamight-job-table-prefs`):
- `setColumnVisibility(col, visible)` — fixed columns cannot be hidden
- `setColumnOrder(cols)` — fixed columns always stay at ends
- `setColumnWidth(col, width)` — minimum 60px enforced

`Jobs.svelte` distributes excess table width proportionally across visible columns.

### Component Hierarchy Reference

```
JobDetail.svelte
  ProviderSelector.svelte
  [Provider]Destination.svelte   ← contains CredentialSelector + TestConnection internally
  PathSelector.svelte
  CompressOptions.svelte         ← between path selector and sync options
  SyncOptions.svelte             ← exclude_dirs grouped by source; orphaned entries highlighted
  SchedulePicker.svelte
  RunLogModal.svelte

Jobs.svelte
  ColumnSelector.svelte
  JobListRow.svelte              ← {#each $tablePreferencesStore.visibleColumns as col}
  RunLogModal.svelte             ← lifted from rows; state managed in Jobs.svelte
```

---

## Configuration

Primary: `dynamight.toml` (copy from `dynamight.toml.example`)
Override: environment variables (SCREAMING_SNAKE_CASE, highest priority)

| Purpose | TOML key | Env var |
|---------|----------|---------|
| JWT secret (required) | `security.jwt_secret` | `JWT_SECRET` |
| Server timezone | `server.timezone` | `TZ` |
| Port | `server.port` | `PORT` |
| Database | `database.url` | `DATABASE_URL` |
| Allowed browse paths | `security.allowed_browse_paths` | `ALLOWED_BROWSE_PATHS` |

Config search order: `DYNAMIGHT_CONFIG` env → `./dynamight.toml` → `/etc/dynamight/dynamight.toml`

---

## Database

Two SQLite databases (WAL mode):
- **Main** (`dynamight.db`): `users`, `jobs`, `schedules`, `credentials`, `app_settings`, `sessions`
- **Logs** (`dynamight-logs.db`): `job_runs`, `log_entries` — separate for performance and easy bulk cleanup

`destination_config` and `sync_options` are JSON blobs. New optional fields on `SyncOptions` only need `#[serde(default)]` — no migration.

---

## Critical Gotchas

- **`models/destination` is private** — import types as `crate::models::CompressDirsOptions`, never `crate::models::destination::CompressDirsOptions`
- **GNU tar required on Alpine** — BusyBox tar lacks `--numeric-owner`; Dockerfile installs the `tar` package explicitly
- **New error codes need three edits** in `errors.rs` (enum + constructor + IntoResponse match) — missing the match arm silently returns 500
- **`SyncOptions` additions**: add `#[serde(default)]` to the field, update the manual `Default` impl, and any `Self { ... }` constructors
- **Zip vs. tar encryption differ**: tar uses `openssl enc -aes-256-cbc -pbkdf2` (password via stdin, stronger); zip uses built-in `-P` flag (weaker)
- **Compression staging path** must not overlap with any source_dir — validated bidirectionally in both the handler and at runtime in `compress_service`
- **Dry-run + compression**: logs planned archives but skips actual archiving
- **RunLogModal** is imported in **both** `Jobs.svelte` and `JobDetail.svelte` (not lifted out of one, used in both)
- **Paraglide manual messages** with parameters require full JSDoc annotations or `svelte-check` fails

---

## Adding New Features — Checklist

### New `SyncOptions` field
1. Add to struct in `destination.rs` with `#[serde(default)]`
2. Update manual `Default` impl and any explicit constructors
3. Add to `SyncOptions` interface in `lib/types.ts`
4. Add UI in `SyncOptions.svelte` (or a dedicated component)
5. Add i18n keys to `en.json` and `de.json`

### New provider
1. Implement `SyncProvider` in `services/providers/<name>.rs`
2. Register in factory (`services/providers/mod.rs`)
3. Add `DestinationConfig` variant in `destination.rs`
4. Add TypeScript type + `createDefaultDestination` case in `lib/types.ts`
5. Create `components/jobs/providers/<Name>Destination.svelte` (embed `CredentialSelector` + `TestConnection`)
6. Register in `ProviderSelector.svelte` and `JobDetail.svelte`

### New error code
1. `ErrorCode` enum in `errors.rs`
2. `ApiError` helper constructor
3. HTTP status arm in `IntoResponse` match
