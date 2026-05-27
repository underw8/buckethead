use aws_sdk_s3::presigning::PresigningConfig;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::time::Duration;
use tauri::{Emitter, State};
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;

use crate::s3_client::AppState;
use super::{
    ListObjectsResult, ObjectInfo, ObjectMeta,
    aws_dt_rfc3339, fmt_sdk_err, bucket_region, get_cached_client, get_object_client,
};

// ── list_objects ──────────────────────────────────────────────────────────────

#[specta::specta]
#[tauri::command]
pub async fn list_objects(
    bucket: String,
    prefix: String,
    continuation_token: Option<String>,
    state: State<'_, AppState>,
) -> Result<ListObjectsResult, String> {
    let fallback = {
        let s = state.0.read().await;
        s.sdk_config
            .as_ref()
            .and_then(|c| c.region().map(|r| r.to_string()))
            .unwrap_or_else(|| "us-east-1".to_string())
    };

    let region = bucket_region(&state, &bucket, &fallback).await;

    let client = get_cached_client(&state, &region).await?;

    let mut req = client
        .list_objects_v2()
        .bucket(&bucket)
        .prefix(&prefix)
        .delimiter("/")
        .max_keys(200);

    if let Some(token) = continuation_token {
        req = req.continuation_token(token);
    }

    let resp = req.send().await.map_err(|e| fmt_sdk_err("list_objects", &e))?;

    let folders: Vec<String> = resp
        .common_prefixes()
        .iter()
        .filter_map(|p| p.prefix().map(|s| s.to_string()))
        .collect();

    let objects: Vec<ObjectInfo> = resp
        .contents()
        .iter()
        .map(|o| ObjectInfo {
            key: o.key().unwrap_or_default().to_string(),
            size: o.size().unwrap_or(0),
            modified: o.last_modified().map(|d| aws_dt_rfc3339(d)),
            etag: o.e_tag().map(|s| s.trim_matches('"').to_string()),
            storage_class: o.storage_class().map(|s| s.as_str().to_string()),
        })
        .collect();

    Ok(ListObjectsResult {
        folders,
        objects,
        next_token: resp.next_continuation_token().map(|s| s.to_string()),
        truncated: resp.is_truncated().unwrap_or(false),
    })
}

// ── presign_url ───────────────────────────────────────────────────────────────

#[specta::specta]
#[tauri::command]
pub async fn presign_url(
    bucket: String,
    key: String,
    expires_in: Option<u32>,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let fallback = {
        let s = state.0.read().await;
        s.sdk_config
            .as_ref()
            .and_then(|c| c.region().map(|r| r.to_string()))
            .unwrap_or_else(|| "us-east-1".to_string())
    };

    let region = bucket_region(&state, &bucket, &fallback).await;

    let client = get_cached_client(&state, &region).await?;

    let secs = expires_in.unwrap_or(600).min(604800) as u64;
    let presigning_config = PresigningConfig::expires_in(Duration::from_secs(secs))
        .map_err(|e| e.to_string())?;

    let url = client
        .get_object()
        .bucket(&bucket)
        .key(&key)
        .presigned(presigning_config)
        .await
        .map_err(|e| fmt_sdk_err("presign_url", &e))?;

    Ok(url.uri().to_string())
}

// ── get_object_text ───────────────────────────────────────────────────────────
// Downloads object content via GetObject (no CORS) and returns as UTF-8 string.
// Capped at 2 MB — larger files should be downloaded instead.

#[specta::specta]
#[tauri::command]
pub async fn get_object_text(
    bucket: String,
    key: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let client = get_object_client(&bucket, &state).await?;

    let resp = client
        .get_object()
        .bucket(&bucket)
        .key(&key)
        .range("bytes=0-2097151")
        .send()
        .await
        .map_err(|e| fmt_sdk_err("get_object_text", &e))?;

    let bytes = resp
        .body
        .collect()
        .await
        .map_err(|e| e.to_string())?
        .into_bytes();

    String::from_utf8(bytes.to_vec())
        .map_err(|_| "File is not valid UTF-8".to_string())
}

// ── save_object ───────────────────────────────────────────────────────────────
// Shows native Save dialog, downloads the object, writes to chosen path.
// Returns false if the user cancelled the dialog.

#[specta::specta]
#[tauri::command]
pub async fn save_object(
    bucket: String,
    key: String,
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
) -> Result<bool, String> {
    let filename = key.split('/').last().unwrap_or("download").to_string();

    let handle = rfd::AsyncFileDialog::new()
        .set_file_name(&filename)
        .save_file()
        .await;

    let Some(dest) = handle else { return Ok(false) };

    let client = get_object_client(&bucket, &state).await?;
    let resp = client
        .get_object()
        .bucket(&bucket)
        .key(&key)
        .send()
        .await
        .map_err(|e| fmt_sdk_err("save_object", &e))?;

    let mut body = resp.body.into_async_read();
    let total = resp.content_length.unwrap_or(0) as u64;
    let mut file = tokio::fs::File::create(dest.path()).await.map_err(|e| e.to_string())?;
    let mut received: u64 = 0;
    let mut buf = vec![0u8; 65536];
    loop {
        let n = body.read(&mut buf).await.map_err(|e| e.to_string())?;
        if n == 0 { break; }
        file.write_all(&buf[..n]).await.map_err(|e| e.to_string())?;
        received += n as u64;
        let _ = app_handle.emit("download:progress", serde_json::json!({
            "bytes_received": received,
            "total_bytes": total,
            "key": key,
        }));
    }
    Ok(true)
}

// ── open_object ───────────────────────────────────────────────────────────────
// Downloads to OS temp dir then opens with the system default app via `open`.

#[specta::specta]
#[tauri::command]
pub async fn open_object(
    bucket: String,
    key: String,
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    let filename = key.split('/').last().unwrap_or("file").to_string();
    let tmp_dir = std::env::temp_dir().join("aws-thathoo");
    std::fs::create_dir_all(&tmp_dir).map_err(|e| e.to_string())?;

    // Clean up temp files older than 1 day
    if let Ok(entries) = std::fs::read_dir(&tmp_dir) {
        let cutoff = std::time::SystemTime::now()
            - std::time::Duration::from_secs(86400);
        for entry in entries.flatten() {
            if let Ok(meta) = entry.metadata() {
                if meta.modified().map(|m| m < cutoff).unwrap_or(false) {
                    let _ = std::fs::remove_file(entry.path());
                }
            }
        }
    }

    // Hash bucket+key to avoid filename collisions across different prefixes
    let mut hasher = DefaultHasher::new();
    format!("{}/{}", bucket, key).hash(&mut hasher);
    let hash = hasher.finish();
    let unique_name = format!("{:016x}_{}", hash, filename);
    let dest = tmp_dir.join(&unique_name);

    // Remove stale copy before writing fresh data
    let _ = std::fs::remove_file(&dest);

    // Stream the download
    let client = get_object_client(&bucket, &state).await?;
    let resp = client
        .get_object()
        .bucket(&bucket)
        .key(&key)
        .send()
        .await
        .map_err(|e| fmt_sdk_err("open_object", &e))?;

    let mut body = resp.body.into_async_read();
    let total = resp.content_length.unwrap_or(0) as u64;
    let mut file = tokio::fs::File::create(&dest).await.map_err(|e| e.to_string())?;
    let mut received: u64 = 0;
    let mut buf = vec![0u8; 65536];
    loop {
        let n = body.read(&mut buf).await.map_err(|e| e.to_string())?;
        if n == 0 { break; }
        file.write_all(&buf[..n]).await.map_err(|e| e.to_string())?;
        received += n as u64;
        let _ = app_handle.emit("download:progress", serde_json::json!({
            "bytes_received": received,
            "total_bytes": total,
            "key": key,
        }));
    }
    drop(file);

    std::process::Command::new("open")
        .arg(&dest)
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}

// ── head_object ───────────────────────────────────────────────────────────────
// Returns HeadObject metadata for a single S3 object.

#[specta::specta]
#[tauri::command]
pub async fn head_object(
    bucket: String,
    key: String,
    state: State<'_, AppState>,
) -> Result<ObjectMeta, String> {
    let client = get_object_client(&bucket, &state).await?;
    let resp = client
        .head_object()
        .bucket(&bucket)
        .key(&key)
        .send()
        .await
        .map_err(|e| fmt_sdk_err("head_object", &e))?;

    let user_meta = resp
        .metadata()
        .map(|m| m.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
        .unwrap_or_default();

    Ok(ObjectMeta {
        content_type: resp.content_type().map(str::to_string),
        content_length: resp.content_length(),
        last_modified: resp.last_modified().map(|d| aws_dt_rfc3339(d)),
        etag: resp.e_tag().map(|s| s.trim_matches('"').to_string()),
        storage_class: resp.storage_class().map(|s| s.as_str().to_string()),
        cache_control: resp.cache_control().map(str::to_string),
        content_encoding: resp.content_encoding().map(str::to_string),
        user_meta,
    })
}
