#!/usr/bin/env bash
# ============================================================
# 境内服务器一键部署脚本
# 用法：bash setup.sh
# 兼容：Ubuntu / Debian（apt） 与 Alibaba Cloud Linux / CentOS / RHEL（dnf/yum）
# 作用：
#   1. 安装 Rust、Node.js(wrangler)
#   2. 克隆/更新仓库
#   3. 引导写入 .env（教务账号 + Cloudflare 令牌）
#   4. 安装 crontab 定时（每天 07:30 / 12:30 / 18:30）
# 完成后再手动跑一次：bash run.sh 验证抓取上传。
# ============================================================
set -euo pipefail
cd "$(dirname "$0")"

REPO_DIR="$(cd .. && pwd)/neuq-classroom-query"
ENV_FILE="$(pwd)/.env"

# ---- 探测发行版，选择包管理器 ----
if command -v dnf >/dev/null 2>&1; then
  PM="dnf"
elif command -v yum >/dev/null 2>&1; then
  PM="yum"
elif command -v apt-get >/dev/null 2>&1; then
  PM="apt"
else
  echo "✖ 找不到 dnf / yum / apt-get，请用 Ubuntu/Debian 或 Alibaba Cloud Linux/CentOS/RHEL" >&2
  exit 1
fi
echo "== 检测到包管理器: $PM =="

echo "== 1/5 安装系统依赖 =="
case "$PM" in
  dnf)
    sudo dnf install -y -q curl git gcc gcc-c++ make pkgconfig openssl-devel >/dev/null
    ;;
  yum)
    sudo yum install -y -q curl git gcc gcc-c++ make pkgconfig openssl-devel >/dev/null
    ;;
  apt)
    sudo apt-get update -qq
    sudo apt-get install -y -qq curl git build-essential pkg-config libssl-dev >/dev/null
    ;;
esac

echo "== 2/5 安装 Rust =="
if ! command -v cargo >/dev/null 2>&1; then
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable --profile minimal
fi
# shellcheck disable=SC1091
. "$HOME/.cargo/env"

echo "== 3/5 安装 Node + wrangler =="
# 用官方 Node LTS 二进制（不依赖发行版包管理器，Alibaba Cloud Linux 也能用）
if ! command -v node >/dev/null 2>&1; then
  NODE_VER="v20.18.0"
  ARCH="$(uname -m)"
  case "$ARCH" in
    x86_64)  NODE_ARCH="x64" ;;
    aarch64) NODE_ARCH="arm64" ;;
    *)       echo "✖ 未知架构 $ARCH，请手动安装 Node 18+"; exit 1 ;;
  esac
  mkdir -p "$HOME/.local/node-$NODE_VER"
  curl -fsSL "https://nodejs.org/dist/${NODE_VER}/node-${NODE_VER}-linux-${NODE_ARCH}.tar.xz" \
    | tar -xJ -C "$HOME/.local/node-$NODE_VER" --strip-components=1
  ln -sf "$HOME/.local/node-$NODE-ver/bin/node" /usr/local/bin/node 2>/dev/null || true
  ln -sf "$HOME/.local/node-$NODE-ver/bin/npm"  /usr/local/bin/npm  2>/dev/null || true
  echo "Node ${NODE_VER} 解压到 $HOME/.local/node-${NODE_VER}"
fi
# 确保后续命令能用 node/npm
if ! command -v node >/dev/null 2>&1; then
  export PATH="$HOME/.local/node-${NODE_VER}/bin:$PATH"
fi
echo "node: $(command -v node) | 版本: $(node -v 2>/dev/null)"
if ! command -v wrangler >/dev/null 2>&1; then
  npm install -g wrangler >/dev/null 2>&1 || true
fi
# run.sh 已自带 npx wrangler 兜底，这里仅尝试全局装一次
echo "wrangler 全局: $(command -v wrangler || echo '将使用 npx wrangler')"

echo "== 4/5 克隆/更新仓库 =="
if [ ! -d "$REPO_DIR/.git" ]; then
  git clone https://github.com/wanYuea/neuq-classroom-query.git "$REPO_DIR"
else
  git -C "$REPO_DIR" pull --ff-only -q || true
fi

echo "== 5/5 写入 .env =="
if [ -f "$ENV_FILE" ]; then
  echo "检测到已有 .env，跳过（如需修改请编辑 $ENV_FILE）"
else
  cat > "$ENV_FILE" <<EOF
# ---- 教务/统一认证账号 ----
YOUR_NEUQ_USERNAME=你的学号
YOUR_NEUQ_PASSWORD=你的统一认证密码

# ---- 抓取走 WebVPN（必需）----
NEUQ_JWXT_BASE_URL=https://vpn.neuq.edu.cn/http/77726476706e69737468656265737421fae05988693e6d456f468ca88d1b203b/eams/
NEUQ_VPN_ENABLED=true
NEUQ_VPN_BASE_URL=https://vpn.neuq.edu.cn
NEUQ_VPN_AUTH_METHOD=cas
TOTAL_DAYS=7

# ---- Cloudflare 直传（到 dash.cloudflare.com 创建 Token，权限：Pages:Edit）----
CLOUDFLARE_API_TOKEN=你的CF令牌
CLOUDFLARE_ACCOUNT_ID=99d2c0563683f2295fb65285cd3c2e68
EOF
  echo "已生成模板，请编辑：$ENV_FILE"
  echo "   填入学号/密码/CF 令牌后，再执行：bash run.sh"
  exit 0
fi

echo "== 安装 crontab（每天 07:30/12:30/18:30，Asia/Shanghai）=="
CRON_LINE="30 7,12,18 * * * cd $(pwd) && bash run.sh >> $(pwd)/run.log 2>&1"
( crontab -l 2>/dev/null | grep -v "deploy-server/run.sh" || true; echo "$CRON_LINE" ) | crontab -
echo "已安装。查看：crontab -l"

echo ""
echo "✔ 部署就绪！"
echo "  1) 编辑 $ENV_FILE 填入真实账号与令牌"
echo "  2) 手动验证：bash run.sh"
echo "  3) 之后每天 07:30 / 12:30 / 18:30 自动抓取并更新 https://neuq-classroom-query-2kb.pages.dev"
echo "  日志：$(pwd)/run.log"
