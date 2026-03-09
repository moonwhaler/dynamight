//! Integration tests for authentication endpoints:
//!   POST /auth/setup-required
//!   POST /auth/setup
//!   POST /auth/login
//!   POST /auth/logout
//!   GET  /auth/me
//!   POST /auth/change-password

mod helpers;

use helpers::TestApp;
use serde_json::json;

// ── Setup ──────────────────────────────────────────────────────────────

#[tokio::test]
async fn setup_required_returns_true_on_fresh_db() {
    let app = TestApp::new().await;

    let resp = app.server.get("/auth/setup-required").await;
    resp.assert_status_ok();

    let body: serde_json::Value = resp.json();
    assert_eq!(body["setup_required"], true);
}

#[tokio::test]
async fn setup_creates_admin_and_clears_setup_required() {
    let app = TestApp::new().await;

    // Create admin
    let resp = app
        .server
        .post("/auth/setup")
        .json(&json!({
            "username": "admin",
            "password": "password123",
        }))
        .await;
    resp.assert_status_ok();

    // setup_required should now be false
    let resp = app.server.get("/auth/setup-required").await;
    let body: serde_json::Value = resp.json();
    assert_eq!(body["setup_required"], false);
}

#[tokio::test]
async fn setup_rejects_short_username() {
    let app = TestApp::new().await;

    let resp = app
        .server
        .post("/auth/setup")
        .json(&json!({
            "username": "ab",
            "password": "password123",
        }))
        .await;
    resp.assert_status(axum::http::StatusCode::BAD_REQUEST);

    let body: serde_json::Value = resp.json();
    assert_eq!(body["code"], "USERNAME_TOO_SHORT");
}

#[tokio::test]
async fn setup_rejects_short_password() {
    let app = TestApp::new().await;

    let resp = app
        .server
        .post("/auth/setup")
        .json(&json!({
            "username": "admin",
            "password": "short",
        }))
        .await;
    resp.assert_status(axum::http::StatusCode::BAD_REQUEST);

    let body: serde_json::Value = resp.json();
    assert_eq!(body["code"], "PASSWORD_TOO_SHORT");
}

#[tokio::test]
async fn setup_rejects_second_call() {
    let app = TestApp::new().await;

    app.setup_admin("admin", "password123").await;

    // Second setup should fail
    let resp = app
        .server
        .post("/auth/setup")
        .json(&json!({
            "username": "admin2",
            "password": "password123",
        }))
        .await;
    resp.assert_status(axum::http::StatusCode::FORBIDDEN);

    let body: serde_json::Value = resp.json();
    assert_eq!(body["code"], "SETUP_ALREADY_DONE");
}

// ── Login ──────────────────────────────────────────────────────────────

#[tokio::test]
async fn login_succeeds_with_correct_credentials() {
    let app = TestApp::new().await;
    app.setup_admin("admin", "password123").await;

    let resp = app
        .server
        .post("/auth/login")
        .json(&json!({
            "username": "admin",
            "password": "password123",
        }))
        .await;
    resp.assert_status_ok();

    let body: serde_json::Value = resp.json();
    assert_eq!(body["success"], true);
    assert_eq!(body["user"]["username"], "admin");

    // Should have Set-Cookie header
    assert!(resp.headers().get("set-cookie").is_some());
}

#[tokio::test]
async fn login_fails_with_wrong_password() {
    let app = TestApp::new().await;
    app.setup_admin("admin", "password123").await;

    let resp = app
        .server
        .post("/auth/login")
        .json(&json!({
            "username": "admin",
            "password": "wrongpassword",
        }))
        .await;
    resp.assert_status(axum::http::StatusCode::UNAUTHORIZED);

    let body: serde_json::Value = resp.json();
    assert_eq!(body["code"], "INVALID_CREDENTIALS");
}

#[tokio::test]
async fn login_fails_with_nonexistent_user() {
    let app = TestApp::new().await;
    app.setup_admin("admin", "password123").await;

    let resp = app
        .server
        .post("/auth/login")
        .json(&json!({
            "username": "noone",
            "password": "password123",
        }))
        .await;
    resp.assert_status(axum::http::StatusCode::UNAUTHORIZED);
}

// ── Logout ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn logout_clears_cookie() {
    let app = TestApp::new().await;
    let set_cookie = app.setup_and_login().await;
    let cookie = TestApp::cookie_value(&set_cookie);

    let resp = app
        .server
        .post("/auth/logout")
        .add_header(
            axum::http::header::COOKIE,
            axum::http::HeaderValue::from_str(&cookie).unwrap(),
        )
        .await;
    resp.assert_status_ok();

    let body: serde_json::Value = resp.json();
    assert_eq!(body["success"], true);

    // The response should set a cookie with Max-Age=0
    let logout_cookie = resp
        .headers()
        .get("set-cookie")
        .unwrap()
        .to_str()
        .unwrap();
    assert!(logout_cookie.contains("Max-Age=0"));
}

// ── Me ─────────────────────────────────────────────────────────────────

#[tokio::test]
async fn me_returns_user_when_authenticated() {
    let app = TestApp::new().await;
    let set_cookie = app.setup_and_login().await;
    let cookie = TestApp::cookie_value(&set_cookie);

    let resp = app
        .server
        .get("/auth/me")
        .add_header(
            axum::http::header::COOKIE,
            axum::http::HeaderValue::from_str(&cookie).unwrap(),
        )
        .await;
    resp.assert_status_ok();

    let body: serde_json::Value = resp.json();
    assert_eq!(body["username"], "admin");
    assert_eq!(body["totp_enabled"], false);
}

#[tokio::test]
async fn me_rejects_unauthenticated() {
    let app = TestApp::new().await;

    let resp = app.server.get("/auth/me").await;
    resp.assert_status(axum::http::StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn me_rejects_invalid_token() {
    let app = TestApp::new().await;

    let resp = app
        .server
        .get("/auth/me")
        .add_header(
            axum::http::header::COOKIE,
            axum::http::HeaderValue::from_static("token=invalid.jwt.token"),
        )
        .await;
    resp.assert_status(axum::http::StatusCode::UNAUTHORIZED);
}

// ── Change Password ────────────────────────────────────────────────────

#[tokio::test]
async fn change_password_succeeds() {
    let app = TestApp::new().await;
    let set_cookie = app.setup_and_login().await;
    let cookie = TestApp::cookie_value(&set_cookie);

    let resp = app
        .server
        .post("/auth/change-password")
        .add_header(
            axum::http::header::COOKIE,
            axum::http::HeaderValue::from_str(&cookie).unwrap(),
        )
        .json(&json!({
            "current_password": "password123",
            "new_password": "newpassword456",
        }))
        .await;
    resp.assert_status_ok();

    let body: serde_json::Value = resp.json();
    assert_eq!(body["success"], true);

    // Should get a new token cookie
    assert!(resp.headers().get("set-cookie").is_some());

    // Old token should no longer work for login-sensitive operations,
    // but the new token should work.  Verify by logging in with new password.
    let resp = app
        .server
        .post("/auth/login")
        .json(&json!({
            "username": "admin",
            "password": "newpassword456",
        }))
        .await;
    resp.assert_status_ok();
}

#[tokio::test]
async fn change_password_rejects_wrong_current() {
    let app = TestApp::new().await;
    let set_cookie = app.setup_and_login().await;
    let cookie = TestApp::cookie_value(&set_cookie);

    let resp = app
        .server
        .post("/auth/change-password")
        .add_header(
            axum::http::header::COOKIE,
            axum::http::HeaderValue::from_str(&cookie).unwrap(),
        )
        .json(&json!({
            "current_password": "wrongcurrent",
            "new_password": "newpassword456",
        }))
        .await;
    resp.assert_status(axum::http::StatusCode::BAD_REQUEST);

    let body: serde_json::Value = resp.json();
    assert_eq!(body["code"], "PASSWORD_INCORRECT");
}

#[tokio::test]
async fn change_password_rejects_short_new_password() {
    let app = TestApp::new().await;
    let set_cookie = app.setup_and_login().await;
    let cookie = TestApp::cookie_value(&set_cookie);

    let resp = app
        .server
        .post("/auth/change-password")
        .add_header(
            axum::http::header::COOKIE,
            axum::http::HeaderValue::from_str(&cookie).unwrap(),
        )
        .json(&json!({
            "current_password": "password123",
            "new_password": "short",
        }))
        .await;
    resp.assert_status(axum::http::StatusCode::BAD_REQUEST);

    let body: serde_json::Value = resp.json();
    assert_eq!(body["code"], "PASSWORD_TOO_SHORT");
}

#[tokio::test]
async fn change_password_rejects_unauthenticated() {
    let app = TestApp::new().await;

    let resp = app
        .server
        .post("/auth/change-password")
        .json(&json!({
            "current_password": "password123",
            "new_password": "newpassword456",
        }))
        .await;
    resp.assert_status(axum::http::StatusCode::UNAUTHORIZED);
}
