//! Integration tests for job CRUD endpoints:
//!   GET    /jobs
//!   POST   /jobs
//!   GET    /jobs/:id
//!   PUT    /jobs/:id
//!   DELETE /jobs/:id
//!   POST   /jobs/:id/clone

mod helpers;

use helpers::TestApp;
use serde_json::json;

/// Helper: authenticate and create a minimal job, returning the job id.
async fn create_test_job(app: &TestApp, cookie: &str, name: &str) -> i64 {
    let resp = app
        .server
        .post("/jobs")
        .add_header(
            axum::http::header::COOKIE,
            axum::http::HeaderValue::from_str(cookie).unwrap(),
        )
        .json(&json!({
            "name": name,
            "source_dirs": ["/tmp/src"],
            "mount_point": "/mnt/backup",
        }))
        .await;
    resp.assert_status(axum::http::StatusCode::CREATED);

    let body: serde_json::Value = resp.json();
    body["id"].as_i64().expect("Job should have an id")
}

// ── List Jobs ──────────────────────────────────────────────────────────

#[tokio::test]
async fn list_jobs_empty() {
    let app = TestApp::new().await;
    let cookie = TestApp::cookie_value(&app.setup_and_login().await);

    let resp = app
        .server
        .get("/jobs")
        .add_header(
            axum::http::header::COOKIE,
            axum::http::HeaderValue::from_str(&cookie).unwrap(),
        )
        .await;
    resp.assert_status_ok();

    let body: Vec<serde_json::Value> = resp.json();
    assert!(body.is_empty());
}

#[tokio::test]
async fn list_jobs_returns_created_jobs() {
    let app = TestApp::new().await;
    let cookie = TestApp::cookie_value(&app.setup_and_login().await);

    create_test_job(&app, &cookie, "Job A").await;
    create_test_job(&app, &cookie, "Job B").await;

    let resp = app
        .server
        .get("/jobs")
        .add_header(
            axum::http::header::COOKIE,
            axum::http::HeaderValue::from_str(&cookie).unwrap(),
        )
        .await;
    resp.assert_status_ok();

    let body: Vec<serde_json::Value> = resp.json();
    assert_eq!(body.len(), 2);
}

#[tokio::test]
async fn list_jobs_rejects_unauthenticated() {
    let app = TestApp::new().await;

    let resp = app.server.get("/jobs").await;
    resp.assert_status(axum::http::StatusCode::UNAUTHORIZED);
}

// ── Create Job ─────────────────────────────────────────────────────────

#[tokio::test]
async fn create_job_succeeds() {
    let app = TestApp::new().await;
    let cookie = TestApp::cookie_value(&app.setup_and_login().await);

    let resp = app
        .server
        .post("/jobs")
        .add_header(
            axum::http::header::COOKIE,
            axum::http::HeaderValue::from_str(&cookie).unwrap(),
        )
        .json(&json!({
            "name": "My Backup",
            "description": "Daily backup",
            "source_dirs": ["/tmp/docs", "/tmp/photos"],
            "mount_point": "/mnt/usb",
            "backup_subdir": "backups",
            "sync_deletes": true,
            "rsync_excludes": ["*.tmp", ".cache"],
        }))
        .await;
    resp.assert_status(axum::http::StatusCode::CREATED);

    let body: serde_json::Value = resp.json();
    assert_eq!(body["name"], "My Backup");
    assert_eq!(body["description"], "Daily backup");
    assert_eq!(body["source_dirs"], json!(["/tmp/docs", "/tmp/photos"]));
    assert_eq!(body["sync_deletes"], true);
    assert_eq!(body["rsync_excludes"], json!(["*.tmp", ".cache"]));
    assert!(body["id"].as_i64().unwrap() > 0);
}

#[tokio::test]
async fn create_job_rejects_empty_name() {
    let app = TestApp::new().await;
    let cookie = TestApp::cookie_value(&app.setup_and_login().await);

    let resp = app
        .server
        .post("/jobs")
        .add_header(
            axum::http::header::COOKIE,
            axum::http::HeaderValue::from_str(&cookie).unwrap(),
        )
        .json(&json!({
            "name": "",
            "source_dirs": ["/tmp/src"],
            "mount_point": "/mnt/backup",
        }))
        .await;
    resp.assert_status(axum::http::StatusCode::BAD_REQUEST);

    let body: serde_json::Value = resp.json();
    assert_eq!(body["code"], "VALIDATION_FIELD_REQUIRED");
}

#[tokio::test]
async fn create_job_rejects_empty_source_dirs() {
    let app = TestApp::new().await;
    let cookie = TestApp::cookie_value(&app.setup_and_login().await);

    let resp = app
        .server
        .post("/jobs")
        .add_header(
            axum::http::header::COOKIE,
            axum::http::HeaderValue::from_str(&cookie).unwrap(),
        )
        .json(&json!({
            "name": "Bad Job",
            "source_dirs": [],
            "mount_point": "/mnt/backup",
        }))
        .await;
    resp.assert_status(axum::http::StatusCode::BAD_REQUEST);

    let body: serde_json::Value = resp.json();
    assert_eq!(body["code"], "SOURCE_DIRS_REQUIRED");
}

#[tokio::test]
async fn create_job_rejects_duplicate_name() {
    let app = TestApp::new().await;
    let cookie = TestApp::cookie_value(&app.setup_and_login().await);

    create_test_job(&app, &cookie, "Unique Name").await;

    let resp = app
        .server
        .post("/jobs")
        .add_header(
            axum::http::header::COOKIE,
            axum::http::HeaderValue::from_str(&cookie).unwrap(),
        )
        .json(&json!({
            "name": "Unique Name",
            "source_dirs": ["/tmp/other"],
            "mount_point": "/mnt/backup",
        }))
        .await;
    resp.assert_status(axum::http::StatusCode::CONFLICT);

    let body: serde_json::Value = resp.json();
    assert_eq!(body["code"], "JOB_NAME_EXISTS");
}

#[tokio::test]
async fn create_job_rejects_duplicate_source_basenames() {
    let app = TestApp::new().await;
    let cookie = TestApp::cookie_value(&app.setup_and_login().await);

    let resp = app
        .server
        .post("/jobs")
        .add_header(
            axum::http::header::COOKIE,
            axum::http::HeaderValue::from_str(&cookie).unwrap(),
        )
        .json(&json!({
            "name": "Dup Basenames",
            "source_dirs": ["/mnt/a/photos", "/mnt/b/photos"],
            "mount_point": "/mnt/backup",
        }))
        .await;
    resp.assert_status(axum::http::StatusCode::BAD_REQUEST);

    let body: serde_json::Value = resp.json();
    assert_eq!(body["code"], "SOURCE_DIRS_DUPLICATE_BASENAMES");
}

#[tokio::test]
async fn create_job_rejects_dangerous_exclude_pattern() {
    let app = TestApp::new().await;
    let cookie = TestApp::cookie_value(&app.setup_and_login().await);

    let resp = app
        .server
        .post("/jobs")
        .add_header(
            axum::http::header::COOKIE,
            axum::http::HeaderValue::from_str(&cookie).unwrap(),
        )
        .json(&json!({
            "name": "Shell Inject",
            "source_dirs": ["/tmp/src"],
            "mount_point": "/mnt/backup",
            "rsync_excludes": ["$(rm -rf /)"],
        }))
        .await;
    resp.assert_status(axum::http::StatusCode::BAD_REQUEST);

    let body: serde_json::Value = resp.json();
    assert_eq!(body["code"], "VALIDATION_INVALID_PATTERN");
}

// ── Get Job ────────────────────────────────────────────────────────────

#[tokio::test]
async fn get_job_returns_existing_job() {
    let app = TestApp::new().await;
    let cookie = TestApp::cookie_value(&app.setup_and_login().await);

    let id = create_test_job(&app, &cookie, "Fetch Me").await;

    let resp = app
        .server
        .get(&format!("/jobs/{}", id))
        .add_header(
            axum::http::header::COOKIE,
            axum::http::HeaderValue::from_str(&cookie).unwrap(),
        )
        .await;
    resp.assert_status_ok();

    let body: serde_json::Value = resp.json();
    assert_eq!(body["id"], id);
    assert_eq!(body["name"], "Fetch Me");
}

#[tokio::test]
async fn get_job_returns_404_for_missing_id() {
    let app = TestApp::new().await;
    let cookie = TestApp::cookie_value(&app.setup_and_login().await);

    let resp = app
        .server
        .get("/jobs/999")
        .add_header(
            axum::http::header::COOKIE,
            axum::http::HeaderValue::from_str(&cookie).unwrap(),
        )
        .await;
    resp.assert_status(axum::http::StatusCode::NOT_FOUND);

    let body: serde_json::Value = resp.json();
    assert_eq!(body["code"], "JOB_NOT_FOUND");
}

// ── Update Job ─────────────────────────────────────────────────────────

#[tokio::test]
async fn update_job_succeeds() {
    let app = TestApp::new().await;
    let cookie = TestApp::cookie_value(&app.setup_and_login().await);

    let id = create_test_job(&app, &cookie, "Original").await;

    let resp = app
        .server
        .put(&format!("/jobs/{}", id))
        .add_header(
            axum::http::header::COOKIE,
            axum::http::HeaderValue::from_str(&cookie).unwrap(),
        )
        .json(&json!({
            "name": "Updated",
            "description": "new description",
        }))
        .await;
    resp.assert_status_ok();

    let body: serde_json::Value = resp.json();
    assert_eq!(body["name"], "Updated");
    assert_eq!(body["description"], "new description");
}

#[tokio::test]
async fn update_job_returns_404_for_missing_id() {
    let app = TestApp::new().await;
    let cookie = TestApp::cookie_value(&app.setup_and_login().await);

    let resp = app
        .server
        .put("/jobs/999")
        .add_header(
            axum::http::header::COOKIE,
            axum::http::HeaderValue::from_str(&cookie).unwrap(),
        )
        .json(&json!({ "name": "No Such Job" }))
        .await;
    resp.assert_status(axum::http::StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn update_job_rejects_duplicate_name() {
    let app = TestApp::new().await;
    let cookie = TestApp::cookie_value(&app.setup_and_login().await);

    create_test_job(&app, &cookie, "Taken").await;
    let id = create_test_job(&app, &cookie, "Other").await;

    let resp = app
        .server
        .put(&format!("/jobs/{}", id))
        .add_header(
            axum::http::header::COOKIE,
            axum::http::HeaderValue::from_str(&cookie).unwrap(),
        )
        .json(&json!({ "name": "Taken" }))
        .await;
    resp.assert_status(axum::http::StatusCode::CONFLICT);

    let body: serde_json::Value = resp.json();
    assert_eq!(body["code"], "JOB_NAME_EXISTS");
}

// ── Delete Job ─────────────────────────────────────────────────────────

#[tokio::test]
async fn delete_job_succeeds() {
    let app = TestApp::new().await;
    let cookie = TestApp::cookie_value(&app.setup_and_login().await);

    let id = create_test_job(&app, &cookie, "To Delete").await;

    let resp = app
        .server
        .delete(&format!("/jobs/{}", id))
        .add_header(
            axum::http::header::COOKIE,
            axum::http::HeaderValue::from_str(&cookie).unwrap(),
        )
        .await;
    resp.assert_status_ok();

    let body: serde_json::Value = resp.json();
    assert_eq!(body["success"], true);

    // Verify it's gone
    let resp = app
        .server
        .get(&format!("/jobs/{}", id))
        .add_header(
            axum::http::header::COOKIE,
            axum::http::HeaderValue::from_str(&cookie).unwrap(),
        )
        .await;
    resp.assert_status(axum::http::StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn delete_job_returns_404_for_missing_id() {
    let app = TestApp::new().await;
    let cookie = TestApp::cookie_value(&app.setup_and_login().await);

    let resp = app
        .server
        .delete("/jobs/999")
        .add_header(
            axum::http::header::COOKIE,
            axum::http::HeaderValue::from_str(&cookie).unwrap(),
        )
        .await;
    resp.assert_status(axum::http::StatusCode::NOT_FOUND);
}

// ── Clone Job ──────────────────────────────────────────────────────────

#[tokio::test]
async fn clone_job_succeeds() {
    let app = TestApp::new().await;
    let cookie = TestApp::cookie_value(&app.setup_and_login().await);

    let id = create_test_job(&app, &cookie, "Original Job").await;

    let resp = app
        .server
        .post(&format!("/jobs/{}/clone", id))
        .add_header(
            axum::http::header::COOKIE,
            axum::http::HeaderValue::from_str(&cookie).unwrap(),
        )
        .await;
    resp.assert_status(axum::http::StatusCode::CREATED);

    let body: serde_json::Value = resp.json();
    // Cloned job gets a new id
    assert_ne!(body["id"].as_i64().unwrap(), id);
    // Name should contain "(clone)"
    let cloned_name = body["name"].as_str().unwrap();
    assert!(
        cloned_name.contains("(clone)"),
        "Expected clone suffix, got: {}",
        cloned_name
    );
}

#[tokio::test]
async fn clone_nonexistent_job_returns_404() {
    let app = TestApp::new().await;
    let cookie = TestApp::cookie_value(&app.setup_and_login().await);

    let resp = app
        .server
        .post("/jobs/999/clone")
        .add_header(
            axum::http::header::COOKIE,
            axum::http::HeaderValue::from_str(&cookie).unwrap(),
        )
        .await;
    resp.assert_status(axum::http::StatusCode::NOT_FOUND);
}

// ── Job fields round-trip ──────────────────────────────────────────────

#[tokio::test]
async fn job_preserves_all_fields() {
    let app = TestApp::new().await;
    let cookie = TestApp::cookie_value(&app.setup_and_login().await);

    let resp = app
        .server
        .post("/jobs")
        .add_header(
            axum::http::header::COOKIE,
            axum::http::HeaderValue::from_str(&cookie).unwrap(),
        )
        .json(&json!({
            "name": "Full Job",
            "description": "All fields set",
            "enabled": false,
            "source_dirs": ["/tmp/a"],
            "mount_point": "/mnt/disk",
            "backup_subdir": "my_backups",
            "sync_deletes": true,
            "rsync_excludes": ["*.log"],
            "checksum_mode": true,
            "compress": true,
            "dry_run": true,
            "bandwidth_limit": 1000,
            "verbosity": "verbose",
        }))
        .await;
    resp.assert_status(axum::http::StatusCode::CREATED);

    let body: serde_json::Value = resp.json();
    assert_eq!(body["enabled"], false);
    assert_eq!(body["backup_subdir"], "my_backups");
    assert_eq!(body["checksum_mode"], true);
    assert_eq!(body["compress"], true);
    assert_eq!(body["dry_run"], true);
    assert_eq!(body["bandwidth_limit"], 1000);
    assert_eq!(body["verbosity"], "verbose");
}
