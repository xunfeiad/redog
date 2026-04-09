# FakeIP 原理详解

## 为什么需要 FakeIP?

### 问题: TUN 模式下的域名丢失

在 TUN (虚拟网卡) 模式下，系统会捕获所有 IP 层数据包。但 IP 包头中只有目标 IP 地址，**没有域名信息**。

正常流程:
```
应用程序 -> DNS 查询 "google.com" -> 得到 142.250.80.46
应用程序 -> 连接 142.250.80.46:443
TUN 捕获 -> 只看到 IP=142.250.80.46, 无法知道这是 google.com
```

这导致**基于域名的规则无法工作**。例如 `DOMAIN-SUFFIX,google.com,Proxy` 无法匹配。

### 问题: DNS 污染

在某些网络环境下，DNS 查询可能被劫持返回错误的 IP 地址:
```
DNS 查询 "google.com" -> 被劫持返回 203.0.113.1 (错误地址)
应用程序连接 203.0.113.1 -> 连接失败或被重定向
```

### FakeIP 的解决方案

FakeIP 拦截 DNS 查询，返回一个虚假的 IP 地址 (从私有地址段分配)，并记录映射关系:

```
应用程序 -> DNS 查询 "google.com"
FakeIP DNS -> 返回 198.18.0.1 (假 IP)
             同时记录: 198.18.0.1 -> google.com

应用程序 -> 连接 198.18.0.1:443
TUN 捕获 -> IP=198.18.0.1
           -> FakeIP 反查: 198.18.0.1 -> google.com
           -> 现在知道域名了! 可以匹配域名规则
           -> 规则匹配后, 代理直接用 google.com 连接远端
              (远端服务器负责真正的 DNS 解析)
```

## 架构图

```
                    ┌─────────────────┐
                    │   应用程序       │
                    │ (浏览器等)       │
                    └──────┬──────────┘
                           │
              ┌────────────┼────────────┐
              │ DNS 查询    │ TCP/UDP    │
              v            │ 连接       │
    ┌─────────────────┐    │            │
    │  FakeIP DNS     │    │            │
    │  Server         │    │            │
    │                 │    │            │
    │  1. 拦截 DNS    │    │            │
    │  2. 分配假 IP   │    │            │
    │  3. 记录映射    │    │            │
    │  4. 返回假 IP   │    │            │
    └────────┬────────┘    │            │
             │             v            │
             │   ┌─────────────────┐    │
             │   │   TUN 设备      │    │
             │   │  (捕获 IP 包)   │    │
             │   └────────┬────────┘    │
             │            │             │
             v            v             │
    ┌───────────────────────────────────┴──┐
    │          隧道核心 (Tunnel)            │
    │                                      │
    │  1. 从 IP 包提取 dst_ip=198.18.0.1  │
    │  2. FakeIP 反查: 198.18.0.1          │
    │     -> host = "google.com"           │
    │  3. 用 host 进行规则匹配             │
    │  4. 选择代理, 用域名连接远端         │
    └──────────────────────────────────────┘
```

## 核心实现

### FakeIP 地址池

```rust
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr};
use std::sync::Mutex;

/// FakeIP 地址池
/// 从私有地址段 (如 198.18.0.0/15) 分配虚假 IP
pub struct FakeIpPool {
    /// IP -> 域名 映射
    ip_to_host: Mutex<HashMap<Ipv4Addr, String>>,
    /// 域名 -> IP 映射
    host_to_ip: Mutex<HashMap<String, Ipv4Addr>>,
    /// 地址池范围
    network: Ipv4Addr,    // 198.18.0.0
    mask: u32,            // /15 = 0xFFFE0000
    /// 当前分配偏移
    offset: Mutex<u32>,
    /// 池大小
    pool_size: u32,       // 2^17 = 131072 个地址
}

impl FakeIpPool {
    /// 为域名分配 FakeIP (或返回已分配的)
    pub fn lookup(&self, host: &str) -> Ipv4Addr {
        // 已有映射则直接返回
        if let Some(&ip) = self.host_to_ip.lock().unwrap().get(host) {
            return ip;
        }

        // 分配新 IP
        let ip = self.allocate();

        // 如果池满了, 新分配会覆盖最早的映射 (LRU 淘汰)
        self.ip_to_host.lock().unwrap().insert(ip, host.to_string());
        self.host_to_ip.lock().unwrap().insert(host.to_string(), ip);

        ip
    }

    /// 分配下一个 IP (环形缓冲区)
    fn allocate(&self) -> Ipv4Addr {
        let mut offset = self.offset.lock().unwrap();
        let ip_num = u32::from(self.network) + (*offset % self.pool_size) + 1;
        *offset = offset.wrapping_add(1);
        Ipv4Addr::from(ip_num)
    }

    /// 反向查找: IP -> 域名
    pub fn reverse_lookup(&self, ip: Ipv4Addr) -> Option<String> {
        self.ip_to_host.lock().unwrap().get(&ip).cloned()
    }

    /// 判断 IP 是否在 FakeIP 范围内
    pub fn contains(&self, ip: IpAddr) -> bool {
        match ip {
            IpAddr::V4(v4) => {
                let ip_num = u32::from(v4);
                let net_num = u32::from(self.network);
                (ip_num & self.mask) == (net_num & self.mask)
            }
            IpAddr::V6(_) => false, // FakeIP 通常只用 IPv4
        }
    }
}
```

### 使用 LRU 缓存优化

实际使用中应该用 LRU 缓存替代简单 HashMap，避免内存无限增长:

```rust
use lru::LruCache;
use std::num::NonZeroUsize;

pub struct FakeIpPool {
    ip_to_host: Mutex<LruCache<Ipv4Addr, String>>,
    host_to_ip: Mutex<LruCache<String, Ipv4Addr>>,
    // ... 其他字段
}

impl FakeIpPool {
    pub fn new(cidr: &str, size: usize) -> Self {
        Self {
            ip_to_host: Mutex::new(LruCache::new(NonZeroUsize::new(size).unwrap())),
            host_to_ip: Mutex::new(LruCache::new(NonZeroUsize::new(size).unwrap())),
            // ...
        }
    }
}
```

### 持久化

FakeIP 映射可以持久化到磁盘，避免重启后所有映射丢失:

```rust
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
struct FakeIpState {
    mappings: Vec<(Ipv4Addr, String)>,
    offset: u32,
}

impl FakeIpPool {
    /// 保存状态到文件
    pub fn save(&self, path: &str) -> Result<(), Error> {
        let state = FakeIpState {
            mappings: self.ip_to_host.lock().unwrap()
                .iter()
                .map(|(&ip, host)| (ip, host.clone()))
                .collect(),
            offset: *self.offset.lock().unwrap(),
        };
        let data = serde_json::to_vec(&state)?;
        std::fs::write(path, data)?;
        Ok(())
    }

    /// 从文件恢复状态
    pub fn load(path: &str) -> Result<Self, Error> {
        let data = std::fs::read(path)?;
        let state: FakeIpState = serde_json::from_slice(&data)?;
        // ... 重建映射
        todo!()
    }
}
```

## FakeIP 与规则匹配的交互

```
配置示例:
  rules:
    - DOMAIN-SUFFIX,google.com,Proxy   # 域名规则
    - IP-CIDR,10.0.0.0/8,DIRECT       # IP 规则
    - GEOIP,CN,DIRECT                  # GeoIP 规则
    - MATCH,Proxy                      # 默认

场景: 浏览器访问 google.com

1. DNS 查询被 FakeIP 拦截:
   google.com -> 198.18.0.1 (假 IP)

2. 浏览器连接 198.18.0.1:443

3. TUN 捕获, 进入隧道:
   metadata = { dst_ip: 198.18.0.1, dst_port: 443, host: None }

4. resolve_metadata:
   - 检测到 198.18.0.1 在 FakeIP 范围
   - 反查: 198.18.0.1 -> "google.com"
   - metadata.host = Some("google.com")

5. 规则匹配:
   - DOMAIN-SUFFIX "google.com" -> 匹配! -> Proxy
   (如果是 Redir-Host 模式, 这里可能先解析真实 IP 再匹配 IP 规则)

6. 通过 Proxy 连接远端:
   - 代理服务器收到域名 "google.com"
   - 由代理服务器进行真正的 DNS 解析
   - 避免了本地 DNS 污染的影响
```

## FakeIP vs Redir-Host 对比

| 特性 | FakeIP | Redir-Host |
|------|--------|------------|
| DNS 解析位置 | 远端 (代理服务器) | 本地 + 远端 |
| 首次连接延迟 | 低 (无需真实 DNS) | 高 (需要等 DNS 响应) |
| 域名规则支持 | 完美 | 完美 |
| IP 规则支持 | 需额外处理 (FakeIP 不是真实 IP) | 直接支持 |
| DNS 污染抵抗 | 完全免疫 | 依赖 fallback 机制 |
| 兼容性 | 部分应用可能异常 (如局域网发现) | 更好 |
| 实现复杂度 | 较高 | 较低 |

## FakeIP 过滤列表

某些域名不应使用 FakeIP (如局域网服务发现、NTP 等):

```yaml
dns:
  fake-ip-filter:
    - "*.local"           # mDNS 本地发现
    - "*.lan"             # 局域网
    - "time.*.com"        # NTP 服务
    - "ntp.*.com"
    - "+.stun.*.*"        # STUN/TURN (WebRTC)
    - "*.msftconnecttest.com"  # Windows 网络检测
    - "*.msftncsi.com"
```

这些域名的 DNS 查询会直接使用真实解析而不走 FakeIP。
