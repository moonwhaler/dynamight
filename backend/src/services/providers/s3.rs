//! S3 and S3-compatible storage provider (AWS, MinIO, Backblaze B2, Cloudflare R2, ...)
//!
//! Sync is incremental: the objects under the destination prefix are listed
//! once; files of the same size whose object is not older than the local file
//! are skipped (the rule `aws s3 sync` uses). Files are streamed from disk,
//! and files above `MULTIPART_THRESHOLD` use multipart upload (a single
//! PutObject is limited to 5 GB). With "delete extraneous" enabled, objects
//! that no longer exist locally are deleted.
//!
//! For custom endpoints, the SDK's default request checksums (CRC32 headers,
//! sent since aws-sdk-s3 1.69) are limited to operations that require them,
//! since not every S3-compatible service accepts them:
//! <https://docs.aws.amazon.com/sdkref/latest/guide/feature-dataintegrity.html>

use super::common::{self, needs_upload, LocalEntry, RemoteFile};
use super::{ProviderCapabilities, ProviderError, SyncContext, SyncProvider, SyncResult, TestConnectionResult};
use crate::models::{CredentialData, DestinationConfig};
use async_trait::async_trait;
use aws_sdk_s3::primitives::ByteStream;
use aws_sdk_s3::types::{CompletedMultipartUpload, CompletedPart, Delete, ObjectIdentifier};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

/// Files larger than this use multipart upload
const MULTIPART_THRESHOLD: u64 = 64 * 1024 * 1024;
/// Minimum multipart part size (S3 requires >= 5 MiB except for the last part)
const MIN_PART_SIZE: u64 = 64 * 1024 * 1024;
/// S3 allows at most 10,000 parts per upload
const MAX_PARTS: u64 = 10_000;
/// DeleteObjects accepts at most 1,000 keys per request
const DELETE_BATCH: usize = 1000;

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

/// Part size for a multipart upload of `size` bytes: at least `MIN_PART_SIZE`,
/// large enough to stay within `MAX_PARTS`, rounded up to whole MiB.
fn part_size_for(size: u64) -> u64 {
    let needed = size.div_ceil(MAX_PARTS);
    let mib = 1024 * 1024;
    needed.max(MIN_PART_SIZE).div_ceil(mib) * mib
}

/// Object key prefix for a source directory (no trailing slash).
fn key_prefix_for(prefix: &str, dest_name: &str) -> String {
    let prefix = prefix.trim_matches('/');
    if prefix.is_empty() {
        dest_name.to_string()
    } else {
        format!("{}/{}", prefix, dest_name)
    }
}

async fn build_client(
    region: &str,
    endpoint: &Option<String>,
    access_key_id: String,
    secret_access_key: String,
) -> aws_sdk_s3::Client {
    let config = aws_config::defaults(aws_config::BehaviorVersion::latest())
        .region(aws_config::Region::new(region.to_string()))
        .credentials_provider(aws_credential_types::Credentials::new(
            access_key_id,
            secret_access_key,
            None,
            None,
            "dynamight",
        ))
        .load()
        .await;

    let mut builder = aws_sdk_s3::config::Builder::from(&config);
    if let Some(ep) = endpoint.as_deref().filter(|e| !e.trim().is_empty()) {
        builder = builder
            .endpoint_url(ep.trim())
            .force_path_style(true)
            .request_checksum_calculation(aws_sdk_s3::config::RequestChecksumCalculation::WhenRequired)
            .response_checksum_validation(aws_sdk_s3::config::ResponseChecksumValidation::WhenRequired);
    }
    aws_sdk_s3::Client::from_conf(builder.build())
}

/// Readable error text of an SDK error (service error code and message, or
/// the full error chain for transport errors).
fn sdk_error_details<E>(e: &aws_sdk_s3::error::SdkError<E, aws_sdk_s3::config::http::HttpResponse>) -> String
where
    E: aws_sdk_s3::error::ProvideErrorMetadata + std::error::Error + 'static,
{
    use aws_sdk_s3::error::SdkError;
    let text = match e {
        SdkError::ServiceError(se) => {
            let code = se.err().code().unwrap_or("").to_string();
            let msg = se.err().message().unwrap_or("").to_string();
            let status = se.raw().status().as_u16();
            match (code.is_empty(), msg.is_empty()) {
                (true, true) => format!("HTTP {}: {}", status, aws_sdk_s3::error::DisplayErrorContext(e)),
                (false, true) => format!("{} (HTTP {})", code, status),
                _ => format!("{}: {} (HTTP {})", code, msg, status),
            }
        }
        other => aws_sdk_s3::error::DisplayErrorContext(other).to_string(),
    };
    // Unparseable responses include the whole raw response; keep logs readable
    if text.chars().count() > 600 {
        format!("{}...", text.chars().take(600).collect::<String>())
    } else {
        text
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
            supports_checksum: false,
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

        match credential {
            Some(CredentialData::S3 { access_key_id, secret_access_key }) => {
                if access_key_id.is_empty() || secret_access_key.is_empty() {
                    return Err(ProviderError::CredentialError("S3 credentials are incomplete".to_string()));
                }
            }
            None => return Err(ProviderError::CredentialError("S3 credentials are required".to_string())),
            _ => return Err(ProviderError::CredentialError("Invalid credential type for S3".to_string())),
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
            DestinationConfig::S3 { bucket, region, endpoint, .. } => (bucket.clone(), region.clone(), endpoint.clone()),
            _ => return Err(ProviderError::ConfigError("Invalid destination type".to_string())),
        };
        let (access_key_id, secret_access_key) = match credential {
            Some(CredentialData::S3 { access_key_id, secret_access_key }) => {
                (access_key_id.clone(), secret_access_key.clone())
            }
            _ => return Err(ProviderError::CredentialError("S3 credentials required".to_string())),
        };

        let client = build_client(&region, &endpoint, access_key_id, secret_access_key).await;

        match client.head_bucket().bucket(&bucket).send().await {
            Ok(_) => Ok(TestConnectionResult {
                success: true,
                message: "Successfully connected to S3 bucket".to_string(),
                details: Some(format!("Bucket: {}, Region: {}", bucket, region)),
                host_key_fingerprint: None,
            }),
            Err(e) => {
                // HeadBucket responses have no body, so use the HTTP status
                let status = e.raw_response().map(|r| r.status().as_u16());
                let detail = sdk_error_details(&e);
                match status {
                    Some(401) | Some(403) => Err(ProviderError::CredentialError(
                        "Access denied. Check your credentials and bucket permissions.".to_string(),
                    )),
                    Some(404) => Err(ProviderError::ConfigError(format!(
                        "Bucket '{}' does not exist or is not accessible.",
                        bucket
                    ))),
                    Some(301) => Err(ProviderError::ConfigError(format!(
                        "Bucket '{}' is in a different region than '{}'.",
                        bucket, region
                    ))),
                    _ => Err(ProviderError::ConnectionError(format!("Failed to connect to S3: {}", detail))),
                }
            }
        }
    }

    async fn sync(&self, ctx: Arc<SyncContext>) -> Result<SyncResult, ProviderError> {
        let mut result = SyncResult::default();

        let (bucket, prefix, region, endpoint, storage_class) = match &ctx.destination {
            DestinationConfig::S3 { bucket, prefix, region, endpoint, storage_class } => {
                (bucket.clone(), prefix.clone(), region.clone(), endpoint.clone(), storage_class.clone())
            }
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
            _ => return Err(ProviderError::CredentialError("S3 credentials required".to_string())),
        };

        ctx.log_info("Starting S3 sync", "s3").await;
        ctx.log_info(&format!("Bucket: {}, Region: {}", bucket, region), "s3").await;
        if let Some(ep) = &endpoint {
            ctx.log_info(&format!("Custom endpoint: {}", ep), "s3").await;
        }

        let client = build_client(&region, &endpoint, access_key_id, secret_access_key).await;
        let storage_class = storage_class
            .filter(|s| !s.is_empty())
            .map(|sc| aws_sdk_s3::types::StorageClass::from(sc.as_str()));

        for source_dir in &ctx.source_dirs {
            if ctx.check_cancelled() {
                ctx.log_warning("Sync cancelled by user", "s3").await;
                return Err(ProviderError::Cancelled);
            }
            if !Path::new(source_dir).exists() {
                ctx.log_warning(&format!("Source directory does not exist: {}", source_dir), "s3").await;
                result.error_count += 1;
                continue;
            }

            let dest_name = Path::new(source_dir)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "backup".to_string());
            let key_prefix = key_prefix_for(&prefix, &dest_name);

            ctx.log_info(&format!("Syncing {} -> s3://{}/{}", source_dir, bucket, key_prefix), "s3").await;

            let target = Target { client: &client, bucket: &bucket, key_prefix: &key_prefix, storage_class: &storage_class };
            match self.sync_source(&ctx, &target, source_dir, &mut result).await {
                Ok(()) => {}
                Err(ProviderError::Cancelled) => return Err(ProviderError::Cancelled),
                Err(e) => {
                    ctx.log_error(&format!("Failed to sync {}: {}", source_dir, e), "s3").await;
                    result.error_count += 1;
                }
            }
        }

        result.success = result.error_count == 0;
        ctx.log_info(
            &format!(
                "S3 sync complete. Files: {}, Bytes: {}, Deleted: {}, Errors: {}",
                result.files_transferred, result.bytes_transferred, result.files_deleted, result.error_count
            ),
            "s3",
        )
        .await;

        Ok(result)
    }
}

struct Target<'a> {
    client: &'a aws_sdk_s3::Client,
    bucket: &'a str,
    key_prefix: &'a str,
    storage_class: &'a Option<aws_sdk_s3::types::StorageClass>,
}

impl S3Provider {
    async fn sync_source(
        &self,
        ctx: &SyncContext,
        target: &Target<'_>,
        source_dir: &str,
        result: &mut SyncResult,
    ) -> Result<(), ProviderError> {
        let dry_run = ctx.options.dry_run;
        let local = common::walk_source_async(source_dir, &ctx.options).await?;
        for msg in &local.skipped {
            ctx.log_warning(msg, "s3").await;
        }

        let remote = self.list_objects(ctx, target).await?;

        let mut skipped = 0u64;
        for file in local.files() {
            if ctx.check_cancelled() {
                return Err(ProviderError::Cancelled);
            }
            let key = format!("{}/{}", target.key_prefix, file.rel);
            if !needs_upload(file, remote.get(&file.rel)) {
                skipped += 1;
                continue;
            }
            if dry_run {
                ctx.log_info(&format!("[DRY RUN] Would upload: {}", key), "s3").await;
                result.files_transferred += 1;
                result.bytes_transferred += file.size as i64;
                continue;
            }

            let upload = if file.size > MULTIPART_THRESHOLD {
                self.multipart_upload(ctx, target, &key, file).await
            } else {
                self.put_object(target, &key, file).await
            };
            match upload {
                Ok(()) => {
                    ctx.log_info(&format!("Uploaded: {}", key), "s3").await;
                    result.files_transferred += 1;
                    result.bytes_transferred += file.size as i64;
                }
                Err(ProviderError::Cancelled) => return Err(ProviderError::Cancelled),
                Err(e) => {
                    ctx.log_error(&format!("Failed to upload {}: {}", key, e), "s3").await;
                    result.error_count += 1;
                }
            }
        }
        if skipped > 0 {
            ctx.log_info(&format!("{} unchanged file(s) skipped", skipped), "s3").await;
        }

        if ctx.options.delete_extraneous {
            let local_paths = local.rel_paths();
            let excludes = common::ExcludeMatcher::new(&ctx.options);
            let mut stale: Vec<String> = remote
                .keys()
                .filter(|rel| !local_paths.contains(*rel) && !excludes.rel_excluded(source_dir, rel))
                .cloned()
                .collect();
            stale.sort();

            if dry_run {
                for rel in &stale {
                    ctx.log_info(&format!("[DRY RUN] Would delete: {}/{}", target.key_prefix, rel), "s3").await;
                }
                result.files_deleted += stale.len() as i64;
            } else if !stale.is_empty() {
                let keys: Vec<String> = stale.iter().map(|rel| format!("{}/{}", target.key_prefix, rel)).collect();
                let (deleted, errors) = self.delete_keys(ctx, target, &keys).await;
                result.files_deleted += deleted;
                result.error_count += errors;
            }
        }

        Ok(())
    }

    /// List objects under the key prefix, keyed by path relative to it.
    async fn list_objects(&self, ctx: &SyncContext, target: &Target<'_>) -> Result<HashMap<String, RemoteFile>, ProviderError> {
        let list_prefix = format!("{}/", target.key_prefix);
        let mut out = HashMap::new();
        let mut pages = target
            .client
            .list_objects_v2()
            .bucket(target.bucket)
            .prefix(&list_prefix)
            .into_paginator()
            .send();

        while let Some(page) = pages.next().await {
            if ctx.check_cancelled() {
                return Err(ProviderError::Cancelled);
            }
            let page = page.map_err(|e| {
                let detail = sdk_error_details(&e);
                ProviderError::ConnectionError(format!("Failed to list objects: {}", detail))
            })?;
            for obj in page.contents() {
                let Some(key) = obj.key() else { continue };
                let Some(rel) = key.strip_prefix(&list_prefix) else { continue };
                if rel.is_empty() || rel.ends_with('/') {
                    continue; // folder marker objects
                }
                out.insert(
                    rel.to_string(),
                    RemoteFile {
                        size: obj.size().unwrap_or(0).max(0) as u64,
                        mtime: obj.last_modified().map(|t| t.secs()).unwrap_or(0),
                        id: None,
                    },
                );
            }
        }
        Ok(out)
    }

    async fn put_object(&self, target: &Target<'_>, key: &str, file: &LocalEntry) -> Result<(), ProviderError> {
        // Streams the file from disk instead of loading it into memory
        let body = ByteStream::from_path(&file.abs)
            .await
            .map_err(|e| ProviderError::TransferError(format!("Cannot read {}: {}", file.abs.display(), e)))?;
        let content_type = mime_guess::from_path(&file.abs).first_or_octet_stream().to_string();

        let mut req = target
            .client
            .put_object()
            .bucket(target.bucket)
            .key(key)
            .content_type(content_type)
            .body(body);
        if let Some(sc) = target.storage_class {
            req = req.storage_class(sc.clone());
        }
        req.send().await.map(|_| ()).map_err(|e| {
            let detail = sdk_error_details(&e);
            ProviderError::TransferError(detail)
        })
    }

    async fn multipart_upload(&self, ctx: &SyncContext, target: &Target<'_>, key: &str, file: &LocalEntry) -> Result<(), ProviderError> {
        let content_type = mime_guess::from_path(&file.abs).first_or_octet_stream().to_string();
        let mut create = target
            .client
            .create_multipart_upload()
            .bucket(target.bucket)
            .key(key)
            .content_type(content_type);
        if let Some(sc) = target.storage_class {
            create = create.storage_class(sc.clone());
        }
        let created = create.send().await.map_err(|e| {
            let detail = sdk_error_details(&e);
            ProviderError::TransferError(format!("Failed to start multipart upload: {}", detail))
        })?;
        let upload_id = created
            .upload_id()
            .ok_or_else(|| ProviderError::TransferError("No multipart upload ID returned".to_string()))?
            .to_string();

        let result = self.upload_parts(ctx, target, key, &upload_id, file).await;
        match result {
            Ok(parts) => target
                .client
                .complete_multipart_upload()
                .bucket(target.bucket)
                .key(key)
                .upload_id(&upload_id)
                .multipart_upload(CompletedMultipartUpload::builder().set_parts(Some(parts)).build())
                .send()
                .await
                .map(|_| ())
                .map_err(|e| {
                    let detail = sdk_error_details(&e);
                    ProviderError::TransferError(format!("Failed to complete multipart upload: {}", detail))
                }),
            Err(e) => {
                // Abort, so the uploaded parts are not stored (and billed) forever
                let _ = target
                    .client
                    .abort_multipart_upload()
                    .bucket(target.bucket)
                    .key(key)
                    .upload_id(&upload_id)
                    .send()
                    .await;
                Err(e)
            }
        }
    }

    async fn upload_parts(
        &self,
        ctx: &SyncContext,
        target: &Target<'_>,
        key: &str,
        upload_id: &str,
        file: &LocalEntry,
    ) -> Result<Vec<CompletedPart>, ProviderError> {
        let part_size = part_size_for(file.size);
        let mut parts = Vec::new();
        let mut offset = 0u64;
        let mut number = 1i32;

        while offset < file.size {
            if ctx.check_cancelled() {
                return Err(ProviderError::Cancelled);
            }
            let len = part_size.min(file.size - offset);
            let body = ByteStream::read_from()
                .path(&file.abs)
                .offset(offset)
                .length(aws_sdk_s3::primitives::Length::Exact(len))
                .build()
                .await
                .map_err(|e| ProviderError::TransferError(format!("Cannot read {}: {}", file.abs.display(), e)))?;

            let uploaded = target
                .client
                .upload_part()
                .bucket(target.bucket)
                .key(key)
                .upload_id(upload_id)
                .part_number(number)
                .content_length(len as i64)
                .body(body)
                .send()
                .await
                .map_err(|e| {
                    let detail = sdk_error_details(&e);
                    ProviderError::TransferError(format!("Part {} failed: {}", number, detail))
                })?;

            parts.push(
                CompletedPart::builder()
                    .part_number(number)
                    .set_e_tag(uploaded.e_tag().map(String::from))
                    .build(),
            );
            offset += len;
            number += 1;
        }
        Ok(parts)
    }

    /// Delete keys in batches; falls back to single deletes if the service
    /// does not support DeleteObjects. Returns (deleted, errors).
    async fn delete_keys(&self, ctx: &SyncContext, target: &Target<'_>, keys: &[String]) -> (i64, i32) {
        let mut deleted = 0i64;
        let mut errors = 0i32;

        for batch in keys.chunks(DELETE_BATCH) {
            let objects: Vec<ObjectIdentifier> = batch
                .iter()
                .filter_map(|k| ObjectIdentifier::builder().key(k).build().ok())
                .collect();
            let delete = Delete::builder().set_objects(Some(objects)).quiet(true).build();

            let batch_result = match delete {
                Ok(d) => target.client.delete_objects().bucket(target.bucket).delete(d).send().await.ok(),
                Err(_) => None,
            };

            match batch_result {
                Some(out) => {
                    let failed: Vec<String> = out.errors().iter().filter_map(|e| e.key().map(String::from)).collect();
                    for key in batch {
                        if failed.contains(key) {
                            ctx.log_error(&format!("Failed to delete {}", key), "s3").await;
                            errors += 1;
                        } else {
                            ctx.log_info(&format!("Deleted: {}", key), "s3").await;
                            deleted += 1;
                        }
                    }
                }
                None => {
                    for key in batch {
                        match target.client.delete_object().bucket(target.bucket).key(key).send().await {
                            Ok(_) => {
                                ctx.log_info(&format!("Deleted: {}", key), "s3").await;
                                deleted += 1;
                            }
                            Err(e) => {
                                let detail = sdk_error_details(&e);
                                ctx.log_error(&format!("Failed to delete {}: {}", key, detail), "s3").await;
                                errors += 1;
                            }
                        }
                    }
                }
            }
        }
        (deleted, errors)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn part_size_respects_limits() {
        assert_eq!(part_size_for(100 * 1024 * 1024), MIN_PART_SIZE);
        // 1 TB needs bigger parts to stay within 10,000
        let tb = 1024u64 * 1024 * 1024 * 1024;
        let ps = part_size_for(tb);
        assert!(tb.div_ceil(ps) <= MAX_PARTS);
        assert_eq!(ps % (1024 * 1024), 0);
    }

    #[test]
    fn key_prefix_joins_cleanly() {
        assert_eq!(key_prefix_for("", "photos"), "photos");
        assert_eq!(key_prefix_for("/backups/", "photos"), "backups/photos");
    }
}
