//! Tauri commands exposed to the frontend.
//!
//! IPC surface is kept minimal and explicit; every command is listed in
//! `invoke_handler!` and validated by the capabilities file
//! (`capabilities/default.json`). See `docs/threat-model.md`.

use std::collections::HashMap;
use std::fs;
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use jiff::Timestamp;
use openconkit_ai_codex::protocol::{
    Account, GetAccountRateLimitsResponse, GetAccountResponse, LoginAccountResponse, PlanType,
    RateLimitWindow,
};
use openconkit_ai_codex::{
    pinned_release, CodexAnalysisRequest, CodexAnalysisResponse, CodexCancellationToken,
    CodexError, CodexService, ANALYSIS_MODEL,
};
use openconkit_application::{
    AiAccountSnapshot, AiAnalysisRepository, AiLoginChallenge, AiLoginMode, AiPlanType,
    AiRateLimitSnapshot, AiRateLimitWindow, AiReviewScope, AiRuntimeStatus, AnalysisRunRepository,
    AppSettings, ArchiveProject, BootstrapStatus, ExportRepository, HomeLayout, ImportSource,
    ListAnalysisRuns, ListProjects, ListRunHistory, ListSourceRevisions, OpenAnalysisRun,
    QuickImport, RegisterProject, RunDetails, RunHistoryEntry, SettingsPatch, SourceImportPolicy,
    SourceRevisionRepository,
};
use openconkit_domain::{
    AiAnalysis, AiAnalysisId, AiAnalysisLanguage, AiAnalysisStatus, AiGroundingStatus,
    AiValidationStatus, AnalysisRun, AnalysisRunId, ExportId, ExportKind, ExportRecord, Finding,
    Project, ProjectId, RunStatus, Sha256Hash, SourceRevision, SourceRevisionId,
    WorkbookDiagnostics,
};
use openconkit_storage::{
    Database, FsSourceStorage, SettingsStore, SqliteAiAnalysisRepository,
    SqliteAnalysisRunRepository, SqliteExportRepository, SqliteFindingRepository,
    SqliteProjectRepository, SqliteSourceRevisionRepository,
};
use openconkit_tool_sdk::{
    AiPreparedContext, AiPromptChunk, AiProviderError, CancellationToken, ExportContext,
    ToolAiProvider, ToolManifest, ToolNavItem, ToolProgress, ToolProgressEvent, ToolRegistry,
    ToolRunContext, ToolRunResponse, TOOL_CONTRACT_VERSION,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter, State};

use crate::error::DesktopError;
use crate::state::AppState;

/// Application version, matching the root `VERSION` file.
#[tauri::command]
pub fn app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Resolved application home directory (absolute path string).
#[tauri::command]
pub fn openconkit_home(state: State<'_, AppState>) -> String {
    state.home.to_string_lossy().into_owned()
}

/// Bootstrap status from this launch (created_fresh, migrations, warnings…).
#[tauri::command]
pub fn bootstrap_status(state: State<'_, AppState>) -> BootstrapStatus {
    state.bootstrap.clone()
}

/// Current application settings.
#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> Result<AppSettings, DesktopError> {
    Ok(state.settings()?.clone())
}

/// Apply a typed settings patch and persist it.
#[tauri::command]
pub fn update_settings(
    state: State<'_, AppState>,
    patch: SettingsPatch,
) -> Result<AppSettings, DesktopError> {
    let mut settings = state.settings()?;
    let mut update_channel = state.update_channel()?;
    let mut next_settings = settings.clone();
    next_settings.apply_typed_patch(patch)?;
    let mut next_update_channel = update_channel.clone();
    next_update_channel.channel = next_settings.update_channel;
    next_update_channel.last_successful_update_check = next_settings.last_successful_update_check;

    let store = SettingsStore::new(&state.home);
    store.save_update_channel(&next_update_channel)?;
    if let Err(err) = store.save_settings(&next_settings) {
        if let Err(rollback) = store.save_update_channel(&update_channel) {
            return Err(DesktopError::Storage(format!(
                "{err}; update-channel rollback also failed: {rollback}"
            )));
        }
        return Err(err.into());
    }

    *settings = next_settings.clone();
    *update_channel = next_update_channel;
    Ok(next_settings)
}

/// Report local AI readiness without launching Codex or using the network.
#[tauri::command]
pub async fn ai_runtime_status(
    state: State<'_, AppState>,
) -> Result<AiRuntimeStatus, DesktopError> {
    let enabled = state.settings()?.privacy.ai_features_enabled;
    let runtime = state.codex.lock().await;
    Ok(AiRuntimeStatus {
        enabled,
        bundled_runtime_available: runtime.bundled_binary_available(),
        selected_runtime_available: runtime.binary_available(),
        using_system_runtime: runtime.using_system_binary(),
        codex_version: runtime.pinned_version()?,
    })
}

/// Read safe ChatGPT account metadata. Tokens never cross this boundary.
#[tauri::command(rename_all = "snake_case")]
pub async fn get_ai_account(
    state: State<'_, AppState>,
    refresh_token: bool,
) -> Result<AiAccountSnapshot, DesktopError> {
    let service = codex_service(&state).await?;
    match service.account(refresh_token).await {
        Ok(response) => map_account_snapshot(response),
        Err(error) => {
            invalidate_codex_on_failure(&state, &error).await;
            Err(error.into())
        }
    }
}

/// Begin an explicit ChatGPT browser or device-code login and open only the
/// URL already allowlisted by the typed Codex service.
#[tauri::command(rename_all = "snake_case")]
pub async fn start_ai_login(
    state: State<'_, AppState>,
    mode: AiLoginMode,
) -> Result<AiLoginChallenge, DesktopError> {
    let service = codex_service(&state).await?;
    let response = match mode {
        AiLoginMode::Browser => service.start_browser_login().await,
        AiLoginMode::DeviceCode => service.start_device_login().await,
    };
    let response = match response {
        Ok(response) => response,
        Err(error) => {
            invalidate_codex_on_failure(&state, &error).await;
            return Err(error.into());
        }
    };
    let (login_id, url, user_code) = match response {
        LoginAccountResponse::Chatgpt { login_id, auth_url } => (login_id, auth_url, None),
        LoginAccountResponse::ChatgptDeviceCode {
            login_id,
            verification_url,
            user_code,
        } => (login_id, verification_url, Some(user_code)),
        LoginAccountResponse::Unsupported => {
            return Err(DesktopError::Coded {
                code: "AI_PROTOCOL_INCOMPATIBLE",
                message: "Codex returned an unsupported login challenge".to_string(),
            });
        }
    };
    if let Err(error) = tauri_plugin_opener::open_url(&url, None::<&str>) {
        let _ = service.cancel_login(&login_id).await;
        return Err(DesktopError::Coded {
            code: "AI_BROWSER_OPEN_FAILED",
            message: error.to_string(),
        });
    }
    Ok(AiLoginChallenge {
        login_id,
        mode,
        user_code,
    })
}

/// Cancel a pending Codex-managed login.
#[tauri::command(rename_all = "snake_case")]
pub async fn cancel_ai_login(
    state: State<'_, AppState>,
    login_id: String,
) -> Result<(), DesktopError> {
    let service = codex_service(&state).await?;
    match service.cancel_login(&login_id).await {
        Ok(()) => Ok(()),
        Err(error) => {
            invalidate_codex_on_failure(&state, &error).await;
            Err(error.into())
        }
    }
}

/// Log out through Codex so it removes its own credential-store entry.
#[tauri::command]
pub async fn logout_ai(state: State<'_, AppState>) -> Result<(), DesktopError> {
    let service = codex_service(&state).await?;
    match service.logout().await {
        Ok(()) => Ok(()),
        Err(error) => {
            invalidate_codex_on_failure(&state, &error).await;
            Err(error.into())
        }
    }
}

/// Refresh the safe ChatGPT plan/rate-limit snapshot.
#[tauri::command]
pub async fn get_ai_rate_limits(
    state: State<'_, AppState>,
) -> Result<AiRateLimitSnapshot, DesktopError> {
    let service = codex_service(&state).await?;
    match service.rate_limits().await {
        Ok(response) => map_rate_limits(response),
        Err(error) => {
            invalidate_codex_on_failure(&state, &error).await;
            Err(error.into())
        }
    }
}

/// Build the exact grounded scope shown in the first-use consent dialog. No
/// process or network request occurs.
#[tauri::command(rename_all = "snake_case")]
pub async fn prepare_ai_review(
    state: State<'_, AppState>,
    run_id: String,
    language: String,
) -> Result<AiReviewScope, DesktopError> {
    require_ai_enabled(&state)?;
    let database = Arc::clone(&state.database);
    let tools = Arc::clone(&state.tools);
    tauri::async_runtime::spawn_blocking(move || {
        prepare_grounded_review(&database, &tools, &run_id, &language).map(|review| review.scope)
    })
    .await
    .map_err(|error| DesktopError::BackgroundTask(error.to_string()))?
}

/// Run one explicitly consented, grounded AI review and persist its lifecycle
/// independently from deterministic findings.
#[tauri::command(rename_all = "snake_case")]
pub async fn run_ai_review(
    state: State<'_, AppState>,
    run_id: String,
    language: String,
    input_scope_hash: String,
    consent: bool,
) -> Result<AiAnalysis, DesktopError> {
    require_ai_enabled(&state)?;
    if !consent {
        return Err(DesktopError::Coded {
            code: "AI_CONSENT_REQUIRED",
            message: "explicit consent is required for each transmitted scope".to_string(),
        });
    }
    let expected_scope_hash = Sha256Hash::from_hex(&input_scope_hash)
        .map_err(|error| DesktopError::InvalidInput(error.to_string()))?;
    let database = Arc::clone(&state.database);
    let tools = Arc::clone(&state.tools);
    let prepared = tauri::async_runtime::spawn_blocking({
        let run_id = run_id.clone();
        let language = language.clone();
        let database = Arc::clone(&database);
        let tools = Arc::clone(&tools);
        move || prepare_grounded_review(&database, &tools, &run_id, &language)
    })
    .await
    .map_err(|error| DesktopError::BackgroundTask(error.to_string()))??;
    if prepared.scope.input_scope_hash != expected_scope_hash {
        return Err(DesktopError::Coded {
            code: "AI_SCOPE_CHANGED",
            message: "the authoritative AI scope changed after consent".to_string(),
        });
    }

    let run_id = prepared.scope.run_id;
    let cancellation = CodexCancellationToken::new();
    {
        let mut active = state.active_ai_runs()?;
        if active.contains_key(&run_id) {
            return Err(DesktopError::Coded {
                code: "AI_ALREADY_RUNNING",
                message: "an AI review is already active for this run".to_string(),
            });
        }
        active.insert(run_id, cancellation.clone());
    }
    let _active_guard = ActiveAiRunGuard {
        active_runs: Arc::clone(&state.active_ai_runs),
        run_id,
    };

    let service = codex_service(&state).await?;
    let account = match service.account(false).await {
        Ok(account) => account,
        Err(error) => {
            invalidate_codex_on_failure(&state, &error).await;
            return Err(error.into());
        }
    };
    if !matches!(account.account, Some(Account::Chatgpt { .. })) {
        return Err(DesktopError::Coded {
            code: "AI_LOGIN_REQUIRED",
            message: "ChatGPT login is required".to_string(),
        });
    }

    let analysis_id = AiAnalysisId::new();
    let codex_version = pinned_release()?.version;
    let mut analysis = AiAnalysis {
        id: analysis_id,
        run_id,
        model: ANALYSIS_MODEL.to_string(),
        codex_version,
        language: if language == "ar" {
            AiAnalysisLanguage::Ar
        } else {
            AiAnalysisLanguage::En
        },
        input_scope_hash: prepared.scope.input_scope_hash.clone(),
        status: AiAnalysisStatus::Pending,
        validation_status: AiValidationStatus::Unvalidated,
        grounding_status: AiGroundingStatus::Pending,
        output: None,
        created_at: Timestamp::now(),
    };
    SqliteAiAnalysisRepository::new(&database).save(&analysis)?;

    let sandbox_relative = format!("{}/{}/{}", HomeLayout::AI_SANDBOX_DIR, run_id, analysis_id);
    let sandbox = match create_confined_directory(&state.home, &sandbox_relative) {
        Ok(path) => path,
        Err(error) => {
            analysis.status = AiAnalysisStatus::Failed;
            SqliteAiAnalysisRepository::new(&database).save(&analysis)?;
            return Err(error);
        }
    };
    let provider = tools
        .get(&prepared.tool_id)
        .and_then(|tool| tool.ai_provider())
        .ok_or_else(|| DesktopError::Coded {
            code: "AI_NOT_SUPPORTED",
            message: "the run tool has no AI provider".to_string(),
        })?;
    let response = execute_grounded_plan(
        &service,
        provider,
        &prepared,
        &language,
        &sandbox,
        cancellation,
    )
    .await;
    cleanup_ai_sandbox(&state.home, &sandbox);

    let response = match response {
        Ok(response) => response,
        Err(GroundedExecutionError::Codex(error)) => {
            analysis.status = AiAnalysisStatus::Failed;
            if matches!(
                error,
                CodexError::AnalysisFailed | CodexError::UnsafeActivity | CodexError::Protocol
            ) {
                analysis.grounding_status = AiGroundingStatus::Rejected;
            }
            SqliteAiAnalysisRepository::new(&database).save(&analysis)?;
            invalidate_codex_on_failure(&state, &error).await;
            return Err(error.into());
        }
        Err(GroundedExecutionError::Grounding(error)) => {
            analysis.status = AiAnalysisStatus::Failed;
            analysis.grounding_status = AiGroundingStatus::Rejected;
            analysis.output = None;
            SqliteAiAnalysisRepository::new(&database).save(&analysis)?;
            return Err(DesktopError::Coded {
                code: match &error {
                    AiProviderError::ContextTooLarge => "AI_SCOPE_TOO_LARGE",
                    _ => "AI_GROUNDING_REJECTED",
                },
                message: error.to_string(),
            });
        }
    };

    analysis.model = response.model;
    analysis.status = AiAnalysisStatus::Completed;
    analysis.grounding_status = AiGroundingStatus::Validated;
    analysis.output = Some(response.output);
    SqliteAiAnalysisRepository::new(&database).save(&analysis)?;
    Ok(analysis)
}

/// Request cancellation of the active paid turn for a run.
#[tauri::command(rename_all = "snake_case")]
pub fn cancel_ai_review(state: State<'_, AppState>, run_id: String) -> Result<bool, DesktopError> {
    let run_id = AnalysisRunId::parse(&run_id)
        .map_err(|error| DesktopError::InvalidInput(error.to_string()))?;
    let active = state.active_ai_runs()?;
    if let Some(token) = active.get(&run_id) {
        token.cancel();
        Ok(true)
    } else {
        Ok(false)
    }
}

const MAX_AI_PROMPT_BYTES: usize = 1536 * 1024;
const MAX_AI_INTERMEDIATE_OUTPUT_BYTES: usize = 64 * 1024;

struct PreparedGroundedReview {
    scope: AiReviewScope,
    tool_id: String,
    context: AiPreparedContext,
    developer_instructions: String,
    source_chunks: Vec<AiPromptChunk>,
    intermediate_output_schema: Value,
    final_output_schema: Value,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AiScopeEnvelope<'a> {
    contract_version: u32,
    run_id: &'a AnalysisRunId,
    source_sha256: &'a Sha256Hash,
    tool_id: &'a str,
    tool_version: &'a str,
    rule_set_version: &'a str,
    language: &'a str,
    model: &'a str,
    developer_instructions: &'a str,
    source_prompts: Vec<&'a str>,
    intermediate_output_schema: &'a Value,
    final_output_schema: &'a Value,
}

enum GroundedExecutionError {
    Codex(CodexError),
    Grounding(AiProviderError),
}

impl From<CodexError> for GroundedExecutionError {
    fn from(error: CodexError) -> Self {
        Self::Codex(error)
    }
}

impl From<AiProviderError> for GroundedExecutionError {
    fn from(error: AiProviderError) -> Self {
        Self::Grounding(error)
    }
}

async fn execute_grounded_plan(
    service: &CodexService,
    provider: &dyn ToolAiProvider,
    prepared: &PreparedGroundedReview,
    language: &str,
    sandbox: &Path,
    cancellation: CodexCancellationToken,
) -> Result<CodexAnalysisResponse, GroundedExecutionError> {
    let chunked = prepared.source_chunks.len() > 1;
    let source_schema = if chunked {
        &prepared.intermediate_output_schema
    } else {
        &prepared.final_output_schema
    };
    let mut validated_outputs = Vec::with_capacity(prepared.source_chunks.len());
    for chunk in &prepared.source_chunks {
        let response = execute_codex_turn(
            service,
            sandbox,
            &prepared.developer_instructions,
            &chunk.input,
            source_schema,
            &cancellation,
        )
        .await?;
        let model = response.model;
        let validated = provider.validate_output(&chunk.validation_context, response.output)?;
        if !chunked {
            return Ok(CodexAnalysisResponse {
                model,
                output: validated,
            });
        }
        require_bounded_intermediate(&validated)?;
        validated_outputs.push(validated);
    }

    loop {
        let final_prompt = provider.synthesis_prompt(language, &validated_outputs)?;
        if final_prompt.len() <= MAX_AI_PROMPT_BYTES {
            let response = execute_codex_turn(
                service,
                sandbox,
                &prepared.developer_instructions,
                &final_prompt,
                &prepared.final_output_schema,
                &cancellation,
            )
            .await?;
            let output = provider.validate_output(&prepared.context, response.output)?;
            return Ok(CodexAnalysisResponse {
                model: response.model,
                output,
            });
        }

        let groups = partition_synthesis_outputs(provider, language, &validated_outputs)?;
        if groups.len() >= validated_outputs.len() {
            return Err(AiProviderError::ContextTooLarge.into());
        }
        let mut reduced = Vec::with_capacity(groups.len());
        for group in groups {
            if group.len() == 1 {
                if let Some(output) = group.into_iter().next() {
                    reduced.push(output);
                }
                continue;
            }
            let prompt = provider.synthesis_prompt(language, &group)?;
            if prompt.len() > MAX_AI_PROMPT_BYTES {
                return Err(AiProviderError::ContextTooLarge.into());
            }
            let response = execute_codex_turn(
                service,
                sandbox,
                &prepared.developer_instructions,
                &prompt,
                &prepared.intermediate_output_schema,
                &cancellation,
            )
            .await?;
            let output = provider.validate_output(&prepared.context, response.output)?;
            require_bounded_intermediate(&output)?;
            reduced.push(output);
        }
        validated_outputs = reduced;
    }
}

async fn execute_codex_turn(
    service: &CodexService,
    sandbox: &Path,
    developer_instructions: &str,
    input: &str,
    output_schema: &Value,
    cancellation: &CodexCancellationToken,
) -> Result<CodexAnalysisResponse, CodexError> {
    service
        .analyze(CodexAnalysisRequest {
            sandbox_directory: sandbox.to_path_buf(),
            developer_instructions: developer_instructions.to_string(),
            input: input.to_string(),
            output_schema: output_schema.clone(),
            timeout: Duration::from_secs(10 * 60),
            cancellation: cancellation.clone(),
        })
        .await
}

fn require_bounded_intermediate(output: &Value) -> Result<(), AiProviderError> {
    let bytes = serde_json::to_vec(output).map_err(|_| AiProviderError::InvalidModelOutput)?;
    if bytes.len() > MAX_AI_INTERMEDIATE_OUTPUT_BYTES {
        return Err(AiProviderError::InvalidModelOutput);
    }
    Ok(())
}

fn partition_synthesis_outputs(
    provider: &dyn ToolAiProvider,
    language: &str,
    outputs: &[Value],
) -> Result<Vec<Vec<Value>>, AiProviderError> {
    let mut groups = Vec::new();
    let mut current = Vec::new();
    for output in outputs {
        let mut candidate = current.clone();
        candidate.push(output.clone());
        if provider.synthesis_prompt(language, &candidate)?.len() <= MAX_AI_PROMPT_BYTES {
            current = candidate;
            continue;
        }
        if current.is_empty() {
            return Err(AiProviderError::ContextTooLarge);
        }
        groups.push(std::mem::take(&mut current));
        current.push(output.clone());
        if provider.synthesis_prompt(language, &current)?.len() > MAX_AI_PROMPT_BYTES {
            return Err(AiProviderError::ContextTooLarge);
        }
    }
    if !current.is_empty() {
        groups.push(current);
    }
    Ok(groups)
}

fn prepare_grounded_review(
    database: &Database,
    tools: &ToolRegistry,
    run_id_raw: &str,
    language: &str,
) -> Result<PreparedGroundedReview, DesktopError> {
    if !matches!(language, "en" | "ar") {
        return Err(DesktopError::InvalidInput(
            "AI review language must be en or ar".to_string(),
        ));
    }
    let run_id = AnalysisRunId::parse(run_id_raw)
        .map_err(|error| DesktopError::InvalidInput(error.to_string()))?;
    let runs = SqliteAnalysisRunRepository::new(database);
    let run = runs
        .find_by_id(&run_id)?
        .ok_or_else(|| DesktopError::InvalidInput("analysis run was not found".to_string()))?;
    if run.status != RunStatus::Completed {
        return Err(DesktopError::InvalidInput(
            "only completed runs can receive an AI review".to_string(),
        ));
    }
    let output = runs
        .find_output(&run_id)?
        .ok_or_else(|| DesktopError::Coded {
            code: "AI_CONTEXT_UNAVAILABLE",
            message: "this legacy run has no complete stored output".to_string(),
        })?;
    let revisions = SqliteSourceRevisionRepository::new(database);
    let revision = revisions
        .find_by_id(&run.source_revision_id)?
        .ok_or_else(|| DesktopError::Storage("run source revision was not found".to_string()))?;
    if revision.project_id != run.project_id || revision.tool_id != run.tool_id {
        return Err(DesktopError::Storage(
            "persisted run provenance is inconsistent".to_string(),
        ));
    }
    let tool = tools
        .get(&run.tool_id)
        .ok_or_else(|| DesktopError::InvalidInput("run tool is not installed".to_string()))?;
    let provider = tool.ai_provider().ok_or_else(|| DesktopError::Coded {
        code: "AI_NOT_SUPPORTED",
        message: "the run tool has no AI provider".to_string(),
    })?;
    let context = provider
        .prepare_context(&output)
        .map_err(|error| DesktopError::Coded {
            code: "AI_CONTEXT_UNAVAILABLE",
            message: error.to_string(),
        })?;
    let developer_instructions =
        provider
            .developer_instructions(language)
            .map_err(|error| DesktopError::Coded {
                code: "AI_CONTEXT_UNAVAILABLE",
                message: error.to_string(),
            })?;
    let source_chunks = provider
        .prompt_chunks(language, &context, MAX_AI_PROMPT_BYTES)
        .map_err(|error| DesktopError::Coded {
            code: match &error {
                AiProviderError::ContextTooLarge => "AI_SCOPE_TOO_LARGE",
                _ => "AI_CONTEXT_UNAVAILABLE",
            },
            message: error.to_string(),
        })?;
    if source_chunks.is_empty()
        || source_chunks
            .iter()
            .any(|chunk| chunk.input.len() > MAX_AI_PROMPT_BYTES)
    {
        return Err(DesktopError::Coded {
            code: "AI_SCOPE_TOO_LARGE",
            message: "the normalized scope could not be partitioned safely".to_string(),
        });
    }
    let final_output_schema = provider.capability().output_schema;
    let intermediate_output_schema = provider.intermediate_output_schema();
    let transmitted_bytes = source_chunks.iter().try_fold(0usize, |total, chunk| {
        total
            .checked_add(developer_instructions.len())
            .and_then(|value| value.checked_add(chunk.input.len()))
            .ok_or_else(|| DesktopError::Coded {
                code: "AI_SCOPE_TOO_LARGE",
                message: "AI scope byte count overflowed".to_string(),
            })
    })?;
    let source_prompts = source_chunks
        .iter()
        .map(|chunk| chunk.input.as_str())
        .collect();
    let envelope = AiScopeEnvelope {
        contract_version: TOOL_CONTRACT_VERSION,
        run_id: &run.id,
        source_sha256: &revision.sha256,
        tool_id: &run.tool_id,
        tool_version: &run.tool_version,
        rule_set_version: &run.rule_set_version,
        language,
        model: ANALYSIS_MODEL,
        developer_instructions: &developer_instructions,
        source_prompts,
        intermediate_output_schema: &intermediate_output_schema,
        final_output_schema: &final_output_schema,
    };
    let envelope_bytes =
        serde_json::to_vec(&envelope).map_err(|error| DesktopError::Tool(error.to_string()))?;
    let input_scope_hash = hash_bytes(&envelope_bytes);
    let transmitted_bytes = u32::try_from(transmitted_bytes).map_err(|_| DesktopError::Coded {
        code: "AI_SCOPE_TOO_LARGE",
        message: "AI scope byte count is not representable".to_string(),
    })?;
    let source_chunk_count =
        u32::try_from(source_chunks.len()).map_err(|_| DesktopError::Coded {
            code: "AI_SCOPE_TOO_LARGE",
            message: "AI source chunk count is not representable".to_string(),
        })?;
    let planned_turn_count = if source_chunk_count > 1 {
        source_chunk_count
            .checked_add(1)
            .ok_or_else(|| DesktopError::Coded {
                code: "AI_SCOPE_TOO_LARGE",
                message: "AI planned turn count overflowed".to_string(),
            })?
    } else {
        1
    };
    Ok(PreparedGroundedReview {
        scope: AiReviewScope {
            run_id,
            source_sha256: revision.sha256,
            source_row_count: context.source_row_count,
            finding_count: context.finding_count,
            source_chunk_count,
            planned_turn_count,
            transmitted_bytes,
            input_scope_hash,
        },
        tool_id: run.tool_id,
        context,
        developer_instructions,
        source_chunks,
        intermediate_output_schema,
        final_output_schema,
    })
}

fn require_ai_enabled(state: &AppState) -> Result<(), DesktopError> {
    if state.settings()?.privacy.ai_features_enabled {
        Ok(())
    } else {
        Err(DesktopError::Coded {
            code: "AI_DISABLED",
            message: "AI features are disabled in privacy settings".to_string(),
        })
    }
}

async fn codex_service(state: &AppState) -> Result<CodexService, DesktopError> {
    require_ai_enabled(state)?;
    state.codex.lock().await.service().await.map_err(Into::into)
}

async fn invalidate_codex_on_failure(state: &AppState, error: &CodexError) {
    if matches!(
        error,
        CodexError::Io(_)
            | CodexError::Protocol
            | CodexError::ProcessExited
            | CodexError::UnsafeActivity
    ) {
        state.codex.lock().await.invalidate().await;
    }
}

fn map_account_snapshot(response: GetAccountResponse) -> Result<AiAccountSnapshot, DesktopError> {
    let codex_version = pinned_release()?.version;
    match response.account {
        None => Ok(AiAccountSnapshot {
            signed_in: false,
            masked_email: None,
            plan_type: None,
            requires_openai_auth: response.requires_openai_auth,
            codex_version,
        }),
        Some(Account::Chatgpt { email, plan_type }) => Ok(AiAccountSnapshot {
            signed_in: true,
            masked_email: email.as_deref().and_then(mask_email),
            plan_type: Some(map_plan_type(plan_type)),
            requires_openai_auth: response.requires_openai_auth,
            codex_version,
        }),
        Some(Account::ApiKey | Account::AmazonBedrock { .. }) => Err(DesktopError::Coded {
            code: "AI_AUTHENTICATION_UNSUPPORTED",
            message: "OpenConKit accepts only Codex-managed ChatGPT login".to_string(),
        }),
    }
}

fn map_rate_limits(
    response: GetAccountRateLimitsResponse,
) -> Result<AiRateLimitSnapshot, DesktopError> {
    Ok(AiRateLimitSnapshot {
        primary: response
            .rate_limits
            .primary
            .map(map_rate_limit_window)
            .transpose()?,
        secondary: response
            .rate_limits
            .secondary
            .map(map_rate_limit_window)
            .transpose()?,
        plan_type: response.rate_limits.plan_type.map(map_plan_type),
        rate_limit_reached: response.rate_limits.rate_limit_reached_type.is_some(),
        spend_control_reached: response.rate_limits.spend_control_reached.unwrap_or(false),
    })
}

fn map_rate_limit_window(window: RateLimitWindow) -> Result<AiRateLimitWindow, DesktopError> {
    let used_percent = u8::try_from(window.used_percent)
        .ok()
        .filter(|percent| *percent <= 100)
        .ok_or_else(|| DesktopError::Coded {
            code: "AI_PROTOCOL_INCOMPATIBLE",
            message: "Codex returned an invalid rate-limit percentage".to_string(),
        })?;
    let window_duration_minutes = window
        .window_duration_mins
        .map(u32::try_from)
        .transpose()
        .map_err(|_| DesktopError::Coded {
            code: "AI_PROTOCOL_INCOMPATIBLE",
            message: "Codex returned an invalid rate-limit duration".to_string(),
        })?;
    Ok(AiRateLimitWindow {
        used_percent,
        window_duration_minutes,
        resets_at: window
            .resets_at
            .map(u32::try_from)
            .transpose()
            .map_err(|_| DesktopError::Coded {
                code: "AI_PROTOCOL_INCOMPATIBLE",
                message: "Codex returned an invalid rate-limit reset time".to_string(),
            })?,
    })
}

fn map_plan_type(plan: PlanType) -> AiPlanType {
    match plan {
        PlanType::Free => AiPlanType::Free,
        PlanType::Go => AiPlanType::Go,
        PlanType::Plus => AiPlanType::Plus,
        PlanType::Pro => AiPlanType::Pro,
        PlanType::Prolite => AiPlanType::Prolite,
        PlanType::Team => AiPlanType::Team,
        PlanType::SelfServeBusinessUsageBased => AiPlanType::SelfServeBusinessUsageBased,
        PlanType::Business => AiPlanType::Business,
        PlanType::EnterpriseCbpUsageBased => AiPlanType::EnterpriseCbpUsageBased,
        PlanType::Enterprise => AiPlanType::Enterprise,
        PlanType::Edu => AiPlanType::Edu,
        PlanType::Unknown => AiPlanType::Unknown,
    }
}

fn mask_email(email: &str) -> Option<String> {
    if email.len() > 254 || email.chars().any(char::is_whitespace) {
        return None;
    }
    let (local, domain) = email.split_once('@')?;
    if local.is_empty() || domain.is_empty() || domain.contains('@') {
        return None;
    }
    let local_first = local.chars().next()?;
    let (domain_stem, suffix) = match domain.rsplit_once('.') {
        Some((stem, suffix)) if !stem.is_empty() && !suffix.is_empty() => {
            (stem, format!(".{suffix}"))
        }
        _ => (domain, String::new()),
    };
    let domain_first = domain_stem.chars().next()?;
    Some(format!("{local_first}***@{domain_first}***{suffix}"))
}

fn hash_bytes(bytes: &[u8]) -> Sha256Hash {
    Sha256Hash::from_bytes(Sha256::digest(bytes).into())
}

fn cleanup_ai_sandbox(home: &Path, directory: &Path) {
    let root = fs::canonicalize(home.join(HomeLayout::AI_SANDBOX_DIR));
    let directory = fs::canonicalize(directory);
    if let (Ok(root), Ok(directory)) = (root, directory) {
        let safe_depth = directory
            .strip_prefix(&root)
            .is_ok_and(|relative| relative.components().count() == 2);
        if safe_depth {
            let parent = directory.parent().map(Path::to_path_buf);
            let _ = fs::remove_dir_all(&directory);
            if let Some(parent) = parent {
                let _ = fs::remove_dir(parent);
            }
        }
    }
}

struct ActiveAiRunGuard {
    active_runs: Arc<Mutex<HashMap<AnalysisRunId, CodexCancellationToken>>>,
    run_id: AnalysisRunId,
}

impl Drop for ActiveAiRunGuard {
    fn drop(&mut self) {
        if let Ok(mut active) = self.active_runs.lock() {
            active.remove(&self.run_id);
        }
    }
}

/// List projects. Pass `include_archived` to include archived ones.
#[tauri::command(rename_all = "snake_case")]
pub fn list_projects(
    state: State<'_, AppState>,
    include_archived: bool,
) -> Result<Vec<Project>, DesktopError> {
    let repo = SqliteProjectRepository::new(&state.database);
    Ok(ListProjects::new(&repo).execute(include_archived)?)
}

/// Register a new project with the given kebab-case `id` and display `name`.
#[tauri::command]
pub fn register_project(
    state: State<'_, AppState>,
    id: String,
    name: String,
) -> Result<Project, DesktopError> {
    let repo = SqliteProjectRepository::new(&state.database);
    Ok(RegisterProject::new(&repo).execute(&id, &name)?)
}

/// Archive a project by id.
#[tauri::command]
pub fn archive_project(state: State<'_, AppState>, id: String) -> Result<(), DesktopError> {
    let repo = SqliteProjectRepository::new(&state.database);
    ArchiveProject::new(&repo).execute(&id)?;
    Ok(())
}

/// Import a source workbook into an existing project using a registered
/// tool's declared extension and size policy.
#[tauri::command(rename_all = "snake_case")]
pub async fn import_source(
    state: State<'_, AppState>,
    project_id: String,
    tool_id: String,
    source_path: String,
) -> Result<SourceRevision, DesktopError> {
    let home = state.home.clone();
    let database = Arc::clone(&state.database);
    let tools = Arc::clone(&state.tools);
    tauri::async_runtime::spawn_blocking(move || {
        let tool = tools
            .get(&tool_id)
            .ok_or_else(|| DesktopError::InvalidInput("unknown tool id".to_string()))?;
        let capabilities = tool.input_capabilities();
        let policy = SourceImportPolicy {
            accepted_extensions: capabilities.accepted_extensions,
            max_file_size_bytes: capabilities.max_file_size_bytes,
        };
        let projects = SqliteProjectRepository::new(&database);
        let revisions = SqliteSourceRevisionRepository::new(&database);
        let storage = FsSourceStorage::new(home);
        ImportSource::new(&projects, &storage, &revisions)
            .execute(
                &project_id,
                &tool_id,
                Path::new(&source_path),
                &policy,
                Some(source_path.clone()),
            )
            .map_err(|err| DesktopError::Storage(err.to_string()))
    })
    .await
    .map_err(|err| DesktopError::BackgroundTask(err.to_string()))?
}

/// Import a workbook into the built-in Quick Analyses project.
#[tauri::command(rename_all = "snake_case")]
pub async fn quick_import_source(
    state: State<'_, AppState>,
    tool_id: String,
    source_path: String,
) -> Result<SourceRevision, DesktopError> {
    let home = state.home.clone();
    let database = Arc::clone(&state.database);
    let tools = Arc::clone(&state.tools);
    tauri::async_runtime::spawn_blocking(move || {
        let tool = tools
            .get(&tool_id)
            .ok_or_else(|| DesktopError::InvalidInput("unknown tool id".to_string()))?;
        let capabilities = tool.input_capabilities();
        let policy = SourceImportPolicy {
            accepted_extensions: capabilities.accepted_extensions,
            max_file_size_bytes: capabilities.max_file_size_bytes,
        };
        let projects = SqliteProjectRepository::new(&database);
        let revisions = SqliteSourceRevisionRepository::new(&database);
        let storage = FsSourceStorage::new(home);
        let (_, revision) = QuickImport::new(&projects, &storage, &revisions)
            .execute(&tool_id, Path::new(&source_path), &policy)
            .map_err(|err| DesktopError::Storage(err.to_string()))?;
        Ok(revision)
    })
    .await
    .map_err(|err| DesktopError::BackgroundTask(err.to_string()))?
}

/// List imported revisions belonging to a project.
#[tauri::command(rename_all = "snake_case")]
pub fn list_source_revisions(
    state: State<'_, AppState>,
    project_id: String,
) -> Result<Vec<SourceRevision>, DesktopError> {
    let repository = SqliteSourceRevisionRepository::new(&state.database);
    ListSourceRevisions::new(&repository)
        .execute(&project_id)
        .map_err(DesktopError::from)
}

#[derive(Deserialize)]
struct PersistableToolOutput {
    findings: Vec<Finding>,
    diagnostics: WorkbookDiagnostics,
}

/// Run a registered tool against an immutable managed source on a blocking
/// worker. The caller provides a UUID so it can issue cancellation while the
/// invoke promise is pending.
#[tauri::command(rename_all = "snake_case")]
#[allow(clippy::too_many_arguments)]
pub async fn run_tool(
    app: AppHandle,
    state: State<'_, AppState>,
    run_id: String,
    project_id: String,
    source_revision_id: String,
    tool_id: String,
    input: Value,
    settings: Value,
) -> Result<ToolRunResponse, DesktopError> {
    let home = state.home.clone();
    let database = Arc::clone(&state.database);
    let tools = Arc::clone(&state.tools);
    let active_runs = Arc::clone(&state.active_runs);
    let app_settings = state.settings()?.clone();
    tauri::async_runtime::spawn_blocking(move || {
        execute_tool_run(
            &app,
            &home,
            &database,
            &tools,
            &active_runs,
            &app_settings,
            &run_id,
            &project_id,
            &source_revision_id,
            &tool_id,
            input,
            settings,
        )
    })
    .await
    .map_err(|err| DesktopError::BackgroundTask(err.to_string()))?
}

/// Request cooperative cancellation of an active run.
#[tauri::command(rename_all = "snake_case")]
pub fn cancel_tool_run(state: State<'_, AppState>, run_id: String) -> Result<bool, DesktopError> {
    let run_id =
        AnalysisRunId::parse(&run_id).map_err(|err| DesktopError::InvalidInput(err.to_string()))?;
    let runs = state.active_runs()?;
    if let Some(token) = runs.get(&run_id) {
        token.cancel();
        Ok(true)
    } else {
        Ok(false)
    }
}

/// List project run history.
#[tauri::command(rename_all = "snake_case")]
pub fn list_analysis_runs(
    state: State<'_, AppState>,
    project_id: String,
) -> Result<Vec<AnalysisRun>, DesktopError> {
    let repository = SqliteAnalysisRunRepository::new(&state.database);
    ListAnalysisRuns::new(&repository)
        .execute(&project_id)
        .map_err(DesktopError::from)
}

/// List enriched project history including source, finding, export and AI status.
#[tauri::command(rename_all = "snake_case")]
pub fn list_run_history(
    state: State<'_, AppState>,
    project_id: String,
) -> Result<Vec<RunHistoryEntry>, DesktopError> {
    let repository = SqliteAnalysisRunRepository::new(&state.database);
    ListRunHistory::new(&repository)
        .execute(&project_id)
        .map_err(DesktopError::from)
}

/// Reopen a run and its authoritative stored findings.
#[tauri::command(rename_all = "snake_case")]
pub fn open_analysis_run(
    state: State<'_, AppState>,
    run_id: String,
) -> Result<RunDetails, DesktopError> {
    let runs = SqliteAnalysisRunRepository::new(&state.database);
    let findings = SqliteFindingRepository::new(&state.database);
    let exports = SqliteExportRepository::new(&state.database);
    let ai_analyses = SqliteAiAnalysisRepository::new(&state.database);
    OpenAnalysisRun::new(&runs, &findings, &exports, &ai_analyses)
        .execute(&run_id)
        .map_err(DesktopError::from)
}

/// Generate a new, immutable report from the exact stored output of a
/// completed run. Report language is independent from the current UI locale.
#[tauri::command(rename_all = "snake_case")]
pub async fn export_analysis_run(
    state: State<'_, AppState>,
    run_id: String,
    kind: ExportKind,
    language: String,
) -> Result<ExportRecord, DesktopError> {
    let home = state.home.clone();
    let database = Arc::clone(&state.database);
    let tools = Arc::clone(&state.tools);
    tauri::async_runtime::spawn_blocking(move || {
        generate_analysis_export(&home, &database, &tools, &run_id, kind, &language)
    })
    .await
    .map_err(|error| DesktopError::BackgroundTask(error.to_string()))?
}

/// List all report artifacts previously generated from a run.
#[tauri::command(rename_all = "snake_case")]
pub fn list_run_exports(
    state: State<'_, AppState>,
    run_id: String,
) -> Result<Vec<ExportRecord>, DesktopError> {
    let run_id = AnalysisRunId::parse(&run_id)
        .map_err(|error| DesktopError::InvalidInput(error.to_string()))?;
    let runs = SqliteAnalysisRunRepository::new(&state.database);
    if runs.find_by_id(&run_id)?.is_none() {
        return Err(DesktopError::InvalidInput(
            "analysis run was not found".to_string(),
        ));
    }
    let exports = SqliteExportRepository::new(&state.database);
    exports.list_by_run(&run_id).map_err(DesktopError::from)
}

/// Show a previously generated report in the operating system's file manager.
///
/// The frontend supplies only persisted identifiers. Rust resolves the
/// recorded managed path, rejects links/escapes, and rechecks the content
/// hash before invoking the file manager.
#[tauri::command(rename_all = "snake_case")]
pub fn reveal_export(
    state: State<'_, AppState>,
    run_id: String,
    export_id: String,
) -> Result<(), DesktopError> {
    let run_id = AnalysisRunId::parse(&run_id)
        .map_err(|error| DesktopError::InvalidInput(error.to_string()))?;
    let export_id = ExportId::parse(&export_id)
        .map_err(|error| DesktopError::InvalidInput(error.to_string()))?;
    let runs = SqliteAnalysisRunRepository::new(&state.database);
    let run = runs
        .find_by_id(&run_id)?
        .ok_or_else(|| DesktopError::InvalidInput("analysis run was not found".to_string()))?;
    let exports = SqliteExportRepository::new(&state.database);
    let record = exports
        .list_by_run(&run_id)?
        .into_iter()
        .find(|candidate| candidate.id == export_id)
        .ok_or_else(|| DesktopError::InvalidInput("export was not found".to_string()))?;
    let path = resolve_recorded_export(&state.home, &run.project_id, &record)?;
    reveal_path(&path)
}

fn generate_analysis_export(
    home: &Path,
    database: &Database,
    tools: &ToolRegistry,
    run_id_raw: &str,
    kind: ExportKind,
    language: &str,
) -> Result<ExportRecord, DesktopError> {
    let run_id = AnalysisRunId::parse(run_id_raw)
        .map_err(|error| DesktopError::InvalidInput(error.to_string()))?;
    let runs = SqliteAnalysisRunRepository::new(database);
    let run = runs
        .find_by_id(&run_id)?
        .ok_or_else(|| DesktopError::InvalidInput("analysis run was not found".to_string()))?;
    if run.status != RunStatus::Completed {
        return Err(DesktopError::InvalidInput(
            "only completed analysis runs can be exported".to_string(),
        ));
    }
    let output = runs.find_output(&run_id)?.ok_or_else(|| {
        DesktopError::InvalidInput(
            "this legacy run has no complete stored output and cannot be reproduced".to_string(),
        )
    })?;
    let revisions = SqliteSourceRevisionRepository::new(database);
    let revision = revisions
        .find_by_id(&run.source_revision_id)?
        .ok_or_else(|| DesktopError::Storage("run source revision was not found".to_string()))?;
    if revision.project_id != run.project_id || revision.tool_id != run.tool_id {
        return Err(DesktopError::Storage(
            "persisted run provenance is inconsistent".to_string(),
        ));
    }

    let tool = tools
        .get(&run.tool_id)
        .ok_or_else(|| DesktopError::InvalidInput("run tool is not installed".to_string()))?;
    let providers = tool.export_providers();
    let mut matching = providers
        .into_iter()
        .filter(|provider| provider.kind() == kind);
    let provider = matching.next().ok_or_else(|| {
        DesktopError::InvalidInput("the run tool does not support this export format".to_string())
    })?;
    if matching.next().is_some() {
        return Err(DesktopError::Registry(
            "tool registered more than one provider for an export format".to_string(),
        ));
    }
    if !provider
        .languages()
        .iter()
        .any(|supported| supported == language)
    {
        return Err(DesktopError::InvalidInput(
            "the export language is not supported".to_string(),
        ));
    }

    let export_id = ExportId::new();
    let relative_dir = format!(
        "{}/{}/{}",
        HomeLayout::project_exports_dir(&run.project_id),
        run.id,
        export_id
    );
    let destination = create_confined_directory(home, &relative_dir)?;
    let report_timestamp = Timestamp::now();
    let requested_ai_language = if language == "ar" {
        AiAnalysisLanguage::Ar
    } else {
        AiAnalysisLanguage::En
    };
    let validated_ai_output = tool.ai_provider().and_then(|provider| {
        let context = provider.prepare_context(&output).ok()?;
        SqliteAiAnalysisRepository::new(database)
            .list_by_run(&run.id)
            .ok()?
            .into_iter()
            .rev()
            .find(|analysis| {
                analysis.language == requested_ai_language
                    && analysis.status == AiAnalysisStatus::Completed
                    && analysis.grounding_status == AiGroundingStatus::Validated
                    && analysis.validation_status != AiValidationStatus::Rejected
            })
            .and_then(|analysis| analysis.output)
            .and_then(|candidate| provider.validate_output(&context, candidate).ok())
    });
    let context = ExportContext {
        run: run.clone(),
        source_revision: revision,
        report_timestamp,
        validated_ai_output,
    };
    let artifact = match provider.export(&context, &output, &destination, language) {
        Ok(artifact) => artifact,
        Err(error) => {
            cleanup_export_destination(home, &destination);
            return Err(error.into());
        }
    };

    let result = (|| {
        if artifact.kind != kind || artifact.language != language {
            return Err(DesktopError::Tool(
                "export provider returned mismatched artifact metadata".to_string(),
            ));
        }
        let artifact_relative = safe_relative_path(&artifact.relative_path)?;
        let artifact_path = verify_export_artifact(&destination, &artifact_relative)?;
        let provider_hash = Sha256Hash::from_hex(&artifact.sha256)
            .map_err(|error| DesktopError::Tool(error.to_string()))?;
        let actual_hash = hash_file(&artifact_path)?;
        if provider_hash != actual_hash {
            return Err(DesktopError::Tool(
                "export provider returned an incorrect content hash".to_string(),
            ));
        }
        let record_relative_path = format!(
            "{}/{}/{}",
            run.id,
            export_id,
            path_to_forward_slashes(&artifact_relative)?
        );
        let record = ExportRecord::new(
            export_id,
            run.id,
            kind,
            language.to_string(),
            record_relative_path,
            actual_hash,
            report_timestamp,
        )
        .map_err(|error| DesktopError::Domain(error.to_string()))?;
        let exports = SqliteExportRepository::new(database);
        exports.save(&record)?;
        Ok(record)
    })();
    if result.is_err() {
        cleanup_export_destination(home, &destination);
    }
    result
}

fn safe_relative_path(raw: &str) -> Result<PathBuf, DesktopError> {
    if raw.is_empty()
        || raw.starts_with('/')
        || raw.contains('\\')
        || raw.contains(':')
        || raw
            .split('/')
            .any(|segment| segment.is_empty() || segment == "." || segment == "..")
    {
        return Err(DesktopError::Tool(
            "export provider returned an unsafe artifact path".to_string(),
        ));
    }
    let path: PathBuf = raw.split('/').collect();
    if path
        .components()
        .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(DesktopError::Tool(
            "export provider returned an unsafe artifact path".to_string(),
        ));
    }
    Ok(path)
}

fn create_confined_directory(home: &Path, relative: &str) -> Result<PathBuf, DesktopError> {
    let canonical_home =
        fs::canonicalize(home).map_err(|error| DesktopError::Storage(error.to_string()))?;
    let relative_path = safe_relative_path(relative)?;
    let mut current = canonical_home.clone();
    for component in relative_path.components() {
        let Component::Normal(segment) = component else {
            return Err(DesktopError::Storage(
                "managed export path contains an unsafe component".to_string(),
            ));
        };
        current.push(segment);
        match fs::symlink_metadata(&current) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() || !metadata.is_dir() {
                    return Err(DesktopError::Storage(
                        "managed export path traverses a non-directory or link".to_string(),
                    ));
                }
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                fs::create_dir(&current)
                    .map_err(|error| DesktopError::Storage(error.to_string()))?;
            }
            Err(error) => return Err(DesktopError::Storage(error.to_string())),
        }
        let canonical =
            fs::canonicalize(&current).map_err(|error| DesktopError::Storage(error.to_string()))?;
        if !canonical.starts_with(&canonical_home) {
            return Err(DesktopError::Storage(
                "managed export path escaped the app home".to_string(),
            ));
        }
        current = canonical;
    }
    Ok(current)
}

fn verify_export_artifact(destination: &Path, relative: &Path) -> Result<PathBuf, DesktopError> {
    let candidate = destination.join(relative);
    let link_metadata =
        fs::symlink_metadata(&candidate).map_err(|error| DesktopError::Tool(error.to_string()))?;
    if link_metadata.file_type().is_symlink() || !link_metadata.is_file() {
        return Err(DesktopError::Tool(
            "export provider did not create a regular file".to_string(),
        ));
    }
    let canonical_destination =
        fs::canonicalize(destination).map_err(|error| DesktopError::Storage(error.to_string()))?;
    let canonical_artifact =
        fs::canonicalize(candidate).map_err(|error| DesktopError::Tool(error.to_string()))?;
    if !canonical_artifact.starts_with(&canonical_destination) {
        return Err(DesktopError::Tool(
            "export artifact escaped its managed destination".to_string(),
        ));
    }
    Ok(canonical_artifact)
}

fn resolve_recorded_export(
    home: &Path,
    project_id: &ProjectId,
    record: &ExportRecord,
) -> Result<PathBuf, DesktopError> {
    let canonical_home =
        fs::canonicalize(home).map_err(|error| DesktopError::Storage(error.to_string()))?;
    let exports_relative = safe_relative_path(&HomeLayout::project_exports_dir(project_id))?;
    let record_relative = safe_relative_path(&record.relative_path)?;
    let exports_dir = fs::canonicalize(canonical_home.join(exports_relative))
        .map_err(|error| DesktopError::Storage(error.to_string()))?;
    if !exports_dir.starts_with(&canonical_home) {
        return Err(DesktopError::Storage(
            "managed exports directory escaped the app home".to_string(),
        ));
    }
    let candidate = exports_dir.join(record_relative);
    let metadata = fs::symlink_metadata(&candidate)
        .map_err(|error| DesktopError::Storage(error.to_string()))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(DesktopError::Storage(
            "recorded export is not a regular file".to_string(),
        ));
    }
    let canonical =
        fs::canonicalize(candidate).map_err(|error| DesktopError::Storage(error.to_string()))?;
    if !canonical.starts_with(&exports_dir) {
        return Err(DesktopError::Storage(
            "recorded export escaped its managed directory".to_string(),
        ));
    }
    if hash_file(&canonical)? != record.sha256 {
        return Err(DesktopError::Storage(
            "recorded export content no longer matches its integrity hash".to_string(),
        ));
    }
    Ok(canonical)
}

fn reveal_path(path: &Path) -> Result<(), DesktopError> {
    #[cfg(target_os = "windows")]
    let mut command = {
        let mut command = Command::new("explorer.exe");
        command.arg(format!("/select,{}", path.display()));
        command
    };

    #[cfg(target_os = "macos")]
    let mut command = {
        let mut command = Command::new("open");
        command.arg("-R").arg(path);
        command
    };

    #[cfg(all(unix, not(target_os = "macos")))]
    let mut command = {
        let mut command = Command::new("xdg-open");
        command.arg(path.parent().ok_or_else(|| {
            DesktopError::Storage("recorded export has no parent directory".to_string())
        })?);
        command
    };

    command
        .spawn()
        .map_err(|error| DesktopError::Storage(error.to_string()))?;
    Ok(())
}

fn hash_file(path: &Path) -> Result<Sha256Hash, DesktopError> {
    let mut file =
        fs::File::open(path).map_err(|error| DesktopError::Storage(error.to_string()))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|error| DesktopError::Storage(error.to_string()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(Sha256Hash::from_bytes(hasher.finalize().into()))
}

fn path_to_forward_slashes(path: &Path) -> Result<String, DesktopError> {
    let mut segments = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(segment) => segments.push(
                segment
                    .to_str()
                    .ok_or_else(|| {
                        DesktopError::Tool("export artifact path is not valid Unicode".to_string())
                    })?
                    .to_string(),
            ),
            _ => {
                return Err(DesktopError::Tool(
                    "export artifact path is unsafe".to_string(),
                ));
            }
        }
    }
    Ok(segments.join("/"))
}

fn cleanup_export_destination(home: &Path, destination: &Path) {
    let home = fs::canonicalize(home);
    let destination = fs::canonicalize(destination);
    if let (Ok(home), Ok(destination)) = (home, destination) {
        if destination.starts_with(home) {
            let _ = fs::remove_dir_all(destination);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn execute_tool_run(
    app: &AppHandle,
    home: &Path,
    database: &Database,
    tools: &ToolRegistry,
    active_runs: &Arc<Mutex<HashMap<AnalysisRunId, CancellationToken>>>,
    app_settings: &AppSettings,
    run_id_raw: &str,
    project_id_raw: &str,
    source_revision_id_raw: &str,
    tool_id: &str,
    input: Value,
    settings: Value,
) -> Result<ToolRunResponse, DesktopError> {
    let run_id = AnalysisRunId::parse(run_id_raw)
        .map_err(|err| DesktopError::InvalidInput(err.to_string()))?;
    let project_id = ProjectId::new(project_id_raw.to_string())
        .map_err(|err| DesktopError::InvalidInput(err.to_string()))?;
    let source_revision_id = SourceRevisionId::parse(source_revision_id_raw)
        .map_err(|err| DesktopError::InvalidInput(err.to_string()))?;
    let tool = tools
        .get(tool_id)
        .ok_or_else(|| DesktopError::InvalidInput("unknown tool id".to_string()))?;
    let manifest = tool.manifest();

    let revisions = SqliteSourceRevisionRepository::new(database);
    let revision = revisions
        .find_by_id(&source_revision_id)?
        .ok_or_else(|| DesktopError::InvalidInput("source revision was not found".to_string()))?;
    if revision.project_id != project_id {
        return Err(DesktopError::InvalidInput(
            "source revision does not belong to the project".to_string(),
        ));
    }
    if revision.tool_id != tool_id {
        return Err(DesktopError::InvalidInput(
            "source revision is not compatible with this tool".to_string(),
        ));
    }
    let workbook_path = resolve_managed_source(home, &revision)?;
    let input = authoritative_tool_input(input, source_revision_id)?;
    let settings = authoritative_tool_settings(settings, app_settings)?;
    let repository = SqliteAnalysisRunRepository::new(database);
    if repository.find_by_id(&run_id)?.is_some() {
        return Err(DesktopError::InvalidInput(
            "analysis run id already exists".to_string(),
        ));
    }

    let cancel = CancellationToken::new();
    {
        let mut active = active_runs
            .lock()
            .map_err(|_| DesktopError::StatePoisoned)?;
        if active.contains_key(&run_id) {
            return Err(DesktopError::InvalidInput(
                "analysis run is already active".to_string(),
            ));
        }
        active.insert(run_id, cancel.clone());
    }
    let _active_guard = ActiveRunGuard {
        active_runs: Arc::clone(active_runs),
        run_id,
    };

    let started_at = Timestamp::now();
    let running = AnalysisRun {
        id: run_id,
        project_id: project_id.clone(),
        source_revision_id,
        tool_id: tool_id.to_string(),
        tool_version: manifest.tool_version,
        rule_set_version: tool.rule_set_version().to_string(),
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        status: RunStatus::Running,
        started_at,
        finished_at: None,
        structure_diagnostics: None,
        overall_confidence: None,
    };
    repository.save(&running)?;

    let context = ToolRunContext {
        run_id: run_id.to_string(),
        project_id: project_id.to_string(),
        source_revision_id: source_revision_id.to_string(),
        workbook_path,
        app_version: env!("CARGO_PKG_VERSION").to_string(),
    };
    let progress = |progress: ToolProgress| {
        let _ = app.emit(
            "tool-progress",
            ToolProgressEvent {
                run_id: run_id.to_string(),
                progress,
            },
        );
    };
    let output = match tool
        .engine()
        .run(&context, &input, &settings, &progress, &cancel)
    {
        Ok(output) => output,
        Err(error) => {
            let status = if matches!(error, openconkit_tool_sdk::ToolError::Cancelled) {
                RunStatus::Cancelled
            } else {
                RunStatus::Failed
            };
            persist_terminal_run(&repository, running, status, None)?;
            return Err(error.into());
        }
    };
    let persistable: PersistableToolOutput = match serde_json::from_value(output.clone()) {
        Ok(output) => output,
        Err(err) => {
            persist_terminal_run(&repository, running, RunStatus::Failed, None)?;
            return Err(DesktopError::Tool(format!(
                "tool output failed contract validation: {err}"
            )));
        }
    };
    if persistable.diagnostics.rule_set_version != running.rule_set_version {
        persist_terminal_run(&repository, running, RunStatus::Failed, None)?;
        return Err(DesktopError::Tool(
            "tool output rule-set version does not match its declaration".to_string(),
        ));
    }
    let completed = AnalysisRun {
        status: RunStatus::Completed,
        finished_at: Some(Timestamp::now()),
        overall_confidence: Some(persistable.diagnostics.interpretation_confidence),
        structure_diagnostics: Some(persistable.diagnostics),
        ..running
    };
    repository.save_with_findings_and_output(&completed, &persistable.findings, &output)?;
    Ok(ToolRunResponse {
        run: completed,
        output,
    })
}

fn persist_terminal_run(
    repository: &SqliteAnalysisRunRepository<'_>,
    run: AnalysisRun,
    status: RunStatus,
    diagnostics: Option<WorkbookDiagnostics>,
) -> Result<(), DesktopError> {
    let terminal = AnalysisRun {
        status,
        finished_at: Some(Timestamp::now()),
        overall_confidence: diagnostics
            .as_ref()
            .map(|diagnostics| diagnostics.interpretation_confidence),
        structure_diagnostics: diagnostics,
        ..run
    };
    repository.save(&terminal)?;
    Ok(())
}

fn authoritative_tool_input(
    mut input: Value,
    source_revision_id: SourceRevisionId,
) -> Result<Value, DesktopError> {
    let object = input
        .as_object_mut()
        .ok_or_else(|| DesktopError::InvalidInput("tool input must be an object".to_string()))?;
    object.insert(
        "source_revision_id".to_string(),
        Value::String(source_revision_id.to_string()),
    );
    Ok(input)
}

fn authoritative_tool_settings(
    mut settings: Value,
    app_settings: &AppSettings,
) -> Result<Value, DesktopError> {
    let object = settings
        .as_object_mut()
        .ok_or_else(|| DesktopError::InvalidInput("tool settings must be an object".to_string()))?;
    object.insert(
        "absolute_tolerance".to_string(),
        Value::String(app_settings.tolerances.absolute_tolerance.to_string()),
    );
    object.insert(
        "relative_tolerance".to_string(),
        Value::String(app_settings.tolerances.relative_tolerance.to_string()),
    );
    object.insert(
        "decimal_precision".to_string(),
        Value::Number(serde_json::Number::from(
            app_settings.tolerances.decimal_precision,
        )),
    );
    Ok(settings)
}

fn resolve_managed_source(home: &Path, revision: &SourceRevision) -> Result<PathBuf, DesktopError> {
    let segments: Vec<&str> = revision.stored_path.split('/').collect();
    if segments.len() != 5
        || segments[0] != "projects"
        || segments[1] != revision.project_id.as_str()
        || segments[2] != "sources"
        || segments.iter().any(|segment| segment.is_empty())
    {
        return Err(DesktopError::Storage(
            "stored source path violates the managed-vault layout".to_string(),
        ));
    }
    let relative = segments.iter().fold(PathBuf::new(), |mut path, segment| {
        path.push(segment);
        path
    });
    if relative
        .components()
        .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(DesktopError::Storage(
            "stored source path contains an unsafe component".to_string(),
        ));
    }
    let candidate = home.join(relative);
    let canonical_home =
        std::fs::canonicalize(home).map_err(|err| DesktopError::Storage(err.to_string()))?;
    let canonical_source =
        std::fs::canonicalize(&candidate).map_err(|err| DesktopError::Storage(err.to_string()))?;
    if !canonical_source.starts_with(&canonical_home) {
        return Err(DesktopError::Storage(
            "stored source path escaped the app home".to_string(),
        ));
    }
    let metadata = std::fs::metadata(&canonical_source)
        .map_err(|err| DesktopError::Storage(err.to_string()))?;
    if !metadata.is_file() {
        return Err(DesktopError::Storage(
            "stored source is not a regular file".to_string(),
        ));
    }
    Ok(canonical_source)
}

struct ActiveRunGuard {
    active_runs: Arc<Mutex<HashMap<AnalysisRunId, CancellationToken>>>,
    run_id: AnalysisRunId,
}

impl Drop for ActiveRunGuard {
    fn drop(&mut self) {
        if let Ok(mut active) = self.active_runs.lock() {
            active.remove(&self.run_id);
        }
    }
}

/// Manifests of every registered tool, in registration order.
#[tauri::command]
pub fn list_tool_manifests(state: State<'_, AppState>) -> Vec<ToolManifest> {
    state.tools.manifests()
}

/// Shell navigation model built from registered tools.
#[tauri::command]
pub fn list_tool_navigation(state: State<'_, AppState>) -> Vec<ToolNavItem> {
    state.tools.navigation()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use openconkit_application::{
        AiAnalysisRepository, FindingRepository, ProjectRepository, SourceRevisionRepository,
    };
    use openconkit_domain::{Confidence, Project, Sha256Hash};
    use openconkit_storage::{
        SqliteAiAnalysisRepository, SqliteFindingRepository, SqliteProjectRepository,
    };
    use openconkit_tool_boq_inspector::{
        BoqInspectorOutput, BoqInspectorSummary, TOOL_ID as BOQ_TOOL_ID,
    };
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_home(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "openconkit-desktop-{label}-{}-{nanos}",
            std::process::id()
        ))
    }

    #[test]
    fn app_version_matches_package_version() {
        assert_eq!(app_version(), env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn stored_run_exports_are_reproducible_and_never_overwritten() {
        let home = temp_home("historical-export");
        fs::create_dir_all(&home).expect("home");
        let database = Database::open_in_memory().expect("database");
        database.migrate().expect("migrations");
        let tools = crate::registry::build_registry().expect("registry");
        let tool = tools.get(BOQ_TOOL_ID).expect("tool");

        let project_id = ProjectId::new("tower-a").expect("project id");
        let project =
            Project::new(project_id.clone(), "Tower A", Timestamp::now()).expect("project");
        SqliteProjectRepository::new(&database)
            .save(&project)
            .expect("save project");
        let revision = SourceRevision::new(
            SourceRevisionId::new(),
            project_id.clone(),
            Sha256Hash::from_bytes([0x21; 32]),
            "priced-boq.xlsx".to_string(),
            None,
            "projects/tower-a/sources/revision/priced-boq.xlsx".to_string(),
            128,
            Timestamp::now(),
            BOQ_TOOL_ID.to_string(),
            None,
        )
        .expect("revision");
        SqliteSourceRevisionRepository::new(&database)
            .save(&revision)
            .expect("save revision");

        let diagnostics = WorkbookDiagnostics {
            rule_set_version: tool.rule_set_version().to_string(),
            interpretation_confidence: Confidence::new(1.0).expect("confidence"),
            ..WorkbookDiagnostics::default()
        };
        let run = AnalysisRun {
            id: AnalysisRunId::new(),
            project_id,
            source_revision_id: revision.id,
            tool_id: BOQ_TOOL_ID.to_string(),
            tool_version: tool.manifest().tool_version,
            rule_set_version: tool.rule_set_version().to_string(),
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            status: RunStatus::Completed,
            started_at: Timestamp::now(),
            finished_at: Some(Timestamp::now()),
            structure_diagnostics: Some(diagnostics.clone()),
            overall_confidence: Some(diagnostics.interpretation_confidence),
        };
        let output = serde_json::to_value(BoqInspectorOutput {
            findings: vec![],
            diagnostics,
            summary: BoqInspectorSummary {
                item_rows: 0,
                finding_count: 0,
                pareto: vec![],
            },
            normalized_rows: vec![],
        })
        .expect("output");
        let runs = SqliteAnalysisRunRepository::new(&database);
        runs.save_with_findings_and_output(&run, &[], &output)
            .expect("save complete aggregate");

        let first = generate_analysis_export(
            &home,
            &database,
            &tools,
            &run.id.to_string(),
            ExportKind::Xlsx,
            "ar",
        )
        .expect("first export");
        let second = generate_analysis_export(
            &home,
            &database,
            &tools,
            &run.id.to_string(),
            ExportKind::Xlsx,
            "ar",
        )
        .expect("second export");

        assert_ne!(first.id, second.id);
        assert_ne!(first.relative_path, second.relative_path);
        for export in [&first, &second] {
            let path = home.join("projects").join("tower-a").join("exports").join(
                export
                    .relative_path
                    .replace('/', std::path::MAIN_SEPARATOR_STR),
            );
            assert!(path.is_file(), "missing {}", path.display());
            assert_eq!(hash_file(&path).expect("hash"), export.sha256);
        }

        let exports = SqliteExportRepository::new(&database)
            .list_by_run(&run.id)
            .expect("list exports");
        assert_eq!(exports.len(), 2);
        let details = OpenAnalysisRun::new(
            &runs,
            &SqliteFindingRepository::new(&database),
            &SqliteExportRepository::new(&database),
            &SqliteAiAnalysisRepository::new(&database),
        )
        .execute(&run.id.to_string())
        .expect("reopen");
        assert_eq!(details.output, Some(output));
        assert_eq!(details.exports.len(), 2);
        assert!(SqliteAiAnalysisRepository::new(&database)
            .list_by_run(&run.id)
            .expect("ai list")
            .is_empty());
        assert!(SqliteFindingRepository::new(&database)
            .list_by_run(&run.id)
            .expect("finding list")
            .is_empty());

        let resolved =
            resolve_recorded_export(&home, &run.project_id, &first).expect("resolve export");
        assert_eq!(
            resolved,
            fs::canonicalize(
                home.join("projects").join("tower-a").join("exports").join(
                    first
                        .relative_path
                        .replace('/', std::path::MAIN_SEPARATOR_STR)
                )
            )
            .expect("canonical export")
        );
        fs::write(&resolved, b"tampered report").expect("tamper");
        assert!(resolve_recorded_export(&home, &run.project_id, &first).is_err());

        fs::remove_dir_all(&home).expect("cleanup");
    }

    #[test]
    fn export_path_validation_rejects_traversal_and_links() {
        for raw in [
            "",
            "../report.pdf",
            "/report.pdf",
            "C:\\report.pdf",
            "a/./b",
        ] {
            assert!(safe_relative_path(raw).is_err(), "{raw:?}");
        }

        let home = temp_home("export-confinement");
        fs::create_dir_all(&home).expect("home");
        let destination = create_confined_directory(&home, "projects/tower-a/exports/run/export")
            .expect("managed directory");
        assert!(destination.starts_with(fs::canonicalize(&home).expect("canonical home")));
        fs::remove_dir_all(&home).expect("cleanup");
    }
}
