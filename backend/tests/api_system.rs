//! Integration tests for system endpoints:
//!   GET /system/health
//!   GET /system/version
//!   GET /system/allowed-paths

mod helpers;

use helpers::TestApp;

// ── Health ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn health_returns_200() {
    let app = TestApp::new().await;

    let resp = app.server.get("/system/health").await;
    resp.assert_status_ok();

    let body: serde_json::Value = resp.json();
    assert_eq!(body["status"], "healthy");
    // Version should match Cargo.toml
    assert!(body["version"].as_str().is_some());
}

// ── Version ────────────────────────────────────────────────────────────

#[tokio::test]
async fn version_returns_build_info() {
    let app = TestApp::new().await;

    let resp = app.server.get("/system/version").await;
    resp.assert_status_ok();

    let body: serde_json::Value = resp.json();
    // These fields are always present (set by build.rs)
    assert!(body["version"].as_str().is_some());
    assert!(body["build"].as_str().is_some());
    assert!(body["date"].as_str().is_some());
}

// ── Allowed Paths ──────────────────────────────────────────────────────

#[tokio::test]
async fn allowed_paths_returns_existing_paths() {
    let app = TestApp::new().await;
    let cookie = TestApp::cookie_value(&app.setup_and_login().await);

    let resp = app
        .server
        .get("/system/allowed-paths")
        .add_header(
            axum::http::header::COOKIE,
            axum::http::HeaderValue::from_str(&cookie).unwrap(),
        )
        .await;
    resp.assert_status_ok();

    let body: serde_json::Value = resp.json();
    let paths = body["paths"].as_array().expect("paths should be an array");
    // /tmp should exist on any system; the test config only includes /tmp
    assert!(
        paths.iter().any(|p| p.as_str() == Some("/tmp")),
        "Expected /tmp in allowed paths, got: {:?}",
        paths
    );
}

#[tokio::test]
async fn allowed_paths_requires_auth() {
    let app = TestApp::new().await;

    let resp = app.server.get("/system/allowed-paths").await;
    resp.assert_status(axum::http::StatusCode::UNAUTHORIZED);
}

// ── Public endpoints do not require auth ───────────────────────────────

#[tokio::test]
async fn health_does_not_require_auth() {
    let app = TestApp::new().await;

    let resp = app.server.get("/system/health").await;
    resp.assert_status_ok();
}

#[tokio::test]
async fn version_does_not_require_auth() {
    let app = TestApp::new().await;

    let resp = app.server.get("/system/version").await;
    resp.assert_status_ok();
}

// ── Protected system endpoints require auth ────────────────────────────

#[tokio::test]
async fn browse_requires_auth() {
    let app = TestApp::new().await;

    let resp = app
        .server
        .get("/system/browse")
        .add_query_param("path", "/tmp")
        .await;
    resp.assert_status(axum::http::StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn browse_works_with_auth() {
    let app = TestApp::new().await;
    let cookie = TestApp::cookie_value(&app.setup_and_login().await);

    let resp = app
        .server
        .get("/system/browse")
        .add_query_param("path", "/tmp")
        .add_header(
            axum::http::header::COOKIE,
            axum::http::HeaderValue::from_str(&cookie).unwrap(),
        )
        .await;
    resp.assert_status_ok();

    let body: serde_json::Value = resp.json();
    assert!(body["path"].as_str().is_some());
    assert!(body["entries"].as_array().is_some());
}
