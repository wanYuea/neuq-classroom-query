# 部署说明（Cloudflare Pages）

本项目已改用 Cloudflare Pages 部署。仓库：`https://github.com/wanYuea/neuq-classroom-query`

## 当前状态（2026-09-09）

| 项 | 值 / 状态 |
| --- | --- |
| Cloudflare Pages 项目 | `neuq-classroom-query` |
| 访问地址 | https://neuq-classroom-query-2kb.pages.dev |
| 构建命令 / 输出目录 | `bash build.sh` / `public` ✅ 已配置 |
| 加密变量 | `YOUR_NEUQ_USERNAME`、`YOUR_NEUQ_PASSWORD` ✅ 已写入 |
| GitHub 仓库 Secrets | 同名两个 ✅ 已更新 |
| GitHub Pages | 已启用（作为备用），定时构建已关闭 |
| **Git 连接** | ❌ 未授权 —— Cloudflare Pages 的 GitHub App 未安装 |
| **站点内容** | ❌ 暂无内容，见下方「HTTP 483」 |

### 还差一步：授权 GitHub App

当前项目是**直传模式**，Cloudflare 不会自动构建，站点永远不会自己更新。
要让它每小时自动构建，需要：

1. 打开 https://github.com/apps/cloudflare-pages 安装，只授权
   `wanYuea/neuq-classroom-query` 这一个仓库
2. 授权后把 Pages 项目改为 Git 连接型（构建配置不变），
   再按下方「第三节」配 Deploy Hook + Cron Worker

改完就能实现「学校恢复访问后自动恢复更新」。

## 一、连接仓库

在 Cloudflare Dashboard → **Workers & Pages** → **Create** → **Pages** → **Connect to Git**，
选中 `wanYuea/neuq-classroom-query`，构建配置填：

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

## 三、启用每小时自动更新

Cloudflare Pages 没有内置定时触发，需要一个 Cron Worker 打 Deploy Hook。

1. Pages 项目 → **Settings** → **Builds & deployments** → **Deploy hooks** → 创建一个 Hook，复制 URL
2. **Workers & Pages** → **Create** → **Worker**，代码用本仓库的 `cloudflare-cron-worker.js`
3. 该 Worker → **Settings** → **Variables and Secrets** → 添加加密变量
   `PAGES_DEPLOY_HOOK`，值为第 1 步的 Hook URL
4. 同页 **Triggers** → **Cron Triggers** → 添加 `0 * * * *`（每小时整点，UTC）
5. 保存即可

## 四、自定义域名（可选）

绑定自己的域名后，把仓库根目录 `CNAME` 的内容改成该域名。

`CNAME` 只在构建期被程序读取，用于和线上版本比对数据哈希、生成
`ALL UPDATED` / `NOT UPDATED` 徽章——它不会被打进 `public/`，所以不会影响 Pages 本身的域名配置。

## 已知问题：教务系统返回 HTTP 483

教务系统 `jwxt.neuq.edu.cn` 目前对外网返回 **483 空响应**（连登录页都拿不到），
抓取阶段会失败、构建中断。已确认：

- 学校官网 `www.neuq.edu.cn` 正常，说明不是学校整体断网
- 上游作者部署在 Cloudflare 的站点自 2026/09/03 起也停止更新

判断是教务系统限制了校外/境外访问，与 GitHub 还是 Cloudflare 无关。
等学校恢复后无需改动，下次定时构建会自动成功。

排查时可用 `curl -i http://jwxt.neuq.edu.cn/eams/loginExt.action` 看是否仍返回 483。
