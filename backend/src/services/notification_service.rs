//! Notifications about backup results via ntfy and a generic webhook.
//!
//! Settings live in `app_settings` under [`SETTINGS_KEY`] as one JSON value,
//! so they travel with the (password-encrypted) config export. Secrets are
//! never sent back to the browser, only whether they are set.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::time::Duration;

pub const SETTINGS_KEY: &str = "notifications";

const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EventKind {
    Failed,
    Overdue,
    Recovered,
    Success,
    Test,
}

impl EventKind {
    fn as_str(self) -> &'static str {
        match self {
            EventKind::Failed => "failed",
            EventKind::Overdue => "overdue",
            EventKind::Recovered => "recovered",
            EventKind::Success => "success",
            EventKind::Test => "test",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct NtfySettings {
    pub enabled: bool,
    /// Server root, e.g. https://ntfy.sh or a self-hosted instance
    pub server: String,
    pub topic: String,
    /// Access token (Bearer), empty for none
    pub token: String,
}

impl Default for NtfySettings {
    fn default() -> Self {
        Self { enabled: false, server: "https://ntfy.sh".into(), topic: String::new(), token: String::new() }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct WebhookSettings {
    pub enabled: bool,
    pub url: String,
    /// Sent as the Authorization header when not empty (e.g. "Bearer …")
    pub auth_header: String,
}

/// Which events are sent.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct EventSettings {
    pub failed: bool,
    pub overdue: bool,
    pub recovered: bool,
    pub success: bool,
}

impl Default for EventSettings {
    fn default() -> Self {
        Self { failed: true, overdue: true, recovered: true, success: false }
    }
}

impl EventSettings {
    fn wants(&self, kind: EventKind) -> bool {
        match kind {
            EventKind::Failed => self.failed,
            EventKind::Overdue => self.overdue,
            EventKind::Recovered => self.recovered,
            EventKind::Success => self.success,
            EventKind::Test => true,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct NotificationSettings {
    pub ntfy: NtfySettings,
    pub webhook: WebhookSettings,
    pub events: EventSettings,
    /// Address of this Dynamight (e.g. https://backup.example.com), used to
    /// link notifications to the job; empty for no link
    pub base_url: String,
}

impl NotificationSettings {
    pub async fn load(db: &SqlitePool) -> Self {
        let value: Option<(String,)> = sqlx::query_as("SELECT value FROM app_settings WHERE key = ?")
            .bind(SETTINGS_KEY)
            .fetch_optional(db)
            .await
            .unwrap_or(None);
        value
            .and_then(|(v,)| serde_json::from_str(&v).ok())
            .unwrap_or_default()
    }

    pub async fn save(&self, db: &SqlitePool) -> sqlx::Result<()> {
        sqlx::query(
            "INSERT INTO app_settings (key, value, updated_at) VALUES (?, ?, CURRENT_TIMESTAMP) \
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = CURRENT_TIMESTAMP",
        )
        .bind(SETTINGS_KEY)
        .bind(serde_json::to_string(self).unwrap_or_default())
        .execute(db)
        .await?;
        Ok(())
    }

    /// Check the settings before saving. Returns the invalid field
    /// ("ntfy_server", "ntfy_topic", "webhook_url" or "base_url").
    pub fn validate(&self) -> Result<(), &'static str> {
        fn http_url(value: &str) -> bool {
            url::Url::parse(value).is_ok_and(|u| matches!(u.scheme(), "http" | "https") && u.host().is_some())
        }
        if self.ntfy.enabled {
            if !http_url(&self.ntfy.server) {
                return Err("ntfy_server");
            }
            let topic_ok = !self.ntfy.topic.is_empty()
                && self.ntfy.topic.len() <= 64
                && self.ntfy.topic.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
            if !topic_ok {
                return Err("ntfy_topic");
            }
        }
        if self.webhook.enabled && !http_url(&self.webhook.url) {
            return Err("webhook_url");
        }
        if !self.base_url.is_empty() && !http_url(&self.base_url) {
            return Err("base_url");
        }
        Ok(())
    }

    fn job_link(&self, job_id: Option<i64>) -> Option<String> {
        if self.base_url.is_empty() {
            return None;
        }
        let base = self.base_url.trim_end_matches('/');
        Some(match job_id {
            Some(id) => format!("{base}/#/jobs/{id}"),
            None => format!("{base}/"),
        })
    }
}

/// Run details included in notifications and the webhook payload.
#[derive(Debug, Clone, Serialize)]
pub struct RunInfo {
    pub id: i64,
    pub status: String,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub files_transferred: i64,
    pub bytes_transferred: i64,
    pub error_count: i64,
}

#[derive(Debug, Clone)]
pub struct Notification {
    pub kind: EventKind,
    pub job_id: Option<i64>,
    pub job_name: String,
    pub title: String,
    pub message: String,
    pub run: Option<RunInfo>,
}

/// Result of sending to one channel (shown by the test button).
#[derive(Debug, Clone, Serialize)]
pub struct ChannelResult {
    pub channel: &'static str,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Clone)]
pub struct Notifier {
    db: SqlitePool,
    http: reqwest::Client,
}

impl Notifier {
    pub fn new(db: SqlitePool) -> Self {
        let http = reqwest::Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .user_agent(concat!("Dynamight/", env!("CARGO_PKG_VERSION")))
            .build()
            .unwrap_or_default();
        Self { db, http }
    }

    /// Send `notification` to every enabled channel if its event is enabled.
    /// Failures are logged; they never affect the backup.
    pub async fn notify(&self, notification: Notification) {
        let settings = NotificationSettings::load(&self.db).await;
        if !settings.events.wants(notification.kind) {
            return;
        }
        for result in self.send(&settings, &notification).await {
            if let Some(error) = result.error {
                tracing::warn!(
                    "Notification '{}' for job '{}' via {} failed: {}",
                    notification.kind.as_str(),
                    notification.job_name,
                    result.channel,
                    error
                );
            }
        }
    }

    /// Send to every enabled channel of `settings`, regardless of the event
    /// selection (used by `notify` and the test button).
    pub async fn send(&self, settings: &NotificationSettings, n: &Notification) -> Vec<ChannelResult> {
        let mut results = Vec::new();
        if settings.ntfy.enabled {
            let result = self.send_ntfy(settings, n).await;
            results.push(ChannelResult { channel: "ntfy", ok: result.is_ok(), error: result.err() });
        }
        if settings.webhook.enabled {
            let result = self.send_webhook(settings, n).await;
            results.push(ChannelResult { channel: "webhook", ok: result.is_ok(), error: result.err() });
        }
        results
    }

    async fn send_ntfy(&self, settings: &NotificationSettings, n: &Notification) -> Result<(), String> {
        // JSON publishing (POST to the server root) handles non-ASCII titles,
        // which the header-based API cannot carry
        let (priority, tag) = match n.kind {
            EventKind::Failed => (4, "rotating_light"),
            EventKind::Overdue => (4, "warning"),
            EventKind::Recovered => (3, "white_check_mark"),
            EventKind::Success => (2, "floppy_disk"),
            EventKind::Test => (3, "bell"),
        };
        let mut body = serde_json::json!({
            "topic": settings.ntfy.topic,
            "title": n.title,
            "message": n.message,
            "priority": priority,
            "tags": [tag],
        });
        if let Some(link) = settings.job_link(n.job_id) {
            body["click"] = serde_json::Value::String(link);
        }
        let mut request = self.http.post(settings.ntfy.server.trim_end_matches('/')).json(&body);
        if !settings.ntfy.token.is_empty() {
            request = request.bearer_auth(&settings.ntfy.token);
        }
        check(request.send().await).await
    }

    async fn send_webhook(&self, settings: &NotificationSettings, n: &Notification) -> Result<(), String> {
        let body = serde_json::json!({
            "event": n.kind.as_str(),
            "title": n.title,
            "message": n.message,
            "timestamp": Utc::now(),
            "url": settings.job_link(n.job_id),
            "job": n.job_id.map(|id| serde_json::json!({ "id": id, "name": n.job_name })),
            "run": n.run,
        });
        let mut request = self.http.post(&settings.webhook.url).json(&body);
        if !settings.webhook.auth_header.is_empty() {
            request = request.header(reqwest::header::AUTHORIZATION, &settings.webhook.auth_header);
        }
        check(request.send().await).await
    }
}

async fn check(response: reqwest::Result<reqwest::Response>) -> Result<(), String> {
    let response = response.map_err(|e| {
        if e.is_timeout() {
            "no answer within 10 seconds".to_string()
        } else if e.is_connect() {
            format!("could not connect ({})", root_cause(&e))
        } else {
            root_cause(&e)
        }
    })?;
    let status = response.status();
    if status.is_success() {
        return Ok(());
    }
    let text = response.text().await.unwrap_or_default();
    let text: String = text.chars().take(200).collect();
    Err(format!("HTTP {}{}", status, if text.is_empty() { String::new() } else { format!(": {}", text.trim()) }))
}

fn root_cause(e: &dyn std::error::Error) -> String {
    let mut cause = e;
    while let Some(source) = cause.source() {
        cause = source;
    }
    cause.to_string()
}

// ───────────────────────── Message texts ─────────────────────────

pub fn format_duration(d: chrono::Duration) -> String {
    let secs = d.num_seconds().max(0);
    match secs {
        s if s < 60 => format!("{s}s"),
        s if s < 3600 => format!("{}m {}s", s / 60, s % 60),
        s => format!("{}h {}m", s / 3600, (s % 3600) / 60),
    }
}

pub fn format_bytes(bytes: i64) -> String {
    const KB: f64 = 1024.0;
    let b = bytes.max(0) as f64;
    if b < KB {
        format!("{} B", bytes.max(0))
    } else if b < KB * KB {
        format!("{:.1} KB", b / KB)
    } else if b < KB * KB * KB {
        format!("{:.1} MB", b / (KB * KB))
    } else {
        format!("{:.2} GB", b / (KB * KB * KB))
    }
}

/// The notification for a finished run.
pub fn run_notification(
    kind: EventKind,
    job_id: i64,
    job_name: &str,
    run: RunInfo,
    dry_run: bool,
    last_error: Option<String>,
) -> Notification {
    let duration = match (run.started_at, run.completed_at) {
        (Some(start), Some(end)) => format!(" in {}", format_duration(end - start)),
        _ => String::new(),
    };
    let stats = format!(
        "{} file(s), {} transferred",
        run.files_transferred,
        format_bytes(run.bytes_transferred)
    );
    let dry = if dry_run { " (dry run)" } else { "" };
    let (title, message) = match kind {
        EventKind::Failed => {
            let mut message = format!(
                "Run #{} failed{}{}. {}, {} error(s).",
                run.id, duration, dry, stats, run.error_count
            );
            if let Some(line) = last_error {
                message.push_str(&format!("\nLast error: {}", line));
            }
            (format!("Backup failed: {}", job_name), message)
        }
        EventKind::Recovered => (
            format!("Backup recovered: {}", job_name),
            format!("Run #{} completed{}{} after earlier problems. {}.", run.id, duration, dry, stats),
        ),
        _ => (
            format!("Backup completed: {}", job_name),
            format!("Run #{} completed{}{}. {}.", run.id, duration, dry, stats),
        ),
    };
    Notification { kind, job_id: Some(job_id), job_name: job_name.to_string(), title, message, run: Some(run) }
}

/// The notification for a job without a successful backup for too long.
pub fn overdue_notification(
    job_id: i64,
    job_name: &str,
    last_success: Option<DateTime<Utc>>,
    now: DateTime<Utc>,
    tz: chrono_tz::Tz,
) -> Notification {
    let message = match last_success {
        Some(at) => format!(
            "No successful backup since {} ({} ago).",
            at.with_timezone(&tz).format("%Y-%m-%d %H:%M %Z"),
            format_age(now - at)
        ),
        None => "No successful backup since the job was created.".to_string(),
    };
    Notification {
        kind: EventKind::Overdue,
        job_id: Some(job_id),
        job_name: job_name.to_string(),
        title: format!("Backup overdue: {}", job_name),
        message,
        run: None,
    }
}

pub fn test_notification() -> Notification {
    Notification {
        kind: EventKind::Test,
        job_id: None,
        job_name: String::new(),
        title: "Dynamight test notification".into(),
        message: "Notifications from Dynamight arrive here.".into(),
        run: None,
    }
}

fn format_age(d: chrono::Duration) -> String {
    let hours = d.num_hours();
    if hours < 48 {
        format!("{} hour(s)", hours.max(0))
    } else {
        format!("{} days", d.num_days())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_settings_mean_nothing_enabled() {
        let s: NotificationSettings = serde_json::from_str("{}").unwrap();
        assert!(!s.ntfy.enabled && !s.webhook.enabled);
        assert_eq!(s.ntfy.server, "https://ntfy.sh");
        assert!(s.events.failed && s.events.overdue && s.events.recovered && !s.events.success);
    }

    #[test]
    fn validation_names_the_problem() {
        let mut s = NotificationSettings::default();
        assert!(s.validate().is_ok());
        s.ntfy.enabled = true;
        assert_eq!(s.validate(), Err("ntfy_topic"));
        s.ntfy.topic = "my-backups_1".into();
        assert!(s.validate().is_ok());
        s.ntfy.topic = "has space".into();
        assert!(s.validate().is_err());
        s.ntfy.topic = "ok".into();
        s.ntfy.server = "ftp://x".into();
        assert_eq!(s.validate(), Err("ntfy_server"));
        s.ntfy.server = "https://ntfy.example.com".into();
        s.webhook.enabled = true;
        assert_eq!(s.validate(), Err("webhook_url"));
        s.webhook.url = "http://homeassistant.local:8123/api/webhook/abc".into();
        assert!(s.validate().is_ok());
        s.base_url = "backup.local".into();
        assert_eq!(s.validate(), Err("base_url"));
    }

    #[test]
    fn links_point_to_the_job() {
        let s = NotificationSettings { base_url: "https://backup.example.com/".into(), ..Default::default() };
        assert_eq!(s.job_link(Some(3)).as_deref(), Some("https://backup.example.com/#/jobs/3"));
        assert_eq!(NotificationSettings::default().job_link(Some(3)), None);
    }

    #[test]
    fn run_texts() {
        let start = Utc::now();
        let run = RunInfo {
            id: 12,
            status: "failed".into(),
            started_at: Some(start),
            completed_at: Some(start + chrono::Duration::seconds(192)),
            files_transferred: 3,
            bytes_transferred: 5 * 1024 * 1024,
            error_count: 1,
        };
        let n = run_notification(EventKind::Failed, 1, "Docs", run.clone(), false, Some("rsync failed".into()));
        assert_eq!(n.title, "Backup failed: Docs");
        assert_eq!(n.message, "Run #12 failed in 3m 12s. 3 file(s), 5.0 MB transferred, 1 error(s).\nLast error: rsync failed");
        let n = run_notification(EventKind::Success, 1, "Docs", run, true, None);
        assert_eq!(n.message, "Run #12 completed in 3m 12s (dry run). 3 file(s), 5.0 MB transferred.");
    }

    #[test]
    fn overdue_text() {
        let now = Utc::now();
        let n = overdue_notification(1, "Archive", Some(now - chrono::Duration::days(3)), now, chrono_tz::Tz::UTC);
        assert!(n.message.starts_with("No successful backup since "));
        assert!(n.message.ends_with("UTC (3 days ago)."), "{}", n.message);
    }

    /// Sends to a local HTTP listener and checks what arrives.
    #[tokio::test]
    async fn sends_ntfy_json_and_webhook_payload() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let mut requests = Vec::new();
            for _ in 0..2 {
                let (mut sock, _) = listener.accept().await.unwrap();
                let mut buf = vec![0u8; 16384];
                let mut len = 0;
                // Read headers and body (Content-Length)
                loop {
                    let n = sock.read(&mut buf[len..]).await.unwrap();
                    len += n;
                    let text = String::from_utf8_lossy(&buf[..len]).to_string();
                    if let Some(head_end) = text.find("\r\n\r\n") {
                        let cl = text
                            .lines()
                            .find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length: ").map(|v| v.trim().parse::<usize>().unwrap()))
                            .unwrap_or(0);
                        if len >= head_end + 4 + cl {
                            requests.push(text);
                            break;
                        }
                    }
                    if n == 0 {
                        break;
                    }
                }
                sock.write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 0\r\n\r\n").await.unwrap();
            }
            requests
        });

        let settings = NotificationSettings {
            ntfy: NtfySettings { enabled: true, server: format!("http://{addr}"), topic: "t".into(), token: "tk_secret".into() },
            webhook: WebhookSettings { enabled: true, url: format!("http://{addr}/hook"), auth_header: "Bearer abc".into() },
            base_url: "https://backup.example.com".into(),
            ..Default::default()
        };
        let db = crate::test_helpers::test_db().await;
        let notifier = Notifier::new(db);
        let n = overdue_notification(4, "Fotos Übersicht", None, Utc::now(), chrono_tz::Tz::UTC);
        let results = notifier.send(&settings, &n).await;
        assert!(results.iter().all(|r| r.ok), "{:?}", results);

        let requests = server.await.unwrap();
        let ntfy = &requests[0];
        assert!(ntfy.starts_with("POST / "));
        assert!(ntfy.to_ascii_lowercase().contains("authorization: bearer tk_secret"));
        assert!(ntfy.contains(r#""title":"Backup overdue: Fotos Übersicht""#));
        assert!(ntfy.contains(r#""click":"https://backup.example.com/#/jobs/4""#));
        let hook = &requests[1];
        assert!(hook.starts_with("POST /hook "));
        assert!(hook.to_ascii_lowercase().contains("authorization: bearer abc"));
        assert!(hook.contains(r#""event":"overdue""#));
        assert!(hook.contains(r#""job":{"id":4,"name":"Fotos Übersicht"}"#));
    }

    #[tokio::test]
    async fn reports_http_errors_per_channel() {
        let settings = NotificationSettings {
            webhook: WebhookSettings { enabled: true, url: "http://127.0.0.1:9/".into(), auth_header: String::new() },
            ..Default::default()
        };
        let notifier = Notifier::new(crate::test_helpers::test_db().await);
        let results = notifier.send(&settings, &test_notification()).await;
        assert_eq!(results.len(), 1);
        assert!(!results[0].ok);
        assert!(results[0].error.as_deref().unwrap().contains("could not connect"), "{:?}", results[0].error);
    }
}
