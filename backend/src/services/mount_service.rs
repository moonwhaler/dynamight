use serde::{Deserialize, Serialize};
use std::process::Command;
use thiserror::Error;
use walkdir::WalkDir;

#[derive(Debug, Error)]
pub enum MountError {
    #[error("Device not found: {0}")]
    DeviceNotFound(String),
    #[error("Mount failed: {0}")]
    MountFailed(String),
    #[error("Unmount failed: {0}")]
    UnmountFailed(String),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Parse error: {0}")]
    ParseError(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsbDrive {
    pub uuid: String,
    pub name: String,
    pub fstype: Option<String>,
    pub size: Option<String>,
    pub mountpoint: Option<String>,
    pub label: Option<String>,
    pub model: Option<String>,
    pub available: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MountPoint {
    pub path: String,
    pub device: String,
    pub fstype: String,
    pub options: String,
}

#[derive(Debug, Deserialize)]
struct LsblkOutput {
    blockdevices: Vec<LsblkDevice>,
}

#[derive(Debug, Deserialize)]
struct LsblkDevice {
    name: String,
    #[serde(default)]
    uuid: Option<String>,
    #[serde(default)]
    fstype: Option<String>,
    #[serde(default)]
    size: Option<String>,
    #[serde(default)]
    mountpoint: Option<String>,
    #[serde(default)]
    label: Option<String>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    tran: Option<String>,
    #[serde(default)]
    children: Option<Vec<LsblkDevice>>,
}

#[derive(Clone)]
pub struct MountService;

impl MountService {
    pub fn new() -> Self {
        Self
    }

    /// Get device path from UUID
    pub fn get_device_by_uuid(&self, uuid: &str) -> Result<String, MountError> {
        let output = Command::new("blkid").args(["-U", uuid]).output()?;

        if output.status.success() {
            let device = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if device.is_empty() {
                Err(MountError::DeviceNotFound(uuid.to_string()))
            } else {
                Ok(device)
            }
        } else {
            Err(MountError::DeviceNotFound(uuid.to_string()))
        }
    }

    /// Mount a device by UUID
    pub fn mount_by_uuid(&self, uuid: &str, mount_point: &str) -> Result<(), MountError> {
        // Create mount point if needed
        std::fs::create_dir_all(mount_point)?;

        // Check if already mounted
        if self.is_mounted(mount_point)? {
            return Ok(());
        }

        let status = Command::new("mount")
            .args(["-U", uuid, mount_point])
            .status()?;

        if status.success() {
            Ok(())
        } else {
            Err(MountError::MountFailed(format!(
                "Failed to mount UUID {} to {}",
                uuid, mount_point
            )))
        }
    }

    /// Unmount a mount point
    pub fn unmount(&self, mount_point: &str) -> Result<(), MountError> {
        // Sync first
        let _ = Command::new("sync").status();

        // Small delay
        std::thread::sleep(std::time::Duration::from_millis(500));

        let status = Command::new("umount").arg(mount_point).status()?;

        if status.success() {
            Ok(())
        } else {
            Err(MountError::UnmountFailed(mount_point.to_string()))
        }
    }

    /// Check if a path is mounted
    pub fn is_mounted(&self, path: &str) -> Result<bool, MountError> {
        let status = Command::new("mountpoint").args(["-q", path]).status()?;
        Ok(status.success())
    }

    /// Get filesystem type for a mount point
    pub fn get_filesystem_type(&self, mount_point: &str) -> Result<String, MountError> {
        let output = Command::new("findmnt")
            .args(["-n", "-o", "FSTYPE", "--target", mount_point])
            .output()?;

        if output.status.success() {
            // Take only the first line in case of multiple mounts (bind mounts, submounts)
            Ok(String::from_utf8_lossy(&output.stdout)
                .lines()
                .next()
                .unwrap_or("unknown")
                .to_string())
        } else {
            Ok("unknown".to_string())
        }
    }

    /// Get available space for a mounted filesystem using df
    fn get_available_space_mounted(mountpoint: &str) -> Option<String> {
        let output = Command::new("df")
            .args(["-h", "--output=avail", mountpoint])
            .output()
            .ok()?;

        if !output.status.success() {
            return None;
        }

        // df output has a header line, so take the second line
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .nth(1)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    }

    /// Get available space for an unmounted ext2/3/4 filesystem by reading the superblock
    fn get_available_space_from_device(device_name: &str, fstype: Option<&str>) -> Option<String> {
        // Only support ext filesystems for now (most common on Linux USB drives)
        let fs = fstype?;
        if !fs.starts_with("ext") {
            return None;
        }

        let device_path = format!("/dev/{}", device_name);
        let output = Command::new("dumpe2fs")
            .args(["-h", &device_path])
            .output()
            .ok()?;

        if !output.status.success() {
            return None;
        }

        let stdout = String::from_utf8_lossy(&output.stdout);

        // Parse Block size, Block count, and Free blocks from dumpe2fs output
        let mut block_size: Option<u64> = None;
        let mut free_blocks: Option<u64> = None;

        for line in stdout.lines() {
            if line.starts_with("Block size:") {
                block_size = line
                    .split(':')
                    .nth(1)
                    .and_then(|s| s.trim().parse().ok());
            } else if line.starts_with("Free blocks:") {
                free_blocks = line
                    .split(':')
                    .nth(1)
                    .and_then(|s| s.trim().parse().ok());
            }
        }

        // Calculate free space in bytes, then format human-readable
        let free_bytes = free_blocks? * block_size?;
        Some(Self::format_bytes(free_bytes))
    }

    /// Format bytes as human-readable string (matches df -h output style)
    fn format_bytes(bytes: u64) -> String {
        const KIB: u64 = 1024;
        const MIB: u64 = KIB * 1024;
        const GIB: u64 = MIB * 1024;
        const TIB: u64 = GIB * 1024;

        if bytes >= TIB {
            format!("{:.1}T", bytes as f64 / TIB as f64)
        } else if bytes >= GIB {
            format!("{:.1}G", bytes as f64 / GIB as f64)
        } else if bytes >= MIB {
            format!("{:.1}M", bytes as f64 / MIB as f64)
        } else if bytes >= KIB {
            format!("{:.1}K", bytes as f64 / KIB as f64)
        } else {
            format!("{}B", bytes)
        }
    }

    /// Get available space - tries mounted path first, falls back to reading device superblock
    fn get_available_space(
        mountpoint: Option<&str>,
        device_name: &str,
        fstype: Option<&str>,
    ) -> Option<String> {
        // If mounted, use df (faster and works for all filesystem types)
        if let Some(mp) = mountpoint {
            if let Some(space) = Self::get_available_space_mounted(mp) {
                return Some(space);
            }
        }

        // For unmounted drives, try reading the superblock directly
        Self::get_available_space_from_device(device_name, fstype)
    }

    /// List USB drives
    pub fn list_usb_drives(&self) -> Result<Vec<UsbDrive>, MountError> {
        let output = Command::new("lsblk")
            .args([
                "-J",
                "-o",
                "NAME,UUID,FSTYPE,SIZE,MOUNTPOINT,LABEL,MODEL,TRAN",
            ])
            .output()?;

        if !output.status.success() {
            return Ok(vec![]);
        }

        let lsblk: LsblkOutput = serde_json::from_slice(&output.stdout)
            .map_err(|e| MountError::ParseError(e.to_string()))?;

        let mut drives = Vec::new();

        for device in lsblk.blockdevices {
            // Check if it's a USB device
            if device.tran.as_deref() == Some("usb") {
                // Trim whitespace from model (lsblk often pads with spaces)
                let device_model = device.model.map(|m| m.trim().to_string()).filter(|m| !m.is_empty());

                // Get partitions
                if let Some(children) = device.children {
                    for partition in children {
                        if let Some(uuid) = partition.uuid {
                            let available = Self::get_available_space(
                                partition.mountpoint.as_deref(),
                                &partition.name,
                                partition.fstype.as_deref(),
                            );
                            drives.push(UsbDrive {
                                uuid,
                                name: partition.name,
                                fstype: partition.fstype,
                                size: partition.size,
                                mountpoint: partition.mountpoint,
                                label: partition.label,
                                // Model comes from parent device, not partition
                                model: device_model.clone(),
                                available,
                            });
                        }
                    }
                } else if let Some(uuid) = device.uuid {
                    // Device itself has a filesystem
                    let available = Self::get_available_space(
                        device.mountpoint.as_deref(),
                        &device.name,
                        device.fstype.as_deref(),
                    );
                    drives.push(UsbDrive {
                        uuid,
                        name: device.name,
                        fstype: device.fstype,
                        size: device.size,
                        mountpoint: device.mountpoint,
                        label: device.label,
                        model: device_model,
                        available,
                    });
                }
            }
        }

        Ok(drives)
    }

    /// List current mount points
    pub fn list_mounts(&self) -> Result<Vec<MountPoint>, MountError> {
        let output = Command::new("findmnt")
            .args(["-J", "-l", "-t", "ext4,ext3,ntfs,vfat,exfat,btrfs,xfs"])
            .output()?;

        if !output.status.success() {
            return Ok(vec![]);
        }

        #[derive(Deserialize)]
        struct FindmntOutput {
            filesystems: Vec<FindmntFs>,
        }

        #[derive(Deserialize)]
        struct FindmntFs {
            target: String,
            source: String,
            fstype: String,
            options: String,
        }

        let findmnt: FindmntOutput = serde_json::from_slice(&output.stdout)
            .map_err(|e| MountError::ParseError(e.to_string()))?;

        Ok(findmnt
            .filesystems
            .into_iter()
            .map(|fs| MountPoint {
                path: fs.target,
                device: fs.source,
                fstype: fs.fstype,
                options: fs.options,
            })
            .collect())
    }

    /// Browse a directory path
    pub fn browse_path(&self, path: &str) -> Result<Vec<DirectoryEntry>, MountError> {
        let entries = std::fs::read_dir(path)?;
        let mut result = Vec::new();

        for entry in entries.flatten() {
            // Skip entries we can't read metadata for (permission denied, broken symlinks, etc.)
            let metadata = match entry.metadata() {
                Ok(m) => m,
                Err(_) => continue,
            };
            let name = entry.file_name().to_string_lossy().to_string();
            let is_dir = metadata.is_dir();

            // Get modified time as Unix timestamp
            let modified = metadata
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs() as i64);

            // Extract file extension for files only
            let extension = if !is_dir {
                std::path::Path::new(&name)
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .map(|s| s.to_lowercase())
            } else {
                None
            };

            result.push(DirectoryEntry {
                name,
                path: entry.path().to_string_lossy().to_string(),
                is_dir,
                size: if metadata.is_file() {
                    Some(metadata.len())
                } else {
                    None
                },
                modified,
                extension,
            });
        }

        // Sort directories first, then files
        result.sort_by(|a, b| match (a.is_dir, b.is_dir) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
        });

        Ok(result)
    }

    /// Search recursively for files matching a query
    pub fn search_path(
        &self,
        base_path: &str,
        query: &str,
        max_results: usize,
        max_depth: usize,
    ) -> Result<(Vec<DirectoryEntry>, bool), MountError> {
        let query_lower = query.to_lowercase();
        let is_glob = query_lower.contains('*') || query_lower.contains('?');
        let mut results = Vec::new();
        let mut truncated = false;

        let walker = WalkDir::new(base_path)
            .max_depth(max_depth)
            .follow_links(false)
            .into_iter()
            .filter_entry(|e| {
                // Skip hidden directories (but not the root)
                if e.depth() > 0 && e.file_name().to_string_lossy().starts_with('.') {
                    return false;
                }
                true
            });

        for entry in walker {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };

            if entry.depth() == 0 {
                continue;
            }

            let name = entry.file_name().to_string_lossy().to_string();
            let name_lower = name.to_lowercase();
            let matches = if is_glob {
                glob_match(&query_lower, &name_lower)
            } else {
                name_lower.contains(&query_lower)
            };
            if !matches {
                continue;
            }

            let metadata = match entry.metadata() {
                Ok(m) => m,
                Err(_) => continue,
            };

            let is_dir = metadata.is_dir();
            let modified = metadata
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs() as i64);
            let extension = if !is_dir {
                std::path::Path::new(&name)
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .map(|s| s.to_lowercase())
            } else {
                None
            };

            results.push(DirectoryEntry {
                name,
                path: entry.path().to_string_lossy().to_string(),
                is_dir,
                size: if metadata.is_file() {
                    Some(metadata.len())
                } else {
                    None
                },
                modified,
                extension,
            });

            if results.len() >= max_results {
                truncated = true;
                break;
            }
        }

        Ok((results, truncated))
    }

    /// Generate a safe mount point path from a drive label
    pub fn generate_mount_point(&self, label: Option<&str>, uuid: &str) -> String {
        let base = label
            .filter(|l| !l.is_empty())
            .unwrap_or(uuid);

        // Sanitize the name: keep only alphanumeric, dash, underscore
        let sanitized: String = base
            .chars()
            .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
            .collect();

        // Limit length and ensure non-empty
        let name = if sanitized.is_empty() {
            uuid[..8.min(uuid.len())].to_string()
        } else {
            sanitized.chars().take(32).collect()
        };

        format!("/mnt/{}", name)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct DirectoryEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub size: Option<u64>,
    pub modified: Option<i64>,
    pub extension: Option<String>,
}

impl Default for MountService {
    fn default() -> Self {
        Self::new()
    }
}

/// Simple glob matching supporting `*` (any sequence) and `?` (single char).
/// Both pattern and text should be pre-lowercased for case-insensitive matching.
fn glob_match(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let t: Vec<char> = text.chars().collect();
    let (plen, tlen) = (p.len(), t.len());
    let (mut pi, mut ti) = (0, 0);
    let (mut star_pi, mut star_ti) = (usize::MAX, 0);

    while ti < tlen {
        if pi < plen && (p[pi] == '?' || p[pi] == t[ti]) {
            pi += 1;
            ti += 1;
        } else if pi < plen && p[pi] == '*' {
            star_pi = pi;
            star_ti = ti;
            pi += 1;
        } else if star_pi != usize::MAX {
            pi = star_pi + 1;
            star_ti += 1;
            ti = star_ti;
        } else {
            return false;
        }
    }

    while pi < plen && p[pi] == '*' {
        pi += 1;
    }

    pi == plen
}
