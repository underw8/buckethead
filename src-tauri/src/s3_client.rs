use aws_config::SdkConfig;
use aws_sdk_s3::Client;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Default)]
pub struct S3State {
    pub sdk_config: Option<SdkConfig>,
    pub bucket_regions: HashMap<String, String>,
    pub clients: HashMap<String, Arc<Client>>,
}

pub struct AppState(pub RwLock<S3State>);
