//! Wire protocol codecs for proxy protocols.
//! Each sub-module handles framing, encryption, and obfuscation for one protocol.

pub mod socks5;

// Future: shadowsocks, vmess, trojan, vless, websocket, grpc, quic
