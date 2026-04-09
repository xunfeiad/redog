# Redog Quick Start 快速开始

## 1. 环境准备

| 依赖 | 最低版本 | 安装 |
|------|---------|------|
| Rust | 1.75+ | `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \| sh` |
| Tauri CLI（GUI 可选） | 2.x | `cargo install tauri-cli --version "^2"` |
| macOS 系统（或 Windows / Linux） | - | - |

确认 Rust 版本：

```bash
rustc --version   # >= 1.75
cargo --version
```

## 2. 获取代码

```bash
git clone <your-repo-url> redog
cd redog
```

目录结构一览：

```
redog/
├── redog/              # 主二进制入口
├── redog-core/         # 核心 trait（ProxyAdapter, Rule, DnsResolver）
├── redog-common/       # 通用工具（LruCache, tcp_connect）
├── redog-component/    # 组件（FakeIP, DomainTrie, NAT, Sniffer）
├── redog-config/       # YAML 配置解析
├── redog-transport/    # 传输层协议（SOCKS5 编解码）
├── redog-adapter/      # 出站适配器（Direct, Reject, Selector, URLTest）
├── redog-listener/     # 入站监听（HTTP / SOCKS5 / Mixed）
├── redog-dns/          # DNS 解析器
├── redog-rules/        # 规则引擎（Domain, IP-CIDR, Port, Match）
├── redog-tunnel/       # 隧道核心（路由匹配、流量中继、统计）
├── redog-api/          # RESTful API（axum, 兼容 Clash API）
├── redog-gui/          # 桌面 GUI（Tauri, 菜单栏/系统托盘）
├── config.yaml         # 示例配置
└── doc/                # 文档
```

## 3. 编写配置

编辑 `config.yaml`（或任意路径，通过 `-c` 指定）：

```yaml
# 基础设置
mixed-port: 7890          # HTTP + SOCKS5 混合代理端口
allow-lan: false          # 是否允许局域网连接
mode: rule                # rule / global / direct
log-level: info           # silent / error / warning / info / debug
external-controller: 127.0.0.1:9090  # RESTful API 地址

# DNS 设置
dns:
  enable: true
  listen: 0.0.0.0:53
  enhanced-mode: fake-ip
  fake-ip-range: 198.18.0.1/16
  nameserver:
    - 8.8.8.8
    - 223.5.5.5

# 代理节点
proxies:
  - name: "my-ss"
    type: ss
    server: your-server.com
    port: 8388
    cipher: aes-256-gcm
    password: "your-password"

# 代理组
proxy-groups:
  - name: "Proxy"
    type: select
    proxies:
      - my-ss
      - DIRECT

# 分流规则（从上到下匹配，首条命中生效）
rules:
  - DOMAIN-SUFFIX,google.com,Proxy
  - DOMAIN-KEYWORD,github,Proxy
  - IP-CIDR,192.168.0.0/16,DIRECT
  - IP-CIDR,10.0.0.0/8,DIRECT
  - IP-CIDR,127.0.0.0/8,DIRECT
  - MATCH,DIRECT
```

## 4. 编译 & 运行

### 4.1 内核（命令行）

```bash
# 编译
cargo build --release --bin redog

# 运行
cargo run --bin redog -- -c config.yaml

# 或直接运行编译后的二进制
./target/release/redog -c config.yaml
```

启动后你会看到：

```
INFO  redog v0.1.0
INFO  mixed listener on 0.0.0.0:7890
INFO  API server on 127.0.0.1:9090
```

### 4.2 GUI 桌面版（macOS / Windows）

```bash
# 先确保内核已启动（GUI 通过 HTTP API 与内核通信）
cargo run --bin redog -- -c config.yaml &

# 启动 GUI
cd redog-gui/src-tauri
cargo tauri dev
```

打包发布版：

```bash
cargo tauri build    # 生成 .app (macOS) / .msi (Windows)
```

## 5. 设置系统代理

### 方式 A：GUI 一键设置

菜单栏点击 Redog 图标 → **设置为系统代理**

### 方式 B：终端手动设置

```bash
export https_proxy=http://127.0.0.1:7890
export http_proxy=http://127.0.0.1:7890
export all_proxy=socks5://127.0.0.1:7890
```

### 方式 C：浏览器扩展

使用 SwitchyOmega 等扩展，设置代理为 `127.0.0.1:7890`（HTTP/SOCKS5 通用）。

## 6. 验证代理

```bash
# 测试 HTTP 代理
curl -x http://127.0.0.1:7890 https://httpbin.org/ip

# 测试 SOCKS5 代理
curl -x socks5://127.0.0.1:7890 https://httpbin.org/ip
```

## 7. RESTful API

内核启动后，API 默认监听 `127.0.0.1:9090`，兼容 Clash API：

| 端点 | 方法 | 说明 |
|------|------|------|
| `/version` | GET | 版本信息 |
| `/traffic` | GET | 实时流量（上传/下载） |
| `/configs` | GET | 当前配置 |
| `/configs` | PATCH | 修改配置（如切换模式） |
| `/proxies` | GET | 所有代理 / 代理组 |
| `/proxies/:name` | PUT | 切换代理组选中节点 |
| `/proxies/:name/delay` | GET | 测速（延迟） |
| `/rules` | GET | 当前规则列表 |
| `/connections` | GET | 活动连接 |

示例：

```bash
# 查看版本
curl http://127.0.0.1:9090/version

# 切换为全局模式
curl -X PATCH http://127.0.0.1:9090/configs -d '{"mode":"global"}'

# 切换代理组选中节点
curl -X PUT http://127.0.0.1:9090/proxies/Proxy -d '{"name":"my-ss"}'

# 节点测速
curl "http://127.0.0.1:9090/proxies/my-ss/delay?url=http://www.gstatic.com/generate_204&timeout=5000"
```

## 8. 常用操作速查

| 操作 | 命令 / 方式 |
|------|------------|
| 切换模式 | API: `PATCH /configs {"mode":"rule"}` 或 GUI 托盘菜单 |
| 切换节点 | API: `PUT /proxies/组名 {"name":"节点名"}` 或 GUI 点选 |
| 重载配置 | API: `PUT /configs {"path":""}` 或 GUI 菜单 |
| 查看连接 | API: `GET /connections` 或 GUI Dashboard |
| 全部测速 | GUI → 延迟测速 |
| 复制终端命令 | GUI 菜单 → 复制终端代理命令 |

## 9. 故障排查

**端口被占用：**
```bash
lsof -i :7890     # 查看谁占了端口
```

**DNS 不生效：**
确保 `dns.enable: true`，且没有其他程序占用 53 端口。

**规则不匹配：**
规则按顺序从上到下匹配，使用 `MATCH` 作为兜底规则。调高 `log-level: debug` 查看匹配日志。

**GUI 连不上内核：**
确认内核已启动且 `external-controller` 地址为 `127.0.0.1:9090`。

---

更多架构原理请参阅 [doc/](.) 目录下的文档。
