//! Live tests for the remote sync providers against real servers.
//!
//! Ignored by default. Start test servers and run:
//!
//! ```sh
//! # WebDAV and SFTP with rclone
//! podman run -d --rm --name dm-webdav -p 18080:8080 -v /tmp/dav:/data rclone/rclone serve webdav /data --addr :8080 --user u --pass p
//! podman run -d --rm --name dm-sftp -p 12222:2222 -v /tmp/sftp:/data rclone/rclone serve sftp /data --addr :2222 --user u --pass p
//!
//! # S3 with SeaweedFS (rclone's S3 server can fail on the multipart upload
//! # this test makes). Without an S3 config it accepts any access key.
//! podman run -d --rm --name dm-s3 -p 19000:8333 docker.io/chrislusf/seaweedfs server -s3 -dir=/data
//! podman exec dm-s3 sh -c 'echo "s3.bucket.create -name testbucket" | weed shell'
//!
//! cargo test --test providers_live -- --ignored --test-threads=1
//! ```
//!
//! `DM_WEBDAV_ROOT` / `DM_SFTP_ROOT` / `DM_S3_ROOT` (the host directory the
//! server stores files in, e.g. `/tmp/dav`) add checks of the files on disk.
//! Leave `DM_S3_ROOT` unset for SeaweedFS: it does not store objects as plain files.

use dynamight::models::{CredentialData, DestinationConfig, SyncOptions};
use dynamight::services::providers::{create_provider, SyncContext, SyncResult};
use std::path::Path;
use std::sync::Arc;

async fn logs_db() -> sqlx::SqlitePool {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    dynamight::db::run_logs_migrations(&pool).await.unwrap();
    pool
}

fn write(path: &Path, content: &[u8]) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, content).unwrap();
}

/// Source tree with special characters, a nested directory and an excluded file.
fn make_source(root: &Path, big_file_mb: usize) -> String {
    let src = root.join("photos");
    write(&src.join("a.txt"), b"alpha");
    write(&src.join("sp ace #1 100%.txt"), b"special");
    write(&src.join("nested/deep/b.txt"), b"bravo");
    write(&src.join("skip.tmp"), b"excluded");
    if big_file_mb > 0 {
        let data: Vec<u8> = (0..big_file_mb * 1024 * 1024).map(|i| (i % 251) as u8).collect();
        write(&src.join("big.bin"), &data);
    }
    src.to_string_lossy().to_string()
}

async fn run(
    destination: &DestinationConfig,
    credential: &CredentialData,
    source: &str,
    delete: bool,
) -> SyncResult {
    let (tx, _) = tokio::sync::broadcast::channel(1000);
    let db = logs_db().await;
    let ctx = Arc::new(SyncContext {
        run_id: 1,
        source_dirs: vec![source.to_string()],
        destination: destination.clone(),
        options: SyncOptions {
            delete_extraneous: delete,
            exclude_patterns: vec!["*.tmp".to_string()],
            ..Default::default()
        },
        credential: Some(credential.clone()),
        log_sender: tx,
        log_writer: dynamight::services::LogWriter::spawn(db.clone()),
        is_cancelled: Arc::new(|_| false),
        oauth: None,
    });
    let provider = create_provider(destination);
    let result = provider.sync(ctx).await.expect("sync failed");

    let rows: Vec<(String, String)> =
        sqlx::query_as("SELECT level, message FROM log_entries ORDER BY id").fetch_all(&db).await.unwrap();
    for (level, message) in rows {
        if level != "info" {
            eprintln!("[{}] {}", level, message);
        }
    }
    result
}

/// Full cycle: initial upload, no-op rerun, change + delete with mirror mode.
async fn exercise(destination: DestinationConfig, credential: CredentialData, big_file_mb: usize, expect_files: &[&str], remote_root: Option<&Path>) {
    let tmp = tempfile::tempdir().unwrap();
    let source = make_source(tmp.path(), big_file_mb);

    let first = run(&destination, &credential, &source, false).await;
    assert_eq!(first.error_count, 0, "first run errors");
    let initial = 3 + usize::from(big_file_mb > 0);
    assert_eq!(first.files_transferred as usize, initial, "first run uploads");

    if let Some(root) = remote_root {
        for f in expect_files {
            assert!(root.join(f).exists(), "missing on remote: {}", f);
        }
        assert!(!root.join("photos/skip.tmp").exists(), "excluded file uploaded");
        if big_file_mb > 0 {
            let len = std::fs::metadata(root.join("photos/big.bin")).unwrap().len();
            assert_eq!(len as usize, big_file_mb * 1024 * 1024, "big file size");
        }
    }

    let second = run(&destination, &credential, &source, false).await;
    assert_eq!(second.error_count, 0, "second run errors");
    assert_eq!(second.files_transferred, 0, "unchanged files were uploaded again");

    // Change one file, remove another, mirror deletes
    std::fs::write(Path::new(&source).join("a.txt"), b"alpha changed").unwrap();
    std::fs::remove_dir_all(Path::new(&source).join("nested")).unwrap();
    let third = run(&destination, &credential, &source, true).await;
    assert_eq!(third.error_count, 0, "third run errors");
    assert_eq!(third.files_transferred, 1, "changed file not uploaded");
    assert!(third.files_deleted >= 1, "removed file not deleted");

    if let Some(root) = remote_root {
        assert_eq!(std::fs::read(root.join("photos/a.txt")).unwrap(), b"alpha changed");
        assert!(!root.join("photos/nested/deep/b.txt").exists(), "deleted file still on remote");
    }
}

fn remote_root(var: &str) -> Option<std::path::PathBuf> {
    std::env::var(var).ok().map(std::path::PathBuf::from)
}

#[tokio::test]
#[ignore]
async fn webdav_live() {
    let destination = DestinationConfig::WebDav {
        url: std::env::var("DM_WEBDAV_URL").unwrap_or_else(|_| "http://127.0.0.1:18080".into()),
        remote_path: "/backups/dynamight".into(),
    };
    let credential = CredentialData::WebDav { username: "u".into(), password: "p".into() };
    let root = remote_root("DM_WEBDAV_ROOT").map(|r| r.join("backups/dynamight"));
    exercise(destination, credential, 12, &["photos/a.txt", "photos/sp ace #1 100%.txt", "photos/nested/deep/b.txt"], root.as_deref()).await;
}

#[tokio::test]
#[ignore]
async fn sftp_live() {
    let mut destination = DestinationConfig::Sftp {
        host: "127.0.0.1".into(),
        port: std::env::var("DM_SFTP_PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(12222),
        username: "u".into(),
        remote_path: "/backups/dynamight".into(),
        key_based_auth: false,
        host_key_fingerprint: None,
    };
    let credential = CredentialData::Sftp { password: Some("p".into()), private_key: None, passphrase: None };

    // Trust on first use: get the fingerprint the way the UI does
    let test = create_provider(&destination).test_connection(&destination, Some(&credential)).await.unwrap();
    let fp = test.host_key_fingerprint.expect("no fingerprint");
    assert!(!fp.ends_with('='), "fingerprint should be in OpenSSH format: {}", fp);
    if let DestinationConfig::Sftp { host_key_fingerprint, .. } = &mut destination {
        *host_key_fingerprint = Some(fp);
    }

    let root = remote_root("DM_SFTP_ROOT").map(|r| r.join("backups/dynamight"));
    exercise(destination, credential, 12, &["photos/a.txt", "photos/sp ace #1 100%.txt", "photos/nested/deep/b.txt"], root.as_deref()).await;
}

#[tokio::test]
#[ignore]
async fn s3_live() {
    let destination = DestinationConfig::S3 {
        bucket: "testbucket".into(),
        prefix: "backups".into(),
        region: "us-east-1".into(),
        endpoint: Some(std::env::var("DM_S3_ENDPOINT").unwrap_or_else(|_| "http://127.0.0.1:19000".into())),
        storage_class: None,
    };
    let credential = CredentialData::S3 { access_key_id: "AKID".into(), secret_access_key: "SECRETKEY".into() };

    let test = create_provider(&destination).test_connection(&destination, Some(&credential)).await;
    assert!(test.is_ok(), "connection test failed: {:?}", test.err());

    // 70 MiB is above the multipart threshold
    let root = remote_root("DM_S3_ROOT").map(|r| r.join("testbucket/backups"));
    exercise(destination, credential, 70, &["photos/a.txt", "photos/sp ace #1 100%.txt", "photos/nested/deep/b.txt"], root.as_deref()).await;
}
