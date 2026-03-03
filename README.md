# Dynamight

A self-hosted backup management system with a web UI, supporting multiple destinations including local drives, cloud storage, SFTP, and WebDAV.

![Rust](https://img.shields.io/badge/Rust-1.83+-orange?logo=rust)
![Svelte](https://img.shields.io/badge/Svelte-5-FF3E00?logo=svelte)
![License](https://img.shields.io/badge/License-MIT-blue)
![Docker](https://img.shields.io/badge/Docker-Ready-2496ED?logo=docker)

## Features

- **Multiple Backup Destinations**
  - Local/USB drives with auto-mount support
  - AWS S3 and S3-compatible (MinIO, Backblaze B2, DigitalOcean Spaces)
  - Google Drive (including shared drives)
  - Microsoft OneDrive
  - SFTP/SSH servers
  - WebDAV (Nextcloud, ownCloud)

- **Job Management**
  - Create, edit, clone, and delete backup jobs
  - Multiple source directories per job
  - Cron-based scheduling (daily, weekly, monthly, custom) with timezone support
  - Manual and scheduled execution
  - Real-time log streaming via WebSocket

- **Directory Compression**
  - Archive source directories (tar.gz or zip) before uploading
  - Optional archive encryption (AES-256-CBC for tar, built-in for zip)
  - Timestamped versioning with configurable per-directory retention
  - Custom archive name prefixes
  - Store-only mode (archive without compression)

- **Customizable Job List**
  - Show, hide, and reorder table columns
  - Drag-to-resize column widths
  - Human-readable schedule display ("Daily at 14:30")
  - Preferences persisted per browser

- **Security**
  - Two-factor authentication (TOTP)
  - AES-256-GCM encrypted credential storage
  - Rate limiting with exponential backoff
  - Argon2 password hashing
  - Secure httpOnly cookies

- **Lightweight**
  - Single binary + SQLite
  - Runs in ~50MB RAM
  - ~80MB Docker image

## Supported Providers

| Provider | Description | Authentication |
|----------|-------------|----------------|
| Local/USB | Rsync to local or mounted drives | None |
| AWS S3 | S3 and S3-compatible storage | Access Key + Secret |
| Google Drive | Google Drive folders and shared drives | OAuth2 |
| OneDrive | Microsoft OneDrive personal/business | OAuth2 |
| SFTP | SSH/SFTP servers | Password or SSH Key |
| WebDAV | Nextcloud, ownCloud, generic WebDAV | Username + Password |

## Quick Start

### Using Docker (Recommended)

```bash
# Clone the repository
git clone https://github.com/yourusername/dynamight.git
cd dynamight

# Create configuration file
cp dynamight.toml.example dynamight.toml

# Edit dynamight.toml and set jwt_secret (required)
# Generate with: openssl rand -base64 32

# Start the container
docker compose up -d
```

Access the UI at **http://localhost:8080** and complete the setup wizard to create your admin account.

### Docker Compose Configuration

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
      # Configuration file
      - ./dynamight.toml:/app/config/dynamight.toml:ro
      # Persistent data
      - dynamight-data:/app/data
      - dynamight-logs:/app/logs
      - /mnt:/mnt:rshared
      # Add your backup sources (read-only recommended):
      # - /home:/source/home:ro
      # - /var/www:/source/www:ro
    environment:
      - TZ=${TZ:-UTC}
      - RUST_LOG=${RUST_LOG:-info,dynamight=debug}
    security_opt:
      - no-new-privileges:true

volumes:
  dynamight-data:
  dynamight-logs:
```

## Configuration

Configuration is done via a TOML file (`dynamight.toml`). Environment variables can override any setting.

### Configuration File

Copy `dynamight.toml.example` to `dynamight.toml` and edit:

```toml
[security]
jwt_secret = "your-secret-here"  # Generate with: openssl rand -base64 32
secure_cookies = true
allowed_browse_paths = ["/mnt", "/home", "/media"]

[server]
host = "0.0.0.0"
port = 8080

[database]
url = "sqlite:data/dynamight.db"
```

See `dynamight.toml.example` for all available options with detailed documentation.

### Configuration Priority

1. **Environment variables** (highest priority) - for Docker/CI overrides
2. **Config file** (`dynamight.toml`)
3. **Built-in defaults** (lowest priority)

### Config File Search Paths

1. `DYNAMIGHT_CONFIG` environment variable (if set)
2. `./dynamight.toml` (current directory)
3. `/etc/dynamight/dynamight.toml` (system-wide)

### Environment Variable Overrides

Any setting can be overridden via environment variables using SCREAMING_SNAKE_CASE:

| Setting | Environment Variable |
|---------|---------------------|
| `security.jwt_secret` | `JWT_SECRET` |
| `server.host` | `HOST` |
| `server.port` | `PORT` |
| `database.url` | `DATABASE_URL` |
| `security.allowed_browse_paths` | `ALLOWED_BROWSE_PATHS` (comma-separated) |

### Key Settings

| Setting | Default | Description |
|---------|---------|-------------|
| `security.jwt_secret` | **required** | Secret for JWT signing and credential encryption |
| `server.host` | `0.0.0.0` | Network interface to bind |
| `server.port` | `8080` | Port to listen on |
| `database.url` | `sqlite:data/dynamight.db` | SQLite database location |
| `security.secure_cookies` | `true` | Require HTTPS for cookies |
| `security.allowed_browse_paths` | `["/mnt", "/home", "/media"]` | Paths users can browse |
| `rate_limit.max_attempts` | `5` | Max failed auth attempts before lockout |

## Local Development

### Prerequisites

- **Rust** 1.83+ ([rustup.rs](https://rustup.rs))
- **Node.js** 20+ ([nodejs.org](https://nodejs.org))
- **rsync** (usually pre-installed on Linux)

### Quick Start

```bash
# Start both backend and frontend with one command
./scripts/dev.sh
```

This will:
- Check dependencies
- Create `dynamight.toml` from `dynamight.toml.example` with a random JWT secret
- Install frontend npm packages
- Start backend on http://localhost:3000
- Start frontend dev server on http://localhost:5173 (with hot-reload)

### Manual Setup

#### Backend

```bash
# Create config file in project root
cp dynamight.toml.example dynamight.toml

# Edit dynamight.toml and set jwt_secret
# Or generate one: openssl rand -base64 32

# Create data directory
mkdir -p data

# Run the backend
cd backend
cargo run
```

#### Frontend

```bash
cd frontend
npm install
npm run dev
```

## Building for Production

### Using Build Script

```bash
./scripts/build.sh
```

Creates `dist/dynamight-<timestamp>.tar.gz` containing:
- Compiled binary (release mode)
- Frontend static files
- Migrations
- Configuration templates
- Installation scripts

### Building Docker Image

```bash
docker build -t dynamight .
```

## Server Deployment

### Using Install Script

```bash
# Extract package
tar -xzf dynamight-*.tar.gz
cd dynamight-*

# Install as system service
sudo ./scripts/install.sh
```

This will:
- Create `dynamight` system user
- Install binary to `/opt/dynamight`
- Create config at `/etc/dynamight/dynamight.toml` (with auto-generated JWT secret)
- Set up data directory at `/var/lib/dynamight`
- Install and configure systemd service

Then configure and start:

```bash
# Review/edit configuration (optional - defaults are sensible)
sudo nano /etc/dynamight/dynamight.toml

# Enable and start service
sudo systemctl enable dynamight
sudo systemctl start dynamight

# Check status
sudo systemctl status dynamight

# View logs
journalctl -u dynamight -f
```

### Uninstalling

```bash
sudo ./scripts/install.sh uninstall
```

## Usage Guide

### Creating a Backup Job

1. Click **"New Job"** on the Dashboard or Jobs page
2. Enter a name and optional description
3. **Select Provider**: Choose your backup destination type
4. **Configure Destination**:
   - **Local/USB**: Set mount point, subdirectory, USB UUID, auto-mount
   - **S3**: Bucket, region, prefix, endpoint (for S3-compatible)
   - **Google Drive**: Folder ID, shared drive ID (optional)
   - **OneDrive**: Folder path, drive ID (optional)
   - **SFTP**: Host, port, username, remote path, auth type
   - **WebDAV**: URL, remote path
5. **Select Credentials** (for cloud providers)
6. **Source Directories**: Browse or type paths to back up
7. **Compress Directories** (optional):
   - Enable to archive each source directory before transfer
   - Set staging path (temporary directory for archives)
   - Choose format (tar.gz or zip) and optionally store-only (no compression)
   - Enable timestamp versioning and set max archives to retain
   - Optionally set an archive password
8. **Sync Options**:
   - Delete extraneous files
   - Exclude patterns and directories
   - Bandwidth limit (rsync/SFTP)
   - Dry run mode
9. Click **"Create Job"**

### Managing Credentials

Credentials for cloud providers are stored encrypted. Manage them in Settings:

1. Click **Settings** (gear icon) in the navbar
2. Go to **Credentials** tab
3. Add, edit, or delete credentials
4. View which jobs use each credential

### Scheduling Backups

Add schedules in the job detail page:

1. Open job details
2. Click **"Add Schedule"**
3. Choose schedule type:
   - **Daily**: Run every day at specified time
   - **Weekly**: Run on specific day of week
   - **Monthly**: Run on specific day of month
   - **Custom**: Enter cron expression
4. Click **"Add Schedule"**

### Two-Factor Authentication

Enable 2FA for additional security:

1. Open Settings (gear icon)
2. Go to **Security** tab
3. Click **Enable 2FA**
4. Scan QR code with authenticator app
5. Enter verification code
6. Save recovery codes

### Managing Job List Columns

Customise which columns appear in the job list:

1. Click the **Columns** button in the job list header
2. Check or uncheck columns to show/hide them (Name and Actions are always visible)
3. Use the up/down arrows to reorder optional columns
4. Drag column borders in the table to resize them
5. Preferences are saved automatically in your browser

### Scheduling with Timezones

Schedules run according to the server's configured timezone (`server.timezone` in `dynamight.toml` or the `TZ` environment variable). Set this to your local timezone (e.g. `Europe/Berlin`, `America/New_York`) so that "Daily at 03:00" means 3 AM local time, not UTC.

## Architecture

```
┌─────────────────┐     ┌─────────────────────────────┐     ┌────────────┐
│   Browser       │────▶│   Axum Server               │────▶│  SQLite    │
│   (Svelte SPA)  │◀────│   (Rust)                    │◀────│            │
└─────────────────┘     └──────────┬──────────────────┘     └────────────┘
                                   │
                                   ▼
                        ┌─────────────────────────────┐
                        │   Provider Layer            │
                        │   ┌─────┐ ┌────┐ ┌──────┐  │
                        │   │Rsync│ │ S3 │ │GDrive│  │
                        │   └─────┘ └────┘ └──────┘  │
                        │   ┌──────┐ ┌────┐ ┌──────┐ │
                        │   │OneDrv│ │SFTP│ │WebDAV│ │
                        │   └──────┘ └────┘ └──────┘ │
                        └─────────────────────────────┘
```

- **Backend**: Rust with Axum framework, SQLite databases (main + logs)
- **Frontend**: Svelte 5 SPA with Tailwind CSS
- **Communication**: REST API + WebSocket for live logs
- **Providers**: Trait-based abstraction for multiple destinations

See [project-description.md](project-description.md) for detailed architecture documentation.

For complete API documentation, see [docs/API.md](docs/API.md).

## API Reference

### Authentication

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/api/auth/setup-required` | GET | Check if setup needed |
| `/api/auth/setup` | POST | Initial admin setup |
| `/api/auth/login` | POST | Authenticate user |
| `/api/auth/logout` | POST | End session |
| `/api/auth/me` | GET | Get current user |
| `/api/auth/change-password` | POST | Change password |

### 2FA / TOTP

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/api/auth/totp/setup` | POST | Generate TOTP secret |
| `/api/auth/totp/enable` | POST | Enable 2FA |
| `/api/auth/totp/disable` | POST | Disable 2FA |
| `/api/auth/totp/validate` | POST | Validate TOTP code |
| `/api/auth/totp/recovery` | POST | Use recovery code |
| `/api/auth/totp/status` | GET | Get 2FA status |

### Jobs

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/api/jobs` | GET | List all jobs |
| `/api/jobs` | POST | Create job |
| `/api/jobs/:id` | GET | Get job details |
| `/api/jobs/:id` | PUT | Update job |
| `/api/jobs/:id` | DELETE | Delete job |
| `/api/jobs/:id/run` | POST | Run job |
| `/api/jobs/:id/cancel` | POST | Cancel running job |
| `/api/jobs/:id/clone` | POST | Clone job |

### Schedules

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/api/jobs/:id/schedules` | GET | List job schedules |
| `/api/jobs/:id/schedules` | POST | Create schedule |
| `/api/schedules/:id` | PUT | Update schedule |
| `/api/schedules/:id` | DELETE | Delete schedule |

### Runs & Logs

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/api/jobs/:id/runs` | GET | List job runs |
| `/api/runs/:id` | GET | Get run details |
| `/api/runs/:id` | DELETE | Delete run |
| `/api/runs/:id/logs` | GET | Get run logs |
| `/api/ws/logs/:runId` | WS | Stream live logs |
| `/api/ws/status` | WS | Global status updates |

### Credentials

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/api/credentials` | GET | List credentials |
| `/api/credentials` | POST | Create credential |
| `/api/credentials/:id` | GET | Get credential |
| `/api/credentials/:id` | PUT | Update credential |
| `/api/credentials/:id` | DELETE | Delete credential |
| `/api/credentials/:id/usage` | GET | Get credential usage |

### Providers

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/api/providers` | GET | List providers |
| `/api/providers/:type/capabilities` | GET | Get capabilities |
| `/api/providers/test` | POST | Test connection |

### System

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/api/system/health` | GET | Health check |
| `/api/system/drives` | GET | List USB drives |
| `/api/system/mounts` | GET | List mount points |
| `/api/system/mount` | POST | Mount drive |
| `/api/system/unmount` | POST | Unmount drive |
| `/api/system/browse` | GET | Browse filesystem |
| `/api/system/mkdir` | POST | Create directory |
| `/api/system/allowed-paths` | GET | Get allowed paths |
| `/api/settings` | GET/PUT | App settings |

## Security Considerations

- **Container Privileges**: Requires `SYS_ADMIN` capability for mount operations. Use `no-new-privileges` security option.
- **Source Mounts**: Mount backup sources as read-only (`:ro`) when possible.
- **HTTPS**: Use a reverse proxy (nginx, Caddy) for HTTPS in production.
- **2FA**: Enable two-factor authentication for additional security.
- **Credentials**: All provider credentials are encrypted with AES-256-GCM.
- **Rate Limiting**: Brute-force protection with exponential backoff.

## Troubleshooting

### Mount Operations Fail

Ensure the container has proper capabilities:

```yaml
cap_add:
  - SYS_ADMIN
devices:
  - /dev:/dev
```

### USB Drives Not Detected

1. Verify `/dev` is mounted in the container
2. Check the drive has a filesystem with UUID: `sudo blkid`
3. Check container logs: `docker compose logs dynamight`

### Permission Denied on Source Directories

Mount source directories as volumes:

```yaml
volumes:
  - /path/to/source:/source/mydata:ro
```

### Cloud Provider Connection Fails

1. Verify credentials are correct
2. Use "Test Connection" button in job configuration
3. Check firewall allows outbound HTTPS
4. For OAuth providers, ensure tokens haven't expired

### Rate Limited

Wait for lockout to expire (check logs for duration) or restart the application.

## Contributing

Contributions are welcome! Please:

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Acknowledgments

- [rsync](https://rsync.samba.org/) — The backbone of reliable file synchronization
- [Axum](https://github.com/tokio-rs/axum) — Ergonomic Rust web framework
- [Svelte](https://svelte.dev/) — Cybernetically enhanced web apps
- [Tailwind CSS](https://tailwindcss.com/) — Utility-first CSS framework
- [AWS SDK for Rust](https://github.com/awslabs/aws-sdk-rust) — S3 integration
- [russh](https://github.com/warp-tech/russh) — SSH/SFTP client
