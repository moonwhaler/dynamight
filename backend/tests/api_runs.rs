//! Integration tests for run details and log filtering:
//!   GET /runs/:id
//!   GET /runs/:id/logs?level=…&q=…

mod helpers;

use axum::http::{header, HeaderValue};
use helpers::TestApp;
use serde_json::{json, Value};

async fn seed(app: &TestApp, cookie: &str) -> i64 {
    let resp = app
        .server
        .post("/jobs")
        .add_header(header::COOKIE, HeaderValue::from_str(cookie).unwrap())
        .json(&json!({ "name": "Logs", "source_dirs": ["/tmp/src"], "mount_point": "/mnt/backup" }))
        .await;
    resp.assert_status(axum::http::StatusCode::CREATED);
    let job_id = resp.json::<Value>()["id"].as_i64().unwrap();

    let (run_id,): (i64,) = sqlx::query_as(
        "INSERT INTO job_runs (job_id, status, started_at, completed_at, summary) \
         VALUES (?, 'failed', datetime('now'), datetime('now'), 'rsync: connection refused') RETURNING id",
    )
    .bind(job_id)
    .fetch_one(&app.db)
    .await
    .unwrap();

    for (level, message) in [
        ("info", "Starting backup job"),
        ("warning", "Disk almost full"),
        ("info", "file 100%_done.txt"),
        ("error", "rsync: connection refused"),
        ("debug", "args"),
    ] {
        sqlx::query(
            "INSERT INTO log_entries (job_run_id, level, message, source, timestamp) VALUES (?, ?, ?, 'test', datetime('now'))",
        )
        .bind(run_id)
        .bind(level)
        .bind(message)
        .execute(&app.logs_db)
        .await
        .unwrap();
    }
    run_id
}

async fn get(app: &TestApp, cookie: &str, path: &str) -> Value {
    let resp = app
        .server
        .get(path)
        .add_header(header::COOKIE, HeaderValue::from_str(cookie).unwrap())
        .await;
    resp.assert_status_ok();
    resp.json()
}

fn messages(body: &Value) -> Vec<String> {
    body["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["message"].as_str().unwrap().to_string())
        .collect()
}

#[tokio::test]
async fn run_summary_is_returned_as_text() {
    let app = TestApp::new().await;
    let cookie = TestApp::cookie_value(&app.setup_and_login().await);
    let run_id = seed(&app, &cookie).await;

    let run = get(&app, &cookie, &format!("/runs/{run_id}")).await;
    assert_eq!(run["summary"], "rsync: connection refused");
}

#[tokio::test]
async fn logs_filter_by_several_levels() {
    let app = TestApp::new().await;
    let cookie = TestApp::cookie_value(&app.setup_and_login().await);
    let run_id = seed(&app, &cookie).await;

    let body = get(&app, &cookie, &format!("/runs/{run_id}/logs?level=warning,error")).await;
    assert_eq!(body["total"], 2);
    assert_eq!(messages(&body), ["Disk almost full", "rsync: connection refused"]);

    let all = get(&app, &cookie, &format!("/runs/{run_id}/logs")).await;
    assert_eq!(all["total"], 5);
}

#[tokio::test]
async fn logs_search_is_case_insensitive_and_literal() {
    let app = TestApp::new().await;
    let cookie = TestApp::cookie_value(&app.setup_and_login().await);
    let run_id = seed(&app, &cookie).await;

    let body = get(&app, &cookie, &format!("/runs/{run_id}/logs?q=RSYNC")).await;
    assert_eq!(messages(&body), ["rsync: connection refused"]);

    // % and _ are searched for, not wildcards
    let body = get(&app, &cookie, &format!("/runs/{run_id}/logs?q=100%25_")).await;
    assert_eq!(messages(&body), ["file 100%_done.txt"]);
    let body = get(&app, &cookie, &format!("/runs/{run_id}/logs?q=%25%25")).await;
    assert_eq!(body["total"], 0);

    // Combined with a level
    let body = get(&app, &cookie, &format!("/runs/{run_id}/logs?level=info&q=start")).await;
    assert_eq!(messages(&body), ["Starting backup job"]);
}
