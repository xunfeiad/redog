use std::sync::atomic::{AtomicBool, Ordering};

static SYS_PROXY_ENABLED: AtomicBool = AtomicBool::new(false);

const PROXY_HOST: &str = "127.0.0.1";
const PROXY_PORT: u16 = 7890;

#[tauri::command]
pub fn get_system_proxy() -> bool {
    SYS_PROXY_ENABLED.load(Ordering::Relaxed)
}

#[tauri::command]
pub fn set_system_proxy(enable: bool) -> Result<(), String> {
    if enable {
        enable_proxy()?;
    } else {
        disable_proxy()?;
    }
    SYS_PROXY_ENABLED.store(enable, Ordering::Relaxed);
    Ok(())
}

pub fn toggle_system_proxy() -> Result<(), String> {
    let cur = SYS_PROXY_ENABLED.load(Ordering::Relaxed);
    set_system_proxy(!cur)
}

#[tauri::command]
pub fn copy_terminal_command() -> Result<String, String> {
    copy_terminal_command_sync()
}

pub fn copy_terminal_command_sync() -> Result<String, String> {
    let cmd = format!(
        "export https_proxy=http://{host}:{port} http_proxy=http://{host}:{port} all_proxy=socks5://{host}:{port}",
        host = PROXY_HOST,
        port = PROXY_PORT
    );
    // Copy to clipboard using platform-specific tools
    #[cfg(target_os = "macos")]
    {
        use std::io::Write;
        use std::process::{Command, Stdio};
        let mut p = Command::new("pbcopy")
            .stdin(Stdio::piped())
            .spawn()
            .map_err(|e| e.to_string())?;
        p.stdin
            .as_mut()
            .unwrap()
            .write_all(cmd.as_bytes())
            .map_err(|e| e.to_string())?;
        let _ = p.wait();
    }
    #[cfg(target_os = "windows")]
    {
        use std::io::Write;
        use std::process::{Command, Stdio};
        let mut p = Command::new("clip")
            .stdin(Stdio::piped())
            .spawn()
            .map_err(|e| e.to_string())?;
        p.stdin
            .as_mut()
            .unwrap()
            .write_all(cmd.as_bytes())
            .map_err(|e| e.to_string())?;
        let _ = p.wait();
    }
    Ok(cmd)
}

// --- macOS implementation via networksetup ---
#[cfg(target_os = "macos")]
fn enable_proxy() -> Result<(), String> {
    let service = active_network_service()?;
    let port = PROXY_PORT.to_string();
    run_networksetup_batch(&[
        &["-setwebproxy", &service, PROXY_HOST, &port],
        &["-setsecurewebproxy", &service, PROXY_HOST, &port],
        &["-setsocksfirewallproxy", &service, PROXY_HOST, &port],
        &["-setwebproxystate", &service, "on"],
        &["-setsecurewebproxystate", &service, "on"],
        &["-setsocksfirewallproxystate", &service, "on"],
    ])
}

#[cfg(target_os = "macos")]
fn disable_proxy() -> Result<(), String> {
    let service = active_network_service()?;
    run_networksetup_batch(&[
        &["-setwebproxystate", &service, "off"],
        &["-setsecurewebproxystate", &service, "off"],
        &["-setsocksfirewallproxystate", &service, "off"],
    ])
}

#[cfg(target_os = "macos")]
fn active_network_service() -> Result<String, String> {
    use std::process::Command;
    let route_output = Command::new("route")
        .args(["-n", "get", "default"])
        .output()
        .map_err(|e| format!("route 命令失败: {}", e))?;
    let route_str = String::from_utf8_lossy(&route_output.stdout);
    let iface = route_str
        .lines()
        .find(|l| l.contains("interface:"))
        .and_then(|l| l.split(':').nth(1))
        .map(|s| s.trim().to_string());

    if let Some(iface) = iface {
        let list_output = Command::new("networksetup")
            .args(["-listallhardwareports"])
            .output()
            .map_err(|e| e.to_string())?;
        let list_str = String::from_utf8_lossy(&list_output.stdout);
        let mut current_service = String::new();
        for line in list_str.lines() {
            if let Some(name) = line.strip_prefix("Hardware Port: ") {
                current_service = name.to_string();
            }
            if let Some(dev) = line.strip_prefix("Device: ") {
                if dev.trim() == iface {
                    return Ok(current_service);
                }
            }
        }
    }
    Ok("Wi-Fi".to_string())
}

/// Run multiple networksetup commands. Try without elevation first;
/// if any command needs admin privileges, re-run ALL commands in a
/// single elevated osascript call (one password prompt).
#[cfg(target_os = "macos")]
fn run_networksetup_batch(commands: &[&[&str]]) -> Result<(), String> {
    use std::process::Command;

    // First try without elevation
    let mut needs_elevation = false;
    for args in commands {
        let output = Command::new("networksetup")
            .args(*args)
            .output()
            .map_err(|e| format!("networksetup 启动失败: {}", e))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("admin privileges") || output.status.code() == Some(14) {
                needs_elevation = true;
                break;
            }
            let stdout = String::from_utf8_lossy(&output.stdout);
            let detail = if stderr.trim().is_empty() { stdout } else { stderr };
            return Err(format!(
                "networksetup {} 失败 (exit {}): {}",
                args.first().unwrap_or(&""),
                output.status.code().unwrap_or(-1),
                detail.trim()
            ));
        }
    }

    if !needs_elevation {
        return Ok(());
    }

    // Build a single shell script with all commands, run with one elevation
    let shell_lines: Vec<String> = commands
        .iter()
        .map(|args| {
            let escaped: Vec<String> = args
                .iter()
                .map(|a| format!("'{}'", a.replace('\'', "'\\''")))
                .collect();
            format!("networksetup {}", escaped.join(" "))
        })
        .collect();
    let shell_script = shell_lines.join(" && ");
    let applescript = format!(
        "do shell script \"{}\" with administrator privileges",
        shell_script.replace('\\', "\\\\").replace('"', "\\\"")
    );
    let elevated = Command::new("osascript")
        .args(["-e", &applescript])
        .output()
        .map_err(|e| format!("osascript 启动失败: {}", e))?;
    if !elevated.status.success() {
        let err = String::from_utf8_lossy(&elevated.stderr);
        if err.contains("User canceled") || err.contains("-128") {
            return Err("用户取消了授权".to_string());
        }
        return Err(format!("提权执行失败: {}", err.trim()));
    }
    Ok(())
}

// --- Windows implementation via registry ---
#[cfg(target_os = "windows")]
fn enable_proxy() -> Result<(), String> {
    use std::process::Command;
    let proxy = format!("{}:{}", PROXY_HOST, PROXY_PORT);
    run_reg(&[
        "add",
        r"HKCU\Software\Microsoft\Windows\CurrentVersion\Internet Settings",
        "/v",
        "ProxyEnable",
        "/t",
        "REG_DWORD",
        "/d",
        "1",
        "/f",
    ])?;
    run_reg(&[
        "add",
        r"HKCU\Software\Microsoft\Windows\CurrentVersion\Internet Settings",
        "/v",
        "ProxyServer",
        "/t",
        "REG_SZ",
        "/d",
        &proxy,
        "/f",
    ])?;
    let _ = Command::new("RUNDLL32.EXE")
        .args(["wininet.dll,InternetSetOption", "0", "0", "0", "0"])
        .status();
    Ok(())
}

#[cfg(target_os = "windows")]
fn disable_proxy() -> Result<(), String> {
    use std::process::Command;
    run_reg(&[
        "add",
        r"HKCU\Software\Microsoft\Windows\CurrentVersion\Internet Settings",
        "/v",
        "ProxyEnable",
        "/t",
        "REG_DWORD",
        "/d",
        "0",
        "/f",
    ])?;
    let _ = Command::new("RUNDLL32.EXE")
        .args(["wininet.dll,InternetSetOption", "0", "0", "0", "0"])
        .status();
    Ok(())
}

#[cfg(target_os = "windows")]
fn run_reg(args: &[&str]) -> Result<(), String> {
    use std::process::Command;
    let output = Command::new("reg")
        .args(args)
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).to_string());
    }
    Ok(())
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn enable_proxy() -> Result<(), String> {
    Err("unsupported platform".into())
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn disable_proxy() -> Result<(), String> {
    Err("unsupported platform".into())
}
