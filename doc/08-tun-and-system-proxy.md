# TUN 设备与系统级代理

## 概述

TUN (网络隧道) 是一种虚拟网络设备，工作在网络层 (L3)，可以捕获系统所有 IP 流量。这是实现"全局透明代理"的关键技术。

## TUN 工作原理

```
┌──────────────────────────────────────────────────┐
│                  用户空间                          │
│                                                   │
│  ┌─────────┐     ┌──────────┐     ┌────────────┐ │
│  │ 浏览器   │     │ 聊天软件  │     │ 其他应用   │ │
│  └────┬────┘     └────┬─────┘     └─────┬──────┘ │
│       │               │                 │         │
│       └───────────┬───┘─────────────────┘         │
│                   │                               │
│              ┌────v────────────────────────┐       │
│              │      代理程序               │       │
│              │  读取 TUN fd -> 解析 IP 包  │       │
│              │  -> 规则匹配 -> 代理转发    │       │
│              └────────────────────────────┘       │
├──────────────────│────────────────────────────────┤
│             ┌────v────┐       内核空间             │
│             │ TUN 设备│                            │
│             │ (utun0) │                            │
│             └────┬────┘                            │
│                  │                                 │
│             ┌────v────────────────────────┐        │
│             │     路由表                  │        │
│             │  0.0.0.0/0 -> utun0         │        │
│             │  (所有流量路由到 TUN 设备)   │        │
│             └─────────────────────────────┘        │
└──────────────────────────────────────────────────┘
```

## TUN 设备操作

### 创建 TUN 设备

```rust
// 使用 tun2 crate (跨平台 TUN 库)

use tun2::{Configuration, Device};

fn create_tun_device() -> Result<Device, Error> {
    let mut config = Configuration::default();

    // 设置 TUN 设备参数
    config
        .name("utun100")           // 设备名 (macOS: utunN, Linux: tunN)
        .address("10.0.0.1")       // TUN 设备 IP
        .netmask("255.255.255.0")  // 子网掩码
        .mtu(1500)                 // MTU
        .up();                      // 启用设备

    #[cfg(target_os = "linux")]
    config.platform_config(|p| {
        p.packet_information(false); // 不需要 PI 头
    });

    let device = tun2::create(&config)?;
    Ok(device)
}
```

### 设置路由表

```rust
use std::process::Command;

/// 配置路由, 将所有流量导向 TUN 设备
fn setup_routes(tun_name: &str, gateway: &str) -> Result<(), Error> {
    #[cfg(target_os = "macos")]
    {
        // macOS: 添加默认路由指向 TUN
        Command::new("route")
            .args(["add", "-net", "0.0.0.0/1", "-interface", tun_name])
            .status()?;
        Command::new("route")
            .args(["add", "-net", "128.0.0.0/1", "-interface", tun_name])
            .status()?;
        // 使用 0.0.0.0/1 + 128.0.0.0/1 而不是 0.0.0.0/0
        // 这样优先级比默认路由高, 且不会覆盖默认路由
    }

    #[cfg(target_os = "linux")]
    {
        // Linux: 使用 ip route
        Command::new("ip")
            .args(["route", "add", "0.0.0.0/1", "dev", tun_name])
            .status()?;
        Command::new("ip")
            .args(["route", "add", "128.0.0.0/1", "dev", tun_name])
            .status()?;
    }

    // 关键: 代理服务器的 IP 必须走原来的网关, 否则会循环
    // proxy_server_ip -> 原始网关
    Command::new("route")
        .args(["add", "-host", "PROXY_SERVER_IP", gateway])
        .status()?;

    Ok(())
}
```

## 从 IP 包中提取连接信息

TUN 设备读取到的是原始 IP 数据包，需要解析:

```rust
/// 解析 IPv4 包头
fn parse_ipv4_packet(packet: &[u8]) -> Option<(IpAddr, IpAddr, u8, u16, u16)> {
    if packet.len() < 20 { return None; }

    let version = (packet[0] >> 4) & 0x0F;
    if version != 4 { return None; }

    let ihl = (packet[0] & 0x0F) as usize * 4;
    let protocol = packet[9]; // 6=TCP, 17=UDP
    let src_ip = IpAddr::from([packet[12], packet[13], packet[14], packet[15]]);
    let dst_ip = IpAddr::from([packet[16], packet[17], packet[18], packet[19]]);

    // 传输层端口 (TCP/UDP 的前 4 字节)
    let (src_port, dst_port) = if packet.len() >= ihl + 4 {
        let src = u16::from_be_bytes([packet[ihl], packet[ihl + 1]]);
        let dst = u16::from_be_bytes([packet[ihl + 2], packet[ihl + 3]]);
        (src, dst)
    } else {
        (0, 0)
    };

    Some((src_ip, dst_ip, protocol, src_port, dst_port))
}
```

## TCP 栈选择

TUN 捕获的是 IP 层数据包，但代理工作在传输层。需要一个用户态 TCP 栈将 IP 包转换为 TCP 流:

### 方案 1: System Stack (系统栈)

```
TUN 读取 IP 包 -> 回写到 TUN (让内核处理 TCP)
                -> 在本地监听端口上接收 TCP 连接
```

原理: 将流量用 NAT 转发到本地代理端口，复用内核 TCP 栈。

### 方案 2: gVisor (用户态网络栈)

```
TUN 读取 IP 包 -> gVisor netstack 处理 TCP/UDP
                -> 得到用户态的 TCP 连接 / UDP 包
                -> 直接传给隧道核心
```

使用 `smoltcp` crate 在 Rust 中实现用户态网络栈:

```rust
use smoltcp::iface::{Interface, SocketSet};
use smoltcp::socket::tcp::Socket as TcpSocket;

/// 用户态 TCP 栈
struct UserspaceTcpStack {
    interface: Interface,
    sockets: SocketSet<'static>,
}

impl UserspaceTcpStack {
    /// 处理从 TUN 读取的 IP 包
    fn process_packet(&mut self, packet: &[u8]) {
        // smoltcp 处理 TCP 三次握手、重传、拥塞控制等
        self.interface.process_packet(packet);
    }

    /// 获取已建立的 TCP 连接
    fn accept(&mut self) -> Option<TcpConnection> {
        // 检查是否有新的 TCP 连接完成握手
        for socket in self.sockets.iter() {
            if socket.may_recv() && socket.is_active() {
                return Some(TcpConnection::from(socket));
            }
        }
        None
    }
}
```

### 方案 3: Mixed Stack (混合)

TCP 使用系统栈 (性能好), UDP 使用用户态栈 (需要完整数据报)。

## TUN 与 DNS 的协作

```
┌──────────────────────────────────────────────────────┐
│  应用程序发起 DNS 查询 (目标: 系统 DNS 53 端口)       │
└──────────────┬───────────────────────────────────────┘
               │
               v
┌──────────────────────────┐
│  TUN 设备捕获 DNS 包      │
│  (目标端口 = 53)          │
└──────────────┬───────────┘
               │ DNS 劫持 (dns-hijack)
               v
┌──────────────────────────┐
│  内置 FakeIP DNS 服务器   │
│  返回虚假 IP              │
└──────────────┬───────────┘
               │
               v
┌──────────────────────────┐
│  应用程序收到假 IP         │
│  发起 TCP/UDP 连接        │
└──────────────┬───────────┘
               │
               v
┌──────────────────────────┐
│  TUN 捕获连接             │
│  FakeIP 反查域名          │
│  规则匹配 -> 代理转发     │
└──────────────────────────┘
```

DNS 劫持配置:
```yaml
tun:
  enable: true
  stack: system
  dns-hijack:
    - any:53        # 劫持所有发往 53 端口的 DNS 查询
    - tcp://any:53  # 包括 TCP DNS
```

## 透明代理对比

| 方式 | 原理 | 平台 | 优势 | 劣势 |
|------|------|------|------|------|
| TUN | 虚拟网卡捕获 IP 包 | 全平台 | 捕获所有流量 | 需要 root/admin 权限 |
| Redirect | iptables REDIRECT | Linux | 简单 | 仅 TCP, 丢失原始目标 |
| TProxy | iptables TPROXY | Linux | TCP+UDP, 保留原始目标 | 需要特殊路由规则 |
| HTTP Proxy | 浏览器/系统代理设置 | 全平台 | 不需要特权 | 仅支持 HTTP/HTTPS |
| SOCKS Proxy | 应用设置 | 全平台 | 不需要特权 | 仅限支持 SOCKS 的应用 |

## Linux iptables Redirect 模式

```rust
/// 设置 iptables 重定向规则
fn setup_iptables_redirect(proxy_port: u16) -> Result<(), Error> {
    let commands = vec![
        // 创建自定义链
        "iptables -t nat -N CLASH",
        // 排除本地地址
        "iptables -t nat -A CLASH -d 0.0.0.0/8 -j RETURN",
        "iptables -t nat -A CLASH -d 127.0.0.0/8 -j RETURN",
        "iptables -t nat -A CLASH -d 10.0.0.0/8 -j RETURN",
        "iptables -t nat -A CLASH -d 172.16.0.0/12 -j RETURN",
        "iptables -t nat -A CLASH -d 192.168.0.0/16 -j RETURN",
        // 重定向所有 TCP 到代理端口
        &format!("iptables -t nat -A CLASH -p tcp -j REDIRECT --to-ports {}", proxy_port),
        // 应用到 OUTPUT 链 (本机流量) 和 PREROUTING 链 (网关流量)
        "iptables -t nat -A OUTPUT -p tcp -j CLASH",
        "iptables -t nat -A PREROUTING -p tcp -j CLASH",
    ];

    for cmd in commands {
        let parts: Vec<&str> = cmd.split_whitespace().collect();
        Command::new(parts[0]).args(&parts[1..]).status()?;
    }

    Ok(())
}

/// 通过 getsockopt 获取原始目标地址 (SO_ORIGINAL_DST)
fn get_original_dst(stream: &TcpStream) -> Result<SocketAddr, Error> {
    use std::os::unix::io::AsRawFd;

    let fd = stream.as_raw_fd();
    // SO_ORIGINAL_DST = 80 (Linux)
    let addr = unsafe {
        let mut addr: libc::sockaddr_in = std::mem::zeroed();
        let mut len = std::mem::size_of_val(&addr) as libc::socklen_t;
        libc::getsockopt(
            fd,
            libc::SOL_IP,
            80, // SO_ORIGINAL_DST
            &mut addr as *mut _ as *mut libc::c_void,
            &mut len,
        );
        SocketAddr::from((
            Ipv4Addr::from(u32::from_be(addr.sin_addr.s_addr)),
            u16::from_be(addr.sin_port),
        ))
    };

    Ok(addr)
}
```

## Linux TPROXY 模式

TPROXY 比 Redirect 更强大，支持 UDP 且保留原始目标地址:

```rust
/// 设置 TPROXY 规则
fn setup_tproxy(proxy_port: u16, mark: u32) -> Result<(), Error> {
    let commands = vec![
        // IP 规则: 标记的包使用特殊路由表
        &format!("ip rule add fwmark {} table 100", mark),
        "ip route add local 0.0.0.0/0 dev lo table 100",

        // TPROXY: TCP
        &format!(
            "iptables -t mangle -A PREROUTING -p tcp -j TPROXY --on-port {} --tproxy-mark {}",
            proxy_port, mark
        ),
        // TPROXY: UDP
        &format!(
            "iptables -t mangle -A PREROUTING -p udp -j TPROXY --on-port {} --tproxy-mark {}",
            proxy_port, mark
        ),
    ];

    for cmd in commands {
        execute_command(cmd)?;
    }

    Ok(())
}
```

TPROXY 监听器需要设置 `IP_TRANSPARENT` socket 选项:

```rust
use socket2::{Socket, Domain, Type, Protocol};

fn create_tproxy_listener(addr: SocketAddr) -> Result<TcpListener, Error> {
    let socket = Socket::new(Domain::IPV4, Type::STREAM, Some(Protocol::TCP))?;

    // IP_TRANSPARENT 允许绑定非本地地址
    socket.set_ip_transparent(true)?;

    // SO_REUSEADDR
    socket.set_reuse_address(true)?;

    socket.bind(&addr.into())?;
    socket.listen(1024)?;

    Ok(TcpListener::from_std(socket.into())?)
}
```
