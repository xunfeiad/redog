# 数据流详解

## TCP 连接处理流程

```
 客户端应用
     │
     v
 ┌──────────┐   Accept + 解析协议头
 │ Listener  │   (HTTP CONNECT / SOCKS5 握手 / TUN 数据包 / redir)
 │ (入站)    │   提取: 目标主机/IP, 目标端口, 源 IP, 源端口
 └────┬─────┘
      │  TcpStream + Metadata{host:"example.com", dst_port:443, Tcp, ...}
      v
 ┌──────────────────────────────────────────────┐
 │                隧道核心 (Tunnel)               │
 │                                               │
 │  1. resolve_metadata()                        │
 │     ├─ FakeIP 模式: 从 IP 反查真实域名         │
 │     ├─ 需要时: DNS 解析域名 -> IP              │
 │     └─ 填充 GeoIP 等字段                      │
 │                                               │
 │  2. sniff() [可选]                            │
 │     ├─ peek 连接的前几个字节                   │
 │     ├─ 提取 TLS SNI 或 HTTP Host              │
 │     └─ 更新 metadata.host                     │
 │                                               │
 │  3. rule_match(metadata) -> (rule, adapter)   │
 │     ├─ for rule in rules.iter():              │
 │     │    if rule.matches(&metadata):          │
 │     │      return (rule, rule.adapter())      │
 │     └─ 最终落到 Match (默认规则)               │
 │                                               │
 │  4. 解析 adapter_name -> Arc<dyn ProxyAdapter>│
 │     ├─ 代理 HashMap 直接查找                   │
 │     └─ 或代理组内部选择                        │
 │                                               │
 │  5. adapter.connect_stream(&metadata).await   │
 │     └─ 返回 Box<dyn ProxyStream>              │
 │                                               │
 │  6. relay(client_stream, remote_stream).await  │
 │     └─ tokio::io::copy_bidirectional           │
 └──────────────────────────────────────────────┘
```

### 详细步骤说明

#### 步骤 1: 元数据解析 (resolve_metadata)

当连接从入站监听器到达隧道核心时，元数据可能不完整:

- **HTTP/SOCKS5**: 已有完整的目标域名和端口
- **TUN 模式**: 只有目标 IP (来自 IP 包头)，没有域名
- **Redirect/TProxy**: 只有原始目标 IP

```rust
async fn resolve_metadata(
    metadata: &mut Metadata,
    dns: &dyn DnsResolver,
    fakeip_pool: &FakeIpPool,
) -> Result<(), Error> {
    // 1. FakeIP 反向查找
    if let Some(dst_ip) = metadata.dst_ip {
        if fakeip_pool.contains(dst_ip) {
            if let Some(host) = dns.fake_ip_lookup(dst_ip).await {
                metadata.host = Some(host);
            }
        }
    }

    // 2. 如果规则需要 IP 但只有域名, 进行 DNS 解析
    if metadata.dst_ip.is_none() {
        if let Some(ref host) = metadata.host {
            let ip = dns.resolve(host).await?;
            metadata.dst_ip = Some(ip);
        }
    }

    Ok(())
}
```

#### 步骤 2: 协议嗅探 (Sniffer)

对于 TUN 模式等只有 IP 没有域名的流量，嗅探器通过窥探连接的前几个字节来还原域名:

```
TLS 流量: 解析 ClientHello 的 SNI (Server Name Indication) 扩展
HTTP 流量: 解析 Host 请求头
QUIC 流量: 解析 Initial 包中的 SNI
```

使用 tokio 的 `AsyncReadExt::peek` 或 `BufReader` 实现非消费性读取:

```rust
use tokio::io::{AsyncReadExt, BufReader};

/// 从 TLS ClientHello 中提取 SNI
async fn sniff_tls<S: AsyncRead + Unpin>(stream: &mut BufReader<S>) -> Option<String> {
    let mut header = [0u8; 5];
    stream.peek(&mut header).await.ok()?;

    // 检查 TLS Handshake 标记
    if header[0] != 0x16 {
        return None; // 不是 TLS
    }

    let length = ((header[3] as usize) << 8) | (header[4] as usize);
    let mut client_hello = vec![0u8; 5 + length];
    stream.peek(&mut client_hello).await.ok()?;

    // 解析 SNI 扩展
    parse_sni(&client_hello[5..])
}

/// 解析 TLS ClientHello 中的 SNI
fn parse_sni(data: &[u8]) -> Option<String> {
    // Handshake type = ClientHello (0x01)
    if data.first()? != &0x01 {
        return None;
    }
    // ... 按 TLS 规范逐字段解析
    // 跳过 version, random, session_id, cipher_suites, compression
    // 找到 extensions, 遍历找 SNI extension (type = 0x0000)
    // 提取 HostName
    todo!("TLS SNI 解析实现")
}
```

#### 步骤 3: 规则匹配 (rule_match)

规则按照配置文件中的声明顺序逐一匹配，第一个命中的规则生效:

```rust
async fn rule_match(
    metadata: &mut Metadata,
    rules: &[Box<dyn Rule>],
    dns: &dyn DnsResolver,
    proxies: &HashMap<String, Arc<dyn ProxyAdapter>>,
) -> (Option<&dyn Rule>, Arc<dyn ProxyAdapter>) {
    for rule in rules.iter() {
        // 部分规则需要先解析 IP
        if rule.should_resolve_ip() && metadata.host.is_some() && metadata.dst_ip.is_none() {
            if let Some(ref host) = metadata.host {
                if let Ok(ip) = dns.resolve(host).await {
                    metadata.dst_ip = Some(ip);
                }
            }
        }

        // 部分规则需要进程信息
        if rule.should_find_process() && metadata.process_name.is_none() {
            metadata.process_name = find_process(&metadata.src_addr);
        }

        if rule.matches(metadata) {
            let adapter_name = rule.adapter();
            if let Some(proxy) = proxies.get(adapter_name) {
                return (Some(rule.as_ref()), proxy.clone());
            }
        }
    }

    // 默认使用 DIRECT
    (None, proxies.get("DIRECT").unwrap().clone())
}
```

#### 步骤 6: 双向转发 (relay)

```rust
use tokio::io::copy_bidirectional;

/// TCP 双向转发
/// tokio 内部会利用 splice(2) (Linux) 实现零拷贝
async fn relay(
    mut client: Box<dyn ProxyStream>,
    mut remote: Box<dyn ProxyStream>,
) -> Result<(u64, u64), Error> {
    let (bytes_tx, bytes_rx) = copy_bidirectional(&mut client, &mut remote).await?;

    tracing::debug!(
        "relay completed: tx={} bytes, rx={} bytes",
        bytes_tx, bytes_rx
    );

    Ok((bytes_tx, bytes_rx))
}
```

在 Linux 上，tokio 底层可以使用 `splice(2)` 系统调用进行内核态零拷贝转发，避免数据在用户态和内核态之间来回复制。

---

## UDP 数据包处理流程

```
 客户端应用
     │  UDP 数据包
     v
 ┌──────────┐
 │ Listener  │   接收数据包, 提取元数据
 └────┬─────┘
      │  (Bytes, Metadata)
      v
 ┌──────────────────────────────────────────────┐
 │  隧道核心 (UDP Task Pool)                     │
 │                                               │
 │  1. hash(session_key) -> worker               │
 │     (相同会话的包分配到同一 tokio task,        │
 │      保证包序)                                │
 │                                               │
 │  2. NAT 表查找: session_key -> session         │
 │     ├─ 存在: 直接在已有会话上转发              │
 │     └─ 不存在:                                │
 │        a. resolve_metadata + 匹配规则          │
 │        b. adapter.connect_datagram().await      │
 │        c. 存入 NAT 表 (DashMap)               │
 │        d. spawn 反向转发 task                  │
 │        e. 转发数据包                           │
 │                                               │
 │  3. 会话超时 -> 清理 NAT 表项                  │
 └──────────────────────────────────────────────┘
```

### UDP NAT 表原理

UDP 是无连接协议，但代理需要维护会话状态:

```rust
use dashmap::DashMap;
use std::sync::Arc;
use tokio::time::{Duration, Instant};

/// UDP 会话
struct UdpSession {
    /// 出站 UDP 连接
    datagram: Arc<dyn ProxyDatagram>,
    /// 最后活跃时间
    last_active: Instant,
    /// 关联元数据
    metadata: Metadata,
}

/// NAT 表 (使用 DashMap 实现无锁并发)
struct NatTable {
    table: DashMap<String, UdpSession>,
    timeout: Duration,  // 默认 120 秒
}

impl NatTable {
    /// 查找或创建会话
    async fn get_or_create(
        &self,
        key: &str,
        create_fn: impl Future<Output = Result<UdpSession, Error>>,
    ) -> Result<Arc<dyn ProxyDatagram>, Error> {
        if let Some(mut session) = self.table.get_mut(key) {
            session.last_active = Instant::now();
            return Ok(session.datagram.clone());
        }

        let session = create_fn.await?;
        let datagram = session.datagram.clone();
        self.table.insert(key.to_string(), session);
        Ok(datagram)
    }

    /// 定期清理超时会话 (tokio::spawn 后台任务)
    async fn cleanup_loop(&self) {
        let mut interval = tokio::time::interval(Duration::from_secs(30));
        loop {
            interval.tick().await;
            self.table.retain(|_, session| {
                session.last_active.elapsed() < self.timeout
            });
        }
    }
}
```

### Worker Pool — tokio task 分片

```rust
use tokio::sync::mpsc;

const NUM_WORKERS: usize = 4;

struct UdpDispatcher {
    senders: Vec<mpsc::Sender<UdpPacket>>,
}

impl UdpDispatcher {
    fn new(handler: Arc<TunnelHandler>) -> Self {
        let mut senders = Vec::with_capacity(NUM_WORKERS);
        for _ in 0..NUM_WORKERS {
            let (tx, mut rx) = mpsc::channel::<UdpPacket>(1024);
            let handler = handler.clone();
            tokio::spawn(async move {
                while let Some(packet) = rx.recv().await {
                    handler.handle_udp_packet(packet).await;
                }
            });
            senders.push(tx);
        }
        Self { senders }
    }

    /// 按会话 key hash 分发到对应 worker
    async fn dispatch(&self, packet: UdpPacket) {
        let idx = fxhash::hash(&packet.session_key()) as usize % NUM_WORKERS;
        let _ = self.senders[idx].send(packet).await;
    }
}
```

---

## DNS 解析流程

```
 应用程序 DNS 查询
     │
     v
 ┌───────────────┐
 │  DNS Server    │  (内置, 监听配置端口)
 └───────┬───────┘
         v
 ┌───────────────┐
 │  Middleware    │  缓存 / 速率限制 / EDNS0 Client Subnet
 └───────┬───────┘
         v
 ┌───────────────┐     ┌──────────────────┐
 │ Policy Router │────>│ Nameserver-policy │ 域名 -> 特定上游
 └───────┬───────┘     └──────────────────┘
         v
 ┌───────────────┐
 │   Resolver    │  尝试主 nameserver, 失败时用 fallback
 └───────┬───────┘
         v
 ┌───────────────┐
 │   Enhancer    │  FakeIP: 分配虚拟 IP, 存储映射
 │   (FakeIP)    │  Redir-Host: 存储真实 IP -> 域名映射
 └───────────────┘
```

### DNS 解析器 Fallback 机制

```rust
impl Resolver {
    async fn exchange(&self, query: &DnsQuery) -> Result<DnsResponse, Error> {
        let domain = &query.name;

        // 1. 策略路由: 特定域名使用特定上游
        if let Some(ns) = self.policy_match(domain) {
            return ns.exchange(query).await;
        }

        // 2. 并发查询主 nameserver (tokio::select! 取最快响应)
        let resp = self.exchange_fastest(&self.main_servers, query).await;

        // 3. 检查是否需要 fallback
        //    - 解析失败
        //    - 返回的 IP 可能被污染 (通过 GeoIP 判断)
        match resp {
            Ok(ref r) if self.need_fallback(r) => {
                self.exchange_fastest(&self.fallback_servers, query).await
            }
            Err(_) => {
                self.exchange_fastest(&self.fallback_servers, query).await
            }
            ok => ok,
        }
    }

    /// 并发查询多个上游, 返回最快的有效响应
    async fn exchange_fastest(
        &self,
        servers: &[Arc<dyn DnsClient>],
        query: &DnsQuery,
    ) -> Result<DnsResponse, Error> {
        use futures::future::select_ok;
        let futures: Vec<_> = servers
            .iter()
            .map(|s| Box::pin(s.exchange(query)))
            .collect();
        let (resp, _remaining) = select_ok(futures).await?;
        Ok(resp)
    }
}
```

### FakeIP 工作原理

详见 [05-fakeip-principle.md](05-fakeip-principle.md)
