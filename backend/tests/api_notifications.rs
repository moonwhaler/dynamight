//! Integration tests for the notification settings endpoints:
//!   GET/PUT /settings/notifications
//!   POST    /settings/notifications/test

mod helpers;

use axum::http::{header, HeaderValue, StatusCode};
use helpers::TestApp;
use serde_json::{json, Value};

fn body(token: Value, auth_header: Value) -> Value {
    json!({
        "ntfy": { "enabled": true, "server": "https://ntfy.example.com", "topic": "backups", "token": token },
        "webhook": { "enabled": false, "url": "", "auth_header": auth_header },
        "events": { "failed": true, "overdue": true, "recovered": false, "success": true },
        "base_url": "https://backup.example.com"
    })
}

#[tokio::test]
async fn settings_round_trip_and_secrets_stay_hidden() {
    let app = TestApp::new().await;
    app.server.get("/settings/notifications").await.assert_status(StatusCode::UNAUTHORIZED);
    let cookie = HeaderValue::from_str(&TestApp::cookie_value(&app.setup_and_login().await)).unwrap();

    // Defaults: nothing enabled
    let resp = app.server.get("/settings/notifications").add_header(header::COOKIE, cookie.clone()).await;
    resp.assert_status_ok();
    let v: Value = resp.json();
    assert_eq!(v["ntfy"]["enabled"], false);
    assert_eq!(v["events"]["failed"], true);

    // Save with a token
    let resp = app
        .server
        .put("/settings/notifications")
        .add_header(header::COOKIE, cookie.clone())
        .json(&body(json!("tk_secret"), Value::Null))
        .await;
    resp.assert_status_ok();
    let v: Value = resp.json();
    assert_eq!(v["ntfy"]["token_set"], true);
    assert!(v.to_string().find("tk_secret").is_none(), "token must not be returned");
    assert_eq!(v["events"]["success"], true);

    // Omitted token is kept, "" clears it
    let resp = app
        .server
        .put("/settings/notifications")
        .add_header(header::COOKIE, cookie.clone())
        .json(&body(Value::Null, Value::Null))
        .await;
    assert_eq!(resp.json::<Value>()["ntfy"]["token_set"], true);
    let resp = app
        .server
        .put("/settings/notifications")
        .add_header(header::COOKIE, cookie.clone())
        .json(&body(json!(""), Value::Null))
        .await;
    assert_eq!(resp.json::<Value>()["ntfy"]["token_set"], false);
}

#[tokio::test]
async fn invalid_settings_name_the_field() {
    let app = TestApp::new().await;
    let cookie = HeaderValue::from_str(&TestApp::cookie_value(&app.setup_and_login().await)).unwrap();
    let mut b = body(Value::Null, Value::Null);
    b["ntfy"]["topic"] = json!("no spaces allowed");
    let resp = app.server.put("/settings/notifications").add_header(header::COOKIE, cookie).json(&b).await;
    resp.assert_status(StatusCode::BAD_REQUEST);
    let v: Value = resp.json();
    assert_eq!(v["code"], "NOTIFICATION_SETTINGS_INVALID");
    assert_eq!(v["params"]["field"], "ntfy_topic");
}

#[tokio::test]
async fn test_endpoint_reports_each_channel() {
    let app = TestApp::new().await;
    let cookie = HeaderValue::from_str(&TestApp::cookie_value(&app.setup_and_login().await)).unwrap();
    let b = json!({
        "ntfy": { "enabled": false, "server": "https://ntfy.sh", "topic": "", "token": null },
        // Port 9 (discard) is closed: the send fails fast
        "webhook": { "enabled": true, "url": "http://127.0.0.1:9/hook", "auth_header": null },
        "events": { "failed": true, "overdue": true, "recovered": true, "success": false }
    });
    let resp = app
        .server
        .post("/settings/notifications/test")
        .add_header(header::COOKIE, cookie.clone())
        .json(&b)
        .await;
    resp.assert_status_ok();
    let v: Value = resp.json();
    let results = v["results"].as_array().unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0]["channel"], "webhook");
    assert_eq!(results[0]["ok"], false);
    assert!(results[0]["error"].as_str().unwrap().contains("could not connect"));

    // Testing does not save
    let v: Value = app.server.get("/settings/notifications").add_header(header::COOKIE, cookie).await.json();
    assert_eq!(v["webhook"]["enabled"], false);
}
