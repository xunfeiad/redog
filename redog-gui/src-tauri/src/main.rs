// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod api;
mod tray;
mod sysproxy;

use tauri::Manager;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            // Build and attach system tray
            let tray = tray::build_tray(app.handle())?;
            tray.on_menu_event(|app, event| {
                tray::handle_menu_click(app, event.id().as_ref());
            });

            // Hide the dock icon on macOS (menu bar app only)
            #[cfg(target_os = "macos")]
            {
                app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            }

            // Hide main window initially
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.hide();
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            api::get_proxies,
            api::get_rules,
            api::get_connections,
            api::get_traffic,
            api::get_version,
            api::get_configs,
            api::patch_configs,
            api::select_proxy,
            api::test_latency,
            api::fetch_subscription,
            api::set_api_config,
            api::get_api_config,
            sysproxy::set_system_proxy,
            sysproxy::get_system_proxy,
            sysproxy::copy_terminal_command,
        ])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                // Hide window instead of closing
                let _ = window.hide();
                api.prevent_close();
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
