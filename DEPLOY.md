# 部署说明（Cloudflare Pages）

本项目已改用 Cloudflare Pages 部署。仓库：`https://github.com/wanYuea/neuq-classroom-query`

## 当前状态（2026-09-09，已完整部署）

| 项 | 值 / 状态 |
| --- | --- |
| Cloudflare Pages 项目 | `neuq-classroom-query`（Git 连接型，✅ 已连接 GitHub） |
| 访问地址 | https://neuq-classroom-query-2kb.pages.dev |
| 构建命令 / 输出目录 | `bash build.sh` / `public` ✅ |
| 加密变量 | `YOUR_NEUQ_USERNAME`、`YOUR_NEUQ_PASSWORD` ✅ |
| 自动构建 | ✅ 每次 push 到 `main` 自动构建部署 |
| 定时更新 | ✅ 每小时（Cron Worker `neuq-pages-cron`，UTC `30 * * * *`） |
| **站点内容** | ✅ 在线。教务系统可达时显示真实数据；不可达时自动降级为状态页 |

**关于「Git 连接」的授权**（已完成，仅记录流程）：应用现名 **Cloudflare Workers and Pages**
（旧 `github.com/apps/cloudflare-pages` 已 404）。如需重装，走 Cloudflare 后台
Workers & Pages → Create application → Pages → Connect to Git → + Add account → Install & Authorize
→ 仓库权限选 `wanYuea/neuq-classroom-query`。查看授权：https://github.com/settings/installations

## 自动更新机制说明

- **GitHub push 触发**：向 `main` 推代码即触发一次构建。
- **每小时定时**：Cloudflare 的 `neuq-pages-cron` Worker（Cron `30 * * * *` UTC）调用
  Cloudflare API 触发一次生产部署（源码 `cloudflare-cron-worker.js` 已同步到仓库，Worker 侧的
  实际部署由 `cf-cron/worker.js` 驱动）。
- **容错**：`build.sh` 在抓取失败（教务系统不可达）时，会生成一个带时间戳的
  「数据暂不可用」状态页并**正常退出**，保证站点始终在线不 404。
  教务系统恢复后，下次构建自动回到真实数据页。

## 一、连接仓库

在 Cloudflare Dashboard → **Workers & Pages** → **Create application** → **Pages** →
**Connect to Git**，选中 `wanYuea/neuq-classroom-query`，构建配置填：

| 字段 | 值 |
| --- | --- |
| Framework preset | `None` |
| Build command | `bash build.sh` |
| Build output directory | `public` |
| Root directory | `/`（留空） |

`build.sh` 会自行检测并安装 Rust，无需额外配置。

## 二、环境变量

Pages 项目 → **Settings** → **Environment variables**：

| 变量名 | 值 | 说明 |
| --- | --- | --- |
| `YOUR_NEUQ_USERNAME` | 学号 | 设为 **Encrypt** |
| `YOUR_NEUQ_PASSWORD` | 密码 | 设为 **Encrypt** |
| `TOTAL_DAYS` | `7` | 可选 |
| `MINIFY_HTML` | `true` | 可选 |

## 三、定时更新（Cron Worker）

Cloudflare Pages 没有内置定时触发。用仓库里的 `cloudflare-cron-worker.js` 建一个
Cron Worker，它每小时调用一次 Cloudflare API 触发生产构建。

> ⚠️ 不要用 Deploy Hook：API 创建的 hook 拿不到 Dashboard 里那个完整 secret URL
> （API 只返回 `hook_id`，POST 裸 `hook_id` 端点会 400），Worker 里没法用它。

1. 本地部署 Worker（已就绪，代码即本仓库的 `cloudflare-cron-worker.js`，Cloudflare 侧名为
   `neuq-pages-cron`，实际文件在 `cf-cron/worker.js`）。重建命令：
   ```bash
   cd cf-cron
   wrangler deploy   # wrangler.toml 里已含 crons = ["30 * * * *"]
   ```
2. 给该 Worker 添加两个**加密**变量：
   - `CF_API_TOKEN` —— Cloudflare API token（至少需 `Pages:Edit`）
   - `CF_ACCOUNT_ID` —— 账号 ID `99d2c0563683f2295fb65285cd3c2e68`
   写入命令：
   ```bash
   printf '<token>' | wrangler secret put CF_API_TOKEN --name neuq-pages-cron
   printf '<account>' | wrangler secret put CF_ACCOUNT_ID --name neuq-pages-cron
   ```
3. Cron 触发器设为 `30 * * * *`（UTC 每小时第 30 分，避开整点排队）。
4. Worker 逻辑：`scheduled` 里 `POST /accounts/{id}/pages/projects/neuq-classroom-query/deployments`
   触发一次生产构建。另带 `fetch` 入口，浏览器访问一次即手动触发一次（便于测试）。

## 四、自定义域名（可选）

绑定自己的域名后，把仓库根目录 `CNAME` 的内容改成该域名。

`CNAME` 只在构建期被程序读取，用于和线上版本比对数据哈希、生成
`ALL UPDATED` / `NOT UPDATED` 徽章——它不会被打进 `public/`，所以不会影响 Pages 本身的域名配置。

## 教务系统返回 HTTP 483（当前已知阻塞）

教务系统 `jwxt.neuq.edu.cn` 自 2026/09/03 前后起对外网返回 **483 空响应**（连登录页都拿不到）。
已从 Cloudflare 西雅图边缘节点实测确认：`jwxt` 483、`www.neuq.edu.cn` 正常。

**站点不会因此挂掉**：`build.sh` 已做容错——抓取失败时生成「数据暂不可用」状态页并正常退出，
站点始终在线。教务系统恢复后，下一次定时构建自动切回真实数据页，无需人工干预。

排查时可用 `curl -i http://jwxt.neuq.edu.cn/eams/loginExt.action` 看是否仍返回 483。
