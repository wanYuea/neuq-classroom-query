use std::sync::Arc;
use std::time::Duration;

use reqwest::Client;
use reqwest_cookie_store::CookieStoreMutex;
use scraper::{Html, Selector};

use crate::config::AppConfig;
use crate::error::{AppError, Result, with_retry};

/// HTTP 客户端封装，支持 cookie 管理和重试
#[derive(Clone)]
pub struct HttpClient {
    client: Client,
    #[allow(dead_code)]
    cookie_store: Arc<CookieStoreMutex>,
    config: Arc<AppConfig>,
}

impl HttpClient {
    /// 创建新的 HTTP 客户端（优化版本）
    pub fn new(config: Arc<AppConfig>) -> Result<Self> {
        let cookie_store = Arc::new(CookieStoreMutex::new(
            reqwest_cookie_store::CookieStore::default(),
        ));

        let client = Client::builder()
            .timeout(config.request_timeout)
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/108.0.0.0 Safari/537.36")
            .cookie_provider(Arc::clone(&cookie_store))
            // 启用 gzip 和 brotli 压缩（减少传输数据量）
            .gzip(true)
            .brotli(true)
            // 连接池配置（性能优化）
            .pool_max_idle_per_host(10)
            .pool_idle_timeout(Duration::from_secs(90))
            // TCP 优化
            .tcp_keepalive(Duration::from_secs(60))
            .tcp_nodelay(true)
            // 连接超时
            .connect_timeout(Duration::from_secs(10))
            .build()
            .map_err(|e| AppError::Config(format!("创建 HTTP 客户端失败: {}", e)))?;

        Ok(Self {
            client,
            cookie_store,
            config,
        })
    }

    /// 获取登录页面并提取 salt
    pub async fn get_salt(&self) -> Result<String> {
        let url = format!("{}loginExt.action", self.config.base_url);

        let response = self.client.get(&url).send().await?;

        if !response.status().is_success() {
            return Err(AppError::Fetch {
                message: format!("获取登录页面失败，状态码: {}", response.status()),
            });
        }

        let html = response.text().await?;
        let document = Html::parse_document(&html);

        // 查找包含 salt 的 script 标签
        let script_selector = Selector::parse("script").map_err(|e| {
            AppError::Other(format!("解析 script 选择器失败: {:?}", e))
        })?;

        for script in document.select(&script_selector) {
            if let Some(text) = script.text().next()
                && let Some(salt) = extract_salt_from_script(text) {
                    return Ok(salt);
                }
        }

        Err(AppError::SaltExtractionFailed)
    }

    /// 登录教务系统
    pub async fn login(&self, username: &str, password: &str) -> Result<()> {
        use sha1::{Digest, Sha1};

        // 获取 salt
        let salt = with_retry(
            &self.config.retry_config,
            || async { self.get_salt().await },
            "获取 salt",
        )
        .await?;

        // 获取 salt 后等待片刻再提交，避免触发"请不要过快点击"风控；
        // VPN 场景下间隔取配置的请求间隔（默认更长）
        let post_salt_delay = if self.config.vpn_enabled {
            self.config.request_delay
        } else {
            Duration::from_secs(1)
        };
        tokio::time::sleep(post_salt_delay).await;

        // 计算密码哈希
        let password_with_salt = format!("{}-{}", salt, password);
        let mut hasher = Sha1::new();
        hasher.update(password_with_salt.as_bytes());
        let hashed_password = hex::encode(hasher.finalize());

        // 发送登录请求
        let url = format!("{}loginExt.action", self.config.base_url);
        let params = [
            ("username", username),
            ("password", &hashed_password),
        ];

        let response = self
            .client
            .post(&url)
            .form(&params)
            .send()
            .await?;

        // 检查登录是否成功（通过重定向判断）
        let final_url = response.url().to_string();
        if final_url.contains("homeExt.action") {
            tracing::debug!("登录成功");
            Ok(())
        } else {
            tracing::error!("登录失败，URL: {}", final_url);
            Err(AppError::LoginFailed {
                reason: "登录失败，未重定向到主页。请检查用户名或密码。".to_string(),
            })
        }
    }

    /// 登录 WebVPN 门户，按配置的认证方式选择 CAS 统一认证或门户本地账号
    pub async fn login_portal(&self) -> Result<()> {
        match self.config.vpn_auth_method.as_str() {
            "local" => self.login_portal_local().await,
            _ => self.login_portal_cas().await,
        }
    }

    /// 使用门户本地账号登录（auth_type=local），多数学校账号库不含校内账号，默认不启用
    ///
    /// 流程：/login 获取 ticket cookie → 提交占位指纹 /set-fingerprint
    /// → 拉取登录页提取 _csrf/captcha_id → POST /do-login。
    async fn login_portal_local(&self) -> Result<()> {
        let origin = self.config.vpn_base_url.trim_end_matches('/').to_string();
        if origin.is_empty() {
            return Ok(());
        }

        tracing::debug!("WebVPN 门户登录开始: {}", origin);

        // 1) 访问 /login 获取初始 ticket cookie（自动跟随跳转到 /fingerprint）
        let resp = self.client.get(format!("{}/login", origin)).send().await?;
        if !resp.status().is_success() {
            return Err(AppError::Fetch {
                message: format!("访问 WebVPN 门户失败，状态码: {}", resp.status()),
            });
        }

        // 2) 提交占位指纹，服务端校验通过后即可进入真实登录页
        let fp_url = format!(
            "{}/set-fingerprint?fingerprint=00000000-0000-4000-8000-000000000000",
            origin
        );
        self.client.get(&fp_url).send().await?;

        // 3) 拉取登录页，提取 _csrf / needCaptcha / captcha_id
        let resp = self.client.get(format!("{}/login", origin)).send().await?;
        if !resp.status().is_success() {
            return Err(AppError::Fetch {
                message: format!("获取 WebVPN 登录页失败，状态码: {}", resp.status()),
            });
        }
        let html = resp.text().await?;

        let get_input = |name: &str| -> Option<String> {
            let selector = Selector::parse(&format!("input[name=\"{}\"]", name)).ok()?;
            let document = Html::parse_document(&html);
            document
                .select(&selector)
                .next()
                .and_then(|el| el.value().attr("value").map(|v| v.to_string()))
        };

        let csrf = get_input("_csrf").ok_or_else(|| AppError::PortalLoginFailed {
            reason: "登录页中未找到 _csrf 字段，页面结构可能已变化".to_string(),
        })?;
        let need_captcha = get_input("needCaptcha").unwrap_or_else(|| "false".to_string()) == "true";
        let captcha_id = get_input("captcha_id").unwrap_or_default();

        if need_captcha {
            return Err(AppError::PortalLoginFailed {
                reason: "门户要求图形验证码（或尝试过于频繁被风控），自动化登录暂不支持".to_string(),
            });
        }

        // 4) 提交登录表单（网瑞达 WebVPN 的 /do-login 接收明文密码）
        let params = [
            ("_csrf", csrf.as_str()),
            ("auth_type", "local"),
            ("username", self.config.vpn_username.as_str()),
            ("password", self.config.vpn_password.as_str()),
            ("needCaptcha", "false"),
            ("captcha_id", captcha_id.as_str()),
            ("captcha", ""),
        ];

        let resp = self
            .client
            .post(format!("{}/do-login", origin))
            .form(&params)
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(AppError::Fetch {
                message: format!("WebVPN 登录请求失败，状态码: {}", resp.status()),
            });
        }

        let json: serde_json::Value = resp.json().await?;
        if json.get("success").and_then(|v| v.as_bool()) == Some(true) {
            tracing::info!("✔ WebVPN 门户登录成功");
            return Ok(());
        }

        let code = json
            .get("error")
            .and_then(|v| v.as_str())
            .unwrap_or("UNKNOWN");
        let message = json.get("message").and_then(|v| v.as_str()).unwrap_or("");
        Err(AppError::PortalLoginFailed {
            reason: portal_error_reason(code, message),
        })
    }

    /// 通过学校统一认证（wisedu CAS）登录 WebVPN 门户
    async fn login_portal_cas(&self) -> Result<()> {
        let origin = self.config.vpn_base_url.trim_end_matches('/').to_string();
        if origin.is_empty() {
            return Ok(());
        }

        tracing::debug!("WebVPN CAS 统一认证登录开始: {}", origin);

        // 1) 访问 /login，获得门户 ticket cookie
        let resp = self.client.get(format!("{}/login", origin)).send().await?;
        if !resp.status().is_success() {
            return Err(AppError::Fetch {
                message: format!("访问 WebVPN 门户失败，状态码: {}", resp.status()),
            });
        }

        // 2) 提交占位指纹，通过设备检查
        let fp_url = format!(
            "{}/set-fingerprint?fingerprint=00000000-0000-4000-8000-000000000000",
            origin
        );
        self.client.get(&fp_url).send().await?;

        // 3) 走 CAS 统一认证（登录后本会话会保留 SSO 票据，供 eams 等应用自动放行）
        self.cas_authenticate(&format!("{}/login?cas_login=true", origin), "WebVPN 门户")
            .await
    }

    /// 教务系统（eams）登录：VPN+CAS 场景走统一认证 SSO，否则走传统的 salt+SHA1 本地登录
    pub async fn login_eams(&self) -> Result<()> {
        if self.config.vpn_enabled && self.config.vpn_auth_method != "local" {
            self.login_eams_cas().await
        } else {
            self.login(&self.config.username, &self.config.password).await
        }
    }

    /// 通过统一认证（CAS）登录教务系统 eams（校外 VPN 场景）
    ///
    /// eams 登录页的“从统一身份认证登录”入口会跳转到 homeExt.action，
    /// 未登录时服务端将其 302 到统一认证并按 service 回跳。若本会话已有
    /// SSO 票据则自动放行，否则需要提交一次凭据。
    async fn login_eams_cas(&self) -> Result<()> {
        let base = self.config.base_url.trim_end_matches('/').to_string();
        if base.is_empty() {
            return Err(AppError::PortalLoginFailed {
                reason: "未配置教务系统地址".to_string(),
            });
        }

        tracing::debug!("教务系统 eams CAS 登录开始: {}", base);
        // 给风控留缓冲，避免“请不要过快点击”
        tokio::time::sleep(self.config.request_delay).await;
        self.cas_authenticate(&format!("{}/homeExt.action", base), "教务系统 eams")
            .await
    }

    /// 统一认证通用流程：访问入口 URL（自动跟随重定向）。若落到 CAS 登录页
    /// 则解析表单并提交凭据；若已被 SSO 自动放行则直接返回。
    async fn cas_authenticate(&self, entry_url: &str, label: &str) -> Result<()> {
        let resp = self.client.get(entry_url).send().await?;
        if !resp.status().is_success() {
            return Err(AppError::Fetch {
                message: format!("{} 登录入口请求失败，状态码: {}", label, resp.status()),
            });
        }
        let page_url = resp.url().clone();
        let html = resp.text().await?;

        // 已被 SSO（会话票据）自动放行，无需再次输入凭据
        if !html.contains("pwdDefaultEncryptSalt") {
            tracing::info!("✔ {} CAS 统一认证登录成功（SSO）", label);
            return Ok(());
        }

        // 需要凭据：解析 CAS 登录页字段（同步块，避免 Html 等非 Send 值跨 await）
        let (action, lt, execution, event_id, rm_shown, dllt, salt) = {
            let document = Html::parse_document(&html);

            let get_input_by_name = |name: &str| -> Option<String> {
                let selector = Selector::parse(&format!("input[name=\"{}\"]", name)).ok()?;
                document
                    .select(&selector)
                    .next()
                    .and_then(|el| el.value().attr("value").map(str::to_string))
            };
            let get_input_by_id = |id: &str| -> Option<String> {
                let selector = Selector::parse(&format!("input#{}", id)).ok()?;
                document
                    .select(&selector)
                    .next()
                    .and_then(|el| el.value().attr("value").map(str::to_string))
            };

            let action = Selector::parse("form#casLoginForm")
                .ok()
                .and_then(|sel| document.select(&sel).next())
                .and_then(|el| el.value().attr("action").map(str::to_string))
                .ok_or_else(|| AppError::PortalLoginFailed {
                    reason: "CAS 登录页缺少登录表单，页面结构可能已变化".to_string(),
                })?;
            let lt = get_input_by_name("lt").unwrap_or_default();
            let execution = get_input_by_name("execution").unwrap_or_default();
            let event_id = get_input_by_name("_eventId").unwrap_or_else(|| "submit".to_string());
            let rm_shown = get_input_by_name("rmShown").unwrap_or_else(|| "1".to_string());
            let dllt = get_input_by_name("dllt")
                .unwrap_or_else(|| "userNamePasswordLogin".to_string());
            let salt = get_input_by_id("pwdDefaultEncryptSalt").ok_or_else(|| {
                AppError::PortalLoginFailed {
                    reason: "CAS 登录页缺少 pwdDefaultEncryptSalt，页面结构可能已变化".to_string(),
                }
            })?;

            if lt.is_empty() || execution.is_empty() {
                return Err(AppError::PortalLoginFailed {
                    reason: "CAS 登录页缺少 lt/execution 字段，页面结构可能已变化".to_string(),
                });
            }
            if salt.len() != 16 {
                return Err(AppError::PortalLoginFailed {
                    reason: "CAS 加密盐长度异常，页面结构可能已变化".to_string(),
                });
            }

            (action, lt, execution, event_id, rm_shown, dllt, salt)
        };

        // 提交凭据（wisedu：明文=随机64字符+密码；AES-128-CBC，key=salt）
        let action_url = if action.starts_with("http") {
            action
        } else {
            reqwest::Url::parse(&page_url.to_string())
                .and_then(|u| u.join(&action).map(|j| j.to_string()))
                .unwrap_or_else(|_| page_url.to_string())
        };
        let enc_password = cas_encrypt_password(&self.config.vpn_password, &salt);
        let resp = self
            .client
            .post(action_url)
            .form(&[
                ("username", self.config.vpn_username.as_str()),
                ("password", enc_password.as_str()),
                ("lt", lt.as_str()),
                ("dllt", dllt.as_str()),
                ("execution", execution.as_str()),
                ("_eventId", event_id.as_str()),
                ("rmShown", rm_shown.as_str()),
            ])
            .send()
            .await?;

        // 成功会跳离 CAS 登录页（回到目标应用）；失败会停留在 CAS 登录页
        let success = resp.status().is_success();
        let final_url = resp.url().clone();
        let final_str = final_url.as_str().to_string();
        let body = resp.text().await.unwrap_or_default();
        if final_str.contains("/authserver/login") || body.contains("pwdDefaultEncryptSalt") {
            return Err(AppError::PortalLoginFailed {
                reason: extract_cas_error(&body),
            });
        }
        if !success {
            return Err(AppError::Fetch {
                message: format!("{} CAS 登录请求未成功", label),
            });
        }

        tracing::info!("✔ {} CAS 统一认证登录成功", label);
        Ok(())
    }

    /// 查询空闲教室
    pub async fn search_free_classrooms(
        &self,
        date: &str,
        time_begin: u8,
        time_end: u8,
    ) -> Result<Vec<RawClassroomEntry>> {
        let url = format!(
            "{}classroom/apply/free!search.action",
            self.config.base_url
        );

        let params = [
            ("classroom.building.id", ""),
            ("cycleTime.dateBegin", date),
            ("cycleTime.dateEnd", date),
            ("timeBegin", &time_begin.to_string()),
            ("timeEnd", &time_end.to_string()),
            ("pageSize", "1000"),
            ("classroom.type.id", ""),
            ("classroom.campus.id", ""),
            ("seats", ""),
            ("classroom.name", ""),
            ("cycleTime.cycleCount", "1"),
            ("cycleTime.cycleType", "1"),
            ("roomApplyTimeType", "0"),
        ];

        let response = self.client.post(&url).form(&params).send().await?;

        if !response.status().is_success() {
            return Err(AppError::Fetch {
                message: format!(
                    "查询空闲教室失败，状态码: {}",
                    response.status()
                ),
            });
        }

        let html = response.text().await?;
        self.parse_classroom_html(&html)
    }

    /// 解析教室查询返回的 HTML
    fn parse_classroom_html(&self, html: &str) -> Result<Vec<RawClassroomEntry>> {
        let document = Html::parse_document(html);
        let mut results = Vec::new();

        // 查找表格
        let table_selector = Selector::parse("table.gridtable").map_err(|e| {
            AppError::Other(format!("解析表格选择器失败: {:?}", e))
        })?;

        let table = match document.select(&table_selector).next() {
            Some(t) => t,
            None => {
                tracing::warn!("未找到结果表格");
                return Ok(results);
            }
        };

        // 获取表头
        let th_selector = Selector::parse("thead th").map_err(|e| {
            AppError::Other(format!("解析表头选择器失败: {:?}", e))
        })?;

        let headers: Vec<String> = table
            .select(&th_selector)
            .map(|th| th.text().collect::<String>().trim().to_string())
            .collect();

        // 获取数据行
        let tr_selector = Selector::parse("tbody tr").map_err(|e| {
            AppError::Other(format!("解析行选择器失败: {:?}", e))
        })?;

        let td_selector = Selector::parse("td").map_err(|e| {
            AppError::Other(format!("解析单元格选择器失败: {:?}", e))
        })?;

        for row in table.select(&tr_selector) {
            let mut entry = serde_json::Map::new();

            for (j, cell) in row.select(&td_selector).enumerate() {
                let header = headers
                    .get(j)
                    .cloned()
                    .unwrap_or_else(|| format!("column{}", j + 1));
                let value = cell.text().collect::<String>().trim().to_string();
                entry.insert(header, serde_json::Value::String(value));
            }

            if !entry.is_empty() {
                results.push(RawClassroomEntry(entry));
            }
        }

        Ok(results)
    }

    /// 延迟一段时间
    pub async fn delay(&self) {
        tokio::time::sleep(self.config.request_delay).await;
    }
}

/// 原始教室条目（从 HTML 解析）
#[derive(Debug, Clone)]
pub struct RawClassroomEntry(pub serde_json::Map<String, serde_json::Value>);

impl RawClassroomEntry {
    /// 获取字段值
    pub fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).and_then(|v| v.as_str())
    }

    /// 转换为 RawClassroomData
    pub fn to_raw_classroom_data(&self, time_slot: String) -> Result<crate::models::RawClassroomData> {
        let building = self.get("教学楼").unwrap_or("").to_string();
        let name = self.get("名称").unwrap_or("").to_string();
        let capacity = self.get("容量").unwrap_or("0").to_string();
        let equipment_config = self.get("教室设备配置").unwrap_or("").to_string();
        let campus = self.get("校区").map(|s| s.to_string());
        let sequence_number = self.get("序号").map(|s| s.to_string());

        Ok(crate::models::RawClassroomData {
            building,
            name,
            capacity,
            equipment_config,
            campus,
            sequence_number,
            time_slot: Some(time_slot),
            extra: HashMap::new(),
        })
    }
}

use std::collections::HashMap;

/// 从 script 文本中提取 salt
fn extract_salt_from_script(text: &str) -> Option<String> {
    // 匹配模式: CryptoJS.SHA1('salt_value-' + form['password'].value)
    let re = regex::Regex::new(r"CryptoJS\.SHA1\('([^']+)-' \+ form\['password'\]\.value\)").ok()?;

    re.captures(text)
        .and_then(|caps| caps.get(1))
        .map(|m| m.as_str().to_string())
}

/// 将 WebVPN 门户登录返回的错误码转换为可读信息
fn portal_error_reason(code: &str, message: &str) -> String {
    let fallback = if message.is_empty() {
        format!("错误码: {}", code)
    } else {
        format!("{} (错误码: {})", message, code)
    };
    match code {
        "INVALID_ACCOUNT" => "账号或密码错误".to_string(),
        "CAPTCHA_FAILED" => "图形验证码校验失败，请稍后重试".to_string(),
        "TOO_MANY_ATTEMPTS" => "尝试次数过多触发风控，请过一段时间再试".to_string(),
        "IP_FORBIDDEN" => "当前 IP 被门户禁止访问".to_string(),
        "WEEK_PASSWORD_FORBID" => "弱密码禁止登录，请先在门户修改密码".to_string(),
        "NEED_CONFIRM" | "NEED_TWO_STEP" | "NEED_TWO_STEP_TOTP" => {
            "需要二次验证（短信/动态码），自动化登录暂不支持".to_string()
        }
        "WECHAT_BINDING" => "需要微信扫码/绑定，自动化登录暂不支持".to_string(),
        _ => fallback,
    }
}

/// 从 CAS 失败页中提取可读的错误原因
fn extract_cas_error(html: &str) -> String {
    const KNOWN: &[&str] = &[
        "用户名或密码错误",
        "密码错误",
        "账号已锁定",
        "账户已被锁定",
        "验证码错误",
        "请输入验证码",
        "验证码不能为空",
        "尝试次数",
        "错误次数",
        "操作频繁",
        "登录风险",
        "认证失败",
    ];
    for k in KNOWN {
        if html.contains(k) {
            return k.to_string();
        }
    }
    "统一认证登录失败（详情以学校认证页提示为准）".to_string()
}

/// wisedu CAS 密码加密字符集（与页面 randomString 一致，去掉了易混淆字符）
const CAS_RANDOM_CHARS: &[u8] = b"ABCDEFGHJKMNPQRSTWXYZabcdefhijkmnprstwxyz2345678";

/// 生成指定长度的 CAS 随机串
fn cas_random_string(len: usize) -> String {
    use rand::Rng;
    let mut out = String::with_capacity(len);
    let mut rng = rand::thread_rng();
    for _ in 0..len {
        let idx = rng.gen_range(0..CAS_RANDOM_CHARS.len());
        out.push(CAS_RANDOM_CHARS[idx] as char);
    }
    out
}

/// AES-128-CBC + PKCS7 加密
fn aes128_cbc_pkcs7_encrypt(plain: &[u8], key: &[u8], iv: &[u8]) -> Vec<u8> {
    use aes::cipher::generic_array::{typenum::U16, GenericArray};
    use aes::cipher::{BlockEncrypt, KeyInit};
    use aes::Aes128;

    let cipher = Aes128::new(GenericArray::from_slice(key));

    // PKCS7 填充
    let pad_len = 16 - (plain.len() % 16);
    let mut data = plain.to_vec();
    data.extend(std::iter::repeat(pad_len as u8).take(pad_len));

    let mut out = Vec::with_capacity(data.len());
    let mut prev = GenericArray::<u8, U16>::clone_from_slice(iv);
    for chunk in data.chunks(16) {
        let mut block = GenericArray::<u8, U16>::clone_from_slice(chunk);
        for (b, p) in block.iter_mut().zip(prev.iter()) {
            *b ^= *p;
        }
        cipher.encrypt_block(&mut block);
        out.extend_from_slice(&block);
        prev = block;
    }
    out
}

/// 按 wisedu 方案加密密码并返回 Base64
///
/// 与统一认证登录页 encrypt.wisedu.js 保持一致：
/// 明文 = 随机 64 字符 + 密码，key = pwdDefaultEncryptSalt（16 字符 → AES-128），
/// iv = 随机 16 字符，AES-128-CBC + PKCS7，输出 Base64。
fn cas_encrypt_password(password: &str, salt: &str) -> String {
    use base64::Engine;
    use base64::engine::general_purpose::STANDARD;

    let plain = format!("{}{}", cas_random_string(64), password);
    let iv = cas_random_string(16);
    let ciphertext = aes128_cbc_pkcs7_encrypt(plain.as_bytes(), salt.as_bytes(), iv.as_bytes());
    STANDARD.encode(ciphertext)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_salt() {
        let script = r#"
            var password = CryptoJS.SHA1('abc123-' + form['password'].value);
        "#;
        let salt = extract_salt_from_script(script);
        assert_eq!(salt, Some("abc123".to_string()));
    }

    #[test]
    fn test_extract_salt_no_match() {
        let script = r#"var x = 1;"#;
        let salt = extract_salt_from_script(script);
        assert_eq!(salt, None);
    }

    #[test]
    fn test_raw_classroom_entry() {
        let mut map = serde_json::Map::new();
        map.insert("教学楼".to_string(), serde_json::Value::String("工学馆".to_string()));
        map.insert("名称".to_string(), serde_json::Value::String("101".to_string()));
        map.insert("容量".to_string(), serde_json::Value::String("60".to_string()));
        map.insert("教室设备配置".to_string(), serde_json::Value::String("普通教室".to_string()));

        let entry = RawClassroomEntry(map);
        assert_eq!(entry.get("教学楼"), Some("工学馆"));
        assert_eq!(entry.get("名称"), Some("101"));

        let data = entry.to_raw_classroom_data("1-2".to_string()).unwrap();
        assert_eq!(data.building, "工学馆");
        assert_eq!(data.time_slot, Some("1-2".to_string()));
    }

    #[test]
    fn test_cas_encrypt_password_shape() {
        // 密码长度 11：明文 64+11=75 → PKCS7 填充到 80 字节（5 块）
        // → base64 长度 4*ceil(80/3)=108
        let enc1 = cas_encrypt_password("abcdefghijk", "0123456789abcdef");
        let enc2 = cas_encrypt_password("abcdefghijk", "0123456789abcdef");
        assert_eq!(enc1.len(), 108);
        // 每次随机前缀/IV，输出应不同
        assert_ne!(enc1, enc2);
        // 必须是合法 base64
        use base64::Engine;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(&enc1)
            .expect("base64 解码失败");
        assert_eq!(bytes.len(), 80);
    }
}
