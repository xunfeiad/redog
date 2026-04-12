# Clash-RS (Redog) 代码审查修复文档

## 概述

本次代码审查共发现 14 类问题，已修复其中 12 项（#5 域名匹配分配、#8 连接追踪克隆暂缓）。修改涉及 15 个文件，覆盖性能优化、正确性修复、代码质量改进三个方面。

---

## 修复清单

### 1. Proxy 选择索引越界 panic 修复

**问题**: `selector.rs` 和 `url_test.rs` 中使用 `self.proxies[idx]` 直接索引，当 proxy 列表更新导致 index 失效时会 panic。

**修复**: 改为 `self.proxies.get(idx)` 安全索引，失效时 fallback 到第一个 proxy。

**涉及文件**:
- `redog-adapter/src/proxy_group/selector.rs`
- `redog-adapter/src/proxy_group/url_test.rs`

---

### 2. 连接处理背压控制

**问题**: `main.rs` 中每个入站连接直接 `tokio::spawn`，无并发上限。高负载下可能无限制创建 task 耗尽内存。

**修复**: 引入 `tokio::sync::Semaphore`，限制最大并发连接处理数为 4096。获取 permit 后才 spawn 处理 task，permit 在连接关闭后自动释放。

**涉及文件**:
- `redog/src/main.rs`

**关键代码**:
```rust
const MAX_CONCURRENT_CONNECTIONS: usize = 4096;

let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT_CONNECTIONS));
while let Some(conn) = rx.recv().await {
    let permit = semaphore.clone().acquire_owned().await?;
    tokio::spawn(async move {
        tunnel.handle_tcp(conn).await;
        drop(permit);
    });
}
```

---

### 3. Listener 错误自动重启

**问题**: Listener 失败仅打日志，应用以残缺状态继续运行。

**修复**: 引入 `run_listener_with_restart` 函数，使用指数退避策略自动重启失败的 Listener。最多重试 10 次，基础延迟 500ms，指数增长。

**涉及文件**:
- `redog/src/main.rs`

---

### 4. 热路径 String 克隆优化

**问题**: `tunnel.rs` 的 `match_adapter` 返回 `(String, String, Arc<dyn ProxyAdapter>)`，每个连接都要克隆 rule_name 和 rule_payload。

**修复**: 引入 `MatchResult` 结构体，使用 `Arc<str>` 替代 `String`。对于静态路径（DIRECT/GLOBAL/MATCH）使用零分配的字面量 `"DIRECT".into()`。

**涉及文件**:
- `redog-tunnel/src/tunnel.rs`

---

### 6. LRU 缓存锁优化

**问题**: 使用 `std::sync::Mutex`，有 poisoning 开销且锁粒度较大。容量为 0 时静默回退到 capacity=1。

**修复**:
- 替换为 `parking_lot::Mutex`，无 poisoning、更小内存占用、spin-first 策略
- 容量为 0 时使用合理默认值 64 并输出警告

**涉及文件**:
- `redog-common/src/cache.rs`
- `redog-common/Cargo.toml`（新增 `parking_lot` 依赖）

---

### 7. Trie 查找避免 Vec 堆分配

**问题**: `trie.rs` 的 `lookup` 每次调用都执行 `domain.split('.').rev().collect::<Vec<_>>()`，在堆上分配 Vec。

**修复**: 使用 `smallvec::SmallVec<[&str; 8]>` 替代 `Vec`。典型域名标签数 <= 8，全部栈上分配，零堆分配。

**涉及文件**:
- `redog-component/src/trie.rs`
- `redog-component/Cargo.toml`（新增 `smallvec` 依赖）

---

### 9. URLTest 健康检查竞态修复

**问题**: `check_all` 中先用 read lock 读 delays，再用 write lock 更新 fastest，两步操作非原子。多线程同时检查会导致状态不一致。

**修复**: 合并为单次 write lock 操作 —— 在同一个锁区间内更新 delays 并计算 fastest。

**涉及文件**:
- `redog-adapter/src/proxy_group/url_test.rs`

**关键代码**:
```rust
// 原来: 分别获取 read lock 和 write lock (竞态窗口)
// 修复后: 单次 write lock 完成所有更新
let mut delays = self.delays.write().unwrap();
let mut fastest = self.fastest.write().unwrap();
for (idx, delay) in &results {
    delays[*idx] = *delay;
}
let current_delay = delays.get(*fastest).copied().unwrap_or(u16::MAX);
// ... 更新 fastest
```

---

### 10. 后台任务优雅关闭

**问题**:
- URLTest 健康检查：`loop` 无退出机制，group drop 后仍在运行
- NAT 表清理：同样是死循环，无取消机制

**修复**: 引入 `tokio_util::sync::CancellationToken`：
- 每个后台任务在 `tokio::select!` 中监听 cancel 信号
- 实现 `Drop` trait，析构时自动取消后台任务
- 提供 `stop_*` 方法支持手动停止

**涉及文件**:
- `redog-adapter/src/proxy_group/url_test.rs`
- `redog-component/src/nat.rs`
- `redog-adapter/Cargo.toml`、`redog-component/Cargo.toml`（新增 `tokio-util` 依赖）

---

### 11. FakeIP 地址分配溢出修复

**问题**: `allocate` 使用 `wrapping_add(1)` 递增 offset，u32 溢出后地址分配不可预测。同时 `pool_size - 1` 未排除广播地址。

**修复**:
- 可用地址数改为 `pool_size - 2`（排除网络地址和广播地址）
- offset 在可用范围内取模，避免 u32 溢出

**涉及文件**:
- `redog-component/src/fakeip.rs`

---

### 12. HTTP Header 解析边界检查

**问题**: `parse_host_port` 中 `s[colon + 2..]` 未检查边界，`:` 在末尾时会 panic。IPv6 解析也存在类似问题。

**修复**: 完全重写 `parse_host_port`，增加完善的边界检查：
- 空字符串检查
- IPv6 格式校验（`[` 开头检查）
- 尾部冒号处理（`host:` → 使用默认端口）
- 端口解析失败返回 `Error` 而非静默 fallback

**涉及文件**:
- `redog-listener/src/http_parse.rs`（新建）

---

### 13. HTTP 解析代码合并

**问题**: `http/mod.rs` 和 `mixed/mod.rs` 各自实现了 `parse_host_port` 和 `parse_connect_target`，逻辑重复。

**修复**: 提取 `http_parse.rs` 共享模块，包含：
- `parse_host_port(s, default_port)` — 通用 host:port 解析（支持 IPv6）
- `extract_host_from_target(target, default_port)` — 从 HTTP 请求 target 提取 host

两个 listener 模块统一调用共享函数，附带完整单元测试。

**涉及文件**:
- `redog-listener/src/http_parse.rs`（新建）
- `redog-listener/src/lib.rs`（导出新模块）
- `redog-listener/src/http/mod.rs`（使用共享解析）
- `redog-listener/src/mixed/mod.rs`（使用共享解析）

---

### 14. 代码质量修复

#### 14a. Port Rule 返回正确 payload

**问题**: `SrcPortRule` 和 `DstPortRule` 的 `payload()` 返回空字符串 `""`。

**修复**: 使用构造函数 `new(port, adapter)`，在构造时预计算 `payload_str = port.to_string()`。

**涉及文件**:
- `redog-rules/src/port.rs`
- `redog-rules/src/parser.rs`

#### 14b. API change_proxy 实现

**问题**: `change_proxy` handler 仅打日志返回成功，实际未切换代理。

**修复**: 通过 `as_any()` downcast 到 `Selector`，调用 `selector.select(&body.name)` 完成切换。为 `ProxyAdapter` trait 新增 `as_any()` 方法。

**涉及文件**:
- `redog-api/src/server.rs`
- `redog-core/src/adapter.rs`（trait 新增 `as_any`）
- `redog-adapter/src/proxy_group/selector.rs`（实现 `as_any`）

#### 14c. 配置验证增强

**问题**: 仅验证端口冲突和 proxy group 引用。

**新增验证**:
- 端口范围检查（不允许 0）
- 空 proxy group 检查（无 proxies 也无 providers）
- 规则格式基础校验（至少包含类型和目标）
- DNS 配置校验（启用 DNS 但无 nameserver）

**涉及文件**:
- `redog-config/src/types.rs`

---

## 新增依赖

| Crate | 版本 | 用途 | 引入位置 |
|-------|------|------|----------|
| `parking_lot` | 0.12 | 高性能 Mutex 替代 std::sync::Mutex | redog-common |
| `smallvec` | 1 | 栈上小数组，避免 Trie 查找堆分配 | redog-component |
| `tokio-util` (sync) | 0.7 | CancellationToken 用于后台任务优雅关闭 | redog-adapter, redog-component |

---

## 暂缓修复项

| 编号 | 问题 | 原因 |
|------|------|------|
| #5 | 域名匹配 `.to_lowercase()` 分配 | 需要修改 Rule trait 接口，影响面较大 |
| #8 | 连接追踪 `list()` 全量克隆 | 需要引入分页 API，涉及前端 dashboard 联动 |

---

## 测试建议

1. **回归测试**: 运行 `cargo test --workspace` 验证所有单元测试通过
2. **并发测试**: 使用 `wrk` 或 `hey` 对 mixed listener 进行压力测试，验证 Semaphore 背压生效
3. **Listener 重启**: 手动 kill 绑定端口的进程，验证 listener 自动重启
4. **FakeIP 边界**: 使用小 CIDR（如 /30）测试 IP 分配环绕
5. **URLTest 竞态**: 并发触发多次 health check，验证 fastest 更新一致性
6. **HTTP 解析**: 测试 IPv6 地址、尾部冒号、空字符串等边界情况
