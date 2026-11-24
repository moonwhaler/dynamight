//! Previews for the file manager: the start of text files and cached
//! thumbnails of images and videos.

use axum::{
    body::Body,
    extract::{Query, State},
    http::{header, HeaderValue},
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;
use serde_json::json;
use std::hash::{Hash, Hasher};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, LazyLock, OnceLock};
use std::time::Duration;
use tokio::sync::Semaphore;

use super::files::{blocking, denied};
use crate::errors::ApiError;
use crate::services::path_guard::resolve_existing;
use crate::AppState;

#[derive(Deserialize)]
pub struct PathQuery {
    pub path: String,
}

/// Text shown in the preview
const TEXT_LIMIT: u64 = 1024 * 1024;

/// GET /files/text?path= - the first MiB of a text file
pub async fn text(State(state): State<Arc<AppState>>, Query(q): Query<PathQuery>) -> Response {
    let canonical = match resolve_existing(&q.path, &state.config.allowed_browse_paths) {
        Ok(p) => p,
        Err(e) => return denied(e),
    };
    let result = blocking(move || -> std::io::Result<serde_json::Value> {
        let file = std::fs::File::open(&canonical)?;
        let size = file.metadata()?.len();
        if !file.metadata()?.is_file() {
            return Err(std::io::Error::from(std::io::ErrorKind::InvalidInput));
        }
        let mut buf = Vec::new();
        file.take(TEXT_LIMIT).read_to_end(&mut buf)?;
        let binary = buf.iter().take(8192).any(|b| *b == 0);
        let text = if binary { String::new() } else { String::from_utf8_lossy(&buf).into_owned() };
        Ok(json!({ "text": text, "binary": binary, "truncated": size > TEXT_LIMIT, "size": size }))
    })
    .await;
    match result {
        Some(Ok(body)) => Json(body).into_response(),
        Some(Err(e)) if e.kind() == std::io::ErrorKind::InvalidInput => ApiError::not_a_file().into_response(),
        _ => ApiError::file_not_found().into_response(),
    }
}

// ── Thumbnails ──────────────────────────────────────────────────────────

const THUMB_SIZE: u32 = 256;
/// Larger images are not decoded for a thumbnail
const MAX_IMAGE_BYTES: u64 = 64 * 1024 * 1024;
const VIDEO_TIMEOUT: Duration = Duration::from_secs(10);
/// Thumbnails made at once (decoding is CPU and memory heavy)
static MAKING: LazyLock<Arc<Semaphore>> = LazyLock::new(|| Arc::new(Semaphore::new(2)));

const IMAGE_EXTS: &[&str] = &["jpg", "jpeg", "png", "webp", "gif"];
const VIDEO_EXTS: &[&str] = &["mp4", "m4v", "mov", "mkv", "webm", "avi"];

fn ffmpeg_available() -> bool {
    static FFMPEG: OnceLock<bool> = OnceLock::new();
    *FFMPEG.get_or_init(|| {
        std::process::Command::new("ffmpeg")
            .arg("-version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|s| s.success())
    })
}

/// Cache file name: changes when the file changes
fn cache_name(path: &Path, size: u64, modified: Option<std::time::SystemTime>) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    path.hash(&mut hasher);
    size.hash(&mut hasher);
    modified.hash(&mut hasher);
    format!("{:016x}.jpg", hasher.finish())
}

/// Decode, turn upright (EXIF) and shrink an image to a JPEG
fn image_thumbnail(path: &Path) -> Option<Vec<u8>> {
    use image::ImageDecoder;
    let mut decoder = image::ImageReader::open(path).ok()?.with_guessed_format().ok()?.into_decoder().ok()?;
    let orientation = decoder.orientation().ok();
    let mut img = image::DynamicImage::from_decoder(decoder).ok()?;
    if let Some(o) = orientation {
        img.apply_orientation(o);
    }
    let thumb = img.thumbnail(THUMB_SIZE, THUMB_SIZE).to_rgb8();
    let mut out = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 80).encode_image(&thumb).ok()?;
    Some(out)
}

/// One frame of a video, via ffmpeg
async fn video_thumbnail(path: &Path) -> Option<Vec<u8>> {
    let child = tokio::process::Command::new("ffmpeg")
        .args(["-hide_banner", "-loglevel", "error", "-ss", "2", "-i"])
        .arg(path)
        .args(["-frames:v", "1", "-vf", &format!("scale={THUMB_SIZE}:-2"), "-f", "mjpeg", "pipe:1"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .ok()?;
    let output = tokio::time::timeout(VIDEO_TIMEOUT, child.wait_with_output()).await.ok()?.ok()?;
    (output.status.success() && !output.stdout.is_empty()).then_some(output.stdout)
}

fn jpeg(bytes: Vec<u8>) -> Response {
    let mut response = Response::new(Body::from(bytes));
    let h = response.headers_mut();
    h.insert(header::CONTENT_TYPE, HeaderValue::from_static("image/jpeg"));
    h.insert(header::CACHE_CONTROL, HeaderValue::from_static("private, max-age=86400"));
    response
}

/// GET /files/thumb?path= - a small JPEG of an image or video (404 when
/// there is none: unsupported type, too large, or no ffmpeg for videos)
pub async fn thumb(State(state): State<Arc<AppState>>, Query(q): Query<PathQuery>) -> Response {
    let canonical = match resolve_existing(&q.path, &state.config.allowed_browse_paths) {
        Ok(p) => p,
        Err(e) => return denied(e),
    };
    let Ok(meta) = std::fs::metadata(&canonical) else {
        return ApiError::file_not_found().into_response();
    };
    let ext = canonical.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    let is_image = IMAGE_EXTS.contains(&ext.as_str()) && meta.len() <= MAX_IMAGE_BYTES;
    let is_video = VIDEO_EXTS.contains(&ext.as_str()) && ffmpeg_available();
    if !meta.is_file() || !(is_image || is_video) {
        return ApiError::file_not_found().into_response();
    }

    let cached: PathBuf = state.thumbs_dir.join(cache_name(&canonical, meta.len(), meta.modified().ok()));
    if let Ok(bytes) = tokio::fs::read(&cached).await {
        // Recently used thumbnails survive the cache trimming
        let touch = cached.clone();
        let _ = blocking(move || std::fs::File::options().write(true).open(&touch).and_then(|f| f.set_modified(std::time::SystemTime::now()))).await;
        return jpeg(bytes);
    }

    let Ok(_permit) = MAKING.clone().acquire_owned().await else {
        return ApiError::file_not_found().into_response();
    };
    let made = if is_image {
        let source = canonical.clone();
        blocking(move || image_thumbnail(&source)).await.flatten()
    } else {
        video_thumbnail(&canonical).await
    };
    let Some(bytes) = made else {
        return ApiError::file_not_found().into_response();
    };
    let dir = state.thumbs_dir.clone();
    let data = bytes.clone();
    let _ = blocking(move || -> std::io::Result<()> {
        std::fs::create_dir_all(&dir)?;
        let tmp = cached.with_extension("tmp");
        std::fs::write(&tmp, &data)?;
        std::fs::rename(&tmp, &cached)
    })
    .await;
    jpeg(bytes)
}

/// Keep the thumbnail cache under `max_bytes`, dropping the least recently
/// used first. Returns how many were removed.
pub fn trim_thumbs(dir: &Path, max_bytes: u64) -> usize {
    let Ok(entries) = std::fs::read_dir(dir) else { return 0 };
    let mut files: Vec<(std::time::SystemTime, u64, PathBuf)> = entries
        .flatten()
        .filter_map(|e| {
            let meta = e.metadata().ok()?;
            meta.is_file().then(|| (meta.modified().unwrap_or(std::time::UNIX_EPOCH), meta.len(), e.path()))
        })
        .collect();
    let mut total: u64 = files.iter().map(|f| f.1).sum();
    files.sort_by_key(|f| f.0);
    let mut removed = 0;
    for (_, size, path) in files {
        if total <= max_bytes {
            break;
        }
        if std::fs::remove_file(&path).is_ok() {
            total -= size;
            removed += 1;
        }
    }
    removed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_thumbnails_are_small_jpegs() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("big.png");
        image::RgbImage::from_pixel(1200, 600, image::Rgb([200, 30, 30])).save(&src).unwrap();
        let bytes = image_thumbnail(&src).unwrap();
        let thumb = image::load_from_memory(&bytes).unwrap();
        assert_eq!((thumb.width(), thumb.height()), (256, 128));
    }

    #[test]
    fn cache_name_changes_with_the_file() {
        let p = Path::new("/a/b.jpg");
        assert_ne!(cache_name(p, 1, None), cache_name(p, 2, None));
        assert_eq!(cache_name(p, 1, None), cache_name(p, 1, None));
    }

    #[test]
    fn trimming_drops_the_oldest() {
        let dir = tempfile::tempdir().unwrap();
        for (i, name) in ["old.jpg", "mid.jpg", "new.jpg"].iter().enumerate() {
            let p = dir.path().join(name);
            std::fs::write(&p, vec![0u8; 100]).unwrap();
            let t = std::time::UNIX_EPOCH + Duration::from_secs(1_000_000 + i as u64 * 100);
            std::fs::File::options().write(true).open(&p).unwrap().set_modified(t).unwrap();
        }
        assert_eq!(trim_thumbs(dir.path(), 200), 1);
        assert!(!dir.path().join("old.jpg").exists());
        assert!(dir.path().join("new.jpg").exists());
    }
}
