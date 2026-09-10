# 部署说明：东秦空闲教室总表

站点：<https://neuq-classroom-query-2kb.pages.dev> ｜ 仓库：<https://github.com/wanYuea/neuq-classroom-query>

## 当前形态（2026-09-10 · GitHub Actions 全自动）

```
GitHub Actions（cron 每小时 + 手动触发）
  ├─ job fetch（almalinux:8 容器）
  │    ├─ 下载 Release 预编译二进制 neuq-classroom-query-linux-x86_64-al8.gz
  │    ├─ WebVPN 门户 CAS 统一认证登录 → 教务系统 eams SSO 放行
  │    ├─ 抓取 7 天 × 12 时段教室数据（任务级 3 次重试 + 退避）
  │    ├─ 组装 public/（index.html + assets + output + .nojekyll）
  │    └─ 上传 artifact
  └─ job deploy（ubuntu-latest）
       └─ cloudflare/wrangler-action 直传 Cloudflare Pages（--branch main）
```

### 配置步骤

1. **Cloudflare Pages**：创建项目（如 `neuq-classroom-query`），保持直传模式，**关闭 Git 自动构建**。
2. **GitHub Secrets**（Settings → Secrets and variables → Actions）：

   | Secret | 说明 |
   |---|---|
   | `NEUQ_USERNAME` | 统一身份认证学号 |
   | `NEUQ_PASSWORD` | 统一身份认证密码 |
   | `CLOUDFLARE_API_TOKEN` | Cloudflare API 令牌（Pages: Edit） |
   | `CLOUDFLARE_ACCOUNT_ID` | Cloudflare 账户 ID |

3. 手动验证：Actions → Refresh & Publish → Run workflow；之后每小时 `cron: '20 * * * *'` 自动运行。

### 为什么这样设计（踩坑记录）

- 教务系统 `jwxt.neuq.edu.cn` 对外网直接返回 483 → 必须走 WebVPN（`vpn.neuq.edu.cn`）。
- **网络封锁实测（2026-09-10）**：
  - 阿里云机房 IP 访问 WebVPN → 403（被封，云服务器方案不可行）；
  - GitHub Actions 的 Azure 出口 IP（如 `4.242.45.25`）→ 302 正常，**且 CAS 登录、数据抓取均成功** —— 这是本方案成立的前提。
  - 早期版本误判"所有境外/机房 IP 均被封"并因此转向境内服务器方案；后经逐项探测推翻。若日后学校收紧策略，运行 `.github/workflows/vpn-reachability-test.yml` 即可快速验证。
- 预编译二进制为 AlmaLinux 8 构建（glibc 2.28 + openssl 1.1.1）→ 抓取 job 必须跑在 `almalinux:8` 容器里。
- AlmaLinux 8 容器内 `dnf install nodejs` 只提供 Node 10，而 wrangler 要求 ≥16.13；旧版 npx 还会吞掉退出码（表现为静默 exit 0 无任何输出）→ 因此部署 job 单独放在 `ubuntu-latest` 上跑官方 `cloudflare/wrangler-action@v3`。
- 教务系统偶发瞬时 500 → 抓取步骤内置任务级 3 次重试（间隔递增），单次失败不会导致整条流水线报废。
- Cloudflare Pages 项目已禁用 Git 自动构建，部署全部为直传（ad_hoc）；抓取失败时**不覆盖**线上已有数据。

### 网络策略变化时的自检

1. 运行 `.github/workflows/vpn-reachability-test.yml`（手动触发），观察 Actions 出口 IP 对 `vpn.neuq.edu.cn` 的状态码：302 = 正常，403 = 被封。
2. 若被封：改走下方"境内服务器方案"或在本机手动运行。

---

## 已归档的备选方案

### 境内服务器方案（WebVPN 封锁数据中心 IP 时使用）

在任意能访问 WebVPN 的境内 Linux 服务器/长期开机的机器上：

```bash
git clone https://github.com/wanYuea/neuq-classroom-query.git
cd neuq-classroom-query/deploy-server
bash setup.sh                 # 装依赖 + 引导写 .env + 装 crontab
# 编辑 .env 填学号/统一认证密码/CF 令牌
./run.sh                      # 抓取 + 直传 Cloudflare Pages
```

细节见 `deploy-server/README.md`。注意：2 GiB 小内存服务器编译需按 `run.sh` 内注释限制并行度，或直接使用 Release 预编译二进制。

### 引导页方案（无数据兜底）

`guide/index.html` 是一个静态引导页（直达教务查询 + WebVPN 登录入口），早期在自动抓取不可行时作为线上兜底页面。修改后可用 `npx wrangler@3 pages deploy guide --project-name neuq-classroom-query --branch main` 直传。

### 手动编译

```shell
cargo build --profile ci    # CI profile 跳过 LTO，编译更快，产物在 target/ci/
./target/ci/neuq-classroom-query deploy
```

详见 `README.rust.md` 与 `build-binary.yml` 工作流。
