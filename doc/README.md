# Redog Technical Documentation

Cross-platform rule-based proxy client implemented in Rust — full technical design documents.

[简体中文](./README.zh-CN.md) | **English**

## Index

| Document | Content |
|----------|---------|
| [01-architecture-overview](01-architecture-overview.md) | Architecture, layering, crate dependency graph |
| [02-core-interfaces](02-core-interfaces.md) | Core traits (ProxyAdapter, Rule, Metadata, Provider) |
| [03-data-flow](03-data-flow.md) | TCP/UDP/DNS data flow with Rust implementation details |
| [04-proxy-protocols](04-proxy-protocols.md) | Protocol internals (SOCKS5, SS, VMess, Trojan, WireGuard) |
| [05-fakeip-principle](05-fakeip-principle.md) | FakeIP address pool and rule-engine integration |
| [06-rule-engine](06-rule-engine.md) | Rule engine, domain trie, CIDR prefix tree, GeoIP/GeoSite |
| [07-proxy-groups](07-proxy-groups.md) | Proxy groups (Selector, URLTest, Fallback, LoadBalance, Relay) |
| [08-tun-and-system-proxy](08-tun-and-system-proxy.md) | TUN device, system proxy, iptables redirect/tproxy |
| [09-config-and-hot-reload](09-config-and-hot-reload.md) | YAML config, ArcSwap hot reload, Provider remote updates |
| [10-api-and-dashboard](10-api-and-dashboard.md) | RESTful API (axum), WebSocket streaming, connection management |
| [11-performance-design](11-performance-design.md) | Performance, zero-copy, buffer pools, io_uring, platform tuning |
| [12-clash-api-reference](12-clash-api-reference.md) | Clash RESTful API reference |
| [13-tauri-guide](13-tauri-guide.md) | Tauri v2 beginner-to-advanced guide |

## Tech Stack

- **Language**: Rust (2021 Edition)
- **Async runtime**: Tokio (multi-thread)
- **HTTP framework**: Axum
- **TLS**: Rustls / Tokio-Rustls
- **DNS**: Hickory-DNS
- **Serialization**: Serde + serde_yaml
- **Logging**: Tracing
