#!/bin/bash
# 轻盘一键编译脚本
# 用法: chmod +x build.sh && ./build.sh

set -e

echo "=== 轻盘 LiteDisk 一键编译 ==="
echo ""

# 检查 Rust
if ! command -v rustc &> /dev/null; then
    echo "[1/4] 安装 Rust..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source "$HOME/.cargo/env"
else
    echo "[1/4] Rust 已安装: $(rustc --version)"
fi

# 检查 Node.js
if ! command -v node &> /dev/null; then
    echo "错误: 需要安装 Node.js 18+"
    echo "请从 https://nodejs.org 下载安装"
    exit 1
fi
echo "[2/4] Node.js 已安装: $(node --version)"

# 安装前端依赖
echo "[3/4] 安装前端依赖..."
cd "$(dirname "$0")"
npm install --silent

# 编译
echo "[4/4] 编译中（首次编译需要下载 Rust 依赖，可能要几分钟）..."
echo ""

case "$1" in
    dev)
        echo "启动开发模式..."
        npm run tauri dev
        ;;
    release|"")
        echo "编译发布版..."
        npm run tauri build
        echo ""
        echo "=== 编译完成 ==="
        echo "产物在: src-tauri/target/release/bundle/"
        echo ""
        ls -la src-tauri/target/release/bundle/*/
        ;;
    android)
        echo "编译 Android 版..."
        rustup target add aarch64-linux-android armv7-linux-androideabi i686-linux-android x86_64-linux-android
        npm run tauri android build
        echo ""
        echo "=== Android 编译完成 ==="
        echo "APK 在: src-tauri/gen/android/app/build/outputs/apk/"
        ;;
    all)
        echo "编译全平台..."
        echo ""
        echo "--- 当前平台 ---"
        npm run tauri build
        
        echo ""
        echo "--- Windows (需要安装 cross 交叉编译工具) ---"
        rustup target add x86_64-pc-windows-gnu
        npm run tauri build --target x86_64-pc-windows-gnu || echo "Windows 交叉编译失败，请在 Windows 上编译"
        
        echo ""
        echo "--- macOS (需要在 macOS 上编译) ---"
        echo "macOS 必须在 macOS 系统上编译，跳过"
        
        echo ""
        echo "=== 全平台编译完成 ==="
        echo "当前平台产物: src-tauri/target/release/bundle/"
        ;;
    *)
        echo "用法: ./build.sh [dev|release|android|all]"
        echo "  dev     - 开发模式（带热重载）"
        echo "  release - 编译发布版（默认）"
        echo "  android - 编译 Android APK"
        echo "  all     - 尝试编译所有平台"
        ;;
esac
