//! Typed stable account and tool-free analysis workflows.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use serde_json::{json, Value};
use tokio::sync::broadcast;
use tokio::sync::Notify;
use url::Url;

use crate::protocol::{
    Account, GetAccountRateLimitsResponse, GetAccountResponse, LoginAccountParams,
    LoginAccountResponse, LoginAppBrand, ThreadStartResponse, TurnCompletedNotification,
    TurnStartResponse, TurnStatus,
};
use crate::{CodexClient, CodexError};

/// Explicit quality-first model selected for the v0.0.1 grounded review.
pub const ANALYSIS_MODEL: &str = "gpt-5.6-sol";
const ANALYSIS_EFFORT: &str = "medium";

struct CancellationInner {
    cancelled: AtomicBool,
    notify: Notify,
}

/// Cooperative cancellation shared between the desktop command and one
/// active Codex turn.
#[derive(Clone)]
pub struct CodexCancellationToken {
    inner: Arc<CancellationInner>,
}

impl CodexCancellationToken {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(CancellationInner {
                cancelled: AtomicBool::new(false),
                notify: Notify::new(),
            }),
        }
    }

    /// Mark the turn cancelled and wake its waiter.
    pub fn cancel(&self) {
        if !self.inner.cancelled.swap(true, Ordering::SeqCst) {
            self.inner.notify.notify_waiters();
        }
    }

    /// Whether cancellation has already been requested.
    pub fn is_cancelled(&self) -> bool {
        self.inner.cancelled.load(Ordering::SeqCst)
    }

    async fn cancelled(&self) {
        if !self.is_cancelled() {
            self.inner.notify.notified().await;
        }
    }
}

impl Default for CodexCancellationToken {
    fn default() -> Self {
        Self::new()
    }
}

/// One grounded, schema-constrained analysis request.
pub struct CodexAnalysisRequest {
    /// Empty run-specific working directory.
    pub sandbox_directory: PathBuf,
    /// Tool-owned developer instructions containing the grounding boundary.
    pub developer_instructions: String,
    /// Exact normalized context and user-visible task.
    pub input: String,
    /// Strict JSON Schema for the final assistant message.
    pub output_schema: Value,
    /// Maximum duration of the paid model turn.
    pub timeout: Duration,
    /// User-driven cooperative cancellation.
    pub cancellation: CodexCancellationToken,
}

/// Valid JSON returned by the schema-constrained Codex turn.
pub struct CodexAnalysisResponse {
    /// Actual model selected by app-server for the thread.
    pub model: String,
    /// Structured assistant output, before tool-specific semantic validation.
    pub output: Value,
}

/// High-level stable operations over one supervised client.
#[derive(Clone)]
pub struct CodexService {
    client: CodexClient,
}

impl CodexService {
    /// Wrap an initialized transport.
    pub fn new(client: CodexClient) -> Self {
        Self { client }
    }

    /// Read the current safe account snapshot.
    pub async fn account(&self, refresh_token: bool) -> Result<GetAccountResponse, CodexError> {
        let response: GetAccountResponse = self
            .client
            .request(
                "account/read",
                json!({
                    "refreshToken": refresh_token
                }),
            )
            .await?;
        if matches!(
            response.account,
            Some(Account::ApiKey | Account::AmazonBedrock { .. })
        ) {
            return Err(CodexError::UnsupportedAuthentication);
        }
        Ok(response)
    }

    /// Begin the Codex-managed ChatGPT browser login flow.
    pub async fn start_browser_login(&self) -> Result<LoginAccountResponse, CodexError> {
        let response: LoginAccountResponse = self
            .client
            .request(
                "account/login/start",
                serde_json::to_value(LoginAccountParams::Chatgpt {
                    use_hosted_login_success_page: true,
                    app_brand: LoginAppBrand::Codex,
                })?,
            )
            .await?;
        match &response {
            LoginAccountResponse::Chatgpt { auth_url, .. } => {
                validate_login_url(auth_url)?;
                Ok(response)
            }
            _ => Err(CodexError::Protocol),
        }
    }

    /// Begin the documented device-code fallback.
    pub async fn start_device_login(&self) -> Result<LoginAccountResponse, CodexError> {
        let response: LoginAccountResponse = self
            .client
            .request(
                "account/login/start",
                serde_json::to_value(LoginAccountParams::ChatgptDeviceCode)?,
            )
            .await?;
        match &response {
            LoginAccountResponse::ChatgptDeviceCode {
                verification_url, ..
            } => {
                validate_login_url(verification_url)?;
                Ok(response)
            }
            _ => Err(CodexError::Protocol),
        }
    }

    /// Cancel a pending login by the opaque ID returned by app-server.
    pub async fn cancel_login(&self, login_id: &str) -> Result<(), CodexError> {
        validate_opaque_id(login_id)?;
        let _: Value = self
            .client
            .request(
                "account/login/cancel",
                json!({
                    "loginId": login_id
                }),
            )
            .await?;
        Ok(())
    }

    /// Log out through app-server so Codex removes its own credentials.
    pub async fn logout(&self) -> Result<(), CodexError> {
        let _: Value = self.client.request("account/logout", json!({})).await?;
        Ok(())
    }

    /// Refresh ChatGPT rate limits.
    pub async fn rate_limits(&self) -> Result<GetAccountRateLimitsResponse, CodexError> {
        self.client
            .request("account/rateLimits/read", json!({}))
            .await
    }

    /// Run one ephemeral, read-only, approval-free structured analysis.
    pub async fn analyze(
        &self,
        request: CodexAnalysisRequest,
    ) -> Result<CodexAnalysisResponse, CodexError> {
        if request.cancellation.is_cancelled() {
            return Err(CodexError::Cancelled);
        }
        validate_analysis_request(&request)?;
        let mut notifications = self.client.subscribe();
        let thread: ThreadStartResponse = self
            .client
            .request(
                "thread/start",
                json!({
                    "model": ANALYSIS_MODEL,
                    "cwd": request.sandbox_directory,
                    "approvalPolicy": "never",
                    "sandbox": "read-only",
                    "developerInstructions": request.developer_instructions,
                    "ephemeral": true
                }),
            )
            .await?;
        let thread_id = thread.thread.id.clone();
        validate_opaque_id(&thread_id)?;
        let started: TurnStartResponse = self
            .client
            .request(
                "turn/start",
                json!({
                    "threadId": thread_id,
                    "input": [{
                        "type": "text",
                        "text": request.input
                    }],
                    "effort": ANALYSIS_EFFORT,
                    "approvalPolicy": "never",
                    "sandboxPolicy": {
                        "type": "readOnly",
                        "networkAccess": false
                    },
                    "outputSchema": request.output_schema
                }),
            )
            .await?;
        let turn_id = started.turn.id;
        validate_opaque_id(&turn_id)?;

        let completion = tokio::select! {
            _ = request.cancellation.cancelled() => {
                interrupt_turn(&self.client, &thread_id, &turn_id).await;
                return Err(CodexError::Cancelled);
            }
            result = tokio::time::timeout(
                request.timeout,
                wait_for_completion(&mut notifications, &thread_id, &turn_id),
            ) => result,
        };
        let completed = match completion {
            Ok(result) => result?,
            Err(_) => {
                interrupt_turn(&self.client, &thread_id, &turn_id).await;
                return Err(CodexError::Timeout);
            }
        };
        if completed.turn.status != TurnStatus::Completed {
            return Err(CodexError::AnalysisFailed);
        }
        let output = extract_safe_final_output(&completed.turn.items)?;
        Ok(CodexAnalysisResponse {
            model: thread.model,
            output,
        })
    }

    /// Stop the underlying child process.
    pub async fn shutdown(&self) {
        self.client.shutdown().await;
    }
}

async fn interrupt_turn(client: &CodexClient, thread_id: &str, turn_id: &str) {
    let _: Result<Value, CodexError> = client
        .request(
            "turn/interrupt",
            json!({
                "threadId": thread_id,
                "turnId": turn_id
            }),
        )
        .await;
}

async fn wait_for_completion(
    notifications: &mut broadcast::Receiver<crate::CodexNotification>,
    thread_id: &str,
    turn_id: &str,
) -> Result<TurnCompletedNotification, CodexError> {
    loop {
        let notification = notifications.recv().await.map_err(|error| match error {
            broadcast::error::RecvError::Closed => CodexError::ProcessExited,
            broadcast::error::RecvError::Lagged(_) => CodexError::Protocol,
        })?;
        if notification.method != "turn/completed" {
            continue;
        }
        let completion: TurnCompletedNotification =
            serde_json::from_value(notification.params).map_err(|_| CodexError::Protocol)?;
        if completion.thread_id == thread_id && completion.turn.id == turn_id {
            return Ok(completion);
        }
    }
}

fn extract_safe_final_output(items: &[Value]) -> Result<Value, CodexError> {
    let mut final_text = None;
    for item in items {
        let item_type = item
            .get("type")
            .and_then(Value::as_str)
            .ok_or(CodexError::Protocol)?;
        match item_type {
            "userMessage" | "reasoning" => {}
            "agentMessage" => {
                let text = item
                    .get("text")
                    .and_then(Value::as_str)
                    .ok_or(CodexError::Protocol)?;
                final_text = Some(text);
            }
            _ => return Err(CodexError::UnsafeActivity),
        }
    }
    let text = final_text.ok_or(CodexError::AnalysisFailed)?;
    serde_json::from_str(text).map_err(|_| CodexError::AnalysisFailed)
}

fn validate_analysis_request(request: &CodexAnalysisRequest) -> Result<(), CodexError> {
    if !request.sandbox_directory.is_absolute()
        || request.developer_instructions.trim().is_empty()
        || request.input.trim().is_empty()
        || request.timeout.is_zero()
        || !request.output_schema.is_object()
    {
        return Err(CodexError::InvalidConfiguration(
            "analysis request is incomplete".to_string(),
        ));
    }
    let mut entries = std::fs::read_dir(&request.sandbox_directory)?;
    if entries.next().transpose()?.is_some() {
        return Err(CodexError::InvalidConfiguration(
            "analysis sandbox must be empty".to_string(),
        ));
    }
    Ok(())
}

fn validate_opaque_id(value: &str) -> Result<(), CodexError> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-_".contains(&byte))
    {
        return Err(CodexError::Protocol);
    }
    Ok(())
}

fn validate_login_url(value: &str) -> Result<(), CodexError> {
    let url = Url::parse(value).map_err(|_| CodexError::Protocol)?;
    let allowed_host = matches!(url.host_str(), Some("chatgpt.com" | "auth.openai.com"));
    if url.scheme() != "https"
        || !allowed_host
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some()
    {
        return Err(CodexError::Protocol);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn login_urls_are_strictly_allowlisted() {
        for allowed in [
            "https://chatgpt.com/auth/callback?state=opaque",
            "https://auth.openai.com/codex/device",
        ] {
            validate_login_url(allowed).expect("allowed");
        }
        for denied in [
            "http://chatgpt.com/auth",
            "https://chatgpt.com.evil.example/auth",
            "https://user:secret@chatgpt.com/auth",
            "https://chatgpt.com:444/auth",
        ] {
            assert!(validate_login_url(denied).is_err(), "{denied}");
        }
    }

    #[test]
    fn safe_output_rejects_any_tool_activity() {
        let items = vec![
            json!({"type": "userMessage", "id": "u", "content": []}),
            json!({
                "type": "commandExecution",
                "id": "cmd",
                "command": "type workbook.xlsx"
            }),
            json!({"type": "agentMessage", "id": "a", "text": "{\"summary\":\"x\"}"}),
        ];
        assert!(matches!(
            extract_safe_final_output(&items),
            Err(CodexError::UnsafeActivity)
        ));
    }

    #[test]
    fn safe_output_requires_plain_json_agent_message() {
        let valid = vec![json!({
            "type": "agentMessage",
            "id": "a",
            "text": "{\"summary\":\"Review the high-value items.\"}"
        })];
        assert_eq!(
            extract_safe_final_output(&valid).expect("valid"),
            json!({"summary": "Review the high-value items."})
        );
        let fenced = vec![json!({
            "type": "agentMessage",
            "id": "a",
            "text": "```json\n{\"summary\":\"x\"}\n```"
        })];
        assert!(matches!(
            extract_safe_final_output(&fenced),
            Err(CodexError::AnalysisFailed)
        ));
    }
}
