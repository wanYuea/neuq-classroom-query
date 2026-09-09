#!/usr/bin/env bash
# ============================================================
# 境内服务器部署：抓取 + 直传 Cloudflare Pages（每 1 次运行）
# 用法：./run.sh
# 前置：已执行过 setup.sh；仓库在 $REPO_DIR
# 说明：
#   - 抓取成功才直传 public/，失败时保留线上已有数据（不覆盖）
#   - WebVPN 门户有境外 IP 封锁，本脚本必须在境内网络/服务器执行
# ============================================================
set -euo pipefail

cd "$(dirname "$0")"

# ---------- 配置 ----------
# 仓库根 = 本脚本所在 deploy-server/ 的上一级
REPO_DIR="$(cd .. && pwd)"
if [ ! -f "$REPO_DIR/Cargo.toml" ]; then
  echo "✖ 找不到仓库源码（$REPO_DIR/Cargo.toml 不存在）" >&2
  echo "  请确认脚本位于仓库的 deploy-server/ 目录内（先 git clone 整个仓库）" >&2
  exit 1
fi
ENV_FILE="$(pwd)/.env"
WORK_DIR="$(pwd)/work"          # 编译与产物目录（避免污染仓库）
CLOUDFLARE_PROJECT="neuq-classroom-query"

# ---------- 加载 .env ----------
if [ ! -f "$ENV_FILE" ]; then
  echo "缺少 .env（先执行 ./setup.sh）" >&2
  exit 1
fi
set -a
# shellcheck disable=SC1090
. "$ENV_FILE"
set +a

# ---------- 拉最新代码 ----------
if [ -d "$REPO_DIR/.git" ]; then
  git -C "$REPO_DIR" pull --ff-only -q || echo "警告：代码更新失败，用本地版本继续" >&2
fi

echo "=== 编译 ==="
# -j 2 限制并行：2GiB 小内存机器上全并行编译会 OOM（SIGKILL），
# 内存富余的机器可设 CARGO_JOBS 提高（如 CARGO_JOBS=4 bash run.sh）
JOBS="${CARGO_JOBS:-2}"
cargo build --manifest-path "$REPO_DIR/Cargo.toml" --profile ci --target-dir "$WORK_DIR/target" -j "$JOBS"

echo "=== 抓取（WebVPN CAS/SSO）==="
# 程序需在仓库目录运行（assets/output 相对路径），环境变量继承自 .env
( cd "$REPO_DIR" && \
  ASSETS_DIR=./assets OUTPUT_DIR=./output \
  "$WORK_DIR/target/ci/neuq-classroom-query" deploy )
RC=$?
if [ $RC -ne 0 ]; then
  echo "✖ 抓取失败（exit=$RC），保留线上已有数据，跳过上传" >&2
  exit 1
fi

echo "=== 组装 public/ ==="
PUBLIC="$(pwd)/public"
rm -rf "$PUBLIC"
mkdir -p "$PUBLIC"
cp "$REPO_DIR/index.html" "$PUBLIC/"
cp -r "$REPO_DIR/assets" "$PUBLIC/"
if [ -d "$REPO_DIR/output" ]; then cp -r "$REPO_DIR/output" "$PUBLIC/"; fi
touch "$PUBLIC/.nojekyll"

echo "=== 直传 Cloudflare Pages ==="
if command -v wrangler >/dev/null 2>&1; then
  wrangler pages deploy "$PUBLIC" --project-name "$CLOUDFLARE_PROJECT" --branch main
elif command -v npx >/dev/null 2>&1; then
  npx --yes wrangler@3 pages deploy "$PUBLIC" --project-name "$CLOUDFLARE_PROJECT" --branch main
else
  echo "✖ 未安装 wrangler/npx，无法直传。请先执行 ./setup.sh" >&2
  exit 1
fi

echo "✔ 完成。站点：https://${CLOUDFLARE_PROJECT}.pages.dev"
