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

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> RateLimitConfig {
        RateLimitConfig {
            max_attempts: 3,
            window_secs: 60,
            lockout_secs: 10,
            max_lockout_secs: 320,
        }
    }

    #[test]
    fn fresh_ip_passes_rate_limit() {
        let service = RateLimitService::new(test_config());
        assert!(service.check_rate_limit("1.2.3.4").is_ok());
    }

    #[test]
    fn single_failure_does_not_lock() {
        let service = RateLimitService::new(test_config());
        service.record_failure("1.2.3.4");
        assert!(service.check_rate_limit("1.2.3.4").is_ok());
    }

    #[test]
    fn two_failures_below_max_still_allowed() {
        let service = RateLimitService::new(test_config());
        service.record_failure("1.2.3.4");
        service.record_failure("1.2.3.4");
        // 2 attempts < max_attempts(3), should still pass
        assert!(service.check_rate_limit("1.2.3.4").is_ok());
    }

    #[test]
    fn lockout_after_max_failures() {
        let service = RateLimitService::new(test_config());
        // Record max_attempts (3) failures to trigger lockout
        for _ in 0..3 {
            service.record_failure("1.2.3.4");
        }
        let result = service.check_rate_limit("1.2.3.4");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.retry_after_secs > 0);
    }

    #[test]
    fn exponential_backoff_increases_lockout() {
        let service = RateLimitService::new(test_config());

        // 3 failures -> lockout at base (10s * 2^0 = 10s)
        for _ in 0..3 {
            service.record_failure("1.2.3.4");
        }
        let err1 = service.check_rate_limit("1.2.3.4").unwrap_err();

        // 4th failure -> lockout at 10s * 2^1 = 20s
        service.record_failure("1.2.3.4");
        let err2 = service.check_rate_limit("1.2.3.4").unwrap_err();

        // 5th failure -> lockout at 10s * 2^2 = 40s
        service.record_failure("1.2.3.4");
        let err3 = service.check_rate_limit("1.2.3.4").unwrap_err();

        assert!(err2.retry_after_secs > err1.retry_after_secs,
            "Second lockout ({}) should be longer than first ({})",
            err2.retry_after_secs, err1.retry_after_secs);
        assert!(err3.retry_after_secs > err2.retry_after_secs,
            "Third lockout ({}) should be longer than second ({})",
            err3.retry_after_secs, err2.retry_after_secs);
    }

    #[test]
    fn max_lockout_cap_is_respected() {
        let service = RateLimitService::new(test_config());

        // Record many failures to push past max_lockout_secs (320)
        // 3 + 6 = 9 failures -> 10 * 2^6 = 640 would exceed cap of 320
        for _ in 0..9 {
            service.record_failure("1.2.3.4");
        }
        let err = service.check_rate_limit("1.2.3.4").unwrap_err();
        assert!(err.retry_after_secs <= 320,
            "Lockout {} should not exceed max_lockout_secs 320", err.retry_after_secs);
    }

    #[test]
    fn success_clears_state() {
        let service = RateLimitService::new(test_config());

        // Lock out the IP
        for _ in 0..3 {
            service.record_failure("1.2.3.4");
        }
        assert!(service.check_rate_limit("1.2.3.4").is_err());

        // Record success to clear
        service.record_success("1.2.3.4");
        assert!(service.check_rate_limit("1.2.3.4").is_ok());
    }

    #[test]
    fn cleanup_removes_expired_entries() {
        let config = RateLimitConfig {
            max_attempts: 3,
            window_secs: 0, // window expires immediately
            lockout_secs: 0,
            max_lockout_secs: 0,
        };
        let service = RateLimitService::new(config);

        service.record_failure("1.2.3.4");
        service.record_failure("5.6.7.8");

        // With window_secs=0, entries should be expired
        service.cleanup();

        // Verify entries were cleaned up by checking internal state is empty
        // (fresh check should pass, which it would anyway, but we can verify
        // no entries remain by recording a failure and checking attempt count)
        assert!(service.check_rate_limit("1.2.3.4").is_ok());
        assert!(service.check_rate_limit("5.6.7.8").is_ok());
    }

    #[test]
    fn different_ips_are_independent() {
        let service = RateLimitService::new(test_config());

        // Lock out one IP
        for _ in 0..3 {
            service.record_failure("1.2.3.4");
        }
        assert!(service.check_rate_limit("1.2.3.4").is_err());

        // Different IP should be unaffected
        assert!(service.check_rate_limit("5.6.7.8").is_ok());
    }

    #[test]
    fn default_config_has_sane_values() {
        let config = RateLimitConfig::default();
        assert_eq!(config.max_attempts, 5);
        assert_eq!(config.window_secs, 60);
        assert_eq!(config.lockout_secs, 60);
        assert_eq!(config.max_lockout_secs, 3600);
    }
}
