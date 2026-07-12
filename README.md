# 轻盘 LiteDisk

轻量级多网盘客户端。自己写的，自己用。

## 特性

- **多网盘统一**：百度、阿里、夸克、123、天翼、OneDrive、Google Drive、PikPak、WebDAV
- **跨盘搬运**：直接把文件从 A 网盘搬到 B 网盘，不经过服务器
- **断点续传**：下载断了从断点继续
- **Token 自动刷新**：不用管 token 过期
- **NAS 模式**：局域网内其他设备通过 WebDAV/FTP/Web 访问你的网盘
- **P2P 直连**：不在同一网络也能远程访问（不经过中转服务器）
- **全平台**：Windows、Linux、macOS、Android（Tauri v2）

## 体积

| 平台 | 预估体积 |
|------|---------|
| Android APK | ~5 MB |
| Windows | ~6 MB |
| Linux | ~5 MB |
| macOS | ~5 MB |

## 技术栈

- **后端**：Rust（Tauri v2）
- **前端**：纯 HTML + CSS + JavaScript（ES5，不用框架）
- **网盘 API**：直接对接各家官方 API
- **NAS**：actix-web + WebDAV
- **P2P**：libp2p

## 编译

### 前置条件

1. 安装 Rust：https://rustup.rs
2. 安装 Node.js 18+
3. 安装 Tauri 依赖：
   - **Linux**: `sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget libssl-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev`
   - **Windows**: 安装 Visual Studio Build Tools
   - **macOS**: `xcode-select --install`

### 编译命令

```bash
cd LiteDisk

# 安装前端依赖（虽然没用框架，但 Tauri CLI 需要 npm）
npm install

# 开发模式（带热重载）
npm run tauri dev

# 编译发布版
npm run tauri build
```

编译产物在 `src-tauri/target/release/bundle/` 目录下。

### Android 编译

```bash
# 安装 Android 目标
rustup target add aarch64-linux-android armv7-linux-androideabi

# 安装 Android Studio + NDK
# 配置好环境变量后：
npm run tauri android build
```

## 使用

1. 启动应用
2. 在设置页添加网盘（需要先去各家开放平台注册应用，获取 API Key 和 Secret Key）
3. 登录授权
4. 开始使用

### 网盘开放平台地址

| 网盘 | 开放平台 |
|------|---------|
| 百度网盘 | https://pan.baidu.com/union/apply |
| 阿里云盘 | https://open.aliyundrive.com |
| 夸克网盘 | https://open.quark.cn |
| 123 云盘 | https://www.123pan.com/developer |
| 天翼云盘 | https://open.189.cn |
| OneDrive | https://portal.azure.com |
| Google Drive | https://console.cloud.google.com |
| 坚果云 | 直接填 WebDAV 地址 + 用户名 + 应用密码 |

## 项目结构

```
LiteDisk/
├── src-tauri/              # Rust 后端
│   ├── src/
│   │   ├── main.rs         # 桌面端入口
│   │   ├── lib.rs          # 库入口 + 全局状态
│   │   ├── commands.rs     # Tauri 命令（前端调用）
│   │   ├── drives/         # 网盘驱动
│   │   │   ├── mod.rs      # DriveManager + CloudDrive trait
│   │   │   ├── baidu.rs    # 百度网盘（参考 AList 逻辑重写）
│   │   │   ├── aliyun.rs   # 阿里云盘
│   │   │   └── others.rs   # 其他网盘（通用 OAuth + WebDAV）
│   │   ├── nas/            # NAS 服务（WebDAV + FTP + Web UI）
│   │   └── p2p/            # P2P 直连（libp2p）
│   ├── Cargo.toml
│   └── tauri.conf.json
├── web/                    # 前端（纯 HTML/CSS/JS）
│   ├── index.html
│   ├── css/
│   │   ├── base.css        # 基础重置
│   │   ├── layout.css      # 布局（侧边栏、主内容、底部导航）
│   │   ├── components.css  # 组件（按钮、输入框、文件列表、模态框）
│   │   └── pages.css       # 页面专属样式
│   └── js/
│       ├── utils.js        # 工具函数
│       ├── storage.js      # 本地存储
│       ├── api.js          # Tauri invoke 封装
│       ├── router.js       # 简易路由
│       └── app.js          # 主应用逻辑
└── README.md
```

## 致谢

- 百度网盘驱动的逻辑参考了 [AList](https://github.com/AlistGo/alist)（GPL v3）的实现，用 Rust 独立重写
- Tauri 框架：https://tauri.app

## 许可证

AGPL v3（与 AList GPL v3 兼容）
