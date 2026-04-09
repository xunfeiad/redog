use tauri::{
    AppHandle, Emitter,
    menu::{Menu, MenuItem, PredefinedMenuItem, Submenu},
    tray::TrayIcon,
    tray::TrayIconBuilder,
    Manager,
};

pub fn build_tray(app: &AppHandle) -> Result<TrayIcon, tauri::Error> {
    // Outbound mode submenu
    let mode_rule = MenuItem::with_id(app, "mode_rule", "● 规则判断 (Rule)", true, None::<&str>)?;
    let mode_global = MenuItem::with_id(app, "mode_global", "  全局代理 (Global)", true, None::<&str>)?;
    let mode_direct = MenuItem::with_id(app, "mode_direct", "  全局直连 (Direct)", true, None::<&str>)?;
    let mode_menu = Submenu::with_id_and_items(
        app,
        "outbound_mode",
        "出站模式 (Outbound Mode)",
        true,
        &[&mode_rule, &mode_global, &mode_direct],
    )?;

    // Proxy groups placeholder
    let proxies_placeholder = MenuItem::with_id(app, "proxies_loading", "加载代理组中...", false, None::<&str>)?;
    let proxies_menu = Submenu::with_id_and_items(
        app,
        "proxy_groups",
        "代理组 (Proxy Groups)",
        true,
        &[&proxies_placeholder],
    )?;

    // System proxy toggle
    let sys_proxy = MenuItem::with_id(app, "sys_proxy", "设置为系统代理 (System Proxy)", true, None::<&str>)?;
    let copy_cmd = MenuItem::with_id(app, "copy_cmd", "复制终端代理命令", true, None::<&str>)?;
    let allow_lan = MenuItem::with_id(app, "allow_lan", "允许局域网连接 (Allow LAN)", true, None::<&str>)?;
    let launch_login = MenuItem::with_id(app, "launch_login", "开机启动 (Launch at Login)", true, None::<&str>)?;

    // Utilities
    let dashboard = MenuItem::with_id(app, "dashboard", "仪表盘 (Dashboard)", true, None::<&str>)?;
    let connections = MenuItem::with_id(app, "connections", "连接 (Connections)", true, None::<&str>)?;
    let logs = MenuItem::with_id(app, "logs", "日志 (Logs)", true, None::<&str>)?;
    let latency_test = MenuItem::with_id(app, "latency_test", "延迟测速 (Latency Test)", true, None::<&str>)?;
    let reload_config = MenuItem::with_id(app, "reload_config", "重载配置 (Reload Config)", true, None::<&str>)?;

    // Config management
    let open_config_dir = MenuItem::with_id(app, "open_config_dir", "打开配置目录", true, None::<&str>)?;
    let about = MenuItem::with_id(app, "about", "关于 Redog", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "退出 (Quit)", true, None::<&str>)?;

    let sep1 = PredefinedMenuItem::separator(app)?;
    let sep2 = PredefinedMenuItem::separator(app)?;
    let sep3 = PredefinedMenuItem::separator(app)?;
    let sep4 = PredefinedMenuItem::separator(app)?;
    let sep5 = PredefinedMenuItem::separator(app)?;

    let menu = Menu::with_items(
        app,
        &[
            &mode_menu,
            &sep1,
            &proxies_menu,
            &sep2,
            &sys_proxy,
            &copy_cmd,
            &allow_lan,
            &launch_login,
            &sep3,
            &dashboard,
            &connections,
            &logs,
            &latency_test,
            &reload_config,
            &sep4,
            &open_config_dir,
            &about,
            &sep5,
            &quit,
        ],
    )?;

    TrayIconBuilder::with_id("main-tray")
        .icon(app.default_window_icon().cloned().unwrap())
        .menu(&menu)
        .show_menu_on_left_click(true)
        .tooltip("Redog")
        .build(app)
}

pub fn handle_menu_click(app: &AppHandle, id: &str) {
    match id {
        "quit" => {
            std::process::exit(0);
        }
        "dashboard" | "connections" | "logs" => {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
                let _ = window.emit("navigate", id);
            }
        }
        "mode_rule" => {
            if let Err(e) = crate::api::set_mode_blocking("rule") {
                tracing::error!("set mode rule failed: {}", e);
            }
        }
        "mode_global" => {
            if let Err(e) = crate::api::set_mode_blocking("global") {
                tracing::error!("set mode global failed: {}", e);
            }
        }
        "mode_direct" => {
            if let Err(e) = crate::api::set_mode_blocking("direct") {
                tracing::error!("set mode direct failed: {}", e);
            }
        }
        "sys_proxy" => {
            if let Err(e) = crate::sysproxy::toggle_system_proxy() {
                tracing::error!("toggle system proxy failed: {}", e);
            }
        }
        "copy_cmd" => {
            if let Err(e) = crate::sysproxy::copy_terminal_command_sync() {
                tracing::error!("copy terminal command failed: {}", e);
            }
        }
        "latency_test" => {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.emit("latency-test", ());
            }
        }
        "reload_config" => {
            let _ = crate::api::reload_config_blocking();
        }
        "open_config_dir" => {
            // Open the config directory in system file manager
            let config_dir = dirs::home_dir()
                .map(|h| h.join(".config").join("redog"))
                .unwrap_or_else(|| std::path::PathBuf::from("."));
            // Create the directory if it doesn't exist
            let _ = std::fs::create_dir_all(&config_dir);
            #[cfg(target_os = "macos")]
            {
                let _ = std::process::Command::new("open")
                    .arg(&config_dir)
                    .spawn();
            }
            #[cfg(target_os = "windows")]
            {
                let _ = std::process::Command::new("explorer")
                    .arg(&config_dir)
                    .spawn();
            }
            #[cfg(target_os = "linux")]
            {
                let _ = std::process::Command::new("xdg-open")
                    .arg(&config_dir)
                    .spawn();
            }
        }
        "about" => {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
                let _ = window.emit("navigate", "about");
            }
        }
        _ => {
            tracing::debug!("unhandled menu click: {}", id);
        }
    }
}
