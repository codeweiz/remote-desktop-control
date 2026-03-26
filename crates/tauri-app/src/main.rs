#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod tray;

fn main() {
    // Initialize tracing for the desktop app
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .try_init();

    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_single_instance::init(|_app, _args, _cwd| {}))
        .plugin(tauri_plugin_notification::init())
        .manage(commands::create_daemon_state())
        .invoke_handler(tauri::generate_handler![
            commands::get_status,
            commands::get_config,
            commands::start_daemon,
            commands::stop_daemon,
            commands::get_sessions,
            commands::create_session,
            commands::delete_session,
        ])
        .setup(|app| {
            tray::create_tray(app)?;
            // Start embedded daemon in background
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = commands::start_embedded_daemon(app_handle).await {
                    tracing::error!("Failed to start embedded daemon: {}", e);
                }
            });
            Ok(())
        });

    builder
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
