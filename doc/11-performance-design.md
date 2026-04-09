# 性能设计与优化

## 关键性能指标

| 指标 | 目标 |
|------|------|
| 单连接吞吐量 | 接近线速 (>1Gbps on 10G NIC) |
| 并发连接数 | >100,000 |
| 规则匹配延迟 | <10μs per connection |
| 内存占用 | <50MB (10K 连接) |
| CPU 开销 | <5% (空闲时), <30% (高负载) |

## 1. 异步运行时: Tokio

```rust
#[tokio::main(flavor = "multi_thread")]
async fn main() {
    // tokio 多线程运行时
    // 默认线程数 = CPU 核心数
    // 每个线程运行一个事件循环
    // 使用 work-stealing 调度器平衡负载
}
```

### 为什么用 Tokio 而不是 `async-std` 或 `smol`?

- **生态最完善**: 几乎所有网络库都基于 tokio
- **io_uring 支持**: 通过 `tokio-uring` 可选启用
- **成熟的调度器**: work-stealing + LIFO slot 优化

### Tokio 调度模型

```
┌─────────────────────────────────┐
│         Tokio Runtime           │
│                                 │
│  Thread 1    Thread 2    ...    │
│  ┌───────┐  ┌───────┐          │
│  │ epoll │  │ epoll │          │
│  │ loop  │  │ loop  │          │
│  └───┬───┘  └───┬───┘          │
│      │          │               │
│  ┌───v───┐  ┌───v───┐          │
│  │ Local │  │ Local │          │
│  │ Queue │  │ Queue │          │
│  └───┬───┘  └───┬───┘          │
│      │    steal  │               │
│      └─────>────┘               │
│                                 │
│  ┌─────────────────────────┐    │
│  │     Global Queue        │    │
│  └─────────────────────────┘    │
└─────────────────────────────────┘
```

## 2. 零拷贝转发

### TCP Relay

```rust
use tokio::io::copy_bidirectional;

/// tokio::io::copy_bidirectional 内部流程:
///
/// Linux:
///   1. 尝试 splice(2): 内核态直接在两个 fd 之间传输数据
///      数据不经过用户空间, 零拷贝
///   2. Fallback: read() + write() 使用 8KB 内核缓冲区
///
/// macOS:
///   直接使用 read() + write(), 但 tokio 会复用缓冲区
async fn relay_tcp(
    mut left: impl AsyncRead + AsyncWrite + Unpin,
    mut right: impl AsyncRead + AsyncWrite + Unpin,
) -> Result<(u64, u64), Error> {
    copy_bidirectional(&mut left, &mut right).await
        .map_err(Into::into)
}
```

### 零拷贝原理 (Linux splice)

```
                    传统拷贝:
                    App read() -> 内核缓冲区 -> 用户缓冲区
                                                    ↓
                    App write() <- 内核缓冲区 <- 用户缓冲区

                    splice 零拷贝:
                    内核 socket A 缓冲区 ──pipe──> 内核 socket B 缓冲区
                    (数据始终在内核空间, 不经过用户空间)
```

## 3. 缓冲区管理

### Bytes + BytesMut (零拷贝缓冲区)

```rust
use bytes::{Bytes, BytesMut, Buf, BufMut};

/// Bytes 使用引用计数, clone 是 O(1) (不复制数据)
/// 适合在多个 task 间共享数据

/// 代理协议帧解码器
struct FrameDecoder {
    buf: BytesMut,
}

impl FrameDecoder {
    /// 从 stream 读取并解码帧
    async fn read_frame<S: AsyncRead + Unpin>(
        &mut self,
        stream: &mut S,
    ) -> Result<Bytes, Error> {
        // 确保缓冲区有足够空间
        if self.buf.remaining() < 4096 {
            self.buf.reserve(8192);
        }

        // 读取数据到缓冲区
        let n = stream.read_buf(&mut self.buf).await?;
        if n == 0 { return Err(Error::ConnectionClosed); }

        // 解析帧长度
        let len = u16::from_be_bytes([self.buf[0], self.buf[1]]) as usize;

        // 切分出帧数据 (零拷贝)
        let _ = self.buf.split_to(2); // 跳过长度
        let frame = self.buf.split_to(len).freeze(); // 零拷贝转换为 Bytes

        Ok(frame)
    }
}
```

### 全局缓冲区池

```rust
use std::sync::OnceLock;

/// 缓冲区大小常量
const RELAY_BUF_SIZE: usize = 32 * 1024; // 32KB
const SMALL_BUF_SIZE: usize = 4 * 1024;  // 4KB

/// 使用 thread_local 缓冲区池避免频繁分配
thread_local! {
    static RELAY_BUF: std::cell::RefCell<Vec<u8>> =
        std::cell::RefCell::new(vec![0u8; RELAY_BUF_SIZE]);
}

/// 或使用 object pool crate
use object_pool::Pool;

static BUF_POOL: OnceLock<Pool<Vec<u8>>> = OnceLock::new();

fn get_pool() -> &'static Pool<Vec<u8>> {
    BUF_POOL.get_or_init(|| {
        Pool::new(128, || vec![0u8; RELAY_BUF_SIZE])
    })
}
```

## 4. 高性能数据结构

### DashMap (无锁并发 HashMap)

```rust
use dashmap::DashMap;

/// 代理映射表 (高并发读, 低频写)
let proxies: DashMap<String, Arc<dyn ProxyAdapter>> = DashMap::new();

/// NAT 表 (高频读写)
let nat_table: DashMap<String, UdpSession> = DashMap::with_capacity(4096);

// DashMap 内部使用分片 + 读写锁
// 读操作几乎无等待
// 写操作只锁定对应分片
```

### Domain Trie 性能

```
基准测试 (100,000 域名规则):

操作                  | 朴素线性扫描 | Trie 树
--------------------|------------|--------
查找 google.com      | ~100μs     | ~0.1μs
查找 a.b.c.d.e.f.com | ~100μs     | ~0.2μs
内存占用              | ~10MB      | ~15MB
```

### CIDR 前缀树性能

```
基准测试 (10,000 CIDR 规则):

操作                  | 朴素线性扫描 | 前缀树
--------------------|------------|--------
IPv4 查找            | ~10μs      | ~0.05μs
IPv6 查找            | ~10μs      | ~0.15μs
```

## 5. 连接池

```rust
use std::collections::VecDeque;
use tokio::sync::Mutex;

/// 到代理服务器的连接池
/// 复用已建立的 TCP 连接, 避免重复握手
pub struct ConnectionPool {
    pools: DashMap<String, Mutex<VecDeque<PooledConnection>>>,
    max_idle: usize,
    max_idle_timeout: Duration,
}

struct PooledConnection {
    stream: Box<dyn ProxyStream>,
    created_at: Instant,
    last_used: Instant,
}

impl ConnectionPool {
    /// 获取或创建连接
    pub async fn get_or_create(
        &self,
        key: &str,
        create: impl Future<Output = Result<Box<dyn ProxyStream>, Error>>,
    ) -> Result<Box<dyn ProxyStream>, Error> {
        // 1. 尝试从池中获取
        if let Some(pool) = self.pools.get(key) {
            let mut queue = pool.lock().await;
            while let Some(conn) = queue.pop_front() {
                // 检查连接是否过期
                if conn.last_used.elapsed() < self.max_idle_timeout {
                    return Ok(conn.stream);
                }
                // 过期连接直接丢弃
            }
        }

        // 2. 创建新连接
        create.await
    }

    /// 归还连接到池
    pub async fn put(&self, key: &str, stream: Box<dyn ProxyStream>) {
        let pool = self.pools
            .entry(key.to_string())
            .or_insert_with(|| Mutex::new(VecDeque::new()));
        let mut queue = pool.lock().await;
        if queue.len() < self.max_idle {
            queue.push_back(PooledConnection {
                stream,
                created_at: Instant::now(),
                last_used: Instant::now(),
            });
        }
    }
}
```

## 6. io_uring 支持 (Linux 5.6+)

```rust
/// 可选启用 io_uring 后端获取极致性能
/// 通过 feature flag 控制

#[cfg(feature = "io-uring")]
use tokio_uring::net::TcpStream as UringTcpStream;

// io_uring 优势:
// 1. 系统调用批量提交 (减少 context switch)
// 2. 异步 I/O 真正零拷贝 (注册固定缓冲区)
// 3. 减少 epoll 的惊群效应
//
// io_uring 局限:
// 1. 仅 Linux 5.6+
// 2. 需要特殊的 buffer 管理 (固定缓冲区)
// 3. 生态支持不如 epoll 成熟
```

## 7. 性能监控

```rust
/// 内建性能指标收集
pub struct Metrics {
    /// 活跃连接数
    pub active_connections: AtomicU64,
    /// 总连接数
    pub total_connections: AtomicU64,
    /// 规则匹配平均耗时 (纳秒)
    pub rule_match_latency_ns: AtomicU64,
    /// DNS 解析平均耗时 (微秒)
    pub dns_resolve_latency_us: AtomicU64,
    /// 代理握手平均耗时 (毫秒)
    pub proxy_handshake_latency_ms: AtomicU64,
}

impl Metrics {
    /// 记录规则匹配耗时
    pub fn record_rule_match(&self, start: Instant) {
        let elapsed = start.elapsed().as_nanos() as u64;
        // 指数移动平均
        let current = self.rule_match_latency_ns.load(Ordering::Relaxed);
        let new_avg = (current * 7 + elapsed) / 8;
        self.rule_match_latency_ns.store(new_avg, Ordering::Relaxed);
    }
}
```

## 8. 平台特定优化

| 平台 | 优化点 |
|------|--------|
| Linux | `splice(2)` 零拷贝, `SO_REUSEPORT`, `TCP_FASTOPEN`, `io_uring` |
| macOS | `kqueue`, `connectx()` TFO |
| Windows | IOCP, Named Pipe |
| 通用 | `TCP_NODELAY`, `SO_KEEPALIVE`, buffer tuning |

```rust
use socket2::Socket;

fn optimize_socket(socket: &Socket) {
    // TCP 无延迟 (禁用 Nagle 算法)
    socket.set_nodelay(true).ok();

    // Keep-Alive
    socket.set_keepalive(true).ok();

    // 增大缓冲区
    socket.set_recv_buffer_size(256 * 1024).ok();
    socket.set_send_buffer_size(256 * 1024).ok();

    #[cfg(target_os = "linux")]
    {
        // TCP Fast Open
        socket.set_tcp_fastopen(5).ok();

        // SO_REUSEPORT (多线程监听同一端口)
        socket.set_reuse_port(true).ok();
    }
}
```
