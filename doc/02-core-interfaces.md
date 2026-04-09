# 核心 Trait 设计

## 设计哲学

整个系统的核心思想是 **面向 trait 编程**。所有核心 trait 定义在 `clash-core` crate 中，该 crate 不依赖任何内部 crate，仅依赖标准库和基础类型库。这是保证无循环依赖的关键。

Rust 的 trait 系统提供了两种多态方式:
- **静态分发 (单态化)**: 编译期确定类型，零开销，用于性能关键路径
- **动态分发 (`dyn Trait`)**: 运行时多态，用于需要异构集合的场景 (如代理列表)

本项目中，代理适配器和规则使用 `Box<dyn Trait>` 动态分发，因为需要在运行时根据配置动态创建不同类型的代理和规则。

## 关键 Trait 定义

### 1. ProxyAdapter — 代理适配器 trait

这是系统中最核心的抽象。所有能承载流量的实体都实现此 trait。

```rust
// clash-core/src/adapter.rs

use async_trait::async_trait;
use std::net::SocketAddr;
use crate::{Metadata, ProxyStream, ProxyDatagram, Error};

/// 代理适配器类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AdapterType {
    Direct,
    Reject,
    Shadowsocks,
    VMess,
    VLESS,
    Trojan,
    WireGuard,
    Hysteria2,
    // 代理组类型
    Selector,
    URLTest,
    Fallback,
    LoadBalance,
    Relay,
}

/// 代理适配器核心 trait
/// 所有代理节点和代理组都实现此 trait
#[async_trait]
pub trait ProxyAdapter: Send + Sync {
    /// 代理名称
    fn name(&self) -> &str;

    /// 代理类型
    fn adapter_type(&self) -> AdapterType;

    /// 建立 TCP 连接
    /// metadata 包含目标地址、端口等连接上下文
    async fn connect_stream(
        &self,
        metadata: &Metadata,
    ) -> Result<Box<dyn ProxyStream>, Error>;

    /// 建立 UDP 连接
    async fn connect_datagram(
        &self,
        metadata: &Metadata,
    ) -> Result<Box<dyn ProxyDatagram>, Error>;

    /// 是否支持 UDP
    fn support_udp(&self) -> bool;

    /// 代理服务器地址
    fn addr(&self) -> Option<SocketAddr> {
        None
    }

    /// 解包内部适配器 (用于代理组)
    fn unwrap(&self, _metadata: &Metadata) -> Option<&dyn ProxyAdapter> {
        None
    }
}
```

**为什么使用 `async_trait`?**
Rust 原生 async trait 在 nightly 中可用，但 `async_trait` 宏提供了稳定版支持。它将 async 方法转化为返回 `Pin<Box<dyn Future>>` 的方法，在代理场景中这个 Box 开销可忽略不计。

### 2. ProxyStream / ProxyDatagram — 连接抽象

```rust
// clash-core/src/conn.rs

use tokio::io::{AsyncRead, AsyncWrite};
use std::net::SocketAddr;
use bytes::Bytes;

/// TCP 代理连接
/// 组合 AsyncRead + AsyncWrite，并附加链路追踪信息
pub trait ProxyStream: AsyncRead + AsyncWrite + Unpin + Send + Sync {
    /// 代理链路 (调试用)
    /// 例如: ["client", "ss-server", "remote"]
    fn chains(&self) -> &[String];

    /// 追加链路节点
    fn append_chain(&mut self, name: String);
}

/// UDP 代理数据报
#[async_trait]
pub trait ProxyDatagram: Send + Sync {
    /// 发送 UDP 数据包到指定目标
    async fn send_to(&self, buf: &[u8], target: &SocketAddr) -> Result<usize, Error>;

    /// 接收 UDP 数据包
    async fn recv_from(&self, buf: &mut [u8]) -> Result<(usize, SocketAddr), Error>;

    /// 关闭连接
    async fn close(&self) -> Result<(), Error>;
}

/// 为标准流实现 ProxyStream 的包装器
pub struct TrackedStream<S: AsyncRead + AsyncWrite + Unpin + Send + Sync> {
    inner: S,
    chains: Vec<String>,
    bytes_read: u64,
    bytes_written: u64,
}
```

### 3. Rule — 规则 trait

```rust
// clash-core/src/rule.rs

/// 规则类型枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleType {
    Domain,
    DomainSuffix,
    DomainKeyword,
    DomainRegex,
    IpCidr,
    SrcIpCidr,
    GeoIP,
    GeoSite,
    ProcessName,
    ProcessPath,
    SrcPort,
    DstPort,
    InboundPort,
    And,
    Or,
    Not,
    RuleSet,
    Match,  // 默认规则 (catch-all)
}

/// 规则 trait
/// 每种规则类型实现此 trait
pub trait Rule: Send + Sync {
    /// 规则类型
    fn rule_type(&self) -> RuleType;

    /// 判断元数据是否匹配
    /// 返回 (是否匹配, 目标适配器名称)
    fn matches(&self, metadata: &Metadata) -> bool;

    /// 匹配后使用的适配器名称
    fn adapter(&self) -> &str;

    /// 规则内容 (如域名、IP 段)
    fn payload(&self) -> &str;

    /// 匹配前是否需要先解析域名为 IP
    fn should_resolve_ip(&self) -> bool;

    /// 匹配前是否需要查找进程名
    fn should_find_process(&self) -> bool;
}
```

### 4. Metadata — 连接元数据

```rust
// clash-core/src/metadata.rs

use std::net::{IpAddr, SocketAddr};

/// 网络类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Network {
    Tcp,
    Udp,
}

/// 入站类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InboundType {
    Http,
    HttpConnect,
    Socks4,
    Socks5,
    Redir,
    TProxy,
    Tun,
    Mixed,
    Inner,
}

/// DNS 模式
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DnsMode {
    Normal,
    FakeIP,
    RedirHost,
}

/// 连接元数据
/// 贯穿整个请求生命周期的上下文信息
#[derive(Debug, Clone)]
pub struct Metadata {
    /// 网络类型: TCP / UDP
    pub network: Network,

    /// 入站类型
    pub inbound_type: InboundType,

    /// 源地址
    pub src_addr: SocketAddr,

    /// 目标 IP (可能为空，如仅有域名时)
    pub dst_ip: Option<IpAddr>,

    /// 目标端口
    pub dst_port: u16,

    /// 目标域名 (可能为空，如 TUN 模式只有 IP)
    pub host: Option<String>,

    /// DNS 模式
    pub dns_mode: DnsMode,

    /// 进程路径
    pub process_path: Option<String>,

    /// 进程名
    pub process_name: Option<String>,

    /// 入站监听地址
    pub inbound_addr: Option<SocketAddr>,

    /// 入站名称
    pub inbound_name: Option<String>,

    /// DSCP 标记
    pub dscp: u8,
}

impl Metadata {
    /// 获取目标地址 (优先域名，其次 IP)
    pub fn remote_host(&self) -> String {
        self.host.clone().unwrap_or_else(|| {
            self.dst_ip
                .map(|ip| ip.to_string())
                .unwrap_or_default()
        })
    }

    /// 获取目标 SocketAddr
    pub fn remote_addr(&self) -> Option<SocketAddr> {
        self.dst_ip.map(|ip| SocketAddr::new(ip, self.dst_port))
    }
}
```

### 5. Provider — 数据提供者 trait

```rust
// clash-core/src/provider.rs

use std::sync::Arc;

/// Provider 数据源类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VehicleType {
    File,
    Http,
    Compatible,
}

/// 基础 Provider trait
#[async_trait]
pub trait Provider: Send + Sync {
    fn name(&self) -> &str;
    fn vehicle_type(&self) -> VehicleType;

    /// 首次加载数据
    async fn initialize(&self) -> Result<(), Error>;

    /// 手动更新数据
    async fn update(&self) -> Result<(), Error>;
}

/// 代理节点 Provider
#[async_trait]
pub trait ProxyProvider: Provider {
    /// 获取当前所有代理
    fn proxies(&self) -> Vec<Arc<dyn ProxyAdapter>>;

    /// 执行健康检查
    async fn health_check(&self);
}

/// 规则集 Provider
#[async_trait]
pub trait RuleProvider: Provider {
    /// 用给定元数据匹配规则
    fn matches(&self, metadata: &Metadata) -> bool;

    /// 规则集是否需要 IP
    fn should_resolve_ip(&self) -> bool;
}
```

### 6. DnsResolver — DNS 解析器 trait

```rust
// clash-core/src/dns.rs

/// DNS 解析器 trait
/// 隧道核心通过此 trait 与 DNS 子系统交互
#[async_trait]
pub trait DnsResolver: Send + Sync {
    /// 解析域名为 IPv4
    async fn resolve_v4(&self, host: &str) -> Result<IpAddr, Error>;

    /// 解析域名为 IPv6
    async fn resolve_v6(&self, host: &str) -> Result<IpAddr, Error>;

    /// 解析域名 (v4 优先)
    async fn resolve(&self, host: &str) -> Result<IpAddr, Error>;

    /// FakeIP 反向查找: IP -> 域名
    async fn fake_ip_lookup(&self, ip: IpAddr) -> Option<String>;

    /// 判断 IP 是否为 FakeIP
    fn is_fake_ip(&self, ip: IpAddr) -> bool;
}
```

## Trait 关系图

```
                    ┌──────────────┐
                    │ ProxyAdapter │ <── 所有代理的基础 trait
                    │   (dyn)      │
                    └──────┬───────┘
                           │ impl
          ┌────────────────┼────────────────┐
          v                v                v
    ┌──────────┐    ┌──────────┐    ┌──────────────┐
    │ Direct   │    │ SS       │    │ ProxyGroup   │
    │ Reject   │    │ VMess    │    │ (Selector,   │
    │          │    │ Trojan   │    │  URLTest,    │
    │          │    │ VLESS    │    │  Fallback,   │
    │          │    │ ...      │    │  LoadBalance)│
    └──────────┘    └──────────┘    └──────────────┘
                                         │
                                   内部持有
                                   Vec<Arc<dyn ProxyAdapter>>

    ┌──────────┐
    │   Rule   │ <── 所有规则的基础 trait
    │   (dyn)  │
    └──────┬───┘
           │ impl
    ┌──────┼──────┬──────────┬──────────┐
    v      v      v          v          v
  Domain  CIDR  GeoIP   Process    LogicRule
                                   (AND/OR/NOT)
                                        │
                                   内部持有
                                   Vec<Box<dyn Rule>>
```

## Rust 特有的设计考量

### 所有权与生命周期

```rust
// 代理适配器使用 Arc 共享所有权 (多个代理组可引用同一代理)
type SharedProxy = Arc<dyn ProxyAdapter>;

// 规则列表使用 Vec<Box<dyn Rule>> (单一所有者)
type RuleList = Vec<Box<dyn Rule>>;

// 配置热重载使用 ArcSwap (原子指针交换)
use arc_swap::ArcSwap;
type HotReloadConfig = ArcSwap<RuntimeConfig>;
```

### 错误处理

```rust
// clash-core/src/error.rs

use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("DNS resolution failed: {0}")]
    DnsResolution(String),

    #[error("Proxy handshake failed: {0}")]
    ProxyHandshake(String),

    #[error("Rule match error: {0}")]
    RuleMatch(String),

    #[error("Config error: {0}")]
    Config(String),

    #[error("Connection closed")]
    ConnectionClosed,

    #[error("Timeout")]
    Timeout,

    #[error("{0}")]
    Other(#[from] anyhow::Error),
}
```

### 异步运行时选择

使用 **tokio** 作为异步运行时:
- `tokio::net::TcpStream` / `UdpSocket` 作为底层 IO
- `tokio::io::copy_bidirectional` 实现零拷贝双向转发
- `tokio::select!` 处理多路复用
- `tokio::sync::RwLock` 保护共享状态
- 可选启用 `io_uring` 后端获取极致性能 (Linux 5.6+)
