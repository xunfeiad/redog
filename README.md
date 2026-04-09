# Redog

[简体中文](./README.zh-CN.md) | **English**

A cross-platform rule-based proxy client implemented in Rust, with a Tauri v2 desktop GUI.

Redog is a Clash-compatible proxy engine with a native desktop dashboard. It speaks the Clash RESTful API, so it can also be used as a frontend for any existing Clash-compatible core (ClashX, Clash Verge, etc.).

## Features

- **Cross-platform**: macOS, Windows, Linux
- **Proxy protocols**: Shadowsocks, VMess, Trojan, SOCKS5, HTTP
- **Rule engine**: DOMAIN / DOMAIN-SUFFIX / DOMAIN-KEYWORD / IP-CIDR / GEOIP / MATCH
- **Proxy groups**: Selector, URLTest, Fallback, LoadBalance, Relay
- **DNS**: Hickory-DNS resolver with FakeIP support
- **System proxy**: one-click toggle with batched privilege elevation on macOS
- **Subscription**: base64, Clash YAML and share URI parsing
- **Tray menu**: quick mode switching, latency testing, connection/log view
- **Clash-compatible API**: `/proxies`, `/rules`, `/connections`, `/traffic`, `/configs`

## Project Structure

```
clash-rs/
├── redog/              # Binary entry point
├── redog-core/         # Core traits and types
├── redog-common/       # Shared utilities
├── redog-config/       # YAML config parsing
├── redog-adapter/      # Proxy adapters (SS/VMess/Trojan/...)
├── redog-component/    # Domain trie / CIDR tree / GeoIP
├── redog-rules/        # Rule engine
├── redog-dns/          # DNS resolver + FakeIP
├── redog-tunnel/       # Tunnel dispatcher and statistics
├── redog-listener/     # Inbound listeners (HTTP/SOCKS/Mixed)
├── redog-transport/    # Transport layer (SOCKS5 client)
├── redog-api/          # Clash-compatible RESTful API (axum)
└── redog-gui/          # Tauri v2 desktop GUI
```

## Quick Start

### Run the core

```bash
cargo run --bin redog -- -c config.yaml
```

### Run the desktop GUI

```bash
cd redog-gui
cargo tauri dev
```

The GUI will auto-detect a running ClashX instance on macOS and load its API secret. You can also point it at any Clash-compatible core via Settings → API URL.

## Tech Stack

| Layer | Technology |
|-------|------------|
| Language | Rust (2021 edition) |
| Async runtime | Tokio (multi-thread) |
| HTTP server | Axum |
| TLS | Rustls / Tokio-Rustls |
| DNS | Hickory-DNS |
| Serialization | Serde + serde_yaml |
| Logging | Tracing |
| Desktop GUI | Tauri v2 + vanilla JS |

## Documentation

Detailed technical design documents live in the [doc/](./doc/) folder:

| Document | Content |
|----------|---------|
| [01-architecture-overview](./doc/01-architecture-overview.md) | Architecture, layering, crate dependency graph |
| [02-core-interfaces](./doc/02-core-interfaces.md) | Core traits (ProxyAdapter, Rule, Metadata, Provider) |
| [03-data-flow](./doc/03-data-flow.md) | TCP/UDP/DNS data flow with Rust implementation details |
| [04-proxy-protocols](./doc/04-proxy-protocols.md) | Protocol internals (SOCKS5, SS, VMess, Trojan, WireGuard) |
| [05-fakeip-principle](./doc/05-fakeip-principle.md) | FakeIP address pool and rule-engine integration |
| [06-rule-engine](./doc/06-rule-engine.md) | Rule engine, domain trie, CIDR prefix tree, GeoIP/GeoSite |
| [07-proxy-groups](./doc/07-proxy-groups.md) | Proxy groups (Selector, URLTest, Fallback, LoadBalance, Relay) |
| [08-tun-and-system-proxy](./doc/08-tun-and-system-proxy.md) | TUN device, system proxy, iptables redirect/tproxy |
| [09-config-and-hot-reload](./doc/09-config-and-hot-reload.md) | YAML config, ArcSwap hot reload, Provider remote updates |
| [10-api-and-dashboard](./doc/10-api-and-dashboard.md) | RESTful API (axum), WebSocket streaming, connection management |
| [11-performance-design](./doc/11-performance-design.md) | Performance, zero-copy, buffer pools, io_uring, platform tuning |
| [12-clash-api-reference](./doc/12-clash-api-reference.md) | Clash RESTful API reference |
| [13-tauri-guide](./doc/13-tauri-guide.md) | Tauri v2 beginner-to-advanced guide |

## License

TBD
