# Dynamight Optimization Plan

> Comprehensive analysis and optimization roadmap for the Dynamight backup application.
> Generated from in-depth codebase review covering backend (Rust/Axum), frontend (Svelte 5/TypeScript), security, and resilience.

---

## Table of Contents

1. [Executive Summary](#executive-summary)
2. [Dead Code Analysis](#dead-code-analysis)
3. [Code Duplication](#code-duplication)
4. [Performance Optimizations](#performance-optimizations)
5. [Security Vulnerabilities](#security-vulnerabilities)
6. [Resilience & Error Handling](#resilience--error-handling)
7. [Architectural Improvements](#architectural-improvements)
8. [Implementation Priorities](#implementation-priorities)

---

## Executive Summary

### Current State
- **Compiler Status**: Both `cargo check` and `npm run check` pass with zero warnings
- **Clippy Status**: Passes cleanly with zero warnings
- **Overall Code Quality**: Well-structured, with quick wins already implemented

### Completed Optimizations (Phase 1)
- ✅ Token extraction consolidated into `extractors.rs` with shared utility function
- ✅ `AuthUser` and `AuthClaims` extractors now actively used (no longer dead code)
- ✅ O(n²) → O(n) performance fix in `list_jobs` using HashMap
- ✅ ~85 lines of duplicated code removed across auth handlers

### Completed Optimizations (Phase 2 - Security)
- ✅ IP spoofing vulnerability fixed with trusted proxy configuration and CIDR support
- ✅ Request body size limits added via `RequestBodyLimitLayer` (configurable, default 10MB)
- ✅ All critical `.unwrap()` calls replaced with `.expect()` or proper error handling

### Key Findings

| Category | Issues Found | Status |
|----------|--------------|--------|
| Dead Code | 8-10 items | 3 resolved (extractors), 1 false positive |
| Code Duplication | ~800+ lines | ~85 lines resolved (token extraction) |
| Performance Issues | 12 items | 1 resolved (O(n) lookup) |
| Security Vulnerabilities | 9 critical items | 2 resolved (IP spoofing, request limits) |
| Resilience Gaps | 7 items | 1 resolved (unwrap panics) |
| Architectural Issues | 5 items | Auth flow consolidated |

### Estimated Effort
- **Quick Wins**: 2-3 days
- **Medium Effort**: 1-2 weeks
- **Full Optimization**: 3-4 weeks

---

## Dead Code Analysis

### Backend (Rust)

#### 1. ~~Unused Extractors~~ ✅ RESOLVED
**Files**: `backend/src/extractors.rs`

**Status**: The `AuthUser` and `AuthClaims` extractors are now actively used:
- `AuthUser` used in `auth.rs` for `me()` and `change_password()` handlers
- `AuthClaims` used in `settings.rs` for `update_settings()` handler
- `extract_token_from_headers()` utility added and used by `middleware.rs` and `get_token()` handler

#### 2. Suppressed Error Types
**File**: `backend/src/errors.rs:12, 102, 110`

The `ErrorCode` enum and `ApiError` struct are marked with `#[allow(dead_code)]` but are actually used. The issue is:
- Not all `ErrorCode` variants are used yet (prepared for gradual migration)
- Some `ApiError` convenience methods are unused

**Recommendation**: Complete the migration to structured error codes across all handlers.

#### 3. Unused Provider Utilities
**File**: `backend/src/services/providers/mod.rs:41-47, 133-136`

```rust
// SyncProgress - prepared for progress callbacks but never used
#[allow(dead_code)]
pub struct SyncProgress {
    pub current_file: Option<String>,
    pub bytes_transferred: i64,
    // ...
}

// log_debug method never called
#[allow(dead_code)]
pub async fn log_debug(&self, message: &str, source: &str) { ... }
```

#### 4. SFTP Provider Dead Code
**File**: `backend/src/services/providers/sftp.rs:503-543`

- `HostKeyVerification` enum variants defined but only partially used
- `get_captured_fingerprint()` and `get_verification_result()` methods never called

### Frontend (TypeScript/Svelte)

#### 1. ~~Unused Store Function~~ (False Positive)
**File**: `frontend/src/lib/stores/fileBrowser.ts`

**Status**: Upon investigation, `clearError()` is exported from the store and used by `authStore` to clear file browser errors on logout. This is NOT dead code.

#### 2. Unused Types
**File**: `frontend/src/lib/types.ts`

- `OAuthCredentialData` (line 174-180): Only used in union type, overly generic
- `SpaceCheckMode` (line 94): Only `'warn'` mode actively used; `'fail'` and `'none'` are dead paths

#### 3. Unused Component Callback
**File**: `frontend/src/components/jobs/CredentialSelector.svelte:16`

```typescript
// onCredentialsChange prop accepted but never invoked
let { onCredentialsChange } = $props<{ onCredentialsChange?: () => void }>();
```

---

## Code Duplication

### ~~Backend: Token Extraction~~ ✅ RESOLVED

**Status**: Token extraction has been consolidated into a single location.

**Changes Made**:
| File | Before | After |
|------|--------|-------|
| `extractors.rs` | Unused extractors | Central `extract_token_from_headers()` utility, `AuthUser` and `AuthClaims` extractors now used |
| `handlers/auth.rs` | 3 manual extractions (~60 lines) | Uses `AuthUser` extractor for `me()`, `change_password()`; uses utility for `get_token()` |
| `handlers/settings.rs` | Manual extraction (~23 lines) | Uses `AuthClaims` extractor |
| `middleware.rs` | Manual extraction (~23 lines) | Uses shared `extract_token_from_headers()` utility |
| `handlers/totp.rs` | Has own helper | Unchanged (uses TOTP-specific session pattern) |

**Result**: ~85 lines of duplicated code removed, single source of truth for token extraction.

### Backend: Re-query After Insert

**Files**: `handlers/jobs.rs:239-243, 391-395, 643-647`

After INSERT/UPDATE, the code immediately re-fetches the same record:

```rust
// Insert
let result = sqlx::query("INSERT INTO jobs ...").execute(&state.db).await;
let id = result.last_insert_rowid();

// Unnecessary re-query
let job: Option<Job> = sqlx::query_as("SELECT * FROM jobs WHERE id = ?")
    .bind(id)
    .fetch_optional(&state.db)
    .await;
```

**Solution**: Use SQLite's RETURNING clause or restructure to avoid the re-query.

### Frontend: Modal/Dialog Infrastructure (~400 lines)

Three components implement nearly identical modal structures:

| Component | Lines | Duplicate Elements |
|-----------|-------|-------------------|
| `PathSelector.svelte` | 228-412 | Header, navigation, listing, footer |
| `SinglePathSelector.svelte` | 240-385 | Same structure |
| `BrowseModal.svelte` | Similar | Same structure |

**Solution**: Create a `GenericBrowseModal.svelte` component with slots for customization.

### Frontend: Provider Destination Boilerplate (~250 lines)

All 5 provider destination components share:
- CredentialSelector with identical props
- TestConnection with identical props
- Label + HelpTooltip pattern
- Container styling

**Files**:
- `GoogleDriveDestination.svelte` (89 lines)
- `OneDriveDestination.svelte` (80 lines)
- `S3Destination.svelte` (135 lines)
- `SftpDestination.svelte` (121 lines)
- `WebDavDestination.svelte` (86 lines)

**Solution**: Extract common wrapper component `ProviderDestinationWrapper.svelte`.

### Frontend: Toggle Switch (~80 lines)

Custom toggle switches duplicated in:
- `LocalDestination.svelte:138-183`
- `SftpDestination.svelte:91-114`
- Inline in other components

**Solution**: Create reusable `ToggleSwitch.svelte` component.

---

## Performance Optimizations

### Backend

#### 1. ~~O(n) Lookup in list_jobs~~ ✅ RESOLVED
**File**: `handlers/jobs.rs`

**Status**: Fixed by converting to HashMap for O(1) lookups.

**Implementation**:
```rust
// Convert to HashMap for O(1) lookups instead of O(n) linear scan per job
let last_runs_map: HashMap<i64, (String, Option<chrono::DateTime<Utc>>)> = last_runs
    .into_iter()
    .map(|(job_id, status, run_at)| (job_id, (status, run_at)))
    .collect();

let response: Vec<JobResponse> = jobs
    .into_iter()
    .map(|job| {
        let (status, run_at) = last_runs_map
            .get(&job.id)
            .map(|(s, t)| (Some(s.clone()), *t))
            .unwrap_or((None, None));
        JobResponse::from(job).with_run_status(status, run_at)
    })
    .collect();
```

**Result**: O(n²) → O(n) complexity improvement for job listing.

#### 2. Sequential Queries in Clone Name Generation
**File**: `handlers/jobs.rs:658-692`

Up to 100 sequential database queries to find unique name:

```rust
loop {
    let exists = sqlx::query_as("SELECT id FROM jobs WHERE name = ?")
        .bind(&candidate)
        .fetch_optional(db).await;
    if exists.is_none() { return candidate; }
    counter += 1;
}
```

**Fix**: Use single query with LIKE pattern or generate timestamp-based name.

#### 3. Missing Connection Pool Optimization
**File**: `main.rs:99-114`

```rust
let db = SqlitePoolOptions::new()
    .max_connections(10)  // Good
    // Missing: min_connections, idle_timeout, after_connect
```

**Recommendation**: Configure `min_connections(2)` for faster initial queries.

### Frontend

#### 1. Unnecessary $effect Hooks (14 instances)
**File**: `routes/JobDetail.svelte:117-133`

Two sequential effects that could be combined:
```typescript
$effect(() => {
    if (destination.type !== destinationType) {
        // First effect
    }
});

$effect(() => {
    loadCapabilities(destinationType);  // Second effect
});
```

**Fix**: Combine into single effect with batched updates.

#### 2. TestConnection Unnecessary Reactivity
**File**: `components/jobs/TestConnection.svelte:58-66`

```typescript
$effect(() => {
    const _dest = destination;  // Accessed but not used
    const _cred = credentialId; // Just for reactivity tracking
    // Reset result when inputs change
    result = null;
});
```

**Fix**: Use proper dependency tracking without dummy variables.

#### 3. Missing Memoization
**File**: `components/jobs/CredentialSelector.svelte:179`

`filteredCredentials` recalculates on every render even when inputs unchanged.

**Fix**: Use derived state with proper dependency tracking.

#### 4. Large Components Need Splitting

| Component | Lines | Recommendation |
|-----------|-------|----------------|
| `History.svelte` | 674 | Extract pagination, filtering, real-time components |
| `SettingsModal.svelte` | 615 | Extract credential manager, TOTP setup |
| `CredentialEditModal.svelte` | 608 | Extract provider-specific forms |
| `JobDetail.svelte` | 750+ | Extract sections into sub-components |

#### 5. Missing Code Splitting

Provider destination components should be lazy-loaded based on selection:
```typescript
// Instead of importing all at once
const ProviderComponent = await import(`./providers/${type}Destination.svelte`);
```

---

## Security Vulnerabilities

### CRITICAL

#### 1. ~~IP Spoofing via X-Forwarded-For (Rate Limiting Bypass)~~ ✅ RESOLVED
**File**: `handlers/auth.rs:44-170`

**Status**: Fixed with comprehensive trusted proxy support.

**Implementation**:
- Added `TRUSTED_PROXIES` config option (comma-separated IPs or CIDRs)
- `extract_client_ip()` now takes trusted proxies as parameter
- Only trusts X-Forwarded-For when direct connection is from trusted proxy
- Uses rightmost untrusted IP from X-Forwarded-For chain
- Supports both exact IP and CIDR notation (e.g., `10.0.0.0/8`, `192.168.1.0/24`)
- When no trusted proxies configured, only direct connection IP is used (safest default)

**New config options**:
- `TRUSTED_PROXIES`: Comma-separated list of trusted proxy IPs/CIDRs

#### 2. ~~Missing Request Size Limits~~ ✅ RESOLVED
**Files**: `main.rs`, `config.rs`, `Cargo.toml`

**Status**: Fixed with configurable request body size limits.

**Implementation**:
- Added `tower-http/limit` feature to Cargo.toml
- Added `RequestBodyLimitLayer` to API routes in `main.rs`
- Default limit: 10MB (configurable via `MAX_REQUEST_BODY_SIZE` env var)
- Protects against DoS via oversized JSON payloads

### HIGH

#### 3. TOCTOU in Path Validation
**File**: `handlers/system.rs:121-134`

```rust
let canonical = std::fs::canonicalize(path)?;  // Time of check
// ... symlink could change here ...
if !is_path_allowed(&canonical, &allowed_paths) {  // Time of use
```

**Fix**: Use atomic operations or hold locks during validation.

#### 4. Weak Encryption Key Derivation
**File**: `services/credential_service.rs:26-37`

```rust
// Using SHA-256 with static salt - should use proper KDF
hasher.update(jwt_secret.as_bytes());
hasher.update(b"credential_encryption_salt");  // Static!
```

**Fix**: Use Argon2 or PBKDF2 for key derivation with random per-credential salt.

#### 5. No Token Revocation Mechanism
**File**: `services/auth_service.rs:59-76`

Tokens valid for 24 hours with no way to revoke if compromised.

**Fix**: Implement token blacklist or use short-lived tokens with refresh.

### MEDIUM

#### 6. WebDAV Credentials Over HTTP
**File**: `services/providers/webdav.rs:123-125`

No validation that WebDAV URL uses HTTPS before sending Basic Auth.

**Fix**: Require `https://` scheme or warn user.

#### 7. No Rate Limiting on Setup Endpoint
**File**: `handlers/auth.rs:324`

The `/auth/setup` endpoint allows unlimited attempts.

**Fix**: Add rate limiting or lockout after N failed attempts.

#### 8. Missing HSTS and CSP Headers
**File**: `main.rs` (missing)

No security headers enforced.

**Fix**: Add tower middleware for security headers:
```rust
.layer(SetResponseHeaderLayer::if_not_present(
    header::STRICT_TRANSPORT_SECURITY,
    HeaderValue::from_static("max-age=31536000; includeSubDomains")
))
```

#### 9. S3 Error Messages May Leak Credentials
**File**: `services/providers/s3.rs:137-150`

Error messages could expose sensitive information in logs.

**Fix**: Sanitize error messages before logging.

---

## Resilience & Error Handling

### Missing Timeouts

#### 1. Rsync Process (CRITICAL)
**File**: `services/providers/rsync.rs:650-793`

No timeout on rsync process - could hang indefinitely.

**Fix**:
```rust
let result = tokio::time::timeout(
    Duration::from_secs(3600),  // 1 hour max
    child.wait()
).await;
```

#### 2. HTTP Requests
**File**: `services/providers/webdav.rs:18`

300-second timeout is excessive.

**Fix**: Use 30-60 second timeout for initial connection, longer for transfers.

### Missing Graceful Shutdown
**File**: `main.rs`

No signal handling for SIGTERM. Running jobs terminate abruptly.

**Fix**:
```rust
let shutdown_signal = async {
    tokio::signal::ctrl_c().await.ok();
};
axum::serve(listener, app)
    .with_graceful_shutdown(shutdown_signal)
    .await?;
```

### Silent Error Swallowing
**File**: `services/backup_service.rs:337-346`

Log insertion errors silently ignored:
```rust
let _ = sqlx::query("INSERT INTO log_entries ...").execute(&self.logs_db).await;
```

**Fix**: At minimum, log the error with `tracing::warn!`.

### ~~Unwrap Panics~~ ✅ RESOLVED

**Status**: All critical `.unwrap()` calls have been replaced with `.expect()` or proper error handling.

**Changes Made**:

| File | Lines | Fix Applied |
|------|-------|-------------|
| `handlers/jobs.rs` | 40-42, 60-62 | `.expect("DANGEROUS_CHARS regex pattern is invalid")`, `.expect("SAFE_PATTERN regex pattern is invalid")` |
| `providers/sftp.rs` | 455 | `path.to_str().ok_or_else(\|\| ProviderError::TransferError(...))` |
| `providers/webdav.rs` | 123-126, 191-193, 243-245, 310-312, 318-320 | `.expect("PROPFIND/MKCOL is a valid HTTP method")` + proper error handling for path conversion |
| `providers/onedrive.rs` | 362-364, 377-380, 476-478 | `.expect("...must succeed after contains check")` + proper error handling for path conversion |
| `providers/googledrive.rs` | 465-467 | `path.to_str().ok_or_else(\|\| ProviderError::TransferError(...))` |

**Result**: Better error context for debugging, no silent panics on invalid UTF-8 paths.

### Race Conditions

#### Job Tracking Race
**File**: `services/backup_service.rs:165-183`

```rust
self.running_jobs.write().await.insert(job.id);  // In-memory update
let result = self.do_execute(job, run_id, schedule_id).await;  // Work
self.running_jobs.write().await.remove(&job.id);  // Remove
// Database not updated atomically with in-memory state
```

**Fix**: Use database as source of truth, with in-memory cache for optimization only.

---

## Architectural Improvements

### 1. Consolidate Authentication Flow

**Current State**: 3 different token extraction methods
- Manual parsing in handlers
- Middleware-based
- Extractor-based (unused)

**Recommendation**: Choose one approach:
- **Option A**: Use middleware exclusively, pass user info via request extensions
- **Option B**: Use `AuthUser` extractor in all handlers

### 2. Extract Repository Layer

**Current State**: SQL queries scattered throughout handlers

**Recommendation**: Create repository modules:
```
services/
  repositories/
    job_repository.rs
    schedule_repository.rs
    credential_repository.rs
```

### 3. Provider Factory Improvements

**Current State**: Each provider instantiated inline

**Recommendation**: Use builder pattern with configuration:
```rust
let provider = ProviderFactory::new()
    .with_timeout(Duration::from_secs(300))
    .with_retry_policy(RetryPolicy::exponential(3))
    .build(destination_type)?;
```

### 4. Frontend State Management

**Current State**: Mix of Svelte 5 runes and custom stores

**Recommendation**: Standardize on either:
- Full Svelte 5 runes with `$state` and `$derived`
- Or consistent store factory pattern

### 5. Error Handling Standardization

**Current State**: Mix of `Result`, `ApiError`, `anyhow::Error`

**Recommendation**: Define clear error boundaries:
- Handlers return `ApiError`
- Services return `Result<T, ServiceError>`
- Providers return `Result<T, ProviderError>`

---

## Implementation Priorities

### Phase 1: Quick Wins ✅ COMPLETED

| Task | Impact | Effort | Status |
|------|--------|--------|--------|
| Extract token helper function | High | Low | ✅ Done - `extract_token_from_headers()` in `extractors.rs` |
| Refactor handlers to use extractors | High | Low | ✅ Done - `AuthUser` and `AuthClaims` now used |
| Fix O(n) lookup in list_jobs | Medium | Low | ✅ Done - HashMap for O(1) lookups |
| Remove unused `clearError()` | Low | Trivial | ⏸️ Skipped - confirmed NOT dead code (used by authStore) |
| Combine duplicate $effects | Low | Low | Pending |
| Add `.expect()` context to unwraps | Medium | Low | ✅ Done - all critical unwraps now have context |

**Note**: `clearError()` in `fileBrowser.ts` was confirmed to be used by `authStore` and is NOT dead code.

### Phase 2: Security Hardening (3-5 days) - PARTIALLY COMPLETED

| Task | Impact | Effort | Status |
|------|--------|--------|--------|
| Fix IP spoofing vulnerability | Critical | Medium | ✅ Done - trusted proxy config with CIDR support |
| Add request size limits | High | Low | ✅ Done - `RequestBodyLimitLayer` added |
| Add operation timeouts | High | Medium | Pending |
| Fix TOCTOU in path validation | Medium | Medium | Pending |
| Add HSTS/CSP headers | Medium | Low | Pending |

### Phase 3: Code Consolidation (1 week)

| Task | Impact | Effort | Files |
|------|--------|--------|-------|
| Create GenericBrowseModal | High | Medium | `PathSelector.svelte`, etc. |
| Create ToggleSwitch component | Medium | Low | Multiple |
| Extract provider destination wrapper | Medium | Medium | Provider components |
| Consolidate authentication flow | High | High | Multiple |
| Extract repository layer | Medium | High | Handlers, new modules |

### Phase 4: Advanced Optimizations (1-2 weeks)

| Task | Impact | Effort | Files |
|------|--------|--------|-------|
| Implement graceful shutdown | High | Medium | `main.rs` |
| Add lazy loading for providers | Medium | Medium | Frontend |
| Implement token revocation | High | High | Auth system |
| Upgrade encryption key derivation | High | Medium | `credential_service.rs` |
| Split large components | Medium | High | `History.svelte`, etc. |

---

## Metrics for Success

After implementing this plan, the codebase should achieve:

- [x] Zero `#[allow(dead_code)]` suppressions for extractors (AuthUser, AuthClaims now used)
- [x] Token extraction in single location (`extractors.rs`)
- [ ] All operations have explicit timeouts
- [x] Request size limits on all endpoints (10MB default, configurable via `MAX_REQUEST_BODY_SIZE`)
- [x] No `.unwrap()` without `.expect()` context (all critical unwraps fixed)
- [ ] Security headers on all responses
- [x] IP spoofing vulnerability fixed (trusted proxy support with CIDR)
- [ ] ~800 lines of duplicate code eliminated (~85 done via token extraction)
- [ ] Modal components reduced to single reusable implementation
- [ ] All large components under 400 lines

---

## Appendix: File Reference

### Backend Files with Issues
- `backend/src/handlers/auth.rs` - ~~Token extraction~~ ✅, ~~IP spoofing~~ ✅
- `backend/src/handlers/jobs.rs` - ~~Performance~~ ✅, ~~Unwrap panics~~ ✅, Query patterns (pending)
- `backend/src/handlers/system.rs` - TOCTOU vulnerability
- `backend/src/middleware.rs` - ~~Token extraction duplication~~ ✅
- `backend/src/extractors.rs` - ~~Unused code~~ ✅ (now central token utility)
- `backend/src/errors.rs` - Dead code markers
- `backend/src/services/backup_service.rs` - Race conditions
- `backend/src/services/credential_service.rs` - Weak encryption
- `backend/src/services/providers/*.rs` - ~~Unwrap panics~~ ✅, timeouts (pending)
- `backend/src/main.rs` - ~~Request size limits~~ ✅
- `backend/src/config.rs` - ~~Trusted proxies config~~ ✅, ~~Request size config~~ ✅

### Frontend Files with Issues
- `frontend/src/lib/stores/fileBrowser.ts` - ~~Dead code~~ (false positive - `clearError()` is used by authStore)
- `frontend/src/lib/types.ts` - Unused types
- `frontend/src/components/jobs/PathSelector.svelte` - Duplication
- `frontend/src/components/jobs/SinglePathSelector.svelte` - Duplication
- `frontend/src/components/jobs/providers/*.svelte` - Boilerplate
- `frontend/src/routes/JobDetail.svelte` - Multiple $effects
- `frontend/src/routes/History.svelte` - Large component
