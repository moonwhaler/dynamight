//! Rsync-based sync provider for local and USB destinations

use super::{ProviderCapabilities, ProviderError, SyncContext, SyncProvider, SyncResult};
use crate::models::{CredentialData, DestinationConfig, LogLevel};
use async_trait::async_trait;
#[cfg(unix)]
use nix::sys::signal::{self, Signal};
#[cfg(unix)]
use nix::unistd::Pid;
use std::path::Path;
use std::process::Stdio;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

pub struct RsyncProvider;

impl RsyncProvider {
    pub fn new() -> Self {
        Self
    }

    /// Build rsync command arguments based on configuration
    fn build_rsync_args(&self, ctx: &SyncContext, fstype: &str) -> Vec<String> {
        let mut args = vec![];

        // Verbosity options
        match ctx.options.verbosity.as_str() {
            "quiet" => {
                args.push("-q".to_string());
            }
            "verbose" => {
                args.extend(["-vh".to_string(), "--stats".to_string(), "--progress".to_string()]);
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
                String::from_utf8_lossy(&o.stdout).trim().to_string()
            }
            _ => "unknown".to_string(),
        }
    }

    /// Mount a USB drive by UUID
    async fn mount_usb(&self, ctx: &SyncContext, uuid: &str, mount_point: &str) -> Result<(), ProviderError> {
        ctx.log_info(&format!("Mounting UUID {} to {}", uuid, mount_point), "mount").await;

        // Create mount point if needed
        if let Err(e) = tokio::fs::create_dir_all(mount_point).await {
            ctx.log_warning(&format!("Could not create mount point: {}", e), "mount").await;
        }

        let mount_result = Command::new("mount")
            .args(["-U", uuid, mount_point])
            .output()
            .await;

        match mount_result {
            Ok(output) if output.status.success() => {
                ctx.log_info("Mount successful", "mount").await;
                Ok(())
            }
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                if stderr.contains("already mounted") {
                    ctx.log_info("Drive already mounted", "mount").await;
                    Ok(())
                } else {
                    ctx.log_error(&format!("Mount failed: {}", stderr), "mount").await;
                    Err(ProviderError::ConfigError(format!("Mount failed: {}", stderr)))
                }
            }
            Err(e) => {
                ctx.log_error(&format!("Mount error: {}", e), "mount").await;
                Err(ProviderError::IoError(e))
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

        ctx.log_info("Starting backup job", "system").await;

        // Mount if needed
        if auto_mount {
            if let Some(uuid) = &usb_uuid {
                self.mount_usb(&ctx, uuid, &mount_point).await?;
            }
        }

        // Detect filesystem type
        let fstype = self.detect_filesystem(&mount_point).await;
        ctx.log_info(&format!("Filesystem type: {}", fstype), "system").await;

        // Build rsync args
        let rsync_args = self.build_rsync_args(&ctx, &fstype);
        ctx.log(LogLevel::Debug, &format!("Rsync args: {:?}", rsync_args), "rsync").await;

        // Execute rsync for each source directory
        for source_dir in &ctx.source_dirs {
            if !Path::new(source_dir).exists() {
                ctx.log_warning(&format!("Source directory does not exist: {}", source_dir), "rsync").await;
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

            // Check for cancellation
            if ctx.check_cancelled() {
                ctx.log_warning("Job cancelled, skipping remaining sources", "system").await;
                return Err(ProviderError::Cancelled);
            }

            // Run rsync
            let mut cmd = Command::new("rsync");
            cmd.args(&rsync_args)
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

            let stdout = child.stdout.take();
            let stderr = child.stderr.take();

            // Track if we need to kill the process
            let mut cancelled = false;

            // Stream stdout
            if let Some(stdout) = stdout {
                let reader = BufReader::new(stdout);
                let mut lines = reader.lines();

                loop {
                    let line = tokio::select! {
                        line_result = lines.next_line() => {
                            match line_result {
                                Ok(Some(line)) => line,
                                Ok(None) => break,
                                Err(_) => break,
                            }
                        }
                        _ = tokio::time::sleep(tokio::time::Duration::from_millis(100)) => {
                            if ctx.check_cancelled() {
                                cancelled = true;
                                break;
                            }
                            continue;
                        }
                    };

                    if ctx.check_cancelled() {
                        cancelled = true;
                        break;
                    }

                    // Parse rsync output for stats
                    if line.contains("Number of files transferred") {
                        if let Some(num) = line.split(':').nth(1) {
                            if let Ok(n) = num.trim().replace(',', "").parse::<i64>() {
                                result.files_transferred += n;
                            }
                        }
                    } else if line.contains("Total transferred file size") {
                        if let Some(size_str) = line.split(':').nth(1) {
                            if let Some(bytes) = size_str.split_whitespace().next() {
                                if let Ok(b) = bytes.replace(',', "").parse::<i64>() {
                                    result.bytes_transferred += b;
                                }
                            }
                        }
                    }

                    ctx.log_info(&line, "rsync").await;
                }
            }

            // Stream stderr (only if not cancelled)
            if !cancelled && !ctx.check_cancelled() {
                if let Some(stderr) = stderr {
                    let reader = BufReader::new(stderr);
                    let mut lines = reader.lines();

                    loop {
                        let line = tokio::select! {
                            line_result = lines.next_line() => {
                                match line_result {
                                    Ok(Some(line)) => line,
                                    Ok(None) => break,
                                    Err(_) => break,
                                }
                            }
                            _ = tokio::time::sleep(tokio::time::Duration::from_millis(100)) => {
                                if ctx.check_cancelled() {
                                    cancelled = true;
                                    break;
                                }
                                continue;
                            }
                        };

                        if ctx.check_cancelled() {
                            cancelled = true;
                            break;
                        }

                        ctx.log_warning(&line, "rsync").await;
                    }
                }
            } else {
                cancelled = true;
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
            }

            // Wait for process
            let status = child.wait().await.ok();

            if let Some(status) = status {
                if !status.success() && !ctx.check_cancelled() {
                    ctx.log_error(
                        &format!("rsync failed with exit code: {:?}", status.code()),
                        "rsync",
                    ).await;
                    result.error_count += 1;
                }
            }

            if ctx.check_cancelled() {
                return Err(ProviderError::Cancelled);
            }
        }

        // Unmount if needed
        if auto_unmount && usb_uuid.is_some() {
            self.unmount(&ctx, &mount_point).await;
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

    async fn is_cancelled(&self, _run_id: i64) -> bool {
        // Cancellation is now handled via ctx.check_cancelled()
        false
    }
}
