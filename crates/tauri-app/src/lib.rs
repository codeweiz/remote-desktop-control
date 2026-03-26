// Tauri mobile requires a lib target. This re-exports the app setup for both
// desktop (bin) and mobile (lib) entry points.

use tauri::Manager;

mod commands;
mod tray;

/// Shared app builder setup used by both desktop `main()` and mobile `run()`.
pub fn create_app() -> tauri::Builder<tauri::Wry> {
    tauri::Builder::default()
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
            // Tray icon (desktop only, skip on mobile)
            #[cfg(desktop)]
            tray::create_tray(app)?;

            // Start embedded daemon in background
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = commands::start_embedded_daemon(app_handle.clone()).await {
                    tracing::error!("Failed to start embedded daemon: {}", e);
                    return;
                }
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
        })
}

/// Mobile entry point.
#[cfg(mobile)]
#[tauri::mobile_entry_point]
pub fn run() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .try_init();

    create_app()
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
