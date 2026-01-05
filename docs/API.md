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

End the current session.

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

### Get Token

Get the current JWT token (for WebSocket authentication).

```
GET /auth/token
```

**Authentication:** Required

**Response:**
```json
{
  "token": "eyJhbGciOiJIUzI1NiIs..."
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
    "destination_config": {
      "mount_point": "/mnt/backup",
      "backup_subdir": "daily",
      "usb_uuid": "1234-5678",
      "auto_mount": true,
      "auto_unmount": true
    },
    "sync_options": {
      "delete_extraneous": false,
      "exclude_patterns": ["*.tmp", ".cache"],
      "bandwidth_limit_kbps": null,
      "dry_run": false,
      "verbosity": "normal"
    },
    "credential_id": null,
    "created_at": "2026-01-01T00:00:00Z",
    "updated_at": "2026-01-01T00:00:00Z",
    "last_run": {
      "id": 5,
      "status": "completed",
      "started_at": "2026-01-05T02:00:00Z",
      "completed_at": "2026-01-05T02:15:00Z"
    },
    "schedules": [
      {
        "id": 1,
        "schedule_type": "daily",
        "hour": 2,
        "minute": 0
      }
    ]
  }
]
```

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
  "destination_config": {
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

**Destination Types:**

| Type | Required Config Fields |
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

Execute a job immediately.

```
POST /jobs/:id/run
```

**Authentication:** Required

**Response:**
```json
{
  "run_id": 10,
  "message": "Job started"
}
```

---

### Cancel Job

Cancel a running job.

```
POST /jobs/:id/cancel
```

**Authentication:** Required

**Response:**
```json
{
  "message": "Job cancelled"
}
```

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
    "schedule_type": "daily",
    "hour": 2,
    "minute": 0,
    "day_of_week": null,
    "day_of_month": null,
    "cron_expression": null,
    "next_run": "2026-01-06T02:00:00Z",
    "created_at": "2026-01-01T00:00:00Z"
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
  "hour": 2,
  "minute": 0
}
```

**Request Body (Weekly):**
```json
{
  "schedule_type": "weekly",
  "hour": 2,
  "minute": 0,
  "day_of_week": 0
}
```
*day_of_week: 0 = Sunday, 6 = Saturday*

**Request Body (Monthly):**
```json
{
  "schedule_type": "monthly",
  "hour": 2,
  "minute": 0,
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
| `limit` | int | Max results (default: 20) |
| `offset` | int | Skip results (default: 0) |

**Response:**
```json
[
  {
    "id": 10,
    "job_id": 1,
    "job_name": "Daily Backup",
    "status": "completed",
    "trigger": "scheduled",
    "schedule_id": 1,
    "started_at": "2026-01-05T02:00:00Z",
    "completed_at": "2026-01-05T02:15:00Z",
    "files_transferred": 150,
    "bytes_transferred": 52428800,
    "files_deleted": 0,
    "error_message": null
  }
]
```

**Status Values:** `pending`, `running`, `completed`, `failed`, `cancelled`

**Trigger Values:** `manual`, `scheduled`

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
    "access_token": "ya29.a0AfH6...",
    "refresh_token": "1//0eXx...",
    "expires_at": 1704499200
  }
}
```

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
  "destination_type": "s3",
  "destination_config": {
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
  "destination_type": "sftp",
  "destination_config": {
    "host": "backup.example.com",
    "port": 22,
    "username": "backup",
    "remote_path": "/backups",
    "key_based_auth": false
  },
  "credential_data": {
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

**Response (failure):**
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
  "parent": "/home",
  "entries": [
    {
      "name": "documents",
      "path": "/home/user/documents",
      "is_dir": true,
      "size": null,
      "modified": "2026-01-01T00:00:00Z"
    },
    {
      "name": "file.txt",
      "path": "/home/user/file.txt",
      "is_dir": false,
      "size": 1024,
      "modified": "2026-01-01T00:00:00Z"
    }
  ]
}
```

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
  "max_runs_per_job": 10,
  "theme": "dark"
}
```

---

### Update Settings

Update application settings.

```
PUT /settings
```

**Authentication:** Required

**Request Body:**
```json
{
  "max_runs_per_job": 20
}
```

**Response:**
```json
{
  "max_runs_per_job": 20,
  "theme": "dark"
}
```

---

## WebSocket

### Live Job Logs

Stream real-time logs for a running job.

```
WebSocket /ws/logs/:run_id?token=<jwt>
```

**Authentication:** JWT token as query parameter

**Server Messages:**
```json
{
  "type": "log",
  "run_id": 10,
  "timestamp": "2026-01-05T02:00:01Z",
  "level": "info",
  "message": "Starting sync..."
}
```

```json
{
  "type": "status",
  "run_id": 10,
  "status": "completed",
  "files_transferred": 150,
  "bytes_transferred": 52428800
}
```

---

### Global Status Updates

Stream status updates for all jobs.

```
WebSocket /ws/status?token=<jwt>
```

**Authentication:** JWT token as query parameter

**Server Messages:**
```json
{
  "type": "job_started",
  "job_id": 1,
  "run_id": 10,
  "job_name": "Daily Backup"
}
```

```json
{
  "type": "job_completed",
  "job_id": 1,
  "run_id": 10,
  "job_name": "Daily Backup",
  "status": "completed"
}
```

```json
{
  "type": "job_progress",
  "job_id": 1,
  "run_id": 10,
  "files_transferred": 50,
  "bytes_transferred": 10485760
}
```

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
