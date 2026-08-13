//! Codex OAuth Authentication Module
//!
//! 实现 OpenAI ChatGPT Plus/Pro 订阅的 OAuth Device Code 流程。
//! 支持多账号管理，每个 Provider 可关联不同的 ChatGPT 账号。
//!
//! ## 认证流程
//! 1. 启动 Device Code 流程，获取 device_auth_id 和 user_code
//! 2. 用户在浏览器中完成 ChatGPT 授权
//! 3. 轮询获取 authorization_code 和 code_verifier（注意：verifier 由服务端返回）
//! 4. 使用 code + verifier 换取 access_token + refresh_token + id_token
//! 5. 自动刷新 access_token（到期前 60 秒）
//!
//! ## 多账号支持
//! - 每个 ChatGPT 账号独立存储 refresh_token
//! - Provider 通过 meta.authBinding 关联账号（auth_provider = "codex_oauth"）
//! - 通过 JWT id_token 提取 chatgpt_account_id 作为账号唯一标识

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};
use tokio::sync::{Mutex, RwLock};

use super::copilot_auth::{GitHubAccount, GitHubDeviceCodeResponse};

/// OpenAI OAuth 客户端 ID（OpenCode 使用，与官方 Codex CLI 相同）
const CODEX_CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";

/// Device Code 启动 URL
const DEVICE_AUTH_USERCODE_URL: &str = "https://auth.openai.com/api/accounts/deviceauth/usercode";

/// Device Code 轮询 URL
const DEVICE_AUTH_TOKEN_URL: &str = "https://auth.openai.com/api/accounts/deviceauth/token";

/// OAuth Token URL（用于 code 换 token 和 refresh token）
const OAUTH_TOKEN_URL: &str = "https://auth.openai.com/oauth/token";

/// Device Code 验证 URL（向用户展示）
const DEVICE_VERIFICATION_URL: &str = "https://auth.openai.com/codex/device";

/// Device Code 流程的 redirect_uri（OpenAI 服务端约定）
const DEVICE_REDIRECT_URI: &str = "https://auth.openai.com/deviceauth/callback";

/// Token 刷新提前量（毫秒）
const TOKEN_REFRESH_BUFFER_MS: i64 = 60_000;

/// Device Code 默认有效时长（秒），OpenAI 文档约定 15 分钟
const DEVICE_CODE_DEFAULT_EXPIRES_IN: u64 = 900;

/// 轮询间隔安全余量（秒）
const POLLING_SAFETY_MARGIN_SECS: u64 = 3;

/// User-Agent
const CODEX_USER_AGENT: &str = "cc-switch-codex-oauth";

/// Codex OAuth 错误
#[derive(Debug, thiserror::Error)]
pub enum CodexOAuthError {
    #[error("等待用户授权中")]
    AuthorizationPending,

    #[error("用户拒绝授权")]
    AccessDenied,

    #[error("Device Code 已过期")]
    ExpiredToken,

    #[error("OAuth Token 获取失败: {0}")]
    TokenFetchFailed(String),

    #[error("Refresh Token 失效或已过期")]
    RefreshTokenInvalid,

    #[error("网络错误: {0}")]
    NetworkError(String),

    #[error("解析错误: {0}")]
    ParseError(String),

    #[error("IO 错误: {0}")]
    IoError(String),

    #[error("账号不存在: {0}")]
    AccountNotFound(String),
}

impl From<reqwest::Error> for CodexOAuthError {
    fn from(err: reqwest::Error) -> Self {
        CodexOAuthError::NetworkError(err.to_string())
    }
}

impl From<std::io::Error> for CodexOAuthError {
    fn from(err: std::io::Error) -> Self {
        CodexOAuthError::IoError(err.to_string())
    }
}

/// OpenAI Device Code 响应
#[derive(Debug, Clone, Deserialize)]
struct DeviceCodeResponse {
    device_auth_id: String,
    user_code: String,
    #[serde(default)]
    interval: Option<serde_json::Value>,
    #[serde(default)]
    expires_in: Option<u64>,
}

/// OpenAI Device Code 轮询响应（成功）
#[derive(Debug, Clone, Deserialize)]
struct DevicePollSuccess {
    authorization_code: String,
    code_verifier: String,
}

/// OAuth Token 响应
#[derive(Debug, Clone, Deserialize)]
struct OAuthTokenResponse {
    access_token: String,
    refresh_token: Option<String>,
    #[serde(default)]
    id_token: Option<String>,
    #[serde(default)]
    expires_in: Option<i64>,
}

/// 解析后的 JWT claims（仅关心 chatgpt_account_id 等字段）
#[derive(Debug, Clone, Default, Deserialize)]
struct IdTokenClaims {
    #[serde(default)]
    sub: Option<String>,
    #[serde(default)]
    exp: Option<i64>,
    #[serde(default)]
    chatgpt_account_id: Option<String>,
    #[serde(default)]
    email: Option<String>,
    #[serde(default)]
    organizations: Vec<OrgClaim>,
    #[serde(default, rename = "https://api.openai.com/auth")]
    openai_auth: Option<OpenAiAuthClaim>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct OrgClaim {
    #[serde(default)]
    id: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct OpenAiAuthClaim {
    #[serde(default)]
    chatgpt_account_id: Option<String>,
    #[serde(default)]
    chatgpt_user_id: Option<String>,
    #[serde(default)]
    user_id: Option<String>,
    #[serde(default)]
    chatgpt_plan_type: Option<String>,
}

/// 缓存的 access_token（含过期时间）
#[derive(Debug, Clone)]
struct CachedAccessToken {
    token: String,
    /// 过期时间戳（毫秒）
    expires_at_ms: i64,
}

impl CachedAccessToken {
    fn is_expiring_soon(&self) -> bool {
        let now = chrono::Utc::now().timestamp_millis();
        self.expires_at_ms - now < TOKEN_REFRESH_BUFFER_MS
    }
}

/// 进行中的 Device Code 条目，带过期时间以便清理放弃的登录流程
#[derive(Debug, Clone)]
struct PendingDeviceCode {
    user_code: String,
    /// Unix 毫秒时间戳，超时后可清理
    expires_at_ms: i64,
}

fn default_true() -> bool {
    true
}

fn default_priority() -> i32 {
    50
}

fn default_concurrency() -> u32 {
    3
}

/// 持久化的账号数据。`id` 是 CC MiniRoute 内部记录 ID，`account_id`
/// 是发给 ChatGPT Codex 上游的 workspace/account ID，两者不能混用。
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CodexAccountData {
    #[serde(default)]
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
    /// 账号邮箱（如果可获取）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan_type: Option<String>,
    /// Access-only 导入必须持久化 access token；正常 Device Code 登录也缓存一份，
    /// 以便应用重启后在 refresh 端点短暂不可用时仍可使用未过期 token。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub access_token: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub access_expires_at_ms: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
    /// 认证时间戳（秒）
    pub authenticated_at: i64,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_true")]
    pub pool_enabled: bool,
    #[serde(default)]
    pub requires_reauth: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cooldown_until_ms: Option<i64>,
    #[serde(default = "default_priority")]
    pub priority: i32,
    #[serde(default = "default_concurrency")]
    pub concurrency: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at_ms: Option<i64>,
    #[serde(default = "default_true")]
    pub auto_pause_on_expired: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proxy_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexOAuthAccount {
    pub id: String,
    pub login: String,
    pub authenticated_at: i64,
    pub is_default: bool,
    pub requires_reauth: bool,
    pub enabled: bool,
    pub pool_enabled: bool,
    pub status: String,
    pub last_error: Option<String>,
    pub cooldown_until_ms: Option<i64>,
    pub priority: i32,
    pub concurrency: u32,
    pub expires_at_ms: Option<i64>,
    pub auto_pause_on_expired: bool,
    pub renewable: bool,
    pub plan_type: Option<String>,
    pub proxy_url: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexOAuthImportOptions {
    #[serde(default = "default_true")]
    pub update_existing: bool,
    #[serde(default = "default_priority")]
    pub priority: i32,
    #[serde(default = "default_concurrency")]
    pub concurrency: u32,
    #[serde(default = "default_true")]
    pub pool_enabled: bool,
    pub expires_at_ms: Option<i64>,
    #[serde(default = "default_true")]
    pub auto_pause_on_expired: bool,
    pub proxy_url: Option<String>,
}

impl Default for CodexOAuthImportOptions {
    fn default() -> Self {
        Self {
            update_existing: true,
            priority: default_priority(),
            concurrency: default_concurrency(),
            pool_enabled: true,
            expires_at_ms: None,
            auto_pause_on_expired: true,
            proxy_url: None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexOAuthImportItem {
    pub index: usize,
    pub name: Option<String>,
    pub action: String,
    pub account_id: Option<String>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexOAuthImportResult {
    pub total: usize,
    pub created: usize,
    pub updated: usize,
    pub skipped: usize,
    pub failed: usize,
    pub items: Vec<CodexOAuthImportItem>,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct CodexOAuthResolvedAccount {
    pub record_id: String,
    pub upstream_account_id: Option<String>,
    pub access_token: String,
    pub proxy_url: Option<String>,
}

pub struct CodexOAuthAccountLease {
    record_id: String,
    active_requests: Arc<RwLock<HashMap<String, u32>>>,
}

impl Drop for CodexOAuthAccountLease {
    fn drop(&mut self) {
        let record_id = self.record_id.clone();
        let active_requests = self.active_requests.clone();
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(async move {
                let mut active = active_requests.write().await;
                if let Some(count) = active.get_mut(&record_id) {
                    *count = count.saturating_sub(1);
                    if *count == 0 {
                        active.remove(&record_id);
                    }
                }
            });
        }
    }
}

#[derive(Debug, Clone)]
struct NormalizedImportAccount {
    access_token: String,
    refresh_token: Option<String>,
    account_id: Option<String>,
    user_id: Option<String>,
    email: Option<String>,
    plan_type: Option<String>,
    access_expires_at_ms: i64,
    identity_keys: Vec<String>,
    warnings: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct CodexOAuthAccountChanges {
    pub enabled: Option<bool>,
    pub pool_enabled: Option<bool>,
    pub priority: Option<i32>,
    pub concurrency: Option<u32>,
    pub expires_at_ms: Option<Option<i64>>,
    pub auto_pause_on_expired: Option<bool>,
    pub proxy_url: Option<Option<String>>,
}

impl CodexAccountData {
    fn status(&self, now_ms: i64) -> &'static str {
        if self.requires_reauth {
            "reauth_required"
        } else if self.expires_at_ms.is_some_and(|expires| expires <= now_ms) {
            "expired"
        } else if !self.enabled {
            "disabled"
        } else if self.cooldown_until_ms.is_some_and(|until| until > now_ms) {
            "cooldown"
        } else {
            "active"
        }
    }

    fn is_schedulable(&self, now_ms: i64) -> bool {
        self.enabled
            && self.pool_enabled
            && !self.requires_reauth
            && !(self.auto_pause_on_expired
                && self.expires_at_ms.is_some_and(|expires| expires <= now_ms))
            && self.cooldown_until_ms.is_none_or(|until| until <= now_ms)
    }

    fn to_public(&self, default_account_id: Option<&str>) -> CodexOAuthAccount {
        let now_ms = chrono::Utc::now().timestamp_millis();
        CodexOAuthAccount {
            id: self.id.clone(),
            login: self
                .email
                .clone()
                .or_else(|| self.account_id.clone())
                .unwrap_or_else(|| format!("Codex account {}", short_id(&self.id))),
            authenticated_at: self.authenticated_at,
            is_default: default_account_id == Some(self.id.as_str()),
            requires_reauth: self.requires_reauth,
            enabled: self.enabled,
            pool_enabled: self.pool_enabled,
            status: self.status(now_ms).to_string(),
            last_error: self.last_error.clone(),
            cooldown_until_ms: self.cooldown_until_ms,
            priority: self.priority,
            concurrency: self.concurrency,
            expires_at_ms: self.expires_at_ms,
            auto_pause_on_expired: self.auto_pause_on_expired,
            renewable: self
                .refresh_token
                .as_deref()
                .is_some_and(|token| !token.is_empty()),
            plan_type: self.plan_type.clone(),
            proxy_url: self.proxy_url.clone(),
        }
    }
}

/// 兼容通用 Device Code 命令的登录成功返回值。
impl From<&CodexAccountData> for GitHubAccount {
    fn from(data: &CodexAccountData) -> Self {
        GitHubAccount {
            id: data.id.clone(),
            login: data
                .email
                .clone()
                .or_else(|| data.account_id.clone())
                .unwrap_or_else(|| format!("Codex account {}", short_id(&data.id))),
            avatar_url: None,
            authenticated_at: data.authenticated_at,
            github_domain: "github.com".to_string(),
        }
    }
}

/// 持久化存储结构（v2；v1 字段均可直接反序列化）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct CodexOAuthStore {
    #[serde(default)]
    version: u32,
    #[serde(default)]
    accounts: HashMap<String, CodexAccountData>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    default_account_id: Option<String>,
}

/// Codex OAuth 认证管理器（多账号）
pub struct CodexOAuthManager {
    accounts: Arc<RwLock<HashMap<String, CodexAccountData>>>,
    default_account_id: Arc<RwLock<Option<String>>>,
    /// 内存缓存的 access_token（不持久化）
    access_tokens: Arc<RwLock<HashMap<String, CachedAccessToken>>>,
    /// 每个账号的刷新锁
    refresh_locks: Arc<RwLock<HashMap<String, Arc<Mutex<()>>>>>,
    /// 进行中的 Device Code 流程：device_auth_id -> {user_code, expires_at_ms}
    /// 过期条目会在 start_device_flow 时被清理，防止放弃的登录流程导致无界增长
    pending_device_codes: Arc<RwLock<HashMap<String, PendingDeviceCode>>>,
    session_bindings: Arc<RwLock<HashMap<String, String>>>,
    response_bindings: Arc<RwLock<HashMap<String, String>>>,
    active_requests: Arc<RwLock<HashMap<String, u32>>>,
    round_robin: Arc<AtomicU64>,
    storage_path: PathBuf,
}

impl CodexOAuthManager {
    pub fn new(data_dir: PathBuf) -> Self {
        let storage_path = data_dir.join("codex_oauth_auth.json");

        let manager = Self {
            accounts: Arc::new(RwLock::new(HashMap::new())),
            default_account_id: Arc::new(RwLock::new(None)),
            access_tokens: Arc::new(RwLock::new(HashMap::new())),
            refresh_locks: Arc::new(RwLock::new(HashMap::new())),
            pending_device_codes: Arc::new(RwLock::new(HashMap::new())),
            session_bindings: Arc::new(RwLock::new(HashMap::new())),
            response_bindings: Arc::new(RwLock::new(HashMap::new())),
            active_requests: Arc::new(RwLock::new(HashMap::new())),
            round_robin: Arc::new(AtomicU64::new(0)),
            storage_path,
        };

        if let Err(e) = manager.load_from_disk_sync() {
            log::warn!("[CodexOAuth] 加载存储失败: {e}");
        }

        manager
    }

    // ==================== 设备码流程 ====================

    /// 启动 Device Code 流程
    ///
    /// 返回 GitHubDeviceCodeResponse 复用现有前端结构，但字段含义对应 OpenAI 的字段：
    /// - device_code = device_auth_id
    /// - user_code = user_code
    /// - verification_uri = https://auth.openai.com/codex/device
    pub async fn start_device_flow(&self) -> Result<GitHubDeviceCodeResponse, CodexOAuthError> {
        log::info!("[CodexOAuth] 启动 Device Code 流程");

        let response = crate::proxy::http_client::get()
            .post(DEVICE_AUTH_USERCODE_URL)
            .header("Content-Type", "application/json")
            .header("User-Agent", CODEX_USER_AGENT)
            .json(&serde_json::json!({ "client_id": CODEX_CLIENT_ID }))
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(CodexOAuthError::NetworkError(format!(
                "Device Code 请求失败: {status} - {text}"
            )));
        }

        let device: DeviceCodeResponse = response
            .json()
            .await
            .map_err(|e| CodexOAuthError::ParseError(e.to_string()))?;

        let interval = parse_interval(device.interval.as_ref());
        let expires_in = device.expires_in.unwrap_or(DEVICE_CODE_DEFAULT_EXPIRES_IN);
        let expires_at_ms = chrono::Utc::now().timestamp_millis() + (expires_in as i64) * 1000;

        // 记录 device_auth_id -> 用户码映射；同时清理所有已过期的条目，
        // 避免用户放弃登录流程导致 HashMap 无界增长
        {
            let mut pending = self.pending_device_codes.write().await;
            let now_ms = chrono::Utc::now().timestamp_millis();
            pending.retain(|_, entry| entry.expires_at_ms > now_ms);
            pending.insert(
                device.device_auth_id.clone(),
                PendingDeviceCode {
                    user_code: device.user_code.clone(),
                    expires_at_ms,
                },
            );
        }

        log::info!(
            "[CodexOAuth] 获取 Device Code 成功，user_code: {}",
            device.user_code
        );

        Ok(GitHubDeviceCodeResponse {
            device_code: device.device_auth_id,
            user_code: device.user_code,
            verification_uri: DEVICE_VERIFICATION_URL.to_string(),
            expires_in,
            interval,
        })
    }

    /// 轮询 Device Code 状态
    ///
    /// 接收 device_code（即 device_auth_id），返回 Some(account) 表示授权成功
    pub async fn poll_for_token(
        &self,
        device_code: &str,
    ) -> Result<Option<GitHubAccount>, CodexOAuthError> {
        let entry = {
            let pending = self.pending_device_codes.read().await;
            pending.get(device_code).cloned()
        };

        let entry = entry.ok_or_else(|| {
            CodexOAuthError::TokenFetchFailed(
                "未找到对应的 user_code，请重新启动登录流程".to_string(),
            )
        })?;

        if entry.expires_at_ms <= chrono::Utc::now().timestamp_millis() {
            let mut pending = self.pending_device_codes.write().await;
            pending.remove(device_code);
            return Err(CodexOAuthError::ExpiredToken);
        }

        let user_code = entry.user_code;

        log::debug!("[CodexOAuth] 轮询 Device Code");

        let poll_response = crate::proxy::http_client::get()
            .post(DEVICE_AUTH_TOKEN_URL)
            .header("Content-Type", "application/json")
            .header("User-Agent", CODEX_USER_AGENT)
            .json(&serde_json::json!({
                "device_auth_id": device_code,
                "user_code": user_code,
            }))
            .send()
            .await?;

        let status = poll_response.status();

        // 403/404 表示用户未完成授权，继续轮询
        if status == reqwest::StatusCode::FORBIDDEN || status == reqwest::StatusCode::NOT_FOUND {
            return Err(CodexOAuthError::AuthorizationPending);
        }

        if status == reqwest::StatusCode::GONE {
            return Err(CodexOAuthError::ExpiredToken);
        }

        if !status.is_success() {
            let text = poll_response.text().await.unwrap_or_default();
            return Err(CodexOAuthError::TokenFetchFailed(format!(
                "{status} - {text}"
            )));
        }

        let success: DevicePollSuccess = poll_response
            .json()
            .await
            .map_err(|e| CodexOAuthError::ParseError(e.to_string()))?;

        log::info!("[CodexOAuth] 用户已授权，正在换取 OAuth Token");

        // 用 authorization_code + code_verifier 换 token
        let tokens = self
            .exchange_code_for_tokens(&success.authorization_code, &success.code_verifier)
            .await?;

        // 清理 pending device code
        {
            let mut pending = self.pending_device_codes.write().await;
            pending.remove(device_code);
        }

        let refresh_token = tokens.refresh_token.clone().ok_or_else(|| {
            CodexOAuthError::TokenFetchFailed("响应缺少 refresh_token".to_string())
        })?;

        let (account_id, user_id, email, plan_type) =
            extract_full_identity(tokens.id_token.as_deref(), Some(&tokens.access_token));
        let account_id = account_id.ok_or_else(|| {
            CodexOAuthError::ParseError("无法从 token 中提取 account_id".to_string())
        })?;
        let access_expires_at_ms = compute_expires_at_ms(tokens.expires_in);
        let normalized = NormalizedImportAccount {
            access_token: tokens.access_token.clone(),
            refresh_token: Some(refresh_token.clone()),
            account_id: Some(account_id.clone()),
            user_id: user_id.clone(),
            email: email.clone(),
            plan_type: plan_type.clone(),
            access_expires_at_ms,
            identity_keys: build_import_identity_keys(
                Some(&account_id),
                user_id.as_deref(),
                email.as_deref(),
                &tokens.access_token,
                true,
            ),
            warnings: Vec::new(),
        };
        let matched_id = {
            let accounts = self.accounts.read().await;
            find_matching_account(&accounts, &normalized)
        };
        let account = if let Some(record_id) = matched_id {
            let public = {
                let mut accounts = self.accounts.write().await;
                let data = accounts
                    .get_mut(&record_id)
                    .ok_or_else(|| CodexOAuthError::AccountNotFound(record_id.clone()))?;
                data.account_id = Some(account_id);
                data.user_id = user_id;
                data.email = email.or(data.email.clone());
                data.plan_type = plan_type.or(data.plan_type.clone());
                data.access_token = Some(tokens.access_token.clone());
                data.access_expires_at_ms = Some(access_expires_at_ms);
                data.refresh_token = Some(refresh_token);
                data.enabled = true;
                data.requires_reauth = false;
                data.last_error = None;
                data.cooldown_until_ms = None;
                GitHubAccount::from(&*data)
            };
            self.access_tokens.write().await.insert(
                record_id,
                CachedAccessToken {
                    token: tokens.access_token,
                    expires_at_ms: access_expires_at_ms,
                },
            );
            self.save_to_disk().await?;
            public
        } else {
            let record_id = {
                let accounts = self.accounts.read().await;
                build_import_record_id(&normalized, &accounts)
            };
            self.add_oauth_account_internal(
                record_id,
                Some(account_id),
                user_id,
                email,
                plan_type,
                Some(tokens.access_token),
                Some(access_expires_at_ms),
                Some(refresh_token),
                default_priority(),
                default_concurrency(),
                true,
                None,
                true,
                None,
            )
            .await?
        };

        Ok(Some(account))
    }

    /// 用 authorization_code + code_verifier 换取 tokens
    async fn exchange_code_for_tokens(
        &self,
        code: &str,
        code_verifier: &str,
    ) -> Result<OAuthTokenResponse, CodexOAuthError> {
        let response = crate::proxy::http_client::get()
            .post(OAUTH_TOKEN_URL)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .header("User-Agent", CODEX_USER_AGENT)
            .form(&[
                ("grant_type", "authorization_code"),
                ("code", code),
                ("redirect_uri", DEVICE_REDIRECT_URI),
                ("client_id", CODEX_CLIENT_ID),
                ("code_verifier", code_verifier),
            ])
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(CodexOAuthError::TokenFetchFailed(format!(
                "Token 交换失败: {status} - {text}"
            )));
        }

        response
            .json()
            .await
            .map_err(|e| CodexOAuthError::ParseError(e.to_string()))
    }

    /// 用 refresh_token 刷新 access_token
    async fn refresh_with_token(
        &self,
        refresh_token: &str,
    ) -> Result<OAuthTokenResponse, CodexOAuthError> {
        let response = crate::proxy::http_client::get()
            .post(OAUTH_TOKEN_URL)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .header("User-Agent", CODEX_USER_AGENT)
            .form(&[
                ("grant_type", "refresh_token"),
                ("refresh_token", refresh_token),
                ("client_id", CODEX_CLIENT_ID),
                ("scope", "openid profile email"),
            ])
            .send()
            .await?;

        let status = response.status();
        if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
            return Err(CodexOAuthError::RefreshTokenInvalid);
        }

        if !status.is_success() {
            let text = response.text().await.unwrap_or_default();
            return Err(CodexOAuthError::TokenFetchFailed(format!(
                "Refresh 失败: {status} - {text}"
            )));
        }

        response
            .json()
            .await
            .map_err(|e| CodexOAuthError::ParseError(e.to_string()))
    }

    // ==================== Token 获取（含自动刷新） ====================

    /// 获取指定账号的有效 access_token（必要时自动刷新）
    pub async fn get_valid_token_for_account(
        &self,
        record_id: &str,
    ) -> Result<String, CodexOAuthError> {
        let now_ms = chrono::Utc::now().timestamp_millis();
        {
            let accounts = self.accounts.read().await;
            let account = accounts
                .get(record_id)
                .ok_or_else(|| CodexOAuthError::AccountNotFound(record_id.to_string()))?;
            if account.requires_reauth {
                return Err(CodexOAuthError::RefreshTokenInvalid);
            }
            if !account.enabled {
                return Err(CodexOAuthError::TokenFetchFailed(
                    "账号已被管理员停用".to_string(),
                ));
            }
            if account
                .expires_at_ms
                .is_some_and(|expires_at| expires_at <= now_ms)
            {
                return Err(CodexOAuthError::TokenFetchFailed(
                    "账号已到期并停止调度".to_string(),
                ));
            }
        }

        // 先检查缓存
        {
            let tokens = self.access_tokens.read().await;
            if let Some(cached) = tokens.get(record_id) {
                if !cached.is_expiring_soon() {
                    return Ok(cached.token.clone());
                }
            }
        }

        log::info!(
            "[CodexOAuth] 账号 {} 的 access_token 需要刷新",
            short_id(record_id)
        );

        let refresh_lock = self.get_refresh_lock(record_id).await;
        let _guard = refresh_lock.lock().await;

        // double-check
        {
            let tokens = self.access_tokens.read().await;
            if let Some(cached) = tokens.get(record_id) {
                if !cached.is_expiring_soon() {
                    return Ok(cached.token.clone());
                }
            }
        }

        let refresh_token = {
            let accounts = self.accounts.read().await;
            let account = accounts
                .get(record_id)
                .ok_or_else(|| CodexOAuthError::AccountNotFound(record_id.to_string()))?;
            account.refresh_token.clone()
        };

        let Some(refresh_token) = refresh_token.filter(|token| !token.trim().is_empty()) else {
            self.mark_requires_reauth(record_id, "access token expired and no refresh token")
                .await?;
            return Err(CodexOAuthError::RefreshTokenInvalid);
        };

        let new_tokens = match self.refresh_with_token(&refresh_token).await {
            Ok(tokens) => tokens,
            Err(CodexOAuthError::RefreshTokenInvalid) => {
                self.mark_requires_reauth(record_id, "refresh token rejected")
                    .await?;
                return Err(CodexOAuthError::RefreshTokenInvalid);
            }
            Err(error) => {
                self.set_last_error(record_id, Some(error.to_string()))
                    .await?;
                return Err(error);
            }
        };

        // 如果服务端返回了新的 refresh_token，更新存储
        if let Some(new_refresh) = new_tokens.refresh_token.clone() {
            if new_refresh != refresh_token {
                let mut accounts = self.accounts.write().await;
                if let Some(account) = accounts.get_mut(record_id) {
                    account.refresh_token = Some(new_refresh);
                }
            }
        }

        let access_token = new_tokens.access_token.clone();
        let expires_at_ms = compute_expires_at_ms(new_tokens.expires_in);

        {
            let mut tokens = self.access_tokens.write().await;
            tokens.insert(
                record_id.to_string(),
                CachedAccessToken {
                    token: access_token.clone(),
                    expires_at_ms,
                },
            );
        }

        {
            let mut accounts = self.accounts.write().await;
            if let Some(account) = accounts.get_mut(record_id) {
                account.access_token = Some(access_token.clone());
                account.access_expires_at_ms = Some(expires_at_ms);
                account.requires_reauth = false;
                account.last_error = None;
            }
        }
        self.save_to_disk().await?;

        Ok(access_token)
    }

    /// 获取默认账号的有效 token
    pub async fn get_valid_token(&self) -> Result<String, CodexOAuthError> {
        match self.resolve_default_account_id().await {
            Some(id) => self.get_valid_token_for_account(&id).await,
            None => Err(CodexOAuthError::AccountNotFound(
                "无可用的 ChatGPT 账号".to_string(),
            )),
        }
    }

    /// 获取默认账号 ID（热路径使用，避免克隆整个账号 HashMap）
    pub async fn default_account_id(&self) -> Option<String> {
        self.resolve_default_account_id().await
    }

    pub async fn upstream_account_id_for(&self, record_id: &str) -> Option<String> {
        self.accounts
            .read()
            .await
            .get(record_id)
            .and_then(|account| account.account_id.clone())
    }

    pub async fn resolve_pool_account(
        &self,
        session_id: Option<&str>,
        previous_response_id: Option<&str>,
    ) -> Result<(CodexOAuthResolvedAccount, CodexOAuthAccountLease), CodexOAuthError> {
        let candidate_ids = self
            .pool_candidate_ids(session_id, previous_response_id)
            .await;
        let mut last_error = None;
        for record_id in candidate_ids {
            match self.acquire_pool_account(&record_id, session_id).await {
                Ok(selection) => return Ok(selection),
                Err(error) => last_error = Some(error),
            }
        }
        Err(last_error.unwrap_or_else(|| {
            CodexOAuthError::TokenFetchFailed("账号池没有可调度的账号".to_string())
        }))
    }

    pub async fn pool_candidate_ids(
        &self,
        session_id: Option<&str>,
        previous_response_id: Option<&str>,
    ) -> Vec<String> {
        let mut candidate_ids = Vec::new();
        let response_binding = match previous_response_id.filter(|id| !id.is_empty()) {
            Some(id) => self.response_bindings.read().await.get(id).cloned(),
            None => None,
        };
        let session_binding = match session_id.filter(|id| !id.is_empty()) {
            Some(id) => self.session_bindings.read().await.get(id).cloned(),
            None => None,
        };
        for binding in [response_binding, session_binding].into_iter().flatten() {
            if !candidate_ids.contains(&binding) {
                candidate_ids.push(binding);
            }
        }

        let now_ms = chrono::Utc::now().timestamp_millis();
        let accounts_snapshot = self.accounts.read().await.clone();
        let active_snapshot = self.active_requests.read().await.clone();
        let mut candidates: Vec<(String, i32, u32, u32)> = accounts_snapshot
            .values()
            .filter(|account| account.is_schedulable(now_ms))
            .map(|account| {
                let active = active_snapshot.get(&account.id).copied().unwrap_or(0);
                (
                    account.id.clone(),
                    account.priority,
                    active,
                    account.concurrency.max(1),
                )
            })
            .collect();
        candidates.sort_by(|a, b| {
            a.1.cmp(&b.1)
                .then_with(|| (a.2 as u64 * b.3 as u64).cmp(&(b.2 as u64 * a.3 as u64)))
                .then_with(|| a.0.cmp(&b.0))
        });
        let start = self.round_robin.fetch_add(1, Ordering::Relaxed) as usize;
        let mut group_start = 0usize;
        while group_start < candidates.len() {
            let mut group_end = group_start + 1;
            while group_end < candidates.len()
                && candidates[group_end].1 == candidates[group_start].1
                && candidates[group_end].2 as u64 * candidates[group_start].3 as u64
                    == candidates[group_start].2 as u64 * candidates[group_end].3 as u64
            {
                group_end += 1;
            }
            let group_len = group_end - group_start;
            if group_len > 1 {
                candidates[group_start..group_end].rotate_left(start % group_len);
            }
            group_start = group_end;
        }
        for (id, _, _, _) in candidates {
            if !candidate_ids.contains(&id) {
                candidate_ids.push(id);
            }
        }

        candidate_ids
    }

    pub async fn acquire_pool_account(
        &self,
        record_id: &str,
        session_id: Option<&str>,
    ) -> Result<(CodexOAuthResolvedAccount, CodexOAuthAccountLease), CodexOAuthError> {
        let now_ms = chrono::Utc::now().timestamp_millis();
        let (enabled, concurrency) = {
            let accounts = self.accounts.read().await;
            let account = accounts
                .get(record_id)
                .ok_or_else(|| CodexOAuthError::AccountNotFound(record_id.to_string()))?;
            if !account.is_schedulable(now_ms) {
                return Err(CodexOAuthError::TokenFetchFailed(
                    "账号当前不可调度".to_string(),
                ));
            }
            (account.enabled, account.concurrency.max(1))
        };
        if !enabled {
            return Err(CodexOAuthError::TokenFetchFailed(
                "账号已被管理员停用".to_string(),
            ));
        }
        {
            let mut active = self.active_requests.write().await;
            let count = active.entry(record_id.to_string()).or_default();
            if *count >= concurrency {
                return Err(CodexOAuthError::TokenFetchFailed(
                    "账号并发槽已满".to_string(),
                ));
            }
            *count += 1;
        }
        let lease = CodexOAuthAccountLease {
            record_id: record_id.to_string(),
            active_requests: self.active_requests.clone(),
        };
        match self.get_valid_token_for_account(record_id).await {
            Ok(access_token) => {
                let account = self.accounts.read().await.get(record_id).cloned();
                let Some(account) = account else {
                    drop(lease);
                    return Err(CodexOAuthError::AccountNotFound(record_id.to_string()));
                };
                if let Some(session_id) = session_id.filter(|id| !id.is_empty()) {
                    self.session_bindings
                        .write()
                        .await
                        .insert(session_id.to_string(), record_id.to_string());
                }
                Ok((
                    CodexOAuthResolvedAccount {
                        record_id: record_id.to_string(),
                        upstream_account_id: account.account_id,
                        access_token,
                        proxy_url: account.proxy_url,
                    },
                    lease,
                ))
            }
            Err(error) => {
                drop(lease);
                Err(error)
            }
        }
    }

    pub async fn bind_response_account(&self, response_id: &str, record_id: &str) {
        if response_id.trim().is_empty() {
            return;
        }
        self.response_bindings
            .write()
            .await
            .insert(response_id.to_string(), record_id.to_string());
    }

    pub async fn report_pool_failure(&self, record_id: &str, status: u16) {
        if matches!(status, 401 | 403) {
            let _ = self
                .mark_requires_reauth(record_id, &format!("upstream status {status}"))
                .await;
        } else if status == 429 {
            let until = chrono::Utc::now().timestamp_millis() + 30_000;
            let mut accounts = self.accounts.write().await;
            if let Some(account) = accounts.get_mut(record_id) {
                account.cooldown_until_ms = Some(until);
                account.last_error = Some("upstream rate limited".to_string());
            }
            drop(accounts);
            let _ = self.save_to_disk().await;
        }
    }

    // ==================== 多账号管理 ====================

    pub async fn list_accounts(&self) -> Vec<CodexOAuthAccount> {
        let accounts = self.accounts.read().await.clone();
        let default_id = self.resolve_default_account_id().await;
        Self::sorted_accounts(&accounts, default_id.as_deref())
    }

    pub async fn remove_account(&self, account_id: &str) -> Result<(), CodexOAuthError> {
        log::info!("[CodexOAuth] 移除账号: {account_id}");

        {
            let mut accounts = self.accounts.write().await;
            if accounts.remove(account_id).is_none() {
                return Err(CodexOAuthError::AccountNotFound(account_id.to_string()));
            }
        }

        {
            let mut tokens = self.access_tokens.write().await;
            tokens.remove(account_id);
        }
        {
            let mut locks = self.refresh_locks.write().await;
            locks.remove(account_id);
        }

        {
            let accounts = self.accounts.read().await;
            let mut default = self.default_account_id.write().await;
            if default.as_deref() == Some(account_id) {
                *default = Self::fallback_default_account_id(&accounts);
            }
        }

        self.save_to_disk().await?;
        Ok(())
    }

    pub async fn set_default_account(&self, account_id: &str) -> Result<(), CodexOAuthError> {
        {
            let accounts = self.accounts.read().await;
            if !accounts.contains_key(account_id) {
                return Err(CodexOAuthError::AccountNotFound(account_id.to_string()));
            }
        }

        {
            let mut default = self.default_account_id.write().await;
            *default = Some(account_id.to_string());
        }

        self.save_to_disk().await?;
        Ok(())
    }

    pub async fn update_account(
        &self,
        record_id: &str,
        changes: CodexOAuthAccountChanges,
    ) -> Result<(), CodexOAuthError> {
        {
            let mut accounts = self.accounts.write().await;
            let account = accounts
                .get_mut(record_id)
                .ok_or_else(|| CodexOAuthError::AccountNotFound(record_id.to_string()))?;
            if let Some(value) = changes.enabled {
                account.enabled = value;
            }
            if let Some(value) = changes.pool_enabled {
                account.pool_enabled = value;
            }
            if let Some(value) = changes.priority {
                account.priority = value.max(0);
            }
            if let Some(value) = changes.concurrency {
                account.concurrency = value;
            }
            if let Some(value) = changes.expires_at_ms {
                account.expires_at_ms = value;
            }
            if let Some(value) = changes.auto_pause_on_expired {
                account.auto_pause_on_expired = value;
            }
            if let Some(value) = changes.proxy_url {
                account.proxy_url = value.and_then(non_empty_string);
            }
        }
        self.save_to_disk().await
    }

    pub async fn import_accounts(
        &self,
        content: &str,
        options: CodexOAuthImportOptions,
    ) -> Result<CodexOAuthImportResult, CodexOAuthError> {
        let entries = parse_import_entries(content)?;
        let mut result = CodexOAuthImportResult {
            total: entries.len(),
            ..Default::default()
        };
        let mut seen = HashMap::<String, (usize, Option<String>)>::new();

        for (offset, value) in entries.into_iter().enumerate() {
            let index = offset + 1;
            let normalized = match normalize_import_entry(&value, index) {
                Ok(value) => value,
                Err(error) => {
                    result.failed += 1;
                    let message = error.to_string();
                    result.errors.push(format!("#{index}: {message}"));
                    result.items.push(CodexOAuthImportItem {
                        index,
                        name: None,
                        action: "failed".to_string(),
                        account_id: None,
                        message: Some(message),
                    });
                    continue;
                }
            };

            let display_name = normalized
                .email
                .clone()
                .or_else(|| normalized.account_id.clone());
            if let Some(previous) = normalized.identity_keys.iter().find_map(|key| {
                seen.get(key).and_then(|(previous, previous_user_id)| {
                    (!identity_conflicts(
                        key,
                        normalized.user_id.as_deref(),
                        previous_user_id.as_deref(),
                    ))
                    .then_some(*previous)
                })
            }) {
                let message = format!("与第 {previous} 条导入项重复，已跳过");
                result.skipped += 1;
                result.warnings.push(format!("#{index}: {message}"));
                result.items.push(CodexOAuthImportItem {
                    index,
                    name: display_name,
                    action: "skipped".to_string(),
                    account_id: None,
                    message: Some(message),
                });
                continue;
            }
            for key in &normalized.identity_keys {
                seen.insert(key.clone(), (index, normalized.user_id.clone()));
            }
            for warning in &normalized.warnings {
                result.warnings.push(format!("#{index}: {warning}"));
            }

            let matched_id = {
                let accounts = self.accounts.read().await;
                find_matching_account(&accounts, &normalized)
            };
            if let Some(record_id) = matched_id {
                if !options.update_existing {
                    result.skipped += 1;
                    result.items.push(CodexOAuthImportItem {
                        index,
                        name: display_name,
                        action: "skipped".to_string(),
                        account_id: Some(record_id),
                        message: Some("账号已存在且未启用更新".to_string()),
                    });
                    continue;
                }

                {
                    let mut accounts = self.accounts.write().await;
                    let account = accounts
                        .get_mut(&record_id)
                        .ok_or_else(|| CodexOAuthError::AccountNotFound(record_id.clone()))?;
                    account.account_id =
                        normalized.account_id.clone().or(account.account_id.clone());
                    account.user_id = normalized.user_id.clone().or(account.user_id.clone());
                    account.email = normalized.email.clone().or(account.email.clone());
                    account.plan_type = normalized.plan_type.clone().or(account.plan_type.clone());
                    account.access_token = Some(normalized.access_token.clone());
                    account.access_expires_at_ms = Some(normalized.access_expires_at_ms);
                    if normalized.refresh_token.is_some() {
                        account.refresh_token = normalized.refresh_token.clone();
                    }
                    account.enabled = true;
                    account.pool_enabled = options.pool_enabled;
                    account.requires_reauth = false;
                    account.last_error = None;
                    account.cooldown_until_ms = None;
                    account.priority = options.priority.max(0);
                    account.concurrency = options.concurrency.max(1);
                    account.expires_at_ms = options.expires_at_ms;
                    account.auto_pause_on_expired = options.auto_pause_on_expired;
                    account.proxy_url = options.proxy_url.clone().and_then(non_empty_string);
                }
                self.access_tokens.write().await.insert(
                    record_id.clone(),
                    CachedAccessToken {
                        token: normalized.access_token,
                        expires_at_ms: normalized.access_expires_at_ms,
                    },
                );
                result.updated += 1;
                result.items.push(CodexOAuthImportItem {
                    index,
                    name: display_name,
                    action: "updated".to_string(),
                    account_id: Some(record_id),
                    message: None,
                });
                continue;
            }

            let record_id = {
                let accounts = self.accounts.read().await;
                build_import_record_id(&normalized, &accounts)
            };
            self.add_oauth_account_internal(
                record_id.clone(),
                normalized.account_id,
                normalized.user_id,
                normalized.email,
                normalized.plan_type,
                Some(normalized.access_token),
                Some(normalized.access_expires_at_ms),
                normalized.refresh_token,
                options.priority.max(0),
                options.concurrency.max(1),
                options.pool_enabled,
                options.expires_at_ms,
                options.auto_pause_on_expired,
                options.proxy_url.clone(),
            )
            .await?;
            result.created += 1;
            result.items.push(CodexOAuthImportItem {
                index,
                name: display_name,
                action: "created".to_string(),
                account_id: Some(record_id),
                message: None,
            });
        }

        self.save_to_disk().await?;
        Ok(result)
    }

    async fn set_last_error(
        &self,
        record_id: &str,
        error: Option<String>,
    ) -> Result<(), CodexOAuthError> {
        {
            let mut accounts = self.accounts.write().await;
            let account = accounts
                .get_mut(record_id)
                .ok_or_else(|| CodexOAuthError::AccountNotFound(record_id.to_string()))?;
            account.last_error = error;
        }
        self.save_to_disk().await
    }

    async fn mark_requires_reauth(
        &self,
        record_id: &str,
        reason: &str,
    ) -> Result<(), CodexOAuthError> {
        {
            let mut accounts = self.accounts.write().await;
            let account = accounts
                .get_mut(record_id)
                .ok_or_else(|| CodexOAuthError::AccountNotFound(record_id.to_string()))?;
            account.requires_reauth = true;
            account.last_error = Some(reason.to_string());
        }
        self.access_tokens.write().await.remove(record_id);
        self.save_to_disk().await
    }

    pub async fn clear_auth(&self) -> Result<(), CodexOAuthError> {
        log::info!("[CodexOAuth] 清除所有认证");

        {
            let mut accounts = self.accounts.write().await;
            accounts.clear();
        }
        {
            let mut default = self.default_account_id.write().await;
            *default = None;
        }
        {
            let mut tokens = self.access_tokens.write().await;
            tokens.clear();
        }
        {
            let mut locks = self.refresh_locks.write().await;
            locks.clear();
        }
        {
            let mut pending = self.pending_device_codes.write().await;
            pending.clear();
        }
        self.session_bindings.write().await.clear();
        self.response_bindings.write().await.clear();
        self.active_requests.write().await.clear();

        if self.storage_path.exists() {
            std::fs::remove_file(&self.storage_path)?;
        }

        Ok(())
    }

    pub async fn is_authenticated(&self) -> bool {
        let accounts = self.accounts.read().await;
        !accounts.is_empty()
    }

    /// 获取认证状态摘要（与 Copilot 的格式保持一致，便于复用前端）
    pub async fn get_status(&self) -> CodexOAuthStatus {
        let accounts_map = self.accounts.read().await.clone();
        let default_id = self.resolve_default_account_id().await;
        let account_list = Self::sorted_accounts(&accounts_map, default_id.as_deref());
        let authenticated = !account_list.is_empty();
        let username = default_id
            .as_ref()
            .and_then(|id| accounts_map.get(id))
            .and_then(|a| a.email.clone())
            .or_else(|| account_list.first().map(|a| a.login.clone()));

        CodexOAuthStatus {
            accounts: account_list,
            default_account_id: default_id,
            authenticated,
            username,
        }
    }

    // ==================== 内部方法 ====================

    #[cfg(test)]
    async fn add_account_internal(
        &self,
        account_id: String,
        refresh_token: String,
        email: Option<String>,
    ) -> Result<GitHubAccount, CodexOAuthError> {
        self.add_oauth_account_internal(
            account_id.clone(),
            Some(account_id),
            None,
            email,
            None,
            None,
            None,
            Some(refresh_token),
            default_priority(),
            default_concurrency(),
            true,
            None,
            true,
            None,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    async fn add_oauth_account_internal(
        &self,
        record_id: String,
        account_id: Option<String>,
        user_id: Option<String>,
        email: Option<String>,
        plan_type: Option<String>,
        access_token: Option<String>,
        access_expires_at_ms: Option<i64>,
        refresh_token: Option<String>,
        priority: i32,
        concurrency: u32,
        pool_enabled: bool,
        expires_at_ms: Option<i64>,
        auto_pause_on_expired: bool,
        proxy_url: Option<String>,
    ) -> Result<GitHubAccount, CodexOAuthError> {
        let now = chrono::Utc::now().timestamp();

        let data = CodexAccountData {
            id: record_id.clone(),
            account_id,
            user_id,
            email,
            plan_type,
            access_token: access_token.clone(),
            access_expires_at_ms,
            refresh_token,
            authenticated_at: now,
            enabled: true,
            pool_enabled,
            requires_reauth: false,
            last_error: None,
            cooldown_until_ms: None,
            priority,
            concurrency,
            expires_at_ms,
            auto_pause_on_expired,
            proxy_url,
        };

        let account = GitHubAccount::from(&data);

        {
            let mut accounts = self.accounts.write().await;
            accounts.insert(record_id.clone(), data);
        }

        if let (Some(token), Some(expires_at_ms)) = (access_token, access_expires_at_ms) {
            self.access_tokens.write().await.insert(
                record_id.clone(),
                CachedAccessToken {
                    token,
                    expires_at_ms,
                },
            );
        }

        {
            let mut default = self.default_account_id.write().await;
            if default.is_none() {
                *default = Some(record_id);
            }
        }

        self.save_to_disk().await?;
        Ok(account)
    }

    fn fallback_default_account_id(accounts: &HashMap<String, CodexAccountData>) -> Option<String> {
        accounts
            .iter()
            .max_by(|(id_a, a), (id_b, b)| {
                a.authenticated_at
                    .cmp(&b.authenticated_at)
                    .then_with(|| id_b.cmp(id_a))
            })
            .map(|(id, _)| id.clone())
    }

    fn sorted_accounts(
        accounts: &HashMap<String, CodexAccountData>,
        default_account_id: Option<&str>,
    ) -> Vec<CodexOAuthAccount> {
        let mut list: Vec<CodexOAuthAccount> = accounts
            .values()
            .map(|account| account.to_public(default_account_id))
            .collect();
        list.sort_by(|a, b| {
            let a_default = default_account_id == Some(a.id.as_str());
            let b_default = default_account_id == Some(b.id.as_str());
            b_default
                .cmp(&a_default)
                .then_with(|| b.authenticated_at.cmp(&a.authenticated_at))
                .then_with(|| a.login.cmp(&b.login))
        });
        list
    }

    async fn resolve_default_account_id(&self) -> Option<String> {
        let stored = self.default_account_id.read().await.clone();
        let accounts = self.accounts.read().await;

        if let Some(id) = stored {
            if accounts.contains_key(&id) {
                return Some(id);
            }
        }

        Self::fallback_default_account_id(&accounts)
    }

    async fn get_refresh_lock(&self, account_id: &str) -> Arc<Mutex<()>> {
        {
            let locks = self.refresh_locks.read().await;
            if let Some(lock) = locks.get(account_id) {
                return Arc::clone(lock);
            }
        }

        let mut locks = self.refresh_locks.write().await;
        Arc::clone(
            locks
                .entry(account_id.to_string())
                .or_insert_with(|| Arc::new(Mutex::new(()))),
        )
    }

    fn write_store_atomic(&self, content: &str) -> Result<(), CodexOAuthError> {
        if let Some(parent) = self.storage_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let parent = self
            .storage_path
            .parent()
            .ok_or_else(|| CodexOAuthError::IoError("无效的存储路径".to_string()))?;
        let file_name = self
            .storage_path
            .file_name()
            .ok_or_else(|| CodexOAuthError::IoError("无效的存储文件名".to_string()))?
            .to_string_lossy()
            .to_string();
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let tmp_path = parent.join(format!("{file_name}.tmp.{ts}"));

        #[cfg(unix)]
        {
            use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

            let mut file = fs::OpenOptions::new()
                .create_new(true)
                .write(true)
                .mode(0o600)
                .open(&tmp_path)?;
            file.write_all(content.as_bytes())?;
            file.flush()?;

            fs::rename(&tmp_path, &self.storage_path)?;
            fs::set_permissions(&self.storage_path, fs::Permissions::from_mode(0o600))?;
        }

        #[cfg(windows)]
        {
            let mut file = fs::OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&tmp_path)?;
            file.write_all(content.as_bytes())?;
            file.flush()?;

            if self.storage_path.exists() {
                let _ = fs::remove_file(&self.storage_path);
            }
            fs::rename(&tmp_path, &self.storage_path)?;
        }

        Ok(())
    }

    fn load_from_disk_sync(&self) -> Result<(), CodexOAuthError> {
        if !self.storage_path.exists() {
            return Ok(());
        }

        let content = std::fs::read_to_string(&self.storage_path)?;
        let mut store: CodexOAuthStore = serde_json::from_str(&content)
            .map_err(|e| CodexOAuthError::ParseError(e.to_string()))?;

        let now_ms = chrono::Utc::now().timestamp_millis();
        let mut migrated = HashMap::with_capacity(store.accounts.len());
        for (legacy_key, mut account) in store.accounts {
            if account.id.trim().is_empty() {
                account.id = legacy_key.clone();
            }
            if account.account_id.is_none() {
                account.account_id = Some(legacy_key.clone());
            }
            if account
                .expires_at_ms
                .is_some_and(|expires_at| expires_at <= now_ms)
                && account.auto_pause_on_expired
            {
                account.enabled = false;
                account.last_error = Some("account expired".to_string());
            }
            let key = account.id.clone();
            migrated.insert(key, account);
        }
        store.accounts = migrated;

        if let Ok(mut accounts) = self.accounts.try_write() {
            *accounts = store.accounts.clone();
            log::info!("[CodexOAuth] 从磁盘加载 {} 个账号", accounts.len());
        }
        if let Ok(mut default) = self.default_account_id.try_write() {
            *default = store.default_account_id;
            if default.is_none() {
                if let Ok(accounts) = self.accounts.try_read() {
                    *default = Self::fallback_default_account_id(&accounts);
                }
            }
        }

        if let Ok(mut tokens) = self.access_tokens.try_write() {
            for (record_id, account) in &store.accounts {
                if let (Some(token), Some(expires_at_ms)) =
                    (account.access_token.clone(), account.access_expires_at_ms)
                {
                    if expires_at_ms > now_ms {
                        tokens.insert(
                            record_id.clone(),
                            CachedAccessToken {
                                token,
                                expires_at_ms,
                            },
                        );
                    }
                }
            }
        }

        Ok(())
    }

    async fn save_to_disk(&self) -> Result<(), CodexOAuthError> {
        let accounts = self.accounts.read().await.clone();
        let default = self.resolve_default_account_id().await;

        let store = CodexOAuthStore {
            version: 2,
            accounts,
            default_account_id: default,
        };

        let content = serde_json::to_string_pretty(&store)
            .map_err(|e| CodexOAuthError::ParseError(e.to_string()))?;

        self.write_store_atomic(&content)?;

        log::info!(
            "[CodexOAuth] 保存到磁盘成功（{} 个账号）",
            store.accounts.len()
        );

        Ok(())
    }
}

/// Codex OAuth 状态摘要
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexOAuthStatus {
    pub accounts: Vec<CodexOAuthAccount>,
    pub default_account_id: Option<String>,
    pub authenticated: bool,
    pub username: Option<String>,
}

// ==================== 工具函数 ====================

/// 解析 OpenAI Device Code 响应中的 interval 字段
///
/// 服务端可能返回字符串或数字，需要兼容
fn parse_interval(value: Option<&serde_json::Value>) -> u64 {
    let raw = match value {
        Some(serde_json::Value::Number(n)) => n.as_u64().unwrap_or(5),
        Some(serde_json::Value::String(s)) => s.parse::<u64>().unwrap_or(5),
        _ => 5,
    };
    raw.max(1) + POLLING_SAFETY_MARGIN_SECS
}

/// 从 expires_in（秒）计算过期时间戳（毫秒）
fn compute_expires_at_ms(expires_in: Option<i64>) -> i64 {
    let now_ms = chrono::Utc::now().timestamp_millis();
    let secs = expires_in.unwrap_or(3600);
    now_ms + secs * 1000
}

/// 解析 JWT 中的 claims
fn parse_jwt_claims(token: &str) -> Option<IdTokenClaims> {
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        return None;
    }
    let decoded = URL_SAFE_NO_PAD.decode(parts[1]).ok()?;
    serde_json::from_slice(&decoded).ok()
}

fn short_id(value: &str) -> String {
    value.chars().take(12).collect()
}

fn non_empty_string(value: String) -> Option<String> {
    let value = value.trim().to_string();
    (!value.is_empty()).then_some(value)
}

fn token_fingerprint(token: &str) -> String {
    let digest = Sha256::digest(token.trim().as_bytes());
    format!("{:x}", digest)
}

fn jwt_expires_at_ms(token: &str) -> Option<i64> {
    parse_jwt_claims(token)?.exp.map(|seconds| seconds * 1000)
}

fn extract_full_identity(
    id_token: Option<&str>,
    access_token: Option<&str>,
) -> (
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
) {
    let mut account_id = None;
    let mut user_id = None;
    let mut email = None;
    let mut plan_type = None;
    for token in [id_token, access_token].into_iter().flatten() {
        let Some(claims) = parse_jwt_claims(token) else {
            continue;
        };
        if account_id.is_none() {
            account_id = claims
                .chatgpt_account_id
                .clone()
                .or_else(|| {
                    claims
                        .openai_auth
                        .as_ref()
                        .and_then(|auth| auth.chatgpt_account_id.clone())
                })
                .or_else(|| claims.organizations.first().and_then(|org| org.id.clone()));
        }
        if user_id.is_none() {
            user_id = claims
                .openai_auth
                .as_ref()
                .and_then(|auth| {
                    auth.chatgpt_user_id
                        .clone()
                        .or_else(|| auth.user_id.clone())
                })
                .or_else(|| claims.sub.clone());
        }
        if email.is_none() {
            email = claims.email.clone();
        }
        if plan_type.is_none() {
            plan_type = claims
                .openai_auth
                .as_ref()
                .and_then(|auth| auth.chatgpt_plan_type.clone());
        }
    }
    (account_id, user_id, email, plan_type)
}

fn parse_import_entries(content: &str) -> Result<Vec<serde_json::Value>, CodexOAuthError> {
    let content = content.trim_start_matches('\u{feff}').trim();
    if content.is_empty() {
        return Err(CodexOAuthError::ParseError("导入内容为空".to_string()));
    }

    if let Ok(value) = serde_json::from_str::<serde_json::Value>(content) {
        let mut entries = Vec::new();
        flatten_import_value(value, &mut entries);
        return Ok(entries);
    }

    let mut entries = Vec::new();
    for (line_index, line) in content.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let value = if line.starts_with('{') || line.starts_with('[') {
            serde_json::from_str(line).map_err(|error| {
                CodexOAuthError::ParseError(format!(
                    "第 {} 行 JSON 解析失败: {error}",
                    line_index + 1
                ))
            })?
        } else {
            serde_json::Value::String(line.to_string())
        };
        flatten_import_value(value, &mut entries);
    }
    if entries.is_empty() {
        return Err(CodexOAuthError::ParseError("没有可导入的账号".to_string()));
    }
    Ok(entries)
}

fn flatten_import_value(value: serde_json::Value, output: &mut Vec<serde_json::Value>) {
    match value {
        serde_json::Value::Array(values) => {
            for value in values {
                flatten_import_value(value, output);
            }
        }
        serde_json::Value::Object(mut object) => {
            for key in ["accounts", "items", "data"] {
                if let Some(serde_json::Value::Array(values)) = object.remove(key) {
                    for value in values {
                        flatten_import_value(value, output);
                    }
                    return;
                }
            }
            output.push(serde_json::Value::Object(object));
        }
        other => output.push(other),
    }
}

fn value_at_paths(value: &serde_json::Value, paths: &[&[&str]]) -> Option<String> {
    for path in paths {
        let mut current = value;
        let mut found = true;
        for segment in *path {
            let Some(next) = current.get(*segment) else {
                found = false;
                break;
            };
            current = next;
        }
        if found {
            if let Some(text) = current
                .as_str()
                .map(str::trim)
                .filter(|text| !text.is_empty())
            {
                return Some(text.to_string());
            }
        }
    }
    None
}

fn normalize_import_entry(
    value: &serde_json::Value,
    index: usize,
) -> Result<NormalizedImportAccount, CodexOAuthError> {
    let access_token = match value {
        serde_json::Value::String(token) => non_empty_string(token.clone()),
        serde_json::Value::Object(_) => value_at_paths(
            value,
            &[
                &["tokens", "access_token"],
                &["tokens", "accessToken"],
                &["access_token"],
                &["accessToken"],
                &["token"],
            ],
        ),
        _ => None,
    }
    .ok_or_else(|| CodexOAuthError::ParseError(format!("第 {index} 条缺少 accessToken")))?;
    let refresh_token = value_at_paths(
        value,
        &[
            &["tokens", "refresh_token"],
            &["tokens", "refreshToken"],
            &["refresh_token"],
            &["refreshToken"],
        ],
    );
    let id_token = value_at_paths(
        value,
        &[
            &["tokens", "id_token"],
            &["tokens", "idToken"],
            &["id_token"],
            &["idToken"],
        ],
    );
    let (jwt_account_id, jwt_user_id, jwt_email, jwt_plan_type) =
        extract_full_identity(id_token.as_deref(), Some(&access_token));
    let account_id = value_at_paths(
        value,
        &[
            &["chatgpt_account_id"],
            &["chatgptAccountId"],
            &["account_id"],
            &["accountId"],
            &["account", "id"],
        ],
    )
    .or(jwt_account_id);
    let user_id = value_at_paths(
        value,
        &[
            &["chatgpt_user_id"],
            &["chatgptUserId"],
            &["user_id"],
            &["userId"],
            &["user", "id"],
        ],
    )
    .or(jwt_user_id);
    let email = value_at_paths(value, &[&["email"], &["user", "email"]]).or(jwt_email);
    let plan_type = value_at_paths(
        value,
        &[
            &["plan_type"],
            &["planType"],
            &["account", "plan_type"],
            &["account", "planType"],
        ],
    )
    .or(jwt_plan_type);
    let access_expires_at_ms = value_at_paths(
        value,
        &[
            &["tokens", "expires_at"],
            &["tokens", "expiresAt"],
            &["expires_at"],
            &["expiresAt"],
        ],
    )
    .and_then(|raw| parse_time_ms(&raw))
    .or_else(|| jwt_expires_at_ms(&access_token))
    .unwrap_or_else(|| chrono::Utc::now().timestamp_millis() + 3_600_000);
    if access_expires_at_ms <= chrono::Utc::now().timestamp_millis() - 120_000 {
        return Err(CodexOAuthError::ParseError(format!(
            "第 {index} 条 access token 已过期"
        )));
    }

    let mut warnings = Vec::new();
    if refresh_token.is_none() {
        warnings.push("未包含 refresh token，access token 过期后将停止调度".to_string());
    }
    if parse_jwt_claims(&access_token).is_none() {
        warnings.push("access token 不是可解析 JWT，无法完整校验身份与过期时间".to_string());
    }
    let identity_keys = build_import_identity_keys(
        account_id.as_deref(),
        user_id.as_deref(),
        email.as_deref(),
        &access_token,
        refresh_token.is_some(),
    );

    Ok(NormalizedImportAccount {
        access_token,
        refresh_token,
        account_id,
        user_id,
        email,
        plan_type,
        access_expires_at_ms,
        identity_keys,
        warnings,
    })
}

fn build_import_identity_keys(
    account_id: Option<&str>,
    user_id: Option<&str>,
    email: Option<&str>,
    access_token: &str,
    renewable: bool,
) -> Vec<String> {
    let fingerprint = token_fingerprint(access_token);
    if !renewable {
        return vec![format!("access:{fingerprint}")];
    }

    let mut keys = Vec::new();
    if let Some(user_id) = user_id.filter(|value| !value.is_empty()) {
        keys.push(format!("user:{user_id}"));
    }
    if let Some(account_id) = account_id.filter(|value| !value.is_empty()) {
        keys.push(format!("account:{account_id}"));
    }
    if account_id.is_none() && user_id.is_none() {
        if let Some(email) = email.filter(|value| !value.is_empty()) {
            keys.push(format!("email:{}", email.to_ascii_lowercase()));
        }
    }
    keys.push(format!("access:{fingerprint}"));
    keys
}

fn parse_time_ms(value: &str) -> Option<i64> {
    if let Ok(number) = value.parse::<i64>() {
        return Some(if number < 10_000_000_000 {
            number * 1000
        } else {
            number
        });
    }
    chrono::DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|value| value.timestamp_millis())
}

fn stored_identity_keys(account: &CodexAccountData) -> Vec<String> {
    let mut keys = Vec::new();
    if let Some(user_id) = account.user_id.as_deref().filter(|value| !value.is_empty()) {
        keys.push(format!("user:{user_id}"));
    }
    if let Some(account_id) = account
        .account_id
        .as_deref()
        .filter(|value| !value.is_empty())
    {
        keys.push(format!("account:{account_id}"));
    }
    if account.account_id.is_none() && account.user_id.is_none() {
        if let Some(email) = account.email.as_deref().filter(|value| !value.is_empty()) {
            keys.push(format!("email:{}", email.to_ascii_lowercase()));
        }
    }
    if let Some(access_token) = account
        .access_token
        .as_deref()
        .filter(|value| !value.is_empty())
    {
        keys.push(format!("access:{}", token_fingerprint(access_token)));
    }
    keys
}

fn identity_conflicts(key: &str, user_id: Option<&str>, stored_user_id: Option<&str>) -> bool {
    key.starts_with("account:")
        && user_id.is_some()
        && stored_user_id.is_some()
        && user_id != stored_user_id
}

fn find_matching_account(
    accounts: &HashMap<String, CodexAccountData>,
    incoming: &NormalizedImportAccount,
) -> Option<String> {
    for key in &incoming.identity_keys {
        for (record_id, account) in accounts {
            if identity_conflicts(key, incoming.user_id.as_deref(), account.user_id.as_deref()) {
                continue;
            }
            if stored_identity_keys(account).contains(key) {
                return Some(record_id.clone());
            }
        }
    }
    None
}

fn build_import_record_id(
    incoming: &NormalizedImportAccount,
    accounts: &HashMap<String, CodexAccountData>,
) -> String {
    let base = incoming
        .account_id
        .clone()
        .or_else(|| incoming.user_id.clone())
        .unwrap_or_else(|| {
            format!(
                "import-{}",
                &token_fingerprint(&incoming.access_token)[..16]
            )
        });
    if !accounts.contains_key(&base) {
        return base;
    }
    let suffix = &token_fingerprint(&incoming.access_token)[..12];
    format!("{base}-{suffix}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_interval_number() {
        let v = serde_json::Value::Number(serde_json::Number::from(5));
        assert_eq!(parse_interval(Some(&v)), 5 + POLLING_SAFETY_MARGIN_SECS);
    }

    #[test]
    fn test_parse_interval_string() {
        let v = serde_json::Value::String("10".to_string());
        assert_eq!(parse_interval(Some(&v)), 10 + POLLING_SAFETY_MARGIN_SECS);
    }

    #[test]
    fn test_parse_interval_default() {
        assert_eq!(parse_interval(None), 5 + POLLING_SAFETY_MARGIN_SECS);
    }

    #[test]
    fn test_parse_interval_min() {
        let v = serde_json::Value::Number(serde_json::Number::from(0));
        // 0 应被提升到 1
        assert_eq!(parse_interval(Some(&v)), 1 + POLLING_SAFETY_MARGIN_SECS);
    }

    #[test]
    fn test_compute_expires_at_ms() {
        let result = compute_expires_at_ms(Some(3600));
        let now = chrono::Utc::now().timestamp_millis();
        // 应在未来约 3600 秒处（允许少量误差）
        assert!(result > now + 3500 * 1000);
        assert!(result < now + 3700 * 1000);
    }

    #[test]
    fn test_compute_expires_at_ms_default() {
        let result = compute_expires_at_ms(None);
        let now = chrono::Utc::now().timestamp_millis();
        assert!(result > now);
    }

    #[test]
    fn test_cached_token_expiring_soon() {
        let now = chrono::Utc::now().timestamp_millis();
        // 30 秒后过期 - 在缓冲期内
        let expiring = CachedAccessToken {
            token: "t".to_string(),
            expires_at_ms: now + 30_000,
        };
        assert!(expiring.is_expiring_soon());

        // 1 小时后过期 - 不在缓冲期内
        let valid = CachedAccessToken {
            token: "t".to_string(),
            expires_at_ms: now + 3_600_000,
        };
        assert!(!valid.is_expiring_soon());
    }

    #[test]
    fn test_parse_jwt_claims_invalid() {
        assert!(parse_jwt_claims("not-a-jwt").is_none());
        assert!(parse_jwt_claims("only.two").is_none());
    }

    #[test]
    fn test_parse_jwt_claims_valid() {
        // Header: {"alg":"none"}
        // Payload: {"chatgpt_account_id":"acc-123","email":"test@example.com"}
        // Signature: empty
        let header = URL_SAFE_NO_PAD.encode(b"{\"alg\":\"none\"}");
        let payload = URL_SAFE_NO_PAD
            .encode(b"{\"chatgpt_account_id\":\"acc-123\",\"email\":\"test@example.com\"}");
        let jwt = format!("{header}.{payload}.");
        let claims = parse_jwt_claims(&jwt).unwrap();
        assert_eq!(claims.chatgpt_account_id.as_deref(), Some("acc-123"));
        assert_eq!(claims.email.as_deref(), Some("test@example.com"));
    }

    #[test]
    fn test_parse_jwt_claims_organizations_fallback() {
        let header = URL_SAFE_NO_PAD.encode(b"{\"alg\":\"none\"}");
        let payload = URL_SAFE_NO_PAD.encode(b"{\"organizations\":[{\"id\":\"org-456\"}]}");
        let jwt = format!("{header}.{payload}.");
        let claims = parse_jwt_claims(&jwt).unwrap();
        assert_eq!(
            claims
                .organizations
                .first()
                .and_then(|o| o.id.clone())
                .as_deref(),
            Some("org-456")
        );
    }

    fn fake_jwt(claims: &str) -> String {
        let header = URL_SAFE_NO_PAD.encode(b"{\"alg\":\"none\"}");
        let payload = URL_SAFE_NO_PAD.encode(claims.as_bytes());
        format!("{header}.{payload}.")
    }

    #[test]
    fn test_normalize_access_only_uses_token_fingerprint_identity() {
        let token = fake_jwt(r#"{"sub":"user-a","chatgpt_account_id":"team-a","exp":4102444800}"#);
        let value = serde_json::json!({
            "tokens": { "access_token": token },
            "chatgpt_account_id": "team-a",
            "chatgpt_user_id": "user-a"
        });
        let normalized = normalize_import_entry(&value, 1).unwrap();
        assert_eq!(normalized.identity_keys.len(), 1);
        assert!(normalized.identity_keys[0].starts_with("access:"));
        assert!(normalized.refresh_token.is_none());
    }

    #[test]
    fn test_team_members_with_same_account_id_do_not_conflict() {
        let first = fake_jwt(r#"{"sub":"user-a","chatgpt_account_id":"team-a","exp":4102444800}"#);
        let second = fake_jwt(r#"{"sub":"user-b","chatgpt_account_id":"team-a","exp":4102444800}"#);
        let first = normalize_import_entry(
            &serde_json::json!({
                "access_token": first,
                "refresh_token": "refresh-a"
            }),
            1,
        )
        .unwrap();
        let second = normalize_import_entry(
            &serde_json::json!({
                "access_token": second,
                "refresh_token": "refresh-b"
            }),
            2,
        )
        .unwrap();
        assert!(second
            .identity_keys
            .iter()
            .any(|key| key == "account:team-a"));
        assert!(identity_conflicts(
            "account:team-a",
            second.user_id.as_deref(),
            first.user_id.as_deref()
        ));

        let mut accounts = HashMap::new();
        accounts.insert(
            "record-a".to_string(),
            CodexAccountData {
                id: "record-a".to_string(),
                account_id: first.account_id,
                user_id: first.user_id,
                email: first.email,
                plan_type: first.plan_type,
                access_token: Some(first.access_token),
                access_expires_at_ms: Some(first.access_expires_at_ms),
                refresh_token: first.refresh_token,
                authenticated_at: 0,
                enabled: true,
                pool_enabled: true,
                requires_reauth: false,
                last_error: None,
                cooldown_until_ms: None,
                priority: 50,
                concurrency: 3,
                expires_at_ms: None,
                auto_pause_on_expired: true,
                proxy_url: None,
            },
        );
        assert!(find_matching_account(&accounts, &second).is_none());
    }

    #[test]
    fn test_access_only_update_does_not_supply_refresh_token() {
        let existing =
            fake_jwt(r#"{"sub":"user-a","chatgpt_account_id":"team-a","exp":4102444800}"#);
        let incoming = normalize_import_entry(
            &serde_json::json!({
                "access_token": existing,
                "chatgpt_account_id": "team-a",
                "chatgpt_user_id": "user-a"
            }),
            1,
        )
        .unwrap();
        assert!(incoming.refresh_token.is_none());
        let old_refresh = Some("keep-me".to_string());
        let merged_refresh = incoming.refresh_token.clone().or(old_refresh.clone());
        assert_eq!(merged_refresh, old_refresh);
    }

    #[tokio::test]
    async fn test_manager_initial_state() {
        let temp = tempfile::tempdir().unwrap();
        let manager = CodexOAuthManager::new(temp.path().to_path_buf());
        assert!(!manager.is_authenticated().await);
        assert!(manager.list_accounts().await.is_empty());
    }

    #[tokio::test]
    async fn test_manager_save_and_load() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().to_path_buf();

        // Manually inject an account through internal methods
        {
            let manager = CodexOAuthManager::new(path.clone());
            manager
                .add_account_internal(
                    "acc-123".to_string(),
                    "rt-secret".to_string(),
                    Some("user@example.com".to_string()),
                )
                .await
                .unwrap();
        }

        // New manager should load from disk
        let manager2 = CodexOAuthManager::new(path);
        let accounts = manager2.list_accounts().await;
        assert_eq!(accounts.len(), 1);
        assert_eq!(accounts[0].id, "acc-123");
    }

    #[tokio::test]
    async fn test_v1_store_migrates_record_id_and_account_id() {
        let temp = tempfile::tempdir().unwrap();
        let storage_path = temp.path().join("codex_oauth_auth.json");
        std::fs::write(
            &storage_path,
            r#"{
                "accounts": {
                    "legacy-account": {
                        "account_id": "legacy-account",
                        "email": "legacy@example.com",
                        "refresh_token": "legacy-refresh",
                        "authenticated_at": 123
                    }
                },
                "default_account_id": "legacy-account"
            }"#,
        )
        .unwrap();

        let manager = CodexOAuthManager::new(temp.path().to_path_buf());
        let accounts = manager.accounts.read().await;
        let account = accounts.get("legacy-account").unwrap();
        assert_eq!(account.id, "legacy-account");
        assert_eq!(account.account_id.as_deref(), Some("legacy-account"));
        assert_eq!(account.refresh_token.as_deref(), Some("legacy-refresh"));
    }

    #[tokio::test]
    async fn test_import_updates_existing_and_preserves_refresh_token() {
        let temp = tempfile::tempdir().unwrap();
        let manager = CodexOAuthManager::new(temp.path().to_path_buf());
        let token = fake_jwt(
            r#"{"sub":"user-a","chatgpt_account_id":"team-a","email":"a@example.com","exp":4102444800}"#,
        );
        manager
            .import_accounts(
                &serde_json::json!({
                    "access_token": token,
                    "refresh_token": "keep-refresh"
                })
                .to_string(),
                CodexOAuthImportOptions::default(),
            )
            .await
            .unwrap();

        let result = manager
            .import_accounts(
                &serde_json::json!({
                    "access_token": token,
                    "chatgpt_account_id": "team-a",
                    "chatgpt_user_id": "user-a"
                })
                .to_string(),
                CodexOAuthImportOptions::default(),
            )
            .await
            .unwrap();
        assert_eq!(result.updated, 1);
        assert_eq!(result.created, 0);
        let accounts = manager.accounts.read().await;
        assert_eq!(accounts.len(), 1);
        assert_eq!(
            accounts.values().next().unwrap().refresh_token.as_deref(),
            Some("keep-refresh")
        );
    }

    #[tokio::test]
    async fn test_import_keeps_two_team_members_in_same_workspace() {
        let temp = tempfile::tempdir().unwrap();
        let manager = CodexOAuthManager::new(temp.path().to_path_buf());
        let first = fake_jwt(r#"{"sub":"user-a","chatgpt_account_id":"team-a","exp":4102444800}"#);
        let second = fake_jwt(r#"{"sub":"user-b","chatgpt_account_id":"team-a","exp":4102444800}"#);
        let content = serde_json::json!([
            { "access_token": first, "refresh_token": "refresh-a" },
            { "access_token": second, "refresh_token": "refresh-b" }
        ])
        .to_string();

        let result = manager
            .import_accounts(&content, CodexOAuthImportOptions::default())
            .await
            .unwrap();
        assert_eq!(result.created, 2);
        assert_eq!(result.skipped, 0);
        assert_eq!(manager.accounts.read().await.len(), 2);
    }

    #[tokio::test]
    async fn test_pool_rotation_never_crosses_priority_or_load_tier() {
        let temp = tempfile::tempdir().unwrap();
        let manager = CodexOAuthManager::new(temp.path().to_path_buf());
        let expires = chrono::Utc::now().timestamp_millis() + 3_600_000;
        for (id, priority, concurrency) in [("p10-a", 10, 2), ("p10-b", 10, 2), ("p20", 20, 2)] {
            manager
                .add_oauth_account_internal(
                    id.to_string(),
                    Some(format!("workspace-{id}")),
                    Some(format!("user-{id}")),
                    Some(format!("{id}@example.com")),
                    Some("plus".to_string()),
                    Some(format!("token-{id}")),
                    Some(expires),
                    None,
                    priority,
                    concurrency,
                    true,
                    None,
                    true,
                    None,
                )
                .await
                .unwrap();
        }

        let first = manager.pool_candidate_ids(None, None).await;
        let second = manager.pool_candidate_ids(None, None).await;
        assert_eq!(first.last().map(String::as_str), Some("p20"));
        assert_eq!(second.last().map(String::as_str), Some("p20"));
        assert_ne!(&first[..2], &second[..2]);

        manager
            .active_requests
            .write()
            .await
            .insert("p10-a".to_string(), 1);
        let loaded = manager.pool_candidate_ids(None, None).await;
        assert_eq!(loaded.first().map(String::as_str), Some("p10-b"));
        assert_eq!(loaded.get(1).map(String::as_str), Some("p10-a"));
        assert_eq!(loaded.last().map(String::as_str), Some("p20"));
    }

    #[tokio::test]
    async fn test_pool_429_cooldown_is_persisted() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().to_path_buf();
        let manager = CodexOAuthManager::new(path.clone());
        manager
            .add_account_internal(
                "rate-limited".to_string(),
                "refresh".to_string(),
                Some("rate@example.com".to_string()),
            )
            .await
            .unwrap();
        manager.report_pool_failure("rate-limited", 429).await;

        let account = manager
            .list_accounts()
            .await
            .into_iter()
            .find(|account| account.id == "rate-limited")
            .unwrap();
        assert_eq!(account.status, "cooldown");
        assert!(account.cooldown_until_ms.is_some());

        let reloaded = CodexOAuthManager::new(path);
        let account = reloaded
            .list_accounts()
            .await
            .into_iter()
            .find(|account| account.id == "rate-limited")
            .unwrap();
        assert_eq!(account.status, "cooldown");
    }

    #[tokio::test]
    async fn test_remove_account() {
        let temp = tempfile::tempdir().unwrap();
        let manager = CodexOAuthManager::new(temp.path().to_path_buf());

        manager
            .add_account_internal(
                "acc-123".to_string(),
                "rt".to_string(),
                Some("a@example.com".to_string()),
            )
            .await
            .unwrap();
        manager
            .add_account_internal(
                "acc-456".to_string(),
                "rt2".to_string(),
                Some("b@example.com".to_string()),
            )
            .await
            .unwrap();

        manager.remove_account("acc-123").await.unwrap();
        let accounts = manager.list_accounts().await;
        assert_eq!(accounts.len(), 1);
        assert_eq!(accounts[0].id, "acc-456");
    }
}
