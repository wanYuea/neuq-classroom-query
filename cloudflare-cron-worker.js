// Cloudflare Pages 本身不支持定时构建，用这个 Worker 的 Cron Trigger 打 Deploy Hook。
// 部署方式见 DEPLOY.md。
export default {
  async scheduled(controller, env, ctx) {
    const hook = env.PAGES_DEPLOY_HOOK;
    if (!hook) {
      console.error("未设置环境变量 PAGES_DEPLOY_HOOK");
      return;
    }

    console.log(`[${new Date().toISOString()}] 触发 Pages 部署...`);
    try {
      const res = await fetch(hook, { method: "POST" });
      if (res.ok) {
        console.log("已触发 Pages 部署");
      } else {
        console.error(`触发失败，状态码 ${res.status}: ${await res.text()}`);
      }
    } catch (e) {
      console.error("请求 Deploy Hook 时出错:", e);
    }
  },
};
