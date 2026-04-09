# Clash RESTful API 参考文档

Redog GUI 通过 HTTP RESTful API 与 Clash/Mihomo 内核通信。本文档基于实际 ClashX 运行环境（内核版本 `db2b5d`）整理。

## 基础信息

| 项目 | 值 |
|------|------|
| 默认地址 | `http://127.0.0.1:9090` |
| 认证方式 | `Authorization: Bearer <secret>` |
| Content-Type | `application/json` |

> **注意**：ClashX 会自动生成 `api-secret`，存储在 macOS 的 `com.west2online.ClashX` preferences 中。Redog GUI 在 macOS 上会自动检测该密钥。

---

## 1. GET /version

获取内核版本号。

**响应示例**：
```json
{
  "version": "db2b5d"
}
```

---

## 2. GET /configs

获取当前运行配置。

**响应示例**：
```json
{
  "port": 0,
  "socks-port": 0,
  "redir-port": 0,
  "tproxy-port": 0,
  "mixed-port": 7890,
  "allow-lan": false,
  "bind-address": "*",
  "authentication": [],
  "mode": "rule",
  "log-level": "info",
  "ipv6": false
}
```

**字段说明**：

| 字段 | 类型 | 说明 |
|------|------|------|
| `port` | int | HTTP 代理端口 (0=未启用) |
| `socks-port` | int | SOCKS5 代理端口 |
| `mixed-port` | int | 混合代理端口 (HTTP+SOCKS5) |
| `allow-lan` | bool | 是否允许局域网连接 |
| `mode` | string | 运行模式: `rule` / `global` / `direct` |
| `log-level` | string | 日志级别: `silent` / `error` / `warning` / `info` / `debug` |

---

## 3. PATCH /configs

修改运行时配置（热更新）。

**请求体**（只需传要修改的字段）：
```json
{
  "mode": "global"
}
```

**常用操作**：
```bash
# 切换模式为全局
curl -X PATCH http://127.0.0.1:9090/configs \
  -H "Authorization: Bearer <secret>" \
  -H "Content-Type: application/json" \
  -d '{"mode":"global"}'

# 允许局域网连接
curl -X PATCH http://127.0.0.1:9090/configs \
  -H "Authorization: Bearer <secret>" \
  -d '{"allow-lan":true}'
```

---

## 4. PUT /configs

重载配置文件。

**请求体**：
```json
{
  "path": "/path/to/config.yaml"
}
```

> `path` 为空时使用当前配置路径。

---

## 5. GET /proxies

获取所有代理节点和代理组。

**响应结构**：
```json
{
  "proxies": {
    "GLOBAL": {
      "name": "GLOBAL",
      "type": "Selector",
      "now": "DIRECT",
      "all": ["节点1", "节点2", "DIRECT"],
      "history": []
    },
    "♻️ 自动选择": {
      "name": "♻️ 自动选择",
      "type": "URLTest",
      "now": "Pro-香港-BGP-06|v202604",
      "all": ["节点1", "节点2"],
      "history": [{"time": "2024-01-01T00:00:00Z", "delay": 120}]
    },
    "节点1": {
      "name": "节点1",
      "type": "Vmess",
      "history": [{"time": "2024-01-01T00:00:00Z", "delay": 85}]
    }
  }
}
```

**字段说明**：

| 字段 | 类型 | 说明 |
|------|------|------|
| `name` | string | 代理/组名称 |
| `type` | string | 类型 (见下表) |
| `now` | string | 当前选中的代理 (仅代理组) |
| `all` | string[] | 该组包含的所有代理名称 (仅代理组) |
| `history` | array | 延迟测试历史记录 |

**代理类型 (`type`)**：

| 类型 | 说明 |
|------|------|
| `Direct` | 直连 |
| `Reject` | 拒绝 |
| `Shadowsocks` | Shadowsocks 节点 |
| `ShadowsocksR` | ShadowsocksR 节点 |
| `Vmess` | VMess 节点 |
| `Trojan` | Trojan 节点 |
| `Selector` | 手动选择组 |
| `URLTest` | 自动测速选择组 |
| `Fallback` | 故障转移组 |
| `LoadBalance` | 负载均衡组 |
| `Relay` | 链式代理组 |

**区分代理组 vs 单个节点**：有 `all` 字段的是代理组，没有的是单个节点。

---

## 6. PUT /proxies/:name

切换代理组中选中的节点。

**URL 参数**：`name` 需 URL 编码（中文/特殊字符）

**请求体**：
```json
{
  "name": "Pro-香港-BGP-01|v202604"
}
```

**示例**：
```bash
# 切换 "🔰国外流量" 组到指定节点
curl -X PUT "http://127.0.0.1:9090/proxies/%F0%9F%94%B0%E5%9B%BD%E5%A4%96%E6%B5%81%E9%87%8F" \
  -H "Authorization: Bearer <secret>" \
  -d '{"name":"Pro-香港-BGP-01|v202604"}'
```

---

## 7. GET /proxies/:name/delay

测试单个节点的延迟。

**查询参数**：

| 参数 | 类型 | 说明 |
|------|------|------|
| `url` | string | 测速目标 URL |
| `timeout` | int | 超时时间 (毫秒) |

**示例**：
```bash
curl "http://127.0.0.1:9090/proxies/Pro-香港-BGP-01/delay?url=http://www.gstatic.com/generate_204&timeout=5000" \
  -H "Authorization: Bearer <secret>"
```

**响应**：
```json
{
  "delay": 120
}
```

> 超时或不可达时返回错误。

---

## 8. GET /rules

获取当前生效的规则列表。

**响应示例**：
```json
{
  "rules": [
    {
      "type": "DomainSuffix",
      "payload": "google.com",
      "proxy": "🔰国外流量"
    },
    {
      "type": "DomainKeyword",
      "payload": "github",
      "proxy": "🔰国外流量"
    },
    {
      "type": "IPCIDR",
      "payload": "192.168.0.0/16",
      "proxy": "DIRECT"
    },
    {
      "type": "Match",
      "payload": "",
      "proxy": "DIRECT"
    }
  ]
}
```

**规则类型**：

| type | 说明 | payload 示例 |
|------|------|-------------|
| `Domain` | 精确域名 | `google.com` |
| `DomainSuffix` | 域名后缀 | `google.com` (匹配 *.google.com) |
| `DomainKeyword` | 域名关键字 | `google` |
| `IPCIDR` | IP CIDR | `192.168.0.0/16` |
| `SrcIPCIDR` | 源 IP CIDR | `192.168.1.0/24` |
| `SrcPort` | 源端口 | `3000` |
| `DstPort` | 目标端口 | `443` |
| `GeoIP` | GeoIP 国家 | `CN` |
| `Match` | 兜底匹配 | (空) |

> 规则按顺序匹配，首条命中生效。本例中共有 **1309** 条规则。

---

## 9. GET /connections

获取所有活动连接。

**响应示例**：
```json
{
  "downloadTotal": 1234567,
  "uploadTotal": 234567,
  "connections": [
    {
      "id": "uuid-string",
      "metadata": {
        "network": "tcp",
        "type": "HTTP",
        "sourceIP": "127.0.0.1",
        "destinationIP": "142.250.80.46",
        "sourcePort": "51234",
        "destinationPort": "443",
        "host": "www.google.com"
      },
      "upload": 1234,
      "download": 5678,
      "start": "2024-01-01T00:00:00.000Z",
      "chains": ["Pro-香港-BGP-01|v202604", "🔰国外流量"],
      "rule": "DomainSuffix",
      "rulePayload": "google.com"
    }
  ]
}
```

**字段说明**：

| 字段 | 说明 |
|------|------|
| `metadata.host` | 目标域名 (SNI/HTTP Host) |
| `metadata.destinationIP` | 目标 IP |
| `metadata.network` | tcp / udp |
| `chains` | 代理链路 (从内到外) |
| `rule` | 匹配的规则类型 |
| `rulePayload` | 匹配的规则内容 |
| `upload` / `download` | 该连接的上传/下载字节数 |

---

## 10. DELETE /connections

关闭所有连接。

```bash
curl -X DELETE http://127.0.0.1:9090/connections \
  -H "Authorization: Bearer <secret>"
```

---

## 11. DELETE /connections/:id

关闭指定连接。

```bash
curl -X DELETE http://127.0.0.1:9090/connections/<id> \
  -H "Authorization: Bearer <secret>"
```

---

## 12. GET /traffic

获取实时流量（WebSocket 或单次请求）。

**响应**：
```json
{
  "up": 1024,
  "down": 4096
}
```

> 单位为 bytes/s。

---

## 13. GET /logs

获取实时日志（WebSocket）。

连接后持续推送：
```json
{
  "type": "info",
  "payload": "connection matched rule DomainSuffix(google.com) using proxy 🔰国外流量"
}
```

---

## 14. GET /providers/proxies

获取代理提供者（proxy-provider）列表。

**响应**：
```json
{
  "providers": {
    "provider-name": {
      "name": "provider-name",
      "type": "HTTP",
      "vehicleType": "HTTP",
      "proxies": [...],
      "updatedAt": "2024-01-01T00:00:00Z"
    }
  }
}
```

---

## 15. PUT /providers/proxies/:name

更新指定代理提供者。

```bash
curl -X PUT http://127.0.0.1:9090/providers/proxies/my-provider \
  -H "Authorization: Bearer <secret>"
```

---

## Redog GUI 扩展命令

Redog GUI 在 Clash API 之上增加了以下 Tauri 命令（仅前端使用）：

| 命令 | 参数 | 说明 |
|------|------|------|
| `set_api_config` | `{baseUrl, secret}` | 设置 API 地址和密钥 |
| `get_api_config` | - | 获取当前 API 配置 |
| `fetch_subscription` | `{url}` | 拉取并解析订阅链接 |
| `set_system_proxy` | `{enable}` | 开关系统代理 |
| `get_system_proxy` | - | 获取系统代理状态 |
| `copy_terminal_command` | - | 复制终端代理命令到剪贴板 |

---

## 错误处理

所有 API 在认证失败时返回：
```json
{
  "message": "Unauthorized"
}
```

HTTP 状态码约定：

| 状态码 | 含义 |
|--------|------|
| 200 | 成功 |
| 204 | 成功 (无响应体) |
| 400 | 请求参数错误 |
| 401 | 未认证 (缺少/错误的 secret) |
| 404 | 资源不存在 |
| 500 | 内部错误 |

---

## 测试脚本

```bash
SECRET="your-secret-here"
BASE="http://127.0.0.1:9090"
AUTH="Authorization: Bearer $SECRET"

# 版本
curl -s -H "$AUTH" $BASE/version | python3 -m json.tool

# 当前模式
curl -s -H "$AUTH" $BASE/configs | python3 -m json.tool

# 代理组列表
curl -s -H "$AUTH" $BASE/proxies | python3 -c "
import sys,json
d = json.load(sys.stdin)
for k,v in d['proxies'].items():
    if v.get('all'):
        print(f'{k} ({v[\"type\"]}): now={v.get(\"now\",\"-\")}, {len(v[\"all\"])} nodes')
"

# 切换为 Rule 模式
curl -s -X PATCH -H "$AUTH" -H "Content-Type: application/json" \
  $BASE/configs -d '{"mode":"rule"}'

# 测速
curl -s -H "$AUTH" "$BASE/proxies/DIRECT/delay?url=http://www.gstatic.com/generate_204&timeout=5000"
```
