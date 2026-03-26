use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    Manager,
};

/// Create the system tray icon and menu for the RTB desktop app.
pub fn create_tray(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let menu = Menu::with_items(
        app,
        &[
            &MenuItem::with_id(app, "dashboard", "Open Dashboard", true, None::<&str>)?,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, "status", "Status: Starting...", false, None::<&str>)?,
            &MenuItem::with_id(app, "sessions", "0 Sessions", false, None::<&str>)?,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, "quit", "Quit RTB", true, None::<&str>)?,
        ],
    )?;

    TrayIconBuilder::new()
        .menu(&menu)
        .tooltip("RTB 2.0")
        .on_menu_event(move |app, event| match event.id().as_ref() {
            "dashboard" => {
                // Show and focus the main window
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                } else {
                    tracing::warn!("Main window not found");
                }
            }
            "quit" => {
                // Perform graceful shutdown
                tracing::info!("Quit requested from tray menu");
                app.exit(0);
            }
            _ => {}
        })
        .build(app)?;

    Ok(())
}
