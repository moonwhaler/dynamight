//! S3 and S3-compatible storage provider (AWS, MinIO, Backblaze B2)

use super::{ProviderCapabilities, ProviderError, SyncContext, SyncProvider, SyncResult, TestConnectionResult};
use crate::models::{CredentialData, DestinationConfig};
use async_trait::async_trait;
use std::path::Path;
use std::sync::Arc;

pub struct S3Provider;

impl S3Provider {
    pub fn new() -> Self {
        Self
    }
}

impl Default for S3Provider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SyncProvider for S3Provider {
    fn provider_type(&self) -> &'static str {
        "s3"
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            supports_delete: true,
            supports_compression: false,
            supports_checksum: true,
            supports_bandwidth_limit: false,
            supports_exclude_patterns: true,
            supports_incremental: true,
            supports_dry_run: true,
            requires_credentials: true,
        }
    }

    fn validate_config(
        &self,
        destination: &DestinationConfig,
        credential: Option<&CredentialData>,
    ) -> Result<(), ProviderError> {
        match destination {
            DestinationConfig::S3 { bucket, region, .. } => {
                if bucket.is_empty() {
                    return Err(ProviderError::ConfigError("Bucket name is required".to_string()));
                }
                if region.is_empty() {
                    return Err(ProviderError::ConfigError("Region is required".to_string()));
                }
            }
            _ => {
                return Err(ProviderError::ConfigError(
                    "S3Provider only supports S3 destination".to_string(),
                ))
            }
        }

        // Validate credentials
        match credential {
            Some(CredentialData::S3 { access_key_id, secret_access_key }) => {
                if access_key_id.is_empty() || secret_access_key.is_empty() {
                    return Err(ProviderError::CredentialError(
                        "S3 credentials are incomplete".to_string(),
                    ));
                }
            }
            None => {
                return Err(ProviderError::CredentialError(
                    "S3 credentials are required".to_string(),
                ))
            }
            _ => {
                return Err(ProviderError::CredentialError(
                    "Invalid credential type for S3".to_string(),
                ))
            }
        }

        Ok(())
    }

    async fn test_connection(
        &self,
        destination: &DestinationConfig,
        credential: Option<&CredentialData>,
    ) -> Result<TestConnectionResult, ProviderError> {
        self.validate_config(destination, credential)?;

        let (bucket, region, endpoint) = match destination {
            DestinationConfig::S3 { bucket, region, endpoint, .. } => {
                (bucket.clone(), region.clone(), endpoint.clone())
            }
            _ => return Err(ProviderError::ConfigError("Invalid destination type".to_string())),
        };

        let (access_key_id, secret_access_key) = match credential {
            Some(CredentialData::S3 { access_key_id, secret_access_key }) => {
                (access_key_id.clone(), secret_access_key.clone())
            }
            _ => return Err(ProviderError::CredentialError("S3 credentials required".to_string())),
        };

        // Build AWS config
        let config_builder = aws_config::defaults(aws_config::BehaviorVersion::latest())
            .region(aws_config::Region::new(region.clone()))
            .credentials_provider(aws_credential_types::Credentials::new(
                access_key_id,
                secret_access_key,
                None,
                None,
                "dynamight",
            ));

        let config = config_builder.load().await;
        let mut s3_config_builder = aws_sdk_s3::config::Builder::from(&config);

        if let Some(ep) = &endpoint {
            s3_config_builder = s3_config_builder.endpoint_url(ep).force_path_style(true);
        }

        let s3_client = aws_sdk_s3::Client::from_conf(s3_config_builder.build());

        // Try to head the bucket to verify access
        match s3_client.head_bucket().bucket(&bucket).send().await {
            Ok(_) => Ok(TestConnectionResult {
                success: true,
                message: "Successfully connected to S3 bucket".to_string(),
                details: Some(format!("Bucket: {}, Region: {}", bucket, region)),
            }),
            Err(e) => {
                let error_msg = format!("{}", e);
                if error_msg.contains("403") || error_msg.contains("Access Denied") {
                    Err(ProviderError::CredentialError(
                        "Access denied. Check your credentials and bucket permissions.".to_string(),
                    ))
                } else if error_msg.contains("404") || error_msg.contains("NoSuchBucket") {
                    Err(ProviderError::ConfigError(
                        format!("Bucket '{}' does not exist or is not accessible.", bucket),
                    ))
                } else {
                    Err(ProviderError::ConnectionError(format!(
                        "Failed to connect to S3: {}",
                        error_msg
                    )))
                }
            }
        }
    }

    async fn sync(&self, ctx: Arc<SyncContext>) -> Result<SyncResult, ProviderError> {
        let mut result = SyncResult::default();

        let (bucket, prefix, region, endpoint, storage_class) = match &ctx.destination {
            DestinationConfig::S3 {
                bucket,
                prefix,
                region,
                endpoint,
                storage_class,
            } => (
                bucket.clone(),
                prefix.clone(),
                region.clone(),
                endpoint.clone(),
                storage_class.clone(),
            ),
            _ => {
                return Err(ProviderError::ConfigError(
                    "S3Provider only supports S3 destination".to_string(),
                ))
            }
        };

        let (access_key_id, secret_access_key) = match &ctx.credential {
            Some(CredentialData::S3 { access_key_id, secret_access_key }) => {
                (access_key_id.clone(), secret_access_key.clone())
            }
            _ => {
                return Err(ProviderError::CredentialError(
                    "S3 credentials required".to_string(),
                ))
            }
        };

        ctx.log_info("Starting S3 sync", "s3").await;
        ctx.log_info(&format!("Bucket: {}, Region: {}", bucket, region), "s3").await;

        if let Some(ep) = &endpoint {
            ctx.log_info(&format!("Custom endpoint: {}", ep), "s3").await;
        }

        // Build AWS config
        let config_builder = aws_config::defaults(aws_config::BehaviorVersion::latest())
            .region(aws_config::Region::new(region.clone()))
            .credentials_provider(aws_credential_types::Credentials::new(
                access_key_id,
                secret_access_key,
                None,
                None,
                "dynamight",
            ));

        let config = config_builder.load().await;

        let mut s3_config_builder = aws_sdk_s3::config::Builder::from(&config);

        if let Some(ep) = endpoint {
            s3_config_builder = s3_config_builder.endpoint_url(ep).force_path_style(true);
        }

        let s3_client = aws_sdk_s3::Client::from_conf(s3_config_builder.build());

        // Sync each source directory
        for source_dir in &ctx.source_dirs {
            if !Path::new(source_dir).exists() {
                ctx.log_warning(&format!("Source directory does not exist: {}", source_dir), "s3").await;
                result.error_count += 1;
                continue;
            }

            let dest_name = Path::new(source_dir)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "backup".to_string());

            let key_prefix = if prefix.is_empty() {
                dest_name.clone()
            } else {
                format!("{}/{}", prefix.trim_end_matches('/'), dest_name)
            };

            ctx.log_info(&format!("Syncing {} -> s3://{}/{}", source_dir, bucket, key_prefix), "s3").await;

            // Walk the source directory and upload files
            match self.sync_directory(&s3_client, source_dir, &bucket, &key_prefix, &storage_class, &ctx, &mut result).await {
                Ok(_) => {}
                Err(e) => {
                    ctx.log_error(&format!("Failed to sync {}: {}", source_dir, e), "s3").await;
                    result.error_count += 1;
                }
            }
        }

        result.success = result.error_count == 0;
        ctx.log_info(
            &format!(
                "S3 sync complete. Files: {}, Bytes: {}, Errors: {}",
                result.files_transferred, result.bytes_transferred, result.error_count
            ),
            "s3",
        ).await;

        Ok(result)
    }
}

impl S3Provider {
    #[allow(clippy::too_many_arguments)]
    async fn sync_directory(
        &self,
        client: &aws_sdk_s3::Client,
        source_dir: &str,
        bucket: &str,
        key_prefix: &str,
        storage_class: &Option<String>,
        ctx: &SyncContext,
        result: &mut SyncResult,
    ) -> Result<(), ProviderError> {
        use tokio::fs;

        let mut entries = fs::read_dir(source_dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            let file_name = entry.file_name().to_string_lossy().to_string();

            // Check exclude patterns
            if ctx.options.exclude_patterns.iter().any(|p| {
                glob::Pattern::new(p).map(|pat| pat.matches(&file_name)).unwrap_or(false)
            }) {
                continue;
            }

            if path.is_dir() {
                let new_prefix = format!("{}/{}", key_prefix, file_name);
                Box::pin(self.sync_directory(client, path.to_str().unwrap(), bucket, &new_prefix, storage_class, ctx, result)).await?;
            } else if path.is_file() {
                let key = format!("{}/{}", key_prefix, file_name);

                if ctx.options.dry_run {
                    ctx.log_info(&format!("[DRY RUN] Would upload: {}", key), "s3").await;
                    result.files_transferred += 1;
                    continue;
                }

                // Read file and upload
                match fs::read(&path).await {
                    Ok(data) => {
                        let file_size = data.len() as i64;

                        let mut put_request = client
                            .put_object()
                            .bucket(bucket)
                            .key(&key)
                            .body(data.into());

                        if let Some(sc) = storage_class {
                            put_request = put_request.storage_class(
                                aws_sdk_s3::types::StorageClass::from(sc.as_str())
                            );
                        }

                        match put_request.send().await {
                            Ok(_) => {
                                ctx.log_info(&format!("Uploaded: {}", key), "s3").await;
                                result.files_transferred += 1;
                                result.bytes_transferred += file_size;
                            }
                            Err(e) => {
                                ctx.log_error(&format!("Failed to upload {}: {}", key, e), "s3").await;
                                result.error_count += 1;
                            }
                        }
                    }
                    Err(e) => {
                        ctx.log_error(&format!("Failed to read {}: {}", path.display(), e), "s3").await;
                        result.error_count += 1;
                    }
                }
            }
        }

        Ok(())
    }
}
