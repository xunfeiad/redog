use serde_json::Value;
use std::sync::RwLock;

// Configurable API base URL and secret
static API_CONFIG: std::sync::LazyLock<RwLock<ApiConfig>> =
    std::sync::LazyLock::new(|| RwLock::new(ApiConfig::detect()));

struct ApiConfig {
    base_url: String,
    secret: String,
}

impl ApiConfig {
    fn detect() -> Self {
        let base_url = std::env::var("REDOG_API_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:9090".to_string());

        // Try to auto-detect ClashX secret from macOS preferences
        let secret = std::env::var("REDOG_API_SECRET").unwrap_or_else(|_| {
            #[cfg(target_os = "macos")]
            {
                detect_clashx_secret().unwrap_or_default()
            }
            #[cfg(not(target_os = "macos"))]
            {
                String::new()
            }
        });

        tracing::info!("API config: url={}, secret={}", base_url, if secret.is_empty() { "(none)" } else { "(set)" });
        ApiConfig { base_url, secret }
    }
}

#[cfg(target_os = "macos")]
fn detect_clashx_secret() -> Option<String> {
    // Read ClashX preferences to get api-secret
    let output = std::process::Command::new("defaults")
        .args(["read", "com.west2online.ClashX", "api-secret"])
        .output()
        .ok()?;
    if output.status.success() {
        let s = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !s.is_empty() {
            tracing::info!("Auto-detected ClashX API secret");
            return Some(s);
        }
    }
    // Also try Clash Verge
    let output2 = std::process::Command::new("defaults")
        .args(["read", "io.github.clash-verge-rev.clash-verge-rev", "api-secret"])
        .output()
        .ok()?;
    if output2.status.success() {
        let s = String::from_utf8_lossy(&output2.stdout).trim().to_string();
        if !s.is_empty() {
            return Some(s);
        }
    }
    None
}

fn api_client() -> reqwest::Client {
    reqwest::Client::new()
}

fn api_url(path: &str) -> String {
    let cfg = API_CONFIG.read().unwrap();
    format!("{}{}", cfg.base_url, path)
}

fn auth_header() -> Option<String> {
    let cfg = API_CONFIG.read().unwrap();
    if cfg.secret.is_empty() {
        None
    } else {
        Some(format!("Bearer {}", cfg.secret))
    }
}

async fn get(path: &str) -> Result<Value, String> {
    let url = api_url(path);
    let client = api_client();
    let mut req = client.get(&url);
    if let Some(auth) = auth_header() {
        req = req.header("Authorization", auth);
    }
    let resp = req.send().await.map_err(|e| e.to_string())?;
    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("HTTP {} - {}", status, body));
    }
    resp.json::<Value>().await.map_err(|e| e.to_string())
}

async fn patch_json(path: &str, body: Value) -> Result<Value, String> {
    let url = api_url(path);
    let client = api_client();
    let mut req = client.patch(&url).json(&body);
    if let Some(auth) = auth_header() {
        req = req.header("Authorization", auth);
    }
    let resp = req.send().await.map_err(|e| e.to_string())?;
    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("HTTP {} - {}", status, body));
    }
    resp.json::<Value>().await.or_else(|_| Ok(Value::Null))
}

async fn put_json(path: &str, body: Value) -> Result<Value, String> {
    let url = api_url(path);
    let client = api_client();
    let mut req = client.put(&url).json(&body);
    if let Some(auth) = auth_header() {
        req = req.header("Authorization", auth);
    }
    let resp = req.send().await.map_err(|e| e.to_string())?;
    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("HTTP {} - {}", status, body));
    }
    resp.json::<Value>().await.or_else(|_| Ok(Value::Null))
}

// --- Tauri commands ---

#[tauri::command]
pub async fn get_proxies() -> Result<Value, String> {
    get("/proxies").await
}

#[tauri::command]
pub async fn get_rules() -> Result<Value, String> {
    get("/rules").await
}

#[tauri::command]
pub async fn get_connections() -> Result<Value, String> {
    get("/connections").await
}

#[tauri::command]
pub async fn get_traffic() -> Result<Value, String> {
    get("/traffic").await
}

#[tauri::command]
pub async fn get_version() -> Result<Value, String> {
    get("/version").await
}

#[tauri::command]
pub async fn get_configs() -> Result<Value, String> {
    get("/configs").await
}

#[tauri::command]
pub async fn patch_configs(body: Value) -> Result<Value, String> {
    patch_json("/configs", body).await
}

#[tauri::command]
pub async fn select_proxy(group: String, name: String) -> Result<Value, String> {
    tracing::info!("select_proxy: group={}, name={}", group, name);
    let path = format!("/proxies/{}", urlencoding::encode(&group));
    let result = put_json(&path, serde_json::json!({ "name": name })).await;
    match &result {
        Ok(_) => tracing::info!("select_proxy success"),
        Err(e) => tracing::error!("select_proxy failed: {}", e),
    }
    result
}

#[tauri::command]
pub async fn test_latency(name: String) -> Result<Value, String> {
    let path = format!(
        "/proxies/{}/delay?url=http://www.gstatic.com/generate_204&timeout=5000",
        urlencoding::encode(&name)
    );
    get(&path).await
}

#[tauri::command]
pub async fn set_api_config(base_url: String, secret: String) -> Result<(), String> {
    let mut cfg = API_CONFIG.write().map_err(|e| e.to_string())?;
    if !base_url.is_empty() {
        cfg.base_url = base_url;
    }
    cfg.secret = secret;
    tracing::info!("API config updated: url={}", cfg.base_url);
    Ok(())
}

#[tauri::command]
pub async fn get_api_config() -> Result<Value, String> {
    let cfg = API_CONFIG.read().map_err(|e| e.to_string())?;
    Ok(serde_json::json!({
        "base_url": cfg.base_url,
        "has_secret": !cfg.secret.is_empty(),
    }))
}

#[tauri::command]
pub async fn fetch_subscription(url: String) -> Result<Value, String> {
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err("Only HTTP/HTTPS URLs allowed".to_string());
    }
    let client = reqwest::Client::builder()
        .user_agent("Redog/0.1")
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("请求失败: {}", e))?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }
    let text = resp.text().await.map_err(|e| e.to_string())?;

    // Try base64 decode
    let decoded = base64_decode(&text);
    let content = decoded.as_deref().unwrap_or(&text);

    let mut nodes = Vec::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(node) = parse_share_uri(line) {
            nodes.push(node);
        }
    }

    // If no share URIs found, try parsing as YAML (Clash format)
    if nodes.is_empty() {
        if let Ok(yaml) = serde_yaml::from_str::<serde_yaml::Value>(&text) {
            if let Some(proxies) = yaml.get("proxies").and_then(|p| p.as_sequence()) {
                for p in proxies {
                    let name = p.get("name").and_then(|v| v.as_str()).unwrap_or("unnamed");
                    let typ = p.get("type").and_then(|v| v.as_str()).unwrap_or("ss");
                    let server = p.get("server").and_then(|v| v.as_str()).unwrap_or("");
                    let port = p.get("port").and_then(|v| v.as_u64()).unwrap_or(0);
                    let password = p
                        .get("password")
                        .or_else(|| p.get("uuid"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    let cipher = p.get("cipher").and_then(|v| v.as_str()).unwrap_or("");
                    let tls = p.get("tls").and_then(|v| v.as_bool()).unwrap_or(false);
                    nodes.push(serde_json::json!({
                        "name": name,
                        "type": typ,
                        "server": server,
                        "port": port,
                        "password": password,
                        "cipher": cipher,
                        "tls": tls,
                    }));
                }
            }
        }
    }

    Ok(serde_json::json!({ "nodes": nodes, "count": nodes.len() }))
}

fn base64_decode(input: &str) -> Option<String> {
    use base64::Engine;
    let cleaned: String = input.trim().chars().filter(|c| !c.is_whitespace()).collect();
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(&cleaned)
        .or_else(|_| base64::engine::general_purpose::STANDARD_NO_PAD.decode(&cleaned))
        .ok()?;
    String::from_utf8(decoded).ok()
}

fn parse_share_uri(uri: &str) -> Option<Value> {
    if uri.starts_with("ss://") {
        let rest = uri.strip_prefix("ss://")?;
        let (main, name) = if let Some(idx) = rest.find('#') {
            (&rest[..idx], urlencoding::decode(&rest[idx + 1..]).unwrap_or_default().to_string())
        } else {
            (rest, "SS Node".to_string())
        };
        let decoded = base64_decode(main).unwrap_or_else(|| main.to_string());
        let at_idx = decoded.rfind('@')?;
        let user_info = &decoded[..at_idx];
        let host_port = &decoded[at_idx + 1..];
        let (cipher, password) = if let Some(idx) = user_info.find(':') {
            (&user_info[..idx], &user_info[idx + 1..])
        } else {
            ("aes-256-gcm", user_info)
        };
        let parts: Vec<&str> = host_port.splitn(2, ':').collect();
        let server = parts.first().unwrap_or(&"");
        let port: u64 = parts.get(1).and_then(|p| p.parse().ok()).unwrap_or(0);
        return Some(serde_json::json!({
            "name": name, "type": "ss", "server": server, "port": port,
            "password": password, "cipher": cipher, "tls": false,
        }));
    }
    if uri.starts_with("vmess://") {
        let json_str = base64_decode(uri.strip_prefix("vmess://")?)?;
        let obj: Value = serde_json::from_str(&json_str).ok()?;
        return Some(serde_json::json!({
            "name": obj.get("ps").or(obj.get("remark")).and_then(|v| v.as_str()).unwrap_or("VMess"),
            "type": "vmess",
            "server": obj.get("add").and_then(|v| v.as_str()).unwrap_or(""),
            "port": obj.get("port").and_then(|v| v.as_str().and_then(|s| s.parse::<u64>().ok()).or(v.as_u64())).unwrap_or(0),
            "password": obj.get("id").and_then(|v| v.as_str()).unwrap_or(""),
            "cipher": obj.get("scy").and_then(|v| v.as_str()).unwrap_or("auto"),
            "tls": obj.get("tls").and_then(|v| v.as_str()) == Some("tls"),
        }));
    }
    if uri.starts_with("trojan://") {
        let without = uri.strip_prefix("trojan://")?;
        let at_idx = without.find('@')?;
        let password = &without[..at_idx];
        let rest = &without[at_idx + 1..];
        let (host_port, name) = if let Some(idx) = rest.find('#') {
            (&rest[..idx], urlencoding::decode(&rest[idx + 1..]).unwrap_or_default().to_string())
        } else {
            (rest, "Trojan".to_string())
        };
        let hp = host_port.split('?').next().unwrap_or(host_port);
        let parts: Vec<&str> = hp.splitn(2, ':').collect();
        let server = parts.first().unwrap_or(&"");
        let port: u64 = parts.get(1).and_then(|p| p.parse().ok()).unwrap_or(443);
        return Some(serde_json::json!({
            "name": name, "type": "trojan", "server": server, "port": port,
            "password": password, "cipher": "", "tls": true,
        }));
    }
    None
}

// Blocking helpers used from the tray (non-async context)
pub fn set_mode_blocking(mode: &str) -> Result<(), String> {
    let mode = mode.to_string();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let body = serde_json::json!({ "mode": mode });
            if let Err(e) = patch_json("/configs", body).await {
                tracing::error!("set_mode_blocking failed: {}", e);
            }
        });
    });
    Ok(())
}

pub fn reload_config_blocking() -> Result<(), String> {
    std::thread::spawn(|| {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let body = serde_json::json!({ "path": "" });
            if let Err(e) = put_json("/configs", body).await {
                tracing::error!("reload_config_blocking failed: {}", e);
            }
        });
    });
    Ok(())
}
