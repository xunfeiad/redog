# Redog GUI

跨平台 (macOS / Windows) 桌面 GUI，风格参考 RedogX。

## 架构

- **Tauri 1.6** - Rust 后端 + WebView 前端
- **系统托盘菜单** - 原生 macOS 菜单栏 / Windows 系统托盘
- **嵌入式窗口** - 仪表盘、连接、日志、设置、关于

后端（`src-tauri/`）通过 HTTP API 调用 `redog` 内核（默认 `127.0.0.1:9090`，与内核的 `external-controller` 一致）。

## 功能

托盘菜单复刻 RedogX：

- 出站模式（规则 / 全局 / 直连）
- 代理组切换
- 设为系统代理（macOS 通过 `networksetup`，Windows 通过注册表）
- 复制终端代理命令（`export https_proxy=...`）
- 允许局域网连接 / 开机启动
- 仪表盘 / 连接 / 日志 / 延迟测速 / 重载配置

## 构建

```bash
# 前置：先启动 redog 内核
cargo run --bin redog -- -c config.yaml

# 另开终端运行 GUI
cd redog-gui/src-tauri
cargo tauri dev      # 开发模式
cargo tauri build    # 打包（生成 .app / .msi / .exe）
```

首次构建需要安装 `tauri-cli`：
```bash
cargo install tauri-cli --version "^1.6"
```

## macOS 菜单栏模式

`main.rs` 通过 `ActivationPolicy::Accessory` 将应用设为仅菜单栏模式（不显示 Dock 图标），行为与 RedogX 一致。

## 图标

需要准备以下图标文件放在 `src-tauri/icons/`：
- `icon.png` / `32x32.png` / `128x128.png` / `128x128@2x.png`（模板图标，用于托盘）
- `icon.icns`（macOS 应用图标）
- `icon.ico`（Windows 应用图标）

生成工具：
```bash
cargo tauri icon path/to/source.png
```
