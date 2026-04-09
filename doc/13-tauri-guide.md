# Tauri 入门到精通指南

> 基于 Tauri v2，结合 Redog 项目实战经验整理。

---

## 目录

1. [Tauri 是什么](#1-tauri-是什么)
2. [环境搭建](#2-环境搭建)
3. [项目结构](#3-项目结构)
4. [核心概念](#4-核心概念)
5. [前后端通信 (Commands)](#5-前后端通信-commands)
6. [事件系统 (Events)](#6-事件系统-events)
7. [系统托盘 (Tray)](#7-系统托盘-tray)
8. [窗口管理](#8-窗口管理)
9. [插件系统](#9-插件系统)
10. [配置详解 (tauri.conf.json)](#10-配置详解)
11. [安全模型](#11-安全模型)
12. [打包发布](#12-打包发布)
13. [常见坑与最佳实践](#13-常见坑与最佳实践)
14. [实战：Redog 菜单栏应用](#14-实战redog-菜单栏应用)

---

## 1. Tauri 是什么

Tauri 是一个用 Rust 构建跨平台桌面应用的框架：

| 对比项 | Tauri v2 | Electron |
|--------|----------|----------|
| 后端语言 | Rust | Node.js |
| 渲染引擎 | 系统 WebView | 内置 Chromium |
| 包体大小 | ~2-5 MB | ~80-150 MB |
| 内存占用 | ~20-50 MB | ~100-300 MB |
| 系统托盘 | 原生支持 | 第三方库 |
| 安全性 | 严格 CSP + 权限 | 相对宽松 |
| 移动端 | v2 支持 iOS/Android | 不支持 |

**核心架构**：

```
┌──────────────────────────────────┐
│           前端 (HTML/JS/CSS)       │  ← 任何前端框架或纯 HTML
│         运行在系统 WebView 中        │
├──────────────────────────────────┤
│            IPC 桥梁                │  ← invoke / event
├──────────────────────────────────┤
│        Rust 后端 (tauri core)      │  ← 系统 API、文件、网络
│         + 你的 Rust 逻辑           │
└──────────────────────────────────┘
```

---

## 2. 环境搭建

### 2.1 前置依赖

```bash
# macOS
xcode-select --install
# 安装 Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Windows
# 安装 Visual Studio Build Tools (C++ 桌面开发)
# 安装 Rust: https://rustup.rs

# Linux (Debian/Ubuntu)
sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file \
  libssl-dev libayatana-appindicator3-dev librsvg2-dev
```

### 2.2 安装 Tauri CLI

```bash
cargo install tauri-cli --version "^2"
```

### 2.3 创建新项目

```bash
# 交互式创建
cargo tauri init

# 或使用 create-tauri-app (推荐，支持前端框架选择)
npm create tauri-app@latest
```

---

## 3. 项目结构

```
my-app/
├── src/                     # 前端源码
│   ├── index.html
│   ├── app.js
│   └── styles.css
├── src-tauri/               # Rust 后端
│   ├── Cargo.toml           # Rust 依赖
│   ├── tauri.conf.json      # Tauri 配置 (核心!)
│   ├── build.rs             # 构建脚本
│   ├── icons/               # 应用图标
│   └── src/
│       └── main.rs          # Rust 入口
```

### 关键文件

**build.rs** — 固定写法：
```rust
fn main() {
    tauri_build::build()
}
```

**Cargo.toml** — 最小依赖：
```toml
[dependencies]
tauri = { version = "2", features = ["tray-icon"] }
tauri-plugin-shell = "2"
serde = { version = "1", features = ["derive"] }
serde_json = "1"

[build-dependencies]
tauri-build = { version = "2", features = [] }
```

---

## 4. 核心概念

### 4.1 App 生命周期

```rust
fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())  // 注册插件
        .setup(|app| {                       // 初始化回调
            // app 已创建，可以操作窗口、托盘等
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![  // 注册命令
            my_command,
        ])
        .on_window_event(|window, event| {   // 窗口事件
            // ...
        })
        .run(tauri::generate_context!())      // 启动!
        .expect("error running app");
}
```

### 4.2 重要 trait

| Trait | 说明 | 使用场景 |
|-------|------|---------|
| `Manager` | 获取窗口、托盘、state | `app.get_webview_window("main")` |
| `Emitter` | 发送事件到前端 | `window.emit("event-name", payload)` |
| `Listener` | 监听来自前端的事件 | `app.listen("event", handler)` |

```rust
use tauri::{Manager, Emitter};

// Manager 让你能 get_webview_window, manage state 等
// Emitter 让你能 emit 事件到前端
// 需要手动 use，否则编译报错 "method not found"
```

---

## 5. 前后端通信 (Commands)

这是 Tauri 最核心的功能。

### 5.1 Rust 端：定义命令

```rust
// 简单命令
#[tauri::command]
fn greet(name: String) -> String {
    format!("Hello, {}!", name)
}

// 异步命令
#[tauri::command]
async fn fetch_data(url: String) -> Result<String, String> {
    reqwest::get(&url)
        .await
        .map_err(|e| e.to_string())?
        .text()
        .await
        .map_err(|e| e.to_string())
}

// 带状态的命令
#[tauri::command]
fn get_count(state: tauri::State<'_, AppState>) -> u32 {
    *state.count.lock().unwrap()
}

// 注册
tauri::Builder::default()
    .invoke_handler(tauri::generate_handler![greet, fetch_data, get_count])
```

### 5.2 前端：调用命令

```javascript
// Tauri v2 API 路径
const { invoke } = window.__TAURI__.core;

// 调用命令
const result = await invoke('greet', { name: 'World' });
```

### 5.3 命名规则 (重要!)

**Tauri v2 命令名和参数名保持 Rust 的 `snake_case`，不做自动转换！**

| Rust 命令名 | 前端调用名 |
|-------------|-----------|
| `get_proxies` | `get_proxies` |
| `select_proxy` | `select_proxy` |
| `set_system_proxy` | `set_system_proxy` |

```rust
// Rust 端
#[tauri::command]
fn get_user_info() -> String { ... }

// 前端调用 (保持 snake_case!)
await invoke('get_user_info'); // ✅ 正确
await invoke('getUserInfo');   // ❌ 错误! Command not found
```

**参数名同样保持 snake_case**：

```rust
// Rust 端
#[tauri::command]
fn set_config(base_url: String, api_secret: String) { ... }

// 前端 — 参数名必须和 Rust 一致
await invoke('set_config', { base_url: '...', api_secret: '...' });
```

> 注意：不要误以为 Tauri 会自动做 camelCase 转换，`Command XXX not found` 错误通常就是命名不匹配导致的。

### 5.4 返回值

```rust
// 返回 Result<T, String> — 前端用 try/catch 捕获错误
#[tauri::command]
async fn may_fail() -> Result<serde_json::Value, String> {
    // Ok(value) → 前端收到 value
    // Err(msg) → 前端 catch 到 msg
}
```

```javascript
try {
    const data = await invoke('mayFail');
    console.log(data); // Rust 的 Ok 值
} catch (e) {
    console.error(e);  // Rust 的 Err 字符串
}
```

---

## 6. 事件系统 (Events)

Commands 是前端→后端的请求-响应。Events 是双向的发布-订阅。

### 6.1 后端 → 前端

```rust
use tauri::Emitter;

// 向所有窗口广播
app.emit("traffic-update", serde_json::json!({"up": 1024, "down": 2048}))
    .unwrap();

// 向特定窗口发送
if let Some(window) = app.get_webview_window("main") {
    window.emit("navigate", "settings").unwrap();
}
```

```javascript
const { listen } = window.__TAURI__.event;

const unlisten = await listen('traffic-update', (event) => {
    console.log(event.payload); // { up: 1024, down: 2048 }
});

// 取消监听
unlisten();
```

### 6.2 前端 → 后端

```javascript
const { emit } = window.__TAURI__.event;
await emit('user-action', { type: 'click', target: 'button' });
```

```rust
use tauri::Listener;

app.listen("user-action", |event| {
    println!("Received: {:?}", event.payload());
});
```

### 6.3 Commands vs Events 选择

| 场景 | 推荐 |
|------|------|
| 请求数据 (前端要结果) | Command |
| 通知 (后端推送，不需要回复) | Event |
| 实时数据流 (流量、日志) | Event |
| 用户操作触发后端逻辑 | Command |

---

## 7. 系统托盘 (Tray)

### 7.1 启用托盘

Cargo.toml：
```toml
tauri = { version = "2", features = ["tray-icon"] }
```

tauri.conf.json：
```json
{
  "app": {
    "trayIcon": {
      "id": "main-tray",
      "iconPath": "icons/icon.png",
      "iconAsTemplate": true,
      "menuOnLeftClick": true
    }
  }
}
```

### 7.2 构建托盘菜单

```rust
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem, Submenu},
    tray::{TrayIcon, TrayIconBuilder},
    Manager,
};

pub fn build_tray(app: &AppHandle) -> Result<TrayIcon, tauri::Error> {
    // 菜单项
    let item1 = MenuItem::with_id(app, "dashboard", "仪表盘", true, None::<&str>)?;
    let item2 = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;

    // 子菜单
    let mode_rule = MenuItem::with_id(app, "mode_rule", "规则模式", true, None::<&str>)?;
    let mode_global = MenuItem::with_id(app, "mode_global", "全局模式", true, None::<&str>)?;
    let mode_menu = Submenu::with_id_and_items(
        app, "mode", "出站模式", true,
        &[&mode_rule, &mode_global],
    )?;

    // 组装菜单
    let menu = Menu::with_items(app, &[&mode_menu, &separator, &item1, &separator, &item2])?;

    // 构建托盘图标
    TrayIconBuilder::with_id("main-tray")
        .icon(app.default_window_icon().cloned().unwrap())
        .menu(&menu)
        .show_menu_on_left_click(true)
        .tooltip("My App")
        .build(app)
}
```

### 7.3 处理菜单点击

```rust
// 在 setup 中绑定
let tray = build_tray(app.handle())?;
tray.on_menu_event(|app, event| {
    match event.id().as_ref() {
        "quit" => std::process::exit(0),
        "dashboard" => {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.show();
                let _ = w.set_focus();
            }
        }
        _ => {}
    }
});
```

### 7.4 动态更新托盘菜单

```rust
// 获取已有托盘
if let Some(tray) = app.tray_by_id("main-tray") {
    // 重新构建并设置菜单
    let new_menu = build_new_menu(app)?;
    tray.set_menu(Some(new_menu))?;
}
```

---

## 8. 窗口管理

### 8.1 在 tauri.conf.json 中定义

```json
{
  "app": {
    "windows": [
      {
        "label": "main",
        "title": "My App",
        "width": 1000,
        "height": 700,
        "visible": false,
        "decorations": true,
        "resizable": true
      }
    ]
  }
}
```

### 8.2 代码中操作窗口

```rust
use tauri::Manager;

// 获取窗口
let window = app.get_webview_window("main").unwrap();

// 显示/隐藏
window.show().unwrap();
window.hide().unwrap();

// 关闭时隐藏而不是真正关闭 (菜单栏应用常用)
builder.on_window_event(|window, event| {
    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
        let _ = window.hide();
        api.prevent_close();  // 阻止真正关闭
    }
});
```

### 8.3 macOS 菜单栏应用 (隐藏 Dock 图标)

```rust
.setup(|app| {
    #[cfg(target_os = "macos")]
    {
        app.set_activation_policy(tauri::ActivationPolicy::Accessory);
    }
    Ok(())
})
```

`ActivationPolicy::Accessory` = 不在 Dock 显示，不在 Cmd+Tab 显示，只有托盘图标。

---

## 9. 插件系统

Tauri v2 把很多功能拆成了独立插件：

| 插件 | 功能 | Cargo 依赖 |
|------|------|-----------|
| `tauri-plugin-shell` | 运行系统命令、打开 URL | `tauri-plugin-shell = "2"` |
| `tauri-plugin-dialog` | 文件选择、消息框 | `tauri-plugin-dialog = "2"` |
| `tauri-plugin-fs` | 文件系统访问 | `tauri-plugin-fs = "2"` |
| `tauri-plugin-http` | HTTP 请求 (前端) | `tauri-plugin-http = "2"` |
| `tauri-plugin-notification` | 系统通知 | `tauri-plugin-notification = "2"` |
| `tauri-plugin-clipboard` | 剪贴板 | `tauri-plugin-clipboard-manager = "2"` |
| `tauri-plugin-autostart` | 开机启动 | `tauri-plugin-autostart = "2"` |
| `tauri-plugin-store` | 持久化存储 | `tauri-plugin-store = "2"` |

### 注册插件

```rust
tauri::Builder::default()
    .plugin(tauri_plugin_shell::init())
    .plugin(tauri_plugin_dialog::init())
    // ...
```

---

## 10. 配置详解

`tauri.conf.json` Tauri v2 格式：

```json
{
  "productName": "Redog",          // 应用名
  "version": "0.1.0",             // 版本号
  "identifier": "com.redog.app",  // 唯一标识 (必须!)

  "build": {
    "beforeBuildCommand": "npm run build",  // 构建前端
    "beforeDevCommand": "npm run dev",      // 开发模式
    "frontendDist": "../dist"               // 前端产物路径
  },

  "app": {
    "withGlobalTauri": true,       // 注入 window.__TAURI__
    "security": {
      "csp": "default-src 'self'; script-src 'self'"
    },
    "trayIcon": { ... },          // 托盘配置
    "windows": [ ... ]            // 窗口配置
  },

  "bundle": {
    "active": true,               // 启用打包
    "icon": [                     // 图标 (多尺寸)
      "icons/32x32.png",
      "icons/128x128.png",
      "icons/128x128@2x.png",
      "icons/icon.icns",
      "icons/icon.ico"
    ],
    "targets": "all",             // 打包目标
    "macOS": { ... },
    "windows": { ... }
  }
}
```

### v1 vs v2 配置对比

| v1 | v2 |
|----|----|
| `tauri.windows` | `app.windows` |
| `tauri.systemTray` | `app.trayIcon` |
| `tauri.allowlist` | 删除，改用插件 |
| `package.productName` | 顶层 `productName` |
| `build.distDir` | `build.frontendDist` |
| `build.devPath` | 删除，用 `beforeDevCommand` |

---

## 11. 安全模型

### 11.1 CSP (Content Security Policy)

```json
{
  "app": {
    "security": {
      "csp": "default-src 'self'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self' 'unsafe-inline'; connect-src 'self' http://127.0.0.1:*"
    }
  }
}
```

- `'self'` — 只允许加载本地资源
- `connect-src` — 允许的网络请求目标
- 生产环境**不要**设为 `null`

### 11.2 命令权限

Tauri v2 引入了细粒度权限系统，通过 `capabilities` 配置：

```json
// src-tauri/capabilities/default.json
{
  "identifier": "default",
  "windows": ["main"],
  "permissions": [
    "core:default",
    "shell:allow-open",
    "dialog:allow-open"
  ]
}
```

---

## 12. 打包发布

### 12.1 构建

```bash
# 开发模式 (热重载)
cargo tauri dev

# 构建发布版
cargo tauri build
```

产物路径：
- macOS: `target/release/bundle/macos/MyApp.app`
- macOS DMG: `target/release/bundle/dmg/MyApp_0.1.0_aarch64.dmg`
- Windows: `target/release/bundle/msi/MyApp_0.1.0_x64.msi`
- Windows NSIS: `target/release/bundle/nsis/MyApp_0.1.0_x64-setup.exe`
- Linux: `target/release/bundle/deb/my-app_0.1.0_amd64.deb`

### 12.2 图标生成

需要多种尺寸：

```bash
# 从 1024x1024 PNG 生成所有尺寸
# macOS .icns
mkdir icon.iconset
sips -z 16 16 icon.png --out icon.iconset/icon_16x16.png
sips -z 32 32 icon.png --out icon.iconset/icon_32x32.png
sips -z 128 128 icon.png --out icon.iconset/icon_128x128.png
sips -z 256 256 icon.png --out icon.iconset/icon_256x256.png
sips -z 512 512 icon.png --out icon.iconset/icon_512x512.png
iconutil -c icns icon.iconset -o icon.icns

# 或用 tauri 自带工具
cargo tauri icon ./icon.png
```

### 12.3 macOS 签名

```bash
# 使用 Apple Developer 证书
export APPLE_SIGNING_IDENTITY="Developer ID Application: Your Name (TEAM_ID)"
cargo tauri build
```

### 12.4 自动更新

tauri.conf.json:
```json
{
  "plugins": {
    "updater": {
      "active": true,
      "endpoints": ["https://releases.myapp.com/{{target}}/{{arch}}/{{current_version}}"],
      "pubkey": "your-public-key"
    }
  }
}
```

---

## 13. 常见坑与最佳实践

### 坑 1: 命令名和参数名必须保持 snake_case

```
Rust: get_user_info  →  JS 必须调 'get_user_info' (不是 getUserInfo!)
Rust: base_url 参数  →  JS 传 { base_url: '...' } (不是 baseUrl!)
```

**如果你看到 `Command XXX not found` 错误，首先检查命名是否一致。** Tauri 不做任何自动命名转换。

### 坑 2: tauri.conf.json 格式变化

v2 完全重构了配置结构，v1 的 `tauri.xxx` 嵌套不再存在。检查报错信息：
```
Error: "identifier" is a required property
Error: Additional properties are not allowed ('package', 'tauri' were unexpected)
```

### 坑 3: `emit` 需要 `use tauri::Emitter`

```rust
// 编译错误: no method named `emit` found
// 解决: 手动导入 trait
use tauri::Emitter;
```

### 坑 4: 托盘图标只支持 PNG/ICO

```
error: invalid extension `svg` used for image, must be `ico` or `png`
```

SVG 不能直接用作托盘图标，需要先转换为 PNG。

### 坑 5: HTTP 204 响应解析 JSON 会失败

Clash API 的 PUT 请求返回 204 No Content：
```rust
// 错误方式
resp.json::<Value>().await?  // 204 没有 body，会报错

// 正确方式
resp.json::<Value>().await.or_else(|_| Ok(Value::Null))
```

### 坑 6: `window.__TAURI__` 在 v2 中的路径

```javascript
// v1
const { invoke } = window.__TAURI__.tauri;

// v2
const { invoke } = window.__TAURI__.core;

// 兼容写法
const invoke = window.__TAURI__?.core?.invoke
            || window.__TAURI__?.tauri?.invoke;
```

### 坑 7: macOS 菜单栏应用需要隐藏 Dock

```rust
// 必须在 setup 中设置，不能在 build 之前
.setup(|app| {
    #[cfg(target_os = "macos")]
    app.set_activation_policy(tauri::ActivationPolicy::Accessory);
    Ok(())
})
```

### 最佳实践

1. **前端框架选择**：纯 HTML 最轻量，React/Vue 适合复杂 UI
2. **状态管理**：用 `app.manage(state)` + `tauri::State` 在命令间共享状态
3. **错误处理**：命令返回 `Result<T, String>`，前端统一 try/catch
4. **日志**：用 `tracing` crate，配合 `tracing-subscriber`
5. **异步**：IO 密集操作用 `async`，CPU 密集用 `tokio::task::spawn_blocking`
6. **托盘菜单**：复杂菜单在 `setup` 中构建，用 `on_menu_event` 处理点击
7. **窗口**: 菜单栏应用默认隐藏窗口，点击托盘菜单时显示

---

## 14. 实战：Redog 菜单栏应用

Redog 项目使用了以下 Tauri 功能：

### 架构总览

```
redog-gui/
├── src/                      # 纯 HTML/JS/CSS 前端
│   ├── index.html            # 单页应用 (SPA)
│   ├── app.js                # 所有交互逻辑
│   └── styles.css            # 样式 (暗色/亮色主题)
└── src-tauri/
    ├── tauri.conf.json       # Tauri 配置
    └── src/
        ├── main.rs           # 入口、托盘、窗口
        ├── tray.rs           # 托盘菜单构建
        ├── api.rs            # Clash API 代理 (commands)
        └── sysproxy.rs       # 系统代理设置
```

### 设计决策

| 决策 | 原因 |
|------|------|
| 纯 HTML，不用 React | 菜单栏工具 UI 简单，无需框架开销 |
| Commands 代理 Clash API | 避免 CORS 问题，可加认证 header |
| localStorage 存节点 | 比 tauri-plugin-store 更简单 |
| macOS `Accessory` 策略 | 纯托盘应用，不占 Dock |
| `on_window_event` 拦截关闭 | 隐藏而非退出，保持后台运行 |

### 关键代码模式

**1. 自动检测 ClashX secret（macOS）**：
```rust
fn detect_clashx_secret() -> Option<String> {
    let output = std::process::Command::new("defaults")
        .args(["read", "com.west2online.ClashX", "api-secret"])
        .output().ok()?;
    // ...
}
```

**2. 全局状态用 LazyLock + RwLock**：
```rust
static API_CONFIG: LazyLock<RwLock<ApiConfig>> =
    LazyLock::new(|| RwLock::new(ApiConfig::detect()));
```

**3. 托盘非 async 上下文调用 async 函数**：
```rust
// 托盘事件回调不是 async，需要 spawn 线程
pub fn set_mode_blocking(mode: &str) -> Result<(), String> {
    let mode = mode.to_string();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async { /* ... */ });
    });
    Ok(())
}
```

**4. 前端安全调用模式**：
```javascript
async function safeInvoke(cmd, args) {
    if (!invoke) throw new Error('Tauri API 未就绪');
    return invoke(cmd, args || {});
}
// 所有调用都通过 safeInvoke，统一错误处理
```

---

## 附录：Tauri v2 迁移检查清单

从 v1 迁移到 v2 时检查：

- [ ] `tauri.conf.json` 重写为 v2 格式
- [ ] `Cargo.toml` 依赖改为 `tauri = "2"`
- [ ] `tauri-build = "2"`
- [ ] 前端 `invoke` 路径从 `__TAURI__.tauri` 改为 `__TAURI__.core`
- [ ] 所有 invoke 命令名保持 snake_case（与 Rust 一致）
- [ ] 所有 invoke 参数名保持 snake_case（与 Rust 一致）
- [ ] `SystemTray` → `TrayIconBuilder`
- [ ] `SystemTrayMenu` → `Menu`
- [ ] `CustomMenuItem` → `MenuItem::with_id`
- [ ] `SystemTraySubmenu` → `Submenu`
- [ ] `app.get_window()` → `app.get_webview_window()`
- [ ] `on_system_tray_event` → `tray.on_menu_event`
- [ ] `menu_on_left_click` → `show_menu_on_left_click`
- [ ] `allowlist` 删除，改用 plugins
- [ ] 添加 `use tauri::Emitter;` (如果用 emit)
- [ ] 添加 `identifier` 到 tauri.conf.json 顶层
