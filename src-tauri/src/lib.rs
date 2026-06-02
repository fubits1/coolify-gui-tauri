pub mod coolify;
pub mod registry;
pub mod secrets;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Init tracing-subscriber so tracing::warn!/info!/debug! actually print.
    // Default to INFO; override via RUST_LOG=coolify_gui_lib=debug etc.
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_target(true)
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_http::init())
        .plugin(tauri_plugin_opener::init())
        .manage(coolify::AppState::new())
        .invoke_handler(tauri::generate_handler![
            coolify::ops::test_connection,
            coolify::ops::get_current_team,
            coolify::ops::set_credentials,
            coolify::ops::load_credentials,
            coolify::ops::clear_credentials,
            coolify::ops::list_resources,
            coolify::ops::get_resource_detail,
            coolify::ops::get_resource_envs,
            coolify::ops::restart_resource,
            coolify::ops::stop_resource,
            coolify::ops::deploy_resource,
            coolify::ops::tail_logs,
            coolify::ops::debug_dump_endpoints,
            registry::commands::check_image,
            registry::commands::read_image_cache,
            secrets::migrate_legacy_token_cmd,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
