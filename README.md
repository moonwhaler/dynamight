# Dynamight

A modern, self-hosted web interface for managing rsync backups. Configure backup jobs, schedule them, and monitor progress in real-time — all from your browser.

![Rust](https://img.shields.io/badge/Rust-1.83+-orange?logo=rust)
![Svelte](https://img.shields.io/badge/Svelte-5-FF3E00?logo=svelte)
![License](https://img.shields.io/badge/License-MIT-blue)
![Docker](https://img.shields.io/badge/Docker-Ready-2496ED?logo=docker)

## Features

- **Visual Job Management** — Create and configure backup jobs through an intuitive web UI
- **USB Drive Support** — Auto-detect USB drives by UUID, with automatic mount/unmount
- **Smart Scheduling** — Daily, weekly, monthly, or custom cron expressions
- **Real-time Logs** — Watch backup progress live via WebSocket streaming
- **Filesystem-Aware** — Automatically adjusts rsync options for NTFS, exFAT, ext4, etc.
- **Rsync Power** — Full control over rsync options with friendly descriptions
- **Lightweight** — Single binary + SQLite, runs in ~50MB RAM
- **Secure** — Argon2 password hashing, JWT sessions, httpOnly cookies

## Quick Start

### Using Docker (Recommended)

```bash
# Clone the repository
git clone https://github.com/yourusername/dynamight.git
cd dynamight/dynamight-web

# Create environment file
cp .env.example .env

# Edit .env and set required values
# JWT_SECRET=your-secret-key-at-least-32-characters
# ADMIN_PASSWORD=your-admin-password

# Start the container
docker compose up -d

# View logs
docker compose logs -f
```

Access the UI at **http://localhost:8080**

Login with username `admin` and the password you set in `.env`.

### Docker Compose Configuration

The default `docker-compose.yml` includes:

```yaml
services:
  dynamight:
    build: .
    ports:
      - "8080:8080"
    cap_add:
      - SYS_ADMIN          # Required for mount operations
    devices:
      - /dev:/dev          # Required for USB detection
    volumes:
      - dynamight-data:/app/data
      - /mnt:/mnt:rshared  # Mount point access
      # Add your backup sources (read-only recommended):
      # - /home:/source/home:ro
      # - /var/www:/source/www:ro
    environment:
      - JWT_SECRET=${JWT_SECRET}
      - ADMIN_PASSWORD=${ADMIN_PASSWORD}
```

**Important**: Add your source directories as read-only volume mounts to make them accessible for backup.

## Local Development

### Prerequisites

- **Rust** 1.83+ ([rustup.rs](https://rustup.rs))
- **Node.js** 20+ ([nodejs.org](https://nodejs.org))
- **rsync** (usually pre-installed on Linux)

### Quick Start (Recommended)

```bash
# Start both backend and frontend with one command
./scripts/dev.sh
```

This will:
- Check dependencies
- Create `.env` from `.env.example` if needed
- Install frontend npm packages
- Start backend on http://localhost:3000
- Start frontend dev server on http://localhost:5173 (with hot-reload)

### Manual Setup

#### Backend

```bash
cd backend

# Create .env file in project root
cat > ../.env << EOF
JWT_SECRET=development-secret-key-change-in-prod
ADMIN_PASSWORD=admin
DATABASE_URL=sqlite:../data/dynamight.db
RUST_LOG=info,dynamight=debug
EOF

# Create data directory
mkdir -p ../data

# Run the backend
cargo run
```

The API server starts at `http://localhost:3000`.

#### Frontend

```bash
cd frontend

# Install dependencies
npm install

# Start development server (proxies API to backend)
npm run dev
```

The frontend dev server starts at `http://localhost:5173` with hot reload.

## Building for Production

### Using Build Script (Recommended)

```bash
./scripts/build.sh
```

This creates a complete deployment package at `dist/dynamight-<timestamp>.tar.gz` containing:
- Compiled binary (release mode)
- Frontend static files
- Migrations
- Configuration templates
- Installation scripts

### Manual Build

```bash
# Build frontend
cd frontend
npm run build

# Build backend (release mode)
cd ../backend
cargo build --release

# The binary is at target/release/dynamight
# Static files should be in a 'static' directory next to the binary
```

## Server Deployment

### Using Install Script (Recommended)

After building, copy the package to your server and run:

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
- Create config at `/etc/dynamight/.env`
- Set up data directory at `/var/lib/dynamight`
- Install and configure systemd service

Then configure and start:

```bash
# Edit configuration (IMPORTANT: change ADMIN_PASSWORD!)
sudo nano /etc/dynamight/.env

# Enable and start service
sudo systemctl enable dynamight
sudo systemctl start dynamight

# Check status
sudo systemctl status dynamight

# View logs
journalctl -u dynamight -f
```

### Manual Deployment

```bash
# Copy files to server
scp -r dynamight user@server:/opt/

# Create systemd service (see scripts/dynamight.service)
sudo cp scripts/dynamight.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable --now dynamight
```

### Uninstalling

```bash
sudo ./scripts/install.sh uninstall
```

## Configuration

### Environment Variables

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `JWT_SECRET` | Yes | - | Secret key for JWT signing (min 32 chars) |
| `ADMIN_PASSWORD` | No | - | Initial admin password (only used on first run) |
| `DATABASE_URL` | No | `sqlite:data/dynamight.db` | SQLite database path |
| `HOST` | No | `0.0.0.0` | Server bind address |
| `PORT` | No | `8080` | Server port |
| `RUST_LOG` | No | `info` | Log level (trace/debug/info/warn/error) |
| `TZ` | No | `UTC` | Timezone for schedules |

### Changing Admin Password

After initial setup, change the password through the UI or by updating the database directly.

## Usage Guide

### Creating a Backup Job

1. Click **"New Job"** on the Dashboard or Jobs page
2. Enter a name and optional description
3. **Mount Configuration**:
   - Select a USB drive from the dropdown (optional)
   - Set the mount point path (e.g., `/mnt/backup`)
   - Enable auto-mount/unmount if using USB
4. **Source Directories**:
   - Click "Browse" to select directories, or
   - Type paths manually and click "Add"
5. **Rsync Options**:
   - **Mirror Mode**: Delete files not in source (use carefully!)
   - **Checksum**: Compare by content instead of time/size
   - **Compression**: Compress during transfer
   - **Dry Run**: Preview without making changes
   - **Excludes**: Patterns like `*.tmp`, `.cache`, `node_modules`
6. Click **"Create Job"**

### Scheduling Backups

After creating a job, add schedules in the job detail page:

1. Click **"Add Schedule"**
2. Choose schedule type:
   - **Daily**: Run every day at specified time
   - **Weekly**: Run on specific day of week
   - **Monthly**: Run on specific day of month
   - **Custom**: Enter cron expression (e.g., `0 2 * * 1-5` for weekdays at 2 AM)
3. Click **"Add Schedule"**

### Running Backups Manually

- Click the **"Run"** button on any job card or detail page
- View real-time progress in the log viewer
- Check results in the **History** page

### Viewing Logs

- **Live**: Logs stream in real-time when a job is running
- **Historical**: Click "View Logs" on any run in the History page

## Architecture

```
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│   Browser       │────▶│   Axum Server   │────▶│   SQLite DB     │
│   (Svelte SPA)  │◀────│   (Rust)        │◀────│                 │
└─────────────────┘     └────────┬────────┘     └─────────────────┘
                                 │
                                 ▼
                        ┌─────────────────┐
                        │   rsync         │
                        │   mount/umount  │
                        │   (subprocess)  │
                        └─────────────────┘
```

- **Backend**: Rust with Axum framework, SQLite database
- **Frontend**: Svelte 5 SPA with Tailwind CSS
- **Communication**: REST API + WebSocket for live logs

See [project-description.md](project-description.md) for detailed architecture documentation.

## Docker Build Details

The Dockerfile uses a multi-stage build:

1. **Stage 1** (node:20-alpine): Builds the Svelte frontend
2. **Stage 2** (rust:1.83-alpine): Compiles the Rust backend
3. **Stage 3** (alpine:3.20): Minimal runtime with rsync and mount tools

Final image size: ~80MB

### Building the Image

```bash
# Build image
docker build -t dynamight .

# Run container
docker run -d \
  --name dynamight \
  --cap-add SYS_ADMIN \
  --device /dev:/dev \
  -p 8080:8080 \
  -v dynamight-data:/app/data \
  -v /mnt:/mnt:rshared \
  -v /home:/source/home:ro \
  -e JWT_SECRET=your-secret-key \
  -e ADMIN_PASSWORD=changeme \
  dynamight
```

## Security Considerations

- **Container Privileges**: Requires `SYS_ADMIN` capability for mount operations. This is less than full `privileged` mode but still grants significant access.
- **Source Mounts**: Mount backup sources as read-only (`:ro`) to prevent accidental modifications.
- **Network**: By default, binds to all interfaces. Use a reverse proxy (nginx, Caddy) for HTTPS.
- **Passwords**: Change the default admin password immediately after setup.

## Troubleshooting

### Mount Operations Fail

Ensure the container has `SYS_ADMIN` capability and `/dev` device access:

```yaml
cap_add:
  - SYS_ADMIN
devices:
  - /dev:/dev
```

### USB Drives Not Detected

1. Check that `/dev` is mounted in the container
2. Verify the drive has a filesystem with UUID: `sudo blkid`
3. Check container logs: `docker compose logs dynamight`

### Permission Denied on Source Directories

Ensure source directories are mounted as volumes in `docker-compose.yml`:

```yaml
volumes:
  - /path/to/source:/source/mydata:ro
```

### Database Locked Errors

SQLite doesn't support high concurrency. This is normal for single-user deployments. If you see frequent lock errors, ensure only one instance is running.

## API Reference

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/api/auth/login` | POST | Authenticate user |
| `/api/auth/logout` | POST | End session |
| `/api/auth/me` | GET | Get current user |
| `/api/jobs` | GET/POST | List/create jobs |
| `/api/jobs/:id` | GET/PUT/DELETE | Job CRUD |
| `/api/jobs/:id/run` | POST | Trigger job execution |
| `/api/jobs/:id/schedules` | GET/POST | Job schedules |
| `/api/jobs/:id/runs` | GET | Execution history |
| `/api/runs/:id/logs` | GET | Get logs for run |
| `/api/system/drives` | GET | List USB drives |
| `/api/system/browse` | GET | Browse filesystem |
| `/api/system/mkdir` | POST | Create directory |
| `/api/ws/logs/:runId` | WS | Stream live logs |

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
