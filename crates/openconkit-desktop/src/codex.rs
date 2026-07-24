//! Lifecycle owner for the optional bundled Codex app-server process.

use std::fs;
use std::path::{Path, PathBuf};

use openconkit_ai_codex::{
    pinned_release, sidecar_binary_name, CodexBinaryKind, CodexClient, CodexClientConfig,
    CodexError, CodexService,
};
use openconkit_application::HomeLayout;
use openconkit_domain::AnalysisRunId;

const MAX_PROCESS_LAUNCHES: u8 = 3;

/// Lazy, bounded-restart Codex runtime. No process starts during app launch.
pub struct CodexRuntime {
    binary: PathBuf,
    bundled_binary: PathBuf,
    binary_kind: CodexBinaryKind,
    codex_home: PathBuf,
    sandbox_root: PathBuf,
    protocol_log: Option<PathBuf>,
    service: Option<CodexService>,
    process_directory: Option<PathBuf>,
    launch_count: u8,
}

impl CodexRuntime {
    pub fn new(
        home: &Path,
        resource_directory: Option<&Path>,
        system_binary: Option<&str>,
        diagnostic_logging_enabled: bool,
    ) -> Self {
        let bundled_binary = locate_binary(resource_directory);
        let (binary, binary_kind) = match system_binary {
            Some(path) => (PathBuf::from(path), CodexBinaryKind::CodexCli),
            None => (bundled_binary.clone(), CodexBinaryKind::StandaloneAppServer),
        };
        Self {
            binary,
            bundled_binary,
            binary_kind,
            codex_home: home.join(HomeLayout::CODEX_HOME_DIR),
            sandbox_root: home.join(HomeLayout::AI_SANDBOX_DIR),
            protocol_log: diagnostic_logging_enabled
                .then(|| home.join(HomeLayout::LOGS_DIR).join("codex-protocol.jsonl")),
            service: None,
            process_directory: None,
            launch_count: 0,
        }
    }

    pub fn binary_available(&self) -> bool {
        fs::metadata(&self.binary).is_ok_and(|metadata| metadata.is_file())
    }

    pub fn bundled_binary_available(&self) -> bool {
        fs::metadata(&self.bundled_binary).is_ok_and(|metadata| metadata.is_file())
    }

    pub fn using_system_binary(&self) -> bool {
        self.binary_kind == CodexBinaryKind::CodexCli
    }

    pub fn pinned_version(&self) -> Result<String, CodexError> {
        Ok(pinned_release()?.version)
    }

    /// Return the current initialized service or launch a new one. The first
    /// process plus at most two crash restarts are allowed per app session.
    pub async fn service(&mut self) -> Result<CodexService, CodexError> {
        if let Some(service) = &self.service {
            return Ok(service.clone());
        }
        if self.launch_count >= MAX_PROCESS_LAUNCHES {
            return Err(CodexError::RestartLimit);
        }
        if !self.binary_available() {
            return Err(CodexError::SidecarUnavailable(
                "bundled executable was not staged".to_string(),
            ));
        }

        let process_directory = create_process_directory(&self.sandbox_root)?;
        self.launch_count = self.launch_count.saturating_add(1);
        let config = match self.binary_kind {
            CodexBinaryKind::StandaloneAppServer => CodexClientConfig::standalone(
                self.binary.clone(),
                self.codex_home.clone(),
                process_directory.clone(),
            )?,
            CodexBinaryKind::CodexCli => CodexClientConfig::system_cli(
                self.binary.clone(),
                self.codex_home.clone(),
                process_directory.clone(),
            )?,
        };
        let config = match &self.protocol_log {
            Some(path) => config.with_protocol_log(path.clone())?,
            None => config,
        };
        match CodexClient::spawn(config, env!("CARGO_PKG_VERSION")).await {
            Ok(client) => {
                let service = CodexService::new(client);
                self.process_directory = Some(process_directory);
                self.service = Some(service.clone());
                Ok(service)
            }
            Err(error) => {
                cleanup_process_directory(&self.sandbox_root, &process_directory);
                Err(error)
            }
        }
    }

    /// Discard a failed connection. A subsequent call may restart it within
    /// the fixed session budget.
    pub async fn invalidate(&mut self) {
        if let Some(service) = self.service.take() {
            service.shutdown().await;
        }
        if let Some(directory) = self.process_directory.take() {
            cleanup_process_directory(&self.sandbox_root, &directory);
        }
    }
}

fn locate_binary(resource_directory: Option<&Path>) -> PathBuf {
    let target = env!("OPENCONKIT_TARGET_TRIPLE");
    let staged_name = sidecar_binary_name(target);
    let packaged_name = if cfg!(target_os = "windows") {
        "codex-app-server.exe"
    } else {
        "codex-app-server"
    };
    let mut candidates = Vec::new();
    if cfg!(debug_assertions) {
        candidates.push(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("binaries")
                .join(&staged_name),
        );
    }
    if let Some(resources) = resource_directory {
        candidates.push(resources.join("codex").join(&staged_name));
        candidates.push(resources.join("codex").join(packaged_name));
        candidates.push(resources.join(packaged_name));
    }
    if let Ok(executable) = std::env::current_exe() {
        if let Some(parent) = executable.parent() {
            candidates.push(parent.join(packaged_name));
        }
    }
    candidates
        .iter()
        .find(|candidate| fs::metadata(candidate).is_ok_and(|metadata| metadata.is_file()))
        .cloned()
        .unwrap_or_else(|| {
            candidates.into_iter().next().unwrap_or_else(|| {
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("binaries")
                    .join(staged_name)
            })
        })
}

fn create_process_directory(root: &Path) -> Result<PathBuf, CodexError> {
    let canonical_root = fs::canonicalize(root)?;
    let name = format!("process-{}", AnalysisRunId::new());
    let directory = canonical_root.join(name);
    fs::create_dir(&directory)?;
    let canonical_directory = fs::canonicalize(&directory)?;
    if !canonical_directory.starts_with(&canonical_root) {
        cleanup_process_directory(&canonical_root, &canonical_directory);
        return Err(CodexError::InvalidConfiguration(
            "process directory escaped AI sandbox".to_string(),
        ));
    }
    Ok(canonical_directory)
}

fn cleanup_process_directory(root: &Path, directory: &Path) {
    let canonical_root = fs::canonicalize(root);
    let canonical_directory = fs::canonicalize(directory);
    if let (Ok(root), Ok(directory)) = (canonical_root, canonical_directory) {
        if directory.parent() == Some(root.as_path())
            && directory.starts_with(&root)
            && directory
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("process-"))
        {
            let _ = fs::remove_dir_all(directory);
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

    #[test]
    fn process_directory_is_empty_and_confined() {
        let root =
            std::env::temp_dir().join(format!("openconkit-ai-root-{}", AnalysisRunId::new()));
        fs::create_dir(&root).expect("root");
        let directory = create_process_directory(&root).expect("process dir");
        assert_eq!(fs::read_dir(&directory).expect("read").count(), 0);
        let canonical_root = fs::canonicalize(&root).expect("canonical");
        assert_eq!(directory.parent(), Some(canonical_root.as_path()));
        cleanup_process_directory(&root, &directory);
        assert!(!directory.exists());
        let _ = fs::remove_dir(&root);
    }
}
