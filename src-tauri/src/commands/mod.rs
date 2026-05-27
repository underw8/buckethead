use aws_sdk_s3::error::ProvideErrorMetadata;
use aws_sdk_s3::Client;
use std::sync::Arc;
use tauri::State;
use tracing::warn;

use crate::s3_client::AppState;

pub mod auth;
pub mod buckets;
pub mod objects;

pub use auth::*;
pub use buckets::*;
pub use objects::*;

// ── Shared types ─────────────────────────────────────────────────────────────

#[derive(serde::Serialize, specta::Type)]
pub struct BucketInfo {
    pub name: String,
    pub created: Option<String>,
}

#[derive(serde::Serialize, specta::Type)]
pub struct ObjectInfo {
    pub key: String,
    #[specta(type = i32)]
    pub size: i64,
    pub modified: Option<String>,
    pub etag: Option<String>,
    pub storage_class: Option<String>,
}

#[derive(serde::Serialize, specta::Type)]
pub struct ObjectMeta {
    pub content_type: Option<String>,
    #[specta(type = Option<i32>)]
    pub content_length: Option<i64>,
    pub last_modified: Option<String>,
    pub etag: Option<String>,
    pub storage_class: Option<String>,
    pub cache_control: Option<String>,
    pub content_encoding: Option<String>,
    pub user_meta: Vec<(String, String)>,
}

#[derive(serde::Serialize, specta::Type)]
pub struct ListObjectsResult {
    pub folders: Vec<String>,
    pub objects: Vec<ObjectInfo>,
    pub next_token: Option<String>,
    pub truncated: bool,
}

// ── Helpers ───────────────────────────────────────────────────────────────────

pub fn aws_dt_rfc3339(d: &aws_sdk_s3::primitives::DateTime) -> String {
    chrono::DateTime::from_timestamp(d.secs(), d.subsec_nanos())
        .unwrap_or_default()
        .to_rfc3339()
}

pub fn make_client(sdk_config: &aws_config::SdkConfig, region: &str) -> Client {
    let s3_conf = aws_sdk_s3::config::Builder::from(sdk_config)
        .region(aws_sdk_s3::config::Region::new(region.to_string()))
        .build();
    Client::from_conf(s3_conf)
}

// Extract the innermost human-readable message from an AWS SDK debug string.
pub fn extract_debug_msg(debug: &str) -> String {
    let mut best = String::new();
    for needle in &[r#"message: ""#, r#"message: Some(""#] {
        let mut haystack = debug;
        while let Some(pos) = haystack.find(needle) {
            let after = &haystack[pos + needle.len()..];
            if let Some(end) = after.find('"') {
                let msg = &after[..end];
                if msg.len() > 4 {
                    best = msg.to_string();
                }
            }
            haystack = &haystack[pos + needle.len()..];
        }
    }
    if !best.is_empty() {
        return best;
    }
    debug.to_string()
}

// Format an AWS SDK error.
pub fn fmt_sdk_err(context: &str, e: &(impl std::fmt::Debug + ProvideErrorMetadata)) -> String {
    let code = e.code();
    let message = e.message();
    let summary = match (code, message) {
        (Some(c), Some(m)) => format!("{c}: {m}"),
        (Some(c), None)    => c.to_string(),
        (None, Some(m))    => m.to_string(),
        (None, None)       => extract_debug_msg(&format!("{e:?}")),
    };
    warn!("[buckethead] {context} — {summary}\n  debug: {e:?}");
    summary
}

// Detect SSO/session token expiry from error strings.
pub fn is_sso_expired(lower: &str) -> bool {
    (lower.contains("token") && lower.contains("expir"))
    || lower.contains("sso") && lower.contains("expir")
    || lower.contains("unauthorizedexception")
    || lower.contains("session token not found")
    || lower.contains("token not found or invalid")
    || lower.contains("expiredtoken")
    || lower.contains("expired token")
}

// Returns a cached S3 client for the given region, creating one if needed.
pub async fn get_cached_client(
    state: &State<'_, AppState>,
    region: &str,
) -> Result<Arc<Client>, String> {
    // Fast path: check cache with read lock
    {
        let s = state.0.read().await;
        if let Some(c) = s.clients.get(region) {
            return Ok(Arc::clone(c));
        }
    }
    // Slow path: build and insert with write lock
    let mut s = state.0.write().await;
    // Re-check after acquiring write lock
    if let Some(c) = s.clients.get(region) {
        return Ok(Arc::clone(c));
    }
    let cfg = s.sdk_config.as_ref().ok_or("Not connected")?;
    let client = Arc::new(make_client(cfg, region));
    s.clients.insert(region.to_string(), Arc::clone(&client));
    Ok(client)
}

// Returns the bucket's actual AWS region (cached on AppState).
pub async fn bucket_region(
    state: &State<'_, AppState>,
    bucket: &str,
    fallback_region: &str,
) -> String {
    // Check cache first
    {
        let s = state.0.read().await;
        if let Some(r) = s.bucket_regions.get(bucket) {
            return r.clone();
        }
    }

    // Use a client with the fallback region to call GetBucketLocation
    let region = {
        let s = state.0.read().await;
        let Some(cfg) = s.sdk_config.as_ref() else {
            return fallback_region.to_string();
        };
        let client = make_client(cfg, fallback_region);
        drop(s);
        match client.get_bucket_location().bucket(bucket).send().await {
            Ok(resp) => {
                let loc = resp
                    .location_constraint()
                    .map(|c| c.as_str())
                    .unwrap_or("");
                if loc.is_empty() {
                    "us-east-1".to_string()
                } else {
                    loc.to_string()
                }
            }
            Err(_) => fallback_region.to_string(),
        }
    };

    // Cache it
    {
        let mut s = state.0.write().await;
        s.bucket_regions.insert(bucket.to_string(), region.clone());
    }

    region
}

// Shared helper: resolve region and return a cached client for a given bucket.
pub async fn get_object_client(
    bucket: &str,
    state: &State<'_, AppState>,
) -> Result<Arc<Client>, String> {
    let fallback = {
        let s = state.0.read().await;
        s.sdk_config
            .as_ref()
            .and_then(|c| c.region().map(|r| r.to_string()))
            .unwrap_or_else(|| "us-east-1".to_string())
    };
    let region = bucket_region(state, bucket, &fallback).await;
    get_cached_client(state, &region).await
}
