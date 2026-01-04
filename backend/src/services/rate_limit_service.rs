use chrono::{DateTime, Duration, Utc};
use dashmap::DashMap;
use std::sync::Arc;

#[derive(Clone)]
pub struct RateLimitConfig {
    pub max_attempts: u32,
    pub window_secs: u64,
    pub lockout_secs: u64,
    pub max_lockout_secs: u64,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            max_attempts: 5,
            window_secs: 60,
            lockout_secs: 60,
            max_lockout_secs: 3600,
        }
    }
}

#[derive(Clone)]
struct RateLimitEntry {
    attempts: u32,
    window_start: DateTime<Utc>,
    locked_until: Option<DateTime<Utc>>,
}

pub struct RateLimitError {
    pub retry_after_secs: i64,
}

pub struct RateLimitService {
    config: RateLimitConfig,
    entries: DashMap<String, RateLimitEntry>,
}

impl RateLimitService {
    pub fn new(config: RateLimitConfig) -> Arc<Self> {
        Arc::new(Self {
            config,
            entries: DashMap::new(),
        })
    }

    /// Check if a request from the given IP is allowed.
    /// Returns Ok(()) if allowed, Err(RateLimitError) if rate limited.
    pub fn check_rate_limit(&self, ip: &str) -> Result<(), RateLimitError> {
        let now = Utc::now();

        if let Some(entry) = self.entries.get(ip) {
            // Check if currently locked out
            if let Some(locked_until) = entry.locked_until {
                if now < locked_until {
                    let retry_after = (locked_until - now).num_seconds();
                    return Err(RateLimitError {
                        retry_after_secs: retry_after.max(1),
                    });
                }
            }
        }

        Ok(())
    }

    /// Record a failed authentication attempt from the given IP.
    /// Applies exponential backoff when max attempts are exceeded.
    pub fn record_failure(&self, ip: &str) {
        let now = Utc::now();
        let window_duration = Duration::seconds(self.config.window_secs as i64);

        let mut entry = self
            .entries
            .entry(ip.to_string())
            .or_insert_with(|| RateLimitEntry {
                attempts: 0,
                window_start: now,
                locked_until: None,
            });

        // Reset window if it has expired and we're not locked out
        if now - entry.window_start > window_duration && entry.locked_until.is_none() {
            entry.attempts = 0;
            entry.window_start = now;
        }

        entry.attempts += 1;

        // Apply lockout if max attempts exceeded
        if entry.attempts >= self.config.max_attempts {
            // Calculate exponential backoff
            let excess_attempts = entry.attempts - self.config.max_attempts;
            let lockout_multiplier = 2_u64.saturating_pow(excess_attempts);
            let lockout_secs = (self.config.lockout_secs * lockout_multiplier)
                .min(self.config.max_lockout_secs);

            entry.locked_until = Some(now + Duration::seconds(lockout_secs as i64));

            tracing::warn!(
                "Rate limit lockout applied for IP {}: {} seconds (attempt {})",
                ip,
                lockout_secs,
                entry.attempts
            );
        }
    }

    /// Record a successful authentication, clearing rate limit state for the IP.
    pub fn record_success(&self, ip: &str) {
        self.entries.remove(ip);
    }

    /// Clean up expired entries to prevent memory growth.
    /// Should be called periodically (e.g., every minute).
    pub fn cleanup(&self) {
        let now = Utc::now();
        let window_duration = Duration::seconds(self.config.window_secs as i64);

        self.entries.retain(|_, entry| {
            // Keep if locked and lockout hasn't expired
            if let Some(locked_until) = entry.locked_until {
                if now < locked_until {
                    return true;
                }
            }

            // Keep if within the tracking window
            now - entry.window_start <= window_duration
        });
    }

    /// Start a background task that periodically cleans up expired entries.
    pub fn start_cleanup_task(service: Arc<Self>) {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
            loop {
                interval.tick().await;
                service.cleanup();
            }
        });
    }
}
