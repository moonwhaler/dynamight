# Security Audit Report: Dynamight-Web

**Date:** 2026-01-04
**Auditor:** Claude Code
**Application:** Dynamight - USB Backup Management Tool
**Tech Stack:** Rust/Axum (backend), Svelte (frontend), SQLite (database)

---

## Executive Summary

This application is a **backup management tool** that manages USB drives, executes rsync backups, and runs privileged system operations. I've identified several **critical and high-severity security vulnerabilities** that must be addressed before exposing this application to the web.

**Overall Risk Level: CRITICAL**

The most severe issue is that **all API endpoints lack authentication middleware**, allowing any unauthenticated user to perform privileged operations including filesystem browsing, directory creation, and backup job execution.

---

## CRITICAL Severity Issues

### 1. Missing Authentication on ALL API Endpoints

**Location:** `backend/src/main.rs:115-156`
**CVSS Score:** 10.0 (Critical)

**The Issue:** There is **NO authentication middleware** protecting any API route. Every single endpoint (jobs, schedules, logs, system operations, websockets) can be accessed by **any unauthenticated user**.

```rust
// main.rs:115-156 - All routes are defined without any auth middleware
let api_routes = Router::new()
    .route("/jobs", get(handlers::jobs::list_jobs).post(handlers::jobs::create_job))
    .route("/system/mount", post(handlers::system::mount_drive))
    // ... NO auth layer applied
```

Only the `/auth/me`, `/auth/change-password`, and TOTP routes manually check for authentication. All other routes (including critical ones like mount/unmount, job execution, and browsing the filesystem) are **completely open**.

**Impact:** An unauthenticated attacker can:
- Mount/unmount any filesystem
- Browse any directory on the system
- Create and execute backup jobs
- Delete all logs and job history
- Create directories anywhere on the system

**Remediation:** Add a global authentication middleware layer to all routes except public endpoints (`/auth/login`, `/auth/setup*`, `/system/health`).

---

### 2. Unauthenticated WebSocket Access

**Location:** `backend/src/handlers/websocket.rs:13-120`
**CVSS Score:** 7.5 (High)

**The Issue:** WebSocket handlers have **zero authentication checks**:

```rust
// websocket.rs:13-19
pub async fn ws_logs_handler(
    ws: WebSocketUpgrade,
    Path(run_id): Path<i64>,
    State(state): State<Arc<AppState>>,
) -> Response {
    ws.on_upgrade(move |socket| handle_logs_socket(socket, run_id, state))
}
// No token validation anywhere
```

**Impact:** Anyone can connect to `/api/ws/logs/:run_id` or `/api/ws/status` and receive real-time backup logs, potentially exposing sensitive file paths, usernames, and system information.

**Remediation:** Validate JWT token from query parameter or cookie before upgrading the WebSocket connection.

---

### 3. Arbitrary Filesystem Browsing

**Location:** `backend/src/handlers/system.rs:77-110`
**CVSS Score:** 9.1 (Critical)

**The Issue:** The `browse_path` endpoint allows browsing any path on the system. The only protection is `canonicalize()` which prevents `..` traversal but allows access to **any absolute path**:

```rust
// system.rs:77-97
pub async fn browse_path(...) -> impl IntoResponse {
    let path = query.path.as_deref().unwrap_or("/");
    let canonical = match std::fs::canonicalize(path) { ... };
    // No check that the path is within allowed directories!
    state.mount_service.browse_path(&canonical.to_string_lossy())
}
```

**Impact:** An attacker can enumerate the entire filesystem: `/etc/passwd`, `/etc/shadow` (if readable), SSH keys, application secrets, etc.

**Remediation:** Implement an allowlist of browseable paths (e.g., only mount points and configured source directories).

---

### 4. Arbitrary Directory Creation

**Location:** `backend/src/handlers/system.rs:117-145`
**CVSS Score:** 8.1 (High)

**The Issue:** The `mkdir` endpoint only blocks `/proc`, `/sys`, `/dev`, and `..` patterns:

```rust
// system.rs:124-128
if path_str.starts_with("/proc")
    || path_str.starts_with("/sys")
    || path_str.starts_with("/dev")
    || path_str.contains("..")
```

**Impact:** An attacker can create directories anywhere else, such as `/root/.ssh/`, `/etc/cron.d/`, `/var/spool/cron/`, potentially enabling further attacks.

**Remediation:** Whitelist allowed parent directories for directory creation (e.g., only under `/mnt`).

---

## HIGH Severity Issues

### 5. CORS Completely Disabled

**Location:** `backend/src/main.rs:162`
**CVSS Score:** 8.1 (High)

```rust
.layer(CorsLayer::permissive())  // Allows ANY origin
```

**Impact:** Any malicious website can make authenticated API requests if a user has an active session. Combined with the lack of CSRF protection, this enables cross-origin attacks.

**Remediation:** Configure specific allowed origins based on deployment environment.

---

### 6. No Rate Limiting on Login/TOTP

**Location:** `backend/src/handlers/auth.rs`, `backend/src/handlers/totp.rs`
**CVSS Score:** 7.5 (High)

**The Issue:** No rate limiting on:
- Password login attempts (`auth.rs:22-112`)
- TOTP code validation (`totp.rs:296-415`)
- Recovery code attempts (`totp.rs:418-554`)

**Impact:** Attackers can brute-force passwords, TOTP codes (1,000,000 combinations), and recovery codes without restriction.

**Remediation:** Implement rate limiting (e.g., 5 failed attempts per minute, exponential backoff).

---

### 7. Missing Secure Cookie Flag

**Location:** `backend/src/handlers/auth.rs:99-102`, `backend/src/handlers/totp.rs:402-404`
**CVSS Score:** 6.5 (Medium-High)

```rust
let cookie = format!(
    "token={}; HttpOnly; SameSite=Strict; Path=/; Max-Age=86400",
    token
);
// Missing: Secure flag
```

**Impact:** When accessed over HTTP, the JWT token can be intercepted via man-in-the-middle attacks.

**Remediation:** Add `Secure` flag when running in production or behind HTTPS.

---

### 8. JWT Secret Weak Validation

**Location:** `backend/src/config.rs:20-21`
**CVSS Score:** 6.0 (Medium)

```rust
jwt_secret: env::var("JWT_SECRET")
    .expect("JWT_SECRET must be set"),
// No minimum length validation
```

**Impact:** A weak or short JWT secret can be brute-forced. The `.env.example` shows a placeholder that users might not change.

**Remediation:** Enforce minimum 32-character secret length and validate entropy.

---

## MEDIUM Severity Issues

### 9. Container Runs as Root + SYS_ADMIN Capability

**Location:** `Dockerfile:71-72`, `docker-compose.yml:8-9`
**CVSS Score:** 6.7 (Medium)

```dockerfile
# Dockerfile - Runs as root (required for mount operations)
ENTRYPOINT ["./dynamight"]
```

```yaml
# docker-compose.yml
cap_add:
  - SYS_ADMIN
devices:
  - /dev:/dev
```

**Impact:** If the application is compromised, the attacker has full root access to the container and can interact with host devices.

**Remediation:** Consider capability dropping after initialization, or using a dedicated mount daemon with minimal privileges.

---

### 10. Rsync Exclude Patterns Not Sanitized

**Location:** `backend/src/services/backup_service.rs:663-665`
**CVSS Score:** 4.3 (Medium)

```rust
for exclude in job.excludes_vec() {
    args.push(format!("--exclude={}", exclude));
}
```

**Impact:** While rsync's `--exclude` is relatively safe, specially crafted patterns could potentially cause unexpected behavior.

**Remediation:** Validate exclude patterns against a safe pattern regex.

---

### 11. Logs Expose Full System Paths

**Location:** `backend/src/services/backup_service.rs:253-605`
**CVSS Score:** 4.3 (Medium)

**Impact:** Job logs contain full file paths which could reveal system structure, usernames, and sensitive directory names to anyone with log access.

**Remediation:** Consider path sanitization in logs or restrict log access more strictly.

---

### 12. No CSP Headers

**Location:** Not implemented
**CVSS Score:** 4.0 (Medium)

**Impact:** The application doesn't set Content-Security-Policy headers, leaving it vulnerable to XSS if any injection point is found in the future.

**Remediation:** Implement restrictive CSP headers via middleware.

---

## LOW Severity / Informational

### 13. Default JWT Token Expiry

The JWT token expires after 24 hours (`Max-Age=86400`). Consider shorter expiry with refresh tokens for sensitive operations.

### 14. No Account Lockout

Failed login attempts don't trigger account lockout, only rate limiting would help (which is also missing).

### 15. Verbose Error Messages

Some error messages expose internal details (e.g., database errors). Consider generic error messages in production.

---

## Summary of Required Fixes (Priority Order)

| Priority | Issue | Severity | Fix |
|----------|-------|----------|-----|
| **P0** | Missing auth middleware | Critical | Add global auth layer to all routes except `/auth/login`, `/auth/setup*`, `/system/health` |
| **P0** | Unauthenticated WebSocket | Critical | Require valid JWT token before upgrading WebSocket |
| **P0** | Arbitrary filesystem browsing | Critical | Restrict browsing to allowed paths only (mount points, source dirs) |
| **P0** | Arbitrary mkdir | High | Whitelist allowed parent directories |
| **P1** | CORS permissive | High | Configure specific allowed origins |
| **P1** | No rate limiting | High | Add rate limiting (e.g., 5 attempts per minute for login) |
| **P1** | Missing Secure cookie flag | Medium | Add `Secure` flag when not on localhost |
| **P2** | JWT secret validation | Medium | Enforce minimum 32-character secret |
| **P2** | Container security | Medium | Consider rootless alternatives or capability dropping |
| **P2** | Add CSP headers | Medium | Implement restrictive Content-Security-Policy |

---

## Proof of Concept Attack Scenarios

### Scenario 1: Complete System Enumeration
```bash
# No authentication required
curl http://target:8080/api/system/browse?path=/etc
curl http://target:8080/api/system/browse?path=/home
curl http://target:8080/api/system/browse?path=/root
```

### Scenario 2: Malicious Backup Job Creation
```bash
# Create a job that syncs sensitive data to attacker-controlled location
curl -X POST http://target:8080/api/jobs \
  -H "Content-Type: application/json" \
  -d '{"name":"exfil","source_dirs":["/etc","/home"],"mount_point":"/mnt/attacker"}'
```

### Scenario 3: WebSocket Log Snooping
```javascript
// Connect without authentication
const ws = new WebSocket('ws://target:8080/api/ws/status');
ws.onmessage = (e) => console.log('Leaked:', e.data);
```

---

## Recommendation

**Do NOT expose this application to the internet in its current state.**

The missing authentication middleware means that any attacker can:

1. Access `/api/system/browse?path=/` to read any file listing
2. Create directories via `/api/system/mkdir`
3. Mount/unmount filesystems
4. Execute arbitrary rsync commands via job creation
5. Delete all logs and history
6. Monitor real-time backup operations via WebSocket

This effectively provides **root-level system access** to any unauthenticated user who can reach the application.

**Minimum requirements before web exposure:**
1. Implement authentication middleware on all protected routes
2. Restrict filesystem operations to allowed paths
3. Add rate limiting on authentication endpoints
4. Configure CORS with specific origins
5. Add the Secure flag to cookies
6. Implement CSP headers

---

## Appendix: Files Reviewed

- `backend/src/main.rs` - Router configuration
- `backend/src/handlers/auth.rs` - Authentication handlers
- `backend/src/handlers/totp.rs` - 2FA handlers
- `backend/src/handlers/system.rs` - System operation handlers
- `backend/src/handlers/jobs.rs` - Job management handlers
- `backend/src/handlers/logs.rs` - Log access handlers
- `backend/src/handlers/schedules.rs` - Schedule handlers
- `backend/src/handlers/websocket.rs` - WebSocket handlers
- `backend/src/services/auth_service.rs` - Auth service
- `backend/src/services/backup_service.rs` - Backup execution
- `backend/src/services/mount_service.rs` - Mount operations
- `backend/src/config.rs` - Configuration
- `frontend/src/lib/api.ts` - Frontend API client
- `Dockerfile` - Container build
- `docker-compose.yml` - Container orchestration
- `.env` / `.env.example` - Environment configuration
