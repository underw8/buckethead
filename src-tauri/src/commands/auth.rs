use aws_config::BehaviorVersion;
use aws_config::profile::ProfileFileCredentialsProvider;
use aws_sdk_s3::config::Region;
use serde::Serialize;
use tauri::State;

use crate::s3_client::AppState;
use super::{BucketInfo, aws_dt_rfc3339, make_client, fmt_sdk_err, is_sso_expired};

// ── ProfileInfo ───────────────────────────────────────────────────────────────

#[derive(Serialize, specta::Type)]
pub struct ProfileInfo {
    pub name: String,
    pub account_id: Option<String>,
    pub role: Option<String>,
    pub sso: bool,
    pub mfa: bool,
}

// ── list_profiles helpers ─────────────────────────────────────────────────────

fn parse_config_val(line: &str) -> Option<String> {
    line.split('=').nth(1).map(|v| v.trim().to_string())
}

fn apply_config_line(entry: &mut ProfileInfo, line: &str) {
    if line.starts_with("sso_account_id") {
        entry.account_id = parse_config_val(line);
    } else if line.starts_with("role_name") {
        entry.role = parse_config_val(line);
    } else if line.starts_with("role_arn") && entry.role.is_none() {
        if let Some(arn) = parse_config_val(line) {
            entry.role = Some(arn.split('/').last().unwrap_or(&arn).to_string());
        }
    } else if line.starts_with("sso_start_url") {
        entry.sso = true;
    } else if line.starts_with("mfa_serial") {
        entry.mfa = true;
    }
}

fn blank_profile(name: String) -> ProfileInfo {
    ProfileInfo { name, account_id: None, role: None, sso: false, mfa: false }
}

fn parse_aws_config(content: &str) -> std::collections::BTreeMap<String, ProfileInfo> {
    let mut map: std::collections::BTreeMap<String, ProfileInfo> = Default::default();
    let mut current: Option<String> = None;
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with('[') && line.ends_with(']') {
            let inner = line[1..line.len() - 1].trim();
            let name = inner.strip_prefix("profile ").unwrap_or(inner).trim().to_string();
            map.entry(name.clone()).or_insert_with(|| blank_profile(name.clone()));
            current = Some(name);
        } else if let Some(ref pname) = current {
            if let Some(entry) = map.get_mut(pname) {
                apply_config_line(entry, line);
            }
        }
    }
    map
}

// ── list_profiles ─────────────────────────────────────────────────────────────

#[specta::specta]
#[tauri::command]
pub async fn list_profiles() -> Result<Vec<ProfileInfo>, String> {
    let home = std::env::var("HOME").unwrap_or_default();

    let config_path = format!("{}/.aws/config", home);
    let mut profile_map = std::fs::read_to_string(&config_path)
        .map(|c| parse_aws_config(&c))
        .unwrap_or_default();

    let creds_path = format!("{}/.aws/credentials", home);
    if let Ok(content) = std::fs::read_to_string(&creds_path) {
        for line in content.lines() {
            let line = line.trim();
            if line.starts_with('[') && line.ends_with(']') {
                let name = line[1..line.len() - 1].trim().to_string();
                profile_map.entry(name.clone()).or_insert_with(|| blank_profile(name));
            }
        }
    }

    Ok(profile_map.into_values().collect())
}

// ── set_profile ───────────────────────────────────────────────────────────────

#[specta::specta]
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

#[specta::specta]
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
