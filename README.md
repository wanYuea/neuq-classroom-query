# 东北大学秦皇岛分校空闲教室总表

> 🌐 **线上站点**：<https://neuq-classroom-query-2kb.pages.dev>（免登录、每小时自动更新、手机电脑均可访问）

本仓库通过 **GitHub Actions** 定时运行 Rust 程序，经学校 WebVPN 与统一身份认证自动抓取教务系统的空闲教室数据，生成纯静态 HTML 后直传 Cloudflare Pages 发布。该流程每 1 小时自动执行一次。

本表不显示机房、实验室、语音室、研讨室、多功能、活动教室、智慧教室、不排课教室、体育教学场地。大学会馆、旧实验楼以及科技楼的部分特殊教室被排除在外。教务系统中信息存在异常项的教室也不会予以显示。

本表定期更新可能占用教室的校园事件。校园事件记录在 `assets/calendar/` 目录下，生成 HTML 时会读取并检查是否有当日事件；若没有事件会按规则选取一条格言。

因节假日调休时，由于教务系统可能未更新，本空教室表无法正常显示。

由于东秦教务系统网站时有变动，可能会导致自动化运行失效，**请以教务系统实际查询结果为准**。

---

## 工作原理

```
GitHub Actions（每小时定时 + 手动触发）
  ├─ job fetch（AlmaLinux 8 容器）
  │    ├─ 从 Release 下载预编译二进制（免编译，秒级准备）
  │    ├─ WebVPN 门户 CAS 统一认证登录 → 教务系统 eams SSO
  │    ├─ 抓取 7 天 × 12 时段空闲教室数据（失败自动重试 + 退避）
  │    └─ 处理数据、生成静态 HTML、组装 public/
  └─ job deploy（ubuntu-latest）
       └─ wrangler 直传 Cloudflare Pages
```

- 预编译二进制通过 GitHub Release（`prebuilt-linux-x64`）分发，Actions 无需编译 Rust。
- 抓取失败时保留线上已有数据，不会用错误状态覆盖站点。
- 若怀疑学校调整了网络策略（如 WebVPN 开始封锁数据中心 IP），可手动运行 `.github/workflows/vpn-reachability-test.yml` 快速探测。

## 本地测试

本地测试时，按照 `.env.example` 所示创建 `.env` 文件并填入对应变量，然后运行：

```shell
cargo run --release -- deploy
```

各环境变量含义见 `.env.example` 内注释；Rust 实现细节见 `README.rust.md`。

## 自动化部署

### 1. Cloudflare Pages 项目

创建一个 Cloudflare Pages 项目作为纯托管（本仓库当前项目名 `neuq-classroom-query`），**无需绑定 Git 构建**——部署由 Actions 直传完成。

### 2. 配置 Repository Secrets

在仓库 **Settings → Secrets and variables → Actions** 中添加：

| Secret | 说明 |
|---|---|
| `NEUQ_USERNAME` | 统一身份认证学号 |
| `NEUQ_PASSWORD` | 统一身份认证密码 |
| `CLOUDFLARE_API_TOKEN` | Cloudflare API 令牌（需 `Cloudflare Pages: Edit` 权限） |
| `CLOUDFLARE_ACCOUNT_ID` | Cloudflare 账户 ID |

### 3. 工作流

流程定义见 [.github/workflows/refresh.yml](.github/workflows/refresh.yml)：

- 定时触发：`cron: '20 * * * *'`（每小时第 20 分钟）
- 手动触发：Actions → Refresh & Publish → Run workflow

> 注意：GitHub 定时工作流在仓库连续 60 天无任何活动后会被自动停用，届时推送任意提交即可重新激活。公开仓库使用 Actions 免费。

其他部署方式（境内服务器定时抓取等）见 [DEPLOY.md](DEPLOY.md)。

---

## 声明

1. **非官方项目**：本项目为个人学习项目，与东北大学秦皇岛分校及教务处等官方机构无任何关联，内容与数据不代表官方立场。
2. **数据来源与准确性**：数据抓取自教务系统对登录用户开放的空闲教室查询功能，仅供学习交流与个人查询参考。教室实际使用情况请**以教务系统及学校相关部门安排为准**；因数据延迟、节假日调休或系统变动造成的误差，本项目不承担责任。
3. **账号与隐私**：程序仅使用使用者**本人**的统一身份认证账号进行查询；学号与密码仅保存于 GitHub Actions Secrets 或本地 `.env` 文件中，代码不会上传或分享任何凭证。线上站点为纯静态页面，**不采集、不存储任何访客的个人信息**。
4. **合理使用**：请勿将本项目用于商业用途，请勿高频抓取或对学校系统造成额外负担，请遵守学校相关规定。因使用本项目而产生的任何问题由使用者自行承担。
5. **版权与致谢**：本项目基于 [@TsiaohanWang/neuq-classroom-query](https://github.com/TsiaohanWang/neuq-classroom-query) 改进而来，感谢原作者的思路与代码，以及[@Ferry-200](https://github.com/Ferry-200) 的优化思路。原作者仓库未声明开源许可证，本项目仅作个人学习用途；如学校方面或相关权利人认为本项目不妥，请提 Issue 或联系我，我会及时下线或删除。

## 相关文档

- [DEPLOY.md](DEPLOY.md) —— 部署细节与踩坑记录
- [README.rust.md](README.rust.md) —— Rust 实现细节（架构、规则、性能）
