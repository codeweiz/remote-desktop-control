#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tauri::Manager;

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
            // Start embedded daemon in background, then inject token into WebView
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = commands::start_embedded_daemon(app_handle.clone()).await {
                    tracing::error!("Failed to start embedded daemon: {}", e);
                    return;
                }
                // Navigate the WebView to the actual server URL with the auth token.
                // The embedded static files can't reach the HTTP/WS server (different origin),
                // so we redirect to the real server address which serves the same frontend.
                let state: tauri::State<'_, commands::DaemonStateRef> = app_handle.state();
                let (token, port) = {
                    let s = state.read().await;
                    (s.token.clone(), s.port)
                };
                if let Some(window) = app_handle.get_webview_window("main") {
                    let url = format!("http://127.0.0.1:{}?token={}", port, token);
                    tracing::info!("Navigating WebView to {}", url);
                    let _ = window.eval(&format!("window.location.href = '{}';", url));
                }
            });
            Ok(())
        });

    builder
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
