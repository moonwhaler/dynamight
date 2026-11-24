//! Rsync-based sync provider for local and USB destinations

use super::{ProviderCapabilities, ProviderError, StorageInfo, SyncContext, SyncProvider, SyncResult};
use crate::models::{CredentialData, DestinationConfig, LogLevel, SyncOptions};
use crate::services::backup_service::format_file_size;
use async_trait::async_trait;
#[cfg(unix)]
use nix::sys::signal::{self, Signal};
#[cfg(unix)]
use nix::unistd::Pid;
use serde::Serialize;
use std::path::Path;
use std::process::Stdio;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

/// Result of a space check operation
#[derive(Debug, Clone, Serialize)]
pub struct SpaceCheckResult {
    /// Total size of all source directories
    pub source_size: u64,
    /// Estimated transfer size (from rsync dry-run)
    pub transfer_size: u64,
    /// Free space at destination
    pub destination_free: u64,
    /// Total space at destination
    pub destination_total: u64,
    /// Whether the transfer will fit
    pub fits: bool,
    /// How many bytes short (if !fits)
    pub deficit: Option<u64>,
    /// Per-source breakdown
    pub sources: Vec<SourceSizeInfo>,
}

/// Size information for a single source directory
#[derive(Debug, Clone, Serialize)]
pub struct SourceSizeInfo {
    pub path: String,
    pub size: u64,
    pub transfer_size: u64,
}

pub struct RsyncProvider;

impl RsyncProvider {
    pub fn new() -> Self {
        Self
    }

    /// Build rsync command arguments based on configuration
    fn build_rsync_args(&self, ctx: &SyncContext, fstype: &str) -> Vec<String> {
        let mut args = vec![];

        // Force line-buffered output so logs stream in real-time
        // (without this, rsync uses block buffering when writing to a pipe,
        // causing long delays on slow destinations like USB drives)
        args.push("--outbuf=L".to_string());

        // Verbosity options
        match ctx.options.verbosity.as_str() {
            "quiet" => {
                args.push("-q".to_string());
            }
            "verbose" => {
                // No -h: it makes --stats print sizes like "3.00M", which
                // cannot be read back as exact byte counts
                args.extend(["-v".to_string(), "--stats".to_string(), "--progress".to_string()]);
            }
            _ => {
                // "normal" (default)
                args.extend(["-v".to_string(), "--stats".to_string()]);
            }
        }

        // Filesystem-aware options
        match fstype {
            "exfat" | "ntfs" | "vfat" | "msdos" | "ntfs3" => {
                // Non-POSIX filesystems - avoid permission errors
                args.extend(["-r", "-l", "-t", "-D"].iter().map(|s| s.to_string()));
            }
            _ => {
                // POSIX-compliant filesystems
                args.push("-a".to_string());
            }
        }

        // Configurable options
        if ctx.options.delete_extraneous {
            args.push("--delete".to_string());
        }

        // Provider-specific options (checksum, compression, ignore-times)
        if let Some(provider_opts) = ctx.options.provider_options.as_ref() {
            if provider_opts.get("checksum_mode").and_then(|v| v.as_bool()).unwrap_or(false) {
                args.push("--checksum".to_string());
            }
            if provider_opts.get("compress").and_then(|v| v.as_bool()).unwrap_or(false) {
                args.push("-z".to_string());
            }
            if provider_opts.get("ignore_times").and_then(|v| v.as_bool()).unwrap_or(false) {
                args.push("--ignore-times".to_string());
            }
        }

        if ctx.options.dry_run {
            args.push("--dry-run".to_string());
        }

        if let Some(limit) = ctx.options.bandwidth_limit_kbps {
            if limit > 0 {
                args.push(format!("--bwlimit={}", limit));
            }
        }

        // Excludes
        for exclude in &ctx.options.exclude_patterns {
            args.push(format!("--exclude={}", exclude));
        }

        // Compress mode: transfer only this job's archives. Everything else is
        // excluded, which also protects it from --delete on the destination.
        if !ctx.options.include_only.is_empty() {
            for pattern in &ctx.options.include_only {
                args.push(format!("--include={}", pattern));
            }
            args.push("--exclude=*".to_string());
        }

        args
    }

    /// Detect the filesystem type at a mount point
    async fn detect_filesystem(&self, mount_point: &str) -> String {
        let output = Command::new("findmnt")
            .args(["-n", "-o", "FSTYPE", "--target", mount_point])
            .output()
            .await;

        match output {
            Ok(o) if o.status.success() => {
                // Take only the first line in case of multiple mounts (bind mounts, submounts)
                String::from_utf8_lossy(&o.stdout)
                    .lines()
                    .next()
                    .unwrap_or("unknown")
                    .to_string()
            }
            _ => "unknown".to_string(),
        }
    }

    /// Mount a USB drive by UUID. Returns `true` if this call mounted it,
    /// `false` if it was already mounted there.
    async fn mount_usb(&self, ctx: &SyncContext, uuid: &str, mount_point: &str) -> Result<bool, ProviderError> {
        ctx.log_info(&format!("Mounting UUID {} to {}", uuid, mount_point), "mount").await;
        match ensure_mounted(uuid, mount_point).await {
            Ok(true) => {
                ctx.log_info("Mount successful", "mount").await;
                Ok(true)
            }
            Ok(false) => {
                ctx.log_info("Drive already mounted", "mount").await;
                Ok(false)
            }
            Err(msg) => {
                ctx.log_error(&msg, "mount").await;
                Err(ProviderError::ConfigError(msg))
            }
        }
    }

    /// Unmount a mount point
    async fn unmount(&self, ctx: &SyncContext, mount_point: &str) {
        ctx.log_info(&format!("Unmounting {}", mount_point), "mount").await;

        // Sync first
        let _ = Command::new("sync").output().await;
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

        let unmount_result = Command::new("umount")
            .arg(mount_point)
            .output()
            .await;

        match unmount_result {
            Ok(output) if output.status.success() => {
                ctx.log_info("Unmount successful", "mount").await;
            }
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                ctx.log_warning(&format!("Unmount warning: {}", stderr), "mount").await;
            }
            Err(e) => {
                ctx.log_warning(&format!("Unmount error: {}", e), "mount").await;
            }
        }
    }

    /// Unmount a mount point (standalone version without SyncContext)
    async fn unmount_standalone(&self, mount_point: &str) {
        let _ = Command::new("sync").output().await;
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        let _ = Command::new("umount").arg(mount_point).output().await;
    }

    /// Build rsync args for space check (dry-run with per-file size output)
    fn build_space_check_args(&self, options: &SyncOptions, fstype: &str) -> Vec<String> {
        // Use --out-format=%l to output only the file size (bytes) for each file
        // that would be transferred. This works reliably in dry-run mode unlike --stats.
        let mut args = vec![
            "--dry-run".to_string(),
            "--out-format=%l".to_string(),
        ];

        // Filesystem-aware options
        match fstype {
            "exfat" | "ntfs" | "vfat" | "msdos" | "ntfs3" => {
                args.extend(["-r", "-l", "-t", "-D"].iter().map(|s| s.to_string()));
            }
            _ => {
                args.push("-a".to_string());
            }
        }

        // Include delete flag if set (affects what would be transferred)
        if options.delete_extraneous {
            args.push("--delete".to_string());
        }

        // Excludes
        for exclude in &options.exclude_patterns {
            args.push(format!("--exclude={}", exclude));
        }

        args
    }

    /// Get the size of a directory using du
    async fn get_directory_size(&self, path: &str) -> Result<u64, ProviderError> {
        let output = Command::new("du")
            .args(["-sb", path])
            .output()
            .await
            .map_err(ProviderError::IoError)?;

        if !output.status.success() {
            return Err(ProviderError::ConfigError(format!(
                "Failed to get size of {}",
                path
            )));
        }

        let output_str = String::from_utf8_lossy(&output.stdout);
        let size_str = output_str.split_whitespace().next().unwrap_or("0");
        size_str
            .parse::<u64>()
            .map_err(|_| ProviderError::ConfigError("Failed to parse directory size".to_string()))
    }

    /// Run rsync dry-run and sum file sizes from output
    /// Uses --out-format=%l which outputs file size (bytes) per file to transfer
    async fn get_transfer_size(
        &self,
        source: &str,
        dest: &str,
        args: &[String],
    ) -> Result<u64, ProviderError> {
        let mut cmd = Command::new("rsync");
        set_rsync_locale(&mut cmd);
        cmd.args(args)
            .arg(format!("{}/", source))
            .arg(format!("{}/", dest))
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let output = cmd.output().await.map_err(ProviderError::IoError)?;

        let stdout = String::from_utf8_lossy(&output.stdout);

        // Each line contains a file size in bytes (from --out-format=%l)
        // Sum all sizes to get total transfer size
        let transfer_size: u64 = stdout
            .lines()
            .filter_map(|line| line.trim().parse::<u64>().ok())
            .sum();

        Ok(transfer_size)
    }

    /// Mount the job's USB drive for a space check (when the job auto-mounts
    /// it) and make sure the right drive is there. Returns whether this call
    /// mounted it; pass that to [`release_mount`](Self::release_mount).
    pub async fn prepare_mount(&self, destination: &DestinationConfig) -> Result<bool, ProviderError> {
        let DestinationConfig::Local { mount_point, usb_uuid: Some(uuid), auto_mount, .. } = destination else {
            return Ok(false);
        };
        let did_mount = if *auto_mount {
            ensure_mounted(uuid, mount_point).await.map_err(ProviderError::ConfigError)?
        } else {
            false
        };
        // Refuse to measure (and create directories in) the empty mount-point
        // directory when the drive is not mounted there
        if let Err(msg) = verify_drive_mounted(uuid, mount_point).await {
            if did_mount {
                self.unmount_standalone(mount_point).await;
            }
            return Err(ProviderError::ConfigError(msg));
        }
        Ok(did_mount)
    }

    /// Unmount a drive [`prepare_mount`](Self::prepare_mount) mounted, if the
    /// job wants it unmounted after use.
    pub async fn release_mount(&self, destination: &DestinationConfig) {
        if let DestinationConfig::Local { mount_point, usb_uuid: Some(_), auto_unmount: true, .. } = destination {
            self.unmount_standalone(mount_point).await;
        }
    }

    /// Measure whether the sources fit on the destination. Does not mount
    /// anything: the destination must be available already.
    pub async fn measure_space(
        &self,
        source_dirs: &[String],
        destination: &DestinationConfig,
        options: &SyncOptions,
    ) -> Result<SpaceCheckResult, ProviderError> {
        let DestinationConfig::Local { mount_point, backup_subdir, .. } = destination else {
            return Err(ProviderError::ConfigError(
                "Space check only supports Local destination".to_string(),
            ));
        };
        let mount_point = mount_point.as_str();

        // Detect filesystem type
        let fstype = self.detect_filesystem(mount_point).await;

        // Build rsync args for dry-run
        let rsync_args = self.build_space_check_args(options, &fstype);

        let mut sources_info = Vec::new();
        let mut total_source_size: u64 = 0;
        let mut total_transfer_size: u64 = 0;

        // Check each source directory
        for source_dir in source_dirs {
            if !Path::new(source_dir).exists() {
                continue;
            }

            // Get source size
            let source_size = self.get_directory_size(source_dir).await.unwrap_or(0);
            total_source_size += source_size;

            // Derive destination path
            let dest_name = Path::new(source_dir)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "backup".to_string());

            let dest = format!("{}/{}/{}", mount_point, backup_subdir, dest_name);

            // Create destination if it doesn't exist (for accurate dry-run)
            let _ = tokio::fs::create_dir_all(&dest).await;

            // Build exclude args for directories specific to this source
            let mut args_with_excludes = rsync_args.clone();
            for exclude_dir in &options.exclude_dirs {
                let exclude_path = std::path::Path::new(exclude_dir);
                let source_path = std::path::Path::new(source_dir);
                if let Ok(relative) = exclude_path.strip_prefix(source_path) {
                    let rel_str = relative.to_string_lossy();
                    args_with_excludes.push(format!("--exclude=/{}", rel_str));
                    args_with_excludes.push(format!("--exclude=/{}/**", rel_str));
                }
            }

            // Get transfer size via rsync dry-run
            let transfer_size = self
                .get_transfer_size(source_dir, &dest, &args_with_excludes)
                .await
                .unwrap_or(source_size); // Fall back to source size if dry-run fails

            total_transfer_size += transfer_size;

            sources_info.push(SourceSizeInfo {
                path: source_dir.clone(),
                size: source_size,
                transfer_size,
            });
        }

        // Get destination free space using statvfs via df command
        // First try the GNU coreutils --output option, then fall back to POSIX df
        let (destination_free, destination_total) = {
            // Try GNU df with --output first (most accurate)
            let output = Command::new("df")
                .args(["--block-size=1", "--output=avail,size", mount_point])
                .output()
                .await;

            match output {
                Ok(out) if out.status.success() => {
                    let stdout = String::from_utf8_lossy(&out.stdout);
                    let mut lines = stdout.lines();
                    lines.next(); // Skip header
                    if let Some(data_line) = lines.next() {
                        let parts: Vec<&str> = data_line.split_whitespace().collect();
                        let free: u64 = parts.first().and_then(|s| s.parse().ok()).unwrap_or(0);
                        let total: u64 = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);
                        if free > 0 || total > 0 {
                            (free, total)
                        } else {
                            tracing::warn!("df returned zero values, output: {:?}", stdout);
                            (0, 0)
                        }
                    } else {
                        tracing::warn!("df output has no data line: {:?}", stdout);
                        (0, 0)
                    }
                }
                Ok(out) => {
                    let stderr = String::from_utf8_lossy(&out.stderr);
                    let stdout = String::from_utf8_lossy(&out.stdout);
                    tracing::warn!(
                        "df --output failed for {}: stderr={}, stdout={}, exit={:?}",
                        mount_point, stderr.trim(), stdout.trim(), out.status.code()
                    );

                    // Fallback: try POSIX df (columns: Filesystem, 1K-blocks, Used, Available, Use%, Mounted on)
                    let fallback = Command::new("df")
                        .args(["-B1", mount_point])
                        .output()
                        .await;

                    match fallback {
                        Ok(fb) if fb.status.success() => {
                            let fb_stdout = String::from_utf8_lossy(&fb.stdout);
                            let mut lines = fb_stdout.lines();
                            lines.next(); // Skip header
                            if let Some(data_line) = lines.next() {
                                let parts: Vec<&str> = data_line.split_whitespace().collect();
                                // POSIX: Filesystem, Size, Used, Avail, Use%, Mount
                                let total: u64 = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);
                                let free: u64 = parts.get(3).and_then(|s| s.parse().ok()).unwrap_or(0);
                                tracing::info!("df fallback succeeded: free={}, total={}", free, total);
                                (free, total)
                            } else {
                                (0, 0)
                            }
                        }
                        Ok(fb) => {
                            tracing::error!(
                                "df fallback also failed for {}: {}",
                                mount_point,
                                String::from_utf8_lossy(&fb.stderr).trim()
                            );
                            (0, 0)
                        }
                        Err(e) => {
                            tracing::error!("df fallback command error: {}", e);
                            (0, 0)
                        }
                    }
                }
                Err(e) => {
                    tracing::error!("df command error for {}: {}", mount_point, e);
                    (0, 0)
                }
            }
        };

        // If we couldn't get destination space info, return an error
        if destination_free == 0 && destination_total == 0 {
            return Err(ProviderError::ConfigError(format!(
                "Could not determine free space at destination '{}'. Is the drive mounted?",
                mount_point
            )));
        }

        // Add 5% safety margin
        let required_with_margin = (total_transfer_size as f64 * 1.05) as u64;
        let fits = required_with_margin <= destination_free;
        let deficit = if fits {
            None
        } else {
            Some(required_with_margin - destination_free)
        };

        Ok(SpaceCheckResult {
            source_size: total_source_size,
            transfer_size: total_transfer_size,
            destination_free,
            destination_total,
            fits,
            deficit,
            sources: sources_info,
        })
    }
}

impl Default for RsyncProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SyncProvider for RsyncProvider {
    fn provider_type(&self) -> &'static str {
        "local"
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            supports_delete: true,
            supports_compression: true,
            supports_checksum: true,
            supports_bandwidth_limit: true,
            supports_exclude_patterns: true,
            supports_incremental: true,
            supports_dry_run: true,
            requires_credentials: false,
        }
    }

    fn validate_config(
        &self,
        destination: &DestinationConfig,
        _credential: Option<&CredentialData>,
    ) -> Result<(), ProviderError> {
        match destination {
            DestinationConfig::Local { mount_point, .. } => {
                if mount_point.is_empty() {
                    return Err(ProviderError::ConfigError("Mount point is required".to_string()));
                }
                Ok(())
            }
            _ => Err(ProviderError::ConfigError(
                "RsyncProvider only supports Local destination".to_string(),
            )),
        }
    }

    async fn sync(&self, ctx: Arc<SyncContext>) -> Result<SyncResult, ProviderError> {
        let mut result = SyncResult::default();

        // Extract local destination config
        let (mount_point, backup_subdir, usb_uuid, auto_mount, auto_unmount) = match &ctx.destination {
            DestinationConfig::Local {
                mount_point,
                backup_subdir,
                usb_uuid,
                auto_mount,
                auto_unmount,
            } => (
                mount_point.clone(),
                backup_subdir.clone(),
                usb_uuid.clone(),
                *auto_mount,
                *auto_unmount,
            ),
            _ => {
                return Err(ProviderError::ConfigError(
                    "RsyncProvider only supports Local destination".to_string(),
                ))
            }
        };

        ctx.log_info("Starting sync", "system").await;

        // Mount if needed
        let mut we_mounted = false;
        if auto_mount {
            if let Some(uuid) = &usb_uuid {
                we_mounted = self.mount_usb(&ctx, uuid, &mount_point).await?;
            }
        }

        // For USB destinations, make sure the expected drive really is mounted at
        // the mount point. Otherwise rsync would write into the empty mount-point
        // directory on the system disk (drive unplugged, or "already mounted"
        // somewhere else by a desktop automounter).
        if let Some(uuid) = &usb_uuid {
            if let Err(msg) = verify_drive_mounted(uuid, &mount_point).await {
                ctx.log_error(&msg, "mount").await;
                if we_mounted && auto_unmount {
                    self.unmount(&ctx, &mount_point).await;
                }
                return Err(ProviderError::ConfigError(msg));
            }
        }

        // Skipped in compress mode: the sources are the archives in the staging
        // directory, whose space is managed by the archive retention
        let space_failed = !ctx.options.compress_dirs_enabled() && self.space_check_fails(&ctx, &mut result).await;
        let sync_outcome = if space_failed {
            Ok(())
        } else {
            self.run_sources(&ctx, &mount_point, &backup_subdir, &mut result).await
        };

        // Capture storage info BEFORE unmounting (so we get the USB drive's space, not the underlying filesystem)
        if sync_outcome.is_ok() {
            if let Ok(info) = self.get_storage_info(&ctx.destination, None).await {
                if info.supported {
                    result.storage_info = Some(info);
                }
            }
        }

        // Unmount if needed - also after cancellation or errors
        if should_unmount(auto_mount, auto_unmount, we_mounted, usb_uuid.is_some()) {
            self.unmount(&ctx, &mount_point).await;
        } else if auto_unmount && usb_uuid.is_some() {
            ctx.log_info("Drive was already mounted before the backup; leaving it mounted", "mount").await;
        }

        sync_outcome?;
        if space_failed {
            result.success = false;
            return Ok(result);
        }
        result.success = result.error_count == 0;
        ctx.log_info(
            &format!(
                "Backup complete. Files: {}, Bytes: {}, Errors: {}",
                result.files_transferred, result.bytes_transferred, result.error_count
            ),
            "system",
        ).await;

        Ok(result)
    }

    async fn get_storage_info(
        &self,
        destination: &DestinationConfig,
        _credential: Option<&CredentialData>,
    ) -> Result<StorageInfo, ProviderError> {
        let mount_point = match destination {
            DestinationConfig::Local { mount_point, .. } => mount_point.clone(),
            _ => return Ok(StorageInfo::default()),
        };

        // Use df to get storage info
        let output = Command::new("df")
            .args(["--block-size=1", "--output=avail,size", &mount_point])
            .output()
            .await;

        match output {
            Ok(out) if out.status.success() => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                let mut lines = stdout.lines();
                lines.next(); // Skip header
                if let Some(data_line) = lines.next() {
                    let parts: Vec<&str> = data_line.split_whitespace().collect();
                    let free: u64 = parts.first().and_then(|s| s.parse().ok()).unwrap_or(0);
                    let total: u64 = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);
                    if free > 0 || total > 0 {
                        return Ok(StorageInfo {
                            free: Some(free),
                            total: Some(total),
                            supported: true,
                        });
                    }
                }
                // Fallback to POSIX df if --output failed
                let fallback = Command::new("df")
                    .args(["-B1", &mount_point])
                    .output()
                    .await;

                if let Ok(fb) = fallback {
                    if fb.status.success() {
                        let fb_stdout = String::from_utf8_lossy(&fb.stdout);
                        let mut lines = fb_stdout.lines();
                        lines.next(); // Skip header
                        if let Some(data_line) = lines.next() {
                            let parts: Vec<&str> = data_line.split_whitespace().collect();
                            let total: u64 = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);
                            let free: u64 = parts.get(3).and_then(|s| s.parse().ok()).unwrap_or(0);
                            if free > 0 || total > 0 {
                                return Ok(StorageInfo {
                                    free: Some(free),
                                    total: Some(total),
                                    supported: true,
                                });
                            }
                        }
                    }
                }
                Ok(StorageInfo::default())
            }
            _ => Ok(StorageInfo::default()),
        }
    }
}

/// Whether to unmount the USB drive after a run.
///
/// With auto-mount on, the drive is left as it was found: one that was
/// already mounted before the run (by the user or a desktop automounter, and
/// possibly in use) stays mounted. With auto-mount off, auto-unmount means
/// "unmount the drive I mounted myself", so it is unmounted.
fn should_unmount(auto_mount: bool, auto_unmount: bool, we_mounted: bool, is_usb: bool) -> bool {
    is_usb && auto_unmount && (we_mounted || !auto_mount)
}

impl RsyncProvider {
    /// The job's space check before copying (destination already mounted).
    /// Returns `true` when the run must stop ("fail" mode and too little
    /// space); that counts as an error. Problems measuring only warn.
    async fn space_check_fails(&self, ctx: &SyncContext, result: &mut SyncResult) -> bool {
        let mode = ctx.options.space_check_mode();
        if mode == "none" {
            return false;
        }
        match self.measure_space(&ctx.source_dirs, &ctx.destination, &ctx.options).await {
            Ok(check) if !check.fits => {
                let msg = format!(
                    "Insufficient space: need {} to transfer, have {} free ({} short)",
                    format_file_size(check.transfer_size),
                    format_file_size(check.destination_free),
                    format_file_size(check.deficit.unwrap_or(0))
                );
                if mode == "fail" {
                    ctx.log_error(&msg, "space-check").await;
                    result.error_count += 1;
                    return true;
                }
                ctx.log_warning(&msg, "space-check").await;
            }
            Ok(check) => {
                ctx.log_info(
                    &format!(
                        "Space check passed: {} to transfer, {} free",
                        format_file_size(check.transfer_size),
                        format_file_size(check.destination_free)
                    ),
                    "space-check",
                )
                .await;
            }
            Err(e) => ctx.log_warning(&format!("Space check failed: {}", e), "space-check").await,
        }
        false
    }

    /// Run rsync once per source directory, streaming its output to the run log.
    ///
    /// Returns `Err(ProviderError::Cancelled)` when the run was cancelled, so the
    /// caller can still unmount before propagating it.
    async fn run_sources(
        &self,
        ctx: &SyncContext,
        mount_point: &str,
        backup_subdir: &str,
        result: &mut SyncResult,
    ) -> Result<(), ProviderError> {
        // Detect filesystem type
        let fstype = self.detect_filesystem(mount_point).await;
        ctx.log_info(&format!("Filesystem type: {}", fstype), "system").await;

        // Build rsync args
        let rsync_args = self.build_rsync_args(ctx, &fstype);
        ctx.log(LogLevel::Debug, &format!("Rsync args: {:?}", rsync_args), "rsync").await;

        for source_dir in &ctx.source_dirs {
            if ctx.check_cancelled() {
                ctx.log_warning("Job cancelled, skipping remaining sources", "system").await;
                return Err(ProviderError::Cancelled);
            }

            if !Path::new(source_dir).exists() {
                // Counted as an error (the run fails), so logged as one
                ctx.log_error(&format!("Source directory does not exist: {}", source_dir), "rsync").await;
                result.error_count += 1;
                continue;
            }

            let dest_name = Path::new(source_dir)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "backup".to_string());

            let dest = format!("{}/{}/{}", mount_point, backup_subdir, dest_name);

            ctx.log_info(&format!("Syncing {} -> {}", source_dir, dest), "rsync").await;

            // Create destination directory
            if let Err(e) = tokio::fs::create_dir_all(&dest).await {
                ctx.log_warning(&format!("Could not create destination: {}", e), "rsync").await;
            }

            // Build exclude args for directories specific to this source
            let mut source_exclude_args: Vec<String> = Vec::new();
            for exclude_dir in &ctx.options.exclude_dirs {
                let exclude_path = Path::new(exclude_dir);
                let source_path = Path::new(source_dir);
                if let Ok(relative) = exclude_path.strip_prefix(source_path) {
                    let rel_str = relative.to_string_lossy();
                    // Exclude the directory and its contents
                    source_exclude_args.push(format!("--exclude=/{}", rel_str));
                    source_exclude_args.push(format!("--exclude=/{}/**", rel_str));
                }
            }

            // Run rsync
            let mut cmd = Command::new("rsync");
            set_rsync_locale(&mut cmd);
            cmd.args(&rsync_args)
                .args(&source_exclude_args)
                .arg(format!("{}/", source_dir))
                .arg(format!("{}/", dest))
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .process_group(0);

            let mut child = match cmd.spawn() {
                Ok(c) => c,
                Err(e) => {
                    ctx.log_error(&format!("Failed to spawn rsync: {}", e), "rsync").await;
                    result.error_count += 1;
                    continue;
                }
            };

            let pid = child.id().unwrap_or(0);
            ctx.log_info(&format!("Spawned rsync with PID {}", pid), "rsync").await;

            // Read stdout and stderr concurrently. Reading one to the end before
            // the other deadlocks once rsync fills the other pipe's buffer
            // (for example with many "Permission denied" warnings).
            let mut out_lines = child.stdout.take().map(|s| BufReader::new(s).lines());
            let mut err_lines = child.stderr.take().map(|s| BufReader::new(s).lines());
            let mut out_done = out_lines.is_none();
            let mut err_done = err_lines.is_none();
            let mut cancelled = false;
            let mut tick = tokio::time::interval(tokio::time::Duration::from_millis(200));

            while !(out_done && err_done) {
                tokio::select! {
                    line = async { out_lines.as_mut().expect("guarded by out_done").next_line().await }, if !out_done => {
                        match line {
                            Ok(Some(line)) => {
                                parse_stats_line(&line, result);
                                ctx.log_info(&line, "rsync").await;
                            }
                            _ => out_done = true,
                        }
                    }
                    line = async { err_lines.as_mut().expect("guarded by err_done").next_line().await }, if !err_done => {
                        match line {
                            Ok(Some(line)) => ctx.log_warning(&line, "rsync").await,
                            _ => err_done = true,
                        }
                    }
                    _ = tick.tick() => {
                        if ctx.check_cancelled() {
                            cancelled = true;
                            break;
                        }
                    }
                }
            }

            // Kill the process if cancelled
            if cancelled || ctx.check_cancelled() {
                ctx.log_warning(&format!("Killing rsync process (PID {})", pid), "system").await;
                // Kill the entire process group (negative PID sends to group)
                #[cfg(unix)]
                if pid > 0 {
                    let _ = signal::killpg(Pid::from_raw(pid as i32), Signal::SIGTERM);
                }
                // Also try to kill via tokio
                let _ = child.kill().await;
                let _ = child.wait().await;
                return Err(ProviderError::Cancelled);
            }

            // Wait for process
            if let Ok(status) = child.wait().await {
                match status.code() {
                    Some(0) => {}
                    // 24 = "some files vanished before they could be transferred":
                    // normal when backing up live data, not a failed backup.
                    Some(24) => {
                        ctx.log_warning(
                            "rsync: some source files vanished during the transfer (exit code 24)",
                            "rsync",
                        ).await;
                    }
                    code => {
                        ctx.log_error(&format!("rsync failed with exit code: {:?}", code), "rsync").await;
                        result.error_count += 1;
                    }
                }
            }
        }

        Ok(())
    }
}

/// Make rsync print numbers the same way on every system.
///
/// rsync formats numbers for the locale ("3.575.620 bytes" in German), and
/// without a UTF-8 locale it prints non-ASCII file names as `\#303\#234`.
/// So numbers use the C format while the character set is kept, falling back
/// to C.UTF-8 when the service has no UTF-8 locale (bare units, containers).
fn set_rsync_locale(cmd: &mut Command) {
    let env = |key: &str| std::env::var(key).ok().filter(|v| !v.is_empty());
    for (key, value) in rsync_locale_env(env("LC_CTYPE").or_else(|| env("LANG"))) {
        cmd.env(key, value);
    }
    cmd.env_remove("LC_ALL");
}

/// The variables [`set_rsync_locale`] sets, given the character-set locale
/// rsync would get once LC_ALL is removed.
fn rsync_locale_env(ctype: Option<String>) -> Vec<(&'static str, &'static str)> {
    let mut vars = vec![("LC_NUMERIC", "C")];
    let utf8 = ctype.is_some_and(|v| {
        let v = v.to_ascii_lowercase();
        v.contains("utf-8") || v.contains("utf8")
    });
    if !utf8 {
        vars.push(("LC_CTYPE", "C.UTF-8"));
    }
    vars
}

/// Read a number from rsync output: "1,234" / "1.234" (thousand separators)
/// or, with `-h`, "3.00M" (units of 1000).
fn parse_rsync_number(value: &str) -> Option<i64> {
    let value = value.trim();
    let unit = match value.chars().last()? {
        'K' | 'k' => 1e3,
        'M' => 1e6,
        'G' => 1e9,
        'T' => 1e12,
        'P' => 1e15,
        _ => return value.replace([',', '.'], "").parse().ok(),
    };
    let number: f64 = value[..value.len() - 1].replace(',', "").parse().ok()?;
    Some((number * unit).round() as i64)
}

/// Parse rsync `--stats` output lines into the sync result.
fn parse_stats_line(line: &str, result: &mut SyncResult) {
    // Handle both "Number of files transferred" (old rsync) and
    // "Number of regular files transferred" (rsync 3.1+)
    if line.contains("files transferred") {
        if let Some(n) = line.split(':').nth(1).and_then(parse_rsync_number) {
            result.files_transferred += n;
        }
    } else if line.contains("Total transferred file size") {
        let bytes = line.split(':').nth(1).and_then(|v| v.split_whitespace().next());
        if let Some(b) = bytes.and_then(parse_rsync_number) {
            result.bytes_transferred += b;
        }
    } else if line.starts_with("sent ") && line.contains(" bytes") && result.bytes_transferred == 0 {
        // Fallback: parse "sent X bytes  received Y bytes" line
        let bytes = line.strip_prefix("sent ").and_then(|v| v.split_whitespace().next());
        if let Some(b) = bytes.and_then(parse_rsync_number) {
            result.bytes_transferred = b;
        }
    }
}

/// What is mounted exactly at a mount point, compared with the expected drive.
#[derive(Debug, PartialEq, Eq)]
enum MountState {
    /// The expected drive (and nothing stacked on it)
    Expected,
    Empty,
    /// Something else; the UUIDs found
    Other(String),
}

fn mount_state(uuid: &str, mounted_uuids: &[String]) -> MountState {
    match mounted_uuids {
        [] => MountState::Empty,
        [only] if only.eq_ignore_ascii_case(uuid.trim()) => MountState::Expected,
        found => MountState::Other(found.join(", ")),
    }
}

fn mount_point_target(mount_point: &str) -> &str {
    match mount_point.trim_end_matches('/') {
        "" => "/",
        t => t,
    }
}

/// UUIDs of the filesystems mounted exactly at `mount_point` (empty when
/// nothing is mounted there; an empty string for a filesystem without UUID).
async fn mounted_uuids(mount_point: &str) -> Result<Vec<String>, String> {
    let target = mount_point_target(mount_point);
    let output = Command::new("findmnt")
        .args(["-n", "-o", "UUID", "--mountpoint", target])
        .output()
        .await
        .map_err(|e| format!("Could not check mount point {}: {}", target, e))?;
    if !output.status.success() {
        // findmnt exits with 1 when nothing is mounted there
        return Ok(Vec::new());
    }
    Ok(String::from_utf8_lossy(&output.stdout).lines().map(|l| l.trim().to_string()).collect())
}

/// Make sure the drive `uuid` is mounted at `mount_point`, mounting it if
/// nothing is mounted there. Returns `true` if this call mounted it.
///
/// Decides by asking findmnt, not by reading mount's (translated) error
/// messages. Never mounts on top of a different device.
async fn ensure_mounted(uuid: &str, mount_point: &str) -> Result<bool, String> {
    let target = mount_point_target(mount_point);
    match mount_state(uuid, &mounted_uuids(target).await?) {
        MountState::Expected => return Ok(false),
        MountState::Other(found) => {
            return Err(format!(
                "A different device (UUID {}) is mounted at {}, expected drive {}.",
                if found.trim().is_empty() { "unknown" } else { &found },
                target,
                uuid
            ))
        }
        MountState::Empty => {}
    }

    // A missing directory makes mount fail with its own message below
    let _ = tokio::fs::create_dir_all(target).await;
    let output = Command::new("mount")
        .args(["-U", uuid, target])
        .output()
        .await
        .map_err(|e| format!("Mount error: {}", e))?;
    if output.status.success() {
        return Ok(true);
    }
    // Mounted by someone else in the meantime (for example an automounter)
    if mount_state(uuid, &mounted_uuids(target).await?) == MountState::Expected {
        return Ok(false);
    }
    Err(format!("Mount failed: {}", String::from_utf8_lossy(&output.stderr).trim()))
}

/// Check that the filesystem mounted exactly at `mount_point` is the drive
/// with `uuid`. Returns a user-facing error message otherwise.
pub(crate) async fn verify_drive_mounted(uuid: &str, mount_point: &str) -> Result<(), String> {
    let target = match mount_point.trim_end_matches('/') {
        "" => "/",
        t => t,
    };

    let output = Command::new("findmnt")
        .args(["-n", "-o", "UUID", "--mountpoint", target])
        .output()
        .await
        .map_err(|e| format!("Could not check mount point {}: {}", target, e))?;

    let mounted_uuid = String::from_utf8_lossy(&output.stdout).trim().to_string();
    mounted_uuid_matches(uuid, target, output.status.success(), &mounted_uuid)
}

fn mounted_uuid_matches(uuid: &str, target: &str, found: bool, mounted_uuid: &str) -> Result<(), String> {
    if !found {
        return Err(format!(
            "Drive {} is not mounted at {}. Refusing to back up into the empty mount-point directory. \
             Connect the drive, or check that it is not mounted somewhere else.",
            uuid, target
        ));
    }
    if !mounted_uuid.eq_ignore_ascii_case(uuid.trim()) {
        return Err(format!(
            "A different device (UUID {}) is mounted at {}, expected drive {}.",
            if mounted_uuid.is_empty() { "unknown" } else { mounted_uuid },
            target,
            uuid
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unmounts_only_drives_it_should() {
        // auto-mount on: only a drive this run mounted
        assert!(should_unmount(true, true, true, true));
        assert!(!should_unmount(true, true, false, true));
        // auto-mount off: the user asked to unmount their own mount
        assert!(should_unmount(false, true, false, true));
        assert!(!should_unmount(true, false, true, true));
        assert!(!should_unmount(true, true, true, false));
    }

    #[test]
    fn parses_rsync_stats() {
        let mut r = SyncResult::default();
        parse_stats_line("Number of regular files transferred: 1,234", &mut r);
        parse_stats_line("Total transferred file size: 5.678.901 bytes", &mut r);
        assert_eq!(r.files_transferred, 1234);
        assert_eq!(r.bytes_transferred, 5_678_901);
    }

    #[test]
    fn rsync_locale_keeps_utf8_and_fixes_numbers() {
        assert_eq!(rsync_locale_env(Some("de_DE.UTF-8".into())), vec![("LC_NUMERIC", "C")]);
        assert_eq!(rsync_locale_env(Some("en_US.utf8".into())), vec![("LC_NUMERIC", "C")]);
        let fallback = vec![("LC_NUMERIC", "C"), ("LC_CTYPE", "C.UTF-8")];
        assert_eq!(rsync_locale_env(None), fallback);
        assert_eq!(rsync_locale_env(Some("C".into())), fallback);
        assert_eq!(rsync_locale_env(Some("POSIX".into())), fallback);
    }

    #[test]
    fn parses_human_readable_stats() {
        let mut r = SyncResult::default();
        parse_stats_line("Total transferred file size: 3.00M bytes", &mut r);
        assert_eq!(r.bytes_transferred, 3_000_000);
        assert_eq!(parse_rsync_number("1.5K"), Some(1500));
        assert_eq!(parse_rsync_number("12,345,678"), Some(12_345_678));
        assert_eq!(parse_rsync_number("x"), None);
    }

    #[test]
    fn sent_line_is_fallback_only() {
        let mut r = SyncResult::default();
        parse_stats_line("sent 2,048 bytes  received 35 bytes", &mut r);
        assert_eq!(r.bytes_transferred, 2048);
        parse_stats_line("sent 9,999 bytes  received 35 bytes", &mut r);
        assert_eq!(r.bytes_transferred, 2048);
    }

    #[test]
    fn mount_state_tells_expected_empty_and_other() {
        let uuid = "1234-ABCD";
        assert_eq!(mount_state(uuid, &[]), MountState::Empty);
        assert_eq!(mount_state(uuid, &["1234-abcd".into()]), MountState::Expected);
        assert_eq!(mount_state(uuid, &["9999-0000".into()]), MountState::Other("9999-0000".into()));
        // Something stacked on top of the drive is not the expected state
        assert!(matches!(
            mount_state(uuid, &["1234-ABCD".into(), "9999-0000".into()]),
            MountState::Other(_)
        ));
        assert!(matches!(mount_state(uuid, &["".into()]), MountState::Other(_)));
    }

    #[test]
    fn mount_check_rejects_unmounted_or_wrong_drive() {
        let uuid = "1234-ABCD";
        assert!(mounted_uuid_matches(uuid, "/mnt/usb", true, "1234-abcd").is_ok());
        assert!(mounted_uuid_matches(uuid, "/mnt/usb", false, "").is_err());
        assert!(mounted_uuid_matches(uuid, "/mnt/usb", true, "9999-0000").is_err());
        assert!(mounted_uuid_matches(uuid, "/mnt/usb", true, "").is_err());
    }
}
