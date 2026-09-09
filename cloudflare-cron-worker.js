// Cloudflare Pages 没有内置「定时重新抓取」，用这个 Worker 的 Cron Trigger
// 调用 Cloudflare API 手动触发一次生产构建，实现「每小时自动更新」。
//
// 环境变量（该 Worker 的 Variables and Secrets）：
//   CF_API_TOKEN    Cloudflare API token（Pages:Edit 权限即可）
//   CF_ACCOUNT_ID   账号 ID
//
// Cron Trigger：`30 * * * *`（UTC 每小时第 30 分）
// 手动触发：GET 访问该 Worker 的 URL 一次 = 触发一次部署（便于测试）
export default {
  async scheduled(event, env, ctx) {
    await trigger(env);
  },

  async fetch(request, env) {
    await trigger(env);
    return new Response("triggered at " + new Date().toISOString());
  },
};

async function trigger(env) {
  const token = env.CF_API_TOKEN;
  const account = env.CF_ACCOUNT_ID;
  if (!token || !account) {
    console.error("缺少环境变量 CF_API_TOKEN 或 CF_ACCOUNT_ID");
    return;
  }

  const project = "neuq-classroom-query";
  const url = `https://api.cloudflare.com/client/v4/accounts/${account}/pages/projects/${project}/deployments`;

  console.log(`[${new Date().toISOString()}] 触发 ${project} 生产构建...`);
  try {
    const res = await fetch(url, {
      method: "POST",
      headers: {
        Authorization: `Bearer ${token}`,
        "Content-Type": "application/json",
      },
    });
    const body = await res.text();
    if (res.ok) {
      console.log("已触发构建，HTTP", res.status);
    } else {
      console.error(`触发失败，HTTP ${res.status}: ${body}`);
    }
  } catch (e) {
    console.error("请求 Cloudflare API 出错:", e);
  }
}
