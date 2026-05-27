use tauri::State;

use crate::s3_client::AppState;
use super::{BucketInfo, aws_dt_rfc3339, make_client};

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
