# Dynamight API Reference

Complete REST API documentation for the Dynamight backup management system.

**Base URL:** `/api`

**Authentication:** Most endpoints require a valid JWT token sent as an `HttpOnly` cookie named `token`. WebSocket endpoints accept the token as a query parameter.

---

## Table of Contents

- [Authentication](#authentication)
- [Two-Factor Authentication (TOTP)](#two-factor-authentication-totp)
- [Jobs](#jobs)
- [Schedules](#schedules)
- [Runs & Logs](#runs--logs)
- [Credentials](#credentials)
- [Providers](#providers)
- [System](#system)
- [Files](#files)
- [Settings](#settings)
- [WebSocket](#websocket)
- [Error Responses](#error-responses)

---

## Authentication

### Check Setup Required

Check if initial setup (admin account creation) is needed.

```
GET /auth/setup-required
```

**Authentication:** None

**Response:**
```json
{
  "setup_required": true
}
```

---

### Initial Setup

Create the initial admin account. Only works when no users exist.

```
POST /auth/setup
```

**Authentication:** None

**Request Body:**
```json
{
  "username": "admin",
  "password": "securepassword123"
}
```

**Response:**
```json
{
  "id": 1,
  "username": "admin"
}
```

Sets `token` cookie on success.

---

### Login

Authenticate with username and password.

```
POST /auth/login
```

**Authentication:** None

**Request Body:**
```json
{
  "username": "admin",
  "password": "securepassword123"
}
```

**Response (no 2FA):**
```json
{
  "id": 1,
  "username": "admin"
}
```

**Response (2FA enabled):**
```json
{
  "totp_required": true,
  "totp_token": "temporary-token-for-totp-validation"
}
```

Sets `token` cookie on success (if no 2FA).

---

### Logout

End the current session. The token is revoked on the server, so copies of it stop working too.

```
POST /auth/logout
```

**Authentication:** Required

**Response:**
```json
{
  "message": "Logged out"
}
```

Clears the `token` cookie.

---

### Get Current User

Get the authenticated user's information.

```
GET /auth/me
```

**Authentication:** Required

**Response:**
```json
{
  "id": 1,
  "username": "admin",
  "totp_enabled": true
}
```

---

### Change Password

Change the current user's password.

```
POST /auth/change-password
```

**Authentication:** Required

**Request Body:**
```json
{
  "current_password": "oldpassword",
  "new_password": "newpassword123"
}
```

**Response:**
```json
{
  "message": "Password changed"
}
```

---

## Two-Factor Authentication (TOTP)

### Get 2FA Status

Check if 2FA is enabled for the current user.

```
GET /auth/totp/status
```

**Authentication:** Required

**Response:**
```json
{
  "enabled": false
}
```

---

### Setup 2FA

Generate a new TOTP secret and QR code.

```
POST /auth/totp/setup
```

**Authentication:** Required

**Response:**
```json
{
  "secret": "JBSWY3DPEHPK3PXP",
  "qr_code": "data:image/png;base64,iVBORw0KGgo...",
  "otpauth_url": "otpauth://totp/Dynamight:admin?secret=JBSWY3DPEHPK3PXP&issuer=Dynamight"
}
```

---

### Enable 2FA

Enable 2FA after verifying a TOTP code.

```
POST /auth/totp/enable
```

**Authentication:** Required

**Request Body:**
```json
{
  "secret": "JBSWY3DPEHPK3PXP",
  "code": "123456"
}
```

**Response:**
```json
{
  "enabled": true,
  "recovery_codes": [
    "ABCD-1234-EFGH",
    "IJKL-5678-MNOP",
    "QRST-9012-UVWX",
    "YZAB-3456-CDEF",
    "GHIJ-7890-KLMN"
  ]
}
```

---

### Disable 2FA

Disable 2FA for the current user.

```
POST /auth/totp/disable
```

**Authentication:** Required

**Request Body:**
```json
{
  "code": "123456"
}
```

**Response:**
```json
{
  "enabled": false
}
```

---

### Validate TOTP Code

Complete login by validating TOTP code (when 2FA is enabled).

```
POST /auth/totp/validate
```

**Authentication:** None (uses totp_token)

**Request Body:**
```json
{
  "totp_token": "temporary-token-from-login",
  "code": "123456"
}
```

**Response:**
```json
{
  "id": 1,
  "username": "admin"
}
```

Sets `token` cookie on success.

---

### Use Recovery Code

Complete login using a recovery code (when 2FA is enabled).

```
POST /auth/totp/recovery
```

**Authentication:** None (uses totp_token)

**Request Body:**
```json
{
  "totp_token": "temporary-token-from-login",
  "recovery_code": "ABCD-1234-EFGH"
}
```

**Response:**
```json
{
  "id": 1,
  "username": "admin",
  "remaining_codes": 4
}
```

Sets `token` cookie on success.

---

## Jobs

### List Jobs

Get all backup jobs.

```
GET /jobs
```

**Authentication:** Required

**Response:**
```json
[
  {
    "id": 1,
    "name": "Daily Backup",
    "description": "Backup home directory",
    "enabled": true,
    "source_dirs": ["/home/user/documents", "/home/user/photos"],
    "destination_type": "local",
    "destination": {
      "type": "local",
      "mount_point": "/mnt/backup",
      "backup_subdir": "daily",
      "usb_uuid": "1234-5678",
      "auto_mount": true,
      "auto_unmount": true
    },
    "sync_options": {
      "delete_extraneous": false,
      "exclude_patterns": ["*.tmp", ".cache"],
      "exclude_dirs": [],
      "bandwidth_limit_kbps": null,
      "dry_run": false,
      "verbosity": "normal",
      "provider_options": null,
      "space_check": "warn",
      "compress_dirs": null
    },
    "credential_id": null,
    "created_at": "2026-01-01T00:00:00Z",
    "updated_at": "2026-01-01T00:00:00Z",
    "last_run_status": "completed",
    "last_run_at": "2026-01-05T02:00:00Z",
    "dest_storage_free": 64944992256,
    "dest_storage_total": 500107862016,
    "dest_storage_updated_at": "2026-01-05T02:15:00Z",
    "schedules": [
      {
        "id": 1,
        "job_id": 1,
        "enabled": true,
        "cron_expression": "00 02 * * *",
        "schedule_type": "daily",
        "time_of_day": "02:00",
        "day_of_week": null,
        "day_of_month": null,
        "last_run_at": "2026-01-05T02:00:00Z",
        "next_run_at": "2026-01-06T02:00:00Z",
        "created_at": "2026-01-01T00:00:00Z",
        "updated_at": "2026-01-01T00:00:00Z"
      }
    ]
  }
]
```

`last_run_*` and `dest_storage_*` are omitted when unknown. The response also contains legacy top-level fields (`mount_point`, `usb_uuid`, `auto_mount`, `auto_unmount`, `backup_subdir`, `sync_deletes`, `rsync_excludes`, `checksum_mode`, `compress`, `dry_run`, `bandwidth_limit`, `verbosity`) kept for backwards compatibility; use `destination` and `sync_options` instead.

---

### Get Job

Get a single job by ID.

```
GET /jobs/:id
```

**Authentication:** Required

**Response:** Same as single item in list response.

---

### Create Job

Create a new backup job.

```
POST /jobs
```

**Authentication:** Required

**Request Body:**
```json
{
  "name": "S3 Backup",
  "description": "Backup to AWS S3",
  "enabled": true,
  "source_dirs": ["/home/user/documents"],
  "destination_type": "s3",
  "destination": {
    "type": "s3",
    "bucket": "my-backup-bucket",
    "prefix": "backups/server1/",
    "region": "us-east-1",
    "endpoint": null,
    "storage_class": "STANDARD"
  },
  "sync_options": {
    "delete_extraneous": false,
    "exclude_patterns": ["*.tmp"],
    "dry_run": false,
    "verbosity": "normal"
  },
  "credential_id": 1
}
```

`destination` is tagged by its `type` field. `destination_type` is optional and defaults to `destination.type`.

**Destination Types:**

| `type` | Fields |
|------|----------------------|
| `local` | `mount_point`, `backup_subdir`, `usb_uuid?`, `auto_mount`, `auto_unmount` |
| `s3` | `bucket`, `prefix`, `region`, `endpoint?`, `storage_class?` |
| `google_drive` | `folder_id`, `shared_drive_id?` |
| `onedrive` | `folder_path`, `drive_id?` |
| `sftp` | `host`, `port`, `username`, `remote_path`, `key_based_auth` |
| `webdav` | `url`, `remote_path` |

**Response:**
```json
{
  "id": 2,
  "name": "S3 Backup",
  ...
}
```

---

### Update Job

Update an existing job.

```
PUT /jobs/:id
```

**Authentication:** Required

**Request Body:** Same as create.

**Response:** Updated job object.

---

### Delete Job

Delete a job and its schedules.

```
DELETE /jobs/:id
```

**Authentication:** Required

**Response:**
```json
{
  "message": "Job deleted"
}
```

---

### Run Job

Start a run now. Backups run one at a time: when another backup is running, the run waits in a queue (status `pending`) and starts when that one finishes.

```
POST /jobs/:id/run
```

**Authentication:** Required

**Response:**
```json
{
  "runId": 10,
  "queued": false
}
```

`queued` is `true` when the run waits for another backup. A job that already has a run running or queued answers `409` with `{"code": "JOB_ALREADY_RUNNING"}`.

---

### Cancel Job

Cancel a running job.

```
POST /jobs/:id/cancel
```

**Authentication:** Required

Stops the job's running run and a queued follow-up run, if any.

**Response:**
```json
{
  "success": true,
  "processKilled": true,
  "cancelled": 1
}
```

`cancelled` is the number of runs stopped. Without an active run the answer is `404` with `{"code": "NO_RUNNING_JOB"}`.

---

### Clone Job

Create a copy of an existing job.

```
POST /jobs/:id/clone
```

**Authentication:** Required

**Response:**
```json
{
  "id": 3,
  "name": "S3 Backup (Copy)",
  ...
}
```

---

## Schedules

### List Job Schedules

Get all schedules for a job.

```
GET /jobs/:id/schedules
```

**Authentication:** Required

**Response:**
```json
[
  {
    "id": 1,
    "job_id": 1,
    "enabled": true,
    "cron_expression": "00 02 * * *",
    "schedule_type": "daily",
    "time_of_day": "02:00",
    "day_of_week": null,
    "day_of_month": null,
    "last_run_at": null,
    "next_run_at": "2026-01-06T02:00:00Z",
    "created_at": "2026-01-01T00:00:00Z",
    "updated_at": "2026-01-01T00:00:00Z"
  }
]
```

---

### Create Schedule

Add a schedule to a job.

```
POST /jobs/:id/schedules
```

**Authentication:** Required

**Request Body (Daily):**
```json
{
  "schedule_type": "daily",
  "time_of_day": "02:00"
}
```

**Request Body (Weekly):**
```json
{
  "schedule_type": "weekly",
  "time_of_day": "02:00",
  "day_of_week": 1
}
```
*day_of_week: 0 = Sunday, 6 = Saturday*

**Request Body (Monthly):**
```json
{
  "schedule_type": "monthly",
  "time_of_day": "02:00",
  "day_of_month": 1
}
```

**Request Body (Custom Cron):**
```json
{
  "schedule_type": "custom",
  "cron_expression": "0 2 * * 1-5"
}
```

If `cron_expression` is given it is used as-is and the other fields are ignored. Otherwise it is built from `schedule_type`, `time_of_day` (`"HH:MM"`, default `"00:00"`), `day_of_week`, and `day_of_month`. All fields are optional; `enabled` defaults to `true`. Invalid expressions return `400` with code `INVALID_CRON`.

**Response:** Created schedule object.

---

### Update Schedule

Update an existing schedule.

```
PUT /schedules/:id
```

**Authentication:** Required

**Request Body:** Same as create.

**Response:** Updated schedule object.

---

### Delete Schedule

Delete a schedule.

```
DELETE /schedules/:id
```

**Authentication:** Required

**Response:**
```json
{
  "message": "Schedule deleted"
}
```

---

## Runs & Logs

### List Job Runs

Get execution history for a job.

```
GET /jobs/:id/runs
```

**Authentication:** Required

**Query Parameters:**
| Parameter | Type | Description |
|-----------|------|-------------|
| `limit` | int | Max results (default: 50) |
| `offset` | int | Skip results (default: 0) |

**Response:**
```json
[
  {
    "id": 10,
    "job_id": 1,
    "schedule_id": 1,
    "status": "completed",
    "started_at": "2026-01-05T02:00:00Z",
    "completed_at": "2026-01-05T02:15:00Z",
    "exit_code": 0,
    "files_transferred": 150,
    "bytes_transferred": 52428800,
    "total_size": 1073741824,
    "error_count": 0,
    "summary": null,
    "log_pruned": false
  }
]
```

**Status Values:** `pending` (queued), `running`, `completed`, `failed`, `cancelled`

`schedule_id` is `null` for manual runs. `summary` holds a failed run's last error line. `log_pruned` is `true` once the log retention removed the run's log lines (the run record stays).

---

### List Recent Runs

Get runs across all jobs, newest first, optionally filtered.

```
GET /runs
```

**Authentication:** Required

**Query Parameters:**
| Parameter | Type | Description |
|-----------|------|-------------|
| `limit` | int | Max results, 1-200 (default: 10) |
| `offset` | int | Skip results (default: 0) |
| `before` | int | Only runs with a smaller id; pass the last id of the previous page to load the next one |
| `job_id` | int | Only runs of this job |
| `status` | string | Comma-separated statuses, e.g. `failed,cancelled` |
| `from` | string | RFC 3339 time; only runs started at or after it |
| `to` | string | RFC 3339 time; only runs started at or before it |

**Response:** Array of run objects (same as [List Job Runs](#list-job-runs)).

---

### Get Run

Get details of a specific run.

```
GET /runs/:id
```

**Authentication:** Required

**Response:** Single run object (same as list item).

---

### Get Run Logs

Get log entries for a run.

```
GET /runs/:id/logs
```

**Authentication:** Required

**Query Parameters:**
| Parameter | Type | Description |
|-----------|------|-------------|
| `limit` | int | Max entries (default: 1000) |
| `offset` | int | Skip entries (default: 0) |
| `level` | string | Filter by level (info, warning, error) |

**Response:**
```json
[
  {
    "id": 1,
    "run_id": 10,
    "timestamp": "2026-01-05T02:00:01Z",
    "level": "info",
    "message": "Starting sync of /home/user/documents"
  },
  {
    "id": 2,
    "run_id": 10,
    "timestamp": "2026-01-05T02:00:02Z",
    "level": "info",
    "message": "documents/file.txt"
  }
]
```

---

### Delete Run

Delete a specific run and its logs.

```
DELETE /runs/:id
```

**Authentication:** Required

**Response:**
```json
{
  "message": "Run deleted"
}
```

---

### Delete Job Runs

Delete all runs for a job.

```
DELETE /jobs/:id/runs
```

**Authentication:** Required

**Response:**
```json
{
  "message": "Runs deleted",
  "count": 15
}
```

---

### Purge All Runs

Delete all runs across all jobs.

```
DELETE /runs
```

**Authentication:** Required

**Response:**
```json
{
  "message": "All runs deleted",
  "count": 100
}
```

---

## Credentials

### List Credentials

Get all saved credentials.

```
GET /credentials
```

**Authentication:** Required

**Query Parameters:**
| Parameter | Type | Description |
|-----------|------|-------------|
| `provider` | string | Filter by provider type |

**Response:**
```json
[
  {
    "id": 1,
    "name": "AWS Production",
    "provider_type": "s3",
    "created_at": "2026-01-01T00:00:00Z",
    "updated_at": "2026-01-01T00:00:00Z"
  },
  {
    "id": 2,
    "name": "Backup Server",
    "provider_type": "sftp",
    "created_at": "2026-01-02T00:00:00Z",
    "updated_at": "2026-01-02T00:00:00Z"
  }
]
```

*Note: Credential secrets are never returned in API responses.*

---

### Get Credential

Get a single credential by ID.

```
GET /credentials/:id
```

**Authentication:** Required

**Response:** Single credential object (without secrets).

---

### Create Credential

Create a new credential.

```
POST /credentials
```

**Authentication:** Required

**Request Body (S3):**
```json
{
  "name": "AWS Production",
  "provider_type": "s3",
  "data": {
    "type": "s3",
    "access_key_id": "AKIAIOSFODNN7EXAMPLE",
    "secret_access_key": "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY"
  }
}
```

**Request Body (SFTP - Password):**
```json
{
  "name": "Backup Server",
  "provider_type": "sftp",
  "data": {
    "type": "sftp",
    "password": "secretpassword"
  }
}
```

**Request Body (SFTP - Key):**
```json
{
  "name": "Backup Server",
  "provider_type": "sftp",
  "data": {
    "type": "sftp",
    "private_key": "-----BEGIN OPENSSH PRIVATE KEY-----\n...",
    "passphrase": "keypassphrase"
  }
}
```

**Request Body (WebDAV):**
```json
{
  "name": "Nextcloud",
  "provider_type": "webdav",
  "data": {
    "type": "webdav",
    "username": "user",
    "password": "password"
  }
}
```

**Request Body (OAuth - Google Drive/OneDrive):**
```json
{
  "name": "Google Account",
  "provider_type": "google_drive",
  "data": {
    "type": "oauth",
    "client_id": "1234567890-abc.apps.googleusercontent.com",
    "client_secret": "GOCSPX-...",
    "refresh_token": "1//0eXx...",
    "access_token": "",
    "expires_at": 0
  }
}
```

OAuth fields:

| Field | Required | Description |
|-------|----------|-------------|
| `refresh_token` | yes | Used to renew access tokens before they expire |
| `client_id` | yes, for renewal | Client ID of your own Google Cloud / Microsoft Entra app |
| `client_secret` | Google: yes; Microsoft: only for confidential clients | Client secret |
| `tenant` | no (Microsoft only) | `common` (default), `consumers`, `organizations` or a tenant ID |
| `scope` | no (Microsoft only) | Scopes for the refresh request, default `offline_access Files.ReadWrite.All` |
| `access_token` | no | May be empty; one is fetched with the refresh token on first use |
| `expires_at` | no | Unix time the access token expires; `0` = refresh on first use |

Renewed tokens (including rotated Microsoft refresh tokens) are stored automatically. On update, OAuth fields that are empty or missing keep their stored values.

**Response:**
```json
{
  "id": 3,
  "name": "AWS Production",
  "provider_type": "s3",
  "created_at": "2026-01-05T00:00:00Z",
  "updated_at": "2026-01-05T00:00:00Z"
}
```

---

### Update Credential

Update a credential's name or data.

```
PUT /credentials/:id
```

**Authentication:** Required

**Request Body:**
```json
{
  "name": "AWS Production (Updated)",
  "data": {
    "type": "s3",
    "access_key_id": "AKIAIOSFODNN7NEWKEY",
    "secret_access_key": "newSecretKey..."
  }
}
```

*Note: `data` is optional - omit to only update name.*

**Response:** Updated credential object.

---

### Delete Credential

Delete a credential.

```
DELETE /credentials/:id
```

**Authentication:** Required

**Response (success):**
```json
{
  "message": "Credential deleted"
}
```

**Response (in use):**
```json
{
  "error": "Credential is in use by 2 job(s)",
  "jobs": ["Daily Backup", "Weekly Backup"]
}
```
*Status: 409 Conflict*

---

### Get Credential Usage

Get jobs using a credential.

```
GET /credentials/:id/usage
```

**Authentication:** Required

**Response:**
```json
{
  "credential_id": 1,
  "jobs": [
    {
      "id": 1,
      "name": "Daily Backup"
    },
    {
      "id": 2,
      "name": "Weekly Backup"
    }
  ]
}
```

---

## Providers

### List Providers

Get all available sync providers.

```
GET /providers
```

**Authentication:** Required

**Response:**
```json
[
  {
    "type": "local",
    "name": "Local / USB",
    "description": "Rsync to local or mounted drives",
    "requires_credentials": false
  },
  {
    "type": "s3",
    "name": "AWS S3",
    "description": "S3 and S3-compatible storage",
    "requires_credentials": true
  },
  {
    "type": "google_drive",
    "name": "Google Drive",
    "description": "Google Drive folders and shared drives",
    "requires_credentials": true
  },
  {
    "type": "onedrive",
    "name": "OneDrive",
    "description": "Microsoft OneDrive",
    "requires_credentials": true
  },
  {
    "type": "sftp",
    "name": "SFTP",
    "description": "SSH/SFTP servers",
    "requires_credentials": true
  },
  {
    "type": "webdav",
    "name": "WebDAV",
    "description": "Nextcloud, ownCloud, generic WebDAV",
    "requires_credentials": true
  }
]
```

---

### Get Provider Capabilities

Get capabilities for a specific provider.

```
GET /providers/:type/capabilities
```

**Authentication:** Required

**Response:**
```json
{
  "provider_type": "local",
  "capabilities": {
    "supports_delete": true,
    "supports_compression": true,
    "supports_checksum": true,
    "supports_bandwidth_limit": true,
    "supports_exclude_patterns": true,
    "supports_incremental": true,
    "supports_dry_run": true,
    "requires_credentials": false
  }
}
```

---

### Test Connection

Test connection to a destination.

```
POST /providers/test
```

**Authentication:** Required

**Request Body (with saved credential):**
```json
{
  "destination": {
    "type": "s3",
    "bucket": "my-bucket",
    "prefix": "backups/",
    "region": "us-east-1"
  },
  "credential_id": 1
}
```

**Request Body (with inline credential):**
```json
{
  "destination": {
    "type": "sftp",
    "host": "backup.example.com",
    "port": 22,
    "username": "backup",
    "remote_path": "/backups",
    "key_based_auth": false
  },
  "credential_data": {
    "type": "sftp",
    "password": "testpassword"
  }
}
```

**Response (success):**
```json
{
  "success": true,
  "message": "Connection successful",
  "details": {
    "bucket": "my-bucket",
    "region": "us-east-1"
  }
}
```

**Response (failure):** HTTP 200 with `success: false`. A failed test is a result, not a request error, so it never answers 401. A saved credential that does not exist answers 404.
```json
{
  "success": false,
  "message": "Access denied: invalid credentials",
  "details": null
}
```

---

## System

### Health Check

Check if the service is running.

```
GET /system/health
```

**Authentication:** None

**Response:**
```json
{
  "status": "ok",
  "version": "0.1.0"
}
```

---

### List Drives

Get available USB drives.

```
GET /system/drives
```

**Authentication:** Required

**Response:**
```json
[
  {
    "name": "sdb1",
    "label": "BACKUP_DRIVE",
    "uuid": "1234-5678",
    "fstype": "ntfs",
    "size": "500G",
    "mountpoint": null
  },
  {
    "name": "sdc1",
    "label": "PHOTOS",
    "uuid": "ABCD-EFGH",
    "fstype": "exfat",
    "size": "1T",
    "mountpoint": "/mnt/photos"
  }
]
```

---

### List Mounts

Get current mount points.

```
GET /system/mounts
```

**Authentication:** Required

**Response:**
```json
[
  {
    "device": "/dev/sdc1",
    "mountpoint": "/mnt/photos",
    "fstype": "exfat",
    "options": "rw,nosuid,nodev"
  }
]
```

---

### Mount Drive

Mount a drive by UUID.

```
POST /system/mount
```

**Authentication:** Required

**Request Body:**
```json
{
  "uuid": "1234-5678",
  "mount_point": "/mnt/backup"
}
```

**Response:**
```json
{
  "message": "Drive mounted",
  "mount_point": "/mnt/backup"
}
```

---

### Unmount Drive

Unmount a drive.

```
POST /system/unmount
```

**Authentication:** Required

**Request Body:**
```json
{
  "mount_point": "/mnt/backup"
}
```

**Response:**
```json
{
  "message": "Drive unmounted"
}
```

---

### Browse Path

Browse directory contents.

```
GET /system/browse
```

**Authentication:** Required

**Query Parameters:**
| Parameter | Type | Description |
|-----------|------|-------------|
| `path` | string | Directory path (default: first allowed path) |

**Response:**
```json
{
  "path": "/home/user",
  "entries": [
    {
      "name": "documents",
      "path": "/home/user/documents",
      "is_dir": true,
      "is_symlink": false,
      "size": null,
      "modified": 1795556800,
      "extension": null
    },
    {
      "name": "file.txt",
      "path": "/home/user/file.txt",
      "is_dir": false,
      "is_symlink": false,
      "size": 1024,
      "modified": 1795556800,
      "extension": "txt"
    }
  ]
}
```

`modified` is in Unix seconds. A symbolic link shows what it points to (`is_symlink: true`). The trash folder and unfinished copies are not listed.

*Note: Only paths within `ALLOWED_BROWSE_PATHS` can be browsed.*

---

### Create Directory

Create a new directory.

```
POST /system/mkdir
```

**Authentication:** Required

**Request Body:**
```json
{
  "path": "/mnt/backup/new-folder"
}
```

**Response:**
```json
{
  "message": "Directory created",
  "path": "/mnt/backup/new-folder"
}
```

---

### Get Allowed Paths

Get paths that can be browsed.

```
GET /system/allowed-paths
```

**Authentication:** Required

**Response:**
```json
{
  "paths": ["/mnt", "/home", "/media"]
}
```

---

## Files

The file manager's own endpoints. Every path must lie within `ALLOWED_BROWSE_PATHS`; it is checked before and after symlinks are resolved.

### Places

```
GET /files/places
```

**Authentication:** Required

**Response:**
```json
{
  "places": [{ "path": "/mnt", "total": 2000398934016, "free": 812345678848 }],
  "drives": [
    {
      "uuid": "f91aeb4d-827f-4003-b327-3b28906d1d18",
      "name": "sdb1",
      "label": "USB2",
      "fstype": "ext4",
      "size": "1.8T",
      "mountpoint": "/mnt/USB2",
      "model": "Extreme SSD",
      "available": "1.2T",
      "total": 1967925211136,
      "free": 1310205534208
    }
  ],
  "trash_items": 3
}
```

`total` and `free` are bytes (`null` when not known, e.g. a drive that is not mounted).

---

### Get File

A file with Range support (`206 Partial Content`), for downloads and for showing files in the browser.

```
GET /files/raw?path=/mnt/USB2/photo.jpg&disposition=inline
```

**Authentication:** Required

| Parameter | Description |
|-----------|-------------|
| `path` | The file |
| `disposition` | `inline` to show the file in the browser; anything else downloads it (`Content-Disposition: attachment`, limited to `MAX_DOWNLOAD_SIZE`) |

With `inline`, images, audio, video and plain text keep their type, PDFs are shown by the browser's viewer, other text (HTML, scripts, source code) is sent as `text/plain`, and binary files are downloaded instead. Inline responses carry `Content-Security-Policy: sandbox`, so nothing in a file can run as the app.

---

### Folder Sizes

```
POST /files/dir-sizes
```

**Authentication:** Required

**Request Body:**
```json
{ "paths": ["/mnt/USB2/photos", "/mnt/USB2/music"] }
```

**Response:** `application/x-ndjson`, one line per folder as soon as its size is known (in any order):
```
{"path":"/mnt/USB2/music","size":4096000}
{"path":"/mnt/USB2/photos","size":81234567}
```

Sizes count files only, do not cross into other mounted drives and are cached for 60 seconds. Closing the request stops the walks. Up to 5000 paths; with `show_directory_sizes` off the response is empty.

---

### Move to Trash

```
POST /files/trash
```

**Authentication:** Required

**Request Body:**
```json
{ "paths": ["/mnt/USB2/old", "/mnt/USB2/notes.txt"] }
```

**Response:**
```json
{
  "moved": [
    {
      "id": "8b425d6c7c284fdbaea7f52da31d7ac0",
      "root": "/mnt/USB2/.dynamight-trash",
      "name": "old",
      "original_path": "/mnt/USB2/old",
      "deleted_at": 1795556852,
      "is_dir": true,
      "size": null
    }
  ],
  "failed": [{ "path": "/mnt/USB2/notes.txt", "reason": "not_found" }]
}
```

Each item moves to a `.dynamight-trash` folder at the top of its drive (or allowed path), so moving is instant. A symbolic link is moved itself, never its target. `reason` is one of `not_allowed`, `not_found`, `protected` (an allowed path itself, or a mount point), `no_anchor` / `cross_device` (no trash on that drive: delete permanently with `DELETE /system/delete` instead) or `io`.

Backups skip `.dynamight-trash` folders and the file manager's unfinished files (`.*.dynamight-part`, `.*.dynamight-upload-*`).

---

### List Trash

```
GET /files/trash
```

**Authentication:** Required

**Response:**
```json
{
  "items": [ { "id": "…", "root": "/mnt/USB2/.dynamight-trash", "name": "old", "original_path": "/mnt/USB2/old", "deleted_at": 1795556852, "is_dir": true, "size": null } ],
  "roots": [ { "root": "/mnt/USB2/.dynamight-trash", "anchor": "/mnt/USB2", "items": 1 } ]
}
```

Only trash folders on attached drives are listed. Items are deleted for good after `trash_retention_days`.

---

### Restore From Trash

```
POST /files/trash/restore
```

**Authentication:** Required

**Request Body:**
```json
{ "items": [{ "root": "/mnt/USB2/.dynamight-trash", "id": "8b425d6c7c284fdbaea7f52da31d7ac0" }], "keep_both": false }
```

**Response:**
```json
{
  "restored": [{ "id": "8b42…", "path": "/mnt/USB2/old" }],
  "conflicts": [],
  "failed": []
}
```

An item whose original place is taken comes back in `conflicts` and stays in the trash; send it again with `"keep_both": true` to restore it as `old (1)`. Missing parent folders are created again.

---

### Delete From Trash

Delete items for good. Needs a recent password check (`POST /system/verify-delete-access`).

```
DELETE /files/trash
```

**Authentication:** Required

**Request Body:** either items, or whole trash folders:
```json
{ "items": [{ "root": "/mnt/USB2/.dynamight-trash", "id": "8b42…" }] }
```
```json
{ "all": true, "roots": ["/mnt/USB2/.dynamight-trash"] }
```

`"all": true` without `roots` empties every trash folder.

**Response:**
```json
{ "purged": 1, "failed": 0 }
```

---

### Copy and Move

```
POST /files/tasks
```

**Authentication:** Required

**Request Body:**
```json
{ "kind": "copy", "sources": ["/mnt/USB2/photos", "/mnt/USB2/notes.txt"], "target": "/mnt/nas/backup" }
```

`kind` is `copy` or `move`. The sources go into the existing folder `target`. Refused with `TARGET_INSIDE_SOURCE` (a folder into itself, or a move to where it already is), or `PATH_NOT_ALLOWED` (a mounted drive, or moving an allowed path itself).

**Response:** `202 Accepted` with the task:
```json
{
  "id": 3,
  "revision": 0,
  "kind": "copy",
  "sources": ["/mnt/USB2/photos"],
  "target": "/mnt/nas/backup",
  "state": "queued",
  "total_bytes": 0,
  "done_bytes": 0,
  "total_files": 0,
  "done_files": 0,
  "current": null,
  "conflict": null,
  "errors": [],
  "failure": null,
  "created_at": 1795558698,
  "finished_at": null
}
```

Tasks run one at a time. `state` is `queued`, `running`, `conflict`, `done`, `failed` or `cancelled`; `revision` grows with every change, so an older copy of a task can be told apart. Copies are written as `.<name>.dynamight-part` and renamed when complete; links are copied as links. A move on one drive is a rename; to another drive each file is copied and then removed, so skipped or failed files stay at the source. Before copying, the free space on the target is checked. Problems with single files are listed in `errors`; the task goes on.

When a name is taken the task waits in state `conflict`:
```json
"conflict": {
  "path": "/mnt/nas/backup/photos/IMG_1.jpg",
  "existing": { "is_dir": false, "size": 4100000, "modified": 1795556800 },
  "incoming": { "is_dir": false, "size": 4200000, "modified": 1795558600 }
}
```
Folders that meet merge: only their files raise conflicts.

---

### List Tasks

```
GET /files/tasks
```

Running tasks and those that ended in the last 10 minutes.

---

### Answer a Conflict

```
POST /files/tasks/:id/resolve
```

```json
{ "choice": "keep_both", "apply_to_all": true }
```

`choice` is `replace` (the existing item goes to the trash first; when its drive has no trash the file is reported as an error instead), `keep_both` (the new item gets a numbered name, `IMG_1 (1).jpg`) or `skip`. With `apply_to_all` later conflicts of the task are answered the same way.

---

### Cancel a Task

```
POST /files/tasks/:id/cancel
```

A file being copied is removed again; what was done before stays.

---

### Rename

```
POST /files/rename
```

```json
{ "path": "/mnt/USB2/IMG_1.jpg", "new_name": "Beach.jpg" }
```

**Response:** `{ "path": "/mnt/USB2/Beach.jpg" }`. Never replaces anything: a taken name gives `409 NAME_TAKEN`. Names must not be empty or contain `/` or `..` (`400 NAME_INVALID`). Changing only the letter case works on drives that ignore it.

---

### Download Several Items

Prepare a download of files and folders as one `.tar` archive, then fetch it with a plain link.

```
POST /files/archive
```

```json
{ "paths": ["/mnt/USB2/photos", "/mnt/USB2/notes.txt"] }
```

**Response:**
```json
{ "url": "/api/files/archive/5f0c…", "filename": "photos-2026-11-24.tar" }
```

`GET` the `url` (valid 5 minutes) to receive the archive, streamed while it is written (no size limit, no `Content-Length`). Names in it are relative to the folder holding the items; links are stored as links. A tar archive (rather than zip) streams any size and opens on Linux, macOS and Windows 11.

---

### Text Preview

```
GET /files/text?path=/mnt/USB2/backup.log
```

**Response:**
```json
{ "text": "…", "binary": false, "truncated": true, "size": 5242880 }
```

The first MiB of the file as text. `binary: true` (and empty `text`) when the file is not text.

---

### Thumbnail

```
GET /files/thumb?path=/mnt/USB2/IMG_1.jpg
```

A JPEG of at most 256 × 256 pixels, turned upright by the photo's orientation. Images (jpg, png, webp, gif up to 64 MB) always; videos (mp4, m4v, mov, mkv, webm, avi) when `ffmpeg` is installed. Other files: `404`. Thumbnails are cached next to the database (`thumbs/`, trimmed to 500 MB).

---

### Upload

Uploads are sent in chunks and resume after a dropped connection, a reload of the page or a server restart.

**1. Start (or resume)**

```
POST /files/uploads
```

```json
{
  "dir": "/mnt/nas/videos",
  "rel_dir": "holiday/2026",
  "name": "clip.mp4",
  "size": 5368709120,
  "mtime": 1795556800000,
  "fingerprint": "clip.mp4|5368709120|1795556800000",
  "on_conflict": "keep_both"
}
```

| Field | Description |
|-------|-------------|
| `dir` | Existing folder the file goes into |
| `rel_dir` | Optional subfolder below `dir` (folder uploads); created as needed |
| `mtime` | Modification time of the original in Unix milliseconds; kept on the uploaded file |
| `fingerprint` | Identifies the client's file; the same file (same folder, name, size and fingerprint) resumes instead of starting over |
| `on_conflict` | `replace` (the existing file goes to the trash) or `keep_both` (`clip (1).mp4`). Without it a taken name gives `409 NAME_TAKEN` with the existing file's `is_dir`, `size` and `modified` in `params` |

**Response:**
```json
{ "id": "9f0c…", "offset": 0, "size": 5368709120, "chunk_size": 8388608 }
```

`offset` is where to continue (above 0 when resuming). An empty file (`size: 0`) is complete right away and the response carries its `path`. Refused with `NOT_ENOUGH_SPACE` when the drive has too little free space.

**2. Send chunks**

```
PUT /files/uploads/:id
Content-Range: bytes 0-8388607/5368709120
```

The body is the chunk (at most `chunk_size` bytes). The chunk must start at the current offset, otherwise `409 UPLOAD_OFFSET_MISMATCH` with the right `offset` in `params`; a chunk cut off by a lost connection is dropped the same way. **Response:** `{ "offset": 8388608, "done": false }`; after the last chunk `{ "offset": …, "done": true, "path": "/mnt/nas/videos/holiday/2026/clip.mp4" }`.

The bytes go to a hidden `.clip.mp4.dynamight-upload-<id>` file next to the target, which is renamed when complete. Only one request at a time may write to an upload (`409 UPLOAD_BUSY`).

**3. After an interruption**

```
GET /files/uploads/:id
```

**Response:** `{ "id": "9f0c…", "offset": 16777216, "size": 5368709120 }`; continue from `offset`.

**Cancel:** `DELETE /files/uploads/:id` removes the partial file. Uploads untouched for 24 hours are removed by the hourly cleanup.

---

## Settings

### Get Settings

Get application settings.

```
GET /settings
```

**Authentication:** Required

**Response:**
```json
{
  "max_runs_per_job": 5,
  "log_retention_days": 90,
  "trash_retention_days": 30,
  "delete_verification_window_minutes": 5,
  "search_timeout_seconds": 10,
  "show_directory_sizes": true
}
```

| Field | Description |
|-------|-------------|
| `max_runs_per_job` | Runs kept per job (older ones are deleted); `null` keeps every run |
| `log_retention_days` | Days a run's log lines are kept; `null` keeps them forever. Each job's newest run and newest failed run always keep their log |
| `trash_retention_days` | Days items stay in the file browser's trash before they are deleted for good; `null` keeps them until the trash is emptied |
| `delete_verification_window_minutes` | How long a password check allows deleting files for good in the file browser (1-60) |
| `search_timeout_seconds` | Time limit of the file browser's subfolder search (3-60) |
| `show_directory_sizes` | Whether the file browser computes folder sizes (on by default) |

---

### Update Settings

Update application settings. Only the fields sent are changed. For `max_runs_per_job`, `log_retention_days` and `trash_retention_days`, `null` means "no limit".

```
PUT /settings
```

**Authentication:** Required

**Request Body:**
```json
{
  "log_retention_days": 30
}
```

**Response:**
```json
{
  "success": true
}
```

---

## WebSocket

### Live Job Logs

Stream the log lines of a run while it runs.

```
WebSocket /ws/logs/:run_id
```

**Authentication:** Session cookie (`token`), sent by the browser with the handshake

**Server Messages:** first `{"type": "ready"}` once the socket listens for the run's lines. Load earlier lines with `GET /runs/:id/logs` after it, so nothing falls between the two. Then one message per log line:
```json
{
  "run_id": 10,
  "level": "info",
  "message": "Syncing /data/photos -> /mnt/backup/backups/photos",
  "source": "rsync",
  "timestamp": "2026-01-05T02:00:01Z"
}
```

The line `Run finished: <status>` (source `system`) is sent once the run's final status and its whole log are stored.

---

### Global Status Updates

Events when runs are queued, start and finish.

```
WebSocket /ws/status
```

**Authentication:** Session cookie (`token`), sent by the browser with the handshake

**Server Messages:**
```json
{
  "type": "job_started",
  "run_id": 10,
  "job_id": 1,
  "timestamp": "2026-01-05T02:00:00Z"
}
```

| `type` | Sent when |
|--------|-----------|
| `job_queued` | A run waits for another backup to finish |
| `job_started` | A run starts |
| `job_completed` | A run ended; its final status and log are stored |
| `resync` | The connection missed events; reload the job list (no other fields) |
| `file_task` | A file manager copy or move changed (progress at most 4 times a second); `task` holds the task as returned by `GET /files/tasks` |
| `file_tasks_resync` | File task events were missed; reload `GET /files/tasks` |

---

## Error Responses

All endpoints return consistent error responses:

**400 Bad Request:**
```json
{
  "error": "Invalid request",
  "message": "Field 'name' is required"
}
```

**401 Unauthorized:**
```json
{
  "error": "Unauthorized",
  "message": "Invalid or expired token"
}
```

**403 Forbidden:**
```json
{
  "error": "Forbidden",
  "message": "Access denied"
}
```

**404 Not Found:**
```json
{
  "error": "Not found",
  "message": "Job not found"
}
```

**409 Conflict:**
```json
{
  "error": "Conflict",
  "message": "Credential is in use"
}
```

**429 Too Many Requests:**
```json
{
  "error": "Rate limited",
  "message": "Too many failed attempts. Try again in 60 seconds.",
  "retry_after": 60
}
```

**500 Internal Server Error:**
```json
{
  "error": "Internal error",
  "message": "An unexpected error occurred"
}
```

---

## Rate Limiting

Authentication endpoints are rate limited:

- **Max attempts:** 5 per minute (configurable)
- **Lockout:** Exponential backoff starting at 60 seconds
- **Max lockout:** 1 hour

Affected endpoints:
- `POST /auth/login`
- `POST /auth/totp/validate`
- `POST /auth/totp/recovery`

When rate limited, the response includes a `Retry-After` header.
