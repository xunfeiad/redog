# Redog 技术文档

基于 Rust 实现的跨平台规则代理工具，完整技术设计文档。

[English](./README.md) | **简体中文**

## 文档索引

| 文档 | 内容 |
|------|------|
| [01-architecture-overview](01-architecture-overview.md) | 架构总览、分层设计、项目目录结构、Crate 依赖关系 |
| [02-core-interfaces](02-core-interfaces.md) | 核心 Trait 设计 (ProxyAdapter, Rule, Metadata, Provider) |
| [03-data-flow](03-data-flow.md) | TCP/UDP/DNS 数据流详解、每个处理步骤的 Rust 实现 |
| [04-proxy-protocols](04-proxy-protocols.md) | 代理协议底层原理 (SOCKS5, SS, VMess, Trojan, WireGuard) |
| [05-fakeip-principle](05-fakeip-principle.md) | FakeIP 原理、地址池实现、与规则引擎的协作 |
| [06-rule-engine](06-rule-engine.md) | 规则引擎、Domain Trie、CIDR 前缀树、GeoIP/GeoSite |
| [07-proxy-groups](07-proxy-groups.md) | 代理组 (Selector, URLTest, Fallback, LoadBalance, Relay) |
| [08-tun-and-system-proxy](08-tun-and-system-proxy.md) | TUN 设备、系统代理、iptables redirect/tproxy |
| [09-config-and-hot-reload](09-config-and-hot-reload.md) | YAML 配置系统、ArcSwap 热重载、Provider 远程更新 |
| [10-api-and-dashboard](10-api-and-dashboard.md) | RESTful API (axum)、WebSocket 实时推送、连接管理 |
| [11-performance-design](11-performance-design.md) | 性能优化、零拷贝、缓冲区池、io_uring、平台优化 |
| [12-clash-api-reference](12-clash-api-reference.md) | Clash RESTful API 参考 |
| [13-tauri-guide](13-tauri-guide.md) | Tauri v2 从入门到精通 |

## 技术栈

- **语言**: Rust (2021 Edition)
- **异步运行时**: Tokio (multi-thread)
- **HTTP 框架**: Axum
- **TLS**: Rustls / Tokio-Rustls
- **DNS**: Hickory-DNS
- **序列化**: Serde + serde_yaml
- **日志**: Tracing
