use aws_config::BehaviorVersion;
use aws_config::profile::ProfileFileCredentialsProvider;
use aws_sdk_s3::config::Region;
use aws_sdk_s3::presigning::PresigningConfig;
use aws_sdk_s3::Client;
use serde::Serialize;
use std::time::Duration;
use tauri::State;

use crate::s3_client::AppState;

// ── Shared types ─────────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct BucketInfo {
    pub name: String,
    pub created: Option<String>,
}

#[derive(Serialize)]
pub struct ObjectInfo {
    pub key: String,
    pub size: i64,
    pub modified: Option<String>,
    pub etag: Option<String>,
}

#[derive(Serialize)]
pub struct ListObjectsResult {
    pub folders: Vec<String>,
    pub objects: Vec<ObjectInfo>,
    pub next_token: Option<String>,
    pub truncated: bool,
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn aws_dt_rfc3339(d: &aws_sdk_s3::primitives::DateTime) -> String {
    chrono::DateTime::from_timestamp(d.secs(), d.subsec_nanos())
        .unwrap_or_default()
        .to_rfc3339()
}

fn make_client(sdk_config: &aws_config::SdkConfig, region: &str) -> Client {
    let s3_conf = aws_sdk_s3::config::Builder::from(sdk_config)
        .region(Region::new(region.to_string()))
        .build();
    Client::from_conf(s3_conf)
}

// Returns the bucket's actual AWS region (cached on AppState).
async fn bucket_region(
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
        drop(s); // release read lock before await
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

// ── list_profiles ─────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn list_profiles() -> Result<Vec<String>, String> {
    let home = std::env::var("HOME").unwrap_or_default();
    let mut profiles: std::collections::BTreeSet<String> = Default::default();

    for path in &[
        format!("{}/.aws/credentials", home),
        format!("{}/.aws/config", home),
    ] {
        if let Ok(content) = std::fs::read_to_string(path) {
            for line in content.lines() {
                let line = line.trim();
                if line.starts_with('[') && line.ends_with(']') {
                    let name = line[1..line.len() - 1].trim();
                    let name = name.strip_prefix("profile ").unwrap_or(name).trim();
                    profiles.insert(name.to_string());
                }
            }
        }
    }

    Ok(profiles.into_iter().collect())
}

// ── set_profile ───────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn set_profile(
    profile: String,
    state: State<'_, AppState>,
) -> Result<Vec<BucketInfo>, String> {
    let credentials_provider = ProfileFileCredentialsProvider::builder()
        .profile_name(&profile)
        .build();

    // us-east-1 for initial credential load; bucket ops auto-detect their region
    let sdk_config = aws_config::defaults(BehaviorVersion::latest())
        .region(Region::new("us-east-1"))
        .credentials_provider(credentials_provider)
        .load()
        .await;

    let client = make_client(&sdk_config, "us-east-1");

    // list_buckets requires s3:ListAllMyBuckets. If denied, fall back to
    // IAM policy introspection to discover buckets from s3:ListBucket ARNs.
    let buckets = match client.list_buckets().send().await {
        Ok(resp) => resp
            .buckets()
            .iter()
            .map(|b| BucketInfo {
                name: b.name().unwrap_or_default().to_string(),
                created: b.creation_date().map(|d| aws_dt_rfc3339(d)),
            })
            .collect(),
        Err(_) => vec![],
    };

    {
        let mut s = state.0.write().await;
        s.sdk_config = Some(sdk_config);
        s.bucket_regions.clear();
    }

    Ok(buckets)
}

// ── list_buckets ──────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn list_buckets(state: State<'_, AppState>) -> Result<Vec<BucketInfo>, String> {
    let s = state.0.read().await;
    let cfg = s.sdk_config.as_ref().ok_or("Not connected")?;

    // list_buckets is a global S3 operation; any region works
    let default_region = "us-east-1";
    let client = make_client(cfg, default_region);
    drop(s);

    let resp = client.list_buckets().send().await.map_err(|e| e.to_string())?;

    Ok(resp
        .buckets()
        .iter()
        .map(|b| BucketInfo {
            name: b.name().unwrap_or_default().to_string(),
            created: b.creation_date().map(|d| aws_dt_rfc3339(d)),
        })
        .collect())
}

// ── list_objects ──────────────────────────────────────────────────────────────

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

    let client = {
        let s = state.0.read().await;
        let cfg = s.sdk_config.as_ref().ok_or("Not connected")?;
        make_client(cfg, &region)
    };

    let mut req = client
        .list_objects_v2()
        .bucket(&bucket)
        .prefix(&prefix)
        .delimiter("/")
        .max_keys(200);

    if let Some(token) = continuation_token {
        req = req.continuation_token(token);
    }

    let resp = req.send().await.map_err(|e| e.to_string())?;

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

#[tauri::command]
pub async fn presign_url(
    bucket: String,
    key: String,
    expires_in: Option<u64>,
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

    let client = {
        let s = state.0.read().await;
        let cfg = s.sdk_config.as_ref().ok_or("Not connected")?;
        make_client(cfg, &region)
    };

    let secs = expires_in.unwrap_or(600);
    let presigning_config = PresigningConfig::expires_in(Duration::from_secs(secs))
        .map_err(|e| e.to_string())?;

    let url = client
        .get_object()
        .bucket(&bucket)
        .key(&key)
        .presigned(presigning_config)
        .await
        .map_err(|e| e.to_string())?;

    Ok(url.uri().to_string())
}

// ── get_object_text ───────────────────────────────────────────────────────────
// Downloads object content via GetObject (no CORS) and returns as UTF-8 string.
// Capped at 2 MB — larger files should be downloaded instead.

#[tauri::command]
pub async fn get_object_text(
    bucket: String,
    key: String,
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

    let client = {
        let s = state.0.read().await;
        let cfg = s.sdk_config.as_ref().ok_or("Not connected")?;
        make_client(cfg, &region)
    };

    let resp = client
        .get_object()
        .bucket(&bucket)
        .key(&key)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let bytes = resp
        .body
        .collect()
        .await
        .map_err(|e| e.to_string())?
        .into_bytes();

    if bytes.len() > 2 * 1024 * 1024 {
        return Err("File too large to preview (> 2 MB)".to_string());
    }

    String::from_utf8(bytes.to_vec())
        .map_err(|_| "File is not valid UTF-8".to_string())
}

// ── shared bytes helper ───────────────────────────────────────────────────────

async fn get_object_bytes(
    bucket: &str,
    key: &str,
    state: &State<'_, AppState>,
) -> Result<Vec<u8>, String> {
    let fallback = {
        let s = state.0.read().await;
        s.sdk_config
            .as_ref()
            .and_then(|c| c.region().map(|r| r.to_string()))
            .unwrap_or_else(|| "us-east-1".to_string())
    };
    let region = bucket_region(state, bucket, &fallback).await;
    let client = {
        let s = state.0.read().await;
        let cfg = s.sdk_config.as_ref().ok_or("Not connected")?;
        make_client(cfg, &region)
    };
    let resp = client
        .get_object()
        .bucket(bucket)
        .key(key)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    Ok(resp.body.collect().await.map_err(|e| e.to_string())?.into_bytes().to_vec())
}

// ── save_object ───────────────────────────────────────────────────────────────
// Shows native Save dialog, downloads the object, writes to chosen path.
// Returns false if the user cancelled the dialog.

#[tauri::command]
pub async fn save_object(
    bucket: String,
    key: String,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    let filename = key.split('/').last().unwrap_or("download").to_string();

    let handle = rfd::AsyncFileDialog::new()
        .set_file_name(&filename)
        .save_file()
        .await;

    let Some(dest) = handle else { return Ok(false) };

    let bytes = get_object_bytes(&bucket, &key, &state).await?;
    std::fs::write(dest.path(), bytes).map_err(|e| e.to_string())?;
    Ok(true)
}

// ── open_object ───────────────────────────────────────────────────────────────
// Downloads to OS temp dir then opens with the system default app via `open`.

#[tauri::command]
pub async fn open_object(
    bucket: String,
    key: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let filename = key.split('/').last().unwrap_or("file").to_string();
    let temp_dir = std::env::temp_dir().join("aws-thathoo");
    std::fs::create_dir_all(&temp_dir).map_err(|e| e.to_string())?;
    let dest = temp_dir.join(&filename);

    let bytes = get_object_bytes(&bucket, &key, &state).await?;
    std::fs::write(&dest, bytes).map_err(|e| e.to_string())?;

    std::process::Command::new("open")
        .arg(&dest)
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}
