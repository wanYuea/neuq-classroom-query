// 东秦空教室速查 · 书签小工具（在教务/WebVPN 页面里点击运行）
// 使用：登录教务后，在 vpn.neuq.edu.cn 任意页面点击该书签
javascript:(() => {
  /* ---- 定位 eams base ---- */
  const path = location.pathname || "";
  const m = path.match(/\/http\/([a-f0-9]+)\/eams/);
  const DEF_ENC = "77726476706e69737468656265737421fae05988693e6d456f468ca88d1b203b";
  if (!m && !location.hostname.includes("vpn") && !location.hostname.includes("jwxt")) {
    alert("请先在浏览器打开东北大学秦皇岛分校教务系统（WebVPN）再使用本工具。");
    return;
  }
  if (!m) {
    // 在 VPN 门户页：跳到教务 eams（会话已登录则直接进入）
    location.href = location.origin + "/http/" + DEF_ENC + "/eams/homeExt.action";
    return;
  }
  const enc = m[1];
  const BASE = location.origin + "/http/" + enc + "/eams/";
  const SLOTS = [
    ["1", "2", "1-2节"],
    ["3", "4", "3-4节"],
    ["5", "6", "5-6节"],
    ["7", "8", "7-8节"],
    ["9", "10", "9-10节"],
    ["11", "12", "11-12节"],
  ];

  /* ---- 轻量 UI ---- */
  if (document.getElementById("nq-query-panel")) {
    document.getElementById("nq-query-panel").remove();
  }
  const panel = document.createElement("div");
  panel.id = "nq-query-panel";
  panel.style.cssText =
    "position:fixed;inset:0;z-index:99999;background:rgba(10,10,30,.45);display:flex;align-items:center;justify-content:center";
  const box = document.createElement("div");
  box.style.cssText =
    "background:#fff;color:#222;width:min(96vw,1080px);max-height:92vh;overflow:auto;border-radius:12px;padding:18px 20px 24px;font:14px/1.6 system-ui,'PingFang SC','Microsoft YaHei',sans-serif;box-shadow:0 10px 40px rgba(0,0,0,.3)";
  box.innerHTML =
    '<div style="display:flex;align-items:center;gap:10px;margin-bottom:6px">' +
    '<span style="font-size:18px;font-weight:700">东秦空教室速查</span>' +
    '<span id="nq-status" style="font-size:12px;color:#888"></span>' +
    '<span style="flex:1"></span>' +
    '<span id="nq-close" style="cursor:pointer;font-size:20px;color:#888" title="关闭">×</span></div>' +
    '<div id="nq-days" style="display:flex;gap:6px;flex-wrap:wrap;margin-bottom:12px"></div>' +
    '<div style="overflow:auto;border:1px solid #e6e6ea;border-radius:8px">' +
    '<table id="nq-table" style="border-collapse:collapse;width:100%;min-width:760px">' +
    "<thead></thead><tbody></tbody></table></div>" +
    '<div style="font-size:11px;color:#999;margin-top:8px">数据来源：教务系统实时查询；仅在你已登录的会话中读取，不上传任何信息。加载失败通常是会话过期，请刷新教务页面重新登录。</div>';
  panel.appendChild(box);
  document.body.appendChild(panel);
  const $ = (id) => document.getElementById(id);
  $("nq-close").onclick = () => panel.remove();
  panel.onclick = (e) => {
    if (e.target === panel) panel.remove();
  };

  /* ---- 日期 ---- */
  const days = [];
  for (let i = 0; i < 7; i++) {
    const d = new Date(Date.now() + i * 86400000);
    days.push(d);
  }
  const fmt = (d) => d.toISOString ? d.toLocaleDateString("zh-CN") : String(d);
  const pad = (n) => String(n).padStart(2, "0");
  const dayStr = (d) => d.getFullYear() + "-" + pad(d.getMonth() + 1) + "-" + pad(d.getDate());
  const wd = ["日", "一", "二", "三", "四", "五", "六"];
  let cur = 0;

  /* ---- 查询一个时段 ---- */
  async function query(date, tb, te) {
    const body = new URLSearchParams();
    body.set("classroom.building.id", "");
    body.set("cycleTime.dateBegin", date);
    body.set("cycleTime.dateEnd", date);
    body.set("timeBegin", tb);
    body.set("timeEnd", te);
    body.set("pageSize", "1000");
    body.set("classroom.type.id", "");
    body.set("classroom.campus.id", "");
    body.set("seats", "");
    body.set("classroom.name", "");
    body.set("cycleTime.cycleCount", "1");
    body.set("cycleTime.cycleType", "1");
    body.set("roomApplyTimeType", "0");
    const r = await fetch(BASE + "classroom/apply/free!search.action", {
      method: "POST",
      headers: { "Content-Type": "application/x-www-form-urlencoded" },
      body: body.toString(),
      credentials: "same-origin",
    });
    const html = await r.text();
    if (html.includes("actionError") || html.includes("登录")) {
      throw new Error("需要登录或会话已过期");
    }
    /* 解析 gridtable */
    const doc = new DOMParser().parseFromString(html, "text/html");
    const table = doc.querySelector("table.gridtable");
    if (!table) return [];
    const thead = table.querySelector("thead");
    const heads = [];
    if (thead) {
      thead.querySelectorAll("th").forEach((th) => heads.push(th.textContent.trim()));
    }
    const rows = [];
    table.querySelectorAll("tbody tr").forEach((tr) => {
      const cells = tr.querySelectorAll("td");
      if (cells.length < 2) return;
      const rec = {};
      heads.forEach((h, i) => {
        if (cells[i]) rec[h] = cells[i].textContent.trim();
      });
      if (!heads.length) {
        rec["教学楼"] = cells[0].textContent.trim();
        rec["名称"] = cells[1].textContent.trim();
      }
      rows.push(rec);
    });
    return rows;
  }

  /* ---- 按楼栋渲染 ---- */
  function render(date) {
    $("nq-table").querySelector("thead").innerHTML = "";
    const tbody = $("nq-table").querySelector("tbody");
    tbody.innerHTML = "";
    $("nq-status").textContent = "查询中…";
    const groups = {}; // slotLabel -> { building -> [names] }
    const jobs = SLOTS.map(async (s) => {
      try {
        const rows = await query(date, s[0], s[1]);
        const g = {};
        rows.forEach((r) => {
          const b = r["教学楼"] || "其他";
          const nm = (r["名称"] || "").replace(new RegExp("^" + b), "").trim();
          if (!nm) return;
          (g[b] = g[b] || []).push(nm);
        });
        groups[s[2]] = g;
      } catch (e) {
        groups[s[2]] = null; // 该时段失败
      }
    });
    Promise.all(jobs).then(() => {
      $("nq-status").textContent = "";
      const thead = $("nq-table").querySelector("thead");
      const tr = document.createElement("tr");
      ["楼栋", ...SLOTS.map((s) => s[2])].forEach((h) => {
        const th = document.createElement("th");
        th.textContent = h;
        th.style.cssText =
          "border:1px solid #e6e6ea;background:#f2f3f7;padding:7px 8px;position:sticky;top:0";
        tr.appendChild(th);
      });
      thead.appendChild(tr);
      const allBuildings = new Set();
      SLOTS.forEach((s) => {
        const g = groups[s[2]];
        if (g) Object.keys(g).forEach((b) => allBuildings.add(b));
      });
      allBuildings.forEach((b) => {
        const rw = document.createElement("tr");
        const t1 = document.createElement("td");
        t1.textContent = b;
        t1.style.cssText = "border:1px solid #e6e6ea;padding:6px 8px;font-weight:600;background:#fafbfc;white-space:nowrap";
        rw.appendChild(t1);
        SLOTS.forEach((s) => {
          const td = document.createElement("td");
          const g = groups[s[2]];
          const list = g && g[b] ? [...g[b]] : [];
          const srt = list.sort((a, b2) => a.localeCompare(b2, "zh-CN-numeric"));
          td.textContent = srt.join(" ");
          td.style.cssText =
            "border:1px solid #e6e6ea;padding:6px 8px;vertical-align:top;font-size:13px";
          if (g === null) td.textContent = "（失败）";
          else if (!list.length) td.textContent = "无";
          rw.appendChild(td);
        });
        tbody.appendChild(rw);
      });
    });
  }

  /* ---- 日期 tab ---- */
  const dayBox = $("nq-days");
  days.forEach((d, i) => {
    const btn = document.createElement("button");
    btn.textContent = d.getMonth() + 1 + "/" + d.getDate() + " 周" + wd[d.getDay()];
    btn.style.cssText =
      "border:1px solid #d6d6de;background:#f6f6fa;border-radius:16px;padding:5px 13px;font-size:13px;cursor:pointer;color:#444";
    btn.onclick = () => {
      cur = i;
      dayBox.querySelectorAll("button").forEach((b) => (b.style.borderColor = "#d6d6de"));
      btn.style.borderColor = "#1B9EF3";
      btn.style.color = "#1B9EF3";
      render(dayStr(days[i]));
    };
    dayBox.appendChild(btn);
    if (i === 0) btn.click();
  });
})();
