# Dynamight - Project Architecture Documentation

> A web-based rsync backup manager with Rust backend and Svelte frontend, designed for Docker deployment with USB mount capabilities.

## Table of Contents

1. [Overview](#overview)
2. [Technology Stack](#technology-stack)
3. [Architecture Diagram](#architecture-diagram)
4. [Directory Structure](#directory-structure)
5. [Backend Architecture](#backend-architecture)
6. [Frontend Architecture](#frontend-architecture)
7. [Data Flow](#data-flow)
8. [Security Model](#security-model)
9. [Docker Deployment](#docker-deployment)
10. [Native Linux Deployment](#native-linux-deployment)
11. [File Reference](#file-reference)

---

## Overview

Dynamight is a self-hosted backup management application that provides a web UI for configuring and executing rsync-based backups. It replaces manual shell scripts with a visual interface while maintaining the power and flexibility of rsync.

### Core Capabilities

- **Job Management**: Create, edit, delete, and execute backup jobs
- **USB Mount Support**: Auto-detect USB drives by UUID, mount/unmount automatically
- **Scheduling**: Cron-based job scheduling (daily, weekly, monthly, custom)
- **Real-time Logs**: WebSocket streaming of backup progress
- **Filesystem-Aware**: Adapts rsync options based on target filesystem (NTFS, exFAT, ext4, etc.)

---

## Technology Stack

| Layer | Technology | Purpose |
|-------|------------|---------|
| Backend Framework | Axum 0.7 | Async web framework with tower middleware |
| Runtime | Tokio | Async runtime for Rust |
| Database | SQLite + sqlx | Embedded database with compile-time checked queries |
| Authentication | Argon2 + JWT | Password hashing and stateless sessions |
| Frontend Framework | Svelte 5 | Reactive UI with runes ($state, $derived, $effect) |
| Build Tool | Vite 6 | Fast frontend bundling |
| Styling | Tailwind CSS 3 | Utility-first CSS |
| Containerization | Docker + Alpine | Minimal runtime image with rsync tools |

---

## Architecture Diagram

```
┌─────────────────────────────────────────────────────────────────────┐
│                        Docker Container                              │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │                    Dynamight Binary                          │   │
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────────┐  │   │
│  │  │   Axum       │  │  Services    │  │   Scheduler      │  │   │
│  │  │   Router     │──│  Layer       │──│   (Background)   │  │   │
│  │  │              │  │              │  │                  │  │   │
│  │  │  /api/*      │  │  - Auth      │  │  Checks cron     │  │   │
│  │  │  /ws/*       │  │  - Backup    │  │  expressions     │  │   │
│  │  │  /* (static) │  │  - Mount     │  │  every 60s       │  │   │
│  │  └──────────────┘  └──────────────┘  └──────────────────┘  │   │
│  │         │                 │                   │             │   │
│  │         ▼                 ▼                   ▼             │   │
│  │  ┌──────────────────────────────────────────────────────┐  │   │
│  │  │                    SQLite Database                    │  │   │
│  │  │  users | jobs | schedules | job_runs | log_entries   │  │   │
│  │  └──────────────────────────────────────────────────────┘  │   │
│  └─────────────────────────────────────────────────────────────┘   │
│                              │                                      │
│                              ▼                                      │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │                    System Interactions                       │   │
│  │  - rsync (subprocess)     - mount/umount (UUID-based)       │   │
│  │  - blkid (device lookup)  - lsblk (drive enumeration)       │   │
│  │  - findmnt (fs detection) - sync (buffer flush)             │   │
│  └─────────────────────────────────────────────────────────────┘   │
│                              │                                      │
└──────────────────────────────┼──────────────────────────────────────┘
                               │
                               ▼
┌─────────────────────────────────────────────────────────────────────┐
│                         Host System                                  │
│  /mnt/*  (mount points)    /dev/*  (USB devices)                   │
│  Source directories (read-only bind mounts)                         │
└─────────────────────────────────────────────────────────────────────┘
```

---

## Directory Structure

```
dynamight-web/
├── Cargo.toml                 # Rust workspace definition
├── Dockerfile                 # Multi-stage build (node → rust → alpine)
├── docker-compose.yml         # Container orchestration with capabilities
├── .env.example               # Environment variable template
├── .gitignore                 # Git ignore patterns
├── project-description.md     # This file
├── README.md                  # User documentation
│
├── scripts/                   # Build and deployment scripts
│   ├── dev.sh                 # Development server (backend + frontend)
│   ├── build.sh               # Production build and packaging
│   ├── install.sh             # System service installation
│   └── dynamight.service      # Systemd unit file template
│
├── migrations/
│   └── 001_initial.sql        # Database schema (SQLite)
│
├── backend/                   # Rust backend application
│   ├── Cargo.toml             # Backend dependencies
│   └── src/
│       ├── main.rs            # Entry point, server setup, router
│       ├── config.rs          # Environment configuration
│       │
│       ├── db/
│       │   └── mod.rs         # Database initialization, migrations, admin setup
│       │
│       ├── models/            # Data structures (request/response/database)
│       │   ├── mod.rs         # Module exports
│       │   ├── user.rs        # User, LoginRequest, etc.
│       │   ├── job.rs         # Job, CreateJobRequest, JobResponse
│       │   ├── schedule.rs    # Schedule, CreateScheduleRequest
│       │   └── log_entry.rs   # LogEntry, LogMessage, JobRun, JobRunStatus
│       │
│       ├── handlers/          # HTTP request handlers (controllers)
│       │   ├── mod.rs         # Module exports
│       │   ├── auth.rs        # login, logout, me, change_password
│       │   ├── jobs.rs        # CRUD + run_job, cancel_job
│       │   ├── schedules.rs   # CRUD for job schedules
│       │   ├── logs.rs        # list_runs, get_run, get_logs
│       │   ├── system.rs      # drives, mounts, browse, mkdir, health
│       │   └── websocket.rs   # ws_logs_handler, ws_status_handler
│       │
│       └── services/          # Business logic layer
│           ├── mod.rs         # Module exports
│           ├── auth_service.rs    # Password hashing, JWT generation/validation
│           ├── backup_service.rs  # rsync execution, log streaming
│           ├── mount_service.rs   # UUID lookup, mount/unmount, drive listing
│           └── scheduler_service.rs # Cron parsing, job triggering
│
└── frontend/                  # Svelte SPA
    ├── package.json           # Node dependencies
    ├── vite.config.ts         # Vite bundler config (proxy to backend)
    ├── svelte.config.js       # Svelte preprocessor config
    ├── tailwind.config.js     # Tailwind theme customization
    ├── postcss.config.js      # PostCSS plugins
    ├── tsconfig.json          # TypeScript config
    ├── tsconfig.node.json     # TypeScript config for Vite
    ├── index.html             # HTML entry point
    │
    ├── public/
    │   └── favicon.svg        # Application icon
    │
    └── src/
        ├── main.ts            # Svelte mount point
        ├── app.css            # Tailwind imports + custom components
        ├── App.svelte         # Root component (auth gate + router)
        │
        ├── lib/
        │   ├── types.ts       # TypeScript interfaces (Job, Schedule, etc.)
        │   ├── api.ts         # Fetch wrapper for all API endpoints
        │   │
        │   └── stores/        # Svelte stores (global state)
        │       ├── auth.ts    # Authentication state + actions
        │       ├── jobs.ts    # Jobs list state + CRUD actions
        │       └── logs.ts    # WebSocket log streaming + status updates
        │
        ├── components/
        │   ├── layout/        # Page structure components
        │   │   ├── Layout.svelte   # Main layout wrapper
        │   │   ├── Navbar.svelte   # Top navigation bar
        │   │   └── Sidebar.svelte  # Left navigation menu
        │   │
        │   ├── jobs/          # Job-related components
        │   │   ├── JobCard.svelte      # Job summary card for lists
        │   │   ├── RsyncOptions.svelte # Rsync option toggles with descriptions
        │   │   ├── SchedulePicker.svelte # Schedule CRUD UI
        │   │   ├── PathSelector.svelte   # Multi-directory picker with browser modal
        │   │   └── SinglePathSelector.svelte # Single path picker (mount point)
        │   │
        │   └── logs/          # Log viewing components
        │       └── LogViewer.svelte    # Scrollable log display
        │
        └── routes/            # Page components (mapped by router)
            ├── Login.svelte   # Login form
            ├── Dashboard.svelte # Overview with stats and recent activity
            ├── Jobs.svelte    # Job list page
            ├── JobDetail.svelte # Job create/edit form
            └── History.svelte # Backup run history with log viewer
```

---

## Backend Architecture

### Entry Point (`main.rs`)

The application bootstraps in this order:

1. Initialize tracing (logging)
2. Load configuration from environment
3. Connect to SQLite database
4. Run migrations
5. Initialize services (AuthService, BackupService, MountService)
6. Start SchedulerService in background tokio task
7. Build Axum router with all routes
8. Start HTTP server

On first launch, the frontend will detect that no users exist and present a setup wizard to create the administrator account.

### Service Layer

#### AuthService (`services/auth_service.rs`)

Handles all authentication concerns:

- `hash_password(password)` → Argon2 hash string
- `verify_password(password, hash)` → bool
- `generate_token(user_id)` → JWT string (24h expiry)
- `validate_token(token)` → Claims { sub, exp, iat, jti }

#### BackupService (`services/backup_service.rs`)

Orchestrates backup execution:

- Maintains `running_jobs: HashSet<i64>` to prevent concurrent runs
- `execute_job(job, run_id, schedule_id)`:
  1. Mount USB if `auto_mount` is true
  2. Detect filesystem type via `findmnt`
  3. Build rsync arguments based on filesystem and job options
  4. Execute rsync for each source directory
  5. Stream stdout/stderr to WebSocket via broadcast channel
  6. Store logs in database
  7. Unmount if `auto_unmount` is true
  8. Return JobResult with stats

#### MountService (`services/mount_service.rs`)

System interaction for storage devices:

- `get_device_by_uuid(uuid)` → device path via `blkid -U`
- `mount_by_uuid(uuid, mount_point)` → creates dir, mounts
- `unmount(mount_point)` → sync + umount
- `is_mounted(path)` → checks via `mountpoint -q`
- `get_filesystem_type(mount_point)` → via `findmnt`
- `list_usb_drives()` → parses `lsblk -J` for USB devices
- `browse_path(path)` → directory listing

#### SchedulerService (`services/scheduler_service.rs`)

Background cron-like scheduler:

- Runs in separate tokio task
- Checks every 60 seconds for due schedules
- Uses `cron` crate to parse expressions and calculate next run
- Spawns job execution in separate task to not block scheduler

### Handler Layer

Handlers are thin controllers that:

1. Extract request data (path params, query params, JSON body)
2. Call appropriate service methods
3. Perform database queries via sqlx
4. Return JSON responses

### Database Schema

```sql
users           -- Single admin user (id, username, password_hash)
jobs            -- Backup job definitions (source_dirs as JSON array)
schedules       -- Cron schedules linked to jobs
job_runs        -- Execution history (status, timestamps, stats)
log_entries     -- Individual log lines from rsync output
sessions        -- JWT token tracking (for future revocation support)
```

---

## Frontend Architecture

### State Management

Uses Svelte 5 runes (`$state`, `$derived`, `$effect`, `$bindable`) for local component state and custom stores for global state.

#### Stores

- **authStore**: Manages user session, login/logout actions
- **jobsStore**: Cached job list with CRUD operations
- **logStore**: WebSocket connection for live log streaming
- **statusStore**: WebSocket for global job status updates

### Routing

Uses `svelte-spa-router` for hash-based routing:

- `/` → Dashboard
- `/jobs` → Job list
- `/jobs/new` → Create job
- `/jobs/:id` → Edit job
- `/history` → Run history

### Component Hierarchy

```
App.svelte
├── (unauthenticated) Login.svelte
└── (authenticated) Layout.svelte
    ├── Navbar.svelte
    ├── Sidebar.svelte
    └── Router → [Dashboard|Jobs|JobDetail|History]
```

### API Communication

All API calls go through `lib/api.ts` which:

- Prepends `/api` to all endpoints
- Sets `Content-Type: application/json`
- Includes credentials (cookies)
- Handles 401 by redirecting to login
- Throws `ApiError` with status and message

---

## Data Flow

### Job Execution Flow

```
User clicks "Run" button
        │
        ▼
POST /api/jobs/:id/run
        │
        ▼
jobs::run_job handler
├── Creates job_run record (status: pending)
├── Spawns async task
│   ├── Updates status to "running"
│   ├── Calls backup_service.execute_job()
│   │   ├── Mounts USB (if configured)
│   │   ├── For each source_dir:
│   │   │   ├── Spawns rsync subprocess
│   │   │   ├── Streams stdout → broadcast channel → WebSocket
│   │   │   └── Stores log entries in DB
│   │   └── Unmounts USB (if configured)
│   └── Updates job_run with results
└── Returns { runId } immediately
        │
        ▼
Frontend connects to WS /api/ws/logs/:runId
        │
        ▼
LogViewer displays streaming logs
```

### First-Time Setup Flow

```
App loads
    │
    ▼
GET /api/auth/setup-required
    │
    ▼
Returns { setup_required: true } if no users exist
    │
    ▼
Frontend shows Setup wizard
    │
    ▼
User enters username + password
    │
    ▼
POST /api/auth/setup { username, password }
    │
    ▼
Creates admin user, auto-logs in
```

### Authentication Flow

```
Login form submit
        │
        ▼
POST /api/auth/login { username, password }
        │
        ▼
auth::login handler
├── Finds user by username
├── Verifies password with Argon2
├── Generates JWT token
└── Sets httpOnly cookie "token=<jwt>"
        │
        ▼
Subsequent requests include cookie
        │
        ▼
Handlers extract token from cookie
        │
        ▼
AuthService.validate_token() verifies JWT
```

---

## Security Model

### Authentication

- **Password Storage**: Argon2id (memory-hard, resistant to GPU attacks)
- **Session Tokens**: JWT with 24-hour expiry
- **Cookie Flags**: `HttpOnly` (no JS access), `SameSite=Strict` (CSRF protection)

### Container Security

- Runs as root (required for mount operations)
- `SYS_ADMIN` capability only (not full privileged mode)
- `no-new-privileges` security option
- Read-only source directory mounts recommended

### Input Validation

- Path traversal prevented via `std::fs::canonicalize()`
- SQL injection prevented via sqlx parameterized queries
- Mount operations validate UUID format

---

## Docker Deployment

### Build Stages

1. **frontend-builder** (node:20-alpine)
   - Installs npm dependencies
   - Runs `vite build` → outputs to `dist/`

2. **backend-builder** (rust:1.83-alpine)
   - Builds release binary with musl (static linking)
   - Includes migrations in binary via `include_str!`

3. **runtime** (alpine:3.20)
   - Installs rsync, mount utilities, NTFS/exFAT support
   - Copies binary and static files
   - Exposes port 8080

### Required Volumes

```yaml
volumes:
  - dynamight-data:/app/data      # SQLite database
  - dynamight-logs:/app/logs      # Application logs
  - /mnt:/mnt:rshared             # Mount point access
  - /home:/source/home:ro         # Example source (customize)
```

### Required Capabilities

```yaml
cap_add:
  - SYS_ADMIN    # For mount/umount syscalls
devices:
  - /dev:/dev    # For USB device access
```

---

## Native Linux Deployment

For deployment without Docker, use the provided scripts:

### Development

```bash
./scripts/dev.sh
```

Starts both backend and frontend with:
- Backend on http://localhost:3000
- Frontend dev server on http://localhost:5173 (hot-reload)
- Auto-installs dependencies if needed

### Production Build

```bash
./scripts/build.sh
```

Creates `dist/dynamight-<timestamp>.tar.gz` containing:
- Compiled release binary
- Frontend static files
- Database migrations
- Installation scripts
- Configuration templates

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
- `/var/log/dynamight/` - Logs (via journald)

Service management:
```bash
sudo systemctl enable dynamight    # Auto-start on boot
sudo systemctl start dynamight     # Start now
sudo systemctl status dynamight    # Check status
journalctl -u dynamight -f         # View logs
```

### Security Hardening (Systemd)

The systemd service includes:
- Dedicated `dynamight` system user
- `NoNewPrivileges=true`
- `ProtectSystem=strict`
- `ProtectHome=read-only`
- `PrivateTmp=true`
- `CAP_SYS_ADMIN` capability for mount operations

---

## File Reference

### Backend Files

| File | Purpose |
|------|---------|
| `main.rs` | Application entry, router setup, server start |
| `config.rs` | Environment variable parsing (DATABASE_URL, JWT_SECRET, etc.) |
| `db/mod.rs` | Migration runner, admin user creation |
| `models/user.rs` | User struct, LoginRequest, ChangePasswordRequest |
| `models/job.rs` | Job struct with JSON array helpers, CreateJobRequest |
| `models/schedule.rs` | Schedule struct, cron expression builder |
| `models/log_entry.rs` | LogEntry, LogMessage (WebSocket), JobRun, JobRunStatus |
| `handlers/auth.rs` | Login/logout/me/change-password/setup endpoints |
| `handlers/jobs.rs` | Job CRUD + run/cancel endpoints |
| `handlers/schedules.rs` | Schedule CRUD for jobs |
| `handlers/logs.rs` | Job run history and log retrieval |
| `handlers/system.rs` | Drive listing, mount operations, file browser, mkdir |
| `handlers/websocket.rs` | WebSocket handlers for log streaming |
| `services/auth_service.rs` | Argon2 hashing, JWT encode/decode |
| `services/backup_service.rs` | Rsync execution, log broadcasting |
| `services/mount_service.rs` | USB detection, mount/unmount operations |
| `services/scheduler_service.rs` | Background cron scheduler |

### Frontend Files

| File | Purpose |
|------|---------|
| `main.ts` | Svelte app mount point |
| `App.svelte` | Root component, auth gate, router setup |
| `app.css` | Tailwind imports, custom component classes |
| `lib/types.ts` | TypeScript interfaces for all data types |
| `lib/api.ts` | Fetch wrapper with all API endpoints |
| `lib/stores/auth.ts` | Auth state store with login/logout actions |
| `lib/stores/jobs.ts` | Jobs list store with CRUD actions |
| `lib/stores/logs.ts` | WebSocket stores for logs and status |
| `components/layout/Layout.svelte` | Page wrapper with navbar and sidebar |
| `components/layout/Navbar.svelte` | Top bar with logo and logout |
| `components/layout/Sidebar.svelte` | Navigation menu |
| `components/jobs/JobCard.svelte` | Job summary card with run button |
| `components/jobs/RsyncOptions.svelte` | Rsync option toggles |
| `components/jobs/SchedulePicker.svelte` | Schedule management UI |
| `components/jobs/PathSelector.svelte` | Multi-directory picker with browser |
| `components/jobs/SinglePathSelector.svelte` | Single path picker for mount point |
| `components/logs/LogViewer.svelte` | Scrollable log display |
| `routes/Login.svelte` | Login page |
| `routes/Setup.svelte` | First-time setup wizard |
| `routes/Dashboard.svelte` | Overview with stats |
| `routes/Jobs.svelte` | Job list page |
| `routes/JobDetail.svelte` | Job create/edit form |
| `routes/History.svelte` | Run history with log modal |

### Configuration Files

| File | Purpose |
|------|---------|
| `Cargo.toml` | Rust workspace definition |
| `backend/Cargo.toml` | Backend dependencies |
| `package.json` | Frontend dependencies |
| `vite.config.ts` | Vite bundler config with API proxy |
| `tailwind.config.js` | Tailwind theme (primary color) |
| `Dockerfile` | Multi-stage container build |
| `docker-compose.yml` | Container orchestration |
| `.env.example` | Environment variable template |
| `migrations/001_initial.sql` | Database schema |

### Scripts

| File | Purpose |
|------|---------|
| `scripts/dev.sh` | Development server - starts backend and frontend with hot-reload |
| `scripts/build.sh` | Production build - compiles and packages for deployment |
| `scripts/install.sh` | Server installation - installs as systemd service |
| `scripts/dynamight.service` | Systemd unit file template |

---

## Key Design Decisions

1. **SQLite over PostgreSQL**: Simplicity for single-user/self-hosted deployments
2. **Argon2 over bcrypt**: More resistant to modern attack vectors
3. **WebSocket for logs**: Real-time feedback without polling
4. **UUID-based mounts**: More reliable than device paths (/dev/sdX)
5. **Filesystem detection**: Prevents permission errors on FAT32/NTFS
6. **Background scheduler**: Independent of HTTP requests
7. **Svelte 5 runes**: Modern reactive primitives over legacy stores
8. **Alpine base image**: Minimal attack surface, small image size

---

## Extension Points

To extend this application:

- **Add OAuth**: Implement additional auth handlers, modify auth_service
- **Add email notifications**: Create notification_service, call from scheduler
- **Add remote backup (SSH)**: Extend backup_service rsync args with SSH options
- **Add backup verification**: Add post-backup checksum verification step
- **Add retention policies**: Add retention table, cleanup service
- **Add multi-user**: Extend user model with roles, add authorization middleware
