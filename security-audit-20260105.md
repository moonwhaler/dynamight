# Security Audit Report - Dynamight

**Date:** January 5, 2026
**Auditor:** Claude Code Security Review
**Application Version:** As of commit 32fb80d
**Scope:** Full backend and frontend codebase review

---

## Executive Summary

Dynamight is a web-based multi-destination backup manager with a Rust backend (Axum) and Svelte frontend. This security audit examined authentication, authorization, input validation, cryptographic implementations, and provider integrations.

### Overall Assessment: **Good with Room for Improvement**

The application demonstrates solid security foundations with proper password hashing (Argon2), credential encryption (AES-256-GCM), rate limiting, and two-factor authentication support. However, several medium-severity issues were identified that should be addressed before production deployment.

### Risk Summary

| Severity | Count | Status |
|----------|-------|--------|
| Critical | 0 | - |
| High | 1 | Open |
| Medium | 4 | Open |
| Low | 4 | Open |
| Informational | 2 | Open |

---

## Table of Contents

1. [High Severity Findings](#high-severity-findings)
2. [Medium Severity Findings](#medium-severity-findings)
3. [Low Severity Findings](#low-severity-findings)
4. [Informational Findings](#informational-findings)
5. [Security Features Well Implemented](#security-features-well-implemented)
6. [Recommendations Summary](#recommendations-summary)
7. [Files Reviewed](#files-reviewed)

---

## High Severity Findings

### H-01: SFTP Host Key Verification Disabled

**Severity:** High
**Location:** `backend/src/services/providers/sftp.rs:401-408`
**CWE:** CWE-295 (Improper Certificate Validation)
**CVSS:** 7.4 (High)

#### Description

The SFTP provider accepts all SSH host keys without verification, equivalent to running SSH with `StrictHostKeyChecking=no`. This completely disables the protection against Man-in-the-Middle attacks.

#### Vulnerable Code

```rust
#[async_trait]
impl russh::client::Handler for SftpClientHandler {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        _server_public_key: &russh_keys::key::PublicKey,
    ) -> Result<bool, Self::Error> {
        // In production, you'd want to verify the host key
        // For now, accept all keys (like ssh -o StrictHostKeyChecking=no)
        Ok(true)
    }
}
```

#### Impact

- An attacker on the network path can intercept SFTP connections
- Credentials (passwords or private keys) can be captured
- Backup data can be read or modified in transit
- Backups could be redirected to attacker-controlled servers

#### Recommendation

Implement proper host key verification:

1. **Store known host keys:** Add a `known_hosts` table in the database to store verified host keys per SFTP credential.

2. **Trust-on-first-use (TOFU):** On first connection, prompt the user to verify and accept the host key fingerprint.

3. **Strict verification mode:** Compare the presented key against the stored key on subsequent connections.

```rust
async fn check_server_key(
    &mut self,
    server_public_key: &russh_keys::key::PublicKey,
) -> Result<bool, Self::Error> {
    let fingerprint = server_public_key.fingerprint();

    // Check against stored fingerprint
    if let Some(stored) = self.expected_fingerprint.as_ref() {
        if &fingerprint == stored {
            return Ok(true);
        }
        // Key mismatch - potential MITM
        tracing::error!("Host key mismatch! Expected: {}, Got: {}", stored, fingerprint);
        return Ok(false);
    }

    // No stored key - first connection (TOFU mode)
    // Store this key for future verification
    self.new_fingerprint = Some(fingerprint);
    Ok(true)
}
```

---

## Medium Severity Findings

### M-01: JWT Algorithm Not Explicitly Validated

**Severity:** Medium
**Location:** `backend/src/services/auth_service.rs:78-88`
**CWE:** CWE-347 (Improper Verification of Cryptographic Signature)
**CVSS:** 5.3 (Medium)

#### Description

The JWT validation uses `Validation::default()` which does not explicitly enforce the expected signing algorithm. While the current implementation uses HS256 with a symmetric secret, not enforcing the algorithm opens the door to algorithm confusion attacks.

#### Vulnerable Code

```rust
pub fn validate_token(&self, token: &str) -> Result<Claims, AuthError> {
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(self.jwt_secret.as_bytes()),
        &Validation::default(),  // Does not enforce algorithm
    )
    .map(|data| data.claims)
    .map_err(|e| match e.kind() {
        jsonwebtoken::errors::ErrorKind::ExpiredSignature => AuthError::TokenExpired,
        _ => AuthError::InvalidToken,
    })
}
```

#### Impact

- Potential for algorithm confusion attacks if the library behavior changes
- Defense-in-depth violation

#### Recommendation

Explicitly specify the expected algorithm:

```rust
pub fn validate_token(&self, token: &str) -> Result<Claims, AuthError> {
    let mut validation = Validation::new(Algorithm::HS256);
    validation.set_required_spec_claims(&["exp", "iat", "sub"]);

    decode::<Claims>(
        token,
        &DecodingKey::from_secret(self.jwt_secret.as_bytes()),
        &validation,
    )
    .map(|data| data.claims)
    .map_err(|e| match e.kind() {
        jsonwebtoken::errors::ErrorKind::ExpiredSignature => AuthError::TokenExpired,
        _ => AuthError::InvalidToken,
    })
}
```

---

### M-02: IP Spoofing Vulnerability in Rate Limiting

**Severity:** Medium
**Location:** `backend/src/handlers/auth.rs:45-70`
**CWE:** CWE-290 (Authentication Bypass by Spoofing)
**CVSS:** 5.3 (Medium)

#### Description

The `extract_client_ip` function trusts `X-Forwarded-For` and `X-Real-IP` headers without verification. If the application is exposed directly to the internet (not behind a trusted reverse proxy), attackers can bypass rate limiting by spoofing these headers.

#### Vulnerable Code

```rust
pub fn extract_client_ip(headers: &HeaderMap, connect_info: Option<&SocketAddr>) -> String {
    // Try X-Forwarded-For header first (common with reverse proxies)
    if let Some(forwarded) = headers.get("x-forwarded-for") {
        if let Ok(forwarded_str) = forwarded.to_str() {
            // Take the first IP in the chain (original client)
            if let Some(ip) = forwarded_str.split(',').next() {
                return ip.trim().to_string();
            }
        }
    }

    // Try X-Real-IP header
    if let Some(real_ip) = headers.get("x-real-ip") {
        if let Ok(ip) = real_ip.to_str() {
            return ip.trim().to_string();
        }
    }
    // ...
}
```

#### Impact

- Attackers can bypass rate limiting by including fake `X-Forwarded-For` headers
- Brute-force attacks on login become possible
- The rate limiting security control is ineffective without a reverse proxy

#### Recommendation

1. Add a configuration option for trusted proxy mode:

```rust
// config.rs
pub struct Config {
    // ...
    pub trust_proxy_headers: bool,
    pub trusted_proxies: Vec<IpAddr>,
}
```

2. Only trust forwarded headers when behind a configured proxy:

```rust
pub fn extract_client_ip(
    headers: &HeaderMap,
    connect_info: Option<&SocketAddr>,
    config: &Config,
) -> String {
    let direct_ip = connect_info.map(|a| a.ip());

    // Only trust proxy headers if configured and request is from trusted proxy
    if config.trust_proxy_headers {
        if let Some(ip) = direct_ip {
            if config.trusted_proxies.contains(&ip) || config.trusted_proxies.is_empty() {
                // Trust X-Forwarded-For from this proxy
                if let Some(forwarded) = headers.get("x-forwarded-for") {
                    // ... existing logic
                }
            }
        }
    }

    // Fall back to direct connection IP
    direct_ip.map(|ip| ip.to_string()).unwrap_or_else(|| "unknown".to_string())
}
```

---

### M-03: Mount/Unmount Operations Not Restricted to Allowed Paths

**Severity:** Medium
**Location:** `backend/src/handlers/system.rs:69-91`
**CWE:** CWE-22 (Path Traversal)
**CVSS:** 6.5 (Medium)

#### Description

The `mount_drive` and `unmount_drive` handlers do not validate that the `mount_point` parameter is within the configured `allowed_browse_paths`. An authenticated attacker could mount drives to arbitrary locations.

#### Vulnerable Code

```rust
pub async fn mount_drive(
    State(state): State<Arc<AppState>>,
    Json(req): Json<MountRequest>,
) -> impl IntoResponse {
    // No validation of mount_point against allowed_browse_paths!
    match state.mount_service.mount_by_uuid(&req.uuid, &req.mount_point) {
        Ok(_) => Json(json!({"success": true})).into_response(),
        Err(_) => ApiError::new(ErrorCode::MountFailed).into_response(),
    }
}

pub async fn unmount_drive(
    State(state): State<Arc<AppState>>,
    Json(req): Json<UnmountRequest>,
) -> impl IntoResponse {
    // No validation of mount_point!
    match state.mount_service.unmount(&req.mount_point) {
        // ...
    }
}
```

#### Impact

- Authenticated users could mount drives to sensitive locations like `/etc`, `/var`, or `/root`
- Could potentially overwrite system files if the mount succeeds
- Unmount operations could disrupt system functionality

#### Recommendation

Validate mount points against allowed paths:

```rust
pub async fn mount_drive(
    State(state): State<Arc<AppState>>,
    Json(req): Json<MountRequest>,
) -> impl IntoResponse {
    let mount_path = Path::new(&req.mount_point);

    // Validate mount point is within allowed paths
    if !is_path_allowed(mount_path, &state.config.allowed_browse_paths) {
        return ApiError::path_not_allowed().into_response();
    }

    // Also validate the mount point doesn't escape after creation
    // (in case it's a new directory that will be created)

    match state.mount_service.mount_by_uuid(&req.uuid, &req.mount_point) {
        Ok(_) => Json(json!({"success": true})).into_response(),
        Err(_) => ApiError::new(ErrorCode::MountFailed).into_response(),
    }
}
```

---

### M-04: Source Directories Not Validated Against Allowed Paths

**Severity:** Medium
**Location:** `backend/src/handlers/jobs.rs:161-164`
**CWE:** CWE-22 (Path Traversal)
**CVSS:** 4.3 (Medium)

#### Description

Job `source_dirs` are not validated against any allowed path whitelist. While authentication is required, a compromised account could configure backup jobs to read any path accessible to the service user.

#### Vulnerable Code

```rust
// Validate at least one source directory
if req.source_dirs.is_empty() {
    return ApiError::source_dirs_required().into_response();
}
// No path validation - any path is accepted!
```

#### Impact

- Compromised accounts could exfiltrate sensitive system files
- Backup jobs could include `/etc/shadow`, private keys, or database files
- Data could be synced to attacker-controlled destinations

#### Recommendation

Add source directory validation:

```rust
// config.rs
pub struct Config {
    // ...
    pub allowed_source_paths: Vec<String>,  // e.g., ["/home", "/data", "/var/www"]
}

// handlers/jobs.rs
for source_dir in &req.source_dirs {
    let path = Path::new(source_dir);

    // Ensure absolute path
    if !path.is_absolute() {
        return ApiError::invalid_path("Source directories must be absolute paths").into_response();
    }

    // Validate against allowed source paths
    if !is_path_allowed(path, &state.config.allowed_source_paths) {
        return ApiError::path_not_allowed().into_response();
    }
}
```

---

## Low Severity Findings

### L-01: Weak Key Derivation for Credential Encryption

**Severity:** Low
**Location:** `backend/src/services/credential_service.rs:26-36`
**CWE:** CWE-328 (Reversible One-Way Hash)
**CVSS:** 3.7 (Low)

#### Description

The encryption key for credentials is derived using simple SHA-256 with a static salt rather than a proper Key Derivation Function (KDF). While not immediately exploitable, this doesn't provide the security properties of a purpose-built KDF.

#### Vulnerable Code

```rust
pub fn new(jwt_secret: &str, db: SqlitePool) -> Self {
    // Derive a 256-bit key from the JWT secret using SHA-256
    let mut hasher = Sha256::new();
    hasher.update(jwt_secret.as_bytes());
    hasher.update(b"credential_encryption_salt");
    let result = hasher.finalize();

    let mut key = [0u8; 32];
    key.copy_from_slice(&result);

    Self { encryption_key: key, db }
}
```

#### Impact

- If the JWT_SECRET has low entropy, the derived key may be weaker than expected
- No computational cost makes rainbow table attacks easier
- Static salt means all installations with the same JWT_SECRET have the same encryption key

#### Recommendation

Use HKDF (HMAC-based Key Derivation Function):

```rust
use hkdf::Hkdf;
use sha2::Sha256;

pub fn new(jwt_secret: &str, db: SqlitePool) -> Self {
    let hkdf = Hkdf::<Sha256>::new(
        Some(b"dynamight-credential-key-derivation-v1"),  // salt
        jwt_secret.as_bytes()
    );

    let mut key = [0u8; 32];
    hkdf.expand(b"credential-encryption", &mut key)
        .expect("32 bytes is a valid length for HKDF-SHA256");

    Self { encryption_key: key, db }
}
```

---

### L-02: No Password Complexity Requirements

**Severity:** Low
**Location:** `backend/src/handlers/auth.rs:344-346`
**CWE:** CWE-521 (Weak Password Requirements)
**CVSS:** 3.7 (Low)

#### Description

Password validation only enforces a minimum length of 8 characters. Passwords like "11111111" or "password" are accepted.

#### Vulnerable Code

```rust
if req.password.len() < 8 {
    return ApiError::password_too_short().into_response();
}
```

#### Impact

- Users may choose weak, easily guessable passwords
- Increases success rate of credential stuffing attacks
- Common passwords may be cracked quickly despite Argon2 hashing

#### Recommendation

Add complexity requirements or use entropy-based validation:

```rust
fn validate_password_strength(password: &str) -> Result<(), &'static str> {
    if password.len() < 8 {
        return Err("Password must be at least 8 characters");
    }

    let has_uppercase = password.chars().any(|c| c.is_uppercase());
    let has_lowercase = password.chars().any(|c| c.is_lowercase());
    let has_digit = password.chars().any(|c| c.is_ascii_digit());
    let has_special = password.chars().any(|c| !c.is_alphanumeric());

    let complexity_score = [has_uppercase, has_lowercase, has_digit, has_special]
        .iter()
        .filter(|&&b| b)
        .count();

    if complexity_score < 3 {
        return Err("Password must contain at least 3 of: uppercase, lowercase, digit, special character");
    }

    Ok(())
}
```

Alternatively, integrate a library like `zxcvbn` for more sophisticated password strength estimation.

---

### L-03: Path Traversal Edge Case in create_directory

**Severity:** Low
**Location:** `backend/src/handlers/system.rs:161-163`
**CWE:** CWE-22 (Path Traversal)
**CVSS:** 3.1 (Low)

#### Description

The `create_directory` handler checks for ".." using a simple string contains check before proper path validation. For non-existent paths, `is_path_allowed` falls back to non-canonicalized path checking.

#### Vulnerable Code

```rust
pub async fn create_directory(
    State(state): State<Arc<AppState>>,
    Json(req): Json<MkdirRequest>,
) -> impl IntoResponse {
    let path = Path::new(&req.path);

    // Security: don't allow path traversal
    let path_str = req.path.as_str();
    if path_str.contains("..") {
        return ApiError::new(ErrorCode::PathTraversalNotAllowed).into_response();
    }
    // ...
}
```

And in `is_path_allowed`:

```rust
} else {
    // For non-existent paths, check the string representation
    // This is safe because we'll canonicalize during actual operations
    path.to_path_buf()
};
```

#### Impact

- Low risk due to multiple layers of validation
- Potential for bypass with unusual path representations
- Defense-in-depth concern

#### Recommendation

Use proper path normalization before validation:

```rust
use std::path::PathBuf;

fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::ParentDir => {
                normalized.pop();
            }
            std::path::Component::Normal(c) => {
                normalized.push(c);
            }
            std::path::Component::RootDir => {
                normalized.push("/");
            }
            _ => {}
        }
    }
    normalized
}
```

---

### L-04: UUID Format Not Strictly Validated for Mount Operations

**Severity:** Low
**Location:** `backend/src/services/mount_service.rs:70-83`
**CWE:** CWE-20 (Improper Input Validation)
**CVSS:** 2.0 (Low)

#### Description

The `mount_by_uuid` function passes the UUID directly to the `mount` command without validating its format. While command injection is prevented by using argument arrays, invalid UUIDs could cause confusing error messages.

#### Vulnerable Code

```rust
pub fn mount_by_uuid(&self, uuid: &str, mount_point: &str) -> Result<(), MountError> {
    // No UUID format validation
    let status = Command::new("mount")
        .args(["-U", uuid, mount_point])
        .status()?;
    // ...
}
```

#### Impact

- Minimal security impact due to use of argument arrays
- Invalid UUIDs may cause unclear error messages
- Defense-in-depth improvement opportunity

#### Recommendation

Validate UUID format before use:

```rust
fn is_valid_uuid(uuid: &str) -> bool {
    // Standard UUID format: xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx
    // Or short format: xxxx-xxxx (common for FAT partitions)
    let uuid_regex = regex::Regex::new(
        r"^([0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}|[0-9a-fA-F]{4}-[0-9a-fA-F]{4})$"
    ).unwrap();
    uuid_regex.is_match(uuid)
}

pub fn mount_by_uuid(&self, uuid: &str, mount_point: &str) -> Result<(), MountError> {
    if !is_valid_uuid(uuid) {
        return Err(MountError::DeviceNotFound(format!("Invalid UUID format: {}", uuid)));
    }
    // ...
}
```

---

## Informational Findings

### I-01: JWT Token Exposed in WebSocket URLs

**Severity:** Informational
**Location:** `backend/src/handlers/websocket.rs`

#### Description

JWT tokens are passed as URL query parameters for WebSocket authentication (e.g., `/api/ws/logs/123?token=eyJ...`). While necessary for WebSocket authentication, these tokens may appear in:
- Server access logs
- Browser history
- Referrer headers (if links are clicked while on the page)

#### Recommendation

1. Ensure server access logs are configured to redact query parameters
2. Consider implementing a short-lived token exchange mechanism:
   - Client requests a one-time WebSocket token via POST
   - Server returns a short-lived (30 second) token
   - Client uses that token for WebSocket connection
   - Token is invalidated after use

---

### I-02: TOTP Secrets Stored Unencrypted

**Severity:** Informational
**Location:** Database schema, `handlers/totp.rs:129-133`

#### Description

TOTP secrets are stored in plaintext in the `users` table, while other credentials are encrypted with AES-256-GCM. This is common practice (Google Authenticator, etc.) but represents an inconsistency in the security model.

#### Recommendation

Consider encrypting TOTP secrets using the same mechanism as credentials for consistency:

```rust
// When enabling TOTP
let encrypted_secret = credential_service.encrypt_totp_secret(&req.secret)?;

sqlx::query(
    "UPDATE users SET totp_secret = ?, totp_enabled = 1 WHERE id = ?"
)
.bind(&encrypted_secret)
.bind(user.id)
.execute(&state.db)
.await?;
```

---

## Security Features Well Implemented

The following security controls are properly implemented and deserve recognition:

### Authentication & Session Management

| Feature | Implementation | Status |
|---------|----------------|--------|
| Password Hashing | Argon2id with secure defaults | Excellent |
| Session Tokens | JWT with 24-hour expiry | Good |
| Cookie Security | HttpOnly, SameSite=Strict | Excellent |
| Secure Flag | Configurable via SECURE_COOKIES | Good |

### Two-Factor Authentication

| Feature | Implementation | Status |
|---------|----------------|--------|
| TOTP Support | RFC 6238 compliant | Excellent |
| Recovery Codes | Argon2 hashed, single-use | Excellent |
| Pending Session Timeout | 5-minute expiry | Good |

### Rate Limiting

| Feature | Implementation | Status |
|---------|----------------|--------|
| Per-IP Tracking | DashMap with cleanup | Good |
| Exponential Backoff | Configurable parameters | Excellent |
| Applied Endpoints | Login, TOTP, Recovery | Good |

### Credential Management

| Feature | Implementation | Status |
|---------|----------------|--------|
| Encryption Algorithm | AES-256-GCM | Excellent |
| Nonce Generation | Random 12-byte per encryption | Excellent |
| Secrets Never Returned | API responses exclude data | Excellent |

### Input Validation

| Feature | Implementation | Status |
|---------|----------------|--------|
| SQL Injection | Parameterized queries (sqlx) | Excellent |
| Rsync Exclude Patterns | Shell character validation | Excellent |
| Path Traversal (Browse) | Canonicalization + whitelist | Good |
| Display Fields | Length limits + control char filtering | Good |

### Command Execution

| Feature | Implementation | Status |
|---------|----------------|--------|
| Shell Injection | Argument arrays, not shell strings | Excellent |
| Process Isolation | process_group(0) for cleanup | Good |

---

## Recommendations Summary

### Immediate Actions (Before Production)

1. **[H-01]** Implement SFTP host key verification
2. **[M-01]** Explicitly validate JWT algorithm
3. **[M-02]** Add trusted proxy configuration for rate limiting

### Short-Term Improvements

4. **[M-03]** Validate mount points against allowed paths
5. **[M-04]** Add source directory path validation
6. **[L-01]** Upgrade to HKDF for key derivation

### Long-Term Enhancements

7. **[L-02]** Implement password complexity requirements
8. **[L-03]** Improve path normalization for create_directory
9. **[I-01]** Consider short-lived WebSocket token exchange
10. **[I-02]** Encrypt TOTP secrets for consistency

---

## Files Reviewed

### Backend (Rust)

- `backend/src/main.rs` - Application entry point, router setup
- `backend/src/config.rs` - Environment configuration
- `backend/src/middleware.rs` - Authentication middleware
- `backend/src/services/auth_service.rs` - Password hashing, JWT
- `backend/src/services/totp_service.rs` - TOTP implementation
- `backend/src/services/credential_service.rs` - Credential encryption
- `backend/src/services/rate_limit_service.rs` - Rate limiting
- `backend/src/services/mount_service.rs` - Mount operations
- `backend/src/services/backup_service.rs` - Job execution
- `backend/src/services/providers/rsync.rs` - Rsync provider
- `backend/src/services/providers/sftp.rs` - SFTP provider
- `backend/src/handlers/auth.rs` - Auth endpoints
- `backend/src/handlers/totp.rs` - 2FA endpoints
- `backend/src/handlers/jobs.rs` - Job CRUD
- `backend/src/handlers/system.rs` - System operations
- `backend/src/handlers/websocket.rs` - WebSocket handlers
- `backend/src/handlers/providers.rs` - Provider endpoints
- `backend/src/handlers/credentials.rs` - Credential endpoints

### Frontend (Svelte/TypeScript)

- `frontend/src/lib/api.ts` - API client

### Documentation

- `docs/project-description.md` - Architecture documentation
- `docs/API.md` - API reference

---

## Audit Methodology

This audit was conducted through manual source code review focusing on:

1. **Authentication flows** - Login, logout, session management
2. **Authorization** - Route protection, middleware
3. **Input validation** - All user-controllable inputs
4. **Cryptographic implementations** - Hashing, encryption, tokens
5. **Command execution** - Shell commands, external processes
6. **File system operations** - Path handling, traversal prevention
7. **Third-party integrations** - SFTP, S3, WebDAV connections
8. **Error handling** - Information disclosure in errors
9. **Configuration** - Secure defaults, environment variables

---

## Disclaimer

This security audit represents a point-in-time assessment based on the code reviewed. New vulnerabilities may be introduced by future changes, and this audit does not guarantee the absence of all security issues. Regular security reviews and penetration testing are recommended.
