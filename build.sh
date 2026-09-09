#!/usr/bin/env bash
# 通用构建脚本：抓取 -> 处理 -> 生成 -> 打包到 public/
#
# Cloudflare Pages 用法：
#   Build command:            bash build.sh
#   Build output directory:   public
#
# 环境变量（在 Cloudflare Pages → Settings → Environment variables 里配置）：
#   YOUR_NEUQ_USERNAME  学号（设为加密变量）
#   YOUR_NEUQ_PASSWORD  密码（设为加密变量）
#   TOTAL_DAYS          查询天数，默认 7
#   MINIFY_HTML         是否压缩 HTML，默认 true
set -euo pipefail

export ASSETS_DIR="${ASSETS_DIR:-./assets}"
export OUTPUT_DIR="${OUTPUT_DIR:-./output}"

# Cloudflare Pages 的构建环境默认不含 Rust，按需安装
if ! command -v cargo >/dev/null 2>&1; then
  echo "未检测到 Rust，开始安装..."
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
  # shellcheck disable=SC1090,SC1091
  . "$HOME/.cargo/env"
fi

echo "=== 编译 ==="
cargo build --profile ci

echo "=== 抓取 / 处理 / 生成 ==="
./target/ci/neuq-classroom-query deploy

echo "=== 打包产物 ==="
mkdir -p public
cp index.html public/
cp -r assets public/
if [ -d output ]; then
  cp -r output public/
fi
touch public/.nojekyll

# CI 环境清理编译产物，节省空间
if [ "${CI:-false}" = "true" ]; then
  rm -rf target
fi

echo "✔ 构建完成，产物位于 public/ ($(du -sh public | cut -f1))"
