//! Service for compressing source directories before transfer.
//!
//! Each source directory is individually compressed into an archive file
//! (tar.gz or zip) and stored in a per-job staging subdirectory before
//! being transferred to the destination via the configured provider.

use crate::models::{CompressDirsOptions, CompressFormat};
use anyhow::Context;
use chrono::Local;
use std::path::{Path, PathBuf};

/// Sanitize a directory name to be safe as an archive filename prefix.
///
/// Replaces any character that is not `[a-zA-Z0-9_-]` with `'_'`.
///
/// This is the **single source of truth** used by both `generate_archive_name()`
/// and `cleanup_old_archives()` to ensure consistent naming — the cleanup prefix
/// always matches what was generated.
pub fn sanitize_dir_name(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Get the file extension for a given archive format.
pub fn format_extension(format: &CompressFormat) -> &'static str {
    match format {
        CompressFormat::TarGz => "tar.gz",
        CompressFormat::Zip => "zip",
    }
}

/// Generate the archive filename for a source directory.
///
/// Naming: `[custom_name_]<sanitized_dir_name>[_<timestamp>].<ext>`
///
/// Examples:
/// - `custom="proj"`, `dir="my documents"`, `timestamp=true`
///   → `"proj_my_documents_2026-03-01T14-30-00.tar.gz"`
/// - `custom=None`, `dir="photos"`, `timestamp=false`
///   → `"photos.tar.gz"`
pub fn generate_archive_name(dir_name: &str, opts: &CompressDirsOptions) -> String {
    let sanitized = sanitize_dir_name(dir_name);
    let ext = format_extension(&opts.format);

    let mut parts: Vec<String> = Vec::new();

    if let Some(ref custom) = opts.custom_name {
        if !custom.is_empty() {
            parts.push(custom.clone());
        }
    }

    parts.push(sanitized);

    if opts.add_timestamp {
        let ts = Local::now().format("%Y-%m-%dT%H-%M-%S").to_string();
        parts.push(ts);
    }

    format!("{}.{}", parts.join("_"), ext)
}

/// Compress a source directory into the per-job staging subdirectory.
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
/// The archive is first written as a temporary file (`<archive_name>.<run_id>.tmp`)
/// and then atomically renamed to the final path on success. The temp file is
/// cleaned up on failure.
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
    // Performed before creating the staging dir to catch configuration errors early.
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

    log_fn(format!(
        "Compressing '{}' → {}",
        source_dir.display(),
        archive_name
    ));

    // Spawn the compression subprocess (no shell — args passed as separate array elements)
    let mut child = match opts.format {
        CompressFormat::TarGz => tokio::process::Command::new("tar")
            .args([
                "-czpf",
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
            .context("Failed to spawn tar. Ensure GNU tar is installed (not BusyBox tar).")?,

        CompressFormat::Zip => tokio::process::Command::new("zip")
            .args(["-r", tmp_path.to_str().unwrap_or_default(), dir_name])
            .current_dir(parent_dir)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .context("Failed to spawn zip. Ensure zip is installed.")?,
    };

    // Poll until the child exits or cancellation is requested (500 ms tick)
    let result: anyhow::Result<()> = loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if status.success() {
                    break Ok(());
                } else {
                    break Err(anyhow::anyhow!(
                        "Compression process exited with code {}",
                        status.code().unwrap_or(-1)
                    ));
                }
            }
            Ok(None) => {
                // Process still running — check for cancellation
                if is_cancelled() {
                    let _ = child.kill().await;
                    let _ = tokio::fs::remove_file(&tmp_path).await;
                    return Err(anyhow::anyhow!("Compression cancelled"));
                }
                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
            }
            Err(e) => {
                break Err(anyhow::anyhow!("Failed to check process status: {}", e));
            }
        }
    };

    if let Err(e) = result {
        let _ = tokio::fs::remove_file(&tmp_path).await;
        return Err(e);
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
/// so the match prefix is guaranteed to align with the generated filenames.
///
/// Returns the number of deleted archives.
pub fn cleanup_old_archives(
    staging_dir: &Path,
    dir_name: &str,
    custom_name: Option<&str>,
    format: &CompressFormat,
    max_count: u32,
) -> anyhow::Result<u32> {
    let ext = format_extension(format);
    let sanitized = sanitize_dir_name(dir_name);

    // Build the prefix that all matching archives share.
    // Mirrors exactly how generate_archive_name() constructs names.
    let prefix = match custom_name {
        Some(cn) if !cn.is_empty() => format!("{}_{}_", cn, sanitized),
        _ => format!("{}_", sanitized),
    };
    let suffix = format!(".{}", ext);

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
            name_str.starts_with(&prefix) && name_str.ends_with(&suffix)
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
