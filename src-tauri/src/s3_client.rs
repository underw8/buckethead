use aws_config::SdkConfig;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Default)]
pub struct S3State {
    pub sdk_config: Option<SdkConfig>,
    pub bucket_regions: HashMap<String, String>,
}

pub struct AppState(pub Arc<RwLock<S3State>>);
