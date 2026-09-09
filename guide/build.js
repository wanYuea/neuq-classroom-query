// 生成 guide-site/index.html：读取 bookmarklet.js -> 内嵌书签链接与复制区
const fs = require("fs");
const path = require("path");

const jsPath = path.join(__dirname, "bookmarklet.js");
let code = fs.readFileSync(jsPath, "utf8");
// 去掉开头的注释说明行（// 起始行），保留代码
code = code.replace(/^\/\/.*(?:\r?\n)/, "");
// 规整：去掉空行前的注释说明，压缩成适合书签的形式（保留功能）
const lines = code.split("\n").filter((l) => l.trim().length);
// 保留可读性：不强制单行；把 href 用 URL 编码即可
const codeText = lines.join("\n");

const href = "javascript:" + encodeURIComponent(codeText);

const esc = (s) =>
  s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");

const html = `<!DOCTYPE html>
<html lang="zh-CN">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>东秦空教室速查 · 书签版</title>
<link rel="icon" href="assets/favicon.svg" type="image/svg+xml">
<link rel="stylesheet" href="assets/styles/main.css">
<link rel="stylesheet" href="assets/styles/font.css">
<style>
  body{display:block}
  .guide{max-width:820px;margin:0 auto;padding:26px 20px 70px}
  h1{font-size:26px;margin:18px 0 4px}
  .sub{color:#666;margin-bottom:20px}
  .card{border:1px solid #e3e3ea;border-radius:12px;padding:16px 18px;margin:12px 0;background:#fbfbfd}
  .card .t{font-weight:600;font-size:15px;display:block;margin-bottom:6px}
  .step{display:flex;gap:10px;margin:10px 0;align-items:flex-start}
  .step .no{flex:0 0 22px;height:22px;border-radius:50%;background:#1B9EF3;color:#fff;display:flex;align-items:center;justify-content:center;font-size:12px;margin-top:2px}
  .dlink{display:inline-block;padding:9px 16px;background:#1B9EF3;color:#fff!important;border-radius:8px;text-decoration:none;font-weight:600}
  textarea{width:100%;height:150px;font:11px/1.5 Consolas,Menlo,monospace;border:1px solid #ddd;border-radius:6px;padding:8px;box-sizing:border-box;color:#333}
  .btns{display:flex;gap:8px;margin-top:8px}
  button{padding:6px 14px;border:1px solid #1B9EF3;background:#fff;color:#1B9EF3;border-radius:6px;cursor:pointer}
  .note{font-size:12px;color:#999;margin-top:6px}
  .go{border-top:1px dashed #ddd;margin-top:22px;padding-top:14px}
</style>
</head>
<body>
<main class="guide">
  <h1>东秦空教室速查 · 书签版</h1>
  <p class="sub">在教务网页里点一下书签，立即看到 7 天 × 每个时段的空闲教室总表（与原站一致）</p>

  <div class="card">
    <span class="t">第 1 步 · 安装书签</span>
    <div class="step"><span class="no">1</span><span>把下面按钮<strong>拖到浏览器书签栏</strong>（或右键收藏）：</span></div>
    <a class="dlink" href="${href}">东秦空教室速查</a>
    <div class="step" style="margin-top:14px"><span class="no">2</span><span>或<strong>复制代码</strong>，在书签管理器里「新建书签」，网址栏粘贴：</span></div>
    <textarea readonly spellcheck="false">${esc(codeText)}</textarea>
    <div class="btns"><button onclick="copyCode()">复制代码</button></div>
    <div class="note">提示：若书签栏不可见，按 Ctrl+Shift+B 打开。</div>
  </div>

  <div class="card">
    <span class="t">第 2 步 · 登录教务</span>
    <p>点击下方按钮打开 WebVPN 统一身份认证，用学号 + 密码登录。<b>只登录这一次，有效期内点书签即可。</b></p>
    <a class="dlink" style="background:#2e7d32" target="_blank" rel="noopener"
       href="https://vpn.neuq.edu.cn/">去登录教务 →</a>
  </div>

  <div class="card">
    <span class="t">第 3 步 · 使用</span>
    <p>在已登录的教务/WebVPN 页面（<code>vpn.neuq.edu.cn</code> 开头）点击书签「东秦空教室速查」，
       弹出面板按日期查看各时段空教室。可随时点 × 关闭。</p>
  </div>

  <div class="go note">
    原理说明：学校教务系统仅向境内开放，且不向第三方网页开放跨域读取（CORS）；因此本工具在教务
    页面自身环境内运行——它使用<b>你已登录的会话</b>实时查询，不上传、不存储任何账号信息。
    若弹出「会话过期」，刷新教务页面重新登录即可。
  </div>
</main>
<script>
function copyCode() {
  const t = document.querySelector("textarea");
  t.select();
  document.execCommand("copy");
  const b = event.target;
  b.textContent = "已复制 ✓";
  setTimeout(() => (b.textContent = "复制代码"), 1500);
}
</script>
</body>
</html>`;

fs.writeFileSync(path.join(__dirname, "index.html"), html);
console.log("index.html 已生成, 书签长度:", href.length, "字符");
