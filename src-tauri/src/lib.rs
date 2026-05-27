mod commands;
mod error;
mod s3_client;

pub use error::AppError;

use std::sync::Arc;
use tokio::sync::RwLock;
use s3_client::S3State;

pub fn run() {
    let state = Arc::new(RwLock::new(S3State::default()));

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .manage(s3_client::AppState(state))
        .invoke_handler(tauri::generate_handler![
            commands::list_profiles,
            commands::set_profile,
            commands::list_buckets,
            commands::list_objects,
            commands::presign_url,
            commands::get_object_text,
            commands::save_object,
            commands::open_object,
            commands::head_object,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
