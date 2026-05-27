use aws_config::BehaviorVersion;
use aws_config::profile::ProfileFileCredentialsProvider;
use aws_sdk_s3::config::Region;
use aws_sdk_s3::error::ProvideErrorMetadata;
use aws_sdk_s3::presigning::PresigningConfig;
use aws_sdk_s3::Client;
use serde::Serialize;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::time::Duration;
use tauri::{Emitter, State};
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;

use crate::s3_client::AppState;

// Extract the innermost human-readable message from an AWS SDK debug string.
// The SDK uses two formats depending on error type:
//   message: "..."          — direct String field (e.g. ResolveEndpointError)
//   message: Some("...")    — Option<String> field (e.g. ServiceError, ProviderError)
fn extract_debug_msg(debug: &str) -> String {
    let mut best = String::new();
    // Try both patterns; collect all non-trivial messages and keep the last (deepest).
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
            // Advance past this occurrence
            haystack = &haystack[pos + needle.len()..];
        }
    }
    if !best.is_empty() {
        return best;
    }
    // No message field found — return full debug string so caller can still pattern-match
    debug.to_string()
}

// Format an AWS SDK error. For DispatchFailure/connector errors the top-level
// ProvideErrorMetadata is empty — extract the innermost message from the debug
// repr so callers can pattern-match on it and users see something readable.
fn fmt_sdk_err(context: &str, e: &(impl std::fmt::Debug + ProvideErrorMetadata)) -> String {
    let code = e.code();
    let message = e.message();
    let summary = match (code, message) {
        (Some(c), Some(m)) => format!("{c}: {m}"),
        (Some(c), None)    => c.to_string(),
        (None, Some(m))    => m.to_string(),
        (None, None)       => extract_debug_msg(&format!("{e:?}")),
    };
    eprintln!("[thathoo] {context} — {summary}\n  debug: {e:?}");
    summary
}

// Detect SSO/session token expiry from error strings. The AWS SDK wraps SSO
// credential errors in DispatchFailure, so we match on the debug repr too.
fn is_sso_expired(lower: &str) -> bool {
    // Classic expiry messages
    (lower.contains("token") && lower.contains("expir"))
    || lower.contains("sso") && lower.contains("expir")
    // SSO UnauthorizedException: "Session token not found or invalid"
    || lower.contains("unauthorizedexception")
    || lower.contains("session token not found")
    || lower.contains("token not found or invalid")
    // AWS IAM token expiry variants
    || lower.contains("expiredtoken")
    || lower.contains("expired token")
}

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
    pub storage_class: Option<String>,
}

#[derive(Serialize)]
pub struct ObjectMeta {
    pub content_type: Option<String>,
    pub content_length: Option<i64>,
    pub last_modified: Option<String>,
    pub etag: Option<String>,
    pub storage_class: Option<String>,
    pub cache_control: Option<String>,
    pub content_encoding: Option<String>,
    pub user_meta: Vec<(String, String)>,
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

// Returns a cached S3 client for the given region, creating one if needed.
async fn get_cached_client(
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
    // Re-check after acquiring write lock (another task may have inserted it)
    if let Some(c) = s.clients.get(region) {
        return Ok(Arc::clone(c));
    }
    let cfg = s.sdk_config.as_ref().ok_or("Not connected")?;
    let client = Arc::new(make_client(cfg, region));
    s.clients.insert(region.to_string(), Arc::clone(&client));
    Ok(client)
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

// ── ProfileInfo ───────────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct ProfileInfo {
    pub name: String,
    pub account_id: Option<String>,
    pub role: Option<String>,
    pub sso: bool,
    pub mfa: bool,
}

// ── list_profiles ─────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn list_profiles() -> Result<Vec<ProfileInfo>, String> {
    let home = std::env::var("HOME").unwrap_or_default();

    // Parse ~/.aws/config for rich profile info
    let config_path = format!("{}/.aws/config", home);
    let mut profile_map: std::collections::BTreeMap<String, ProfileInfo> = Default::default();

    if let Ok(content) = std::fs::read_to_string(&config_path) {
        let mut current: Option<String> = None;
        for line in content.lines() {
            let line = line.trim();
            if line.starts_with('[') && line.ends_with(']') {
                let inner = line[1..line.len() - 1].trim();
                let name = inner.strip_prefix("profile ").unwrap_or(inner).trim().to_string();
                profile_map.entry(name.clone()).or_insert_with(|| ProfileInfo {
                    name: name.clone(),
                    account_id: None,
                    role: None,
                    sso: false,
                    mfa: false,
                });
                current = Some(name);
            } else if let Some(ref pname) = current {
                if let Some(entry) = profile_map.get_mut(pname) {
                    if line.starts_with("sso_account_id") {
                        if let Some(val) = line.split('=').nth(1) {
                            entry.account_id = Some(val.trim().to_string());
                        }
                    } else if line.starts_with("role_name") {
                        if let Some(val) = line.split('=').nth(1) {
                            entry.role = Some(val.trim().to_string());
                        }
                    } else if line.starts_with("role_arn") && entry.role.is_none() {
                        if let Some(val) = line.split('=').nth(1) {
                            let arn = val.trim();
                            let basename = arn.split('/').last().unwrap_or(arn);
                            entry.role = Some(basename.to_string());
                        }
                    } else if line.starts_with("sso_start_url") {
                        entry.sso = true;
                    } else if line.starts_with("mfa_serial") {
                        entry.mfa = true;
                    }
                }
            }
        }
    }

    // Also pick up profiles from ~/.aws/credentials that aren't in config
    let creds_path = format!("{}/.aws/credentials", home);
    if let Ok(content) = std::fs::read_to_string(&creds_path) {
        for line in content.lines() {
            let line = line.trim();
            if line.starts_with('[') && line.ends_with(']') {
                let name = line[1..line.len() - 1].trim().to_string();
                profile_map.entry(name.clone()).or_insert_with(|| ProfileInfo {
                    name,
                    account_id: None,
                    role: None,
                    sso: false,
                    mfa: false,
                });
            }
        }
    }

    Ok(profile_map.into_values().collect())
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

    // list_buckets requires s3:ListAllMyBuckets. If denied (AccessDenied),
    // swallow and return empty — user can add buckets manually. All other
    // errors (bad credentials, SSO expiry, network) are surfaced.
    let buckets = match client.list_buckets().send().await {
        Ok(resp) => resp
            .buckets()
            .iter()
            .map(|b| BucketInfo {
                name: b.name().unwrap_or_default().to_string(),
                created: b.creation_date().map(|d| aws_dt_rfc3339(d)),
            })
            .collect(),
        Err(e) => {
            let msg = fmt_sdk_err("set_profile/list_buckets", &e);
            let lower = msg.to_lowercase();
            if lower.contains("access denied") || lower.contains("accessdenied") {
                // Permission denied on ListAllMyBuckets — not a fatal error
                vec![]
            } else if is_sso_expired(&lower) {
                return Err(format!("SSO_EXPIRED::{}", profile));
            } else if lower.contains("mfa") || lower.contains("multifactor") || lower.contains("token code") {
                return Err(format!("MFA_REQUIRED::{}", profile));
            } else if lower.contains("missing region") || lower.contains("invalid configuration") {
                return Err(format!("CREDENTIALS_ERROR::Profile '{}' failed credential resolution: {}. If using temporary credentials (source_profile with STS token), they may have expired.", profile, msg));
            } else if lower.contains("credential") || lower.contains("no credentials") {
                return Err(format!("CREDENTIALS_ERROR::{}", msg));
            } else {
                return Err(msg);
            }
        }
    };

    {
        let mut s = state.0.write().await;
        s.sdk_config = Some(sdk_config);
        s.bucket_regions.clear();
        s.clients.clear();
    }

    Ok(buckets)
}

// ── set_profile_mfa ───────────────────────────────────────────────────────────

#[tauri::command]
pub async fn set_profile_mfa(
    profile: String,
    mfa_token: String,
    state: State<'_, AppState>,
) -> Result<Vec<BucketInfo>, String> {
    // Read ~/.aws/config to find the mfa_serial for this profile
    let home = std::env::var("HOME").unwrap_or_default();
    let config_path = format!("{}/.aws/config", home);
    let mut mfa_serial = String::new();
    let mut in_profile = false;
    if let Ok(content) = std::fs::read_to_string(&config_path) {
        for line in content.lines() {
            let l = line.trim();
            if l == format!("[profile {}]", profile) || l == format!("[{}]", profile) {
                in_profile = true;
            } else if l.starts_with('[') {
                in_profile = false;
            } else if in_profile && l.starts_with("mfa_serial") {
                mfa_serial = l.split('=').nth(1).unwrap_or("").trim().to_string();
            }
        }
    }

    // Call aws sts get-session-token with the MFA token
    let output = std::process::Command::new("aws")
        .args(&[
            "sts", "get-session-token",
            "--profile", &profile,
            "--serial-number", &mfa_serial,
            "--token-code", &mfa_token,
            "--output", "json",
        ])
        .output()
        .map_err(|e| e.to_string())?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr).to_string();
        return Err(format!("MFA failed: {}", err));
    }

    // Parse the STS response and inject credentials as env vars
    let resp: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| e.to_string())?;
    let creds = &resp["Credentials"];

    std::env::set_var("AWS_ACCESS_KEY_ID", creds["AccessKeyId"].as_str().unwrap_or(""));
    std::env::set_var("AWS_SECRET_ACCESS_KEY", creds["SecretAccessKey"].as_str().unwrap_or(""));
    std::env::set_var("AWS_SESSION_TOKEN", creds["SessionToken"].as_str().unwrap_or(""));

    // Now delegate to set_profile which will pick up the env vars
    set_profile(profile, state).await
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

    let client = get_cached_client(&state, &region).await?;

    let secs = expires_in.unwrap_or(600).min(604800_u64);
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

    let client = get_cached_client(&state, &region).await?;

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

// ── shared bytes helper ───────────────────────────────────────────────────────

async fn get_object_client(
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

// ── save_object ───────────────────────────────────────────────────────────────
// Shows native Save dialog, downloads the object, writes to chosen path.
// Returns false if the user cancelled the dialog.

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

    // Task 4: clean up temp files older than 1 day
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

    // Task 3: hash bucket+key to avoid filename collisions across different prefixes
    let mut hasher = DefaultHasher::new();
    format!("{}/{}", bucket, key).hash(&mut hasher);
    let hash = hasher.finish();
    let unique_name = format!("{:016x}_{}", hash, filename);
    let dest = tmp_dir.join(&unique_name);

    // Task 4: remove stale copy before writing fresh data
    let _ = std::fs::remove_file(&dest);

    // Task 6: stream the download instead of buffering in memory
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
