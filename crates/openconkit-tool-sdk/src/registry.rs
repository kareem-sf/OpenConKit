//! Compile-time tool registry.
//!
//! The registry is the shell's only view over the compiled-in tools: it
//! provides manifests, lookup by id, and the navigation model
//! ([`ToolNavItem`]) the shell renders. Registration is explicit and static —
//! adding a tool is a code change that goes through review, never a runtime
//! download.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::manifest::ToolManifest;
use crate::tool::Tool;
use crate::TOOL_CONTRACT_VERSION;

/// Errors that can occur while building a [`ToolRegistry`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RegistryError {
    /// A tool with the same id was already registered.
    #[error("duplicate tool id: {0:?}")]
    DuplicateToolId(String),

    /// Two tools attempted to mount the same shell route.
    #[error("duplicate tool route: {0:?}")]
    DuplicateRoute(String),

    /// A tool targets an incompatible host contract.
    #[error("tool {tool_id:?} targets contract {found}, but the host requires {required}")]
    ContractVersionMismatch {
        /// Tool being rejected.
        tool_id: String,
        /// Version declared by the tool.
        found: u32,
        /// Version required by this host.
        required: u32,
    },

    /// Manifest metadata is malformed or unsafe to mount.
    #[error("invalid manifest for tool {tool_id:?}: {message}")]
    InvalidManifest {
        /// Tool being rejected.
        tool_id: String,
        /// Validation detail.
        message: String,
    },

    /// Input capabilities are empty, malformed, or unbounded.
    #[error("invalid input capabilities for tool {tool_id:?}: {message}")]
    InvalidInputCapabilities {
        /// Tool being rejected.
        tool_id: String,
        /// Validation detail.
        message: String,
    },

    /// Permission declarations conflict with optional capabilities.
    #[error("invalid permission declaration for tool {tool_id:?}: {message}")]
    InvalidPermissions {
        /// Tool being rejected.
        tool_id: String,
        /// Validation detail.
        message: String,
    },

    /// Tool-owned migrations are not append-only and monotonic.
    #[error("invalid state migrations for tool {tool_id:?}: {message}")]
    InvalidStateMigrations {
        /// Tool being rejected.
        tool_id: String,
        /// Validation detail.
        message: String,
    },
}

/// One entry of the shell's tool navigation model.
///
/// Built exclusively from registered tools' manifests — shell navigation
/// data comes only from [`ToolRegistry::navigation`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct ToolNavItem {
    /// Id of the tool this entry routes to.
    pub tool_id: String,
    /// Shell route the tool is mounted at, e.g. `/tools/boq-inspector`.
    pub route: String,
    /// Icon reference (asset path or id) resolved by the shell.
    pub icon: String,
    /// i18n key for the tool's display name.
    pub name_key: String,
    /// i18n key for the tool's one-line description.
    pub description_key: String,
}

/// Registry of all tools compiled into the application.
#[derive(Default)]
pub struct ToolRegistry {
    tools: Vec<Box<dyn Tool>>,
}

impl ToolRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a tool. Called from shell composition code at startup.
    ///
    /// Validates the complete declaration before adding it. Any failure leaves
    /// the registry unchanged.
    pub fn register(&mut self, tool: Box<dyn Tool>) -> Result<(), RegistryError> {
        let manifest = tool.manifest();
        validate_tool_declaration(&*tool, &manifest)?;
        let id = manifest.id;
        if self
            .tools
            .iter()
            .any(|existing| existing.manifest().id == id)
        {
            return Err(RegistryError::DuplicateToolId(id));
        }
        if self
            .tools
            .iter()
            .any(|existing| existing.manifest().route == manifest.route)
        {
            return Err(RegistryError::DuplicateRoute(manifest.route));
        }
        self.tools.push(tool);
        Ok(())
    }

    /// Manifests of every registered tool, in registration order.
    pub fn manifests(&self) -> Vec<ToolManifest> {
        self.tools.iter().map(|tool| tool.manifest()).collect()
    }

    /// Look up a tool by its manifest id.
    pub fn get(&self, id: &str) -> Option<&dyn Tool> {
        self.tools
            .iter()
            .find(|tool| tool.manifest().id == id)
            .map(|tool| &**tool as &dyn Tool)
    }

    /// The shell's navigation model, in registration order.
    ///
    /// Shell navigation data comes only from here.
    pub fn navigation(&self) -> Vec<ToolNavItem> {
        self.tools
            .iter()
            .map(|tool| {
                let manifest = tool.manifest();
                ToolNavItem {
                    tool_id: manifest.id,
                    route: manifest.route,
                    icon: manifest.icon,
                    name_key: manifest.name_key,
                    description_key: manifest.description_key,
                }
            })
            .collect()
    }

    /// Number of registered tools.
    pub fn len(&self) -> usize {
        self.tools.len()
    }

    /// Whether the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }
}

fn validate_tool_declaration(
    tool: &dyn Tool,
    manifest: &ToolManifest,
) -> Result<(), RegistryError> {
    let tool_id = manifest.id.clone();
    if manifest.contract_version != TOOL_CONTRACT_VERSION {
        return Err(RegistryError::ContractVersionMismatch {
            tool_id,
            found: manifest.contract_version,
            required: TOOL_CONTRACT_VERSION,
        });
    }
    if !is_kebab_case(&manifest.id) {
        return Err(invalid_manifest(
            manifest,
            "id must be non-empty kebab-case",
        ));
    }
    if semver::Version::parse(&manifest.tool_version).is_err() {
        return Err(invalid_manifest(
            manifest,
            "tool_version must be valid semantic versioning",
        ));
    }
    let rule_set_version = tool.rule_set_version();
    if rule_set_version.is_empty()
        || rule_set_version.len() > 64
        || !rule_set_version
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
    {
        return Err(invalid_manifest(
            manifest,
            "rule_set_version must be non-empty safe ASCII",
        ));
    }
    let expected_route = format!("/tools/{}", manifest.id);
    if manifest.route != expected_route {
        return Err(invalid_manifest(
            manifest,
            format!("route must equal {expected_route:?}"),
        ));
    }
    for (field, value) in [
        ("name_key", manifest.name_key.as_str()),
        ("description_key", manifest.description_key.as_str()),
        ("icon", manifest.icon.as_str()),
    ] {
        if value.trim().is_empty() {
            return Err(invalid_manifest(
                manifest,
                format!("{field} must not be blank"),
            ));
        }
    }
    if !manifest.name_key.starts_with("tools.") || !manifest.name_key.ends_with(".name") {
        return Err(invalid_manifest(
            manifest,
            "name_key must match tools.<tool>.name",
        ));
    }
    if !manifest.description_key.starts_with("tools.")
        || !manifest.description_key.ends_with(".description")
    {
        return Err(invalid_manifest(
            manifest,
            "description_key must match tools.<tool>.description",
        ));
    }
    if manifest.icon.starts_with('/')
        || manifest.icon.contains('\\')
        || manifest.icon.split('/').any(|segment| segment == "..")
    {
        return Err(invalid_manifest(
            manifest,
            "icon must be a safe relative forward-slashed path",
        ));
    }

    let capabilities = tool.input_capabilities();
    if capabilities.max_file_size_bytes == 0 {
        return Err(invalid_capabilities(
            manifest,
            "max_file_size_bytes must be greater than zero",
        ));
    }
    if capabilities.accepted_extensions.is_empty() {
        return Err(invalid_capabilities(
            manifest,
            "at least one accepted extension is required",
        ));
    }
    let mut normalized_extensions = std::collections::BTreeSet::new();
    for extension in &capabilities.accepted_extensions {
        let valid = extension.starts_with('.')
            && extension.len() > 1
            && extension
                .chars()
                .skip(1)
                .all(|character| character.is_ascii_lowercase() || character.is_ascii_digit());
        if !valid {
            return Err(invalid_capabilities(
                manifest,
                format!("extension {extension:?} must be lowercase ASCII with a leading dot"),
            ));
        }
        if !normalized_extensions.insert(extension) {
            return Err(invalid_capabilities(
                manifest,
                format!("duplicate extension {extension:?}"),
            ));
        }
    }

    let permissions = tool.permissions();
    if permissions.network && !permissions.ai {
        return Err(RegistryError::InvalidPermissions {
            tool_id: manifest.id.clone(),
            message: "network access is allowed only for explicitly declared AI features"
                .to_string(),
        });
    }
    if permissions.ai != tool.ai_capability().is_some() {
        return Err(RegistryError::InvalidPermissions {
            tool_id: manifest.id.clone(),
            message: "ai permission must exactly match ai_capability()".to_string(),
        });
    }

    let mut previous = 0;
    for migration in tool.state_migrations() {
        if migration.version == 0 || migration.version <= previous {
            return Err(RegistryError::InvalidStateMigrations {
                tool_id: manifest.id.clone(),
                message: format!(
                    "version {} must be greater than previous version {previous}",
                    migration.version
                ),
            });
        }
        if migration.description.trim().is_empty() {
            return Err(RegistryError::InvalidStateMigrations {
                tool_id: manifest.id.clone(),
                message: format!("migration {} description is blank", migration.version),
            });
        }
        previous = migration.version;
    }
    Ok(())
}

fn invalid_manifest(manifest: &ToolManifest, message: impl Into<String>) -> RegistryError {
    RegistryError::InvalidManifest {
        tool_id: manifest.id.clone(),
        message: message.into(),
    }
}

fn invalid_capabilities(manifest: &ToolManifest, message: impl Into<String>) -> RegistryError {
    RegistryError::InvalidInputCapabilities {
        tool_id: manifest.id.clone(),
        message: message.into(),
    }
}

fn is_kebab_case(value: &str) -> bool {
    !value.is_empty()
        && !value.starts_with('-')
        && !value.ends_with('-')
        && !value.contains("--")
        && value.chars().all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-'
        })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use std::path::PathBuf;

    use crate::capabilities::{InputCapabilities, ToolPermissions};
    use crate::engine::{ToolEngine, ToolRunContext};
    use crate::error::ToolError;
    use crate::progress::{CancellationToken, ProgressCallback};
    use crate::state::ToolStateMigration;
    use crate::TOOL_CONTRACT_VERSION;

    struct NoopEngine;

    impl ToolEngine for NoopEngine {
        fn run(
            &self,
            _context: &ToolRunContext,
            _input: &serde_json::Value,
            _settings: &serde_json::Value,
            _progress: ProgressCallback<'_>,
            _cancel: &CancellationToken,
        ) -> Result<serde_json::Value, ToolError> {
            Ok(serde_json::Value::Null)
        }
    }

    struct DummyTool {
        id: &'static str,
        contract_version: u32,
        tool_version: &'static str,
        capabilities: InputCapabilities,
        permissions: ToolPermissions,
        migrations: &'static [ToolStateMigration],
        engine: NoopEngine,
    }

    impl DummyTool {
        fn new(id: &'static str) -> Self {
            Self {
                id,
                contract_version: TOOL_CONTRACT_VERSION,
                tool_version: "0.1.0",
                capabilities: InputCapabilities {
                    accepted_extensions: vec![".xlsx".to_string()],
                    max_file_size_bytes: 10 * 1024 * 1024,
                    accepts_multiple: false,
                },
                permissions: ToolPermissions {
                    reads_source_files: true,
                    writes_exports: false,
                    network: false,
                    ai: false,
                },
                migrations: &[],
                engine: NoopEngine,
            }
        }
    }

    impl Tool for DummyTool {
        fn manifest(&self) -> ToolManifest {
            ToolManifest {
                id: self.id.to_string(),
                contract_version: self.contract_version,
                tool_version: self.tool_version.to_string(),
                name_key: format!("tools.{}.name", self.id),
                description_key: format!("tools.{}.description", self.id),
                icon: format!("icons/{}.svg", self.id),
                route: format!("/tools/{}", self.id),
            }
        }

        fn input_capabilities(&self) -> InputCapabilities {
            self.capabilities.clone()
        }

        fn permissions(&self) -> ToolPermissions {
            self.permissions
        }

        fn rule_set_version(&self) -> &'static str {
            "test-rules"
        }

        fn engine(&self) -> &dyn ToolEngine {
            &self.engine
        }

        fn state_migrations(&self) -> &[ToolStateMigration] {
            self.migrations
        }
    }

    #[test]
    fn registry_lists_manifests_in_registration_order() {
        let mut registry = ToolRegistry::new();
        assert!(registry.is_empty());
        registry
            .register(Box::new(DummyTool::new("alpha")))
            .expect("registers");
        registry
            .register(Box::new(DummyTool::new("beta")))
            .expect("registers");

        assert_eq!(registry.len(), 2);
        let ids: Vec<String> = registry.manifests().into_iter().map(|m| m.id).collect();
        assert_eq!(ids, vec!["alpha".to_string(), "beta".to_string()]);
    }

    #[test]
    fn register_rejects_duplicate_ids_without_adding() {
        let mut registry = ToolRegistry::new();
        registry
            .register(Box::new(DummyTool::new("alpha")))
            .expect("registers");

        let err = registry
            .register(Box::new(DummyTool::new("alpha")))
            .expect_err("duplicate rejected");
        assert_eq!(err, RegistryError::DuplicateToolId("alpha".to_string()));
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn register_rejects_incompatible_contract_without_adding() {
        let mut registry = ToolRegistry::new();
        let mut tool = DummyTool::new("alpha");
        tool.contract_version = TOOL_CONTRACT_VERSION + 1;

        let error = registry
            .register(Box::new(tool))
            .expect_err("contract mismatch rejected");
        assert!(matches!(
            error,
            RegistryError::ContractVersionMismatch { .. }
        ));
        assert!(registry.is_empty());
    }

    #[test]
    fn register_rejects_malformed_manifest_and_capabilities() {
        let mut invalid_id = ToolRegistry::new();
        let error = invalid_id
            .register(Box::new(DummyTool::new("Not-Kebab")))
            .expect_err("invalid id rejected");
        assert!(matches!(error, RegistryError::InvalidManifest { .. }));

        let mut invalid_version = ToolRegistry::new();
        let mut tool = DummyTool::new("alpha");
        tool.tool_version = "version-one";
        let error = invalid_version
            .register(Box::new(tool))
            .expect_err("invalid semver rejected");
        assert!(matches!(error, RegistryError::InvalidManifest { .. }));

        let mut invalid_extension = ToolRegistry::new();
        let mut tool = DummyTool::new("alpha");
        tool.capabilities.accepted_extensions = vec!["XLSX".to_string()];
        let error = invalid_extension
            .register(Box::new(tool))
            .expect_err("invalid extension rejected");
        assert!(matches!(
            error,
            RegistryError::InvalidInputCapabilities { .. }
        ));
    }

    #[test]
    fn register_rejects_network_without_ai_and_non_monotonic_migrations() {
        let mut invalid_permissions = ToolRegistry::new();
        let mut tool = DummyTool::new("alpha");
        tool.permissions.network = true;
        let error = invalid_permissions
            .register(Box::new(tool))
            .expect_err("network permission rejected");
        assert!(matches!(error, RegistryError::InvalidPermissions { .. }));

        static INVALID_MIGRATIONS: [ToolStateMigration; 2] = [
            ToolStateMigration {
                version: 2,
                description: "second",
            },
            ToolStateMigration {
                version: 1,
                description: "first",
            },
        ];
        let mut invalid_migrations = ToolRegistry::new();
        let mut tool = DummyTool::new("alpha");
        tool.migrations = &INVALID_MIGRATIONS;
        let error = invalid_migrations
            .register(Box::new(tool))
            .expect_err("migration ordering rejected");
        assert!(matches!(
            error,
            RegistryError::InvalidStateMigrations { .. }
        ));
    }

    #[test]
    fn get_returns_tool_by_id() {
        let mut registry = ToolRegistry::new();
        registry
            .register(Box::new(DummyTool::new("alpha")))
            .expect("registers");

        let tool = registry.get("alpha").expect("found");
        assert_eq!(tool.manifest().id, "alpha");
        assert!(registry.get("missing").is_none());
    }

    #[test]
    fn navigation_is_built_from_manifests() {
        let mut registry = ToolRegistry::new();
        registry
            .register(Box::new(DummyTool::new("alpha")))
            .expect("registers");
        registry
            .register(Box::new(DummyTool::new("beta")))
            .expect("registers");

        let nav = registry.navigation();
        assert_eq!(
            nav,
            vec![
                ToolNavItem {
                    tool_id: "alpha".to_string(),
                    route: "/tools/alpha".to_string(),
                    icon: "icons/alpha.svg".to_string(),
                    name_key: "tools.alpha.name".to_string(),
                    description_key: "tools.alpha.description".to_string(),
                },
                ToolNavItem {
                    tool_id: "beta".to_string(),
                    route: "/tools/beta".to_string(),
                    icon: "icons/beta.svg".to_string(),
                    name_key: "tools.beta.name".to_string(),
                    description_key: "tools.beta.description".to_string(),
                },
            ]
        );
    }

    #[test]
    fn registered_engine_is_reachable_through_get() {
        let mut registry = ToolRegistry::new();
        registry
            .register(Box::new(DummyTool::new("alpha")))
            .expect("registers");

        let context = ToolRunContext {
            run_id: "run-1".to_string(),
            project_id: "project-1".to_string(),
            source_revision_id: "rev-1".to_string(),
            workbook_path: PathBuf::from("stored/workbook.xlsx"),
            app_version: "0.0.1".to_string(),
        };
        let output = registry
            .get("alpha")
            .expect("found")
            .engine()
            .run(
                &context,
                &serde_json::Value::Null,
                &serde_json::Value::Null,
                &|_| {},
                &CancellationToken::new(),
            )
            .expect("run succeeds");
        assert_eq!(output, serde_json::Value::Null);
    }
}
