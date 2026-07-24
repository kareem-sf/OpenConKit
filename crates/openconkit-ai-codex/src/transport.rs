//! Supervised JSONL-over-stdio transport for the pinned Codex app-server.

use std::collections::HashMap;
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::{broadcast, oneshot, Mutex};
use tokio::task::JoinHandle;

use crate::client::CodexClientConfig;
use crate::diagnostics::{Direction, ProtocolLogger};
use crate::pin::pinned_release;
use crate::profile::prepare_codex_home;
use crate::protocol::{ClientInfo, InitializeParams};
use crate::CodexError;

const MAX_MESSAGE_BYTES: usize = 32 * 1024 * 1024;
const NOTIFICATION_BUFFER: usize = 128;
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(5);

type PendingResponse = oneshot::Sender<Result<Value, CodexError>>;

struct Shared {
    stdin: Mutex<ChildStdin>,
    pending: Mutex<HashMap<u64, PendingResponse>>,
    notifications: broadcast::Sender<CodexNotification>,
    next_request_id: AtomicU64,
    request_timeout: Duration,
    logger: Option<Arc<ProtocolLogger>>,
}

struct ClientInner {
    shared: Arc<Shared>,
    child: Mutex<Child>,
    reader_task: JoinHandle<()>,
    stderr_task: JoinHandle<()>,
}

/// One server notification. Parameters remain opaque until a typed workflow
/// opts into a documented method.
#[derive(Debug, Clone, PartialEq)]
pub struct CodexNotification {
    pub method: String,
    pub params: Value,
}

/// Initialized, supervised Codex app-server connection.
#[derive(Clone)]
pub struct CodexClient {
    inner: Arc<ClientInner>,
}

impl CodexClient {
    /// Verify, launch and initialize one app-server process.
    pub async fn spawn(config: CodexClientConfig, app_version: &str) -> Result<Self, CodexError> {
        prepare_codex_home(config.codex_home())?;
        ensure_empty_working_directory(config.working_directory())?;
        verify_binary_version(&config).await?;

        let mut command = Command::new(config.binary());
        command
            .args(config.args())
            .current_dir(config.working_directory())
            .env("CODEX_HOME", config.codex_home())
            .env_remove("CODEX_ACCESS_TOKEN")
            .env_remove("CODEX_API_KEY")
            .env_remove("OPENAI_API_KEY")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        let mut child = command.spawn()?;
        let stdin = child.stdin.take().ok_or(CodexError::Protocol)?;
        let stdout = child.stdout.take().ok_or(CodexError::Protocol)?;
        let stderr = child.stderr.take().ok_or(CodexError::Protocol)?;
        let (notifications, _) = broadcast::channel(NOTIFICATION_BUFFER);
        let logger = config
            .protocol_log()
            .and_then(|path| ProtocolLogger::new(path).ok())
            .map(Arc::new);
        let shared = Arc::new(Shared {
            stdin: Mutex::new(stdin),
            pending: Mutex::new(HashMap::new()),
            notifications,
            next_request_id: AtomicU64::new(1),
            request_timeout: config.request_timeout(),
            logger,
        });

        let reader_shared = Arc::clone(&shared);
        let reader_task = tokio::spawn(async move {
            read_stdout(stdout, reader_shared).await;
        });
        let stderr_logger = shared.logger.clone();
        let stderr_task = tokio::spawn(async move {
            drain_stderr(stderr, stderr_logger).await;
        });
        let client = Self {
            inner: Arc::new(ClientInner {
                shared,
                child: Mutex::new(child),
                reader_task,
                stderr_task,
            }),
        };

        let initialized = client
            .request_value(
                "initialize",
                serde_json::to_value(InitializeParams {
                    client_info: ClientInfo {
                        name: "openconkit".to_string(),
                        title: Some("OpenConKit".to_string()),
                        version: app_version.to_string(),
                    },
                })?,
                config.request_timeout(),
            )
            .await;
        if let Err(error) = initialized {
            client.shutdown().await;
            return Err(error);
        }
        if let Err(error) = client.send_notification("initialized", json!({})).await {
            client.shutdown().await;
            return Err(error);
        }
        Ok(client)
    }

    /// Subscribe before starting a turn so no completion notification is lost.
    pub fn subscribe(&self) -> broadcast::Receiver<CodexNotification> {
        self.inner.shared.notifications.subscribe()
    }

    /// Send a typed request over the stable protocol.
    pub async fn request<T: DeserializeOwned>(
        &self,
        method: &str,
        params: Value,
    ) -> Result<T, CodexError> {
        let value = self
            .request_value(method, params, self.inner.shared.request_timeout)
            .await?;
        serde_json::from_value(value).map_err(|_| CodexError::Protocol)
    }

    /// Send a typed request with a workflow-specific timeout.
    pub async fn request_with_timeout<T: DeserializeOwned>(
        &self,
        method: &str,
        params: Value,
        timeout: Duration,
    ) -> Result<T, CodexError> {
        let value = self.request_value(method, params, timeout).await?;
        serde_json::from_value(value).map_err(|_| CodexError::Protocol)
    }

    async fn request_value(
        &self,
        method: &str,
        params: Value,
        timeout: Duration,
    ) -> Result<Value, CodexError> {
        if method.is_empty() || timeout.is_zero() {
            return Err(CodexError::InvalidConfiguration(
                "request method and timeout must be non-empty".to_string(),
            ));
        }
        let id = self
            .inner
            .shared
            .next_request_id
            .fetch_add(1, Ordering::Relaxed);
        let message = json!({
            "id": id,
            "method": method,
            "params": params
        });
        let (sender, receiver) = oneshot::channel();
        self.inner.shared.pending.lock().await.insert(id, sender);
        if let Err(error) = write_message(&self.inner.shared, &message).await {
            self.inner.shared.pending.lock().await.remove(&id);
            return Err(error);
        }

        match tokio::time::timeout(timeout, receiver).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err(CodexError::ProcessExited),
            Err(_) => {
                self.inner.shared.pending.lock().await.remove(&id);
                Err(CodexError::Timeout)
            }
        }
    }

    async fn send_notification(&self, method: &str, params: Value) -> Result<(), CodexError> {
        write_message(
            &self.inner.shared,
            &json!({
                "method": method,
                "params": params
            }),
        )
        .await
    }

    /// Stop the subprocess and bounded background readers.
    pub async fn shutdown(&self) {
        {
            let mut child = self.inner.child.lock().await;
            let _ = child.start_kill();
            let _ = tokio::time::timeout(SHUTDOWN_TIMEOUT, child.wait()).await;
        }
        self.inner.reader_task.abort();
        self.inner.stderr_task.abort();
        fail_all_pending(&self.inner.shared).await;
    }
}

async fn verify_binary_version(config: &CodexClientConfig) -> Result<(), CodexError> {
    let expected = pinned_release()?.version;
    let output = tokio::time::timeout(
        config.request_timeout(),
        Command::new(config.binary())
            .arg("--version")
            .current_dir(config.working_directory())
            .env("CODEX_HOME", config.codex_home())
            .env_remove("CODEX_ACCESS_TOKEN")
            .env_remove("CODEX_API_KEY")
            .env_remove("OPENAI_API_KEY")
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .output(),
    )
    .await
    .map_err(|_| CodexError::Timeout)??;
    if !output.status.success() {
        return Err(CodexError::SidecarUnavailable(
            "version check failed".to_string(),
        ));
    }
    let stdout = String::from_utf8(output.stdout).map_err(|_| CodexError::Protocol)?;
    let actual = stdout
        .split_ascii_whitespace()
        .find(|part| {
            part.bytes()
                .all(|byte| byte.is_ascii_digit() || byte == b'.')
                && part.matches('.').count() == 2
        })
        .ok_or(CodexError::Protocol)?;
    if actual != expected {
        return Err(CodexError::SidecarUnavailable(format!(
            "expected version {expected}, found {actual}"
        )));
    }
    Ok(())
}

fn ensure_empty_working_directory(path: &std::path::Path) -> Result<(), CodexError> {
    std::fs::create_dir_all(path)?;
    let mut entries = std::fs::read_dir(path)?;
    if entries.next().transpose()?.is_some() {
        return Err(CodexError::InvalidConfiguration(
            "Codex working directory must be empty".to_string(),
        ));
    }
    Ok(())
}

async fn write_message(shared: &Shared, message: &Value) -> Result<(), CodexError> {
    let mut bytes = serde_json::to_vec(message)?;
    if bytes.len() > MAX_MESSAGE_BYTES {
        return Err(CodexError::MessageTooLarge);
    }
    if let Some(logger) = &shared.logger {
        logger.record_message(Direction::Outbound, message, bytes.len());
    }
    bytes.push(b'\n');
    let mut stdin = shared.stdin.lock().await;
    stdin.write_all(&bytes).await?;
    stdin.flush().await?;
    Ok(())
}

async fn read_stdout(mut stdout: tokio::process::ChildStdout, shared: Arc<Shared>) {
    let mut chunk = [0_u8; 16 * 1024];
    let mut buffered = Vec::new();
    loop {
        let read = match stdout.read(&mut chunk).await {
            Ok(0) => break,
            Ok(read) => read,
            Err(_) => break,
        };
        buffered.extend_from_slice(&chunk[..read]);
        if buffered.len() > MAX_MESSAGE_BYTES && !buffered.contains(&b'\n') {
            break;
        }

        let mut consumed = 0;
        while let Some(relative_end) = buffered[consumed..].iter().position(|byte| *byte == b'\n') {
            let end = consumed + relative_end;
            let line = &buffered[consumed..end];
            if line.len() > MAX_MESSAGE_BYTES {
                fail_all_pending(&shared).await;
                return;
            }
            if !line.is_empty() && !handle_line(line, &shared).await {
                fail_all_pending(&shared).await;
                return;
            }
            consumed = end + 1;
        }
        if consumed > 0 {
            buffered.drain(..consumed);
        }
    }
    fail_all_pending(&shared).await;
}

async fn handle_line(line: &[u8], shared: &Shared) -> bool {
    let Ok(message) = serde_json::from_slice::<Value>(line) else {
        if let Some(logger) = &shared.logger {
            logger.record_malformed(Direction::Inbound, line.len());
        }
        return false;
    };
    if let Some(logger) = &shared.logger {
        logger.record_message(Direction::Inbound, &message, line.len());
    }
    match classify_message(message) {
        InboundMessage::Response { id, result } => {
            if let Some(sender) = shared.pending.lock().await.remove(&id) {
                let _ = sender.send(result);
            }
        }
        InboundMessage::Notification(notification) => {
            let _ = shared.notifications.send(notification);
        }
        InboundMessage::ServerRequest { id } => {
            let _ = write_message(
                shared,
                &json!({
                    "id": id,
                    "error": {
                        "code": -32601,
                        "message": "OpenConKit does not permit server-initiated actions"
                    }
                }),
            )
            .await;
        }
        InboundMessage::Invalid => return false,
    }
    true
}

enum InboundMessage {
    Response {
        id: u64,
        result: Result<Value, CodexError>,
    },
    Notification(CodexNotification),
    ServerRequest {
        id: Value,
    },
    Invalid,
}

fn classify_message(message: Value) -> InboundMessage {
    let method = message.get("method").and_then(Value::as_str);
    let id_value = message.get("id");
    if let (Some(_), Some(id)) = (method, id_value) {
        return InboundMessage::ServerRequest { id: id.clone() };
    }
    if let Some(method) = method {
        return InboundMessage::Notification(CodexNotification {
            method: method.to_string(),
            params: message.get("params").cloned().unwrap_or(Value::Null),
        });
    }
    let Some(id) = id_value.and_then(Value::as_u64) else {
        return InboundMessage::Invalid;
    };
    if let Some(result) = message.get("result") {
        return InboundMessage::Response {
            id,
            result: Ok(result.clone()),
        };
    }
    let code = message
        .get("error")
        .and_then(|error| error.get("code"))
        .and_then(Value::as_i64);
    match code {
        Some(code) => InboundMessage::Response {
            id,
            result: Err(CodexError::Server { code }),
        },
        None => InboundMessage::Invalid,
    }
}

async fn fail_all_pending(shared: &Shared) {
    let pending = {
        let mut pending = shared.pending.lock().await;
        std::mem::take(&mut *pending)
    };
    for (_, sender) in pending {
        let _ = sender.send(Err(CodexError::ProcessExited));
    }
}

async fn drain_stderr(
    mut stderr: tokio::process::ChildStderr,
    logger: Option<Arc<ProtocolLogger>>,
) {
    let mut buffer = [0_u8; 16 * 1024];
    loop {
        match stderr.read(&mut buffer).await {
            Ok(0) | Err(_) => break,
            Ok(read) => {
                if let Some(logger) = &logger {
                    logger.record_stderr(read);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    #[test]
    fn response_error_discards_sensitive_message() {
        let inbound = classify_message(json!({
            "id": 9,
            "error": {
                "code": -32000,
                "message": "token for person@example.com failed"
            }
        }));
        match inbound {
            InboundMessage::Response {
                id,
                result: Err(CodexError::Server { code }),
            } => {
                assert_eq!(id, 9);
                assert_eq!(code, -32000);
            }
            _ => panic!("unexpected envelope"),
        }
    }

    #[test]
    fn server_requests_are_classified_fail_closed() {
        let inbound = classify_message(json!({
            "id": "approval-1",
            "method": "item/commandExecution/requestApproval",
            "params": {"command": "type secret.txt"}
        }));
        assert!(matches!(inbound, InboundMessage::ServerRequest { .. }));
    }

    #[test]
    fn notifications_preserve_only_method_and_params() {
        let inbound = classify_message(json!({
            "method": "turn/completed",
            "params": {"threadId": "thread-1", "turn": {"id": "turn-1"}}
        }));
        match inbound {
            InboundMessage::Notification(notification) => {
                assert_eq!(notification.method, "turn/completed");
                assert_eq!(notification.params["threadId"], "thread-1");
            }
            _ => panic!("unexpected envelope"),
        }
    }

    #[test]
    fn synthetic_jsonl_contract_fixture_classifies_expected_envelopes() {
        let fixture = include_str!("../tests/fixtures/protocol/envelopes.jsonl");
        let envelopes = fixture
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).expect("fixture JSON"))
            .map(classify_message)
            .collect::<Vec<_>>();
        assert_eq!(envelopes.len(), 3);
        assert!(matches!(
            &envelopes[0],
            InboundMessage::Response {
                id: 11,
                result: Ok(_)
            }
        ));
        assert!(matches!(
            &envelopes[1],
            InboundMessage::Notification(CodexNotification { method, .. })
                if method == "account/rateLimits/updated"
        ));
        assert!(matches!(
            &envelopes[2],
            InboundMessage::ServerRequest { .. }
        ));
    }
}
