pub mod coolify;
pub mod registry;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_keyring::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_http::init())
        .plugin(tauri_plugin_opener::init())
        .manage(coolify::AppState::new())
        .invoke_handler(tauri::generate_handler![
            coolify::ops::test_connection,
            coolify::ops::set_credentials,
            coolify::ops::list_resources,
            coolify::ops::get_resource_detail,
            coolify::ops::restart_resource,
            coolify::ops::stop_resource,
            coolify::ops::deploy_resource,
            coolify::ops::tail_logs,
            registry::commands::check_image,
            registry::commands::read_image_cache,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
