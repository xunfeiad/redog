# 代理组原理

## 概述

代理组是对多个代理节点的高层抽象，实现了与单个代理相同的 `ProxyAdapter` trait。隧道核心不区分单个代理和代理组，这使得代理组可以嵌套组合。

## 代理组类型

```
┌─────────────────────────────────────────────────────────┐
│                     ProxyAdapter trait                    │
├─────────┬───────────┬───────────┬───────────┬───────────┤
│Selector │ URLTest   │ Fallback  │LoadBalance│  Relay    │
│手动选择  │ 自动测速   │ 故障转移   │ 负载均衡  │ 代理链    │
│         │           │           │           │           │
│ 用户通过 │ 定期测速   │ 选择第一个 │ 分散流量   │ 串联多个   │
│ API 切换│ 选最快    │ 可用节点   │ 到多个节点 │ 代理节点   │
└─────────┴───────────┴───────────┴───────────┴───────────┘
```

## 1. Selector (手动选择)

最简单的代理组。用户通过 API 手动选择使用哪个代理。

```rust
use std::sync::RwLock;

pub struct Selector {
    name: String,
    proxies: Vec<Arc<dyn ProxyAdapter>>,
    /// 当前选中的代理索引
    selected: RwLock<usize>,
}

#[async_trait]
impl ProxyAdapter for Selector {
    fn name(&self) -> &str { &self.name }
    fn adapter_type(&self) -> AdapterType { AdapterType::Selector }

    async fn connect_stream(&self, metadata: &Metadata) -> Result<Box<dyn ProxyStream>, Error> {
        let idx = *self.selected.read().unwrap();
        self.proxies[idx].connect_stream(metadata).await
    }

    async fn connect_datagram(&self, metadata: &Metadata) -> Result<Box<dyn ProxyDatagram>, Error> {
        let idx = *self.selected.read().unwrap();
        self.proxies[idx].connect_datagram(metadata).await
    }

    fn support_udp(&self) -> bool {
        let idx = *self.selected.read().unwrap();
        self.proxies[idx].support_udp()
    }
}

impl Selector {
    /// API 调用: 切换选中的代理
    pub fn select(&self, name: &str) -> Result<(), Error> {
        let idx = self.proxies.iter()
            .position(|p| p.name() == name)
            .ok_or(Error::Config(format!("proxy not found: {}", name)))?;
        *self.selected.write().unwrap() = idx;
        Ok(())
    }
}
```

## 2. URLTest (自动测速选择)

定期对所有代理进行延迟测试，自动选择最快的节点。

```rust
use tokio::time::{Duration, interval};

pub struct URLTest {
    name: String,
    proxies: Vec<Arc<dyn ProxyAdapter>>,
    /// 测试 URL (通常用 http://www.gstatic.com/generate_204)
    test_url: String,
    /// 测试间隔
    test_interval: Duration,
    /// 可接受的延迟差异 (容忍度)
    /// 只有新节点比当前节点快 tolerance 以上才切换
    tolerance: u16,
    /// 当前最快的代理索引
    fastest: RwLock<usize>,
    /// 各代理的延迟历史
    delay_history: RwLock<Vec<DelayHistory>>,
}

impl URLTest {
    /// 后台健康检查任务
    pub async fn health_check_loop(self: Arc<Self>) {
        let mut ticker = interval(self.test_interval);
        loop {
            ticker.tick().await;
            self.check_all().await;
        }
    }

    /// 并发测试所有代理的延迟
    async fn check_all(&self) {
        use futures::future::join_all;

        let futures: Vec<_> = self.proxies.iter().enumerate().map(|(idx, proxy)| {
            let url = self.test_url.clone();
            async move {
                let start = std::time::Instant::now();
                let result = url_test(proxy.as_ref(), &url).await;
                let delay = match result {
                    Ok(_) => start.elapsed().as_millis() as u16,
                    Err(_) => u16::MAX, // 失败标记为最大延迟
                };
                (idx, delay)
            }
        }).collect();

        let results = join_all(futures).await;

        // 更新延迟历史
        let mut history = self.delay_history.write().unwrap();
        for (idx, delay) in &results {
            history[*idx].push(*delay);
        }

        // 找到最快的代理
        let current_fastest = *self.fastest.read().unwrap();
        let current_delay = results.iter()
            .find(|(idx, _)| *idx == current_fastest)
            .map(|(_, d)| *d)
            .unwrap_or(u16::MAX);

        if let Some((new_fastest, new_delay)) = results.iter()
            .min_by_key(|(_, delay)| *delay)
        {
            // 只有比当前快 tolerance 以上才切换 (避免频繁切换)
            if *new_delay + self.tolerance < current_delay {
                *self.fastest.write().unwrap() = *new_fastest;
                tracing::info!(
                    "URLTest '{}': switched to '{}' ({}ms -> {}ms)",
                    self.name,
                    self.proxies[*new_fastest].name(),
                    current_delay,
                    new_delay,
                );
            }
        }
    }
}

/// URL 延迟测试: 发送 HTTP HEAD 请求, 测量响应时间
async fn url_test(proxy: &dyn ProxyAdapter, url: &str) -> Result<(), Error> {
    let metadata = Metadata::from_url(url)?;
    let mut stream = proxy.connect_stream(&metadata).await?;

    // 发送 HTTP HEAD 请求
    let request = format!(
        "HEAD {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
        metadata.path(),
        metadata.remote_host(),
    );
    stream.write_all(request.as_bytes()).await?;

    // 读取响应状态行
    let mut buf = vec![0u8; 128];
    stream.read(&mut buf).await?;

    // 检查 HTTP 2xx / 204 响应
    let response = String::from_utf8_lossy(&buf);
    if response.contains("200") || response.contains("204") || response.contains("301") {
        Ok(())
    } else {
        Err(Error::ProxyHandshake(format!("unexpected response: {}", response)))
    }
}
```

## 3. Fallback (故障转移)

按优先级排序，始终使用第一个可用的代理:

```rust
pub struct Fallback {
    name: String,
    proxies: Vec<Arc<dyn ProxyAdapter>>,
    /// 各代理是否存活
    alive: RwLock<Vec<bool>>,
}

#[async_trait]
impl ProxyAdapter for Fallback {
    async fn connect_stream(&self, metadata: &Metadata) -> Result<Box<dyn ProxyStream>, Error> {
        let alive = self.alive.read().unwrap();
        // 选择第一个存活的代理
        for (idx, proxy) in self.proxies.iter().enumerate() {
            if alive[idx] {
                return proxy.connect_stream(metadata).await;
            }
        }
        // 如果全部不可用, 尝试第一个
        self.proxies[0].connect_stream(metadata).await
    }
}
```

## 4. LoadBalance (负载均衡)

将流量分散到多个代理节点:

```rust
use std::hash::{Hash, Hasher};
use std::collections::hash_map::DefaultHasher;

pub enum LoadBalanceStrategy {
    /// 一致性哈希: 相同目标地址总是使用同一代理
    ConsistentHash,
    /// 轮询
    RoundRobin,
}

pub struct LoadBalance {
    name: String,
    proxies: Vec<Arc<dyn ProxyAdapter>>,
    strategy: LoadBalanceStrategy,
    counter: AtomicUsize,
}

#[async_trait]
impl ProxyAdapter for LoadBalance {
    async fn connect_stream(&self, metadata: &Metadata) -> Result<Box<dyn ProxyStream>, Error> {
        let proxy = self.select(metadata);
        proxy.connect_stream(metadata).await
    }
}

impl LoadBalance {
    fn select(&self, metadata: &Metadata) -> &Arc<dyn ProxyAdapter> {
        let alive_proxies: Vec<_> = self.proxies.iter()
            .filter(|p| self.is_alive(p.name()))
            .collect();

        if alive_proxies.is_empty() {
            return &self.proxies[0];
        }

        match self.strategy {
            LoadBalanceStrategy::ConsistentHash => {
                // 基于目标地址哈希选择
                let mut hasher = DefaultHasher::new();
                metadata.remote_host().hash(&mut hasher);
                let idx = (hasher.finish() as usize) % alive_proxies.len();
                alive_proxies[idx]
            }
            LoadBalanceStrategy::RoundRobin => {
                let idx = self.counter.fetch_add(1, Ordering::Relaxed) % alive_proxies.len();
                alive_proxies[idx]
            }
        }
    }
}
```

## 5. Relay (代理链)

串联多个代理节点，实现流量经过多跳转发:

```
Client -> Proxy A -> Proxy B -> Proxy C -> Target
```

```rust
pub struct Relay {
    name: String,
    proxies: Vec<Arc<dyn ProxyAdapter>>,
}

#[async_trait]
impl ProxyAdapter for Relay {
    async fn connect_stream(&self, metadata: &Metadata) -> Result<Box<dyn ProxyStream>, Error> {
        if self.proxies.is_empty() {
            return Err(Error::Config("relay has no proxies".into()));
        }

        // 第一跳: 连接到第一个代理
        let first_metadata = Metadata {
            host: Some(self.proxies[1].addr()?.to_string()),
            dst_ip: Some(self.proxies[1].addr()?.ip()),
            dst_port: self.proxies[1].addr()?.port(),
            ..metadata.clone()
        };
        let mut stream = self.proxies[0].connect_stream(&first_metadata).await?;

        // 中间跳: 通过已建立的连接, 逐个代理握手
        for i in 1..self.proxies.len() - 1 {
            let next_metadata = Metadata {
                host: Some(self.proxies[i + 1].addr()?.to_string()),
                dst_ip: Some(self.proxies[i + 1].addr()?.ip()),
                dst_port: self.proxies[i + 1].addr()?.port(),
                ..metadata.clone()
            };
            // 在已有流上进行下一跳代理握手
            stream = self.proxies[i].connect_through(stream, &next_metadata).await?;
        }

        // 最后一跳: 连接到真正的目标
        let last = self.proxies.last().unwrap();
        stream = last.connect_through(stream, metadata).await?;

        Ok(stream)
    }
}
```

## 代理组嵌套

由于代理组和单个代理实现相同的 `ProxyAdapter` trait，它们可以任意嵌套:

```yaml
proxy-groups:
  - name: "Auto-HK"
    type: url-test
    proxies: [hk-1, hk-2, hk-3]

  - name: "Auto-JP"
    type: url-test
    proxies: [jp-1, jp-2]

  - name: "Select"
    type: select
    proxies: [Auto-HK, Auto-JP, DIRECT]  # 包含其他代理组
```

```
Select (Selector)
  ├── Auto-HK (URLTest)
  │     ├── hk-1 (Shadowsocks)
  │     ├── hk-2 (VMess)
  │     └── hk-3 (Trojan)
  ├── Auto-JP (URLTest)
  │     ├── jp-1 (Shadowsocks)
  │     └── jp-2 (VLESS)
  └── DIRECT
```

## 健康检查机制

```rust
/// 统一的健康检查器
pub struct HealthChecker {
    /// 测试 URL
    url: String,
    /// 检查间隔
    interval: Duration,
    /// 超时时间
    timeout: Duration,
    /// 懒惰模式: 只在最近有流量时才检查
    lazy: bool,
}

impl HealthChecker {
    pub async fn check(&self, proxy: &dyn ProxyAdapter) -> HealthStatus {
        let start = tokio::time::Instant::now();
        let result = tokio::time::timeout(
            self.timeout,
            url_test(proxy, &self.url),
        ).await;

        match result {
            Ok(Ok(_)) => HealthStatus::Alive {
                delay: start.elapsed().as_millis() as u16,
            },
            Ok(Err(e)) => HealthStatus::Dead {
                reason: e.to_string(),
            },
            Err(_) => HealthStatus::Dead {
                reason: "timeout".to_string(),
            },
        }
    }
}

pub enum HealthStatus {
    Alive { delay: u16 },
    Dead { reason: String },
}
```
