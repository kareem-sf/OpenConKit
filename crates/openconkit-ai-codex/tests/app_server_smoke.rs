//! Opt-in smoke test against the staged pinned app-server binary.
//!
//! CI uses synthetic protocol fixtures and never performs a live login or
//! paid request. Release preparation can set `OPENCONKIT_CODEX_SMOKE_BINARY`
//! and run this ignored test to verify initialization and the logged-out
//! account state.

#![allow(clippy::expect_used)]

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use openconkit_ai_codex::protocol::GetAccountResponse;
use openconkit_ai_codex::{CodexClient, CodexClientConfig};
use serde_json::json;

#[tokio::test]
#[ignore = "requires OPENCONKIT_CODEX_SMOKE_BINARY"]
async fn initializes_pinned_binary_in_logged_out_mode() {
    let binary = PathBuf::from(
        std::env::var_os("OPENCONKIT_CODEX_SMOKE_BINARY")
            .expect("set OPENCONKIT_CODEX_SMOKE_BINARY"),
    );
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("openconkit-codex-smoke-{suffix}"));
    let home = root.join("codex-home");
    let sandbox = root.join("sandbox");
    let config =
        CodexClientConfig::standalone(binary, home, sandbox).expect("valid smoke configuration");
    let client = CodexClient::spawn(config, env!("CARGO_PKG_VERSION"))
        .await
        .expect("initialize app-server");
    let account: GetAccountResponse = client
        .request("account/read", json!({"refreshToken": false}))
        .await
        .expect("read logged-out account");
    assert!(account.account.is_none());
    client.shutdown().await;
    std::fs::remove_dir_all(root).expect("cleanup");
}
