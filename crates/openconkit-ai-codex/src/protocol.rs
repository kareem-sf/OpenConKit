//! Stable Codex app-server protocol subset used by OpenConKit.
//!
//! These bindings mirror the pinned `0.145.0` stable V2 JSON schema. Fields
//! outside OpenConKit's account and analyzer surfaces remain opaque JSON so
//! upstream additive changes do not weaken our security boundary.

use serde::{Deserialize, Serialize};

/// Client identity sent during the mandatory initialization handshake.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientInfo {
    pub name: String,
    pub title: Option<String>,
    pub version: String,
}

/// Stable initialization request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeParams {
    pub client_info: ClientInfo,
}

/// Safe ChatGPT account data returned to the host.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Account {
    ApiKey,
    Chatgpt {
        email: Option<String>,
        #[serde(rename = "planType")]
        plan_type: PlanType,
    },
    AmazonBedrock {
        #[serde(default, rename = "usesCodexManagedCredentials")]
        uses_codex_managed_credentials: bool,
    },
}

/// ChatGPT plan classifications published by the pinned protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanType {
    Free,
    Go,
    Plus,
    Pro,
    Prolite,
    Team,
    SelfServeBusinessUsageBased,
    Business,
    EnterpriseCbpUsageBased,
    Enterprise,
    Edu,
    #[serde(other)]
    Unknown,
}

/// `account/read` response.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetAccountResponse {
    pub account: Option<Account>,
    pub requires_openai_auth: bool,
}

/// Supported login requests. OpenConKit deliberately has no API-key variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum LoginAccountParams {
    Chatgpt {
        #[serde(rename = "useHostedLoginSuccessPage")]
        use_hosted_login_success_page: bool,
        #[serde(rename = "appBrand")]
        app_brand: LoginAppBrand,
    },
    ChatgptDeviceCode,
}

/// Branding for the official hosted login success page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LoginAppBrand {
    Codex,
    Chatgpt,
}

/// Browser or device-code login challenge.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum LoginAccountResponse {
    Chatgpt {
        #[serde(rename = "loginId")]
        login_id: String,
        #[serde(rename = "authUrl")]
        auth_url: String,
    },
    ChatgptDeviceCode {
        #[serde(rename = "loginId")]
        login_id: String,
        #[serde(rename = "verificationUrl")]
        verification_url: String,
        #[serde(rename = "userCode")]
        user_code: String,
    },
    #[serde(other)]
    Unsupported,
}

/// One ChatGPT rate-limit window.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RateLimitWindow {
    pub used_percent: i32,
    pub window_duration_mins: Option<i64>,
    pub resets_at: Option<i64>,
}

/// Backward-compatible rate-limit snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RateLimitSnapshot {
    pub primary: Option<RateLimitWindow>,
    pub secondary: Option<RateLimitWindow>,
    pub plan_type: Option<PlanType>,
    pub rate_limit_reached_type: Option<serde_json::Value>,
    pub spend_control_reached: Option<bool>,
}

/// `account/rateLimits/read` response.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetAccountRateLimitsResponse {
    pub rate_limits: RateLimitSnapshot,
}

/// Minimal thread-start response needed by the analyzer.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadStartResponse {
    pub model: String,
    pub model_provider: String,
    pub thread: ThreadIdentity,
}

/// Stable identity of a Codex thread.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ThreadIdentity {
    pub id: String,
}

/// Minimal turn-start response.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct TurnStartResponse {
    pub turn: Turn,
}

/// Turn fields used to correlate lifecycle and extract the final message.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Turn {
    pub id: String,
    pub status: TurnStatus,
    #[serde(default)]
    pub items: Vec<serde_json::Value>,
    pub error: Option<serde_json::Value>,
}

/// Terminal and active turn states.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TurnStatus {
    Completed,
    Interrupted,
    Failed,
    InProgress,
}

/// `turn/completed` notification.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnCompletedNotification {
    pub thread_id: String,
    pub turn: Turn,
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn parses_chatgpt_account_without_accepting_tokens() {
        let response: GetAccountResponse = serde_json::from_value(serde_json::json!({
            "account": {
                "type": "chatgpt",
                "email": "quantity.surveyor@example.com",
                "planType": "business"
            },
            "requiresOpenaiAuth": true
        }))
        .expect("account");
        assert!(matches!(
            response.account,
            Some(Account::Chatgpt {
                plan_type: PlanType::Business,
                ..
            })
        ));
    }

    #[test]
    fn serializes_only_supported_login_modes() {
        let browser = serde_json::to_value(LoginAccountParams::Chatgpt {
            use_hosted_login_success_page: true,
            app_brand: LoginAppBrand::Codex,
        })
        .expect("browser");
        assert_eq!(browser["type"], "chatgpt");
        assert!(browser.get("apiKey").is_none());

        let device =
            serde_json::to_value(LoginAccountParams::ChatgptDeviceCode).expect("device code");
        assert_eq!(device["type"], "chatgptDeviceCode");
    }

    #[test]
    fn parses_rate_limit_snapshot() {
        let response: GetAccountRateLimitsResponse = serde_json::from_value(serde_json::json!({
            "rateLimits": {
                "primary": {
                    "usedPercent": 25,
                    "windowDurationMins": 300,
                    "resetsAt": 1_780_000_000
                },
                "secondary": null,
                "planType": "plus",
                "rateLimitReachedType": null,
                "spendControlReached": false
            },
            "rateLimitResetCredits": null
        }))
        .expect("limits");
        assert_eq!(
            response.rate_limits.primary.expect("primary").used_percent,
            25
        );
    }
}
