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
#
# 容错：若抓取阶段因教务系统不可达而失败，会生成一个「数据暂不可用」状态页
# 并正常退出（构建不报红）。等教务系统恢复后，自动回到真实数据页。
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
if ./target/ci/neuq-classroom-query deploy; then
  GEN_OK=1
  echo "✔ 数据抓取与生成成功"
else
  GEN_OK=0
  echo "✖ 数据抓取失败（教务系统可能不可达），将生成状态页" >&2
fi

echo "=== 打包产物 ==="
mkdir -p public

if [ "$GEN_OK" = "1" ]; then
  cp index.html public/
else
  # 教务系统不可达：写一个说明当前状态的状态页
  # 注意：这里的相对路径依赖页面位于 {site}/ 根目录，与正式页保持一致
  cat > public/index.html <<'EOF'
<!DOCTYPE html>
<html lang="zh-CN">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>东秦空闲教室总表 · 数据暂不可用</title>
<link rel="icon" href="assets/favicon.svg" type="image/svg+xml">
<link rel="stylesheet" href="assets/styles/main.css">
<link rel="stylesheet" href="assets/styles/font.css">
</head>
<body>
<main class="collapsible-container">
  <h1>东秦空闲教室总表</h1>
  <p class="date-display">数据暂不可用</p>
  <div class="emergency-info">
    <p><strong>教务系统当前无法访问，暂时拿不到教室数据。</strong></p>
    <p>本站数据由程序自动抓取东秦教务系统生成。最近自动更新尝试中，教务系统对请求返回
       <code>HTTP 483</code>（空响应），连登录页都无法打开。</p>
    <p>已排查：学校官网 <code>www.neuq.edu.cn</code> 访问正常，仅
       <code>jwxt.neuq.edu.cn</code> 不可达——属教务系统对外访问限制，与本站部署无关。</p>
    <p>教务系统恢复后，本页会在下一次自动构建时替换为真实教室数据。</p>
  </div>
  <p class="info-text">页面状态更新于 <!--TS-->（北京时间）</p>
  <p class="info-text">
    <a href="https://github.com/wanYuea/neuq-classroom-query">项目仓库</a>
    · 数据以东秦教务系统实际查询结果为准
  </p>
</main>
</body>
</html>
EOF
  # 用当前 UTC+8 时间替换占位符
  TS=$(TZ=Asia/Shanghai date "+%Y-%m-%d %H:%M" 2>/dev/null || date -u -d "+8 hours" "+%Y-%m-%d %H:%M")
  sed -i "s|<!--TS-->|$TS|" public/index.html
  echo "已生成状态页，时间戳：$TS"
fi

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
