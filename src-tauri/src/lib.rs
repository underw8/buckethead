mod commands;
mod error;
mod s3_client;

pub use error::AppError;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .manage(s3_client::AppState(tokio::sync::RwLock::new(s3_client::S3State::default())))
        .invoke_handler(tauri::generate_handler![
            commands::list_profiles,
            commands::set_profile,
            commands::set_profile_mfa,
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
