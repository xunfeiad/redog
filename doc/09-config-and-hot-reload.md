# 配置系统与热重载

## 配置文件格式

使用 YAML 格式，通过 `serde` + `serde_yaml` 进行反序列化:

```yaml
# 基本配置
port: 7890                    # HTTP 代理端口
socks-port: 7891              # SOCKS5 代理端口
mixed-port: 7890              # 混合端口 (HTTP + SOCKS)
redir-port: 7892              # Redirect 端口 (Linux)
tproxy-port: 7893             # TPROXY 端口 (Linux)
allow-lan: false              # 是否允许局域网访问
bind-address: "*"             # 监听地址
mode: rule                    # rule | global | direct
log-level: info               # silent | error | warning | info | debug

# 外部控制
external-controller: 127.0.0.1:9090
external-ui: dashboard        # Web UI 目录
secret: ""                    # API 密钥

# DNS 配置
dns:
  enable: true
  listen: 0.0.0.0:53
  enhanced-mode: fake-ip
  fake-ip-range: 198.18.0.1/16
  fake-ip-filter:
    - "*.local"
    - "*.lan"
  nameserver:
    - https://dns.google/dns-query
    - tls://1.1.1.1:853
  fallback:
    - https://1.0.0.1/dns-query
  fallback-filter:
    geoip: true
    geoip-code: CN
  nameserver-policy:
    "geosite:cn":
      - 223.5.5.5
      - 114.114.114.114

# TUN 配置
tun:
  enable: false
  stack: system                # system | gvisor | mixed
  dns-hijack:
    - any:53
  auto-route: true
  auto-detect-interface: true

# 代理节点
proxies:
  - name: "ss-hk"
    type: ss
    server: 1.2.3.4
    port: 8388
    cipher: aes-256-gcm
    password: "secret"

  - name: "vmess-jp"
    type: vmess
    server: 5.6.7.8
    port: 443
    uuid: "a3482e88-..."
    alterId: 0
    cipher: auto
    tls: true
    network: ws
    ws-opts:
      path: /path
      headers:
        Host: example.com

  - name: "trojan-us"
    type: trojan
    server: 9.10.11.12
    port: 443
    password: "trojan-password"
    sni: example.com

# 代理组
proxy-groups:
  - name: "Auto"
    type: url-test
    proxies: [ss-hk, vmess-jp, trojan-us]
    url: http://www.gstatic.com/generate_204
    interval: 300
    tolerance: 50

  - name: "Select"
    type: select
    proxies: [Auto, ss-hk, vmess-jp, trojan-us, DIRECT]

# 代理提供者 (远程订阅)
proxy-providers:
  subscription:
    type: http
    url: "https://example.com/sub"
    interval: 3600
    path: ./providers/sub.yaml
    health-check:
      enable: true
      url: http://www.gstatic.com/generate_204
      interval: 600

# 规则提供者
rule-providers:
  reject-list:
    type: http
    behavior: domain
    url: "https://example.com/reject.yaml"
    path: ./providers/reject.yaml
    interval: 86400

# 规则 (按顺序匹配)
rules:
  - RULE-SET,reject-list,REJECT
  - DOMAIN-SUFFIX,google.com,Auto
  - DOMAIN-KEYWORD,facebook,Auto
  - IP-CIDR,192.168.0.0/16,DIRECT
  - IP-CIDR,10.0.0.0/8,DIRECT
  - GEOIP,CN,DIRECT
  - MATCH,Select
```

## Rust 配置结构体

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct Config {
    #[serde(default = "default_port")]
    pub port: Option<u16>,

    #[serde(rename = "socks-port")]
    pub socks_port: Option<u16>,

    #[serde(rename = "mixed-port")]
    pub mixed_port: Option<u16>,

    #[serde(rename = "redir-port")]
    pub redir_port: Option<u16>,

    #[serde(rename = "tproxy-port")]
    pub tproxy_port: Option<u16>,

    #[serde(rename = "allow-lan", default)]
    pub allow_lan: bool,

    #[serde(rename = "bind-address", default = "default_bind")]
    pub bind_address: String,

    #[serde(default = "default_mode")]
    pub mode: TunnelMode,

    #[serde(rename = "log-level", default = "default_log_level")]
    pub log_level: LogLevel,

    #[serde(rename = "external-controller")]
    pub external_controller: Option<String>,

    #[serde(rename = "external-ui")]
    pub external_ui: Option<String>,

    pub secret: Option<String>,

    pub dns: Option<DnsConfig>,
    pub tun: Option<TunConfig>,

    #[serde(default)]
    pub proxies: Vec<ProxyConfig>,

    #[serde(rename = "proxy-groups", default)]
    pub proxy_groups: Vec<ProxyGroupConfig>,

    #[serde(rename = "proxy-providers", default)]
    pub proxy_providers: HashMap<String, ProviderConfig>,

    #[serde(rename = "rule-providers", default)]
    pub rule_providers: HashMap<String, RuleProviderConfig>,

    #[serde(default)]
    pub rules: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct DnsConfig {
    pub enable: bool,
    pub listen: Option<String>,
    #[serde(rename = "enhanced-mode")]
    pub enhanced_mode: Option<DnsMode>,
    #[serde(rename = "fake-ip-range")]
    pub fake_ip_range: Option<String>,
    #[serde(rename = "fake-ip-filter", default)]
    pub fake_ip_filter: Vec<String>,
    #[serde(default)]
    pub nameserver: Vec<String>,
    #[serde(default)]
    pub fallback: Vec<String>,
    #[serde(rename = "nameserver-policy", default)]
    pub nameserver_policy: HashMap<String, Vec<String>>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TunConfig {
    pub enable: bool,
    #[serde(default = "default_stack")]
    pub stack: String,
    #[serde(rename = "dns-hijack", default)]
    pub dns_hijack: Vec<String>,
    #[serde(rename = "auto-route", default)]
    pub auto_route: bool,
    #[serde(rename = "auto-detect-interface", default)]
    pub auto_detect_interface: bool,
}

/// 代理节点配置 (使用 serde 的 tagged enum)
#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum ProxyConfig {
    #[serde(rename = "ss")]
    Shadowsocks {
        name: String,
        server: String,
        port: u16,
        cipher: String,
        password: String,
        #[serde(default)]
        udp: bool,
    },
    #[serde(rename = "vmess")]
    VMess {
        name: String,
        server: String,
        port: u16,
        uuid: String,
        #[serde(rename = "alterId", default)]
        alter_id: u16,
        cipher: String,
        #[serde(default)]
        tls: bool,
        #[serde(default)]
        network: Option<String>,
        #[serde(rename = "ws-opts")]
        ws_opts: Option<WsOpts>,
    },
    #[serde(rename = "trojan")]
    Trojan {
        name: String,
        server: String,
        port: u16,
        password: String,
        #[serde(default)]
        sni: Option<String>,
    },
    // ... 其他代理类型
}
```

## 配置验证

```rust
impl Config {
    /// 加载并验证配置
    pub fn load(path: &str) -> Result<Self, Error> {
        let content = std::fs::read_to_string(path)?;
        let config: Config = serde_yaml::from_str(&content)?;
        config.validate()?;
        Ok(config)
    }

    fn validate(&self) -> Result<(), Error> {
        // 1. 端口冲突检查
        let mut ports = Vec::new();
        if let Some(p) = self.port { ports.push(p); }
        if let Some(p) = self.socks_port { ports.push(p); }
        if let Some(p) = self.mixed_port { ports.push(p); }
        let unique: HashSet<_> = ports.iter().collect();
        if unique.len() != ports.len() {
            return Err(Error::Config("port conflict detected".into()));
        }

        // 2. 规则引用的代理必须存在
        let proxy_names: HashSet<_> = self.proxies.iter()
            .map(|p| p.name())
            .chain(self.proxy_groups.iter().map(|g| g.name.as_str()))
            .chain(["DIRECT", "REJECT"].iter().copied())
            .collect();

        for rule in &self.rules {
            let parts: Vec<&str> = rule.splitn(3, ',').collect();
            let adapter = parts.last().unwrap();
            if !proxy_names.contains(adapter) {
                return Err(Error::Config(
                    format!("rule references unknown proxy: {}", adapter)
                ));
            }
        }

        // 3. 代理组引用的代理必须存在
        for group in &self.proxy_groups {
            for proxy_name in &group.proxies {
                if !proxy_names.contains(proxy_name.as_str()) {
                    return Err(Error::Config(
                        format!("proxy group '{}' references unknown proxy: {}",
                            group.name, proxy_name)
                    ));
                }
            }
        }

        Ok(())
    }
}
```

## 热重载机制

使用 `arc-swap` 实现配置的原子替换，确保正在处理的连接不受影响:

```rust
use arc_swap::ArcSwap;
use notify::{Watcher, RecursiveMode, Event};

/// 运行时状态 (从配置构建)
pub struct RuntimeState {
    pub proxies: HashMap<String, Arc<dyn ProxyAdapter>>,
    pub rules: Vec<Box<dyn Rule>>,
    pub dns_resolver: Arc<dyn DnsResolver>,
    pub mode: TunnelMode,
}

/// 全局运行时 (使用 ArcSwap 支持原子替换)
pub struct Runtime {
    state: ArcSwap<RuntimeState>,
    config_path: String,
}

impl Runtime {
    /// 热重载配置
    pub async fn reload(&self) -> Result<(), Error> {
        // 1. 加载新配置
        let new_config = Config::load(&self.config_path)?;

        // 2. 构建新的运行时状态
        let new_state = self.build_state(&new_config).await?;

        // 3. 原子替换 (旧状态在所有引用释放后自动清理)
        let old_state = self.state.swap(Arc::new(new_state));

        // 4. 关闭旧状态中不再需要的资源
        // (旧连接可以继续使用旧的 proxy, 因为它们持有 Arc)
        tracing::info!("config reloaded successfully");

        // 5. 可选: 关闭所有现有连接 (强制使用新规则)
        // self.close_all_connections();

        Ok(())
    }

    /// 获取当前运行时状态 (读操作, 无锁)
    pub fn state(&self) -> arc_swap::Guard<Arc<RuntimeState>> {
        self.state.load()
    }
}
```

### 文件监控

```rust
/// 监控配置文件变化, 自动触发重载
async fn watch_config(runtime: Arc<Runtime>, config_path: String) {
    let (tx, mut rx) = tokio::sync::mpsc::channel(1);

    let mut watcher = notify::recommended_watcher(move |result: Result<Event, _>| {
        if let Ok(event) = result {
            if event.kind.is_modify() {
                let _ = tx.blocking_send(());
            }
        }
    }).unwrap();

    watcher.watch(
        std::path::Path::new(&config_path),
        RecursiveMode::NonRecursive,
    ).unwrap();

    // 防抖: 文件可能在短时间内触发多次修改事件
    while rx.recv().await.is_some() {
        // 等待 500ms 防抖
        tokio::time::sleep(Duration::from_millis(500)).await;
        // 清空队列中的重复事件
        while rx.try_recv().is_ok() {}

        match runtime.reload().await {
            Ok(_) => tracing::info!("config reloaded"),
            Err(e) => tracing::error!("config reload failed: {}", e),
        }
    }
}
```

## Provider 远程更新机制

```rust
/// HTTP Provider: 定期从远程 URL 拉取数据
pub struct HttpProvider {
    name: String,
    url: String,
    path: PathBuf,        // 本地缓存路径
    interval: Duration,   // 更新间隔
    data: ArcSwap<ProviderData>,
}

impl HttpProvider {
    /// 后台更新循环
    pub async fn update_loop(self: Arc<Self>) {
        let mut ticker = tokio::time::interval(self.interval);
        loop {
            ticker.tick().await;
            match self.fetch_and_update().await {
                Ok(_) => tracing::info!("provider '{}' updated", self.name),
                Err(e) => tracing::warn!("provider '{}' update failed: {}", self.name, e),
            }
        }
    }

    async fn fetch_and_update(&self) -> Result<(), Error> {
        // 1. HTTP GET 获取数据
        let client = reqwest::Client::new();
        let resp = client.get(&self.url)
            .timeout(Duration::from_secs(30))
            .send()
            .await?;
        let body = resp.text().await?;

        // 2. 解析数据
        let new_data = ProviderData::parse(&body)?;

        // 3. 原子替换
        self.data.store(Arc::new(new_data));

        // 4. 写入本地缓存
        tokio::fs::write(&self.path, &body).await?;

        Ok(())
    }
}
```

## RESTful API 热更新端点

```rust
use axum::{Router, Json, extract::State};

async fn patch_config(
    State(runtime): State<Arc<Runtime>>,
    Json(patch): Json<ConfigPatch>,
) -> Result<Json<()>, ApiError> {
    // 部分更新: 只更新指定字段
    if let Some(mode) = patch.mode {
        runtime.set_mode(mode);
    }
    if let Some(allow_lan) = patch.allow_lan {
        runtime.set_allow_lan(allow_lan);
    }
    Ok(Json(()))
}

async fn put_config(
    State(runtime): State<Arc<Runtime>>,
    Json(body): Json<ConfigReload>,
) -> Result<Json<()>, ApiError> {
    // 完整重载
    if body.force {
        runtime.reload().await?;
    }
    Ok(Json(()))
}
```
