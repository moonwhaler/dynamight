//! Service for compressing source directories before transfer.
//!
//! Each source directory is individually archived (tar.gz, tar, or zip) into a
//! shared staging directory (`CompressDirsOptions::staging_path`). Archive filenames
//! embed the job ID so multiple jobs can safely share the same staging path without
//! collision. After archiving, the staging directory is passed to the provider as
//! the effective source for the transfer.

use crate::models::{CompressDirsOptions, CompressFormat};
use anyhow::Context;
use chrono::Local;
use std::path::{Path, PathBuf};

/// Sanitize a directory name to be safe as an archive filename segment.
///
/// Replaces any character that is not `[a-zA-Z0-9_-]` with `'_'`, then trims
/// leading and trailing underscores so that directory names like `.dotnet` become
/// `dotnet` rather than `_dotnet` (which would produce double underscores when
/// joined with other segments).
///
/// This is the **single source of truth** used by both `generate_archive_name()`
/// and `cleanup_old_archives()` to ensure consistent naming.
pub fn sanitize_dir_name(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim_matches('_')
        .to_string()
}

/// Get the file extension for a given archive format and options.
///
/// `store_only` affects tar-based formats: TarGz in store-only mode produces a
/// plain `.tar` file (no gzip compression). Zip always uses `.zip` regardless.
///
/// `encrypted` adds a `.enc` suffix (openssl AES-256-CBC). This applies to zip
/// too: zip's built-in encryption is weak and needs the password on the
/// command line, so password-protected zips are encrypted with openssl as well.
/// Every extension `format_extension` can return. Older versions also wrote
/// password-protected zips as plain `.zip` (zip -P), which is included.
pub const ARCHIVE_EXTENSIONS: &[&str] = &["tar.gz", "tar.gz.enc", "tar", "tar.enc", "zip", "zip.enc"];

/// Glob patterns that match exactly the archives of one job: names end in
/// `_<job id>.<archive extension>`. Unlike `*_<id>.*`, these cannot match
/// another job's archive whose custom name happens to contain `_<id>.`.
pub fn job_archive_patterns(job_id: i64) -> Vec<String> {
    ARCHIVE_EXTENSIONS.iter().map(|ext| format!("*_{}.{}", job_id, ext)).collect()
}

pub fn format_extension(format: &CompressFormat, store_only: bool, encrypted: bool) -> &'static str {
    match format {
        CompressFormat::TarGz => match (store_only, encrypted) {
            (false, false) => "tar.gz",
            (false, true) => "tar.gz.enc",
            (true, false) => "tar",
            (true, true) => "tar.enc",
        },
        CompressFormat::Zip => {
            if encrypted {
                "zip.enc"
            } else {
                "zip"
            }
        }
    }
}

/// Generate the archive filename for a source directory.
///
/// Naming: `[<timestamp>_][custom_name_]<sanitized_dir_name>_<job_id>.<ext>`
///
/// The `job_id` suffix is always appended to the sanitized directory name,
/// ensuring archives from different jobs are unique even when stored in the
/// same staging directory.
///
/// The timestamp is always the **first** segment when enabled, making archives
/// sort chronologically by name and clearly marking when they were created.
pub fn generate_archive_name(dir_name: &str, opts: &CompressDirsOptions, job_id: i64) -> String {
    let sanitized = sanitize_dir_name(dir_name);
    let has_password = opts.password.as_deref().map(|p| !p.is_empty()).unwrap_or(false);
    let ext = format_extension(&opts.format, opts.store_only, has_password);

    let mut parts: Vec<String> = Vec::new();

    if opts.add_timestamp {
        let ts = Local::now().format("%Y-%m-%dT%H-%M-%S").to_string();
        parts.push(ts);
    }

    if let Some(ref custom) = opts.custom_name {
        if !custom.is_empty() {
            parts.push(custom.clone());
        }
    }

    parts.push(format!("{}_{}", sanitized, job_id));

    format!("{}.{}", parts.join("_"), ext)
}

/// Poll a spawned child process until it exits or cancellation is requested.
///
/// stderr is drained concurrently: if it is only read after exit, a process
/// that writes more than one pipe buffer of warnings blocks forever. Up to
/// `MAX_STDERR_LINES` lines are kept; they are logged through `log_fn` and the
/// last few are included in the error on failure.
///
/// `warning_codes` are non-zero exit codes that still mean the output is
/// usable (logged as a warning, not an error).
///
/// On failure or cancellation the `cleanup_path` file is removed. This helper
/// is called for both the archiving and encryption phases.
async fn run_and_poll(
    mut child: tokio::process::Child,
    cleanup_path: &Path,
    is_cancelled: &impl Fn() -> bool,
    cancel_msg: &'static str,
    warning_codes: &[i32],
    log_fn: &impl Fn(String),
) -> anyhow::Result<()> {
    const MAX_STDERR_LINES: usize = 50;

    let stderr_task = child.stderr.take().map(|stderr| {
        tokio::spawn(async move {
            use tokio::io::AsyncBufReadExt;
            let mut lines = tokio::io::BufReader::new(stderr).lines();
            let mut kept: std::collections::VecDeque<String> = std::collections::VecDeque::new();
            let mut total = 0usize;
            while let Ok(Some(line)) = lines.next_line().await {
                total += 1;
                if kept.len() == MAX_STDERR_LINES {
                    kept.pop_front();
                }
                kept.push_back(line);
            }
            (kept, total)
        })
    });

    let status: anyhow::Result<std::process::ExitStatus> = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Ok(status),
            Ok(None) => {
                if is_cancelled() {
                    let _ = child.kill().await;
                    let _ = child.wait().await;
                    let _ = tokio::fs::remove_file(cleanup_path).await;
                    return Err(anyhow::anyhow!("{}", cancel_msg));
                }
                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
            }
            Err(e) => break Err(anyhow::anyhow!("Failed to check process status: {}", e)),
        }
    };

    let (stderr_lines, stderr_total) = match stderr_task {
        Some(task) => task.await.unwrap_or_default(),
        None => Default::default(),
    };

    let result = match status {
        Ok(status) if status.success() => Ok(()),
        Ok(status) => {
            let code = status.code().unwrap_or(-1);
            if warning_codes.contains(&code) {
                log_fn(format!("Warning: process exited with code {} (archive was still created)", code));
                Ok(())
            } else {
                let tail: Vec<&str> = stderr_lines.iter().rev().take(5).rev().map(String::as_str).collect();
                Err(anyhow::anyhow!(
                    "Process exited with code {}{}",
                    code,
                    if tail.is_empty() { String::new() } else { format!(": {}", tail.join(" | ")) }
                ))
            }
        }
        Err(e) => Err(e),
    };

    for line in &stderr_lines {
        log_fn(line.clone());
    }
    if stderr_total > stderr_lines.len() {
        log_fn(format!("({} earlier warning lines omitted)", stderr_total - stderr_lines.len()));
    }

    if result.is_err() {
        let _ = tokio::fs::remove_file(cleanup_path).await;
    }

    result
}

/// Convert absolute exclude paths to archive-relative paths for tar/zip.
///
/// Each entry in `exclude_paths` that is a strict child of `source_dir` is
/// converted to a path relative to the archive root (`dir_name/sub/path`).
/// Entries belonging to a different source directory are silently skipped —
/// they will be applied during their own `compress_directory` call.
///
/// Example:
///   source_dir  = /home/user/docs
///   dir_name    = "docs"
///   exclude     = /home/user/docs/cache  →  "docs/cache"
fn archive_relative_excludes(source_dir: &Path, dir_name: &str, exclude_paths: &[String]) -> Vec<String> {
    let prefix = format!("{}/", source_dir.display());
    exclude_paths
        .iter()
        .filter_map(|ex| {
            ex.strip_prefix(&prefix)
                .map(|within| format!("{}/{}", dir_name, within))
        })
        .collect()
}

/// Compress (or archive) a source directory into the staging directory.
///
/// Archives are stored directly in `opts.staging_path` (no subdirectory).
/// The `job_id` is embedded in every archive filename to prevent collisions
/// when multiple jobs share the same staging path.
///
/// Parameters:
/// - `source_dir`:    absolute path to the directory to compress
/// - `job_id`:        appended to archive filenames (e.g. `photos_42.tar.gz`)
/// - `run_id`:        used for unique temp file naming
/// - `opts`:          compression options
/// - `exclude_paths`: absolute paths to exclude from the archive; only strict
///   children of `source_dir` are applied — others are silently skipped
/// - `log_fn`:        called to emit log lines during compression
/// - `is_cancelled`:  checked every 500 ms; if it returns `true`, the
///   subprocess is killed and an error is returned
///
/// When `store_only` is set, files are archived without compression
/// (tar without `-z`, or `zip -0`).
///
/// When a password is set for tar-based formats, a two-step process is used:
/// 1. Archive to an intermediate temp file
/// 2. Encrypt with `openssl enc -aes-256-cbc -pbkdf2` (password via stdin)
///
/// The final output carries a `.enc` extension (e.g. `archive.tar.gz.enc`).
///
/// Returns the path to the created archive file.
pub async fn compress_directory(
    source_dir: &Path,
    job_id: i64,
    run_id: i64,
    opts: &CompressDirsOptions,
    exclude_paths: &[String],
    log_fn: impl Fn(String),
    is_cancelled: impl Fn() -> bool,
) -> anyhow::Result<PathBuf> {
    let staging_dir = PathBuf::from(&opts.staging_path);

    // Runtime overlap check: staging_dir must not overlap with source_dir.
    let staging_str = staging_dir.to_string_lossy();
    let source_str = source_dir.to_string_lossy();

    let staging_inside_source = staging_str.starts_with(source_str.as_ref())
        && (staging_str.len() == source_str.len()
            || staging_str[source_str.len()..].starts_with('/'));
    let source_inside_staging = source_str.starts_with(staging_str.as_ref())
        && (source_str.len() == staging_str.len()
            || source_str[staging_str.len()..].starts_with('/'));

    if staging_inside_source || source_inside_staging {
        anyhow::bail!(
            "Staging directory '{}' must not overlap with source directory '{}'",
            staging_dir.display(),
            source_dir.display()
        );
    }

    // Create the staging directory if it doesn't exist yet
    tokio::fs::create_dir_all(&staging_dir)
        .await
        .with_context(|| {
            format!(
                "Failed to create staging directory '{}'",
                staging_dir.display()
            )
        })?;

    let dir_name = source_dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("backup");

    // Compute archive-relative exclude paths for this source directory.
    // Paths belonging to other source directories are filtered out here.
    let rel_excludes = archive_relative_excludes(source_dir, dir_name, exclude_paths);

    let archive_name = generate_archive_name(dir_name, opts, job_id);
    let final_path = staging_dir.join(&archive_name);
    // Unique temp path per run to prevent conflicts between concurrent runs
    let tmp_path = staging_dir.join(format!("{}.{}.tmp", archive_name, run_id));

    let parent_dir = source_dir.parent().ok_or_else(|| {
        anyhow::anyhow!(
            "Source directory has no parent: {}",
            source_dir.display()
        )
    })?;

    let has_password = opts.password.as_deref().map(|p| !p.is_empty()).unwrap_or(false);

    log_fn(format!(
        "Archiving '{}' → {}",
        source_dir.display(),
        archive_name
    ));

    if !rel_excludes.is_empty() {
        log_fn(format!(
            "Excluding {} director{} from archive: {}",
            rel_excludes.len(),
            if rel_excludes.len() == 1 { "y" } else { "ies" },
            rel_excludes.join(", ")
        ));
    }

    // Phase 1: create the archive. With a password it goes to an intermediate
    // file first, which phase 2 encrypts with openssl.
    let raw_path = if has_password {
        staging_dir.join(format!("{}.inter.{}.tmp", archive_name, run_id))
    } else {
        tmp_path.clone()
    };

    match &opts.format {
        CompressFormat::Zip => {
            let mut cmd = tokio::process::Command::new("zip");
            if opts.store_only {
                cmd.arg("-0"); // store only, no compression
            }
            // -y stores symlinks as links instead of following them
            cmd.args(["-r", "-y"]);
            cmd.args([raw_path.to_str().unwrap_or_default(), dir_name]);
            // -x patterns must come AFTER the source argument (Info-ZIP requirement)
            for rel in &rel_excludes {
                cmd.args(["-x", rel.as_str()]);
                cmd.args(["-x", &format!("{}/*", rel)]);
            }
            let child = cmd
                .current_dir(parent_dir)
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .context("Failed to spawn zip. Ensure zip is installed.")?;

            // zip exit 18 = some files could not be read; the archive is still written
            run_and_poll(child, &raw_path, &is_cancelled, "Archiving cancelled", &[18], &log_fn).await?;
        }

        CompressFormat::TarGz => {
            // `-z` for gzip compression; omit for store-only (plain tar)
            let tar_flags = if opts.store_only { "-cpf" } else { "-czpf" };

            // --exclude flags must appear BEFORE the source dir argument (GNU tar requirement)
            let mut cmd = tokio::process::Command::new("tar");
            cmd.args([
                tar_flags,
                raw_path.to_str().unwrap_or_default(),
                "--numeric-owner",
                "-C",
                parent_dir.to_str().unwrap_or_default(),
            ]);
            for rel in &rel_excludes {
                cmd.arg(format!("--exclude={}", rel));
            }
            cmd.arg(dir_name);
            let child = cmd
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .context("Failed to spawn tar. Ensure GNU tar is installed (not BusyBox tar).")?;

            // GNU tar exit 1 = some files changed while being read; the archive is complete
            run_and_poll(child, &raw_path, &is_cancelled, "Archiving cancelled", &[1], &log_fn).await?;
        }
    }

    // Phase 2: encrypt with openssl AES-256-CBC. The password is passed via
    // stdin so it never shows up in the process list (zip -P would expose it).
    if has_password {
        let password = opts.password.as_deref().unwrap_or_default();
        log_fn("Encrypting archive (AES-256-CBC)".to_string());

        let enc_child = tokio::process::Command::new("openssl")
            .args([
                "enc",
                "-aes-256-cbc",
                "-pbkdf2",
                "-in",
                raw_path.to_str().unwrap_or_default(),
                "-out",
                tmp_path.to_str().unwrap_or_default(),
                "-pass",
                "stdin",
            ])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            .spawn();

        let mut enc_child = match enc_child {
            Ok(c) => c,
            Err(e) => {
                let _ = tokio::fs::remove_file(&raw_path).await;
                return Err(anyhow::Error::new(e).context("Failed to spawn openssl. Ensure openssl is installed."));
            }
        };

        // Write password to openssl's stdin and close the pipe
        if let Some(mut stdin) = enc_child.stdin.take() {
            use tokio::io::AsyncWriteExt;
            let _ = stdin.write_all(password.as_bytes()).await;
            // stdin is dropped here, closing the pipe and signalling EOF to openssl
        }

        let enc_result =
            run_and_poll(enc_child, &tmp_path, &is_cancelled, "Encryption cancelled", &[], &log_fn).await;

        // Always remove the intermediate unencrypted archive, even on success
        let _ = tokio::fs::remove_file(&raw_path).await;

        enc_result?;
    }

    // Atomic rename: temp → final path
    tokio::fs::rename(&tmp_path, &final_path)
        .await
        .with_context(|| {
            format!(
                "Failed to finalize archive at '{}'",
                final_path.display()
            )
        })?;

    log_fn(format!("Archive ready: {}", final_path.display()));

    Ok(final_path)
}

/// Remove the oldest archives in the staging directory for a given source
/// directory and job, keeping at most `max_count` archives.
///
/// Uses `sanitize_dir_name(dir_name)` and `job_id` — identical to
/// `generate_archive_name()` — so the match pattern is guaranteed to align
/// with the generated filenames.
///
/// Archive names follow `[TIMESTAMP_][custom_]sanitized_jobid.ext`, so the
/// `sanitized_jobid` segment is always the last before the extension. Matching
/// is done by suffix (`ends_with`) to handle both timestamped and
/// non-timestamped variants.
///
/// Returns the number of deleted archives.
pub fn cleanup_old_archives(
    staging_dir: &Path,
    dir_name: &str,
    custom_name: Option<&str>,
    max_count: u32,
    job_id: i64,
) -> anyhow::Result<u32> {
    // Archives of every format count: archives written before a settings
    // change (e.g. old password zips named `.zip`, or `.tar.gz` before a
    // switch to zip) belong to the same directory and are rotated out
    // instead of being kept forever.
    let sanitized = format!("{}_{}", sanitize_dir_name(dir_name), job_id);

    // The stem without any timestamp prefix: [custom_]sanitized_jobid
    let stem = match custom_name {
        Some(cn) if !cn.is_empty() => format!("{}_{}", cn, sanitized),
        _ => sanitized,
    };
    let is_archive_of_dir = |name: &str| {
        ARCHIVE_EXTENSIONS.iter().any(|ext| {
            let base_name = format!("{}.{}", stem, ext);
            // Exact base name (no timestamp) OR timestamp-prefixed variant
            name == base_name || name.ends_with(&format!("_{}", base_name))
        })
    };

    let entries = std::fs::read_dir(staging_dir).with_context(|| {
        format!(
            "Failed to read staging directory '{}'",
            staging_dir.display()
        )
    })?;

    let mut matching: Vec<(PathBuf, std::time::SystemTime)> = entries
        .filter_map(|e| e.ok())
        .filter(|e| {
            let name = e.file_name();
            is_archive_of_dir(&name.to_string_lossy())
        })
        .filter_map(|e| {
            let mtime = e.metadata().ok()?.modified().ok()?;
            Some((e.path(), mtime))
        })
        .collect();

    let count = matching.len() as u32;
    if count <= max_count {
        return Ok(0);
    }

    // Sort oldest first, then delete the excess
    matching.sort_by_key(|(_, mtime)| *mtime);
    let to_delete = (count - max_count) as usize;
    let mut deleted = 0u32;

    for (path, _) in matching.into_iter().take(to_delete) {
        if let Err(e) = std::fs::remove_file(&path) {
            tracing::warn!(
                "Failed to delete old archive '{}': {}",
                path.display(),
                e
            );
        } else {
            deleted += 1;
        }
    }

    Ok(deleted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{CompressDirsOptions, CompressFormat};

    fn default_opts() -> CompressDirsOptions {
        CompressDirsOptions {
            enabled: true,
            format: CompressFormat::TarGz,
            add_timestamp: false,
            custom_name: None,
            max_archives_per_dir: None,
            staging_path: "/tmp/staging".to_string(),
            password: None,
            store_only: false,
        }
    }

    // --- sanitize_dir_name tests ---

    #[test]
    fn sanitize_alphanumeric_passes_through() {
        assert_eq!(sanitize_dir_name("myDir123"), "myDir123");
    }

    #[test]
    fn sanitize_hyphens_and_underscores_preserved() {
        assert_eq!(sanitize_dir_name("my-dir_name"), "my-dir_name");
    }

    #[test]
    fn sanitize_spaces_become_underscore() {
        assert_eq!(sanitize_dir_name("my dir name"), "my_dir_name");
    }

    #[test]
    fn sanitize_dots_become_underscore() {
        assert_eq!(sanitize_dir_name("file.name.ext"), "file_name_ext");
    }

    #[test]
    fn sanitize_leading_trailing_underscores_trimmed() {
        assert_eq!(sanitize_dir_name("__hello__"), "hello");
    }

    #[test]
    fn sanitize_dotnet_becomes_dotnet() {
        assert_eq!(sanitize_dir_name(".dotnet"), "dotnet");
    }

    #[test]
    fn sanitize_empty_string() {
        assert_eq!(sanitize_dir_name(""), "");
    }

    #[test]
    fn sanitize_special_characters() {
        assert_eq!(sanitize_dir_name("a@b#c$d"), "a_b_c_d");
    }

    // --- format_extension tests ---

    #[test]
    fn format_extension_tar_gz() {
        assert_eq!(format_extension(&CompressFormat::TarGz, false, false), "tar.gz");
    }

    #[test]
    fn format_extension_tar_gz_encrypted() {
        assert_eq!(format_extension(&CompressFormat::TarGz, false, true), "tar.gz.enc");
    }

    #[test]
    fn format_extension_tar_store_only() {
        assert_eq!(format_extension(&CompressFormat::TarGz, true, false), "tar");
    }

    #[test]
    fn format_extension_tar_store_only_encrypted() {
        assert_eq!(format_extension(&CompressFormat::TarGz, true, true), "tar.enc");
    }

    #[test]
    fn format_extension_zip() {
        // Zip ignores store_only; encryption (openssl) adds .enc like tar
        assert_eq!(format_extension(&CompressFormat::Zip, false, false), "zip");
        assert_eq!(format_extension(&CompressFormat::Zip, true, false), "zip");
        assert_eq!(format_extension(&CompressFormat::Zip, false, true), "zip.enc");
        assert_eq!(format_extension(&CompressFormat::Zip, true, true), "zip.enc");
    }

    #[test]
    fn retention_counts_all_formats_of_one_dir_only() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        let base = std::time::SystemTime::now() - std::time::Duration::from_secs(1000);
        let files = [
            ("2026-01-01T00-00-00_photos_42.zip", 1),     // legacy password zip
            ("2026-01-02T00-00-00_photos_42.tar.gz", 2),  // before a format switch
            ("2026-01-03T00-00-00_photos_42.zip.enc", 3), // current
            ("2026-01-01T00-00-00_photos_7.zip", 0),      // other job
            ("2026-01-01T00-00-00_docs_42.zip", 0),       // other directory
        ];
        for (name, age) in files {
            let path = dir.join(name);
            std::fs::write(&path, b"x").unwrap();
            let f = std::fs::File::options().write(true).open(&path).unwrap();
            f.set_modified(base + std::time::Duration::from_secs(age * 10)).unwrap();
        }

        let deleted = cleanup_old_archives(dir, "photos", None, 1, 42).unwrap();
        assert_eq!(deleted, 2);
        assert!(dir.join("2026-01-03T00-00-00_photos_42.zip.enc").exists());
        assert!(!dir.join("2026-01-01T00-00-00_photos_42.zip").exists());
        assert!(!dir.join("2026-01-02T00-00-00_photos_42.tar.gz").exists());
        assert!(dir.join("2026-01-01T00-00-00_photos_7.zip").exists());
        assert!(dir.join("2026-01-01T00-00-00_docs_42.zip").exists());
    }

    #[test]
    fn job_archive_patterns_match_only_that_job() {
        let patterns = job_archive_patterns(42);
        let matches = |name: &str| patterns.iter().any(|p| glob::Pattern::new(p).unwrap().matches(name));
        assert!(matches("2026-01-01T00-00-00_photos_42.tar.gz"));
        assert!(matches("photos_42.zip.enc"));
        assert!(!matches("photos_142.tar.gz"));
        assert!(!matches("x_42.notes_7.zip"));
        assert!(!matches("photos_42.tar.gz.tmp"));
    }

    // --- generate_archive_name tests ---

    #[test]
    fn archive_name_basic_no_timestamp_no_custom() {
        let opts = default_opts();
        let name = generate_archive_name("photos", &opts, 42);
        assert_eq!(name, "photos_42.tar.gz");
    }

    #[test]
    fn archive_name_with_custom_name() {
        let mut opts = default_opts();
        opts.custom_name = Some("mybackup".to_string());
        let name = generate_archive_name("photos", &opts, 7);
        assert_eq!(name, "mybackup_photos_7.tar.gz");
    }

    #[test]
    fn archive_name_with_password_changes_extension() {
        let mut opts = default_opts();
        opts.password = Some("secret123".to_string());
        let name = generate_archive_name("docs", &opts, 1);
        assert_eq!(name, "docs_1.tar.gz.enc");
    }

    #[test]
    fn archive_name_with_timestamp_contains_date_pattern() {
        let mut opts = default_opts();
        opts.add_timestamp = true;
        let name = generate_archive_name("data", &opts, 5);
        // Should match pattern: YYYY-MM-DDTHH-MM-SS_data_5.tar.gz
        assert!(name.ends_with("_data_5.tar.gz"), "Got: {}", name);
        assert!(name.contains("T"), "Timestamp should contain 'T' separator: {}", name);
        // Verify it starts with a date-like pattern (4 digits)
        assert!(name.chars().next().unwrap().is_ascii_digit(), "Should start with timestamp: {}", name);
    }

    #[test]
    fn archive_name_zip_format() {
        let mut opts = default_opts();
        opts.format = CompressFormat::Zip;
        let name = generate_archive_name("files", &opts, 99);
        assert_eq!(name, "files_99.zip");
    }

    #[test]
    fn archive_name_store_only_tar() {
        let mut opts = default_opts();
        opts.store_only = true;
        let name = generate_archive_name("logs", &opts, 3);
        assert_eq!(name, "logs_3.tar");
    }

    #[test]
    fn archive_name_sanitizes_dir_name() {
        let opts = default_opts();
        let name = generate_archive_name(".dotnet", &opts, 1);
        assert_eq!(name, "dotnet_1.tar.gz");
    }

    #[test]
    fn archive_name_empty_custom_name_ignored() {
        let mut opts = default_opts();
        opts.custom_name = Some("".to_string());
        let name = generate_archive_name("photos", &opts, 1);
        // Empty custom name should be ignored
        assert_eq!(name, "photos_1.tar.gz");
    }

    #[test]
    fn archive_name_empty_password_not_encrypted() {
        let mut opts = default_opts();
        opts.password = Some("".to_string());
        let name = generate_archive_name("docs", &opts, 1);
        // Empty password should not add .enc
        assert_eq!(name, "docs_1.tar.gz");
    }
}

#[cfg(test)]
mod run_and_poll_tests {
    use super::*;

    fn sh(script: &str) -> tokio::process::Child {
        tokio::process::Command::new("sh")
            .args(["-c", script])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap()
    }

    #[tokio::test]
    async fn does_not_deadlock_on_large_stderr() {
        // ~400 KB of stderr: far more than a pipe buffer
        let child = sh("i=0; while [ $i -lt 8000 ]; do echo 'warning: some long tar message about a file' >&2; i=$((i+1)); done");
        let logged = std::sync::Mutex::new(0usize);
        let res = tokio::time::timeout(
            std::time::Duration::from_secs(20),
            run_and_poll(child, Path::new("/nonexistent"), &|| false, "cancelled", &[], &|_| {
                *logged.lock().unwrap() += 1;
            }),
        )
        .await
        .expect("run_and_poll hung on a full stderr pipe");
        assert!(res.is_ok());
        assert!(*logged.lock().unwrap() > 0);
    }

    #[tokio::test]
    async fn warning_exit_code_is_ok_and_error_includes_stderr() {
        let ok = run_and_poll(sh("exit 1"), Path::new("/nonexistent"), &|| false, "c", &[1], &|_| {}).await;
        assert!(ok.is_ok());

        let err = run_and_poll(sh("echo 'disk full' >&2; exit 2"), Path::new("/nonexistent"), &|| false, "c", &[1], &|_| {})
            .await
            .unwrap_err();
        assert!(err.to_string().contains("code 2"));
        assert!(err.to_string().contains("disk full"));
    }
}
