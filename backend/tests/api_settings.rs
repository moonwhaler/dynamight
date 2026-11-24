//! Integration tests for app settings: GET/PUT /settings

mod helpers;

use axum::http::{header, HeaderValue};
use helpers::TestApp;
use serde_json::{json, Value};

async fn put(app: &TestApp, cookie: &str, body: Value) {
    app.server
        .put("/settings")
        .add_header(header::COOKIE, HeaderValue::from_str(cookie).unwrap())
        .json(&body)
        .await
        .assert_status_ok();
}

async fn get(app: &TestApp, cookie: &str) -> Value {
    let resp = app
        .server
        .get("/settings")
        .add_header(header::COOKIE, HeaderValue::from_str(cookie).unwrap())
        .await;
    resp.assert_status_ok();
    resp.json()
}

#[tokio::test]
async fn retention_defaults_to_five_and_can_be_unlimited() {
    let app = TestApp::new().await;
    let cookie = TestApp::cookie_value(&app.setup_and_login().await);

    assert_eq!(get(&app, &cookie).await["max_runs_per_job"], 5);

    put(&app, &cookie, json!({ "max_runs_per_job": 20 })).await;
    assert_eq!(get(&app, &cookie).await["max_runs_per_job"], 20);

    put(&app, &cookie, json!({ "max_runs_per_job": null })).await;
    assert_eq!(get(&app, &cookie).await["max_runs_per_job"], Value::Null);
}

#[tokio::test]
async fn saving_one_setting_keeps_the_others() {
    let app = TestApp::new().await;
    let cookie = TestApp::cookie_value(&app.setup_and_login().await);

    put(&app, &cookie, json!({ "max_runs_per_job": 20, "delete_verification_window_minutes": 15 })).await;
    put(&app, &cookie, json!({ "search_timeout_seconds": 30 })).await;
    put(&app, &cookie, json!({ "show_directory_sizes": true })).await;

    let s = get(&app, &cookie).await;
    assert_eq!(s["max_runs_per_job"], 20);
    assert_eq!(s["delete_verification_window_minutes"], 15);
    assert_eq!(s["search_timeout_seconds"], 30);
    assert_eq!(s["show_directory_sizes"], true);
}

#[tokio::test]
async fn log_retention_defaults_to_ninety_days_and_can_be_off() {
    let app = TestApp::new().await;
    let cookie = TestApp::cookie_value(&app.setup_and_login().await);

    assert_eq!(get(&app, &cookie).await["log_retention_days"], 90);
    put(&app, &cookie, json!({ "log_retention_days": 30 })).await;
    put(&app, &cookie, json!({ "search_timeout_seconds": 20 })).await;
    assert_eq!(get(&app, &cookie).await["log_retention_days"], 30);
    put(&app, &cookie, json!({ "log_retention_days": null })).await;
    assert_eq!(get(&app, &cookie).await["log_retention_days"], Value::Null);
}

#[tokio::test]
async fn trash_retention_defaults_to_thirty_days_and_can_be_off() {
    let app = TestApp::new().await;
    let cookie = TestApp::cookie_value(&app.setup_and_login().await);

    let s = get(&app, &cookie).await;
    assert_eq!(s["trash_retention_days"], 30);
    assert_eq!(s["show_directory_sizes"], true);

    put(&app, &cookie, json!({ "trash_retention_days": 7 })).await;
    assert_eq!(get(&app, &cookie).await["trash_retention_days"], 7);

    put(&app, &cookie, json!({ "trash_retention_days": null })).await;
    assert_eq!(get(&app, &cookie).await["trash_retention_days"], Value::Null);
}
