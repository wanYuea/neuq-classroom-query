# 部署说明：东秦空闲教室总表

站点：https://neuq-classroom-query-2kb.pages.dev ｜ 仓库：https://github.com/wanYuea/neuq-classroom-query

## 当前形态（2026-09-09 · 登录后直达查询）

不做自动抓取、不需要任何服务器。站点是一个**引导页**（源码 `guide/index.html`），提供：

1. 「直达 · 空闲教室查询」按钮 —— 打开教务的空闲教室查询页
2. 「教务门户登录」按钮 —— WebVPN 统一身份认证登录

**为何如此**：教务系统仅向境内开放且不对第三方页面开放跨域（CORS 实测无
`Access-Control-Allow-Origin`），第三方网页无法"代读"数据；WebVPN 又封锁境外 IP，
云端自动抓取不可行。因此改为「用户在自己浏览器里登录教务 → 在教务页面查询」。

部署/修改引导页（需境内网络，无需服务器）：
```bash
cd guide
# 编辑 index.html 后，用 wrangler 直传（需 CF token，权限 Pages:Edit）
npx wrangler@3 pages deploy . --project-name neuq-classroom-query --branch main
```

---

# 以下为已存档的自动抓取方案（当前未启用）


## 架构（最终形态）

```
[境内服务器/本机]                [境外静态托管]
  cron 每天 07:30/12:30/18:30
      │  bash deploy-server/run.sh
      │    ├─ 编译 Rust 二进制
      │    ├─ WebVPN 门户 CAS 统一认证登录（境内 IP 才能过）
      │    ├─ eams SSO 放行 → 抓取 7 天 × 7 时段教室数据
      │    └─ 组装 public/ → wrangler 直传
      ▼
  Cloudflare Pages（纯托管，已禁用 Git 自动构建）
      ▼
  https://neuq-classroom-query-2kb.pages.dev
```

**为什么这样设计**（踩坑总结）：
- 教务系统 `jwxt.neuq.edu.cn` 对外网返回 483 → 必须走 WebVPN（`vpn.neuq.edu.cn`）。
- WebVPN 门户对**境外 IP 直接 403**：GitHub Actions（美国 Azure）、Cloudflare 构建机（境外边缘）实测均被拒。
  只有**境内网络/服务器**能完成 CAS 登录。→ 抓取必须放在境内执行。
- 因此境外只保留「静态托管」职责；Cloudflare Pages 项目已 **禁用 Git 自动构建**，
  数据一律由境内 `run.sh` 直传（抓取失败时**不覆盖**线上已有数据）。

## 一、境内服务器部署（唯一生效路径）

在任意能访问 WebVPN 的境内 Linux 服务器/长期开机的机器上：

```bash
git clone https://github.com/wanYuea/neuq-classroom-query.git
cd neuq-classroom-query/deploy-server
bash setup.sh                 # 装依赖 + 引导写 .env + 装 crontab
# 编辑 .env 填学号/统一认证密码/CF 令牌
bash run.sh                   # 手动跑一次，验证抓取并直传
crontab -l                    # 确认每天 07:30/12:30/18:30 已排程
tail -f run.log               # 观察运行
```

`.env` 要点：`YOUR_NEUQ_USERNAME/PASSWORD` = 统一身份认证账号（学号+密码）；
`CLOUDFLARE_API_TOKEN` 在 https://dash.cloudflare.com/profile/api-tokens 创建（权限 `Cloudflare Pages: Edit`，
账号级）；`CLOUDFLARE_ACCOUNT_ID` = `99d2c0563683f2295fb65285cd3c2e68`。

> 国内便宜方案：阿里云/腾讯云轻量应用服务器（新用户低至几十元/年）或学生机；
> 校园网内长期在线的设备也可以。

## 二、抓取流程说明（代码内置）

`deploy` 子命令在 `NEUQ_VPN_ENABLED=true` 时自动执行：
1. **WebVPN 门户登录**（`login_portal`）：伪造 fingerprint → CAS(金智 wisedu) 登录。
   - CAS 密码加密：AES-128-CBC，key=页面下发的 `pwdDefaultEncryptSalt`(16B UTF-8)，
     iv=16 位随机串，明文前拼 64 位随机串，输出 Base64（算法见 `client.rs::cas_encrypt_password`）。
2. **教务 eams SSO**（`login_eams_cas`）：访问 `homeExt.action`，同一会话经 SSO 自动放行（无需再输密码）。
3. **抓取**：7 天各自独立会话并行；每天 7 个时段串行；`Semaphore(2)` 限并发，防教务风控。
4. 每天结果落 `output/output-day-N/`，经清洗后生成 `index.html`。

`build.sh` 同时保留「抓取失败 → 降级状态页」的容错（境外/异常时保证站点不 404）。

## 三、环境变量

| 变量 | 说明 |
| --- | --- |
| `YOUR_NEUQ_USERNAME` / `YOUR_NEUQ_PASSWORD` | 统一身份认证账号（学号/密码），必需 |
| `NEUQ_JWXT_BASE_URL` | eams 的 WebVPN 重写地址（默认已指向 vpn.neuq.edu.cn） |
| `NEUQ_VPN_ENABLED` | `true` 启用 WebVPN 门户登录（base_url 含 vpn. 时自动开启） |
| `NEUQ_VPN_BASE_URL` | WebVPN 门户（默认 https://vpn.neuq.edu.cn） |
| `NEUQ_VPN_AUTH_METHOD` | `cas`（统一认证）或 `local` |
| `NEUQ_VPN_USERNAME/PASSWORD` | 门户专用账号（缺省回退教务账号） |
| `TOTAL_DAYS` | 抓取天数，默认 7 |
| `CLOUDFLARE_API_TOKEN` / `CLOUDFLARE_ACCOUNT_ID` | 直传 Pages 用（见 deploy-server/.env） |

## 四、故障排查

- **境外访问 VPN 403**：正常现象，WebVPN 有境外 IP 封锁，抓取必须在境内跑。
- **教务返回 483**：教务系统对外不可达，改用 WebVPN 地址即可（本项目默认地址即 VPN）。
- **抓取失败保留旧数据**：`run.sh` 只在成功时上传，线上数据不会因单次失败被清掉。
- 想立即刷新：`bash run.sh` 手动执行一次。

历史备注：曾尝试 GitHub Actions（美国 IP）与 Cloudflare 定时构建作为执行引擎，均被 WebVPN 403 拦截，
故当前只用境内服务器驱动；代码仓库中保留了完整的 VPN CAS/SSO 实现供复用。
