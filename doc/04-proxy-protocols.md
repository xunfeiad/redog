# 代理协议底层原理

## 概述

所有代理协议的本质都是: **在客户端和服务端之间建立一个加密隧道，将目标地址信息编码进协议头，在服务端解码后转发到真实目标**。

区别在于:
- 握手方式 (认证、协商)
- 加密算法 (AEAD、流加密)
- 帧格式 (长度前缀、分隔符)
- 混淆策略 (TLS 伪装、HTTP 伪装)

---

## 1. SOCKS5 协议 (入站)

SOCKS5 是最基础的代理协议，定义于 [RFC 1928](https://tools.ietf.org/html/rfc1928)。

### 握手流程

```
Client                              Proxy
  │                                   │
  │─── 方法协商请求 ──────────────────>│
  │    VER=0x05, NMETHODS, METHODS    │
  │                                   │
  │<── 方法协商响应 ──────────────────│
  │    VER=0x05, METHOD              │
  │                                   │
  │─── 连接请求 ─────────────────────>│
  │    VER, CMD, RSV, ATYP,          │
  │    DST.ADDR, DST.PORT            │
  │                                   │
  │<── 连接响应 ─────────────────────│
  │    VER, REP, RSV, ATYP,          │
  │    BND.ADDR, BND.PORT            │
  │                                   │
  │<========= 数据转发 =============>│
```

### 关键字段

```rust
/// SOCKS5 地址类型
enum Atyp {
    IPv4 = 0x01,      // 4 字节 IPv4
    Domain = 0x03,     // 1 字节长度 + 域名
    IPv6 = 0x04,       // 16 字节 IPv6
}

/// SOCKS5 命令
enum Command {
    Connect = 0x01,        // TCP 连接
    Bind = 0x02,           // TCP 绑定 (很少用)
    UdpAssociate = 0x03,   // UDP 关联
}
```

### Rust 实现要点

```rust
use tokio::io::{AsyncReadExt, AsyncWriteExt};

async fn handle_socks5(mut stream: TcpStream) -> Result<Metadata, Error> {
    // 1. 读取方法协商
    let ver = stream.read_u8().await?;
    assert_eq!(ver, 0x05);
    let nmethods = stream.read_u8().await? as usize;
    let mut methods = vec![0u8; nmethods];
    stream.read_exact(&mut methods).await?;

    // 2. 回复: 无认证 (0x00)
    stream.write_all(&[0x05, 0x00]).await?;

    // 3. 读取连接请求
    let mut header = [0u8; 4];
    stream.read_exact(&mut header).await?;
    let cmd = header[1];
    let atyp = header[3];

    // 4. 解析目标地址
    let metadata = match atyp {
        0x01 => { /* IPv4: 读 4 字节 */ }
        0x03 => { /* 域名: 读 1 字节长度 + N 字节域名 */ }
        0x04 => { /* IPv6: 读 16 字节 */ }
        _ => return Err(Error::InvalidAtyp),
    };

    // 5. 回复连接成功
    stream.write_all(&[0x05, 0x00, 0x00, 0x01, 0, 0, 0, 0, 0, 0]).await?;

    Ok(metadata)
}
```

---

## 2. Shadowsocks 协议

Shadowsocks (SS) 是一个加密代理协议，设计目标是简单、快速、难以检测。

### 原理

```
Client                    SS Server                 Target
  │                          │                        │
  │── [加密(地址+数据)] ────>│                        │
  │                          │── 解密, 提取地址 ──────>│
  │                          │── 转发数据 ────────────>│
  │                          │                        │
  │<── [加密(响应数据)] ────│<── 响应 ────────────────│
```

### AEAD 加密 (推荐)

SS 现代版本使用 AEAD (Authenticated Encryption with Associated Data) 加密:

```
[salt][encrypted_payload_length][length_tag][encrypted_payload][payload_tag]
 |                              |                              |
 随机盐值                    2字节长度                      实际数据
 (用于派生密钥)              (AEAD加密)                    (AEAD加密)
```

**密钥派生过程:**

```rust
use ring::hkdf;

/// 从密码和盐值派生子密钥
fn derive_key(password: &[u8], salt: &[u8], key_len: usize) -> Vec<u8> {
    // 1. 先用 EVP_BytesToKey 从密码生成主密钥 (兼容原始 SS)
    let master_key = evp_bytes_to_key(password, key_len);

    // 2. 使用 HKDF 从主密钥 + salt 派生会话密钥
    let prk = hkdf::Salt::new(hkdf::HKDF_SHA1_FOR_LEGACY_USE_ONLY, salt)
        .extract(&master_key);
    let okm = prk.expand(&[b"ss-subkey"], key_len).unwrap();

    let mut key = vec![0u8; key_len];
    okm.fill(&mut key).unwrap();
    key
}
```

**支持的 AEAD 算法:**

| 算法 | 密钥长度 | Salt 长度 | Tag 长度 |
|------|----------|-----------|----------|
| AES-128-GCM | 16 | 16 | 16 |
| AES-256-GCM | 32 | 32 | 16 |
| ChaCha20-Poly1305 | 32 | 32 | 16 |

### 数据帧格式

```
TCP 流:
┌──────┬────────────────────┬──────┬─────────────────┬──────┐
│ Salt │ Encrypted Length   │ Tag  │ Encrypted Data  │ Tag  │
│(可变)│ (2 bytes encrypted)│(16B) │ (N bytes)       │(16B) │
└──────┴────────────────────┴──────┴─────────────────┴──────┘
  ^                          ^
  |                          |
  仅首包携带 Salt             后续帧无 Salt
  (用于派生会话密钥)

首个加密数据块包含目标地址:
┌──────┬───────────────┬──────┬──────────────┐
│ ATYP │ Address       │ Port │ Payload Data │
│(1B)  │ (variable)    │ (2B) │ (remaining)  │
└──────┴───────────────┴──────┴──────────────┘
```

### Rust 实现核心

```rust
use aes_gcm::{Aes256Gcm, Key, Nonce};
use aes_gcm::aead::{Aead, NewAead};

struct ShadowsocksStream<S> {
    inner: S,
    cipher: Aes256Gcm,
    nonce: [u8; 12],     // 递增 nonce
    read_buf: BytesMut,
}

impl<S: AsyncRead + AsyncWrite + Unpin> ShadowsocksStream<S> {
    /// 解密读取一帧数据
    async fn read_frame(&mut self) -> Result<Bytes, Error> {
        // 1. 读取加密长度 (2 + 16 bytes)
        let mut len_buf = [0u8; 2 + 16];
        self.inner.read_exact(&mut len_buf).await?;

        let len_plaintext = self.cipher.decrypt(
            &self.next_nonce(),
            &len_buf[..],
        )?;
        let data_len = u16::from_be_bytes([len_plaintext[0], len_plaintext[1]]) as usize;

        // 2. 读取加密数据 (data_len + 16 bytes tag)
        let mut data_buf = vec![0u8; data_len + 16];
        self.inner.read_exact(&mut data_buf).await?;

        let plaintext = self.cipher.decrypt(
            &self.next_nonce(),
            &data_buf[..],
        )?;

        Ok(Bytes::from(plaintext))
    }

    /// 递增 nonce (小端序)
    fn next_nonce(&mut self) -> Nonce {
        let nonce = Nonce::from(self.nonce);
        // 小端序递增
        for byte in &mut self.nonce {
            *byte = byte.wrapping_add(1);
            if *byte != 0 { break; }
        }
        nonce
    }
}
```

---

## 3. VMess 协议

VMess 是 V2Ray 定义的加密传输协议，比 SS 更复杂，支持动态端口、时间戳认证等。

### 认证机制

VMess 使用 UUID 作为用户标识，结合时间戳进行认证:

```
认证信息 = HMAC-MD5(UUID, UTC时间戳(精确到分钟))
```

服务端维护一个时间窗口 (通常 ±2 分钟)，预计算所有有效用户的认证信息，进行 O(1) 查找。

### 请求头格式

```
┌────────────┬──────────────────────────────────────────────┐
│ Auth Info  │              Encrypted Header                 │
│ (16 bytes) │ (AES-128-CFB, key=MD5(UUID), iv=MD5(auth*4))│
│ HMAC-MD5   │                                              │
├────────────┼──────────────────────────────────────────────┤
│            │  Version (1B)                                │
│            │  Data IV (16B) - 随机, 用于数据加密          │
│            │  Data Key (16B) - 随机, 用于数据加密         │
│            │  Response Auth (1B) - 服务端回复验证          │
│            │  Option (1B) - 功能选项                      │
│            │  Security (1B) - 加密方式                    │
│            │  Command (1B) - TCP/UDP                      │
│            │  Port (2B)                                   │
│            │  Address Type (1B)                           │
│            │  Address (variable)                          │
│            │  Padding (random)                            │
│            │  CRC32 (4B) - 校验                          │
└────────────┴──────────────────────────────────────────────┘
```

### 数据传输 (AEAD 模式)

```
每个数据块:
┌──────────────────┬──────┬─────────────────┬──────┐
│ Encrypted Length │ Tag  │ Encrypted Data  │ Tag  │
│ (2 bytes)        │(16B) │ (N bytes)       │(16B) │
└──────────────────┴──────┴─────────────────┴──────┘

Length = 0 表示流结束
```

---

## 4. Trojan 协议

Trojan 协议设计极简，直接复用 TLS 实现加密，协议开销最小。

### 核心思想

**把代理流量伪装成正常的 HTTPS 流量。** 外部观察者看到的是标准 TLS 连接，无法区分代理流量和普通 HTTPS 网站访问。

### 协议结构

```
TLS 层: 标准 TLS 1.3 握手 (与正常 HTTPS 完全一致)
  │
  v
应用数据层:
┌─────────────────┬──────┬──────┬───────────────┬──────┬──────────┐
│ Password Hash   │ CRLF │ CMD  │ Address+Port  │ CRLF │ Payload  │
│ (56B, SHA224)   │(\r\n)│ (1B) │ (SOCKS5 格式) │(\r\n)│ (data)   │
└─────────────────┴──────┴──────┴───────────────┴──────┴──────────┘
   ^                                                      ^
   |                                                      |
   仅首包携带                                            后续数据直接透传
   (认证+目标地址)                                       (零额外开销)
```

### 关键设计

```rust
use sha2::{Sha224, Digest};

/// Trojan 密码哈希
fn trojan_password_hash(password: &str) -> String {
    let hash = Sha224::digest(password.as_bytes());
    hex::encode(hash)
}

/// Trojan 首包构造
fn build_trojan_header(password: &str, metadata: &Metadata) -> BytesMut {
    let mut buf = BytesMut::new();

    // 56 字节密码哈希
    buf.extend_from_slice(trojan_password_hash(password).as_bytes());
    buf.extend_from_slice(b"\r\n");

    // CMD: 0x01=TCP, 0x03=UDP
    buf.put_u8(match metadata.network {
        Network::Tcp => 0x01,
        Network::Udp => 0x03,
    });

    // 地址 (SOCKS5 格式)
    encode_socks5_address(&mut buf, metadata);
    buf.extend_from_slice(b"\r\n");

    buf
}
```

**防检测机制:** 如果收到非法请求 (密码错误)，Trojan 服务端会将流量转发到一个真实的 HTTPS 网站，使得主动探测无法区分代理服务器和普通网站。

---

## 5. WireGuard 协议

WireGuard 是一个现代 VPN 协议，基于 Noise Protocol Framework。

### 握手 (Noise IK 模式)

```
Initiator                          Responder
    │                                  │
    │── Handshake Initiation ────────>│
    │   (ephemeral key, static key,   │
    │    encrypted, MAC)              │
    │                                  │
    │<── Handshake Response ──────────│
    │   (ephemeral key, encrypted,    │
    │    MAC)                         │
    │                                  │
    │<===== Data (ChaCha20-Poly1305) =====>│
```

### 密钥派生

```rust
// Noise_IKpsk2 握手
// 使用 Curve25519 ECDH + BLAKE2s + ChaCha20-Poly1305

/// WireGuard 传输密钥对
struct TransportKeys {
    send_key: [u8; 32],    // ChaCha20-Poly1305 发送密钥
    recv_key: [u8; 32],    // ChaCha20-Poly1305 接收密钥
    send_nonce: u64,       // 递增发送 nonce
    recv_nonce: u64,       // 递增接收 nonce
}
```

### 数据包格式

```
┌──────┬──────────┬─────────┬──────────────────┬──────┐
│ Type │ Receiver │ Counter │ Encrypted Data   │ Tag  │
│ (1B) │ (4B)     │ (8B)    │ (IP packet)      │(16B) │
│ 0x04 │          │ nonce   │ ChaCha20-Poly1305│      │
└──────┴──────────┴─────────┴──────────────────┴──────┘
```

---

## 6. 传输层封装 (Transport)

代理协议可以叠加不同的传输层封装，进一步增强混淆能力:

### WebSocket 传输

```
标准 HTTP/1.1 Upgrade 握手:

GET /path HTTP/1.1
Host: example.com
Upgrade: websocket
Connection: Upgrade
Sec-WebSocket-Key: ...

HTTP/1.1 101 Switching Protocols
Upgrade: websocket
Connection: Upgrade

=> 之后的 WebSocket 帧承载代理数据
```

### gRPC 传输

```
伪装为 gRPC 服务调用:
HTTP/2 POST /ServiceName/Method
Content-Type: application/grpc

=> HTTP/2 DATA 帧承载代理数据
```

### 传输层在 Rust 中的组合

```rust
/// 传输层是 AsyncRead + AsyncWrite 的包装
/// 通过组合实现协议栈:

// SS over WebSocket over TLS:
let tls_stream = TlsConnector::connect(tcp_stream).await?;
let ws_stream = WebSocketStream::connect(tls_stream).await?;
let ss_stream = ShadowsocksStream::new(ws_stream, cipher, key);

// Trojan over TLS (最简单):
let tls_stream = TlsConnector::connect(tcp_stream).await?;
let trojan_stream = TrojanStream::new(tls_stream, password);

// VMess over gRPC over TLS:
let tls_stream = TlsConnector::connect(tcp_stream).await?;
let grpc_stream = GrpcStream::connect(tls_stream, service_name).await?;
let vmess_stream = VMessStream::new(grpc_stream, uuid, security);
```

这种组合得益于 Rust 的 trait 系统: 只要内层实现了 `AsyncRead + AsyncWrite`，外层就可以包装它，形成任意深度的协议栈。
