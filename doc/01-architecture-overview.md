# 架构总览

## 项目简介

本项目是一个跨平台、基于规则的网络隧道代理工具，类似于 Clash/Mihomo，使用 **Rust** 语言实现。利用 Rust 的零成本抽象、内存安全和 async/await 异步模型，实现高性能、高并发的代理服务。

## 核心架构图

```
┌─────────────────────────────────────────────────────────────────────┐
│                         管理层 (Management)                         │
│                   RESTful API  /  Web Dashboard                     │
├─────────────────────────────────────────────────────────────────────┤
│                         配置层 (Configuration)                      │
│              YAML/TOML Parser  /  热重载  /  Providers              │
├──────────┬──────────────────────────────────────────┬───────────────┤
│          │            隧道核心 (Tunnel Core)         │               │
│  入站    │  ┌────────────┐    ┌───────────────┐     │  出站         │
│  监听层  │  │  元数据     │───>│  规则引擎      │    │  适配层       │
│ Inbound  │->│  解析器     │    │  (匹配循环)    │--->│ Outbound     │
│          │  └────────────┘    └───────────────┘     │               │
│ - HTTP   │  ┌────────────┐    ┌───────────────┐     │ - Direct      │
│ - SOCKS5 │  │  协议嗅探   │    │  代理组        │    │ - Shadowsocks │
│ - Redir  │  │ (TLS/HTTP) │    │  (selector,   │    │ - VMess       │
│ - TProxy │  └────────────┘    │   url-test,   │    │ - Trojan      │
│ - TUN    │                    │   fallback,   │    │ - WireGuard   │
│ - Mixed  │                    │   load-balance)│    │ - VLESS       │
│          │                    └───────────────┘     │ - Hysteria2   │
├──────────┴──────────────────────────────────────────┴───────────────┤
│                        DNS 子系统 (DNS Subsystem)                    │
│    Resolver / FakeIP / DoH / DoT / DoQ / 基于策略的路由              │
├─────────────────────────────────────────────────────────────────────┤
│                        传输层 (Transport Layer)                      │
│   协议编解码: SS, VMess, VLESS, Trojan, QUIC, gRPC, WebSocket       │
├─────────────────────────────────────────────────────────────────────┤
│                       基础设施层 (Infrastructure)                    │
│   连接池 / NAT表 / 内存管理 / Dialer / TLS / MMDB / GeoIP          │
└─────────────────────────────────────────────────────────────────────┘
```

## 分层设计理念

系统采用 **管道模型 (Pipeline Model)**：流量通过入站监听器进入，经隧道核心的规则引擎分类，最后通过出站代理适配器转发。

| 层级 | 职责 | 关键特性 |
|------|------|----------|
| 管理层 | API 接口、配置管理、监控 | RESTful、WebSocket 实时推送 |
| 配置层 | YAML 解析、热重载、Provider | 运行时无缝切换 |
| 隧道核心 | 路由决策、规则匹配、流量调度 | 有序规则链、代理组 |
| 入站监听 | 协议接入、连接建立 | 多协议复用 |
| 出站适配 | 协议编码、远端连接 | 统一 trait 抽象 |
| DNS 子系统 | 域名解析、FakeIP、分流 | 避免 DNS 污染 |
| 传输层 | 协议编解码 | 加密、混淆 |
| 基础设施 | 通用组件 | 高性能数据结构 |

## Crate 依赖关系图

```
                 main (binary crate)
                  │
            ┌─────┴─────┐
            v            v
        api (hub)     config
            │            │
     ┌──────┼────────────┤
     v      v            v
  listener  tunnel    adapter
     │        │       ┌──┴───┐
     │        │       v      v
     │        │   outbound  proxy_group
     │        │       │      │
     │        v       v      v
     │      rules  transport  provider
     │        │       │        │
     └────────┴───┬───┴────────┘
                  v
              core (traits)  <── 所有 crate 依赖 (纯 trait 定义)
                  │
                  v
              component      <── 基础设施 (trie, cidr, fakeip 等)
                  │
                  v
              common         <── 通用工具 (无领域知识)
```

**关键原则**: `core` crate 仅包含 trait 定义和基础类型，不依赖任何内部 crate。所有其他 crate 通过 `core` 中的 trait 交互，利用 Rust 的 trait 系统实现零成本多态。

## 项目目录结构 (Cargo Workspace)

```
clash-rs/
├── Cargo.toml                   # Workspace 根配置
├── clash/                       # 主二进制 crate
│   ├── Cargo.toml
│   └── src/
│       └── main.rs              # 入口, tokio::main
├── clash-core/                  # 核心 trait 与类型定义
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── adapter.rs           # ProxyAdapter, ProxyConnector trait
│       ├── rule.rs              # Rule trait + RuleType enum
│       ├── metadata.rs          # Metadata 结构体
│       ├── conn.rs              # ProxyStream, ProxyDatagram trait
│       ├── provider.rs          # ProxyProvider, RuleProvider trait
│       └── error.rs             # 统一错误类型
├── clash-config/                # 配置解析
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── parser.rs            # YAML/TOML 解析
│       ├── types.rs             # 配置结构体 (serde)
│       └── validation.rs        # 配置校验
├── clash-tunnel/                # 核心路由引擎
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── tunnel.rs            # handle_tcp, handle_udp, rule_match
│       ├── relay.rs             # 双向流量转发
│       └── statistics.rs        # 流量统计
├── clash-adapter/               # 代理适配器
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── outbound/
│       │   ├── mod.rs
│       │   ├── direct.rs        # Direct 直连
│       │   ├── reject.rs        # Reject 拒绝
│       │   ├── shadowsocks.rs   # SS 代理
│       │   ├── vmess.rs         # VMess 代理
│       │   ├── trojan.rs        # Trojan 代理
│       │   ├── wireguard.rs     # WireGuard
│       │   └── vless.rs         # VLESS
│       ├── proxy_group/
│       │   ├── mod.rs
│       │   ├── selector.rs      # 手动选择
│       │   ├── url_test.rs      # 自动测速选择
│       │   ├── fallback.rs      # 故障转移
│       │   ├── load_balance.rs  # 负载均衡
│       │   └── relay.rs         # 代理链
│       └── provider/
│           ├── mod.rs
│           ├── file.rs          # 本地文件 Provider
│           └── http.rs          # HTTP 远程 Provider
├── clash-listener/              # 入站监听器
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── http/                # HTTP 代理监听
│       ├── socks/               # SOCKS5 监听
│       ├── redir/               # Linux redirect
│       ├── tproxy/              # Linux TPROXY
│       ├── mixed/               # HTTP + SOCKS 混合
│       └── tun/                 # TUN 设备
├── clash-dns/                   # DNS 子系统
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── resolver.rs          # DNS 解析器
│       ├── enhancer.rs          # FakeIP 映射
│       ├── server.rs            # 内置 DNS 服务器
│       ├── client.rs            # 上游 DNS 客户端 (DoH/DoT/DoQ)
│       └── policy.rs           # 域名策略路由
├── clash-rules/                 # 规则实现
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── domain.rs            # DOMAIN, DOMAIN-SUFFIX, DOMAIN-KEYWORD
│       ├── ipcidr.rs            # IP-CIDR, IP-CIDR6
│       ├── geoip.rs             # GEOIP (MMDB)
│       ├── geosite.rs           # GEOSITE
│       ├── process.rs           # PROCESS-NAME
│       ├── port.rs              # SRC-PORT, DST-PORT
│       ├── logic.rs             # AND, OR, NOT 组合规则
│       ├── ruleset.rs           # RULE-SET 远程规则集
│       └── final_match.rs       # MATCH 默认规则
├── clash-transport/             # 线路协议编解码
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── shadowsocks/         # SS AEAD 加密
│       ├── vmess/               # VMess 协议
│       ├── trojan/              # Trojan 协议
│       ├── vless/               # VLESS 协议
│       ├── websocket/           # WebSocket 传输
│       ├── grpc/                # gRPC 传输
│       └── quic/                # QUIC 传输
├── clash-component/             # 共享基础设施
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── fakeip.rs            # FakeIP 地址池
│       ├── geodata.rs           # GeoIP/GeoSite 数据库
│       ├── mmdb.rs              # MaxMind DB 读取
│       ├── trie.rs              # 域名 Trie 树
│       ├── cidr.rs              # CIDR 前缀树
│       ├── sniffer.rs           # TLS/HTTP 协议嗅探
│       ├── nat.rs               # UDP NAT 会话表
│       └── process.rs           # 进程名查找 (平台相关)
├── clash-api/                   # RESTful API
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── server.rs            # HTTP 服务器 (axum)
│       ├── routes/              # 路由处理器
│       └── websocket.rs         # WebSocket 日志/流量推送
└── clash-common/                # 通用工具
    ├── Cargo.toml
    └── src/
        ├── lib.rs
        ├── net.rs               # 网络工具函数
        └── cache.rs             # LRU 缓存
```

## 为什么选择 Rust?

| 特性 | 优势 |
|------|------|
| **零成本抽象** | trait 动态分发或单态化，代理适配器接口无运行时开销 |
| **内存安全** | 无 GC，所有权系统保证无数据竞争、无 use-after-free |
| **async/await** | tokio 异步运行时，单线程即可处理数万并发连接 |
| **零拷贝** | `bytes::Bytes` + `tokio::io::copy` 减少内存拷贝 |
| **交叉编译** | `cross` 工具链支持 Linux/macOS/Windows/Android/iOS |
| **类型系统** | enum + pattern matching 天然适合规则匹配和协议解析 |
| **性能** | 无 GC 暂停，确定性延迟，适合高吞吐代理场景 |

## 核心 Rust 依赖

| Crate | 用途 |
|-------|------|
| `tokio` | 异步运行时 (multi-thread + io_uring 可选) |
| `hyper` / `axum` | HTTP 服务器 (API 层) |
| `tokio-rustls` / `rustls` | TLS 实现 |
| `trust-dns-resolver` / `hickory-dns` | DNS 解析 |
| `serde` + `serde_yaml` | 配置序列化/反序列化 |
| `bytes` + `tokio-util` | 零拷贝缓冲区、编解码器 |
| `maxminddb` | GeoIP MMDB 读取 |
| `tun2` / `smoltcp` | TUN 设备 / 用户态 TCP 栈 |
| `quinn` | QUIC 传输 (Hysteria, TUIC, DoQ) |
| `tokio-tungstenite` | WebSocket 传输 |
| `tracing` | 结构化日志与链路追踪 |
| `dashmap` | 无锁并发 HashMap (NAT 表、连接追踪) |
| `arc-swap` | 原子指针交换 (热重载配置) |
| `anyhow` / `thiserror` | 错误处理 |
