# Redog

**简体中文** | [English](./README.md)

基于 Rust 实现的跨平台规则代理客户端，配套 Tauri v2 桌面 GUI。

Redog 是一个兼容 Clash 的代理内核，自带原生桌面仪表盘。由于它直接说 Clash RESTful API 协议，也可以作为任意 Clash 兼容内核（ClashX、Clash Verge 等）的前端使用。

## 功能特性

- **跨平台**：macOS、Windows、Linux
- **代理协议**：Shadowsocks、VMess、Trojan、SOCKS5、HTTP
- **规则引擎**：DOMAIN / DOMAIN-SUFFIX / DOMAIN-KEYWORD / IP-CIDR / GEOIP / MATCH
- **代理组**：Selector、URLTest、Fallback、LoadBalance、Relay
- **DNS**：Hickory-DNS，支持 FakeIP
- **系统代理**：一键开关，macOS 下批量提权只弹一次密码
- **订阅解析**：base64、Clash YAML、分享链接
- **托盘菜单**：快速切换模式、测延迟、查看连接/日志
- **Clash 兼容 API**：`/proxies`、`/rules`、`/connections`、`/traffic`、`/configs`

## 项目结构

```
clash-rs/
├── redog/              # 二进制入口
├── redog-core/         # 核心 trait 和类型
├── redog-common/       # 公共工具
├── redog-config/       # YAML 配置解析
├── redog-adapter/      # 代理适配器 (SS/VMess/Trojan/...)
├── redog-component/    # 域名 Trie / CIDR 树 / GeoIP
├── redog-rules/        # 规则引擎
├── redog-dns/          # DNS 解析器 + FakeIP
├── redog-tunnel/       # 隧道分发器和统计
├── redog-listener/     # 入站监听器 (HTTP/SOCKS/Mixed)
├── redog-transport/    # 传输层 (SOCKS5 客户端)
├── redog-api/          # Clash 兼容 RESTful API (axum)
└── redog-gui/          # Tauri v2 桌面 GUI
```

## 快速开始

### 运行核心

```bash
cargo run --bin redog -- -c config.yaml
```

### 运行桌面 GUI

```bash
cd redog-gui
cargo tauri dev
```

GUI 会在 macOS 下自动探测本机运行的 ClashX 实例并读取其 API secret。你也可以在「设置 → API 地址」中手动指向任何 Clash 兼容内核。

## 技术栈

| 层级 | 技术 |
|------|------|
| 语言 | Rust (2021 Edition) |
| 异步运行时 | Tokio (multi-thread) |
| HTTP 框架 | Axum |
| TLS | Rustls / Tokio-Rustls |
| DNS | Hickory-DNS |
| 序列化 | Serde + serde_yaml |
| 日志 | Tracing |
| 桌面 GUI | Tauri v2 + 原生 JS |

## 技术文档

详细的技术设计文档位于 [doc/](./doc/) 目录：

| 文档 | 内容 |
|------|------|
| [01-architecture-overview](./doc/01-architecture-overview.md) | 架构总览、分层设计、项目目录结构、Crate 依赖关系 |
| [02-core-interfaces](./doc/02-core-interfaces.md) | 核心 Trait 设计 (ProxyAdapter, Rule, Metadata, Provider) |
| [03-data-flow](./doc/03-data-flow.md) | TCP/UDP/DNS 数据流详解、每个处理步骤的 Rust 实现 |
| [04-proxy-protocols](./doc/04-proxy-protocols.md) | 代理协议底层原理 (SOCKS5, SS, VMess, Trojan, WireGuard) |
| [05-fakeip-principle](./doc/05-fakeip-principle.md) | FakeIP 原理、地址池实现、与规则引擎的协作 |
| [06-rule-engine](./doc/06-rule-engine.md) | 规则引擎、Domain Trie、CIDR 前缀树、GeoIP/GeoSite |
| [07-proxy-groups](./doc/07-proxy-groups.md) | 代理组 (Selector, URLTest, Fallback, LoadBalance, Relay) |
| [08-tun-and-system-proxy](./doc/08-tun-and-system-proxy.md) | TUN 设备、系统代理、iptables redirect/tproxy |
| [09-config-and-hot-reload](./doc/09-config-and-hot-reload.md) | YAML 配置系统、ArcSwap 热重载、Provider 远程更新 |
| [10-api-and-dashboard](./doc/10-api-and-dashboard.md) | RESTful API (axum)、WebSocket 实时推送、连接管理 |
| [11-performance-design](./doc/11-performance-design.md) | 性能优化、零拷贝、缓冲区池、io_uring、平台优化 |
| [12-clash-api-reference](./doc/12-clash-api-reference.md) | Clash RESTful API 参考 |
| [13-tauri-guide](./doc/13-tauri-guide.md) | Tauri v2 从入门到精通 |

## 许可证

待定
