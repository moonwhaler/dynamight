//! Service for compressing source directories before transfer.
//!
//! Each source directory is individually archived (tar.gz, tar, or zip) and
//! stored in a per-job staging subdirectory before being transferred to the
//! destination via the configured provider.

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
/// `encrypted` adds a `.enc` suffix for tar-based formats (openssl AES-256).
/// Zip encryption is built into the format so the extension does not change.
pub fn format_extension(format: &CompressFormat, store_only: bool, encrypted: bool) -> &'static str {
    match format {
        CompressFormat::TarGz => match (store_only, encrypted) {
            (false, false) => "tar.gz",
            (false, true) => "tar.gz.enc",
            (true, false) => "tar",
            (true, true) => "tar.enc",
        },
        CompressFormat::Zip => "zip",
    }
}

/// Generate the archive filename for a source directory.
///
/// Naming: `[<timestamp>_][custom_name_]<sanitized_dir_name>.<ext>`
///
/// The timestamp is always the **first** segment when enabled, making archives
/// sort chronologically by name and clearly marking when they were created.
pub fn generate_archive_name(dir_name: &str, opts: &CompressDirsOptions) -> String {
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

    parts.push(sanitized);

    format!("{}.{}", parts.join("_"), ext)
}

/// Poll a spawned child process until it exits or cancellation is requested.
///
/// On failure or cancellation the `cleanup_path` file is removed. This helper
/// is called for both the compression and encryption phases in the two-step
/// encrypted-tar workflow.
async fn run_and_poll(
    mut child: tokio::process::Child,
    cleanup_path: &Path,
    is_cancelled: &impl Fn() -> bool,
    cancel_msg: &'static str,
) -> anyhow::Result<()> {
    let result: anyhow::Result<()> = loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if status.success() {
                    break Ok(());
                } else {
                    break Err(anyhow::anyhow!(
                        "Process exited with code {}",
                        status.code().unwrap_or(-1)
                    ));
                }
            }
            Ok(None) => {
                if is_cancelled() {
                    let _ = child.kill().await;
                    let _ = tokio::fs::remove_file(cleanup_path).await;
                    return Err(anyhow::anyhow!("{}", cancel_msg));
                }
                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
            }
            Err(e) => break Err(anyhow::anyhow!("Failed to check process status: {}", e)),
        }
    };

    if result.is_err() {
        let _ = tokio::fs::remove_file(cleanup_path).await;
    }

    result
}

/// Compress (or archive) a source directory into the per-job staging subdirectory.
///
/// Parameters:
/// - `source_dir`:   absolute path to the directory to compress
/// - `job_id`:       used for the per-job staging subdir name (`staging_path/<job_id>/`)
/// - `run_id`:       used for unique temp file naming
/// - `opts`:         compression options
/// - `log_fn`:       called to emit log lines during compression
/// - `is_cancelled`: checked every 500 ms; if it returns `true`, the
///                   subprocess is killed and an error is returned
///
/// When `store_only` is set, files are archived without compression
/// (tar without `-z`, or `zip -0`).
///
/// When a password is set for tar-based formats, a two-step process is used:
/// 1. Archive to an intermediate temp file
/// 2. Encrypt with `openssl enc -aes-256-cbc -pbkdf2` (password via stdin)
/// The final output carries a `.enc` extension (e.g. `archive.tar.gz.enc`).
///
/// Returns the path to the created archive file.
pub async fn compress_directory(
    source_dir: &Path,
    job_id: i64,
    run_id: i64,
    opts: &CompressDirsOptions,
    log_fn: impl Fn(String),
    is_cancelled: impl Fn() -> bool,
) -> anyhow::Result<PathBuf> {
    let staging_dir = PathBuf::from(&opts.staging_path).join(job_id.to_string());

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

    let archive_name = generate_archive_name(dir_name, opts);
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

    match &opts.format {
        CompressFormat::Zip => {
            let mut cmd = tokio::process::Command::new("zip");
            if opts.store_only {
                cmd.arg("-0"); // store only, no compression
            }
            cmd.arg("-r");
            if has_password {
                if let Some(ref pass) = opts.password {
                    cmd.args(["-P", pass.as_str()]);
                }
            }
            cmd.args([tmp_path.to_str().unwrap_or_default(), dir_name]);
            let child = cmd
                .current_dir(parent_dir)
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .context("Failed to spawn zip. Ensure zip is installed.")?;

            run_and_poll(child, &tmp_path, &is_cancelled, "Archiving cancelled").await?;
        }

        CompressFormat::TarGz => {
            // `-z` for gzip compression; omit for store-only (plain tar)
            let tar_flags = if opts.store_only { "-cpf" } else { "-czpf" };

            if has_password {
                let password = opts.password.as_deref().unwrap();

                // Phase 1: Create the tar archive to an intermediate temp file.
                // The final output will be the openssl-encrypted version.
                let inter_path =
                    staging_dir.join(format!("{}.inter.{}.tmp", archive_name, run_id));

                let tar_child = tokio::process::Command::new("tar")
                    .args([
                        tar_flags,
                        inter_path.to_str().unwrap_or_default(),
                        "--numeric-owner",
                        "-C",
                        parent_dir.to_str().unwrap_or_default(),
                        dir_name,
                    ])
                    .stdin(std::process::Stdio::null())
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::piped())
                    .spawn()
                    .context(
                        "Failed to spawn tar. Ensure GNU tar is installed (not BusyBox tar).",
                    )?;

                run_and_poll(tar_child, &inter_path, &is_cancelled, "Archiving cancelled")
                    .await?;

                // Phase 2: Encrypt the intermediate archive with openssl AES-256-CBC.
                // Password is passed via stdin to avoid exposing it in the process list.
                log_fn("Encrypting archive (AES-256-CBC)".to_string());

                let mut enc_child = tokio::process::Command::new("openssl")
                    .args([
                        "enc",
                        "-aes-256-cbc",
                        "-pbkdf2",
                        "-in",
                        inter_path.to_str().unwrap_or_default(),
                        "-out",
                        tmp_path.to_str().unwrap_or_default(),
                        "-pass",
                        "stdin",
                    ])
                    .stdin(std::process::Stdio::piped())
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::piped())
                    .spawn()
                    .context("Failed to spawn openssl. Ensure openssl is installed.")?;

                // Write password to openssl's stdin and close the pipe
                if let Some(mut stdin) = enc_child.stdin.take() {
                    use tokio::io::AsyncWriteExt;
                    let _ = stdin.write_all(password.as_bytes()).await;
                    // stdin is dropped here, closing the pipe and signalling EOF to openssl
                }

                let enc_result =
                    run_and_poll(enc_child, &tmp_path, &is_cancelled, "Encryption cancelled")
                        .await;

                // Always remove the intermediate unencrypted archive — even on success
                let _ = tokio::fs::remove_file(&inter_path).await;

                enc_result?;
            } else {
                // No password: write archive directly to tmp_path
                let child = tokio::process::Command::new("tar")
                    .args([
                        tar_flags,
                        tmp_path.to_str().unwrap_or_default(),
                        "--numeric-owner",
                        "-C",
                        parent_dir.to_str().unwrap_or_default(),
                        dir_name,
                    ])
                    .stdin(std::process::Stdio::null())
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::piped())
                    .spawn()
                    .context(
                        "Failed to spawn tar. Ensure GNU tar is installed (not BusyBox tar).",
                    )?;

                run_and_poll(child, &tmp_path, &is_cancelled, "Archiving cancelled").await?;
            }
        }
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

/// Remove the oldest archives in the per-job staging directory for a given
/// source directory, keeping at most `max_count` archives.
///
/// Uses `sanitize_dir_name(dir_name)` — identical to `generate_archive_name()` —
/// so the match suffix is guaranteed to align with the generated filenames.
///
/// Archive names follow `[TIMESTAMP_][custom_]sanitized.ext`, so the sanitized
/// name is always the last segment before the extension. Matching is done by
/// suffix (`ends_with`) to handle both timestamped and non-timestamped variants.
///
/// Returns the number of deleted archives.
pub fn cleanup_old_archives(
    staging_dir: &Path,
    dir_name: &str,
    custom_name: Option<&str>,
    format: &CompressFormat,
    store_only: bool,
    has_password: bool,
    max_count: u32,
) -> anyhow::Result<u32> {
    let ext = format_extension(format, store_only, has_password);
    let sanitized = sanitize_dir_name(dir_name);

    // The base name without any timestamp prefix: [custom_]sanitized.ext
    let base_name = match custom_name {
        Some(cn) if !cn.is_empty() => format!("{}_{}.{}", cn, sanitized, ext),
        _ => format!("{}.{}", sanitized, ext),
    };
    // The suffix used to match timestamped variants: "_[custom_]sanitized.ext"
    let ts_suffix = format!("_{}", base_name);

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
            let name_str = name.to_string_lossy().into_owned();
            // Match exact base name (no timestamp) OR timestamp-prefixed variant
            name_str == base_name || name_str.ends_with(&ts_suffix)
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
