use std::path::PathBuf;
use std::time::Duration;

use crate::error::{AppError, Result};

/// 应用程序配置
#[derive(Clone)]
pub struct AppConfig {
    /// 教务系统用户名
    pub username: String,
    /// 教务系统密码
    pub password: String,
    /// 教务系统基础 URL
    pub base_url: String,
    /// 是否先登录 WebVPN 门户（校外访问教务系统时开启）
    pub vpn_enabled: bool,
    /// WebVPN 门户地址（协议+主机，如 https://vpn.neuq.edu.cn）
    pub vpn_base_url: String,
    /// WebVPN 门户用户名（缺省回退到教务用户名）
    pub vpn_username: String,
    /// WebVPN 门户密码（缺省回退到教务密码）
    pub vpn_password: String,
    /// WebVPN 门户认证方式：cas（统一认证）或 local（门户本地账号），默认 cas
    pub vpn_auth_method: String,
    /// 请求超时时间
    pub request_timeout: Duration,
    /// 请求间隔时间
    pub request_delay: Duration,
    /// 总共处理的天数
    pub total_days: u8,
    /// 重试配置
    pub retry_config: crate::error::RetryConfig,
    /// 输出目录
    pub output_dir: PathBuf,
    /// 静态资源目录
    pub assets_dir: PathBuf,
    /// 是否覆盖已有数据
    pub force_overwrite: bool,
    /// 是否压缩 HTML 输出
    pub minify_html: bool,
}

impl std::fmt::Debug for AppConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppConfig")
            .field("username", &self.username)
            .field("password", &"[REDACTED]")
            .field("base_url", &self.base_url)
            .field("vpn_enabled", &self.vpn_enabled)
            .field("vpn_base_url", &self.vpn_base_url)
            .field("vpn_username", &self.vpn_username)
            .field("vpn_password", &"[REDACTED]")
            .field("vpn_auth_method", &self.vpn_auth_method)
            .field("request_timeout", &self.request_timeout)
            .field("request_delay", &self.request_delay)
            .field("total_days", &self.total_days)
            .field("output_dir", &self.output_dir)
            .field("assets_dir", &self.assets_dir)
            .field("force_overwrite", &self.force_overwrite)
            .field("minify_html", &self.minify_html)
            .finish()
    }
}

impl AppConfig {
    /// 从环境变量加载配置
    pub fn from_env() -> Result<Self> {
        let username = std::env::var("YOUR_NEUQ_USERNAME").map_err(|_| {
            AppError::EnvVarMissing {
                name: "YOUR_NEUQ_USERNAME".to_string(),
            }
        })?;

        let password = std::env::var("YOUR_NEUQ_PASSWORD").map_err(|_| {
            AppError::EnvVarMissing {
                name: "YOUR_NEUQ_PASSWORD".to_string(),
            }
        })?;

        let base_url = std::env::var("NEUQ_JWXT_BASE_URL")
            .unwrap_or_else(|_| "https://vpn.neuq.edu.cn/http/77726476706e69737468656265737421fae05988693e6d456f468ca88d1b203b/eams/".to_string());

        // 是否启用 WebVPN 门户登录：可显式配置，未配置时根据 base_url 是否指向 VPN 自动判断
        let vpn_enabled = std::env::var("NEUQ_VPN_ENABLED")
            .map(|v| v == "true" || v == "1")
            .unwrap_or_else(|_| base_url.contains("vpn."));

        // 门户协议+主机，可显式配置，缺省从 base_url 推导
        let vpn_base_url = std::env::var("NEUQ_VPN_BASE_URL").unwrap_or_else(|_| {
            reqwest::Url::parse(&base_url)
                .ok()
                .filter(|u| u.host_str().is_some())
                .map(|u| {
                    format!(
                        "{}://{}{}",
                        u.scheme(),
                        u.host_str().unwrap_or(""),
                        u.port().map(|p| format!(":{}", p)).unwrap_or_default()
                    )
                })
                .unwrap_or_default()
        });

        let vpn_username = std::env::var("NEUQ_VPN_USERNAME").unwrap_or_else(|_| username.clone());
        let vpn_password = std::env::var("NEUQ_VPN_PASSWORD").unwrap_or_else(|_| password.clone());
        let vpn_auth_method = std::env::var("NEUQ_VPN_AUTH_METHOD")
            .unwrap_or_else(|_| "cas".to_string())
            .to_ascii_lowercase();

        let request_timeout_secs: u64 = std::env::var("REQUEST_TIMEOUT_SECS")
            .unwrap_or_else(|_| "45".to_string())
            .parse()
            .map_err(|_| AppError::Config("REQUEST_TIMEOUT_SECS 必须是有效的数字".to_string()))?;

        let request_delay_ms: u64 = std::env::var("REQUEST_DELAY_MS")
            .unwrap_or_else(|_| "2000".to_string())
            .parse()
            .map_err(|_| AppError::Config("REQUEST_DELAY_MS 必须是有效的数字".to_string()))?;

        let total_days: u8 = std::env::var("TOTAL_DAYS")
            .unwrap_or_else(|_| "7".to_string())
            .parse()
            .map_err(|_| AppError::Config("TOTAL_DAYS 必须是 1-30 之间的数字".to_string()))?;

        if !(1..=30).contains(&total_days) {
            return Err(AppError::Config(
                "TOTAL_DAYS 必须在 1 到 30 之间".to_string(),
            ));
        }

        let max_retries: u32 = std::env::var("MAX_RETRIES")
            .unwrap_or_else(|_| "3".to_string())
            .parse()
            .map_err(|_| AppError::Config("MAX_RETRIES 必须是有效的数字".to_string()))?;

        let current_dir = std::env::current_dir()
            .map_err(|e| AppError::Config(format!("无法获取当前目录: {}", e)))?;

        let output_dir = std::env::var("OUTPUT_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| current_dir.join("output"));

        let assets_dir = std::env::var("ASSETS_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| current_dir.join("assets"));

        let force_overwrite = std::env::var("FORCE_OVERWRITE")
            .map(|v| v == "true" || v == "1")
            .unwrap_or(false);

        let minify_html = std::env::var("MINIFY_HTML")
            .map(|v| v == "true" || v == "1")
            .unwrap_or(true); // 默认启用压缩

        Ok(Self {
            username,
            password,
            base_url,
            vpn_enabled,
            vpn_base_url,
            vpn_username,
            vpn_password,
            vpn_auth_method,
            request_timeout: Duration::from_secs(request_timeout_secs),
            request_delay: Duration::from_millis(request_delay_ms),
            total_days,
            retry_config: crate::error::RetryConfig {
                max_retries,
                initial_delay: Duration::from_secs(1),
                max_delay: Duration::from_secs(30),
                backoff_factor: 2.0,
            },
            output_dir,
            assets_dir,
            force_overwrite,
            minify_html,
        })
    }
}

/// 时段配置
#[derive(Debug, Clone, Copy)]
pub struct TimeSlot {
    pub begin: u8,
    pub end: u8,
    pub file_suffix: &'static str,
    pub label: &'static str,
}

/// 预定义的查询时段
pub const TIME_SLOTS: &[TimeSlot] = &[
    TimeSlot {
        begin: 1,
        end: 2,
        file_suffix: "1-2",
        label: "上午第1-2节",
    },
    TimeSlot {
        begin: 3,
        end: 4,
        file_suffix: "3-4",
        label: "上午第3-4节",
    },
    TimeSlot {
        begin: 5,
        end: 6,
        file_suffix: "5-6",
        label: "下午第5-6节",
    },
    TimeSlot {
        begin: 7,
        end: 8,
        file_suffix: "7-8",
        label: "下午第7-8节",
    },
    TimeSlot {
        begin: 1,
        end: 8,
        file_suffix: "1-8",
        label: "昼间第1-8节",
    },
    TimeSlot {
        begin: 9,
        end: 10,
        file_suffix: "9-10",
        label: "晚上第9-10节",
    },
    TimeSlot {
        begin: 11,
        end: 12,
        file_suffix: "11-12",
        label: "晚上第11-12节",
    },
];

/// 教学楼配置
#[derive(Debug, Clone)]
pub struct BuildingConfig {
    pub name: &'static str,
    pub code: &'static str,
    pub has_floors: bool,
    pub floor_count: u8,
}

/// 预定义的教学楼配置
pub const BUILDINGS: &[BuildingConfig] = &[
    BuildingConfig {
        name: "工学馆",
        code: "GXG",
        has_floors: true,
        floor_count: 7,
    },
    BuildingConfig {
        name: "基础楼",
        code: "JCL",
        has_floors: false,
        floor_count: 0,
    },
    BuildingConfig {
        name: "综合实验楼",
        code: "ZHSYL",
        has_floors: false,
        floor_count: 0,
    },
    BuildingConfig {
        name: "地质楼",
        code: "DZL",
        has_floors: false,
        floor_count: 0,
    },
    BuildingConfig {
        name: "管理楼",
        code: "GLL",
        has_floors: false,
        floor_count: 0,
    },
    BuildingConfig {
        name: "科技楼",
        code: "KJL",
        has_floors: false,
        floor_count: 0,
    },
    BuildingConfig {
        name: "人文楼",
        code: "RWL",
        has_floors: false,
        floor_count: 0,
    },
];

/// 禁止的教室设备配置类型
pub const FORBIDDEN_CONFIGS: &[&str] = &[
    "体育教学场地",
    "机房",
    "实验室",
    "活动教室",
    "研讨室",
    "多功能",
    "智慧教室",
    "不排课教室",
    "语音室",
];

/// 禁止的教学楼
pub const FORBIDDEN_BUILDINGS: &[&str] = &["大学会馆", "旧实验楼"];

/// 顺序时段（用于计算全天空闲）
pub const SEQUENTIAL_SLOTS: &[&str] = &["1-2", "3-4", "5-6", "7-8", "9-10", "11-12"];

/// 适用删除线逻辑的时段
pub const STRIKETHROUGH_APPLICABLE_SLOTS: &[&str] = &["1-2", "3-4", "5-6", "7-8", "9-10"];

/// 时段标签映射
pub const TIME_SLOT_LABELS: &[(&str, &str)] = &[
    ("1-2", "上午第1-2节"),
    ("3-4", "上午第3-4节"),
    ("5-6", "下午第5-6节"),
    ("7-8", "下午第7-8节"),
    ("1-8", "昼间第1-8节"),
    ("9-10", "晚上第9-10节"),
    ("11-12", "晚上第11-12节"),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_time_slots() {
        assert_eq!(TIME_SLOTS.len(), 7);
        assert_eq!(TIME_SLOTS[0].file_suffix, "1-2");
        assert_eq!(TIME_SLOTS[4].file_suffix, "1-8");
    }

    #[test]
    fn test_buildings() {
        assert_eq!(BUILDINGS.len(), 7);
        assert!(BUILDINGS[0].has_floors);
        assert!(!BUILDINGS[1].has_floors);
    }

    #[test]
    fn test_forbidden_lists() {
        assert!(FORBIDDEN_CONFIGS.contains(&"机房"));
        assert!(FORBIDDEN_BUILDINGS.contains(&"大学会馆"));
    }
}
