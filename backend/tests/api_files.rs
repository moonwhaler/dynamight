//! Integration tests for the file manager endpoints (/files/*).
//! The test app allows browsing /tmp, so every test works in its own
//! folder below /tmp.

mod helpers;

use axum::http::{header, HeaderValue, StatusCode};
use helpers::TestApp;
use serde_json::{json, Value};
use std::path::Path;

fn cookie_header(cookie: &str) -> HeaderValue {
    HeaderValue::from_str(cookie).unwrap()
}

fn workdir() -> tempfile::TempDir {
    tempfile::tempdir_in("/tmp").unwrap()
}

fn s(p: &Path) -> String {
    p.to_string_lossy().to_string()
}

#[tokio::test]
async fn raw_serves_ranges_and_downloads() {
    let app = TestApp::new().await;
    let cookie = TestApp::cookie_value(&app.setup_and_login().await);
    let dir = workdir();
    let file = dir.path().join("data.bin");
    std::fs::write(&file, b"0123456789").unwrap();

    let resp = app
        .server
        .get("/files/raw")
        .add_query_param("path", s(&file))
        .add_header(header::COOKIE, cookie_header(&cookie))
        .add_header(header::RANGE, HeaderValue::from_static("bytes=2-5"))
        .await;
    resp.assert_status(StatusCode::PARTIAL_CONTENT);
    assert_eq!(resp.as_bytes().as_ref(), b"2345");
    let disposition = resp.headers()[header::CONTENT_DISPOSITION].to_str().unwrap().to_string();
    assert!(disposition.starts_with("attachment"), "{disposition}");

    // Outside the allowed paths
    app.server
        .get("/files/raw")
        .add_query_param("path", "/etc/hostname")
        .add_header(header::COOKIE, cookie_header(&cookie))
        .await
        .assert_status(StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn raw_inline_never_runs_html() {
    let app = TestApp::new().await;
    let cookie = TestApp::cookie_value(&app.setup_and_login().await);
    let dir = workdir();
    let html = dir.path().join("page.html");
    std::fs::write(&html, "<script>alert(1)</script>").unwrap();
    let svg = dir.path().join("icon.svg");
    std::fs::write(&svg, "<svg xmlns='http://www.w3.org/2000/svg'/>").unwrap();

    let get = |path: String| {
        app.server
            .get("/files/raw")
            .add_query_param("path", path)
            .add_query_param("disposition", "inline")
            .add_header(header::COOKIE, cookie_header(&cookie))
    };

    let resp = get(s(&html)).await;
    resp.assert_status_ok();
    assert_eq!(resp.headers()[header::CONTENT_TYPE], "text/plain; charset=utf-8");
    assert!(resp.headers()[header::CONTENT_SECURITY_POLICY].to_str().unwrap().starts_with("sandbox"));
    assert!(resp.headers()[header::CONTENT_DISPOSITION].to_str().unwrap().starts_with("inline"));

    let resp = get(s(&svg)).await;
    assert_eq!(resp.headers()[header::CONTENT_TYPE], "image/svg+xml");
    assert!(resp.headers()[header::CONTENT_SECURITY_POLICY].to_str().unwrap().starts_with("sandbox"));
}

#[tokio::test]
async fn trash_and_restore() {
    let app = TestApp::new().await;
    let cookie = TestApp::cookie_value(&app.setup_and_login().await);
    let dir = workdir();
    let file = dir.path().join("report.txt");
    std::fs::write(&file, b"hello").unwrap();

    let resp = app
        .server
        .post("/files/trash")
        .add_header(header::COOKIE, cookie_header(&cookie))
        .json(&json!({ "paths": [s(&file), "/etc/hostname", s(&dir.path().join("missing"))] }))
        .await;
    resp.assert_status_ok();
    let body: Value = resp.json();
    assert_eq!(body["moved"].as_array().unwrap().len(), 1);
    let reasons: Vec<&str> = body["failed"].as_array().unwrap().iter().map(|f| f["reason"].as_str().unwrap()).collect();
    assert_eq!(reasons, vec!["not_allowed", "not_found"]);
    assert!(!file.exists());
    let item = body["moved"][0].clone();
    assert_eq!(item["original_path"], s(&dir.path().canonicalize().unwrap().join("report.txt")));

    // The trash folder is hidden from the listing
    let listing: Value = app
        .server
        .get("/system/browse")
        .add_query_param("path", "/tmp")
        .add_header(header::COOKIE, cookie_header(&cookie))
        .await
        .json();
    assert!(listing["entries"].as_array().unwrap().iter().all(|e| e["name"] != ".dynamight-trash"));

    // Listed in the trash
    let trash: Value = app.server.get("/files/trash").add_header(header::COOKIE, cookie_header(&cookie)).await.json();
    assert!(trash["items"].as_array().unwrap().iter().any(|i| i["id"] == item["id"]));

    // A new file took the name: conflict first, then keep both
    std::fs::write(&file, b"new").unwrap();
    let refs = json!([{ "root": item["root"], "id": item["id"] }]);
    let body: Value = app
        .server
        .post("/files/trash/restore")
        .add_header(header::COOKIE, cookie_header(&cookie))
        .json(&json!({ "items": refs }))
        .await
        .json();
    assert_eq!(body["conflicts"].as_array().unwrap().len(), 1);
    let body: Value = app
        .server
        .post("/files/trash/restore")
        .add_header(header::COOKIE, cookie_header(&cookie))
        .json(&json!({ "items": refs, "keep_both": true }))
        .await
        .json();
    assert_eq!(body["restored"].as_array().unwrap().len(), 1);
    assert_eq!(std::fs::read(dir.path().join("report (1).txt")).unwrap(), b"hello");
    assert_eq!(std::fs::read(&file).unwrap(), b"new");
}

#[tokio::test]
async fn trash_refuses_allowed_root_and_unknown_refs() {
    let app = TestApp::new().await;
    let cookie = TestApp::cookie_value(&app.setup_and_login().await);

    let body: Value = app
        .server
        .post("/files/trash")
        .add_header(header::COOKIE, cookie_header(&cookie))
        .json(&json!({ "paths": ["/tmp"] }))
        .await
        .json();
    // The allowed root itself is never moved (its parent is not even allowed)
    assert!(body["moved"].as_array().unwrap().is_empty());
    assert_eq!(body["failed"][0]["reason"], "not_allowed");
    assert!(Path::new("/tmp").is_dir());

    app.server
        .post("/files/trash/restore")
        .add_header(header::COOKIE, cookie_header(&cookie))
        .json(&json!({ "items": [{ "root": "/etc", "id": "0123456789abcdef0123456789abcdef" }] }))
        .await
        .assert_status(StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn purge_needs_the_password_check() {
    let app = TestApp::new().await;
    let cookie = TestApp::cookie_value(&app.setup_and_login().await);
    let dir = workdir();
    let file = dir.path().join("old.txt");
    std::fs::write(&file, b"x").unwrap();
    let body: Value = app
        .server
        .post("/files/trash")
        .add_header(header::COOKIE, cookie_header(&cookie))
        .json(&json!({ "paths": [s(&file)] }))
        .await
        .json();
    let item = body["moved"][0].clone();
    let purge = json!({ "items": [{ "root": item["root"], "id": item["id"] }] });

    let resp = app
        .server
        .delete("/files/trash")
        .add_header(header::COOKIE, cookie_header(&cookie))
        .json(&purge)
        .await;
    assert_eq!(resp.json::<Value>()["code"], "DELETE_VERIFICATION_REQUIRED");

    app.server
        .post("/system/verify-delete-access")
        .add_header(header::COOKIE, cookie_header(&cookie))
        .json(&json!({ "password": "password123" }))
        .await
        .assert_status_ok();
    let body: Value = app
        .server
        .delete("/files/trash")
        .add_header(header::COOKIE, cookie_header(&cookie))
        .json(&purge)
        .await
        .json();
    assert_eq!(body["purged"], 1);
    let root = Path::new(item["root"].as_str().unwrap());
    assert!(!root.join(item["id"].as_str().unwrap()).exists());
}

#[tokio::test]
async fn dir_sizes_stream_one_line_per_folder() {
    let app = TestApp::new().await;
    let cookie = TestApp::cookie_value(&app.setup_and_login().await);
    let dir = workdir();
    std::fs::create_dir_all(dir.path().join("a/deep")).unwrap();
    std::fs::write(dir.path().join("a/deep/f"), vec![0u8; 1000]).unwrap();
    std::fs::create_dir(dir.path().join("b")).unwrap();

    let resp = app
        .server
        .post("/files/dir-sizes")
        .add_header(header::COOKIE, cookie_header(&cookie))
        .json(&json!({ "paths": [s(&dir.path().join("a")), s(&dir.path().join("b")), "/etc"] }))
        .await;
    resp.assert_status_ok();
    assert_eq!(resp.headers()[header::CONTENT_TYPE], "application/x-ndjson");
    let mut sizes: Vec<(String, u64)> = resp
        .text()
        .lines()
        .map(|l| {
            let v: Value = serde_json::from_str(l).unwrap();
            (v["path"].as_str().unwrap().to_string(), v["size"].as_u64().unwrap())
        })
        .collect();
    sizes.sort();
    assert_eq!(sizes, vec![(s(&dir.path().join("a")), 1000), (s(&dir.path().join("b")), 0)]);
}

#[tokio::test]
async fn places_list_allowed_paths_with_space() {
    let app = TestApp::new().await;
    let cookie = TestApp::cookie_value(&app.setup_and_login().await);
    let body: Value = app.server.get("/files/places").add_header(header::COOKIE, cookie_header(&cookie)).await.json();
    let places = body["places"].as_array().unwrap();
    assert_eq!(places[0]["path"], "/tmp");
    assert!(places[0]["total"].as_u64().unwrap() > 0);
    assert!(body["drives"].is_array());
    assert!(body["trash_items"].is_u64());
}

#[tokio::test]
async fn rename_never_replaces() {
    let app = TestApp::new().await;
    let cookie = TestApp::cookie_value(&app.setup_and_login().await);
    let dir = workdir();
    std::fs::write(dir.path().join("a.txt"), b"a").unwrap();
    std::fs::write(dir.path().join("b.txt"), b"b").unwrap();
    let rename = |path: String, name: &str| {
        app.server
            .post("/files/rename")
            .add_header(header::COOKIE, cookie_header(&cookie))
            .json(&json!({ "path": path, "new_name": name }))
    };

    rename(s(&dir.path().join("a.txt")), "b.txt").await.assert_status(StatusCode::CONFLICT);
    rename(s(&dir.path().join("a.txt")), "../x").await.assert_status(StatusCode::BAD_REQUEST);
    rename(s(&dir.path().join("a.txt")), ".dynamight-trash").await.assert_status(StatusCode::BAD_REQUEST);
    rename(s(&dir.path().join("a.txt")), "c.txt").await.assert_status_ok();
    assert_eq!(std::fs::read(dir.path().join("c.txt")).unwrap(), b"a");
    assert_eq!(std::fs::read(dir.path().join("b.txt")).unwrap(), b"b");
}

#[tokio::test]
async fn copy_task_runs_and_reports() {
    let app = TestApp::new().await;
    let cookie = TestApp::cookie_value(&app.setup_and_login().await);
    let dir = workdir();
    std::fs::create_dir_all(dir.path().join("src/sub")).unwrap();
    std::fs::write(dir.path().join("src/sub/f"), b"data").unwrap();
    std::fs::create_dir(dir.path().join("dst")).unwrap();

    // Into itself is refused
    app.server
        .post("/files/tasks")
        .add_header(header::COOKIE, cookie_header(&cookie))
        .json(&json!({ "kind": "copy", "sources": [s(&dir.path().join("src"))], "target": s(&dir.path().join("src/sub")) }))
        .await
        .assert_status(StatusCode::BAD_REQUEST);

    let resp = app
        .server
        .post("/files/tasks")
        .add_header(header::COOKIE, cookie_header(&cookie))
        .json(&json!({ "kind": "copy", "sources": [s(&dir.path().join("src"))], "target": s(&dir.path().join("dst")) }))
        .await;
    resp.assert_status(StatusCode::ACCEPTED);
    let id = resp.json::<Value>()["id"].as_u64().unwrap();

    let mut state = String::new();
    for _ in 0..100 {
        let tasks: Value = app.server.get("/files/tasks").add_header(header::COOKIE, cookie_header(&cookie)).await.json();
        let task = tasks.as_array().unwrap().iter().find(|t| t["id"] == id).unwrap().clone();
        state = task["state"].as_str().unwrap().to_string();
        if state == "done" {
            assert_eq!(task["done_bytes"], 4);
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    assert_eq!(state, "done");
    assert_eq!(std::fs::read(dir.path().join("dst/src/sub/f")).unwrap(), b"data");
}

#[tokio::test]
async fn folders_download_as_one_archive() {
    let app = TestApp::new().await;
    let cookie = TestApp::cookie_value(&app.setup_and_login().await);
    let dir = workdir();
    std::fs::create_dir_all(dir.path().join("photos/2024")).unwrap();
    std::fs::write(dir.path().join("photos/2024/a.jpg"), b"jpg").unwrap();
    std::fs::write(dir.path().join("-notes.txt"), b"n").unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink("/etc/hostname", dir.path().join("photos/outside")).unwrap();

    let prepared: Value = app
        .server
        .post("/files/archive")
        .add_header(header::COOKIE, cookie_header(&cookie))
        .json(&json!({ "paths": [s(&dir.path().join("photos")), s(&dir.path().join("-notes.txt"))] }))
        .await
        .json();
    let url = prepared["url"].as_str().unwrap().trim_start_matches("/api").to_string();
    let resp = app.server.get(&url).add_header(header::COOKIE, cookie_header(&cookie)).await;
    resp.assert_status_ok();
    assert!(prepared["filename"].as_str().unwrap().ends_with(".tar"));
    let out = dir.path().join("out.tar");
    std::fs::write(&out, resp.as_bytes()).unwrap();
    let listing = std::process::Command::new("tar").arg("-tvf").arg(&out).output().unwrap();
    let names = String::from_utf8_lossy(&listing.stdout).to_string();
    assert!(names.contains("photos/2024/a.jpg"), "{names}");
    assert!(names.contains("-notes.txt"), "{names}");
    // The link is stored as a link, not as the file it points to
    assert!(names.contains("photos/outside -> /etc/hostname"), "{names}");
}

#[tokio::test]
async fn text_and_thumbnails() {
    let app = TestApp::new().await;
    let cookie = TestApp::cookie_value(&app.setup_and_login().await);
    let dir = workdir();
    std::fs::write(dir.path().join("log.txt"), "line 1\nline 2\n").unwrap();
    image::RgbImage::from_pixel(800, 400, image::Rgb([10, 200, 10])).save(dir.path().join("pic.png")).unwrap();

    let body: Value = app
        .server
        .get("/files/text")
        .add_query_param("path", s(&dir.path().join("log.txt")))
        .add_header(header::COOKIE, cookie_header(&cookie))
        .await
        .json();
    assert_eq!(body["text"], "line 1\nline 2\n");
    assert_eq!(body["truncated"], false);

    let resp = app
        .server
        .get("/files/thumb")
        .add_query_param("path", s(&dir.path().join("pic.png")))
        .add_header(header::COOKIE, cookie_header(&cookie))
        .await;
    resp.assert_status_ok();
    assert_eq!(resp.headers()[header::CONTENT_TYPE], "image/jpeg");
    let thumb = image::load_from_memory(resp.as_bytes()).unwrap();
    assert_eq!(thumb.width(), 256);

    app.server
        .get("/files/thumb")
        .add_query_param("path", s(&dir.path().join("log.txt")))
        .add_header(header::COOKIE, cookie_header(&cookie))
        .await
        .assert_status(StatusCode::NOT_FOUND);
}

async fn start_upload(app: &TestApp, cookie: &str, body: Value) -> axum_test::TestResponse {
    app.server.post("/files/uploads").add_header(header::COOKIE, cookie_header(cookie)).json(&body).await
}

async fn put_chunk(app: &TestApp, cookie: &str, id: &str, start: usize, data: &[u8], total: usize) -> axum_test::TestResponse {
    app.server
        .put(&format!("/files/uploads/{id}"))
        .add_header(header::COOKIE, cookie_header(cookie))
        .add_header(
            header::CONTENT_RANGE,
            HeaderValue::from_str(&format!("bytes {}-{}/{}", start, start + data.len() - 1, total)).unwrap(),
        )
        .bytes(data.to_vec().into())
        .await
}

#[tokio::test]
async fn upload_in_chunks_and_resume() {
    let app = TestApp::new().await;
    let cookie = TestApp::cookie_value(&app.setup_and_login().await);
    let dir = workdir();
    let data: Vec<u8> = (0..300u32).map(|i| (i % 251) as u8).collect();
    let body = json!({ "dir": s(dir.path()), "rel_dir": "album/2024", "name": "song.mp3", "size": 300, "fingerprint": "song.mp3|300|1", "mtime": 1_700_000_000_000i64 });

    let first: Value = start_upload(&app, &cookie, body.clone()).await.json();
    let id = first["id"].as_str().unwrap().to_string();
    assert_eq!(first["offset"], 0);

    put_chunk(&app, &cookie, &id, 0, &data[..100], 300).await.assert_status_ok();
    // A chunk from the wrong place is refused with the real offset
    let wrong = put_chunk(&app, &cookie, &id, 200, &data[200..], 300).await;
    wrong.assert_status(StatusCode::CONFLICT);
    assert_eq!(wrong.json::<Value>()["params"]["offset"], 100);

    // Starting the same file again resumes it
    let again: Value = start_upload(&app, &cookie, body).await.json();
    assert_eq!(again["id"], id.as_str());
    assert_eq!(again["offset"], 100);
    let status: Value = app.server.get(&format!("/files/uploads/{id}")).add_header(header::COOKIE, cookie_header(&cookie)).await.json();
    assert_eq!(status["offset"], 100);

    // The part file is hidden while uploading
    let listing: Value = app
        .server
        .get("/system/browse")
        .add_query_param("path", s(&dir.path().join("album/2024")))
        .add_header(header::COOKIE, cookie_header(&cookie))
        .await
        .json();
    assert!(listing["entries"].as_array().unwrap().is_empty());

    let done: Value = put_chunk(&app, &cookie, &id, 100, &data[100..], 300).await.json();
    assert_eq!(done["done"], true);
    let target = dir.path().join("album/2024/song.mp3");
    assert_eq!(std::fs::read(&target).unwrap(), data);
    let mtime = std::fs::metadata(&target).unwrap().modified().unwrap();
    assert_eq!(mtime.duration_since(std::time::UNIX_EPOCH).unwrap().as_secs(), 1_700_000_000);
    app.server
        .get(&format!("/files/uploads/{id}"))
        .add_header(header::COOKIE, cookie_header(&cookie))
        .await
        .assert_status(StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn upload_never_replaces_without_a_choice() {
    let app = TestApp::new().await;
    let cookie = TestApp::cookie_value(&app.setup_and_login().await);
    let dir = workdir();
    std::fs::write(dir.path().join("a.txt"), b"old").unwrap();
    let body = json!({ "dir": s(dir.path()), "name": "a.txt", "size": 3, "fingerprint": "a|3|1" });

    let taken = start_upload(&app, &cookie, body.clone()).await;
    taken.assert_status(StatusCode::CONFLICT);
    assert_eq!(taken.json::<Value>()["params"]["size"], 3);

    let mut keep = body.clone();
    keep["on_conflict"] = json!("keep_both");
    let started: Value = start_upload(&app, &cookie, keep).await.json();
    let id = started["id"].as_str().unwrap();
    put_chunk(&app, &cookie, id, 0, b"new", 3).await.assert_status_ok();
    assert_eq!(std::fs::read(dir.path().join("a.txt")).unwrap(), b"old");
    assert_eq!(std::fs::read(dir.path().join("a (1).txt")).unwrap(), b"new");

    // Names with folders in them are refused
    let mut bad = body;
    bad["name"] = json!("../x");
    start_upload(&app, &cookie, bad).await.assert_status(StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn empty_files_and_cancel() {
    let app = TestApp::new().await;
    let cookie = TestApp::cookie_value(&app.setup_and_login().await);
    let dir = workdir();

    let empty: Value = start_upload(&app, &cookie, json!({ "dir": s(dir.path()), "name": "empty", "size": 0, "fingerprint": "e" }))
        .await
        .json();
    assert!(empty["path"].as_str().unwrap().ends_with("/empty"));
    assert_eq!(std::fs::metadata(dir.path().join("empty")).unwrap().len(), 0);

    let started: Value = start_upload(&app, &cookie, json!({ "dir": s(dir.path()), "name": "big", "size": 10, "fingerprint": "b" }))
        .await
        .json();
    let id = started["id"].as_str().unwrap();
    put_chunk(&app, &cookie, id, 0, b"12345", 10).await.assert_status_ok();
    app.server
        .delete(&format!("/files/uploads/{id}"))
        .add_header(header::COOKIE, cookie_header(&cookie))
        .await
        .assert_status_ok();
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1, "only the empty file is left");
}

#[tokio::test]
async fn stale_uploads_are_removed() {
    let app = TestApp::new().await;
    let cookie = TestApp::cookie_value(&app.setup_and_login().await);
    let dir = workdir();
    let started: Value = start_upload(&app, &cookie, json!({ "dir": s(dir.path()), "name": "old.bin", "size": 10, "fingerprint": "o" }))
        .await
        .json();
    let id = started["id"].as_str().unwrap().to_string();
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);

    sqlx::query("UPDATE uploads SET updated_at = datetime('now', '-2 days') WHERE id = ?")
        .bind(&id)
        .execute(&app.db)
        .await
        .unwrap();
    dynamight::handlers::uploads::remove_stale(&app.db).await;
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    app.server
        .get(&format!("/files/uploads/{id}"))
        .add_header(header::COOKIE, cookie_header(&cookie))
        .await
        .assert_status(StatusCode::NOT_FOUND);
}
